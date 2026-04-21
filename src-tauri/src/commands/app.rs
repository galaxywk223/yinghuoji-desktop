use chrono::{Duration, Local, Utc};
use rusqlite::params;
use serde_json::json;
use tauri::{AppHandle, Manager, State};

use crate::models::{ProfileUpdatePayload, SettingItemPayload};
use crate::{AppLifecycleState, AppResult, AppState};

use super::common::{
    connection, dashboard_greeting, profile_json, recent_records_json, settings_json,
};
use crate::db;

#[tauri::command]
pub fn app_prepare_exit_for_update(
    app: AppHandle,
    lifecycle_state: State<'_, AppLifecycleState>,
) -> AppResult<serde_json::Value> {
    lifecycle_state.mark_exit_requested();
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.set_skip_taskbar(false);
    }
    Ok(json!({
        "success": true,
        "message": "已准备进入更新安装流程"
    }))
}

#[tauri::command]
pub fn app_cancel_exit_for_update(
    lifecycle_state: State<'_, AppLifecycleState>,
) -> AppResult<serde_json::Value> {
    lifecycle_state.clear_exit_requested();
    Ok(json!({
        "success": true,
        "message": "已恢复正常关闭拦截状态"
    }))
}

#[tauri::command]
pub fn profile_get(state: State<'_, AppState>) -> AppResult<serde_json::Value> {
    let conn = connection(&state)?;
    Ok(json!({ "success": true, "user": profile_json(&conn)? }))
}

#[tauri::command]
pub fn profile_update(
    state: State<'_, AppState>,
    payload: ProfileUpdatePayload,
) -> AppResult<serde_json::Value> {
    let conn = connection(&state)?;
    conn.execute(
        "UPDATE local_profile SET username = ?1, email = ?2 WHERE id = 1",
        params![payload.username.trim(), payload.email.unwrap_or_default()],
    )?;
    Ok(json!({
        "success": true,
        "message": "本地档案更新成功",
        "user": profile_json(&conn)?
    }))
}

#[tauri::command]
pub fn settings_get(state: State<'_, AppState>) -> AppResult<serde_json::Value> {
    let conn = connection(&state)?;
    Ok(json!({ "success": true, "settings": settings_json(&conn)? }))
}

#[tauri::command]
pub fn settings_set(
    state: State<'_, AppState>,
    items: Vec<SettingItemPayload>,
) -> AppResult<serde_json::Value> {
    let conn = connection(&state)?;
    for item in items {
        db::set_setting(&conn, &item.key, &item.value.to_string())?;
    }
    Ok(json!({
        "success": true,
        "message": "设置已更新",
        "settings": settings_json(&conn)?
    }))
}

#[tauri::command]
pub fn dashboard_summary(state: State<'_, AppState>) -> AppResult<serde_json::Value> {
    let conn = connection(&state)?;
    let today = Local::now().date_naive().format("%Y-%m-%d").to_string();
    let today_minutes: i64 = conn.query_row(
        "SELECT COALESCE(SUM(actual_duration), 0) FROM log_entry WHERE log_date = ?1",
        params![today],
        |row| row.get(0),
    )?;
    let latest_record_date: Option<String> = conn
        .query_row(
            "SELECT log_date FROM log_entry ORDER BY log_date DESC, created_at DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .ok();
    let total_records: i64 =
        conn.query_row("SELECT COUNT(*) FROM log_entry", [], |row| row.get(0))?;
    let milestones_count: i64 =
        conn.query_row("SELECT COUNT(*) FROM milestone", [], |row| row.get(0))?;
    let countdown_total: i64 =
        conn.query_row("SELECT COUNT(*) FROM countdown_event", [], |row| row.get(0))?;
    let next_countdown = conn
        .query_row(
            "SELECT title, target_datetime_utc FROM countdown_event ORDER BY target_datetime_utc ASC LIMIT 1",
            [],
            |row| {
                let title: String = row.get(0)?;
                let target: String = row.get(1)?;
                let target_dt = db::parse_rfc3339(&target)
                    .map_err(|_| rusqlite::Error::InvalidQuery)?;
                let remaining_days = (target_dt.date_naive() - Utc::now().date_naive())
                    .num_days()
                    .max(0);
                Ok(json!({
                    "title": title,
                    "remaining_days": remaining_days
                }))
            },
        )
        .ok();
    let random_motto = conn
        .query_row(
            "SELECT id, content FROM motto ORDER BY RANDOM() LIMIT 1",
            [],
            |row| {
                Ok(json!({
                    "id": row.get::<_, i64>(0)?,
                    "content": row.get::<_, String>(1)?
                }))
            },
        )
        .ok();

    let _ = recent_records_json(&conn, 5)?;
    let _ = Duration::days(0);

    Ok(json!({
        "success": true,
        "data": {
            "greeting": dashboard_greeting(),
            "today_duration_minutes": today_minutes,
            "today_duration_formatted": db::format_minutes(today_minutes),
            "total_records": total_records,
            "latest_record_date": latest_record_date,
            "countdown_total": countdown_total,
            "next_countdown": next_countdown,
            "milestones_count": milestones_count,
            "random_motto": random_motto
        }
    }))
}
