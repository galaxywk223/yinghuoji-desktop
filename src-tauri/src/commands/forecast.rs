use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::sync::{LazyLock, Mutex};
use std::thread;

use anyhow::{anyhow, Result};
use chrono::{Duration, Local, NaiveDate};
use serde_json::{json, Value};
use smartcore::linalg::basic::arrays::Array;
use smartcore::linalg::basic::matrix::DenseMatrix;
use smartcore::linear::logistic_regression::{
    LogisticRegression, LogisticRegressionParameters,
};
use smartcore::xgboost::{XGRegressor, XGRegressorParameters};

use crate::{db, AppState};

pub const UNAVAILABLE_REASON: &str = "历史数据不足，暂不提供预测";
pub const MODEL_FAILURE_REASON: &str = "预测模型训练失败，暂不提供预测";
pub const LOW_CONFIDENCE_REASON: &str = "历史回测误差较高，已回退到保守基线预测";
pub const PENDING_FORECAST_REASON: &str = "预测计算中，请稍后刷新";
pub const FORECAST_ERROR_REASON: &str = "预测生成失败，请稍后重试";
pub const CONFIDENCE_LEVEL: f64 = 0.8;
pub const ACCURACY_GATE_WAPE: f64 = 0.4;
pub const MODEL_SELECTION_STRATEGY: &str = "lowest_wape_then_rmse_with_weighted_blend";
pub const FORECAST_DATASET_KEYS: [&str; 4] = [
    "daily_duration_data",
    "daily_efficiency_data",
    "weekly_duration_data",
    "weekly_efficiency_data",
];

const FORECAST_CACHE_DIRNAME: &str = "chart_forecasts";
const FORECAST_CACHE_KEY: &str = "local";
const FORECAST_READY_MESSAGE: &str = "预测结果已就绪";

const DAILY_LAGS: [usize; 4] = [1, 7, 14, 28];
const DAILY_WINDOWS: [usize; 3] = [7, 14, 28];
const WEEKLY_LAGS: [usize; 4] = [1, 2, 4, 8];
const WEEKLY_WINDOWS: [usize; 2] = [4, 8];
const DAILY_RECENT_WINDOW: usize = 56;
const WEEKLY_RECENT_WINDOW: usize = 16;

type BinaryLogisticRegression = LogisticRegression<f64, u32, DenseMatrix<f64>, Vec<u32>>;
type SmartcoreXGRegressor = XGRegressor<f64, f64, DenseMatrix<f64>, Vec<f64>>;

static FORECAST_CACHE: LazyLock<Mutex<HashMap<String, Value>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
static FORECAST_INFLIGHT: LazyLock<Mutex<HashSet<String>>> =
    LazyLock::new(|| Mutex::new(HashSet::new()));

#[derive(Clone, Copy)]
pub struct ForecastConfig {
    pub frequency: &'static str,
    pub horizon: usize,
    pub min_history: usize,
    pub season_length: usize,
    pub validation_window: usize,
    pub lags: &'static [usize],
    pub windows: &'static [usize],
    pub slope_window: usize,
}

pub const DAILY_CONFIG: ForecastConfig = ForecastConfig {
    frequency: "daily",
    horizon: 14,
    min_history: 28,
    season_length: 7,
    validation_window: 28,
    lags: &DAILY_LAGS,
    windows: &DAILY_WINDOWS,
    slope_window: 7,
};

pub const WEEKLY_CONFIG: ForecastConfig = ForecastConfig {
    frequency: "weekly",
    horizon: 8,
    min_history: 12,
    season_length: 4,
    validation_window: 8,
    lags: &WEEKLY_LAGS,
    windows: &WEEKLY_WINDOWS,
    slope_window: 4,
};

#[derive(Clone)]
pub struct TrendForecastRequest {
    pub daily_labels: Vec<String>,
    pub daily_duration_values: Vec<Option<f64>>,
    pub daily_efficiency_values: Vec<Option<f64>>,
    pub daily_stage_features: Vec<Vec<f64>>,
    pub daily_future_stage_features: Vec<Vec<f64>>,
    pub weekly_labels: Vec<String>,
    pub weekly_duration_values: Vec<Option<f64>>,
    pub weekly_efficiency_values: Vec<Option<f64>>,
    pub weekly_stage_features: Vec<Vec<f64>>,
    pub weekly_future_stage_features: Vec<Vec<f64>>,
    pub global_start_date: NaiveDate,
    pub last_log_date: NaiveDate,
    pub daily_current_label: Option<String>,
    pub weekly_current_label: Option<String>,
    pub weekly_duration_display_divisor: f64,
}

pub fn default_forecast(status: &str, reason: &str) -> Value {
    json!({
        "labels": Vec::<String>::new(),
        "prediction": Vec::<f64>::new(),
        "lower": Vec::<f64>::new(),
        "upper": Vec::<f64>::new(),
        "model_name": Value::Null,
        "history_points": 0,
        "horizon": 0,
        "trained_on": "all_history",
        "confidence_level": CONFIDENCE_LEVEL,
        "accuracy_threshold": ACCURACY_GATE_WAPE,
        "selection_strategy": MODEL_SELECTION_STRATEGY,
        "validation_wape": Value::Null,
        "validation_rmse": Value::Null,
        "baseline_wape": Value::Null,
        "baseline_rmse": Value::Null,
        "model_candidates": Vec::<Value>::new(),
        "fallback_from_model": Value::Null,
        "available": false,
        "reason": reason,
        "status": status,
    })
}

pub fn build_pending_forecast_bundle() -> Value {
    let mut bundle = serde_json::Map::new();
    for dataset_key in FORECAST_DATASET_KEYS {
        bundle.insert(
            dataset_key.to_string(),
            default_forecast("pending", PENDING_FORECAST_REASON),
        );
    }
    Value::Object(bundle)
}

pub fn build_unavailable_forecast_bundle(reason: &str) -> Value {
    let mut bundle = serde_json::Map::new();
    for dataset_key in FORECAST_DATASET_KEYS {
        bundle.insert(dataset_key.to_string(), default_forecast("unavailable", reason));
    }
    Value::Object(bundle)
}

pub fn mark_forecast_bundle_ready(forecast_bundle: Value) -> Value {
    let mut marked = serde_json::Map::new();
    for dataset_key in FORECAST_DATASET_KEYS {
        let mut forecast = forecast_bundle
            .get(dataset_key)
            .cloned()
            .unwrap_or_else(|| default_forecast("unavailable", ""));
        if let Some(object) = forecast.as_object_mut() {
            let available = object
                .get("available")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let current_status = object
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("unavailable");
            object.insert(
                "status".to_string(),
                Value::String(
                    if available {
                        if current_status == "conservative" {
                            "conservative"
                        } else {
                            "ready"
                        }
                    } else {
                        "unavailable"
                    }
                    .to_string(),
                ),
            );
        }
        marked.insert(dataset_key.to_string(), forecast);
    }
    Value::Object(marked)
}

