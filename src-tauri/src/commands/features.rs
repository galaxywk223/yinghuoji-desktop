use std::fs;

use chrono::Utc;
use rusqlite::{params, OptionalExtension};
use sanitize_filename::sanitize;
use serde_json::{json, Value};
use tauri::State;
use uuid::Uuid;

use crate::models::{
    CountdownPayload, MilestoneCategoryPayload, MilestonePayload, MilestonesListQuery, MottoPayload,
};
use crate::{AppResult, AppState};

use super::common::{attachment_view_json, connection, invalid, remove_attachment_file};
use crate::db;

#[tauri::command]
pub fn countdowns_list(state: State<'_, AppState>) -> AppResult<Value> {
    let conn = connection(&state)?;
    let now = Utc::now();
    let mut stmt = conn.prepare(
        "SELECT id, title, target_datetime_utc, created_at_utc FROM countdown_event ORDER BY target_datetime_utc ASC",
    )?;
    let items = stmt
        .query_map([], |row| {
            let target = row.get::<_, String>(2)?;
            let created = row.get::<_, String>(3)?;
            let target_dt =
                db::parse_rfc3339(&target).map_err(|_| rusqlite::Error::InvalidQuery)?;
            let created_dt =
                db::parse_rfc3339(&created).map_err(|_| rusqlite::Error::InvalidQuery)?;
            let remaining = target_dt - now;
            let is_expired = remaining.num_seconds() < 0;
            let remaining_days = if is_expired {
                0
            } else {
                remaining.num_days().max(0)
            };
            let total = (target_dt - created_dt).num_seconds().max(1);
            let elapsed = (now - created_dt).num_seconds().max(0);
            let progress = ((elapsed as f64 / total as f64) * 100.0).clamp(0.0, 100.0);
            let card_status = if is_expired {
                "expired"
            } else if remaining_days < 1 {
                "urgent"
            } else if remaining_days < 7 {
                "warning"
            } else {
                "normal"
            };
            Ok(json!({
                "id": row.get::<_, i64>(0)?,
                "title": row.get::<_, String>(1)?,
                "target_datetime_utc": target,
                "created_at_utc": created,
                "user_id": 1,
                "remaining_days": remaining_days,
                "is_expired": is_expired,
                "progress_percentage": (progress * 100.0).round() / 100.0,
                "card_status": card_status
            }))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(json!({ "success": true, "countdowns": items }))
}

#[tauri::command]
pub fn countdown_get(state: State<'_, AppState>, countdown_id: i64) -> AppResult<Value> {
    let conn = connection(&state)?;
    let item = conn
        .query_row(
            "SELECT id, title, target_datetime_utc, created_at_utc FROM countdown_event WHERE id = ?1",
            params![countdown_id],
            |row| {
                Ok(json!({
                    "id": row.get::<_, i64>(0)?,
                    "title": row.get::<_, String>(1)?,
                    "target_datetime_utc": row.get::<_, String>(2)?,
                    "created_at_utc": row.get::<_, String>(3)?,
                    "user_id": 1
                }))
            },
        )
        .optional()?
        .ok_or_else(|| invalid("倒计时事件不存在"))?;
    Ok(json!({ "success": true, "countdown": item }))
}

#[tauri::command]
pub fn countdown_create(state: State<'_, AppState>, payload: CountdownPayload) -> AppResult<Value> {
    let conn = connection(&state)?;
    conn.execute(
        "INSERT INTO countdown_event (title, target_datetime_utc, created_at_utc) VALUES (?1, ?2, ?3)",
        params![payload.title.trim(), payload.target_datetime_utc, db::now_utc_iso()],
    )?;
    let id = conn.last_insert_rowid();
    Ok(json!({
        "success": true,
        "message": "倒计时事件创建成功",
        "countdown": countdown_get(state, id)?.get("countdown").cloned()
    }))
}

#[tauri::command]
pub fn countdown_update(
    state: State<'_, AppState>,
    countdown_id: i64,
    payload: CountdownPayload,
) -> AppResult<Value> {
    let conn = connection(&state)?;
    conn.execute(
        "UPDATE countdown_event SET title = ?1, target_datetime_utc = ?2 WHERE id = ?3",
        params![
            payload.title.trim(),
            payload.target_datetime_utc,
            countdown_id
        ],
    )?;
    Ok(json!({
        "success": true,
        "message": "倒计时事件更新成功",
        "countdown": countdown_get(state, countdown_id)?.get("countdown").cloned()
    }))
}

#[tauri::command]
pub fn countdown_delete(state: State<'_, AppState>, countdown_id: i64) -> AppResult<Value> {
    let conn = connection(&state)?;
    conn.execute(
        "DELETE FROM countdown_event WHERE id = ?1",
        params![countdown_id],
    )?;
    Ok(json!({ "success": true, "message": "倒计时事件已删除" }))
}

#[tauri::command]
pub fn mottos_list(state: State<'_, AppState>) -> AppResult<Value> {
    let conn = connection(&state)?;
    let mut stmt = conn.prepare("SELECT id, content, created_at FROM motto ORDER BY id DESC")?;
    let items = stmt
        .query_map([], |row| {
            Ok(json!({
                "id": row.get::<_, i64>(0)?,
                "content": row.get::<_, String>(1)?,
                "user_id": 1,
                "created_at": row.get::<_, String>(2)?,
                "is_favorite": false
            }))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(json!({ "success": true, "mottos": items }))
}

#[tauri::command]
pub fn motto_get(state: State<'_, AppState>, motto_id: i64) -> AppResult<Value> {
    let conn = connection(&state)?;
    let item = conn
        .query_row(
            "SELECT id, content, created_at FROM motto WHERE id = ?1",
            params![motto_id],
            |row| {
                Ok(json!({
                    "id": row.get::<_, i64>(0)?,
                    "content": row.get::<_, String>(1)?,
                    "user_id": 1,
                    "created_at": row.get::<_, String>(2)?,
                    "is_favorite": false
                }))
            },
        )
        .optional()?
        .ok_or_else(|| invalid("座右铭不存在"))?;
    Ok(json!({ "success": true, "motto": item }))
}

#[tauri::command]
pub fn motto_random(state: State<'_, AppState>) -> AppResult<Value> {
    let conn = connection(&state)?;
    let motto = conn
        .query_row(
            "SELECT id, content, created_at FROM motto ORDER BY RANDOM() LIMIT 1",
            [],
            |row| {
                Ok(json!({
                    "id": row.get::<_, i64>(0)?,
                    "content": row.get::<_, String>(1)?,
                    "user_id": 1,
                    "created_at": row.get::<_, String>(2)?,
                    "is_favorite": false
                }))
            },
        )
        .optional()?;
    Ok(json!({
        "success": true,
        "content": motto.as_ref().and_then(|item| item["content"].as_str()).unwrap_or("书山有路勤为径，学海无涯苦作舟。"),
        "motto": motto
    }))
}

#[tauri::command]
pub fn motto_create(state: State<'_, AppState>, payload: MottoPayload) -> AppResult<Value> {
    let conn = connection(&state)?;
    conn.execute(
        "INSERT INTO motto (content, created_at) VALUES (?1, ?2)",
        params![payload.content.trim(), db::now_local_iso()],
    )?;
    let id = conn.last_insert_rowid();
    Ok(json!({
        "success": true,
        "message": "座右铭创建成功",
        "motto": motto_get(state, id)?.get("motto").cloned()
    }))
}

#[tauri::command]
pub fn motto_update(
    state: State<'_, AppState>,
    motto_id: i64,
    payload: MottoPayload,
) -> AppResult<Value> {
    let conn = connection(&state)?;
    conn.execute(
        "UPDATE motto SET content = ?1 WHERE id = ?2",
        params![payload.content.trim(), motto_id],
    )?;
    Ok(json!({
        "success": true,
        "message": "座右铭更新成功",
        "motto": motto_get(state, motto_id)?.get("motto").cloned()
    }))
}

#[tauri::command]
pub fn motto_delete(state: State<'_, AppState>, motto_id: i64) -> AppResult<Value> {
    let conn = connection(&state)?;
    conn.execute("DELETE FROM motto WHERE id = ?1", params![motto_id])?;
    Ok(json!({ "success": true, "message": "座右铭已删除" }))
}

#[tauri::command]
pub fn milestones_list(state: State<'_, AppState>, query: MilestonesListQuery) -> AppResult<Value> {
    let conn = connection(&state)?;
    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(10).clamp(1, 50);
    let filter_sql = query
        .category_id
        .map(|id| format!("WHERE category_id = {id}"))
        .unwrap_or_default();
    let total: i64 = conn.query_row(
        &format!("SELECT COUNT(*) FROM milestone {filter_sql}"),
        [],
        |row| row.get(0),
    )?;
    let mut stmt = conn.prepare(&format!(
        "SELECT id, title, event_date, description, category_id, created_at
         FROM milestone {filter_sql}
         ORDER BY event_date DESC, id DESC
         LIMIT {} OFFSET {}",
        per_page,
        (page - 1) * per_page
    ))?;
    let bases = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<i64>>(4)?,
                row.get::<_, String>(5)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut milestones = Vec::new();
    for (id, title, event_date, description, category_id, created_at) in bases {
        let mut att_stmt = conn.prepare(
            "SELECT id FROM milestone_attachment WHERE milestone_id = ?1 ORDER BY uploaded_at ASC",
        )?;
        let att_ids = att_stmt
            .query_map(params![id], |row| row.get::<_, i64>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let mut attachments = Vec::new();
        for att_id in att_ids {
            if let Some(item) = attachment_view_json(&conn, att_id)? {
                attachments.push(item);
            }
        }
        milestones.push(json!({
            "id": id,
            "title": title,
            "event_date": event_date,
            "description": description,
            "category_id": category_id,
            "user_id": 1,
            "created_at": created_at,
            "attachments": attachments
        }));
    }
    Ok(json!({
        "success": true,
        "milestones": milestones,
        "pagination": {
            "page": page,
            "per_page": per_page,
            "total": total,
            "pages": (total + per_page - 1) / per_page,
            "has_next": page * per_page < total,
            "has_prev": page > 1
        }
    }))
}

#[tauri::command]
pub fn milestone_get(state: State<'_, AppState>, milestone_id: i64) -> AppResult<Value> {
    let list = milestones_list(
        state,
        MilestonesListQuery {
            category_id: None,
            page: Some(1),
            per_page: Some(5000),
        },
    )?;
    let item = list["milestones"]
        .as_array()
        .and_then(|items| {
            items
                .iter()
                .find(|item| item["id"].as_i64() == Some(milestone_id))
                .cloned()
        })
        .ok_or_else(|| invalid("里程碑不存在"))?;
    Ok(json!({ "success": true, "milestone": item }))
}

#[tauri::command]
pub fn milestone_create(state: State<'_, AppState>, payload: MilestonePayload) -> AppResult<Value> {
    let conn = connection(&state)?;
    conn.execute(
        "INSERT INTO milestone (title, event_date, description, category_id, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            payload.title.trim(),
            payload.event_date.unwrap_or_else(|| chrono::Local::now()
                .date_naive()
                .format("%Y-%m-%d")
                .to_string()),
            payload.description.unwrap_or_default(),
            payload.category_id,
            db::now_local_iso()
        ],
    )?;
    let id = conn.last_insert_rowid();
    Ok(json!({
        "success": true,
        "message": "里程碑创建成功",
        "milestone": milestone_get(state, id)?.get("milestone").cloned()
    }))
}

#[tauri::command]
pub fn milestone_update(
    state: State<'_, AppState>,
    milestone_id: i64,
    payload: MilestonePayload,
) -> AppResult<Value> {
    let conn = connection(&state)?;
    let current = milestone_get(state.clone(), milestone_id)?;
    let event_date = payload.event_date.unwrap_or_else(|| {
        current["milestone"]["event_date"]
            .as_str()
            .unwrap_or("")
            .to_string()
    });
    conn.execute(
        "UPDATE milestone SET title = ?1, event_date = ?2, description = ?3, category_id = ?4 WHERE id = ?5",
        params![
            payload.title.trim(),
            event_date,
            payload.description.unwrap_or_default(),
            payload.category_id,
            milestone_id
        ],
    )?;
    Ok(json!({
        "success": true,
        "message": "里程碑更新成功",
        "milestone": milestone_get(state, milestone_id)?.get("milestone").cloned()
    }))
}

#[tauri::command]
pub fn milestone_delete(state: State<'_, AppState>, milestone_id: i64) -> AppResult<Value> {
    let conn = connection(&state)?;
    let mut stmt =
        conn.prepare("SELECT file_path FROM milestone_attachment WHERE milestone_id = ?1")?;
    let paths = stmt
        .query_map(params![milestone_id], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    for path in paths {
        remove_attachment_file(&state, &path);
    }
    conn.execute("DELETE FROM milestone WHERE id = ?1", params![milestone_id])?;
    Ok(json!({ "success": true, "message": "里程碑已删除" }))
}

#[tauri::command]
pub fn milestone_categories_list(state: State<'_, AppState>) -> AppResult<Value> {
    let conn = connection(&state)?;
    let mut stmt = conn.prepare("SELECT id, name FROM milestone_category ORDER BY name ASC")?;
    let items = stmt
        .query_map([], |row| {
            let id = row.get::<_, i64>(0)?;
            let count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM milestone WHERE category_id = ?1",
                    params![id],
                    |count_row| count_row.get(0),
                )
                .unwrap_or(0);
            Ok(json!({
                "id": id,
                "name": row.get::<_, String>(1)?,
                "user_id": 1,
                "milestone_count": count
            }))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(json!({ "success": true, "categories": items }))
}

#[tauri::command]
pub fn milestone_category_create(
    state: State<'_, AppState>,
    payload: MilestoneCategoryPayload,
) -> AppResult<Value> {
    let conn = connection(&state)?;
    conn.execute(
        "INSERT INTO milestone_category (name) VALUES (?1)",
        params![payload.name.trim()],
    )?;
    let id = conn.last_insert_rowid();
    Ok(json!({
        "success": true,
        "message": "分类创建成功",
        "category": { "id": id, "name": payload.name.trim(), "user_id": 1, "milestone_count": 0 }
    }))
}

#[tauri::command]
pub fn milestone_category_update(
    state: State<'_, AppState>,
    category_id: i64,
    payload: MilestoneCategoryPayload,
) -> AppResult<Value> {
    let conn = connection(&state)?;
    conn.execute(
        "UPDATE milestone_category SET name = ?1 WHERE id = ?2",
        params![payload.name.trim(), category_id],
    )?;
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM milestone WHERE category_id = ?1",
            params![category_id],
            |row| row.get(0),
        )
        .unwrap_or(0);
    Ok(json!({
        "success": true,
        "message": "分类更新成功",
        "category": { "id": category_id, "name": payload.name.trim(), "user_id": 1, "milestone_count": count }
    }))
}

#[tauri::command]
pub fn milestone_category_delete(state: State<'_, AppState>, category_id: i64) -> AppResult<Value> {
    let conn = connection(&state)?;
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM milestone WHERE category_id = ?1",
        params![category_id],
        |row| row.get(0),
    )?;
    if count > 0 {
        return Err(invalid("无法删除：该分类下仍有关联里程碑"));
    }
    conn.execute(
        "DELETE FROM milestone_category WHERE id = ?1",
        params![category_id],
    )?;
    Ok(json!({ "success": true, "message": "分类已删除" }))
}

#[tauri::command]
pub fn milestone_attachment_upload(
    state: State<'_, AppState>,
    milestone_id: i64,
    file_name: String,
    file_bytes: Vec<u8>,
) -> AppResult<Value> {
    let conn = connection(&state)?;
    let ext = file_name
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    let relative = sanitize(format!(
        "milestone_{}_{}.{}",
        milestone_id,
        Uuid::new_v4(),
        ext
    ));
    fs::write(state.attachments_dir.join(&relative), file_bytes)?;
    conn.execute(
        "INSERT INTO milestone_attachment (milestone_id, file_path, original_filename, uploaded_at)
         VALUES (?1, ?2, ?3, ?4)",
        params![milestone_id, relative, file_name, db::now_local_iso()],
    )?;
    let id = conn.last_insert_rowid();
    Ok(json!({
        "success": true,
        "message": "附件上传成功",
        "attachment": attachment_view_json(&conn, id)?
    }))
}

#[tauri::command]
pub fn milestone_attachment_delete(
    state: State<'_, AppState>,
    milestone_id: i64,
    attachment_id: i64,
) -> AppResult<Value> {
    let conn = connection(&state)?;
    let attachment =
        attachment_view_json(&conn, attachment_id)?.ok_or_else(|| invalid("附件不存在"))?;
    if attachment["milestone_id"].as_i64() != Some(milestone_id) {
        return Err(invalid("附件不存在"));
    }
    remove_attachment_file(&state, attachment["file_path"].as_str().unwrap_or(""));
    conn.execute(
        "DELETE FROM milestone_attachment WHERE id = ?1",
        params![attachment_id],
    )?;
    Ok(json!({ "success": true, "message": "附件已删除" }))
}

#[tauri::command]
pub fn milestone_attachment_get(state: State<'_, AppState>, file_path: String) -> AppResult<Value> {
    let bytes = fs::read(state.attachments_dir.join(&file_path))?;
    Ok(json!({ "success": true, "data": bytes, "file_name": file_path }))
}
