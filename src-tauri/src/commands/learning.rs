use std::collections::BTreeMap;

use anyhow::Result;
use chrono::Datelike;
use rusqlite::params;
use serde_json::{json, Value};
use tauri::State;

use crate::db;
use crate::models::{
    CategoryPayload, CategoryTrendQuery, ChartsCategoryQuery, ChartsOverviewQuery, RecentRecordsQuery,
    RecordPayload, RecordsListQuery, StagePayload, StatsQuery, StructuredRecordsQuery,
    SubcategoryMergePayload, SubcategoryUpdatePayload,
};
use crate::{AppResult, AppState};

use super::common::{
    active_stage_id, categories_json, connection, ensure_stage_exists, invalid, moving_average_points,
    record_json_by_id, stage_json_by_id, stages_json, subcategory_json_by_id, labels_for_daily,
};

fn labels_for_weekly(
    conn: &rusqlite::Connection,
    stage_id: Option<i64>,
) -> Result<(Vec<String>, Vec<f64>, Vec<f64>)> {
    if let Some(stage_id) = stage_id {
        let stage_start = db::stage_start_date(conn, stage_id)?;
        let mut duration_map = BTreeMap::<(i32, i32), i64>::new();
        let mut weekly_map = BTreeMap::<(i32, i32), f64>::new();

        let mut log_stmt =
            conn.prepare("SELECT log_date, COALESCE(actual_duration, 0) FROM log_entry WHERE stage_id = ?1")?;
        let logs = log_stmt
            .query_map(params![stage_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        for (log_date_str, duration) in logs {
            let log_date = db::parse_date(&log_date_str)?;
            let (_, _, year, week_num) = db::get_custom_week_window(log_date, stage_start);
            *duration_map.entry((year, week_num)).or_insert(0) += duration;
        }

        let mut weekly_stmt =
            conn.prepare("SELECT year, week_num, COALESCE(efficiency, 0) FROM weekly_data WHERE stage_id = ?1 ORDER BY year ASC, week_num ASC")?;
        let rows = weekly_stmt
            .query_map(params![stage_id], |row| {
                Ok((row.get::<_, i32>(0)?, row.get::<_, i32>(1)?, row.get::<_, f64>(2)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        for (year, week_num, efficiency) in rows {
            weekly_map.insert((year, week_num), efficiency);
        }

        let mut labels = Vec::new();
        let mut duration = Vec::new();
        let mut efficiency = Vec::new();
        for ((year, week_num), score) in weekly_map {
            labels.push(format!("{year} W{week_num:02}"));
            duration.push(
                (duration_map.get(&(year, week_num)).copied().unwrap_or(0) as f64 / 60.0 * 10.0)
                    .round()
                    / 10.0,
            );
            efficiency.push((score * 100.0).round() / 100.0);
        }
        return Ok((labels, duration, efficiency));
    }

    let mut stage_stmt = conn.prepare("SELECT id FROM stage ORDER BY start_date ASC")?;
    let stage_ids = stage_stmt
        .query_map([], |row| row.get::<_, i64>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut combined = BTreeMap::<String, (f64, f64, usize)>::new();
    for stage_id in stage_ids {
        let (labels, duration, efficiency) = labels_for_weekly(conn, Some(stage_id))?;
        for (idx, label) in labels.iter().enumerate() {
            let entry = combined.entry(label.clone()).or_insert((0.0, 0.0, 0));
            entry.0 += duration[idx];
            entry.1 += efficiency[idx];
            entry.2 += 1;
        }
    }
    let labels = combined.keys().cloned().collect::<Vec<_>>();
    let duration = labels
        .iter()
        .map(|label| combined.get(label).map(|item| (item.0 * 10.0).round() / 10.0).unwrap_or(0.0))
        .collect::<Vec<_>>();
    let efficiency = labels
        .iter()
        .map(|label| {
            combined
                .get(label)
                .map(|item| ((item.1 / item.2 as f64) * 100.0).round() / 100.0)
                .unwrap_or(0.0)
        })
        .collect::<Vec<_>>();
    Ok((labels, duration, efficiency))
}

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
    let start_date = payload
        .start_date
        .unwrap_or_else(|| chrono::Local::now().date_naive().format("%Y-%m-%d").to_string());
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
pub fn stage_update(state: State<'_, AppState>, stage_id: i64, payload: StagePayload) -> AppResult<Value> {
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
        conn.execute("UPDATE log_entry SET stage_id = ?1 WHERE id = ?2", params![next_stage_id, log_id])?;
    }
    conn.execute("DELETE FROM daily_data WHERE stage_id = ?1", params![stage_id])?;
    conn.execute("DELETE FROM weekly_data WHERE stage_id = ?1", params![stage_id])?;
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
    conn.execute("INSERT INTO category (name) VALUES (?1)", params![payload.name.trim()])?;
    let category_id = conn.last_insert_rowid();
    Ok(json!({
        "success": true,
        "message": "分类创建成功",
        "category": { "id": category_id, "name": payload.name.trim(), "user_id": 1 }
    }))
}

#[tauri::command]
pub fn category_update(state: State<'_, AppState>, category_id: i64, payload: CategoryPayload) -> AppResult<Value> {
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
    conn.execute("DELETE FROM sub_category WHERE id = ?1", params![subcategory_id])?;
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
    let source =
        subcategory_json_by_id(&conn, subcategory_id)?.ok_or_else(|| invalid("待合并子分类不存在"))?;
    let target = subcategory_json_by_id(&conn, payload.target_subcategory_id)?
        .ok_or_else(|| invalid("目标子分类不存在"))?;
    let moved = conn.execute(
        "UPDATE log_entry SET subcategory_id = ?1 WHERE subcategory_id = ?2",
        params![payload.target_subcategory_id, subcategory_id],
    )?;
    conn.execute("DELETE FROM sub_category WHERE id = ?1", params![subcategory_id])?;
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
    let mut stmt = conn.prepare("SELECT id FROM log_entry WHERE stage_id = ?1 ORDER BY log_date ASC, id ASC")?;
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
pub fn records_recent(
    state: State<'_, AppState>,
    query: RecentRecordsQuery,
) -> AppResult<Value> {
    let conn = connection(&state)?;
    let limit = query.limit.unwrap_or(10).clamp(1, 50);
    let mut stmt = conn.prepare("SELECT id FROM log_entry ORDER BY datetime(created_at) DESC LIMIT ?1")?;
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
        .query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, Option<i64>>(1)?)))?
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

fn aggregate_subcategory_metric(
    conn: &rusqlite::Connection,
    subcategory_id: i64,
    stage_id: Option<i64>,
    start_date: Option<chrono::NaiveDate>,
    end_date: Option<chrono::NaiveDate>,
    metric_mode: &str,
) -> Result<f64> {
    let mut sql =
        "SELECT log_date, COALESCE(actual_duration, 0), COALESCE(mood, 3) FROM log_entry WHERE subcategory_id = ?1"
            .to_string();
    if let Some(stage_id) = stage_id {
        sql.push_str(&format!(" AND stage_id = {stage_id}"));
    }
    sql.push_str(" ORDER BY log_date ASC");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map(params![subcategory_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let filtered = rows
        .into_iter()
        .filter(|(date, _, _)| {
            let log_date = db::parse_date(date).ok();
            match (log_date, start_date, end_date) {
                (Some(log_date), Some(start), Some(end)) => log_date >= start && log_date <= end,
                (Some(log_date), Some(start), None) => log_date >= start,
                (Some(log_date), None, Some(end)) => log_date <= end,
                (Some(_), None, None) => true,
                _ => false,
            }
        })
        .collect::<Vec<_>>();
    if metric_mode == "efficiency" {
        let total_duration: i64 = filtered.iter().map(|item| item.1).sum();
        if total_duration == 0 {
            return Ok(0.0);
        }
        let weighted: i64 = filtered.iter().map(|item| item.1 * item.2).sum();
        let avg_mood = weighted as f64 / total_duration as f64;
        let total_hours = total_duration as f64 / 60.0;
        return Ok((avg_mood * (1.0 + total_hours).ln() * 100.0).round() / 100.0);
    }
    Ok((filtered.iter().map(|item| item.1).sum::<i64>() as f64 / 60.0 * 100.0).round() / 100.0)
}

fn category_chart_data(
    conn: &rusqlite::Connection,
    stage_id: Option<i64>,
    start_date: Option<chrono::NaiveDate>,
    end_date: Option<chrono::NaiveDate>,
    metric_mode: &str,
) -> Result<Value> {
    let categories = categories_json(conn, true)?;
    let mut main_labels = Vec::new();
    let mut main_data = Vec::new();
    let mut drilldown = serde_json::Map::new();
    for category in categories {
        let category_name = category["name"].as_str().unwrap_or_default().to_string();
        let mut sub_labels = Vec::new();
        let mut sub_values = Vec::new();
        let mut total = 0.0_f64;
        for sub in category["subcategories"].as_array().cloned().unwrap_or_default() {
            let sub_id = sub["id"].as_i64().unwrap_or_default();
            let value = aggregate_subcategory_metric(conn, sub_id, stage_id, start_date, end_date, metric_mode)?;
            if value > 0.0 {
                sub_labels.push(sub["name"].as_str().unwrap_or_default().to_string());
                sub_values.push(value);
                total += value;
            }
        }
        if total > 0.0 {
            main_labels.push(category_name.clone());
            main_data.push((total * 100.0).round() / 100.0);
            drilldown.insert(category_name, json!({ "labels": sub_labels, "data": sub_values }));
        }
    }
    Ok(json!({
        "main": { "labels": main_labels, "data": main_data },
        "drilldown": drilldown
    }))
}

#[tauri::command]
pub fn charts_overview(state: State<'_, AppState>, query: ChartsOverviewQuery) -> AppResult<Value> {
    let conn = connection(&state)?;
    let stage_id = query
        .stage_id
        .filter(|item| item != "all" && !item.is_empty())
        .and_then(|item| item.parse::<i64>().ok());
    let (weekly_labels, weekly_duration, weekly_efficiency) = labels_for_weekly(&conn, stage_id)?;
    let (daily_labels, daily_duration, daily_efficiency) = labels_for_daily(&conn, stage_id)?;
    let avg_daily_minutes = if daily_duration.is_empty() {
        0
    } else {
        ((daily_duration.iter().sum::<f64>() / daily_duration.len() as f64) * 60.0).round() as i64
    };
    Ok(json!({
        "has_data": !weekly_labels.is_empty() || !daily_labels.is_empty(),
        "kpis": {
            "avg_daily_minutes": avg_daily_minutes,
            "avg_daily_formatted": db::format_minutes(avg_daily_minutes),
            "efficiency_star": if daily_efficiency.is_empty() { Value::String("--".to_string()) } else { json!((daily_efficiency.iter().sum::<f64>() / daily_efficiency.len() as f64 * 100.0).round() / 100.0) },
            "weekly_trend": weekly_duration.last().copied().unwrap_or(0.0)
        },
        "weekly_duration_data": {
            "labels": weekly_labels,
            "actuals": weekly_duration,
            "trends": moving_average_points(&weekly_duration),
            "ongoing": false,
            "ongoing_label": Value::Null,
            "ongoing_value": Value::Null,
            "forecast": { "available": false, "status": "unavailable", "reason": "桌面端当前版本未启用本地趋势预测", "labels": [], "prediction": [], "lower": [], "upper": [] }
        },
        "weekly_efficiency_data": {
            "labels": weekly_labels,
            "actuals": weekly_efficiency,
            "trends": moving_average_points(&weekly_efficiency),
            "ongoing": false,
            "ongoing_label": Value::Null,
            "ongoing_value": Value::Null,
            "forecast": { "available": false, "status": "unavailable", "reason": "桌面端当前版本未启用本地趋势预测", "labels": [], "prediction": [], "lower": [], "upper": [] }
        },
        "daily_duration_data": {
            "labels": daily_labels,
            "actuals": daily_duration,
            "trends": moving_average_points(&daily_duration),
            "ongoing": false,
            "ongoing_label": Value::Null,
            "ongoing_value": Value::Null,
            "forecast": { "available": false, "status": "unavailable", "reason": "桌面端当前版本未启用本地趋势预测", "labels": [], "prediction": [], "lower": [], "upper": [] }
        },
        "daily_efficiency_data": {
            "labels": daily_labels,
            "actuals": daily_efficiency,
            "trends": moving_average_points(&daily_efficiency),
            "ongoing": false,
            "ongoing_label": Value::Null,
            "ongoing_value": Value::Null,
            "forecast": { "available": false, "status": "unavailable", "reason": "桌面端当前版本未启用本地趋势预测", "labels": [], "prediction": [], "lower": [], "upper": [] }
        },
        "stage_annotations": stages_json(&conn)?,
        "forecast_status": { "state": "unavailable", "signature": Value::Null, "message": "桌面端当前版本未启用本地趋势预测", "updated_at": Value::Null, "trained_for_date": Value::Null }
    }))
}

#[tauri::command]
pub fn charts_categories(state: State<'_, AppState>, query: ChartsCategoryQuery) -> AppResult<Value> {
    let conn = connection(&state)?;
    let stage_id = query
        .stage_id
        .filter(|item| item != "all" && !item.is_empty())
        .and_then(|item| item.parse::<i64>().ok());
    let _ = query.range_mode;
    let data = category_chart_data(
        &conn,
        stage_id,
        query.start_date.map(|v| db::parse_date(&v)).transpose()?,
        query.end_date.map(|v| db::parse_date(&v)).transpose()?,
        query.metric_mode.as_deref().unwrap_or("duration"),
    )?;
    Ok(data)
}

#[tauri::command]
pub fn charts_category_trend(
    state: State<'_, AppState>,
    query: CategoryTrendQuery,
) -> AppResult<Value> {
    let conn = connection(&state)?;
    let granularity = query.granularity.unwrap_or_else(|| "daily".to_string());
    let metric_mode = query.metric_mode.unwrap_or_else(|| "duration".to_string());
    let start_date = query.start_date.map(|v| db::parse_date(&v)).transpose()?;
    let end_date = query.end_date.map(|v| db::parse_date(&v)).transpose()?;
    let mut map = BTreeMap::<String, f64>::new();

    let subcategory_ids = if let Some(subcategory_id) = query.subcategory_id {
        vec![subcategory_id]
    } else if let Some(category_id) = query.category_id {
        let mut stmt =
            conn.prepare("SELECT id FROM sub_category WHERE category_id = ?1 ORDER BY name ASC")?;
        let ids = stmt
            .query_map(params![category_id], |row| row.get::<_, i64>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        ids
    } else {
        let mut stmt = conn.prepare("SELECT id FROM sub_category ORDER BY name ASC")?;
        let ids = stmt
            .query_map([], |row| row.get::<_, i64>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        ids
    };

    for subcategory_id in subcategory_ids {
        let mut sql =
            "SELECT log_date, COALESCE(actual_duration, 0), COALESCE(mood, 3) FROM log_entry WHERE subcategory_id = ?1"
                .to_string();
        if let Some(stage_id) = query.stage_id {
            sql.push_str(&format!(" AND stage_id = {stage_id}"));
        }
        sql.push_str(" ORDER BY log_date ASC");
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt
            .query_map(params![subcategory_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        for (log_date_str, duration, mood) in rows {
            let log_date = db::parse_date(&log_date_str)?;
            if start_date.is_some_and(|start| log_date < start) {
                continue;
            }
            if end_date.is_some_and(|end| log_date > end) {
                continue;
            }
            let label = if granularity == "weekly" {
                if let Some(stage_id) = query.stage_id {
                    let stage_start = db::stage_start_date(&conn, stage_id)?;
                    let (_, _, year, week_num) = db::get_custom_week_window(log_date, stage_start);
                    format!("{year} W{week_num:02}")
                } else {
                    let iso = log_date.iso_week();
                    format!("{} W{:02}", iso.year(), iso.week())
                }
            } else {
                log_date_str.clone()
            };
            let value = if metric_mode == "efficiency" {
                let hours = duration as f64 / 60.0;
                mood as f64 * (1.0 + hours).ln()
            } else {
                duration as f64 / 60.0
            };
            *map.entry(label).or_insert(0.0) += value;
        }
    }

    let labels = map.keys().cloned().collect::<Vec<_>>();
    let data = map
        .values()
        .map(|item| (item * 100.0).round() / 100.0)
        .collect::<Vec<_>>();
    Ok(json!({
        "success": true,
        "data": {
            "labels": labels,
            "data": data,
            "granularity": granularity
        }
    }))
}

#[tauri::command]
pub fn charts_stages(state: State<'_, AppState>) -> AppResult<Value> {
    let conn = connection(&state)?;
    Ok(json!({ "success": true, "data": { "stages": stages_json(&conn)? } }))
}