pub fn attach_forecast_bundle(payload: &mut Value, forecast_bundle: &Value) {
    for dataset_key in FORECAST_DATASET_KEYS {
        if let Some(dataset) = payload.get_mut(dataset_key).and_then(Value::as_object_mut) {
            let forecast = forecast_bundle
                .get(dataset_key)
                .cloned()
                .unwrap_or_else(|| default_forecast("unavailable", ""));
            dataset.insert("forecast".to_string(), forecast);
        }
    }
}

pub fn resolve_forecast_entry(
    app_state: &AppState,
    signature: &str,
    request: TrendForecastRequest,
    force_sync: bool,
    force_retrain: bool,
) -> Result<Value> {
    let trained_for_date = Local::now().date_naive().format("%Y-%m-%d").to_string();
    if force_sync {
        let bundle = mark_forecast_bundle_ready(build_trend_forecasts(request)?);
        return Ok(json!({
            "signature": signature,
            "state": "ready",
            "message": FORECAST_READY_MESSAGE,
            "updated_at": db::now_utc_iso(),
            "trained_for_date": trained_for_date,
            "forecast_bundle": bundle,
        }));
    }

    if force_retrain {
        clear_persisted_forecast_entry(app_state);
        FORECAST_CACHE
            .lock()
            .map_err(|_| anyhow!("forecast cache lock poisoned"))?
            .remove(FORECAST_CACHE_KEY);
    }

    if let Some(entry) = cached_forecast_entry(signature, &trained_for_date)? {
        return Ok(entry);
    }
    if !force_retrain {
        if let Some(entry) = load_persisted_forecast_entry(app_state, signature, &trained_for_date)? {
            FORECAST_CACHE
                .lock()
                .map_err(|_| anyhow!("forecast cache lock poisoned"))?
                .insert(FORECAST_CACHE_KEY.to_string(), entry.clone());
            return Ok(entry);
        }
    }

    let pending_entry = json!({
        "signature": signature,
        "state": "pending",
        "message": PENDING_FORECAST_REASON,
        "updated_at": db::now_utc_iso(),
        "trained_for_date": trained_for_date,
        "forecast_bundle": build_pending_forecast_bundle(),
    });
    FORECAST_CACHE
        .lock()
        .map_err(|_| anyhow!("forecast cache lock poisoned"))?
        .insert(FORECAST_CACHE_KEY.to_string(), pending_entry.clone());

    let should_spawn = {
        let mut inflight = FORECAST_INFLIGHT
            .lock()
            .map_err(|_| anyhow!("forecast inflight lock poisoned"))?;
        inflight.insert(FORECAST_CACHE_KEY.to_string())
    };
    if should_spawn {
        let app_state = app_state.clone();
        let signature = signature.to_string();
        thread::spawn(move || {
            let result = build_trend_forecasts(request).map(mark_forecast_bundle_ready);
            let entry = match result {
                Ok(forecast_bundle) => json!({
                    "signature": signature,
                    "state": "ready",
                    "message": FORECAST_READY_MESSAGE,
                    "updated_at": db::now_utc_iso(),
                    "trained_for_date": trained_for_date,
                    "forecast_bundle": forecast_bundle,
                }),
                Err(_) => json!({
                    "signature": signature,
                    "state": "error",
                    "message": FORECAST_ERROR_REASON,
                    "updated_at": db::now_utc_iso(),
                    "trained_for_date": trained_for_date,
                    "forecast_bundle": {
                        "daily_duration_data": default_forecast("error", FORECAST_ERROR_REASON),
                        "daily_efficiency_data": default_forecast("error", FORECAST_ERROR_REASON),
                        "weekly_duration_data": default_forecast("error", FORECAST_ERROR_REASON),
                        "weekly_efficiency_data": default_forecast("error", FORECAST_ERROR_REASON),
                    },
                }),
            };
            let _ = store_forecast_entry(&app_state, entry);
            if let Ok(mut inflight) = FORECAST_INFLIGHT.lock() {
                inflight.remove(FORECAST_CACHE_KEY);
            }
        });
    }

    Ok(pending_entry)
}

#[derive(Clone)]
enum Model {
    SeasonalNaive,
    Ridge,
    RecentRidge { window: usize },
    LogXgBoost { window: Option<usize> },
    TwoStageDuration { window: Option<usize> },
    Blend { components: Vec<(Box<Model>, f64)> },
}

#[derive(Clone)]
struct Candidate {
    name: String,
    model: Model,
    wape: f64,
    rmse: f64,
    residuals: Vec<Vec<f64>>,
}

#[derive(Clone)]
struct RidgeModel {
    means: Vec<f64>,
    scales: Vec<f64>,
    weights: Vec<f64>,
    bias: f64,
}

pub fn build_trend_forecasts(request: TrendForecastRequest) -> Result<Value> {
    let daily_eff_seed = seed_forecast(&request.daily_efficiency_values, DAILY_CONFIG);
    let daily_duration = create_forecast(
        &request.daily_labels,
        &request.daily_duration_values,
        DAILY_CONFIG,
        request.global_start_date,
        request.last_log_date,
        request.daily_current_label.as_deref(),
        combine_exog(Some(&request.daily_efficiency_values), Some(&request.daily_stage_features)).as_deref(),
        combine_exog_from_scalars(Some(&daily_eff_seed), Some(&request.daily_future_stage_features)).as_deref(),
        1.0,
        "duration",
    )?;
    let daily_duration_pred = extract_forecast_numbers(&daily_duration["prediction"]);
    let daily_efficiency = create_forecast(
        &request.daily_labels,
        &request.daily_efficiency_values,
        DAILY_CONFIG,
        request.global_start_date,
        request.last_log_date,
        request.daily_current_label.as_deref(),
        combine_exog(Some(&request.daily_duration_values), Some(&request.daily_stage_features)).as_deref(),
        combine_exog_from_scalars(Some(&daily_duration_pred), Some(&request.daily_future_stage_features)).as_deref(),
        1.0,
        "efficiency",
    )?;

    let weekly_eff_seed = seed_forecast(&request.weekly_efficiency_values, WEEKLY_CONFIG);
    let weekly_duration = create_forecast(
        &request.weekly_labels,
        &request.weekly_duration_values,
        WEEKLY_CONFIG,
        request.global_start_date,
        request.last_log_date,
        request.weekly_current_label.as_deref(),
        combine_exog(Some(&request.weekly_efficiency_values), Some(&request.weekly_stage_features)).as_deref(),
        combine_exog_from_scalars(Some(&weekly_eff_seed), Some(&request.weekly_future_stage_features)).as_deref(),
        request.weekly_duration_display_divisor,
        "duration",
    )?;
    let weekly_duration_pred = extract_forecast_numbers(&weekly_duration["prediction"])
        .into_iter()
        .map(|v| v * request.weekly_duration_display_divisor.max(1.0))
        .collect::<Vec<_>>();
    let weekly_efficiency = create_forecast(
        &request.weekly_labels,
        &request.weekly_efficiency_values,
        WEEKLY_CONFIG,
        request.global_start_date,
        request.last_log_date,
        request.weekly_current_label.as_deref(),
        combine_exog(Some(&request.weekly_duration_values), Some(&request.weekly_stage_features)).as_deref(),
        combine_exog_from_scalars(Some(&weekly_duration_pred), Some(&request.weekly_future_stage_features)).as_deref(),
        1.0,
        "efficiency",
    )?;

    Ok(json!({
        "daily_duration_data": daily_duration,
        "daily_efficiency_data": daily_efficiency,
        "weekly_duration_data": weekly_duration,
        "weekly_efficiency_data": weekly_efficiency,
    }))
}

