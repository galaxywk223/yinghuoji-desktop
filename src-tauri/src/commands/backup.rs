use std::collections::HashMap;
use std::fs;
use std::io::{Cursor, Read, Write};
use std::path::Path;

use rusqlite::{params, Connection};
use sanitize_filename::sanitize;
use serde_json::{json, Value};
use tauri::State;
use zip::{write::SimpleFileOptions, CompressionMethod, ZipArchive, ZipWriter};

use crate::{AppResult, AppState};

use super::common::{
    attachment_view_json, categories_json, connection, profile_json, record_json_by_id, stages_json,
};
use crate::db;

fn read_json_entry(
    archive: &mut ZipArchive<Cursor<Vec<u8>>>,
    name: &str,
) -> Result<Vec<Value>, String> {
    let normalized_target = name.replace('\\', "/");
    let entry_name = (0..archive.len()).find_map(|index| {
        let file = archive.by_index(index).ok()?;
        let candidate = file.name().replace('\\', "/");
        if candidate == normalized_target || candidate.ends_with(&format!("/{normalized_target}")) {
            Some(file.name().to_string())
        } else {
            None
        }
    });

    match entry_name {
        Some(entry_name) => match archive.by_name(&entry_name) {
            Ok(mut file) => {
                let mut buf = Vec::new();
                file.read_to_end(&mut buf).map_err(|e| e.to_string())?;
                serde_json::from_slice::<Vec<Value>>(&buf).map_err(|e| e.to_string())
            }
            Err(_) => Ok(Vec::new()),
        },
        None => Ok(Vec::new()),
    }
}

fn is_attachment_entry(name: &str) -> bool {
    let normalized = name.replace('\\', "/");
    !normalized.ends_with('/')
        && (normalized.starts_with("attachments/") || normalized.contains("/attachments/"))
}

fn value_as_i64(value: &Value) -> Option<i64> {
    value
        .as_i64()
        .or_else(|| value.as_str().and_then(|item| item.parse::<i64>().ok()))
}

fn value_as_string(value: &Value) -> Option<String> {
    value
        .as_str()
        .map(|item| item.to_string())
        .or_else(|| (!value.is_null()).then(|| value.to_string()))
}

fn basename(value: &str) -> String {
    Path::new(value)
        .file_name()
        .and_then(|item| item.to_str())
        .unwrap_or(value)
        .to_string()
}

fn attachment_import_key(title: &str, event_date: &str, description: &str) -> String {
    format!(
        "{}\u{1f}|{}\u{1f}|{}",
        title.trim(),
        event_date.trim(),
        description.trim()
    )
}

fn attachment_preferred_name(original_path: &str, original_filename: &str) -> String {
    let original_filename = basename(original_filename);
    let original_path_name = basename(original_path);
    let original_filename_has_ext = Path::new(&original_filename)
        .extension()
        .and_then(|item| item.to_str())
        .is_some();
    let original_path_has_ext = Path::new(&original_path_name)
        .extension()
        .and_then(|item| item.to_str())
        .is_some();

    if original_filename_has_ext {
        original_filename
    } else if original_path_has_ext {
        original_path_name
    } else if !original_filename.trim().is_empty() {
        original_filename
    } else {
        original_path_name
    }
}

fn next_unique_attachment_name(
    state: &AppState,
    used_names: &mut HashMap<String, usize>,
    preferred_name: &str,
) -> String {
    let fallback = if preferred_name.trim().is_empty() {
        "attachment.bin"
    } else {
        preferred_name
    };
    let sanitized = sanitize(basename(fallback));
    let stem = Path::new(&sanitized)
        .file_stem()
        .and_then(|item| item.to_str())
        .unwrap_or("attachment");
    let ext = Path::new(&sanitized)
        .extension()
        .and_then(|item| item.to_str())
        .map(|item| format!(".{item}"))
        .unwrap_or_default();

    let mut index = used_names.get(&sanitized).copied().unwrap_or(0);
    loop {
        let candidate = if index == 0 {
            sanitized.clone()
        } else {
            format!("{stem}_{index}{ext}")
        };
        if !state.attachments_dir.join(&candidate).exists() {
            used_names.insert(sanitized.clone(), index + 1);
            return candidate;
        }
        index += 1;
    }
}

