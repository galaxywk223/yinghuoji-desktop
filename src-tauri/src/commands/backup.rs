use std::collections::HashMap;
use std::fs;
use std::io::{Cursor, Read, Write};

use rusqlite::params;
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
    match archive.by_name(name) {
        Ok(mut file) => {
            let mut buf = Vec::new();
            file.read_to_end(&mut buf).map_err(|e| e.to_string())?;
            Ok(serde_json::from_slice::<Vec<Value>>(&buf).unwrap_or_default())
        }
        Err(_) => Ok(Vec::new()),
    }
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
        let mut stmt = conn.prepare("SELECT id, name, category_id FROM sub_category ORDER BY id ASC")?;
        let items = stmt.query_map([], |row| {
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
        let mut stmt = conn.prepare("SELECT id, log_date, efficiency, stage_id FROM daily_data ORDER BY id ASC")?;
        let items = stmt.query_map([], |row| {
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
        let mut stmt = conn.prepare("SELECT id, year, week_num, efficiency, stage_id FROM weekly_data ORDER BY id ASC")?;
        let items = stmt.query_map([], |row| {
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
    let milestone_categories = super::features::milestone_categories_list(state.clone())?["categories"]
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
        let mut stmt =
            conn.prepare("SELECT id FROM milestone_attachment ORDER BY id ASC")?;
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
        ("data/ai_insight.json", json!([])),
        ("data/ai_chat_session.json", super::ai::ai_chat_sessions(state.clone())?["data"].clone()),
        ("data/ai_chat_message.json", super::ai::ai_history_export_messages(state.clone())?),
    ] {
        writer.start_file(name, options).map_err(|e| super::common::invalid(&e.to_string()))?;
        writer
            .write_all(&serde_json::to_vec_pretty(&value).map_err(|e| super::common::invalid(&e.to_string()))?)
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

    let bytes = writer.finish().map_err(|e| super::common::invalid(&e.to_string()))?.into_inner();
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
    backup_clear_all(state.clone())?;
    let conn = connection(&state)?;
    let mut archive =
        ZipArchive::new(Cursor::new(file_bytes)).map_err(|e| super::common::invalid(&e.to_string()))?;

    let settings = read_json_entry(&mut archive, "data/setting.json")
        .map_err(|e| super::common::invalid(&e))?;
    for item in settings {
        if let Some(key) = item["key"].as_str() {
            db::set_setting(&conn, key, &item["value"].to_string())?;
        }
    }

    let mut stage_map = HashMap::<i64, i64>::new();
    for item in read_json_entry(&mut archive, "data/stage.json").map_err(|e| super::common::invalid(&e))? {
        conn.execute(
            "INSERT INTO stage (name, start_date) VALUES (?1, ?2)",
            params![item["name"].as_str().unwrap_or(""), item["start_date"].as_str().unwrap_or("")],
        )?;
        if let Some(old_id) = item["id"].as_i64() {
            stage_map.insert(old_id, conn.last_insert_rowid());
        }
    }

    let mut category_map = HashMap::<i64, i64>::new();
    for item in read_json_entry(&mut archive, "data/category.json").map_err(|e| super::common::invalid(&e))? {
        conn.execute(
            "INSERT INTO category (name) VALUES (?1)",
            params![item["name"].as_str().unwrap_or("")],
        )?;
        if let Some(old_id) = item["id"].as_i64() {
            category_map.insert(old_id, conn.last_insert_rowid());
        }
    }

    let mut sub_map = HashMap::<i64, i64>::new();
    for item in read_json_entry(&mut archive, "data/sub_category.json").map_err(|e| super::common::invalid(&e))? {
        let category_id = item["category_id"]
            .as_i64()
            .and_then(|old| category_map.get(&old).copied())
            .unwrap_or_default();
        conn.execute(
            "INSERT INTO sub_category (name, category_id) VALUES (?1, ?2)",
            params![item["name"].as_str().unwrap_or(""), category_id],
        )?;
        if let Some(old_id) = item["id"].as_i64() {
            sub_map.insert(old_id, conn.last_insert_rowid());
        }
    }

    for item in read_json_entry(&mut archive, "data/log_entry.json").map_err(|e| super::common::invalid(&e))? {
        let stage_id = item["stage_id"]
            .as_i64()
            .and_then(|old| stage_map.get(&old).copied())
            .unwrap_or_default();
        let subcategory_id = item["subcategory_id"]
            .as_i64()
            .and_then(|old| sub_map.get(&old).copied());
        conn.execute(
            "INSERT INTO log_entry (log_date, time_slot, task, actual_duration, mood, notes, stage_id, subcategory_id, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                item["log_date"].as_str().unwrap_or(""),
                item["time_slot"].as_str(),
                item["task"].as_str().unwrap_or(""),
                item["actual_duration"].as_i64().unwrap_or(0),
                item["mood"].as_i64(),
                item["notes"].as_str().unwrap_or(""),
                stage_id,
                subcategory_id,
                item["created_at"].as_str().unwrap_or(&db::now_utc_iso()),
                item["updated_at"].as_str()
            ],
        )?;
    }

    for item in read_json_entry(&mut archive, "data/motto.json").map_err(|e| super::common::invalid(&e))? {
        conn.execute(
            "INSERT INTO motto (content, created_at) VALUES (?1, ?2)",
            params![item["content"].as_str().unwrap_or(""), item["created_at"].as_str().unwrap_or(&db::now_local_iso())],
        )?;
    }

    for idx in 0..archive.len() {
        let mut file = archive
            .by_index(idx)
            .map_err(|e| super::common::invalid(&e.to_string()))?;
        let name = file.name().to_string();
        if name.starts_with("attachments/") && !name.ends_with('/') {
            let relative = sanitize(name.trim_start_matches("attachments/"));
            let mut buf = Vec::new();
            file.read_to_end(&mut buf)
                .map_err(|e| super::common::invalid(&e.to_string()))?;
            fs::write(state.attachments_dir.join(relative), buf)?;
        }
    }

    Ok(json!({ "success": true, "message": "导入成功" }))
}