fn create_forecast(
    labels: &[String],
    series: &[Option<f64>],
    config: ForecastConfig,
    global_start_date: NaiveDate,
    last_log_date: NaiveDate,
    current_label: Option<&str>,
    exog_history: Option<&[Vec<f64>]>,
    future_exog: Option<&[Vec<f64>]>,
    display_divisor: f64,
    target_kind: &str,
) -> Result<Value> {
    let history_points = series.len();
    let future_labels = future_labels(labels, config, global_start_date, last_log_date, current_label)?;
    let history = series.iter().map(|v| v.unwrap_or(0.0)).collect::<Vec<_>>();
    if history_points < config.min_history {
        return Ok(empty_forecast(&future_labels, config.horizon, history_points, UNAVAILABLE_REASON, None, None, None, None, None, vec![], "unavailable"));
    }

    let recent_window = recent_window(config);
    let mut candidates = Vec::new();
    for (name, model) in [
        ("Seasonal Naive", Model::SeasonalNaive),
        ("Ridge Autoregression", Model::Ridge),
        ("Recent Ridge Autoregression", Model::RecentRidge { window: recent_window }),
    ] {
        push_candidate(&mut candidates, name, model, &history, config, exog_history);
    }
    if target_kind == "duration" {
        for (name, model) in [
            ("Two-Stage Duration Autoregression", Model::TwoStageDuration { window: None }),
            ("Log-XGBoost Autoregression", Model::LogXgBoost { window: None }),
            ("Recent Log-XGBoost Autoregression", Model::LogXgBoost { window: Some(recent_window) }),
        ] {
            push_candidate(&mut candidates, name, model, &history, config, exog_history);
        }
    } else {
        for (name, model) in [
            ("Log-XGBoost Autoregression", Model::LogXgBoost { window: None }),
            ("Recent Log-XGBoost Autoregression", Model::LogXgBoost { window: Some(recent_window) }),
        ] {
            push_candidate(&mut candidates, name, model, &history, config, exog_history);
        }
    }
    let ranked = ranked_candidates(&candidates);
    if ranked.len() >= 2 {
        let blend = Model::Blend {
            components: ranked.iter().take(3).map(|c| (Box::new(c.model.clone()), 1.0 / c.wape.max(0.05))).collect(),
        };
        if let Ok((wape, rmse, residuals)) = backtest(&blend, &history, config, exog_history) {
            candidates.push(Candidate { name: "Weighted Blend".to_string(), model: blend, wape, rmse, residuals });
        }
    }
    if candidates.is_empty() {
        return Ok(empty_forecast(&future_labels, config.horizon, history_points, MODEL_FAILURE_REASON, None, None, None, None, None, vec![], "unavailable"));
    }

    let ranked = ranked_candidates(&candidates);
    let selected = ranked.first().cloned().ok_or_else(|| anyhow!("no forecast candidate selected"))?;
    let baseline = candidates.iter().find(|c| c.name == "Seasonal Naive");
    let serialized = serialize_candidates(&candidates, &selected.name);
    finalize_selected_forecast(
        &future_labels,
        &history,
        history_points,
        config,
        &selected,
        baseline,
        serialized,
        display_divisor,
        exog_history,
        future_exog,
    )
}

fn empty_forecast(labels: &[String], horizon: usize, history_points: usize, reason: &str, model_name: Option<String>, validation_wape: Option<f64>, validation_rmse: Option<f64>, baseline_wape: Option<f64>, baseline_rmse: Option<f64>, model_candidates: Vec<Value>, status: &str) -> Value {
    json!({
        "labels": labels,
        "prediction": Vec::<f64>::new(),
        "lower": Vec::<f64>::new(),
        "upper": Vec::<f64>::new(),
        "model_name": model_name,
        "history_points": history_points,
        "horizon": horizon,
        "trained_on": "all_history",
        "confidence_level": CONFIDENCE_LEVEL,
        "accuracy_threshold": ACCURACY_GATE_WAPE,
        "selection_strategy": MODEL_SELECTION_STRATEGY,
        "validation_wape": round_metric(validation_wape),
        "validation_rmse": round_metric(validation_rmse),
        "baseline_wape": round_metric(baseline_wape),
        "baseline_rmse": round_metric(baseline_rmse),
        "model_candidates": model_candidates,
        "fallback_from_model": Value::Null,
        "available": false,
        "reason": reason,
        "status": status,
    })
}

fn populated_forecast(
    labels: &[String],
    prediction: &[f64],
    lower: &[f64],
    upper: &[f64],
    history_points: usize,
    horizon: usize,
    model_name: &str,
    validation_wape: f64,
    validation_rmse: f64,
    baseline_wape: f64,
    baseline_rmse: f64,
    model_candidates: Vec<Value>,
    display_divisor: f64,
    reason: &str,
    status: &str,
    fallback_from_model: Option<&str>,
) -> Value {
    let divisor = display_divisor.max(1.0);
    json!({
        "labels": labels,
        "prediction": prediction.iter().map(|v| round2((v / divisor).max(0.0))).collect::<Vec<_>>(),
        "lower": lower.iter().map(|v| round2((v / divisor).max(0.0))).collect::<Vec<_>>(),
        "upper": upper.iter().map(|v| round2((v / divisor).max(0.0))).collect::<Vec<_>>(),
        "model_name": model_name,
        "history_points": history_points,
        "horizon": horizon,
        "trained_on": "all_history",
        "confidence_level": CONFIDENCE_LEVEL,
        "accuracy_threshold": ACCURACY_GATE_WAPE,
        "selection_strategy": MODEL_SELECTION_STRATEGY,
        "validation_wape": round_metric(Some(validation_wape)),
        "validation_rmse": round_metric(Some(validation_rmse / divisor)),
        "baseline_wape": round_metric(Some(baseline_wape)),
        "baseline_rmse": round_metric(Some(baseline_rmse / divisor)),
        "model_candidates": model_candidates,
        "fallback_from_model": fallback_from_model,
        "available": true,
        "reason": reason,
        "status": status,
    })
}

