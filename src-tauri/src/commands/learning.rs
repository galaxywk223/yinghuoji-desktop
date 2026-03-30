use std::collections::{BTreeMap, HashMap};

use anyhow::Result;
use chrono::{Datelike, Duration, Local, NaiveDate};
use rusqlite::params;
use serde_json::{json, Value};
use tauri::State;

use crate::db;
use crate::models::{
    CategoryPayload, CategoryTrendQuery, ChartsCategoryQuery, ChartsOverviewQuery,
    RecentRecordsQuery, RecordPayload, RecordsListQuery, StagePayload, StatsQuery,
    StructuredRecordsQuery, SubcategoryMergePayload, SubcategoryUpdatePayload,
};
use crate::{AppResult, AppState};

use super::common::{
    active_stage_id, categories_json, connection, ensure_stage_exists, invalid,
    moving_average_points, record_json_by_id, stage_json_by_id, stages_json,
    subcategory_json_by_id,
};
use super::forecast::{
    attach_forecast_bundle, build_unavailable_forecast_bundle, resolve_forecast_entry,
    TrendForecastRequest,
};

#[tauri::command]
pub fn stages_list(state: State<'_, AppState>) -> AppResult<Value> {
    let conn = connection(&state)?;
    Ok(json!({ "success": true, "stages": stages_json(&conn)? }))
}

#[tauri::command]
pub fn stage_get(state: State<'_, AppState>, stage_id: i64) -> AppResult<Value> {
    let conn = connection(&state)?;
    let stage = stage_json_by_id(&conn, stage_id)?.ok_or_else(|| invalid("阶段不存在"))?;
    Ok(json!({ "success": true, "stage": stage }))
}

#[tauri::command]
pub fn stage_create(state: State<'_, AppState>, payload: StagePayload) -> AppResult<Value> {
    let conn = connection(&state)?;
    let start_date = payload.start_date.unwrap_or_else(|| {
        chrono::Local::now()
            .date_naive()
            .format("%Y-%m-%d")
            .to_string()
    });
    conn.execute(
        "INSERT INTO stage (name, start_date) VALUES (?1, ?2)",
        params![payload.name.trim(), start_date],
    )?;
    let stage_id = conn.last_insert_rowid();
    db::ensure_log_stage_consistency(&conn)?;
    if active_stage_id(&conn)? == 0 {
        db::set_setting(&conn, "active_stage_id", &stage_id.to_string())?;
    }
    Ok(json!({
        "success": true,
        "message": "阶段创建成功",
        "stage": stage_json_by_id(&conn, stage_id)?
    }))
}

#[tauri::command]
pub fn stage_update(
    state: State<'_, AppState>,
    stage_id: i64,
    payload: StagePayload,
) -> AppResult<Value> {
    let conn = connection(&state)?;
    ensure_stage_exists(&conn, stage_id)?;
    let current = stage_json_by_id(&conn, stage_id)?.ok_or_else(|| invalid("阶段不存在"))?;
    let start_date = payload
        .start_date
        .unwrap_or_else(|| current["start_date"].as_str().unwrap_or("").to_string());
    conn.execute(
        "UPDATE stage SET name = ?1, start_date = ?2 WHERE id = ?3",
        params![payload.name.trim(), start_date, stage_id],
    )?;
    db::ensure_log_stage_consistency(&conn)?;
    Ok(json!({
        "success": true,
        "message": "阶段更新成功",
        "stage": stage_json_by_id(&conn, stage_id)?
    }))
}