fn consume_attachment_bytes(
    attachment_blobs: &mut HashMap<String, Vec<Vec<u8>>>,
    candidates: &[String],
) -> Option<Vec<u8>> {
    for candidate in candidates {
        if let Some(bytes_list) = attachment_blobs.get_mut(candidate) {
            if let Some(bytes) = bytes_list.pop() {
                if bytes_list.is_empty() {
                    attachment_blobs.remove(candidate);
                }
                return Some(bytes);
            }
        }
    }
    None
}

fn ai_insights_json(conn: &Connection) -> rusqlite::Result<Vec<Value>> {
    let mut stmt = conn.prepare(
        "SELECT id, insight_type, scope, scope_reference, start_date, end_date, next_start_date,
                next_end_date, input_snapshot, output_text, created_at
         FROM ai_insight
         ORDER BY id ASC",
    )?;
    let rows = stmt.query_map([], |row| {
        let snapshot: Option<String> = row.get(8)?;
        Ok(json!({
            "id": row.get::<_, i64>(0)?,
            "user_id": 1,
            "insight_type": row.get::<_, String>(1)?,
            "scope": row.get::<_, String>(2)?,
            "scope_reference": row.get::<_, Option<i64>>(3)?,
            "start_date": row.get::<_, Option<String>>(4)?,
            "end_date": row.get::<_, Option<String>>(5)?,
            "next_start_date": row.get::<_, Option<String>>(6)?,
            "next_end_date": row.get::<_, Option<String>>(7)?,
            "input_snapshot": snapshot.and_then(|item| serde_json::from_str::<Value>(&item).ok()),
            "output_text": row.get::<_, String>(9)?,
            "created_at": row.get::<_, String>(10)?,
        }))
    })?;
    rows.collect()
}

fn ai_chat_sessions_json(conn: &Connection) -> rusqlite::Result<Vec<Value>> {
    let mut stmt = conn.prepare(
        "SELECT id, title, scope, scope_reference, date_reference, created_at, updated_at,
                last_message_at
         FROM ai_chat_session
         ORDER BY datetime(last_message_at) DESC, id DESC",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(json!({
            "id": row.get::<_, i64>(0)?,
            "user_id": 1,
            "title": row.get::<_, String>(1)?,
            "scope": row.get::<_, String>(2)?,
            "scope_reference": row.get::<_, Option<i64>>(3)?,
            "date_reference": row.get::<_, Option<String>>(4)?,
            "created_at": row.get::<_, String>(5)?,
            "updated_at": row.get::<_, String>(6)?,
            "last_message_at": row.get::<_, String>(7)?,
        }))
    })?;
    rows.collect()
}

fn ai_chat_messages_json(conn: &Connection) -> rusqlite::Result<Vec<Value>> {
    let mut stmt = conn.prepare(
        "SELECT id, session_id, role, content, scope, scope_reference, date_reference,
                generation_mode, model_name, meta_snapshot, created_at
         FROM ai_chat_message
         ORDER BY id ASC",
    )?;
    let rows = stmt.query_map([], |row| {
        let meta: Option<String> = row.get(9)?;
        Ok(json!({
            "id": row.get::<_, i64>(0)?,
            "session_id": row.get::<_, i64>(1)?,
            "user_id": 1,
            "role": row.get::<_, String>(2)?,
            "content": row.get::<_, String>(3)?,
            "scope": row.get::<_, String>(4)?,
            "scope_reference": row.get::<_, Option<i64>>(5)?,
            "date_reference": row.get::<_, Option<String>>(6)?,
            "generation_mode": row.get::<_, Option<String>>(7)?,
            "model_name": row.get::<_, Option<String>>(8)?,
            "meta": meta
                .and_then(|item| serde_json::from_str::<Value>(&item).ok())
                .unwrap_or_else(|| json!({})),
            "created_at": row.get::<_, String>(10)?,
        }))
    })?;
    rows.collect()
}