fn finalize_selected_forecast(
    future_labels: &[String],
    history: &[f64],
    history_points: usize,
    config: ForecastConfig,
    selected: &Candidate,
    baseline: Option<&Candidate>,
    serialized: Vec<Value>,
    display_divisor: f64,
    exog_history: Option<&[Vec<f64>]>,
    future_exog: Option<&[Vec<f64>]>,
) -> Result<Value> {
    if selected.wape > ACCURACY_GATE_WAPE {
        if let Some(baseline) = baseline {
            let prediction = run_model(
                &baseline.model,
                history,
                config,
                config.horizon,
                exog_history,
                future_exog,
            )?;
            let (lower, upper) = intervals(&prediction, &baseline.residuals);
            let fallback_from_model = if baseline.name == selected.name {
                None
            } else {
                Some(selected.name.as_str())
            };
            return Ok(populated_forecast(
                future_labels,
                &prediction,
                &lower,
                &upper,
                history_points,
                config.horizon,
                baseline.name.as_str(),
                baseline.wape,
                baseline.rmse,
                baseline.wape,
                baseline.rmse,
                serialized,
                display_divisor,
                LOW_CONFIDENCE_REASON,
                "conservative",
                fallback_from_model,
            ));
        }
        return Ok(empty_forecast(
            future_labels,
            config.horizon,
            history_points,
            LOW_CONFIDENCE_REASON,
            Some(selected.name.clone()),
            Some(selected.wape),
            Some(selected.rmse / display_divisor.max(1.0)),
            baseline.map(|candidate| candidate.wape),
            baseline.map(|candidate| candidate.rmse / display_divisor.max(1.0)),
            serialized,
            "unavailable",
        ));
    }

    let prediction = run_model(
        &selected.model,
        history,
        config,
        config.horizon,
        exog_history,
        future_exog,
    )?;
    let (lower, upper) = intervals(&prediction, &selected.residuals);
    Ok(populated_forecast(
        future_labels,
        &prediction,
        &lower,
        &upper,
        history_points,
        config.horizon,
        selected.name.as_str(),
        selected.wape,
        selected.rmse,
        baseline.map(|candidate| candidate.wape).unwrap_or(selected.wape),
        baseline.map(|candidate| candidate.rmse).unwrap_or(selected.rmse),
        serialized,
        display_divisor,
        "",
        "ready",
        None,
    ))
}

fn ranked_candidates(items: &[Candidate]) -> Vec<Candidate> {
    let mut out = items.to_vec();
    out.sort_by(|a, b| a.wape.partial_cmp(&b.wape).unwrap_or(Ordering::Equal).then_with(|| a.rmse.partial_cmp(&b.rmse).unwrap_or(Ordering::Equal)).then_with(|| a.name.cmp(&b.name)));
    out
}

fn serialize_candidates(items: &[Candidate], selected_name: &str) -> Vec<Value> {
    ranked_candidates(items).into_iter().map(|c| json!({
        "model_name": c.name,
        "validation_wape": round_metric(Some(c.wape)),
        "validation_rmse": round_metric(Some(c.rmse)),
        "selected": c.name == selected_name,
    })).collect()
}

fn future_labels(labels: &[String], config: ForecastConfig, global_start_date: NaiveDate, last_log_date: NaiveDate, current_label: Option<&str>) -> Result<Vec<String>> {
    if labels.is_empty() && current_label.is_none() { return Ok(Vec::new()); }
    let mut out = Vec::new();
    let mut remaining = config.horizon;
    let anchor = current_label.or_else(|| labels.last().map(|s| s.as_str()));
    if let Some(label) = current_label { out.push(label.to_string()); remaining = remaining.saturating_sub(1); }
    if config.frequency == "daily" {
        let last_date = anchor.ok_or_else(|| anyhow!("missing daily forecast anchor")) .and_then(db::parse_date)?;
        for step in 1..=remaining { out.push((last_date + Duration::days(step as i64)).format("%Y-%m-%d").to_string()); }
        return Ok(out);
    }
    for step in 1..=remaining {
        let day = last_log_date + Duration::days((step as i64) * 7);
        let (_, _, year, week_num) = db::get_custom_week_window(day, global_start_date);
        out.push(format!("{year}-W{week_num:02}"));
    }
    Ok(out)
}

fn backtest(model: &Model, series: &[f64], config: ForecastConfig, exog_history: Option<&[Vec<f64>]>) -> Result<(f64, f64, Vec<Vec<f64>>)> {
    if series.len() < config.min_history { return Err(anyhow!("insufficient history")); }
    let start_origin = config.min_history.max(series.len().saturating_sub(config.validation_window));
    let mut actual = Vec::new();
    let mut predicted = Vec::new();
    let mut residuals = vec![Vec::<f64>::new(); config.horizon];
    for origin in start_origin..series.len() {
        let steps = config.horizon.min(series.len() - origin);
        if steps == 0 { continue; }
        let train_exog = exog_history.map(|rows| rows[..origin].to_vec());
        let future_exog = exog_history.map(|rows| rows[origin..origin + steps].to_vec());
        let pred = run_model(model, &series[..origin], config, steps, train_exog.as_deref(), future_exog.as_deref())?;
        let act = &series[origin..origin + steps];
        actual.extend_from_slice(act);
        predicted.extend(pred.iter().copied());
        for (i, (p, a)) in pred.iter().zip(act.iter()).enumerate() { residuals[i].push(*a - *p); }
    }
    if actual.is_empty() { return Err(anyhow!("empty backtest output")); }
    Ok((wape(&actual, &predicted), rmse(&actual, &predicted), residuals))
}

fn run_model(model: &Model, series: &[f64], config: ForecastConfig, horizon: usize, exog_history: Option<&[Vec<f64>]>, future_exog: Option<&[Vec<f64>]>) -> Result<Vec<f64>> {
    match model {
        Model::SeasonalNaive => seasonal_naive(series, config, horizon),
        Model::Ridge => ridge_predict(series, config, horizon, exog_history, future_exog),
        Model::RecentRidge { window } => {
            let (recent_series, recent_exog) = slice_recent_history(series, exog_history, *window);
            ridge_predict(&recent_series, config, horizon, recent_exog.as_deref(), future_exog)
        }
        Model::LogXgBoost { window } => {
            let (recent_series, recent_exog) = slice_history_for_window(series, exog_history, *window);
            log_xgboost_predict(&recent_series, config, horizon, recent_exog.as_deref(), future_exog)
        }
        Model::TwoStageDuration { window } => {
            let (recent_series, recent_exog) = slice_history_for_window(series, exog_history, *window);
            two_stage_duration_predict(&recent_series, config, horizon, recent_exog.as_deref(), future_exog)
        }
        Model::Blend { components } => {
            let total = components.iter().map(|(_, w)| *w).sum::<f64>().max(1e-6);
            let mut out = vec![0.0; horizon];
            for (candidate, weight) in components {
                let pred = run_model(candidate, series, config, horizon, exog_history, future_exog)?;
                for (i, value) in pred.iter().enumerate() { out[i] += *value * (*weight / total); }
            }
            Ok(out)
        }
    }
}

fn seasonal_naive(series: &[f64], config: ForecastConfig, horizon: usize) -> Result<Vec<f64>> {
    if series.len() < config.season_length { return Err(anyhow!("insufficient history for seasonal naive")); }
    Ok((1..=horizon).map(|step| series[series.len() - config.season_length + ((step - 1) % config.season_length)]).collect())
}

fn ridge_predict(series: &[f64], config: ForecastConfig, horizon: usize, exog_history: Option<&[Vec<f64>]>, future_exog: Option<&[Vec<f64>]>) -> Result<Vec<f64>> {
    let (x, y) = supervised_dataset(series, config, exog_history)?;
    if y.len() < (config.min_history / 2).max(8) { return Err(anyhow!("insufficient ridge rows")); }
    let weights = sample_weights(y.len());
    let model = fit_ridge(&x, &y, &weights, 1.2)?;
    let exog = extend_exog(exog_history, future_exog, horizon);
    let mut history = series.to_vec();
    let mut out = Vec::with_capacity(horizon);
    for _ in 0..horizon {
        let row = feature_row(&history, history.len(), config, exog.as_deref())?;
        let value = model.predict(&row).max(0.0);
        out.push(value);
        history.push(value);
    }
    Ok(out)
}