#[tauri::command]
pub fn stage_delete(state: State<'_, AppState>, stage_id: i64) -> AppResult<Value> {
    let conn = connection(&state)?;
    ensure_stage_exists(&conn, stage_id)?;
    let mut remaining_stmt =
        conn.prepare("SELECT id, start_date FROM stage WHERE id != ?1 ORDER BY start_date DESC")?;
    let remaining = remaining_stmt
        .query_map(params![stage_id], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let has_logs: i64 = conn.query_row(
        "SELECT COUNT(*) FROM log_entry WHERE stage_id = ?1",
        params![stage_id],
        |row| row.get(0),
    )?;
    if remaining.is_empty() && has_logs > 0 {
        return Err(invalid("最后一个阶段下仍有关联记录，无法删除"));
    }
    let mut logs_stmt = conn.prepare("SELECT id, log_date FROM log_entry WHERE stage_id = ?1")?;
    let logs = logs_stmt
        .query_map(params![stage_id], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    for (log_id, log_date_str) in logs {
        if remaining.is_empty() {
            continue;
        }
        let log_date = db::parse_date(&log_date_str)?;
        let mut next_stage_id = remaining.last().map(|item| item.0).unwrap_or_default();
        for (candidate_id, start_date) in &remaining {
            if db::parse_date(start_date)? <= log_date {
                next_stage_id = *candidate_id;
                break;
            }
        }
        conn.execute(
            "UPDATE log_entry SET stage_id = ?1 WHERE id = ?2",
            params![next_stage_id, log_id],
        )?;
    }
    conn.execute(
        "DELETE FROM daily_data WHERE stage_id = ?1",
        params![stage_id],
    )?;
    conn.execute(
        "DELETE FROM weekly_data WHERE stage_id = ?1",
        params![stage_id],
    )?;
    conn.execute("DELETE FROM stage WHERE id = ?1", params![stage_id])?;
    for (remaining_id, _) in remaining {
        db::recalculate_efficiency_for_stage(&conn, remaining_id)?;
    }
    if active_stage_id(&conn)? == stage_id {
        let next_active: i64 = conn
            .query_row(
                "SELECT COALESCE(id, 0) FROM stage ORDER BY start_date DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);
        db::set_setting(&conn, "active_stage_id", &next_active.to_string())?;
    }
    Ok(json!({ "success": true, "message": "阶段已删除" }))
}

#[tauri::command]
pub fn categories_list(
    state: State<'_, AppState>,
    include_subcategories: Option<bool>,
) -> AppResult<Value> {
    let conn = connection(&state)?;
    Ok(json!({
        "success": true,
        "categories": categories_json(&conn, include_subcategories.unwrap_or(false))?
    }))
}

#[tauri::command]
pub fn category_get(state: State<'_, AppState>, category_id: i64) -> AppResult<Value> {
    let conn = connection(&state)?;
    let category = categories_json(&conn, true)?
        .into_iter()
        .find(|item| item["id"].as_i64() == Some(category_id))
        .ok_or_else(|| invalid("分类不存在"))?;
    Ok(json!({ "success": true, "category": category }))
}

#[tauri::command]
pub fn category_create(state: State<'_, AppState>, payload: CategoryPayload) -> AppResult<Value> {
    let conn = connection(&state)?;
    conn.execute(
        "INSERT INTO category (name) VALUES (?1)",
        params![payload.name.trim()],
    )?;
    let category_id = conn.last_insert_rowid();
    Ok(json!({
        "success": true,
        "message": "分类创建成功",
        "category": { "id": category_id, "name": payload.name.trim(), "user_id": 1 }
    }))
}

#[tauri::command]
pub fn category_update(
    state: State<'_, AppState>,
    category_id: i64,
    payload: CategoryPayload,
) -> AppResult<Value> {
    let conn = connection(&state)?;
    conn.execute(
        "UPDATE category SET name = ?1 WHERE id = ?2",
        params![payload.name.trim(), category_id],
    )?;
    Ok(json!({
        "success": true,
        "message": "分类更新成功",
        "category": { "id": category_id, "name": payload.name.trim(), "user_id": 1 }
    }))
}

#[tauri::command]
pub fn category_delete(state: State<'_, AppState>, category_id: i64) -> AppResult<Value> {
    let conn = connection(&state)?;
    let sub_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sub_category WHERE category_id = ?1",
        params![category_id],
        |row| row.get(0),
    )?;
    if sub_count > 0 {
        return Err(invalid("无法删除，请先移除此分类下的所有标签"));
    }
    conn.execute("DELETE FROM category WHERE id = ?1", params![category_id])?;
    Ok(json!({ "success": true, "message": "分类已删除" }))
}

#[tauri::command]
pub fn subcategory_create(
    state: State<'_, AppState>,
    category_id: i64,
    payload: CategoryPayload,
) -> AppResult<Value> {
    let conn = connection(&state)?;
    conn.execute(
        "INSERT INTO sub_category (name, category_id) VALUES (?1, ?2)",
        params![payload.name.trim(), category_id],
    )?;
    let subcategory_id = conn.last_insert_rowid();
    Ok(json!({
        "success": true,
        "message": "子分类创建成功",
        "subcategory": { "id": subcategory_id, "name": payload.name.trim(), "category_id": category_id }
    }))
}

#[tauri::command]
pub fn subcategory_update(
    state: State<'_, AppState>,
    subcategory_id: i64,
    payload: SubcategoryUpdatePayload,
) -> AppResult<Value> {
    let conn = connection(&state)?;
    let category_id = payload.category_id.unwrap_or_else(|| {
        conn.query_row(
            "SELECT category_id FROM sub_category WHERE id = ?1",
            params![subcategory_id],
            |row| row.get::<_, i64>(0),
        )
        .unwrap_or_default()
    });
    conn.execute(
        "UPDATE sub_category SET name = ?1, category_id = ?2 WHERE id = ?3",
        params![payload.name.trim(), category_id, subcategory_id],
    )?;
    Ok(json!({
        "success": true,
        "message": "子分类更新成功",
        "subcategory": { "id": subcategory_id, "name": payload.name.trim(), "category_id": category_id }
    }))
}

#[tauri::command]
pub fn subcategory_delete(state: State<'_, AppState>, subcategory_id: i64) -> AppResult<Value> {
    let conn = connection(&state)?;
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM log_entry WHERE subcategory_id = ?1",
        params![subcategory_id],
        |row| row.get(0),
    )?;
    if count > 0 {
        return Err(invalid("无法删除该标签，因为它已关联了学习记录"));
    }
    conn.execute(
        "DELETE FROM sub_category WHERE id = ?1",
        params![subcategory_id],
    )?;
    Ok(json!({ "success": true, "message": "子分类已删除" }))
}

#[tauri::command]
pub fn subcategory_merge(
    state: State<'_, AppState>,
    subcategory_id: i64,
    payload: SubcategoryMergePayload,
) -> AppResult<Value> {
    let conn = connection(&state)?;
    if subcategory_id == payload.target_subcategory_id {
        return Err(invalid("不能合并到自身"));
    }
    let source = subcategory_json_by_id(&conn, subcategory_id)?
        .ok_or_else(|| invalid("待合并子分类不存在"))?;
    let target = subcategory_json_by_id(&conn, payload.target_subcategory_id)?
        .ok_or_else(|| invalid("目标子分类不存在"))?;
    let moved = conn.execute(
        "UPDATE log_entry SET subcategory_id = ?1 WHERE subcategory_id = ?2",
        params![payload.target_subcategory_id, subcategory_id],
    )?;
    conn.execute(
        "DELETE FROM sub_category WHERE id = ?1",
        params![subcategory_id],
    )?;
    Ok(json!({
        "success": true,
        "message": format!("已将标签 \"{}\" 合并到 \"{}\"", source["name"].as_str().unwrap_or(""), target["name"].as_str().unwrap_or("")),
        "moved_records": moved,
        "target_subcategory": target
    }))
}

#[tauri::command]
pub fn records_structured(
    state: State<'_, AppState>,
    query: StructuredRecordsQuery,
) -> AppResult<Value> {
    let conn = connection(&state)?;
    let stage = stage_json_by_id(&conn, query.stage_id)?.ok_or_else(|| invalid("阶段不存在"))?;
    let stage_start = db::stage_start_date(&conn, query.stage_id)?;
    let mut stmt =
        conn.prepare("SELECT id FROM log_entry WHERE stage_id = ?1 ORDER BY log_date ASC, id ASC")?;
    let ids = stmt
        .query_map(params![query.stage_id], |row| row.get::<_, i64>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut grouped: BTreeMap<(i32, i32), BTreeMap<String, Vec<Value>>> = BTreeMap::new();
    for id in ids {
        if let Some(record) = record_json_by_id(&conn, id)? {
            let log_date = db::parse_date(record["log_date"].as_str().unwrap_or(""))?;
            let (_, _, year, week_num) = db::get_custom_week_window(log_date, stage_start);
            grouped
                .entry((year, week_num))
                .or_default()
                .entry(record["log_date"].as_str().unwrap_or("").to_string())
                .or_default()
                .push(record);
        }
    }
    let desc = query.sort.unwrap_or_else(|| "desc".to_string()) == "desc";
    let mut weeks = grouped
        .into_iter()
        .map(|((year, week_num), days)| {
            let efficiency: f64 = conn
                .query_row(
                    "SELECT COALESCE(efficiency, 0) FROM weekly_data WHERE year = ?1 AND week_num = ?2 AND stage_id = ?3",
                    params![year, week_num, query.stage_id],
                    |row| row.get(0),
                )
                .unwrap_or(0.0);
            let mut day_items = days
                .into_iter()
                .map(|(date, logs)| {
                    let total_duration = logs
                        .iter()
                        .map(|item| item["actual_duration"].as_i64().unwrap_or(0))
                        .sum::<i64>();
                    let daily_eff: f64 = conn
                        .query_row(
                            "SELECT COALESCE(efficiency, 0) FROM daily_data WHERE log_date = ?1 AND stage_id = ?2",
                            params![date, query.stage_id],
                            |row| row.get(0),
                        )
                        .unwrap_or(0.0);
                    json!({
                        "date": date,
                        "efficiency": daily_eff,
                        "logs": logs,
                        "total_duration": total_duration
                    })
                })
                .collect::<Vec<_>>();
            if desc {
                day_items.reverse();
            }
            json!({
                "year": year,
                "week_num": week_num,
                "efficiency": efficiency,
                "days": day_items
            })
        })
        .collect::<Vec<_>>();
    if desc {
        weeks.reverse();
    }
    Ok(json!({
        "success": true,
        "data": weeks,
        "weeks": weeks,
        "stage_name": stage["name"]
    }))
}

#[tauri::command]
pub fn records_list(state: State<'_, AppState>, query: RecordsListQuery) -> AppResult<Value> {
    let conn = connection(&state)?;
    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(20).clamp(1, 100);
    let mut sql = "SELECT id FROM log_entry WHERE 1=1".to_string();
    if let Some(stage_id) = query.stage_id {
        sql.push_str(&format!(" AND stage_id = {stage_id}"));
    }
    if let Some(subcategory_id) = query.subcategory_id {
        sql.push_str(&format!(" AND subcategory_id = {subcategory_id}"));
    } else if let Some(category_id) = query.category_id {
        sql.push_str(&format!(
            " AND subcategory_id IN (SELECT id FROM sub_category WHERE category_id = {category_id})"
        ));
    }
    if let Some(start_date) = &query.start_date {
        sql.push_str(&format!(" AND log_date >= '{start_date}'"));
    }
    if let Some(end_date) = &query.end_date {
        sql.push_str(&format!(" AND log_date <= '{end_date}'"));
    }
    let count_sql = sql.replacen("SELECT id", "SELECT COUNT(*)", 1);
    let total: i64 = conn.query_row(&count_sql, [], |row| row.get(0))?;
    sql.push_str(&format!(
        " ORDER BY log_date DESC, id DESC LIMIT {} OFFSET {}",
        per_page,
        (page - 1) * per_page
    ));
    let mut stmt = conn.prepare(&sql)?;
    let ids = stmt
        .query_map([], |row| row.get::<_, i64>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut items = Vec::new();
    for id in ids {
        if let Some(item) = record_json_by_id(&conn, id)? {
            items.push(item);
        }
    }
    Ok(json!({
        "success": true,
        "data": {
            "records": items,
            "pagination": {
                "page": page,
                "per_page": per_page,
                "total": total,
                "pages": (total + per_page - 1) / per_page,
                "has_prev": page > 1,
                "has_next": page * per_page < total
            }
        }
    }))
}

#[tauri::command]
pub fn records_recent(state: State<'_, AppState>, query: RecentRecordsQuery) -> AppResult<Value> {
    let conn = connection(&state)?;
    let limit = query.limit.unwrap_or(10).clamp(1, 50);
    let mut stmt =
        conn.prepare("SELECT id FROM log_entry ORDER BY datetime(created_at) DESC LIMIT ?1")?;
    let ids = stmt
        .query_map(params![limit], |row| row.get::<_, i64>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut items = Vec::new();
    for id in ids {
        if let Some(item) = record_json_by_id(&conn, id)? {
            items.push(item);
        }
    }
    Ok(json!({ "success": true, "data": items }))
}

#[tauri::command]
pub fn record_get(state: State<'_, AppState>, record_id: i64) -> AppResult<Value> {
    let conn = connection(&state)?;
    let record = record_json_by_id(&conn, record_id)?.ok_or_else(|| invalid("记录不存在"))?;
    Ok(json!({ "success": true, "data": record }))
}

#[tauri::command]
pub fn record_create(state: State<'_, AppState>, payload: RecordPayload) -> AppResult<Value> {
    let conn = connection(&state)?;
    let log_date = db::parse_date(&payload.log_date)?;
    let stage = db::stage_for_date(&conn, log_date)?
        .ok_or_else(|| invalid("该日期早于最早阶段，请先创建对应阶段或调整日期"))?;
    let actual_duration = db::normalize_duration_minutes(payload.actual_duration);
    if actual_duration <= 0 {
        return Err(invalid("时长必须大于0"));
    }
    let _ = payload.stage_id;
    conn.execute(
        "INSERT INTO log_entry (log_date, time_slot, task, actual_duration, mood, notes, stage_id, subcategory_id, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            payload.log_date,
            payload.time_slot,
            payload.task.trim(),
            actual_duration,
            payload.mood,
            payload.notes.unwrap_or_default(),
            stage.0,
            payload.subcategory_id,
            db::now_utc_iso()
        ],
    )?;
    let record_id = conn.last_insert_rowid();
    db::update_efficiency_for_date(&conn, stage.0, log_date)?;
    Ok(json!({
        "success": true,
        "message": "记录创建成功",
        "data": record_json_by_id(&conn, record_id)?
    }))
}

#[tauri::command]
pub fn record_update(
    state: State<'_, AppState>,
    record_id: i64,
    payload: RecordPayload,
) -> AppResult<Value> {
    let conn = connection(&state)?;
    let original = record_json_by_id(&conn, record_id)?.ok_or_else(|| invalid("记录不存在"))?;
    let old_stage_id = original["stage_id"].as_i64().unwrap_or_default();
    let old_date = db::parse_date(original["log_date"].as_str().unwrap_or(""))?;
    let log_date = db::parse_date(&payload.log_date)?;
    let stage = db::stage_for_date(&conn, log_date)?
        .ok_or_else(|| invalid("该日期早于最早阶段，请先创建对应阶段或调整日期"))?;
    let actual_duration = db::normalize_duration_minutes(payload.actual_duration);
    conn.execute(
        "UPDATE log_entry
         SET task = ?1, subcategory_id = ?2, log_date = ?3, actual_duration = ?4, time_slot = ?5,
             notes = ?6, mood = ?7, stage_id = ?8, updated_at = ?9
         WHERE id = ?10",
        params![
            payload.task.trim(),
            payload.subcategory_id,
            payload.log_date,
            actual_duration,
            payload.time_slot,
            payload.notes.unwrap_or_default(),
            payload.mood,
            stage.0,
            db::now_utc_iso(),
            record_id
        ],
    )?;
    db::recalculate_efficiency_for_stage(&conn, old_stage_id)?;
    if old_stage_id != stage.0 {
        db::recalculate_efficiency_for_stage(&conn, stage.0)?;
    } else {
        db::update_efficiency_for_date(&conn, stage.0, log_date)?;
    }
    if old_date != log_date {
        db::update_efficiency_for_date(&conn, old_stage_id, old_date)?;
    }
    Ok(json!({
        "success": true,
        "message": "记录更新成功",
        "data": record_json_by_id(&conn, record_id)?
    }))
}

#[tauri::command]
pub fn record_delete(state: State<'_, AppState>, record_id: i64) -> AppResult<Value> {
    let conn = connection(&state)?;
    let original = record_json_by_id(&conn, record_id)?.ok_or_else(|| invalid("记录不存在"))?;
    let stage_id = original["stage_id"].as_i64().unwrap_or_default();
    conn.execute("DELETE FROM log_entry WHERE id = ?1", params![record_id])?;
    db::recalculate_efficiency_for_stage(&conn, stage_id)?;
    Ok(json!({ "success": true, "message": "记录删除成功" }))
}

#[tauri::command]
pub fn record_statistics(state: State<'_, AppState>, query: StatsQuery) -> AppResult<Value> {
    let conn = connection(&state)?;
    let mut sql = "SELECT COALESCE(actual_duration, 0), mood FROM log_entry WHERE 1=1".to_string();
    if let Some(stage_id) = query.stage_id {
        sql.push_str(&format!(" AND stage_id = {stage_id}"));
    }
    if let Some(days) = query.days {
        let start = chrono::Local::now().date_naive() - chrono::Duration::days(days.max(0));
        sql.push_str(&format!(" AND log_date >= '{}'", start.format("%Y-%m-%d")));
    }
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, Option<i64>>(1)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let total_records = rows.len() as i64;
    let total_minutes = rows.iter().map(|item| item.0).sum::<i64>();
    let avg_minutes = if total_records > 0 {
        total_minutes as f64 / total_records as f64
    } else {
        0.0
    };
    let moods = rows.iter().filter_map(|item| item.1).collect::<Vec<_>>();
    Ok(json!({
        "success": true,
        "data": {
            "total_records": total_records,
            "total_minutes": total_minutes,
            "total_hours": (total_minutes as f64 / 60.0 * 100.0).round() / 100.0,
            "avg_minutes": (avg_minutes * 100.0).round() / 100.0,
            "avg_mood": if moods.is_empty() { Value::Null } else { json!(moods.iter().sum::<i64>() as f64 / moods.len() as f64) }
        }
    }))
}

fn parse_stage_filter(value: Option<&str>) -> Option<i64> {
    value
        .filter(|item| !item.is_empty() && *item != "all")
        .and_then(|item| item.parse::<i64>().ok())
}

fn normalize_range_mode(value: Option<&str>) -> String {
    value
        .map(|item| item.trim().to_lowercase())
        .filter(|item| !item.is_empty())
        .unwrap_or_else(|| "all".to_string())
}

fn parse_optional_date(value: Option<String>) -> AppResult<Option<NaiveDate>> {
    value
        .map(|item| db::parse_date(&item).map_err(Into::into))
        .transpose()
}

fn stage_date_window(conn: &rusqlite::Connection, stage_id: i64) -> Result<(NaiveDate, NaiveDate)> {
    let start_date = db::stage_start_date(conn, stage_id)?;
    let last_log: Option<String> = conn.query_row(
        "SELECT MAX(log_date) FROM log_entry WHERE stage_id = ?1",
        params![stage_id],
        |row| row.get(0),
    )?;
    let end_date = last_log
        .as_deref()
        .map(db::parse_date)
        .transpose()?
        .unwrap_or_else(|| Local::now().date_naive());
    Ok((start_date, end_date))
}

fn selected_granularity(
    range_mode: &str,
    start_date: NaiveDate,
    end_date: NaiveDate,
    granularity: Option<&str>,
) -> String {
    let override_mode = granularity
        .map(|item| item.trim().to_lowercase())
        .filter(|item| item == "daily" || item == "weekly");
    if let Some(mode) = override_mode {
        return mode;
    }
    let delta_days = (end_date - start_date).num_days() + 1;
    if delta_days <= 35 || range_mode == "daily" {
        "daily".to_string()
    } else {
        "weekly".to_string()
    }
}

fn zero_filled_duration_series(
    start_date: NaiveDate,
    end_date: NaiveDate,
    granularity: &str,
) -> Value {
    let days = (0..=(end_date - start_date).num_days())
        .map(|offset| start_date + Duration::days(offset))
        .collect::<Vec<_>>();
    if granularity == "daily" {
        return json!({
            "labels": days.iter().map(|day| day.format("%Y-%m-%d").to_string()).collect::<Vec<_>>(),
            "data": vec![0.0; days.len()],
            "granularity": "daily",
            "start": start_date.format("%Y-%m-%d").to_string(),
            "end": end_date.format("%Y-%m-%d").to_string()
        });
    }

    let mut week_map = BTreeMap::<String, f64>::new();
    for day in days {
        let week_start = day - Duration::days(day.weekday().num_days_from_monday() as i64);
        week_map
            .entry(week_start.format("%Y-%m-%d").to_string())
            .or_insert(0.0);
    }
    json!({
        "labels": week_map.keys().cloned().collect::<Vec<_>>(),
        "data": week_map.values().copied().collect::<Vec<_>>(),
        "granularity": "weekly",
        "start": start_date.format("%Y-%m-%d").to_string(),
        "end": end_date.format("%Y-%m-%d").to_string()
    })
}

fn prepare_stage_annotations(conn: &rusqlite::Connection) -> Result<Vec<Value>> {
    let mut stmt =
        conn.prepare("SELECT id, name, start_date FROM stage ORDER BY start_date ASC")?;
    let stages = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    if stages.is_empty() {
        return Ok(Vec::new());
    }

    let global_start = db::parse_date(&stages[0].2)?;
    let last_log: Option<String> =
        conn.query_row("SELECT MAX(log_date) FROM log_entry", [], |row| row.get(0))?;
    let last_log_date = last_log
        .as_deref()
        .map(db::parse_date)
        .transpose()?
        .unwrap_or(global_start);

    let mut annotations = Vec::new();
    for (index, (_stage_id, name, start_date)) in stages.iter().enumerate() {
        let stage_start = db::parse_date(start_date)?;
        let end_date = if let Some((_, _, next_start_date)) = stages.get(index + 1) {
            db::parse_date(next_start_date)? - Duration::days(1)
        } else {
            last_log_date
        };
        let (_, _, start_year, start_week) = db::get_custom_week_window(stage_start, global_start);
        let (_, _, end_year, end_week) = db::get_custom_week_window(end_date, global_start);
        annotations.push(json!({
            "name": name,
            "start_week_label": format!("{start_year}-W{start_week:02}"),
            "end_week_label": format!("{end_year}-W{end_week:02}")
        }));
    }
    Ok(annotations)
}

fn category_chart_data_duration(
    conn: &rusqlite::Connection,
    stage_id: Option<i64>,
    start_date: Option<NaiveDate>,
    end_date: Option<NaiveDate>,
) -> Result<Option<Value>> {
    let mut sql = String::from(
        "SELECT c.name, sc.name, SUM(COALESCE(l.actual_duration, 0))
         FROM log_entry l
         JOIN sub_category sc ON l.subcategory_id = sc.id
         JOIN category c ON sc.category_id = c.id
         WHERE 1 = 1",
    );
    if let Some(stage_id) = stage_id {
        sql.push_str(&format!(" AND l.stage_id = {stage_id}"));
    }
    if let Some(start_date) = start_date {
        sql.push_str(&format!(
            " AND l.log_date >= '{}'",
            start_date.format("%Y-%m-%d")
        ));
    }
    if let Some(end_date) = end_date {
        sql.push_str(&format!(
            " AND l.log_date <= '{}'",
            end_date.format("%Y-%m-%d")
        ));
    }
    sql.push_str(" GROUP BY c.name, sc.name");

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    if rows.is_empty() {
        let mut legacy_sql = String::from(
            "SELECT legacy_category, SUM(COALESCE(actual_duration, 0))
             FROM log_entry
             WHERE legacy_category IS NOT NULL AND legacy_category != ''",
        );
        if let Some(stage_id) = stage_id {
            legacy_sql.push_str(&format!(" AND stage_id = {stage_id}"));
        }
        if let Some(start_date) = start_date {
            legacy_sql.push_str(&format!(
                " AND log_date >= '{}'",
                start_date.format("%Y-%m-%d")
            ));
        }
        if let Some(end_date) = end_date {
            legacy_sql.push_str(&format!(
                " AND log_date <= '{}'",
                end_date.format("%Y-%m-%d")
            ));
        }
        legacy_sql.push_str(" GROUP BY legacy_category");

        let mut legacy_stmt = conn.prepare(&legacy_sql)?;
        let legacy_rows = legacy_stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        if legacy_rows.is_empty() {
            return Ok(None);
        }
        let mut items = legacy_rows
            .into_iter()
            .map(|(name, duration)| (name, ((duration as f64 / 60.0) * 100.0).round() / 100.0))
            .collect::<Vec<_>>();
        items.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        return Ok(Some(json!({
            "main": {
                "labels": items.iter().map(|item| item.0.clone()).collect::<Vec<_>>(),
                "data": items.iter().map(|item| item.1).collect::<Vec<_>>()
            },
            "drilldown": {}
        })));
    }

    let mut category_totals = HashMap::<String, f64>::new();
    let mut sub_map = HashMap::<String, Vec<(String, f64)>>::new();
    for (category_name, sub_name, duration) in rows {
        let hours = duration as f64 / 60.0;
        *category_totals.entry(category_name.clone()).or_insert(0.0) += hours;
        sub_map
            .entry(category_name)
            .or_default()
            .push((sub_name, (hours * 100.0).round() / 100.0));
    }

    let mut sorted_categories = category_totals.into_iter().collect::<Vec<_>>();
    sorted_categories.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let mut drilldown = serde_json::Map::new();
    for (category_name, _) in &sorted_categories {
        if let Some(subs) = sub_map.get_mut(category_name) {
            subs.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            drilldown.insert(
                category_name.clone(),
                json!({
                    "labels": subs.iter().map(|item| item.0.clone()).collect::<Vec<_>>(),
                    "data": subs.iter().map(|item| item.1).collect::<Vec<_>>()
                }),
            );
        }
    }

    Ok(Some(json!({
        "main": {
            "labels": sorted_categories.iter().map(|item| item.0.clone()).collect::<Vec<_>>(),
            "data": sorted_categories.iter().map(|item| (item.1 * 100.0).round() / 100.0).collect::<Vec<_>>()
        },
        "drilldown": drilldown
    })))
}

fn category_chart_data_efficiency(
    conn: &rusqlite::Connection,
    stage_id: Option<i64>,
    start_date: Option<NaiveDate>,
    end_date: Option<NaiveDate>,
) -> Result<Option<Value>> {
    let mut sql = String::from(
        "SELECT c.name, sc.name, l.log_date,
                SUM(COALESCE(l.actual_duration, 0)),
                SUM(COALESCE(l.actual_duration, 0) * COALESCE(l.mood, 3))
         FROM log_entry l
         JOIN sub_category sc ON l.subcategory_id = sc.id
         JOIN category c ON sc.category_id = c.id
         WHERE 1 = 1",
    );
    if let Some(stage_id) = stage_id {
        sql.push_str(&format!(" AND l.stage_id = {stage_id}"));
    }
    if let Some(start_date) = start_date {
        sql.push_str(&format!(
            " AND l.log_date >= '{}'",
            start_date.format("%Y-%m-%d")
        ));
    }
    if let Some(end_date) = end_date {
        sql.push_str(&format!(
            " AND l.log_date <= '{}'",
            end_date.format("%Y-%m-%d")
        ));
    }
    sql.push_str(" GROUP BY c.name, sc.name, l.log_date");

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(4)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    if rows.is_empty() {
        let mut legacy_sql = String::from(
            "SELECT legacy_category, log_date,
                    SUM(COALESCE(actual_duration, 0)),
                    SUM(COALESCE(actual_duration, 0) * COALESCE(mood, 3))
             FROM log_entry
             WHERE legacy_category IS NOT NULL AND legacy_category != ''",
        );
        if let Some(stage_id) = stage_id {
            legacy_sql.push_str(&format!(" AND stage_id = {stage_id}"));
        }
        if let Some(start_date) = start_date {
            legacy_sql.push_str(&format!(
                " AND log_date >= '{}'",
                start_date.format("%Y-%m-%d")
            ));
        }
        if let Some(end_date) = end_date {
            legacy_sql.push_str(&format!(
                " AND log_date <= '{}'",
                end_date.format("%Y-%m-%d")
            ));
        }
        legacy_sql.push_str(" GROUP BY legacy_category, log_date");

        let mut legacy_stmt = conn.prepare(&legacy_sql)?;
        let legacy_rows = legacy_stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        if legacy_rows.is_empty() {
            return Ok(None);
        }

        let mut legacy_totals = HashMap::<String, f64>::new();
        for (name, _date, duration, weighted_mood) in legacy_rows {
            if duration <= 0 {
                continue;
            }
            let hours = duration as f64 / 60.0;
            let avg_mood = weighted_mood as f64 / duration as f64;
            *legacy_totals.entry(name).or_insert(0.0) += avg_mood * (1.0 + hours).ln();
        }
        let mut items = legacy_totals
            .into_iter()
            .map(|(name, value)| (name, (value * 100.0).round() / 100.0))
            .collect::<Vec<_>>();
        items.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        return Ok(Some(json!({
            "main": {
                "labels": items.iter().map(|item| item.0.clone()).collect::<Vec<_>>(),
                "data": items.iter().map(|item| item.1).collect::<Vec<_>>()
            },
            "drilldown": {}
        })));
    }

    let mut category_totals = HashMap::<String, f64>::new();
    let mut sub_totals = HashMap::<String, HashMap<String, f64>>::new();
    for (category_name, sub_name, _log_date, duration, weighted_mood) in rows {
        if duration <= 0 {
            continue;
        }
        let hours = duration as f64 / 60.0;
        let avg_mood = weighted_mood as f64 / duration as f64;
        let daily_efficiency = avg_mood * (1.0 + hours).ln();
        *category_totals.entry(category_name.clone()).or_insert(0.0) += daily_efficiency;
        *sub_totals
            .entry(category_name)
            .or_default()
            .entry(sub_name)
            .or_insert(0.0) += daily_efficiency;
    }

    let mut sorted_categories = category_totals.into_iter().collect::<Vec<_>>();
    sorted_categories.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let mut drilldown = serde_json::Map::new();
    for (category_name, _) in &sorted_categories {
        if let Some(subs) = sub_totals.get(category_name) {
            let mut sorted_subs = subs
                .iter()
                .map(|(name, value)| (name.clone(), (value * 100.0).round() / 100.0))
                .collect::<Vec<_>>();
            sorted_subs.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            drilldown.insert(
                category_name.clone(),
                json!({
                    "labels": sorted_subs.iter().map(|item| item.0.clone()).collect::<Vec<_>>(),
                    "data": sorted_subs.iter().map(|item| item.1).collect::<Vec<_>>()
                }),
            );
        }
    }

    Ok(Some(json!({
        "main": {
            "labels": sorted_categories.iter().map(|item| item.0.clone()).collect::<Vec<_>>(),
            "data": sorted_categories.iter().map(|item| (item.1 * 100.0).round() / 100.0).collect::<Vec<_>>()
        },
        "drilldown": drilldown
    })))
}

#[derive(Clone)]
struct OverviewDataset {
    labels: Vec<String>,
    actuals: Vec<f64>,
    training_labels: Vec<String>,
    training_actuals: Vec<Option<f64>>,
    training_stage_features: Vec<Vec<f64>>,
    future_stage_features: Vec<Vec<f64>>,
    ongoing: bool,
    ongoing_label: Option<String>,
    ongoing_value: Option<f64>,
}

#[derive(Clone)]
struct OverviewContext {
    global_start_date: NaiveDate,
    last_log_date: NaiveDate,
    kpis: Value,
    stage_annotations: Vec<Value>,
    daily_duration: OverviewDataset,
    daily_efficiency: OverviewDataset,
    weekly_duration: OverviewDataset,
    weekly_efficiency: OverviewDataset,
}

fn week_start(day: NaiveDate) -> NaiveDate {
    day - Duration::days(day.weekday().num_days_from_monday() as i64)
}

fn is_incomplete_daily_bucket(bucket_date: NaiveDate, today: NaiveDate) -> bool {
    bucket_date == today
}

fn is_incomplete_weekly_bucket(bucket_start: NaiveDate, bucket_end: NaiveDate, today: NaiveDate) -> bool {
    bucket_start <= today && today <= bucket_end && today < bucket_end
}

fn resolve_stage_snapshot(stages: &[(i64, String, NaiveDate)], target_date: NaiveDate) -> Vec<f64> {
    let mut current_index = 0usize;
    for (index, (_, _, start_date)) in stages.iter().enumerate() {
        if *start_date <= target_date {
            current_index = index;
        } else {
            break;
        }
    }
    let current_stage = &stages[current_index];
    let stage_age_days = (target_date - current_stage.2).num_days().max(0) as f64;
    let stage_index_norm = if stages.len() > 1 {
        current_index as f64 / (stages.len() - 1) as f64
    } else {
        0.0
    };
    let stage_start_flag = if target_date == current_stage.2 { 1.0 } else { 0.0 };
    vec![
        (stage_age_days * 100.0).round() / 100.0,
        (stage_index_norm * 10000.0).round() / 10000.0,
        stage_start_flag,
    ]
}

fn forecast_signature(context: &OverviewContext) -> String {
    let payload = json!({
        "global_start_date": context.global_start_date.format("%Y-%m-%d").to_string(),
        "daily_duration_data": {
            "training_labels": context.daily_duration.training_labels,
            "training_actuals": context.daily_duration.training_actuals,
            "training_stage_features": context.daily_duration.training_stage_features,
            "future_stage_features": context.daily_duration.future_stage_features,
        },
        "daily_efficiency_data": {
            "training_labels": context.daily_efficiency.training_labels,
            "training_actuals": context.daily_efficiency.training_actuals,
            "training_stage_features": context.daily_efficiency.training_stage_features,
            "future_stage_features": context.daily_efficiency.future_stage_features,
        },
        "weekly_duration_data": {
            "training_labels": context.weekly_duration.training_labels,
            "training_actuals": context.weekly_duration.training_actuals,
            "training_stage_features": context.weekly_duration.training_stage_features,
            "future_stage_features": context.weekly_duration.future_stage_features,
        },
        "weekly_efficiency_data": {
            "training_labels": context.weekly_efficiency.training_labels,
            "training_actuals": context.weekly_efficiency.training_actuals,
            "training_stage_features": context.weekly_efficiency.training_stage_features,
            "future_stage_features": context.weekly_efficiency.future_stage_features,
        },
    });
    let digest = payload.to_string();
    let mut hash = 1469598103934665603u64;
    for byte in digest.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(1099511628211u64);
    }
    format!("{hash:016x}")
}

fn build_overview_context(conn: &rusqlite::Connection) -> Result<Option<OverviewContext>> {
    let mut stage_stmt = conn.prepare("SELECT id, name, start_date FROM stage ORDER BY start_date ASC")?;
    let stages = stage_stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                db::parse_date(&row.get::<_, String>(2)?).map_err(|_| rusqlite::Error::InvalidQuery)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    if stages.is_empty() {
        return Ok(None);
    }

    let mut log_stmt = conn.prepare("SELECT log_date FROM log_entry ORDER BY log_date ASC")?;
    let log_dates = log_stmt
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    if log_dates.is_empty() {
        return Ok(None);
    }

    let first_log_date = db::parse_date(&log_dates[0])?;
    let last_log_date = db::parse_date(log_dates.last().map(String::as_str).unwrap_or(""))?;
    let global_start_date = stages[0].2;
    let today = Local::now().date_naive();
    let date_range = (0..=(last_log_date - first_log_date).num_days())
        .map(|offset| first_log_date + Duration::days(offset))
        .collect::<Vec<_>>();

    let mut duration_stmt =
        conn.prepare("SELECT log_date, SUM(COALESCE(actual_duration, 0)) FROM log_entry GROUP BY log_date")?;
    let duration_rows = duration_stmt
        .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let daily_duration_map = duration_rows.into_iter().collect::<HashMap<_, _>>();

    let mut eff_stmt = conn.prepare("SELECT log_date, efficiency FROM daily_data ORDER BY log_date ASC")?;
    let eff_rows = eff_stmt
        .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let daily_efficiency_map = eff_rows.into_iter().collect::<HashMap<_, _>>();

    let daily_labels = date_range
        .iter()
        .map(|day| day.format("%Y-%m-%d").to_string())
        .collect::<Vec<_>>();
    let daily_duration_actuals = date_range
        .iter()
        .map(|day| {
            let key = day.format("%Y-%m-%d").to_string();
            ((daily_duration_map.get(&key).copied().unwrap_or(0) as f64 / 60.0) * 100.0).round() / 100.0
        })
        .collect::<Vec<_>>();
    let daily_efficiency_optional = date_range
        .iter()
        .map(|day| daily_efficiency_map.get(&day.format("%Y-%m-%d").to_string()).copied())
        .collect::<Vec<_>>();
    let daily_efficiency_actuals = daily_efficiency_optional
        .iter()
        .map(|value| value.unwrap_or(0.0))
        .collect::<Vec<_>>();
    let daily_stage_features = date_range
        .iter()
        .map(|day| resolve_stage_snapshot(&stages, *day))
        .collect::<Vec<_>>();

    let daily_incomplete = date_range
        .last()
        .copied()
        .map(|day| is_incomplete_daily_bucket(day, today))
        .unwrap_or(false);
    let daily_train_len = if daily_incomplete {
        daily_labels.len().saturating_sub(1)
    } else {
        daily_labels.len()
    };
    let daily_future_start = if daily_incomplete {
        *date_range.last().unwrap_or(&last_log_date)
    } else {
        last_log_date + Duration::days(1)
    };
    let daily_future_stage_features = (0..14)
        .map(|offset| resolve_stage_snapshot(&stages, daily_future_start + Duration::days(offset as i64)))
        .collect::<Vec<_>>();

    #[derive(Default)]
    struct WeeklyAgg {
        duration_hours_total: f64,
        efficiency_total: f64,
        days: usize,
        stage_rows: Vec<Vec<f64>>,
        anchor_day: Option<NaiveDate>,
    }

    let mut weekly_map = BTreeMap::<(i32, i32), WeeklyAgg>::new();
    for (index, day) in date_range.iter().enumerate() {
        let (_, _, year, week_num) = db::get_custom_week_window(*day, global_start_date);
        let entry = weekly_map.entry((year, week_num)).or_default();
        entry.duration_hours_total += daily_duration_actuals.get(index).copied().unwrap_or(0.0);
        entry.efficiency_total += daily_efficiency_optional.get(index).and_then(|value| *value).unwrap_or(0.0);
        entry.days += 1;
        entry.stage_rows.push(daily_stage_features.get(index).cloned().unwrap_or_default());
        if entry.anchor_day.is_none() {
            entry.anchor_day = Some(*day);
        }
    }

    let mut weekly_labels = Vec::new();
    let mut weekly_duration_actuals = Vec::new();
    let mut weekly_duration_totals = Vec::new();
    let mut weekly_efficiency_actuals = Vec::new();
    let mut weekly_stage_features = Vec::new();
    let mut weekly_anchor_days = Vec::new();
    for ((year, week_num), agg) in &weekly_map {
        let anchor = agg.anchor_day.unwrap_or(last_log_date);
        let bucket_start = week_start(anchor);
        let bucket_end = bucket_start + Duration::days(6);
        let elapsed_days = if is_incomplete_weekly_bucket(bucket_start, bucket_end, today) {
            (today - bucket_start).num_days() + 1
        } else {
            agg.days as i64
        }
        .clamp(1, 7) as f64;
        weekly_labels.push(format!("{year}-W{week_num:02}"));
        weekly_duration_actuals.push((agg.duration_hours_total / elapsed_days * 100.0).round() / 100.0);
        weekly_duration_totals.push((agg.duration_hours_total * 100.0).round() / 100.0);
        weekly_efficiency_actuals.push((agg.efficiency_total / elapsed_days * 100.0).round() / 100.0);
        let avg_stage_age_days = agg.stage_rows.iter().map(|row| row.first().copied().unwrap_or(0.0)).sum::<f64>()
            / agg.stage_rows.len().max(1) as f64;
        let avg_stage_index = agg.stage_rows.iter().map(|row| row.get(1).copied().unwrap_or(0.0)).sum::<f64>()
            / agg.stage_rows.len().max(1) as f64;
        let stage_reset_flag = agg.stage_rows.iter().map(|row| row.get(2).copied().unwrap_or(0.0)).fold(0.0, f64::max);
        weekly_stage_features.push(vec![
            ((avg_stage_age_days / 7.0) * 100.0).round() / 100.0,
            (avg_stage_index * 10000.0).round() / 10000.0,
            stage_reset_flag,
        ]);
        weekly_anchor_days.push(anchor);
    }

    let weekly_incomplete = weekly_anchor_days
        .last()
        .copied()
        .map(|anchor| is_incomplete_weekly_bucket(week_start(anchor), week_start(anchor) + Duration::days(6), today))
        .unwrap_or(false);
    let weekly_train_len = if weekly_incomplete {
        weekly_labels.len().saturating_sub(1)
    } else {
        weekly_labels.len()
    };
    // Keep weekly future features stable within the same day even when the
    // user creates the first record for the current week.
    let weekly_reference_date = today;
    let weekly_future_stage_features = (0..8)
        .map(|offset| {
            let snapshot = resolve_stage_snapshot(&stages, weekly_reference_date + Duration::days((offset * 7) as i64));
            vec![
                ((snapshot.first().copied().unwrap_or(0.0) / 7.0) * 100.0).round() / 100.0,
                snapshot.get(1).copied().unwrap_or(0.0),
                snapshot.get(2).copied().unwrap_or(0.0),
            ]
        })
        .collect::<Vec<_>>();

    let avg_daily_minutes = if daily_duration_actuals.is_empty() {
        0
    } else {
        ((daily_duration_actuals.iter().sum::<f64>() / daily_duration_actuals.len() as f64) * 60.0).round() as i64
    };
    let efficiency_star = if daily_efficiency_actuals.is_empty() {
        Value::String("--".to_string())
    } else {
        json!((daily_efficiency_actuals.iter().sum::<f64>() / daily_efficiency_actuals.len() as f64 * 100.0).round() / 100.0)
    };

    Ok(Some(OverviewContext {
        global_start_date,
        last_log_date,
        kpis: json!({
            "avg_daily_minutes": avg_daily_minutes,
            "avg_daily_formatted": db::format_minutes(avg_daily_minutes),
            "efficiency_star": efficiency_star,
            "weekly_trend": weekly_duration_actuals.last().copied().unwrap_or(0.0),
        }),
        stage_annotations: prepare_stage_annotations(conn)?,
        daily_duration: OverviewDataset {
            labels: daily_labels.clone(),
            actuals: daily_duration_actuals.clone(),
            training_labels: daily_labels[..daily_train_len].to_vec(),
            training_actuals: daily_duration_actuals[..daily_train_len].iter().copied().map(Some).collect(),
            training_stage_features: daily_stage_features[..daily_train_len].to_vec(),
            future_stage_features: daily_future_stage_features,
            ongoing: daily_incomplete,
            ongoing_label: daily_incomplete.then(|| daily_labels.last().cloned()).flatten(),
            ongoing_value: daily_incomplete.then(|| daily_duration_actuals.last().copied()).flatten(),
        },
        daily_efficiency: OverviewDataset {
            labels: daily_labels.clone(),
            actuals: daily_efficiency_actuals.clone(),
            training_labels: daily_labels[..daily_train_len].to_vec(),
            training_actuals: daily_efficiency_optional[..daily_train_len].to_vec(),
            training_stage_features: daily_stage_features[..daily_train_len].to_vec(),
            future_stage_features: (0..14)
                .map(|offset| resolve_stage_snapshot(&stages, daily_future_start + Duration::days(offset as i64)))
                .collect(),
            ongoing: daily_incomplete,
            ongoing_label: daily_incomplete.then(|| daily_labels.last().cloned()).flatten(),
            ongoing_value: daily_incomplete.then(|| daily_efficiency_optional.last().and_then(|value| *value)).flatten(),
        },
        weekly_duration: OverviewDataset {
            labels: weekly_labels.clone(),
            actuals: weekly_duration_actuals.clone(),
            training_labels: weekly_labels[..weekly_train_len].to_vec(),
            training_actuals: weekly_duration_totals[..weekly_train_len].iter().copied().map(Some).collect(),
            training_stage_features: weekly_stage_features[..weekly_train_len].to_vec(),
            future_stage_features: weekly_future_stage_features.clone(),
            ongoing: weekly_incomplete,
            ongoing_label: weekly_incomplete.then(|| weekly_labels.last().cloned()).flatten(),
            ongoing_value: weekly_incomplete.then(|| weekly_duration_actuals.last().copied()).flatten(),
        },
        weekly_efficiency: OverviewDataset {
            labels: weekly_labels.clone(),
            actuals: weekly_efficiency_actuals.clone(),
            training_labels: weekly_labels[..weekly_train_len].to_vec(),
            training_actuals: weekly_efficiency_actuals[..weekly_train_len].iter().copied().map(Some).collect(),
            training_stage_features: weekly_stage_features[..weekly_train_len].to_vec(),
            future_stage_features: weekly_future_stage_features,
            ongoing: weekly_incomplete,
            ongoing_label: weekly_incomplete.then(|| weekly_labels.last().cloned()).flatten(),
            ongoing_value: weekly_incomplete.then(|| weekly_efficiency_actuals.last().copied()).flatten(),
        },
    }))
}

fn build_overview_forecast_request(context: &OverviewContext) -> TrendForecastRequest {
    TrendForecastRequest {
        daily_labels: context.daily_duration.training_labels.clone(),
        daily_duration_values: context.daily_duration.training_actuals.clone(),
        daily_efficiency_values: context.daily_efficiency.training_actuals.clone(),
        daily_stage_features: context.daily_duration.training_stage_features.clone(),
        daily_future_stage_features: context.daily_duration.future_stage_features.clone(),
        weekly_labels: context.weekly_duration.training_labels.clone(),
        weekly_duration_values: context.weekly_duration.training_actuals.clone(),
        weekly_efficiency_values: context.weekly_efficiency.training_actuals.clone(),
        weekly_stage_features: context.weekly_duration.training_stage_features.clone(),
        weekly_future_stage_features: context.weekly_duration.future_stage_features.clone(),
        global_start_date: context.global_start_date,
        last_log_date: context.last_log_date,
        daily_current_label: context.daily_duration.ongoing_label.clone(),
        weekly_current_label: context.weekly_duration.ongoing_label.clone(),
        weekly_duration_display_divisor: 7.0,
    }
}

fn build_overview_payload(
    conn: &rusqlite::Connection,
    app_state: &AppState,
    force_sync_forecasts: bool,
    force_retrain_forecasts: bool,
) -> Result<Value> {
    let Some(context) = build_overview_context(conn)? else {
        return Ok(json!({
            "has_data": false,
            "kpis": {
                "avg_daily_minutes": 0,
                "avg_daily_formatted": "--",
                "efficiency_star": "--",
                "weekly_trend": "--"
            },
            "stage_annotations": [],
            "forecast_status": {
                "state": "unavailable",
                "signature": Value::Null,
                "message": "暂无可用于预测的历史数据",
                "updated_at": Value::Null,
                "trained_for_date": Value::Null
            }
        }));
    };

    let signature = forecast_signature(&context);
    let forecast_entry = resolve_forecast_entry(
        app_state,
        &signature,
        build_overview_forecast_request(&context),
        force_sync_forecasts,
        force_retrain_forecasts,
    )?;
    let mut payload = json!({
        "has_data": true,
        "kpis": context.kpis,
        "weekly_duration_data": {
            "labels": context.weekly_duration.labels,
            "actuals": context.weekly_duration.actuals,
            "trends": moving_average_points(&context.weekly_duration.actuals),
            "ongoing": context.weekly_duration.ongoing,
            "ongoing_label": context.weekly_duration.ongoing_label,
            "ongoing_value": context.weekly_duration.ongoing_value
        },
        "weekly_efficiency_data": {
            "labels": context.weekly_efficiency.labels,
            "actuals": context.weekly_efficiency.actuals,
            "trends": moving_average_points(&context.weekly_efficiency.actuals),
            "ongoing": context.weekly_efficiency.ongoing,
            "ongoing_label": context.weekly_efficiency.ongoing_label,
            "ongoing_value": context.weekly_efficiency.ongoing_value
        },
        "daily_duration_data": {
            "labels": context.daily_duration.labels,
            "actuals": context.daily_duration.actuals,
            "trends": moving_average_points(&context.daily_duration.actuals),
            "ongoing": context.daily_duration.ongoing,
            "ongoing_label": context.daily_duration.ongoing_label,
            "ongoing_value": context.daily_duration.ongoing_value
        },
        "daily_efficiency_data": {
            "labels": context.daily_efficiency.labels,
            "actuals": context.daily_efficiency.actuals,
            "trends": moving_average_points(&context.daily_efficiency.actuals),
            "ongoing": context.daily_efficiency.ongoing,
            "ongoing_label": context.daily_efficiency.ongoing_label,
            "ongoing_value": context.daily_efficiency.ongoing_value
        },
        "stage_annotations": context.stage_annotations,
        "forecast_status": {
            "state": forecast_entry["state"].clone(),
            "signature": signature,
            "message": forecast_entry["message"].clone(),
            "updated_at": forecast_entry["updated_at"].clone(),
            "trained_for_date": forecast_entry["trained_for_date"].clone()
        }
    });
    let forecast_bundle = forecast_entry
        .get("forecast_bundle")
        .cloned()
        .unwrap_or_else(|| build_unavailable_forecast_bundle(""));
    attach_forecast_bundle(&mut payload, &forecast_bundle);
    Ok(payload)
}

#[tauri::command]
pub fn charts_overview(state: State<'_, AppState>, query: ChartsOverviewQuery) -> AppResult<Value> {
    let conn = connection(&state)?;
    let _ = (&query.view, &query.stage_id);
    Ok(build_overview_payload(&conn, &state, false, false)?)
}

#[tauri::command]
pub fn charts_overview_forecast_status(state: State<'_, AppState>) -> AppResult<Value> {
    let conn = connection(&state)?;
    let payload = build_overview_payload(&conn, &state, false, false)?;
    if !payload["has_data"].as_bool().unwrap_or(false) {
        return Ok(json!({
            "status": "unavailable",
            "signature": Value::Null,
            "message": "暂无可用于预测的历史数据",
            "updated_at": Value::Null,
            "trained_for_date": Value::Null,
            "forecasts": build_unavailable_forecast_bundle(""),
        }));
    }
    Ok(json!({
        "status": payload["forecast_status"]["state"].clone(),
        "signature": payload["forecast_status"]["signature"].clone(),
        "message": payload["forecast_status"]["message"].clone(),
        "updated_at": payload["forecast_status"]["updated_at"].clone(),
        "trained_for_date": payload["forecast_status"]["trained_for_date"].clone(),
        "forecasts": {
            "daily_duration_data": payload["daily_duration_data"]["forecast"].clone(),
            "daily_efficiency_data": payload["daily_efficiency_data"]["forecast"].clone(),
            "weekly_duration_data": payload["weekly_duration_data"]["forecast"].clone(),
            "weekly_efficiency_data": payload["weekly_efficiency_data"]["forecast"].clone()
        }
    }))
}

#[tauri::command]
pub fn charts_overview_forecast_retrain(state: State<'_, AppState>) -> AppResult<Value> {
    let conn = connection(&state)?;
    let payload = build_overview_payload(&conn, &state, false, true)?;
    if !payload["has_data"].as_bool().unwrap_or(false) {
        return Ok(json!({
            "status": "unavailable",
            "signature": Value::Null,
            "message": "暂无可用于预测的历史数据",
            "updated_at": Value::Null,
            "trained_for_date": Value::Null,
            "forecasts": build_unavailable_forecast_bundle(""),
        }));
    }
    Ok(json!({
        "status": payload["forecast_status"]["state"].clone(),
        "signature": payload["forecast_status"]["signature"].clone(),
        "message": if payload["forecast_status"]["state"].as_str() == Some("pending") {
            Value::String("已开始重新训练预测模型".to_string())
        } else {
            payload["forecast_status"]["message"].clone()
        },
        "updated_at": payload["forecast_status"]["updated_at"].clone(),
        "trained_for_date": payload["forecast_status"]["trained_for_date"].clone(),
        "forecasts": {
            "daily_duration_data": payload["daily_duration_data"]["forecast"].clone(),
            "daily_efficiency_data": payload["daily_efficiency_data"]["forecast"].clone(),
            "weekly_duration_data": payload["weekly_duration_data"]["forecast"].clone(),
            "weekly_efficiency_data": payload["weekly_efficiency_data"]["forecast"].clone()
        }
    }))
}

#[tauri::command]
pub fn charts_categories(
    state: State<'_, AppState>,
    query: ChartsCategoryQuery,
) -> AppResult<Value> {
    let conn = connection(&state)?;
    let _ = &query.range_mode;
    let stage_id = parse_stage_filter(query.stage_id.as_deref());
    let start_date = parse_optional_date(query.start_date)?;
    let end_date = parse_optional_date(query.end_date)?;
    let metric_mode = query.metric_mode.unwrap_or_else(|| "duration".to_string());
    let data = if metric_mode == "efficiency" {
        category_chart_data_efficiency(&conn, stage_id, start_date, end_date)?
    } else {
        category_chart_data_duration(&conn, stage_id, start_date, end_date)?
    };

    Ok(data.unwrap_or_else(|| {
        json!({
            "main": { "labels": Vec::<String>::new(), "data": Vec::<f64>::new() },
            "drilldown": {}
        })
    }))
}

#[tauri::command]
pub fn charts_category_trend(
    state: State<'_, AppState>,
    query: CategoryTrendQuery,
) -> AppResult<Value> {
    let conn = connection(&state)?;
    let metric_mode = query.metric_mode.unwrap_or_else(|| "duration".to_string());
    let range_mode = normalize_range_mode(query.range_mode.as_deref());
    let stage_id = parse_stage_filter(query.stage_id.as_deref());
    let mut start_date = parse_optional_date(query.start_date)?;
    let mut end_date = parse_optional_date(query.end_date)?;

    let base_rows = if metric_mode == "efficiency" {
        let mut sql = String::from(
            "SELECT l.log_date,
                    SUM(COALESCE(l.actual_duration, 0)),
                    SUM(COALESCE(l.actual_duration, 0) * COALESCE(l.mood, 3))
             FROM log_entry l
             WHERE 1 = 1",
        );
        if let Some(subcategory_id) = query.subcategory_id {
            sql.push_str(&format!(" AND l.subcategory_id = {subcategory_id}"));
        } else if let Some(category_id) = query.category_id {
            sql.push_str(
                " AND l.subcategory_id IN (SELECT id FROM sub_category WHERE category_id = ",
            );
            sql.push_str(&category_id.to_string());
            sql.push(')');
        }
        if let Some(stage_id) = stage_id {
            sql.push_str(&format!(" AND l.stage_id = {stage_id}"));
        }
        sql.push_str(" GROUP BY l.log_date ORDER BY l.log_date ASC");

        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    db::parse_date(&row.get::<_, String>(0)?)
                        .map_err(|_| rusqlite::Error::InvalidQuery)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        rows.into_iter()
            .map(|(date, duration, weighted_mood)| {
                let value = if duration <= 0 {
                    0.0
                } else {
                    let hours = duration as f64 / 60.0;
                    let avg_mood = weighted_mood as f64 / duration as f64;
                    avg_mood * (1.0 + hours).ln()
                };
                (date, value)
            })
            .collect::<Vec<_>>()
    } else {
        let mut sql = String::from(
            "SELECT l.log_date, SUM(COALESCE(l.actual_duration, 0))
             FROM log_entry l
             WHERE 1 = 1",
        );
        if let Some(subcategory_id) = query.subcategory_id {
            sql.push_str(&format!(" AND l.subcategory_id = {subcategory_id}"));
        } else if let Some(category_id) = query.category_id {
            sql.push_str(
                " AND l.subcategory_id IN (SELECT id FROM sub_category WHERE category_id = ",
            );
            sql.push_str(&category_id.to_string());
            sql.push(')');
        }
        if let Some(stage_id) = stage_id {
            sql.push_str(&format!(" AND l.stage_id = {stage_id}"));
        }
        sql.push_str(" GROUP BY l.log_date ORDER BY l.log_date ASC");

        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    db::parse_date(&row.get::<_, String>(0)?)
                        .map_err(|_| rusqlite::Error::InvalidQuery)?,
                    row.get::<_, i64>(1)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        rows.into_iter()
            .map(|(date, duration)| (date, duration as f64 / 60.0))
            .collect::<Vec<_>>()
    };

    if start_date.is_none() || end_date.is_none() {
        if range_mode == "stage" {
            if let Some(stage_id) = stage_id {
                let (stage_start, stage_end) = stage_date_window(&conn, stage_id)?;
                start_date = start_date.or(Some(stage_start));
                end_date = end_date.or(Some(stage_end));
            }
        } else if range_mode == "all" {
            start_date = start_date.or_else(|| base_rows.first().map(|item| item.0));
            end_date = end_date.or_else(|| base_rows.last().map(|item| item.0));
        }
    }

    let today = Local::now().date_naive();
    let mut start_date = start_date.unwrap_or_else(|| {
        end_date
            .unwrap_or(today)
            .checked_sub_signed(Duration::weeks(11))
            .unwrap_or(today)
    });
    let mut end_date = end_date.unwrap_or(today);
    if start_date > end_date {
        std::mem::swap(&mut start_date, &mut end_date);
    }

    let mut used_legacy_name: Option<String> = None;
    let mut filtered_rows = base_rows
        .into_iter()
        .filter(|(date, _)| *date >= start_date && *date <= end_date)
        .collect::<Vec<_>>();

    if filtered_rows.is_empty() && metric_mode != "efficiency" {
        if let Some(category_id) = query.category_id {
            let category_name: Option<String> = conn
                .query_row(
                    "SELECT name FROM category WHERE id = ?1",
                    params![category_id],
                    |row| row.get(0),
                )
                .ok();
            if let Some(category_name) = category_name {
                used_legacy_name = Some(category_name.clone());
                let mut legacy_sql = String::from(
                    "SELECT log_date, SUM(COALESCE(actual_duration, 0))
                     FROM log_entry
                     WHERE legacy_category = ?1",
                );
                if let Some(stage_id) = stage_id {
                    legacy_sql.push_str(&format!(" AND stage_id = {stage_id}"));
                }
                legacy_sql.push_str(&format!(
                    " AND log_date >= '{}' AND log_date <= '{}'",
                    start_date.format("%Y-%m-%d"),
                    end_date.format("%Y-%m-%d")
                ));
                legacy_sql.push_str(" GROUP BY log_date ORDER BY log_date ASC");
                let mut legacy_stmt = conn.prepare(&legacy_sql)?;
                filtered_rows = legacy_stmt
                    .query_map(params![category_name], |row| {
                        Ok((
                            db::parse_date(&row.get::<_, String>(0)?)
                                .map_err(|_| rusqlite::Error::InvalidQuery)?,
                            row.get::<_, i64>(1)? as f64 / 60.0,
                        ))
                    })?
                    .collect::<rusqlite::Result<Vec<_>>>()?;
            }
        }
    }

    let granularity = selected_granularity(
        &range_mode,
        start_date,
        end_date,
        query.granularity.as_deref(),
    );

    if filtered_rows.is_empty() {
        let mut payload = zero_filled_duration_series(start_date, end_date, &granularity);
        if let Some(legacy_name) = used_legacy_name {
            payload["legacy_name"] = json!(legacy_name);
        }
        return Ok(json!({ "success": true, "data": payload }));
    }

    let days = (0..=(end_date - start_date).num_days())
        .map(|offset| start_date + Duration::days(offset))
        .collect::<Vec<_>>();
    let day_map = filtered_rows
        .into_iter()
        .map(|(date, value)| (date, (value * 100.0).round() / 100.0))
        .collect::<BTreeMap<_, _>>();

    let payload = if granularity == "daily" {
        json!({
            "labels": days.iter().map(|day| day.format("%Y-%m-%d").to_string()).collect::<Vec<_>>(),
            "data": days.iter().map(|day| day_map.get(day).copied().unwrap_or(0.0)).collect::<Vec<_>>(),
            "granularity": "daily",
            "start": start_date.format("%Y-%m-%d").to_string(),
            "end": end_date.format("%Y-%m-%d").to_string()
        })
    } else {
        let mut week_map = BTreeMap::<String, f64>::new();
        for day in days {
            let week_start = day - Duration::days(day.weekday().num_days_from_monday() as i64);
            let key = week_start.format("%Y-%m-%d").to_string();
            let value = day_map.get(&day).copied().unwrap_or(0.0);
            *week_map.entry(key).or_insert(0.0) += value;
        }
        json!({
            "labels": week_map.keys().cloned().collect::<Vec<_>>(),
            "data": week_map.values().map(|item| (item * 100.0).round() / 100.0).collect::<Vec<_>>(),
            "granularity": "weekly",
            "start": start_date.format("%Y-%m-%d").to_string(),
            "end": end_date.format("%Y-%m-%d").to_string()
        })
    };

    let mut payload = payload;
    if let Some(legacy_name) = used_legacy_name {
        payload["legacy_name"] = json!(legacy_name);
    }
    Ok(json!({
        "success": true,
        "data": {
            "labels": payload["labels"].clone(),
            "data": payload["data"].clone(),
            "granularity": payload["granularity"].clone(),
            "start": payload["start"].clone(),
            "end": payload["end"].clone(),
            "legacy_name": payload.get("legacy_name").cloned().unwrap_or(Value::Null),
        }
    }))
}

#[tauri::command]
pub fn charts_stages(state: State<'_, AppState>) -> AppResult<Value> {
    let conn = connection(&state)?;
    Ok(json!({ "success": true, "data": { "stages": stages_json(&conn)? } }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_dataset() -> OverviewDataset {
        OverviewDataset {
            labels: vec![
                "2025-03-01".to_string(),
                "2025-03-02".to_string(),
                "2025-03-03".to_string(),
            ],
            actuals: vec![1.2, 2.4, 3.6],
            training_labels: vec![
                "2025-03-01".to_string(),
                "2025-03-02".to_string(),
                "2025-03-03".to_string(),
            ],
            training_actuals: vec![Some(1.2), Some(2.4), Some(3.6)],
            training_stage_features: vec![
                vec![0.0, 0.0, 0.0],
                vec![1.0, 0.0, 0.0],
                vec![2.0, 0.0, 0.0],
            ],
            future_stage_features: vec![vec![3.0, 0.0, 0.0], vec![4.0, 0.0, 0.0]],
            ongoing: true,
            ongoing_label: Some("2025-03-11".to_string()),
            ongoing_value: Some(4.2),
        }
    }

    fn sample_context() -> OverviewContext {
        OverviewContext {
            global_start_date: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
            last_log_date: NaiveDate::from_ymd_opt(2025, 3, 11).unwrap(),
            kpis: json!({}),
            stage_annotations: vec![],
            daily_duration: sample_dataset(),
            daily_efficiency: sample_dataset(),
            weekly_duration: OverviewDataset {
                ongoing_label: Some("2025-W11".to_string()),
                ..sample_dataset()
            },
            weekly_efficiency: OverviewDataset {
                ongoing_label: Some("2025-W11".to_string()),
                ..sample_dataset()
            },
        }
    }

    #[test]
    fn forecast_signature_ignores_ongoing_buckets_and_last_log_date() {
        let base = sample_context();
        let mut changed = sample_context();
        changed.last_log_date = NaiveDate::from_ymd_opt(2025, 3, 12).unwrap();
        changed.daily_duration.ongoing = false;
        changed.daily_duration.ongoing_label = Some("2025-03-12".to_string());
        changed.daily_duration.ongoing_value = Some(9.9);
        changed.daily_efficiency.ongoing = false;
        changed.daily_efficiency.ongoing_label = Some("2025-03-12".to_string());
        changed.daily_efficiency.ongoing_value = Some(8.8);
        changed.weekly_duration.ongoing = false;
        changed.weekly_duration.ongoing_label = Some("2025-W12".to_string());
        changed.weekly_duration.ongoing_value = Some(7.7);
        changed.weekly_efficiency.ongoing = false;
        changed.weekly_efficiency.ongoing_label = Some("2025-W12".to_string());
        changed.weekly_efficiency.ongoing_value = Some(6.6);

        assert_eq!(forecast_signature(&base), forecast_signature(&changed));
    }

    #[test]
    fn forecast_signature_changes_when_completed_history_changes() {
        let base = sample_context();
        let mut changed = sample_context();
        changed.daily_duration.training_actuals[1] = Some(9.9);

        assert_ne!(forecast_signature(&base), forecast_signature(&changed));
    }
}