#[tauri::command]
pub fn backup_export_zip(state: State<'_, AppState>) -> AppResult<Value> {
    let conn = connection(&state)?;
    let username = profile_json(&conn)?
        .get("username")
        .and_then(Value::as_str)
        .unwrap_or("user")
        .to_string();

    let mut writer = ZipWriter::new(Cursor::new(Vec::<u8>::new()));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    let settings = super::common::settings_json(&conn)?
        .as_object()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(|(key, value)| json!({ "key": key, "value": value, "user_id": 1 }))
        .collect::<Vec<_>>();
    let categories = categories_json(&conn, false)?;
    let subcategories = {
        let mut stmt =
            conn.prepare("SELECT id, name, category_id FROM sub_category ORDER BY id ASC")?;
        let items = stmt
            .query_map([], |row| {
                Ok(json!({
                    "id": row.get::<_, i64>(0)?,
                    "name": row.get::<_, String>(1)?,
                    "category_id": row.get::<_, i64>(2)?
                }))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        items
    };
    let records = {
        let mut stmt = conn.prepare("SELECT id FROM log_entry ORDER BY id ASC")?;
        let ids = stmt
            .query_map([], |row| row.get::<_, i64>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let mut items = Vec::new();
        for id in ids {
            if let Some(item) = record_json_by_id(&conn, id)? {
                items.push(item);
            }
        }
        items
    };
    let daily_data = {
        let mut stmt = conn
            .prepare("SELECT id, log_date, efficiency, stage_id FROM daily_data ORDER BY id ASC")?;
        let items = stmt
            .query_map([], |row| {
                Ok(json!({
                    "id": row.get::<_, i64>(0)?,
                    "log_date": row.get::<_, String>(1)?,
                    "efficiency": row.get::<_, Option<f64>>(2)?,
                    "stage_id": row.get::<_, i64>(3)?
                }))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        items
    };
    let weekly_data = {
        let mut stmt = conn.prepare(
            "SELECT id, year, week_num, efficiency, stage_id FROM weekly_data ORDER BY id ASC",
        )?;
        let items = stmt
            .query_map([], |row| {
                Ok(json!({
                    "id": row.get::<_, i64>(0)?,
                    "year": row.get::<_, i64>(1)?,
                    "week_num": row.get::<_, i64>(2)?,
                    "efficiency": row.get::<_, Option<f64>>(3)?,
                    "stage_id": row.get::<_, i64>(4)?
                }))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        items
    };
    let mottos = super::features::mottos_list(state.clone())?["mottos"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let milestone_categories = super::features::milestone_categories_list(state.clone())?
        ["categories"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let milestones = super::features::milestones_list(
        state.clone(),
        crate::models::MilestonesListQuery {
            category_id: None,
            page: Some(1),
            per_page: Some(5000),
        },
    )?["milestones"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let attachments = {
        let mut stmt = conn.prepare("SELECT id FROM milestone_attachment ORDER BY id ASC")?;
        let ids = stmt
            .query_map([], |row| row.get::<_, i64>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let mut items = Vec::new();
        for id in ids {
            if let Some(item) = attachment_view_json(&conn, id)? {
                items.push(item);
            }
        }
        items
    };
    let countdowns = super::features::countdowns_list(state.clone())?["countdowns"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let local_profile = vec![profile_json(&conn)?];
    let ai_insights = ai_insights_json(&conn)?;
    let ai_chat_sessions = ai_chat_sessions_json(&conn)?;
    let ai_chat_messages = ai_chat_messages_json(&conn)?;

    for (name, value) in [
        ("data/setting.json", json!(settings)),
        ("data/stage.json", json!(stages_json(&conn)?)),
        ("data/category.json", json!(categories)),
        ("data/sub_category.json", json!(subcategories)),
        ("data/log_entry.json", json!(records)),
        ("data/daily_data.json", json!(daily_data)),
        ("data/weekly_data.json", json!(weekly_data)),
        ("data/motto.json", json!(mottos)),
        ("data/milestone_category.json", json!(milestone_categories)),
        ("data/milestone.json", json!(milestones)),
        ("data/milestone_attachment.json", json!(attachments)),
        ("data/countdown_event.json", json!(countdowns)),
        ("data/local_profile.json", json!(local_profile)),
        ("data/ai_insight.json", json!(ai_insights)),
        ("data/ai_chat_session.json", json!(ai_chat_sessions)),
        ("data/ai_chat_message.json", json!(ai_chat_messages)),
    ] {
        writer
            .start_file(name, options)
            .map_err(|e| super::common::invalid(&e.to_string()))?;
        writer
            .write_all(
                &serde_json::to_vec_pretty(&value)
                    .map_err(|e| super::common::invalid(&e.to_string()))?,
            )
            .map_err(|e| super::common::invalid(&e.to_string()))?;
    }

    for entry in fs::read_dir(&state.attachments_dir)? {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            writer.start_file(
                format!("attachments/{}", entry.file_name().to_string_lossy()),
                options,
            )?;
            writer.write_all(&fs::read(entry.path())?)?;
        }
    }

    let bytes = writer
        .finish()
        .map_err(|e| super::common::invalid(&e.to_string()))?
        .into_inner();
    Ok(json!({
        "success": true,
        "data": bytes,
        "file_name": format!("yinghuoji_backup_{}.zip", sanitize(username))
    }))
}

#[tauri::command]
pub fn backup_clear_all(state: State<'_, AppState>) -> AppResult<Value> {
    let conn = connection(&state)?;
    for table in [
        "ai_chat_message",
        "ai_chat_session",
        "ai_insight",
        "milestone_attachment",
        "milestone",
        "milestone_category",
        "countdown_event",
        "motto",
        "weekly_data",
        "daily_data",
        "log_entry",
        "sub_category",
        "category",
        "stage",
    ] {
        conn.execute(&format!("DELETE FROM {table}"), [])?;
    }
    db::set_setting(&conn, "active_stage_id", "0")?;
    db::remove_dir_contents(&state.attachments_dir)?;
    Ok(json!({ "success": true, "message": "您的所有个人数据(包括附件)已被成功清空!" }))
}

#[tauri::command]
pub fn backup_import_zip(
    state: State<'_, AppState>,
    file_name: String,
    file_bytes: Vec<u8>,
) -> AppResult<Value> {
    let _ = file_name;
    let mut archive = ZipArchive::new(Cursor::new(file_bytes))
        .map_err(|e| super::common::invalid(&e.to_string()))?;

    let settings = read_json_entry(&mut archive, "data/setting.json")
        .map_err(|e| super::common::invalid(&e))?;
    let stages =
        read_json_entry(&mut archive, "data/stage.json").map_err(|e| super::common::invalid(&e))?;
    let categories = read_json_entry(&mut archive, "data/category.json")
        .map_err(|e| super::common::invalid(&e))?;
    let subcategories = read_json_entry(&mut archive, "data/sub_category.json")
        .map_err(|e| super::common::invalid(&e))?;
    let records = read_json_entry(&mut archive, "data/log_entry.json")
        .map_err(|e| super::common::invalid(&e))?;
    let mottos =
        read_json_entry(&mut archive, "data/motto.json").map_err(|e| super::common::invalid(&e))?;
    let milestone_categories = read_json_entry(&mut archive, "data/milestone_category.json")
        .map_err(|e| super::common::invalid(&e))?;
    let milestones = read_json_entry(&mut archive, "data/milestone.json")
        .map_err(|e| super::common::invalid(&e))?;
    let milestone_attachments = read_json_entry(&mut archive, "data/milestone_attachment.json")
        .map_err(|e| super::common::invalid(&e))?;
    let countdowns = read_json_entry(&mut archive, "data/countdown_event.json")
        .map_err(|e| super::common::invalid(&e))?;
    let local_profiles = read_json_entry(&mut archive, "data/local_profile.json")
        .map_err(|e| super::common::invalid(&e))?;
    let ai_insights = read_json_entry(&mut archive, "data/ai_insight.json")
        .map_err(|e| super::common::invalid(&e))?;
    let ai_chat_sessions = read_json_entry(&mut archive, "data/ai_chat_session.json")
        .map_err(|e| super::common::invalid(&e))?;
    let ai_chat_messages = read_json_entry(&mut archive, "data/ai_chat_message.json")
        .map_err(|e| super::common::invalid(&e))?;

    let mut attachment_blobs = HashMap::<String, Vec<Vec<u8>>>::new();
    for idx in 0..archive.len() {
        let mut file = archive
            .by_index(idx)
            .map_err(|e| super::common::invalid(&e.to_string()))?;
        let name = file.name().to_string();
        if is_attachment_entry(&name) {
            let mut buf = Vec::new();
            file.read_to_end(&mut buf)
                .map_err(|e| super::common::invalid(&e.to_string()))?;
            attachment_blobs
                .entry(basename(&name))
                .or_default()
                .push(buf);
        }
    }

    let mut conn = connection(&state)?;
    let tx = conn.transaction()?;

    for table in [
        "ai_chat_message",
        "ai_chat_session",
        "ai_insight",
        "milestone_attachment",
        "milestone",
        "milestone_category",
        "countdown_event",
        "motto",
        "weekly_data",
        "daily_data",
        "log_entry",
        "sub_category",
        "category",
        "stage",
    ] {
        tx.execute(&format!("DELETE FROM {table}"), [])?;
    }
    db::set_setting(&tx, "active_stage_id", "0")?;
    let _ = db::remove_dir_contents(&state.attachments_dir);

    if let Some(profile) = local_profiles.first() {
        tx.execute(
            "UPDATE local_profile SET username = ?1, email = ?2, created_at = ?3 WHERE id = 1",
            params![
                profile["username"].as_str().unwrap_or("学习者"),
                profile["email"].as_str().unwrap_or(""),
                profile["created_at"]
                    .as_str()
                    .unwrap_or(&db::now_local_iso())
            ],
        )?;
    }

    for item in &settings {
        if let Some(key) = item["key"].as_str() {
            let value = item["value"]
                .as_str()
                .map(|text| text.to_string())
                .unwrap_or_else(|| item["value"].to_string());
            db::set_setting(&tx, key, &value)?;
        }
    }

    let mut stage_map = HashMap::<i64, i64>::new();
    for item in stages {
        tx.execute(
            "INSERT INTO stage (name, start_date) VALUES (?1, ?2)",
            params![
                item["name"].as_str().unwrap_or("未命名阶段"),
                item["start_date"].as_str().unwrap_or("")
            ],
        )?;
        if let Some(old_id) = value_as_i64(&item["id"]) {
            stage_map.insert(old_id, tx.last_insert_rowid());
        }
    }

    let mut category_map = HashMap::<i64, i64>::new();
    let mut category_name_map = HashMap::<String, i64>::new();
    for item in categories {
        let name = item["name"].as_str().unwrap_or("未命名分类").trim();
        tx.execute("INSERT INTO category (name) VALUES (?1)", params![name])?;
        let new_id = tx.last_insert_rowid();
        category_name_map.insert(name.to_string(), new_id);
        if let Some(old_id) = value_as_i64(&item["id"]) {
            category_map.insert(old_id, new_id);
        }
    }

    let mut sub_map = HashMap::<i64, i64>::new();
    let mut sub_name_map = HashMap::<(i64, String), i64>::new();
    for item in subcategories {
        let Some(category_id) = value_as_i64(&item["category_id"])
            .and_then(|old_id| category_map.get(&old_id).copied())
        else {
            continue;
        };
        let name = item["name"].as_str().unwrap_or("未命名标签").trim();
        tx.execute(
            "INSERT INTO sub_category (name, category_id) VALUES (?1, ?2)",
            params![name, category_id],
        )?;
        let new_id = tx.last_insert_rowid();
        sub_name_map.insert((category_id, name.to_string()), new_id);
        if let Some(old_id) = value_as_i64(&item["id"]) {
            sub_map.insert(old_id, new_id);
        }
    }

    let mut milestone_category_map = HashMap::<i64, i64>::new();
    for item in milestone_categories {
        let name = item["name"].as_str().unwrap_or("未命名里程碑分类").trim();
        tx.execute(
            "INSERT INTO milestone_category (name) VALUES (?1)",
            params![name],
        )?;
        if let Some(old_id) = value_as_i64(&item["id"]) {
            milestone_category_map.insert(old_id, tx.last_insert_rowid());
        }
    }

    let mut milestone_map = HashMap::<i64, i64>::new();
    let mut milestone_key_map = HashMap::<String, i64>::new();
    let mut milestone_key_by_old_id = HashMap::<i64, String>::new();
    for item in milestones {
        let category_id = value_as_i64(&item["category_id"])
            .and_then(|old_id| milestone_category_map.get(&old_id).copied());
        let title = item["title"].as_str().unwrap_or("未命名成就");
        let event_date = item["event_date"]
            .as_str()
            .unwrap_or_else(|| item["target_date"].as_str().unwrap_or(""));
        let description = item["description"].as_str().unwrap_or("");
        tx.execute(
            "INSERT INTO milestone (title, event_date, description, category_id, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                title,
                event_date,
                description,
                category_id,
                item["created_at"].as_str().unwrap_or(&db::now_local_iso())
            ],
        )?;
        let new_milestone_id = tx.last_insert_rowid();
        let import_key = attachment_import_key(title, event_date, description);
        milestone_key_map.insert(import_key.clone(), new_milestone_id);
        if let Some(old_id) = value_as_i64(&item["id"]) {
            milestone_map.insert(old_id, new_milestone_id);
            milestone_key_by_old_id.insert(old_id, import_key);
        }
    }

    for item in records {
        let log_date = item["log_date"].as_str().unwrap_or("");
        let mapped_stage_id = value_as_i64(&item["stage_id"])
            .and_then(|old_id| stage_map.get(&old_id).copied())
            .or_else(|| {
                db::parse_date(log_date).ok().and_then(|date| {
                    db::stage_for_date(&tx, date)
                        .ok()
                        .flatten()
                        .map(|tuple| tuple.0)
                })
            })
            .ok_or_else(|| super::common::invalid("导入失败：无法为学习记录匹配阶段"))?;

        let mapped_subcategory_id = value_as_i64(&item["subcategory_id"])
            .and_then(|old_id| sub_map.get(&old_id).copied())
            .or_else(|| {
                let sub = &item["subcategory"];
                let category_id = value_as_i64(&sub["category_id"])
                    .and_then(|old_id| category_map.get(&old_id).copied())
                    .or_else(|| {
                        sub["category"]["name"]
                            .as_str()
                            .and_then(|name| category_name_map.get(name).copied())
                    });
                match (category_id, sub["name"].as_str()) {
                    (Some(category_id), Some(sub_name)) => sub_name_map
                        .get(&(category_id, sub_name.to_string()))
                        .copied(),
                    _ => None,
                }
            });

        tx.execute(
            "INSERT INTO log_entry (
                log_date, time_slot, task, actual_duration, legacy_category, mood, notes,
                stage_id, subcategory_id, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                log_date,
                item["time_slot"].as_str(),
                item["task"].as_str().unwrap_or(""),
                value_as_i64(&item["actual_duration"]).unwrap_or(0),
                item["legacy_category"].as_str(),
                value_as_i64(&item["mood"]),
                item["notes"].as_str().unwrap_or(""),
                mapped_stage_id,
                mapped_subcategory_id,
                item["created_at"].as_str().unwrap_or(&db::now_utc_iso()),
                item["updated_at"].as_str()
            ],
        )?;
    }

    let mut attachment_name_uses = HashMap::<String, usize>::new();
    let mut missing_attachment_targets = Vec::<String>::new();
    let mut missing_attachment_binaries = Vec::<String>::new();
    for item in milestone_attachments {
        let milestone_id = value_as_i64(&item["milestone_id"]).and_then(|old_id| {
            milestone_map.get(&old_id).copied().or_else(|| {
                milestone_key_by_old_id
                    .get(&old_id)
                    .and_then(|key| milestone_key_map.get(key).copied())
            })
        });
        let Some(milestone_id) = milestone_id else {
            let detail = format!(
                "milestone_id={}, file_path={}",
                item["milestone_id"],
                item["file_path"].as_str().unwrap_or("")
            );
            missing_attachment_targets.push(detail);
            continue;
        };
        let original_path = item["file_path"].as_str().unwrap_or("");
        let original_filename = item["original_filename"]
            .as_str()
            .map(|item| item.to_string())
            .unwrap_or_else(|| basename(original_path));
        let file_candidates = vec![basename(original_path), basename(&original_filename)];
        let preferred_name = attachment_preferred_name(original_path, &original_filename);
        let Some(bytes) = consume_attachment_bytes(&mut attachment_blobs, &file_candidates) else {
            missing_attachment_binaries.push(format!(
                "{} ({})",
                original_filename,
                item["file_path"].as_str().unwrap_or("")
            ));
            continue;
        };
        let relative_name =
            next_unique_attachment_name(&state, &mut attachment_name_uses, &preferred_name);
        fs::write(state.attachments_dir.join(&relative_name), bytes)?;

        tx.execute(
            "INSERT INTO milestone_attachment (milestone_id, file_path, original_filename, uploaded_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                milestone_id,
                relative_name,
                original_filename,
                item["uploaded_at"].as_str().unwrap_or(&db::now_local_iso())
            ],
        )?;
    }

    if !missing_attachment_targets.is_empty() {
        let _ = db::remove_dir_contents(&state.attachments_dir);
        return Err(super::common::invalid(&format!(
            "导入失败：{} 个附件无法匹配到目标成就。首个异常：{}",
            missing_attachment_targets.len(),
            missing_attachment_targets[0]
        )));
    }

    if !missing_attachment_binaries.is_empty() {
        let _ = db::remove_dir_contents(&state.attachments_dir);
        return Err(super::common::invalid(&format!(
            "导入失败：ZIP 中缺少 {} 个附件文件。首个缺失项：{}",
            missing_attachment_binaries.len(),
            missing_attachment_binaries[0]
        )));
    }

    for item in countdowns {
        tx.execute(
            "INSERT INTO countdown_event (title, target_datetime_utc, created_at_utc)
             VALUES (?1, ?2, ?3)",
            params![
                item["title"].as_str().unwrap_or("未命名倒计时"),
                item["target_datetime_utc"].as_str().unwrap_or(""),
                item["created_at_utc"]
                    .as_str()
                    .unwrap_or(&db::now_utc_iso())
            ],
        )?;
    }

    for item in mottos {
        tx.execute(
            "INSERT INTO motto (content, created_at) VALUES (?1, ?2)",
            params![
                item["content"].as_str().unwrap_or(""),
                item["created_at"].as_str().unwrap_or(&db::now_local_iso())
            ],
        )?;
    }

    for item in ai_insights {
        tx.execute(
            "INSERT INTO ai_insight (
                insight_type, scope, scope_reference, start_date, end_date, next_start_date,
                next_end_date, input_snapshot, output_text, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                item["insight_type"].as_str().unwrap_or("analysis"),
                item["scope"].as_str().unwrap_or("global"),
                value_as_i64(&item["scope_reference"]),
                item["start_date"].as_str(),
                item["end_date"].as_str(),
                item["next_start_date"].as_str(),
                item["next_end_date"].as_str(),
                value_as_string(&item["input_snapshot"]),
                item["output_text"].as_str().unwrap_or(""),
                item["created_at"].as_str().unwrap_or(&db::now_local_iso())
            ],
        )?;
    }

    let mut ai_session_map = HashMap::<i64, i64>::new();
    for item in ai_chat_sessions {
        tx.execute(
            "INSERT INTO ai_chat_session (
                title, scope, scope_reference, date_reference, created_at, updated_at, last_message_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                item["title"].as_str().unwrap_or("新的对话"),
                item["scope"].as_str().unwrap_or("global"),
                value_as_i64(&item["scope_reference"]),
                item["date_reference"].as_str(),
                item["created_at"].as_str().unwrap_or(&db::now_local_iso()),
                item["updated_at"].as_str().unwrap_or(&db::now_local_iso()),
                item["last_message_at"].as_str().unwrap_or(&db::now_local_iso())
            ],
        )?;
        if let Some(old_id) = value_as_i64(&item["id"]) {
            ai_session_map.insert(old_id, tx.last_insert_rowid());
        }
    }

    for item in ai_chat_messages {
        let Some(session_id) = value_as_i64(&item["session_id"])
            .and_then(|old_id| ai_session_map.get(&old_id).copied())
        else {
            continue;
        };
        tx.execute(
            "INSERT INTO ai_chat_message (
                session_id, role, content, scope, scope_reference, date_reference,
                generation_mode, model_name, meta_snapshot, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                session_id,
                item["role"].as_str().unwrap_or("assistant"),
                item["content"].as_str().unwrap_or(""),
                item["scope"].as_str().unwrap_or("global"),
                value_as_i64(&item["scope_reference"]),
                item["date_reference"].as_str(),
                item["generation_mode"].as_str(),
                item["model_name"].as_str(),
                value_as_string(&item["meta"]),
                item["created_at"].as_str().unwrap_or(&db::now_local_iso())
            ],
        )?;
    }

    for stage_id in stage_map.values() {
        db::recalculate_efficiency_for_stage(&tx, *stage_id)?;
    }

    if let Some(active_stage_id) = settings
        .iter()
        .find(|item| item["key"].as_str() == Some("active_stage_id"))
        .and_then(|item| value_as_i64(&item["value"]))
        .and_then(|old_id| stage_map.get(&old_id).copied())
    {
        db::set_setting(&tx, "active_stage_id", &active_stage_id.to_string())?;
    }

    tx.commit()?;
    Ok(json!({ "success": true, "message": "导入成功" }))
}