fn log_xgboost_predict(series: &[f64], config: ForecastConfig, horizon: usize, exog_history: Option<&[Vec<f64>]>, future_exog: Option<&[Vec<f64>]>) -> Result<Vec<f64>> {
    let transformed = series.iter().map(|value| value.max(0.0).ln_1p()).collect::<Vec<_>>();
    let (x, y) = supervised_dataset(&transformed, config, exog_history)?;
    if y.len() < (config.min_history / 2).max(8) {
        return Err(anyhow!("insufficient xgboost rows"));
    }
    let model = fit_xgboost(&x, &y)?;
    let exog = extend_exog(exog_history, future_exog, horizon);
    let mut history = transformed;
    let mut out = Vec::with_capacity(horizon);
    for _ in 0..horizon {
        let row = feature_row(&history, history.len(), config, exog.as_deref())?;
        let row_matrix = DenseMatrix::from_2d_vec(&vec![row])?;
        let predicted_log = model.predict(&row_matrix)?.into_iter().next().unwrap_or(0.0);
        let value = predicted_log.max(0.0).exp_m1().max(0.0);
        out.push(value);
        history.push(predicted_log.max(0.0));
    }
    Ok(out)
}

fn two_stage_duration_predict(series: &[f64], config: ForecastConfig, horizon: usize, exog_history: Option<&[Vec<f64>]>, future_exog: Option<&[Vec<f64>]>) -> Result<Vec<f64>> {
    let (rows, target) = supervised_dataset(series, config, exog_history)?;
    if target.len() < (config.min_history / 2).max(8) {
        return Err(anyhow!("insufficient two-stage rows"));
    }

    let threshold = activity_threshold(config);
    let active_labels = target
        .iter()
        .map(|value| u32::from(*value > threshold))
        .collect::<Vec<_>>();
    let active_rate = active_labels.iter().map(|value| *value as f64).sum::<f64>() / active_labels.len().max(1) as f64;
    let active_rows = rows
        .iter()
        .zip(target.iter())
        .filter(|(_, value)| **value > threshold)
        .map(|(row, value)| (row.clone(), value.ln_1p()))
        .collect::<Vec<_>>();
    let inactive_values = target.iter().copied().filter(|value| *value <= threshold).collect::<Vec<_>>();
    let inactive_level = if inactive_values.is_empty() { 0.0 } else { avg(&inactive_values) };

    let classifier = fit_active_classifier(&rows, &active_labels)?;
    let intensity_model = fit_active_intensity_model(&active_rows)?;
    let active_default = if active_rows.is_empty() {
        0.0
    } else {
        avg(&active_rows.iter().map(|(_, value)| value.exp_m1()).collect::<Vec<_>>())
    };

    let exog = extend_exog(exog_history, future_exog, horizon);
    let mut history = series.to_vec();
    let mut out = Vec::with_capacity(horizon);
    for _ in 0..horizon {
        let row = feature_row(&history, history.len(), config, exog.as_deref())?;
        let active_prob = predict_active_probability(&classifier, &row, active_rate)?;
        let active_intensity = predict_active_intensity(&intensity_model, &row, active_default)?;
        let value = ((active_prob * active_intensity) + ((1.0 - active_prob) * inactive_level)).max(0.0);
        out.push(value);
        history.push(value);
    }
    Ok(out)
}

fn push_candidate(
    candidates: &mut Vec<Candidate>,
    name: &str,
    model: Model,
    history: &[f64],
    config: ForecastConfig,
    exog_history: Option<&[Vec<f64>]>,
) {
    if let Ok((wape, rmse, residuals)) = backtest(&model, history, config, exog_history) {
        if wape.is_finite() && rmse.is_finite() {
            candidates.push(Candidate {
                name: name.to_string(),
                model,
                wape,
                rmse,
                residuals,
            });
        }
    }
}

fn recent_window(config: ForecastConfig) -> usize {
    if config.frequency == "weekly" {
        WEEKLY_RECENT_WINDOW
    } else {
        DAILY_RECENT_WINDOW
    }
}

fn activity_threshold(config: ForecastConfig) -> f64 {
    if config.frequency == "daily" { 0.25 } else { 1.0 }
}

fn slice_history_for_window(
    series: &[f64],
    exog_history: Option<&[Vec<f64>]>,
    window: Option<usize>,
) -> (Vec<f64>, Option<Vec<Vec<f64>>>) {
    if let Some(window) = window {
        slice_recent_history(series, exog_history, window)
    } else {
        (series.to_vec(), exog_history.map(|rows| rows.to_vec()))
    }
}

fn slice_recent_history(
    series: &[f64],
    exog_history: Option<&[Vec<f64>]>,
    window: usize,
) -> (Vec<f64>, Option<Vec<Vec<f64>>>) {
    let width = window.min(series.len());
    let offset = series.len().saturating_sub(width);
    (
        series[offset..].to_vec(),
        exog_history.map(|rows| rows[offset..].to_vec()),
    )
}

fn fit_xgboost(rows: &[Vec<f64>], target: &[f64]) -> Result<SmartcoreXGRegressor> {
    if rows.is_empty() || target.is_empty() {
        return Err(anyhow!("empty xgboost input"));
    }
    let matrix = DenseMatrix::from_2d_vec(&rows.to_vec())?;
    let params = XGRegressorParameters::default()
        .with_n_estimators(80)
        .with_max_depth(3)
        .with_learning_rate(0.08)
        .with_subsample(0.9)
        .with_lambda(1.5)
        .with_gamma(0.0)
        .with_min_child_weight(1);
    Ok(XGRegressor::fit(&matrix, &target.to_vec(), params)?)
}

fn fit_active_classifier(rows: &[Vec<f64>], labels: &[u32]) -> Result<Option<BinaryLogisticRegression>> {
    if rows.is_empty() || labels.is_empty() || labels.iter().all(|value| *value == labels[0]) {
        return Ok(None);
    }
    let matrix = DenseMatrix::from_2d_vec(&rows.to_vec())?;
    let params = LogisticRegressionParameters::default().with_alpha(0.1);
    Ok(Some(LogisticRegression::fit(&matrix, &labels.to_vec(), params)?))
}

fn fit_active_intensity_model(active_rows: &[(Vec<f64>, f64)]) -> Result<Option<SmartcoreXGRegressor>> {
    if active_rows.len() < 6 {
        return Ok(None);
    }
    let rows = active_rows.iter().map(|(row, _)| row.clone()).collect::<Vec<_>>();
    let target = active_rows.iter().map(|(_, value)| *value).collect::<Vec<_>>();
    Ok(Some(fit_xgboost(&rows, &target)?))
}

