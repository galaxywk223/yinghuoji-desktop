const FORECAST_KEYS = [
  "weekly_duration_data",
  "weekly_efficiency_data",
  "daily_duration_data",
  "daily_efficiency_data",
] as const;

type ForecastKey = (typeof FORECAST_KEYS)[number];

export function createDefaultForecast() {
  return {
    labels: [],
    prediction: [],
    lower: [],
    upper: [],
    model_name: null,
    history_points: 0,
    horizon: 0,
    trained_on: "all_history",
    confidence_level: 0.8,
    accuracy_threshold: 0.4,
    selection_strategy: "lowest_wape_then_rmse_with_weighted_blend",
    validation_wape: null,
    validation_rmse: null,
    baseline_wape: null,
    baseline_rmse: null,
    model_candidates: [],
    fallback_from_model: null,
    available: false,
    reason: "",
    status: "unavailable",
  };
}

export function createDefaultTrendDataset() {
  return {
    labels: [],
    actuals: [],
    trends: [],
    ongoing: false,
    ongoing_label: null,
    ongoing_value: null,
    forecast: createDefaultForecast(),
  };
}

export function createDefaultTrendsState() {
  return {
    weekly_duration_data: createDefaultTrendDataset(),
    weekly_efficiency_data: createDefaultTrendDataset(),
    daily_duration_data: createDefaultTrendDataset(),
    daily_efficiency_data: createDefaultTrendDataset(),
  };
}

export function normalizeForecastStatus(source?: any, fallback?: any) {
  return {
    state: source?.state || fallback?.state || "idle",
    signature: source?.signature || fallback?.signature || null,
    message: source?.message || fallback?.message || "",
    updated_at: source?.updated_at || fallback?.updated_at || null,
    trained_for_date:
      source?.trained_for_date || fallback?.trained_for_date || null,
  };
}

export function normalizeTrendPayload(data: any) {
  const base = createDefaultTrendsState();
  for (const key of FORECAST_KEYS) {
    base[key] = {
      ...createDefaultTrendDataset(),
      ...(data?.[key] || {}),
      forecast: {
        ...createDefaultForecast(),
        ...(data?.[key]?.forecast || {}),
      },
    };
  }
  return base;
}

export function mergeForecastBundleIntoTrends(
  currentTrends: ReturnType<typeof createDefaultTrendsState>,
  forecasts?: Record<string, any> | null,
) {
  if (!forecasts) {
    return currentTrends;
  }

  const next = { ...currentTrends };
  for (const key of FORECAST_KEYS) {
    next[key] = {
      ...currentTrends[key],
      forecast: {
        ...createDefaultForecast(),
        ...(forecasts[key] || {}),
      },
    };
  }
  return next;
}

export function mergeForecastBundleIntoPayload(
  payload: any,
  forecasts?: Record<string, any> | null,
  forecastStatus?: any,
) {
  if (!payload?.has_data || !forecasts) {
    return payload;
  }

  const next = {
    ...payload,
    forecast_status: {
      ...(payload.forecast_status || {}),
      ...(forecastStatus || {}),
    },
  };

  for (const key of FORECAST_KEYS) {
    next[key] = {
      ...(payload[key] || {}),
      forecast: {
        ...createDefaultForecast(),
        ...(forecasts[key] || {}),
      },
    };
  }

  return next;
}

export function createPendingForecastBundle(reason = "预测计算中，请稍后刷新") {
  const bundle = {} as Record<ForecastKey, ReturnType<typeof createDefaultForecast>>;
  for (const key of FORECAST_KEYS) {
    bundle[key] = {
      ...createDefaultForecast(),
      status: "pending",
      reason,
    };
  }
  return bundle;
}