fn predict_active_probability(
    classifier: &Option<BinaryLogisticRegression>,
    row: &[f64],
    empirical_prob: f64,
) -> Result<f64> {
    let Some(classifier) = classifier else {
        return Ok(empirical_prob.clamp(0.0, 1.0));
    };
    let score = row
        .iter()
        .enumerate()
        .fold(*classifier.intercept().get((0, 0)), |acc, (index, value)| {
            acc + (*classifier.coefficients().get((0, index)) * *value)
        });
    Ok((1.0 / (1.0 + (-score).exp())).clamp(0.0, 1.0))
}

fn predict_active_intensity(
    model: &Option<SmartcoreXGRegressor>,
    row: &[f64],
    fallback: f64,
) -> Result<f64> {
    let Some(model) = model else {
        return Ok(fallback.max(0.0));
    };
    let row_matrix = DenseMatrix::from_2d_vec(&vec![row.to_vec()])?;
    let predicted = model.predict(&row_matrix)?.into_iter().next().unwrap_or(0.0);
    Ok(predicted.max(0.0).exp_m1().max(0.0))
}

fn supervised_dataset(series: &[f64], config: ForecastConfig, exog: Option<&[Vec<f64>]>) -> Result<(Vec<Vec<f64>>, Vec<f64>)> {
    let start = max_lookback(config);
    let mut rows = Vec::new();
    let mut target = Vec::new();
    for index in start..series.len() {
        rows.push(feature_row(series, index, config, exog)?);
        target.push(series[index]);
    }
    Ok((rows, target))
}

fn feature_row(series: &[f64], index: usize, config: ForecastConfig, exog: Option<&[Vec<f64>]>) -> Result<Vec<f64>> {
    let mut row = Vec::new();
    for lag in config.lags { row.push(series[index - lag]); }
    for window in config.windows {
        let slice = &series[index - window..index];
        row.push(avg(slice));
        row.push(stdev(slice));
        row.push(slice.iter().copied().fold(f64::MIN, f64::max));
        row.push(slice.iter().copied().fold(f64::MAX, f64::min));
    }
    let slope_slice = &series[index - config.slope_window..index];
    row.push(slope(slope_slice));
    let refs = seasonal_refs(series, index, config.season_length);
    row.push(avg(&refs));
    row.push(stdev(&refs));
    row.push(*refs.first().unwrap_or(&0.0));
    let threshold = activity_threshold(config);
    let active = &series[index.saturating_sub(config.season_length)..index];
    row.push(if active.is_empty() { 0.0 } else { active.iter().filter(|v| **v > threshold).count() as f64 / active.len() as f64 });
    row.extend(calendar(index, config));
    if let Some(rows) = exog {
        if !rows.is_empty() {
            let features = rows[0].len();
            for feature_index in 0..features {
                let column = rows
                    .iter()
                    .map(|r| r.iter().nth(feature_index).copied().unwrap_or(0.0))
                    .collect::<Vec<_>>();
                row.push(column.iter().nth(index).copied().unwrap_or(0.0));
                row.push(
                    column
                        .iter()
                        .nth(index.saturating_sub(1))
                        .copied()
                        .unwrap_or(0.0),
                );
                for window in config.windows {
                    let slice = &column[index - window..index];
                    row.push(avg(slice));
                    row.push(stdev(slice));
                }
            }
        }
    }
    Ok(row)
}

fn fit_ridge(rows: &[Vec<f64>], target: &[f64], sample_weights: &[f64], alpha: f64) -> Result<RidgeModel> {
    if rows.is_empty() || target.is_empty() { return Err(anyhow!("empty ridge input")); }
    let cols = rows[0].len();
    let weight_sum = sample_weights.iter().sum::<f64>().max(1e-6);
    let mut means = vec![0.0; cols];
    let mut scales = vec![1.0; cols];
    for col in 0..cols {
        means[col] = rows.iter().zip(sample_weights.iter()).map(|(r, w)| r[col] * *w).sum::<f64>() / weight_sum;
        let variance = rows.iter().zip(sample_weights.iter()).map(|(r, w)| { let d = r[col] - means[col]; d * d * *w }).sum::<f64>() / weight_sum;
        let scale = variance.sqrt();
        scales[col] = if scale.is_finite() && scale > 1e-6 { scale } else { 1.0 };
    }
    let x = rows.iter().map(|r| r.iter().enumerate().map(|(i, v)| (v - means[i]) / scales[i]).collect::<Vec<_>>()).collect::<Vec<_>>();
    let mut weights = vec![0.0; cols];
    let mut bias = target.iter().zip(sample_weights.iter()).map(|(y, w)| y * *w).sum::<f64>() / weight_sum;
    for _ in 0..800 {
        let mut grad_w = vec![0.0; cols];
        let mut grad_b = 0.0;
        for (row, (y, w)) in x.iter().zip(target.iter().zip(sample_weights.iter())) {
            let err = bias + dot(&weights, row) - *y;
            grad_b += 2.0 * *w * err;
            for col in 0..cols { grad_w[col] += 2.0 * *w * err * row[col]; }
        }
        grad_b /= weight_sum;
        for col in 0..cols {
            grad_w[col] = (grad_w[col] / weight_sum) + (2.0 * alpha * weights[col]);
            weights[col] -= 0.03 * grad_w[col];
        }
        bias -= 0.03 * grad_b;
    }
    Ok(RidgeModel { means, scales, weights, bias })
}

impl RidgeModel {
    fn predict(&self, row: &[f64]) -> f64 {
        let x = row.iter().enumerate().map(|(i, v)| (v - self.means[i]) / self.scales[i]).collect::<Vec<_>>();
        self.bias + dot(&self.weights, &x)
    }
}

fn extend_exog(history: Option<&[Vec<f64>]>, future: Option<&[Vec<f64>]>, horizon: usize) -> Option<Vec<Vec<f64>>> {
    let hist = history?.to_vec();
    if hist.is_empty() { return None; }
    let fallback = hist.last().cloned().unwrap_or_default();
    let width = fallback.len();
    let mut out = hist.clone();
    for index in 0..horizon {
        let row = future.and_then(|rows| rows.get(index)).cloned().unwrap_or_else(|| fallback.clone());
        out.push(if row.len() == width { row } else { fallback.clone() });
    }
    Some(out)
}

fn intervals(prediction: &[f64], residuals: &[Vec<f64>]) -> (Vec<f64>, Vec<f64>) {
    let flat = residuals.iter().flat_map(|v| v.iter().copied()).collect::<Vec<_>>();
    let mut lower = Vec::with_capacity(prediction.len());
    let mut upper = Vec::with_capacity(prediction.len());
    for (index, value) in prediction.iter().enumerate() {
        let bucket = residuals.get(index).filter(|b| !b.is_empty()).cloned().unwrap_or_else(|| flat.clone());
        let lo = (*value + quantile(&bucket, 0.1)).max(0.0);
        let hi = (*value + quantile(&bucket, 0.9)).max(lo);
        lower.push(lo);
        upper.push(hi);
    }
    (lower, upper)
}

fn combine_exog(scalars: Option<&[Option<f64>]>, features: Option<&[Vec<f64>]>) -> Option<Vec<Vec<f64>>> {
    let len = scalars.map(|v| v.len()).unwrap_or(0).max(features.map(|v| v.len()).unwrap_or(0));
    if len == 0 { return None; }
    let mut rows = Vec::with_capacity(len);
    for index in 0..len {
        let mut row = Vec::new();
        if let Some(values) = scalars { row.push(values.get(index).and_then(|v| *v).unwrap_or(0.0)); }
        if let Some(items) = features { if let Some(feature_row) = items.get(index) { row.extend(feature_row.iter().copied()); } }
        rows.push(row);
    }
    Some(rows)
}

fn combine_exog_from_scalars(scalars: Option<&[f64]>, features: Option<&[Vec<f64>]>) -> Option<Vec<Vec<f64>>> {
    let len = scalars.map(|v| v.len()).unwrap_or(0).max(features.map(|v| v.len()).unwrap_or(0));
    if len == 0 { return None; }
    let mut rows = Vec::with_capacity(len);
    for index in 0..len {
        let mut row = Vec::new();
        if let Some(values) = scalars { row.push(values.iter().nth(index).copied().unwrap_or(0.0)); }
        if let Some(items) = features { if let Some(feature_row) = items.get(index) { row.extend(feature_row.iter().copied()); } }
        rows.push(row);
    }
    Some(rows)
}

fn seed_forecast(series: &[Option<f64>], config: ForecastConfig) -> Vec<f64> {
    let numeric = series.iter().map(|v| v.unwrap_or(0.0)).collect::<Vec<_>>();
    if numeric.is_empty() { return vec![0.0; config.horizon]; }
    if numeric.len() >= config.season_length {
        seasonal_naive(&numeric, config, config.horizon).unwrap_or_else(|_| vec![*numeric.last().unwrap_or(&0.0); config.horizon])
    } else {
        vec![*numeric.last().unwrap_or(&0.0); config.horizon]
    }
}

fn extract_forecast_numbers(value: &Value) -> Vec<f64> {
    value.as_array().cloned().unwrap_or_default().into_iter().map(|v| v.as_f64().unwrap_or(0.0)).collect()
}

fn cached_forecast_entry(signature: &str, trained_for_date: &str) -> Result<Option<Value>> {
    let cache = FORECAST_CACHE
        .lock()
        .map_err(|_| anyhow!("forecast cache lock poisoned"))?;
    Ok(cache
        .get(FORECAST_CACHE_KEY)
        .cloned()
        .filter(|entry| entry_matches(entry, signature, trained_for_date)))
}

fn load_persisted_forecast_entry(
    app_state: &AppState,
    signature: &str,
    trained_for_date: &str,
) -> Result<Option<Value>> {
    let path = forecast_cache_file(app_state)?;
    if !path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(path)?;
    let payload: Value = serde_json::from_str(&raw)?;
    Ok(entry_matches(&payload, signature, trained_for_date).then_some(payload))
}

fn store_forecast_entry(app_state: &AppState, entry: Value) -> Result<()> {
    let state = entry
        .get("state")
        .and_then(Value::as_str)
        .unwrap_or("unavailable");
    FORECAST_CACHE
        .lock()
        .map_err(|_| anyhow!("forecast cache lock poisoned"))?
        .insert(FORECAST_CACHE_KEY.to_string(), entry.clone());
    if state == "ready" {
        persist_forecast_entry(app_state, &entry)?;
    }
    Ok(())
}

fn persist_forecast_entry(app_state: &AppState, entry: &Value) -> Result<()> {
    let path = forecast_cache_file(app_state)?;
    let payload = serde_json::to_string_pretty(entry)?;
    fs::write(path, payload)?;
    Ok(())
}

fn clear_persisted_forecast_entry(app_state: &AppState) {
    if let Ok(path) = forecast_cache_file(app_state) {
        let _ = fs::remove_file(path);
    }
}

fn forecast_cache_file(app_state: &AppState) -> Result<std::path::PathBuf> {
    let dir = app_state.base_dir.join(FORECAST_CACHE_DIRNAME);
    fs::create_dir_all(&dir)?;
    Ok(dir.join("local.json"))
}

fn entry_matches(entry: &Value, signature: &str, trained_for_date: &str) -> bool {
    entry.get("signature").and_then(Value::as_str) == Some(signature)
        && entry.get("trained_for_date").and_then(Value::as_str) == Some(trained_for_date)
}

fn sample_weights(size: usize) -> Vec<f64> {
    if size == 0 { return Vec::new(); }
    if size == 1 { return vec![1.0]; }
    (0..size).map(|i| 0.7 + (0.6 * (i as f64 / (size - 1) as f64))).collect()
}

fn max_lookback(config: ForecastConfig) -> usize {
    config.lags.iter().copied().max().unwrap_or(1).max(config.windows.iter().copied().max().unwrap_or(1)).max(config.slope_window)
}

fn seasonal_refs(series: &[f64], index: usize, season: usize) -> Vec<f64> {
    let mut out = Vec::new();
    for period in 1..=4 {
        let offset = season * period;
        if index < offset { break; }
        out.push(series[index - offset]);
    }
    if out.is_empty() { vec![0.0] } else { out }
}

fn calendar(index: usize, config: ForecastConfig) -> Vec<f64> {
    if config.frequency == "daily" {
        let slot = index % 7;
        return (0..7).map(|i| if i == slot { 1.0 } else { 0.0 }).collect();
    }
    let slot = index % config.season_length;
    (0..config.season_length).map(|i| if i == slot { 1.0 } else { 0.0 }).collect()
}

fn avg(values: &[f64]) -> f64 { if values.is_empty() { 0.0 } else { values.iter().sum::<f64>() / values.len() as f64 } }
fn stdev(values: &[f64]) -> f64 { if values.len() <= 1 { 0.0 } else { let m = avg(values); (values.iter().map(|v| { let d = *v - m; d * d }).sum::<f64>() / values.len() as f64).sqrt() } }
fn slope(values: &[f64]) -> f64 {
    if values.len() <= 1 { return 0.0; }
    let x_mean = (values.len() - 1) as f64 / 2.0;
    let y_mean = avg(values);
    let mut num = 0.0; let mut den = 0.0;
    for (i, value) in values.iter().enumerate() { let x = i as f64; num += (x - x_mean) * (*value - y_mean); den += (x - x_mean) * (x - x_mean); }
    if den <= 1e-8 { 0.0 } else { num / den }
}
fn dot(left: &[f64], right: &[f64]) -> f64 { left.iter().zip(right.iter()).map(|(a, b)| a * b).sum::<f64>() }
fn wape(actual: &[f64], predicted: &[f64]) -> f64 { let den = actual.iter().map(|v| v.abs()).sum::<f64>(); let num = actual.iter().zip(predicted.iter()).map(|(a, p)| (a - p).abs()).sum::<f64>(); if den <= 1e-8 { num / actual.len().max(1) as f64 } else { num / den } }
fn rmse(actual: &[f64], predicted: &[f64]) -> f64 { (actual.iter().zip(predicted.iter()).map(|(a, p)| { let e = a - p; e * e }).sum::<f64>() / actual.len().max(1) as f64).sqrt() }
fn round2(value: f64) -> f64 { (value * 100.0).round() / 100.0 }
fn round_metric(value: Option<f64>) -> Option<f64> { value.and_then(|v| if v.is_finite() { Some((v * 10000.0).round() / 10000.0) } else { None }) }
fn quantile(values: &[f64], q: f64) -> f64 {
    if values.is_empty() { return 0.0; }
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));
    if sorted.len() == 1 { return sorted[0]; }
    let pos = (sorted.len() - 1) as f64 * q.clamp(0.0, 1.0);
    let lo = pos.floor() as usize;
    let hi = pos.ceil() as usize;
    if lo == hi { sorted[lo] } else { (sorted[lo] * (1.0 - (pos - lo as f64))) + (sorted[hi] * (pos - lo as f64)) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn daily_labels(len: usize) -> Vec<String> {
        let start = NaiveDate::from_ymd_opt(2025, 1, 1).unwrap();
        (0..len)
            .map(|offset| {
                (start + Duration::days(offset as i64))
                    .format("%Y-%m-%d")
                    .to_string()
            })
            .collect()
    }

    fn weekly_labels(len: usize) -> Vec<String> {
        (1..=len).map(|week| format!("2025-W{week:02}")).collect()
    }

    fn option_series(values: &[f64]) -> Vec<Option<f64>> {
        values.iter().copied().map(Some).collect()
    }

    #[test]
    fn recent_window_uses_frequency_specific_size() {
        assert_eq!(recent_window(DAILY_CONFIG), DAILY_RECENT_WINDOW);
        assert_eq!(recent_window(WEEKLY_CONFIG), WEEKLY_RECENT_WINDOW);

        let weekly_series = (1..=40)
            .map(|value| ((value % 6) as f64 * 1.7) + (value as f64 * 0.2))
            .collect::<Vec<_>>();
        let expected = ridge_predict(
            &weekly_series[weekly_series.len() - WEEKLY_RECENT_WINDOW..],
            WEEKLY_CONFIG,
            4,
            None,
            None,
        )
        .unwrap();
        let actual = run_model(
            &Model::RecentRidge {
                window: recent_window(WEEKLY_CONFIG),
            },
            &weekly_series,
            WEEKLY_CONFIG,
            4,
            None,
            None,
        )
        .unwrap();

        assert_eq!(actual.len(), expected.len());
        for (left, right) in actual.iter().zip(expected.iter()) {
            assert!((left - right).abs() < 1e-6);
        }
    }

    #[test]
    fn seasonal_series_keeps_forecast_available_with_expanded_candidates() {
        let pattern = [2.0, 2.8, 3.6, 4.1, 4.8, 5.4, 6.2];
        let values = (0..84)
            .map(|index| pattern[index % pattern.len()] + ((index / pattern.len()) as f64 * 0.03))
            .collect::<Vec<_>>();
        let labels = daily_labels(values.len());
        let forecast = create_forecast(
            &labels,
            &option_series(&values),
            DAILY_CONFIG,
            NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
            NaiveDate::from_ymd_opt(2025, 3, 25).unwrap(),
            None,
            None,
            None,
            1.0,
            "duration",
        )
        .unwrap();

        assert_eq!(forecast["status"], "ready");
        assert_eq!(forecast["available"], true);
        assert_eq!(forecast["prediction"].as_array().unwrap().len(), DAILY_CONFIG.horizon);
        assert!(forecast["model_name"].as_str().is_some());
        assert!(
            forecast["model_candidates"]
                .as_array()
                .unwrap()
                .len()
                >= 6
        );
    }

    #[test]
    fn intermittent_duration_series_keeps_nonlinear_candidates_available() {
        let values = (0..112)
            .map(|index| match index % 11 {
                0 => 18.0 + ((index / 11) % 4) as f64 * 4.0,
                1 => 7.5 + ((index / 11) % 3) as f64 * 1.8,
                2 if index % 22 == 2 => 4.5,
                2 => 1.0,
                _ => 0.0,
            })
            .collect::<Vec<_>>();
        let labels = daily_labels(values.len());
        let forecast = create_forecast(
            &labels,
            &option_series(&values),
            DAILY_CONFIG,
            NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
            NaiveDate::from_ymd_opt(2025, 4, 22).unwrap(),
            None,
            None,
            None,
            1.0,
            "duration",
        )
        .unwrap();

        let candidate_names = forecast["model_candidates"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|item| item["model_name"].as_str())
            .collect::<Vec<_>>();
        assert!(candidate_names.contains(&"Two-Stage Duration Autoregression"));
        assert!(candidate_names.contains(&"Log-XGBoost Autoregression"));

        let chosen_names = [
            forecast["model_name"].as_str().unwrap_or(""),
            forecast["fallback_from_model"].as_str().unwrap_or(""),
        ];
        assert!(chosen_names.iter().any(|name| {
            *name == "Two-Stage Duration Autoregression"
                || *name == "Log-XGBoost Autoregression"
                || *name == "Recent Log-XGBoost Autoregression"
        }));
    }

    #[test]
    fn high_error_forecast_falls_back_to_conservative_baseline() {
        let values = vec![4.0, 7.5, 5.0, 8.0, 4.2, 7.8, 5.1, 8.2, 4.4, 7.9, 5.3, 8.4];
        let future_labels = weekly_labels(WEEKLY_CONFIG.horizon);
        let baseline = Candidate {
            name: "Seasonal Naive".to_string(),
            model: Model::SeasonalNaive,
            wape: 0.52,
            rmse: 3.4,
            residuals: vec![vec![0.5; 8]; WEEKLY_CONFIG.horizon],
        };
        let selected = Candidate {
            name: "Log-XGBoost Autoregression".to_string(),
            model: Model::LogXgBoost { window: None },
            wape: 0.47,
            rmse: 2.9,
            residuals: vec![vec![1.0; 8]; WEEKLY_CONFIG.horizon],
        };
        let forecast = finalize_selected_forecast(
            &future_labels,
            &values,
            values.len(),
            WEEKLY_CONFIG,
            &selected,
            Some(&baseline),
            serialize_candidates(&[baseline.clone(), selected.clone()], &selected.name),
            1.0,
            None,
            None,
        )
        .unwrap();

        assert_eq!(forecast["status"], "conservative");
        assert_eq!(forecast["available"], true);
        assert_eq!(forecast["model_name"], "Seasonal Naive");
        assert_eq!(forecast["reason"], LOW_CONFIDENCE_REASON);
        assert!(forecast["fallback_from_model"].as_str().is_some());
    }

    #[test]
    fn insufficient_history_stays_unavailable_without_conservative_fallback() {
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0, 3.5, 2.5, 1.5];
        let labels = weekly_labels(values.len());
        let forecast = create_forecast(
            &labels,
            &option_series(&values),
            WEEKLY_CONFIG,
            NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
            NaiveDate::from_ymd_opt(2025, 2, 26).unwrap(),
            None,
            None,
            None,
            1.0,
            "efficiency",
        )
        .unwrap();

        assert_eq!(forecast["status"], "unavailable");
        assert_eq!(forecast["available"], false);
        assert!(forecast["fallback_from_model"].is_null());
    }
}
