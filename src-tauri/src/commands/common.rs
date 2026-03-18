use std::collections::BTreeMap;
use std::fs;

use anyhow::{anyhow, Result};
use chrono::{Local, Timelike};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};

use crate::db;
use crate::models::AiConfigView;
use crate::{AppState, FrontendError};

pub const KEYRING_SERVICE: &str = "yinghuoji-desktop";
pub const KEYRING_AI_KEY: &str = "qwen-api-key";

pub fn connection(state: &AppState) -> Result<Connection> {
    db::open_connection(state)
}

pub fn invalid(message: &str) -> FrontendError {
    FrontendError {
        message: message.to_string(),
    }
}

pub fn profile_json(conn: &Connection) -> Result<Value> {
    Ok(conn.query_row(
        "SELECT id, username, email, created_at FROM local_profile WHERE id = 1",
        [],
        |row| {
            Ok(json!({
                "id": row.get::<_, i64>(0)?,
                "username": row.get::<_, String>(1)?,
                "email": row.get::<_, String>(2)?,
                "created_at": row.get::<_, String>(3)?,
            }))
        },
    )?)
}

pub fn settings_json(conn: &Connection) -> Result<Value> {
    let mut stmt = conn.prepare("SELECT key, value FROM app_setting ORDER BY key ASC")?;
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut map = serde_json::Map::new();
    for (key, value) in rows {
        if let Ok(parsed) = serde_json::from_str::<Value>(&value) {
            map.insert(key, parsed);
        } else {
            map.insert(key, Value::String(value));
        }
    }
    Ok(Value::Object(map))
}

pub fn stage_json_by_id(conn: &Connection, stage_id: i64) -> Result<Option<Value>> {
    Ok(conn
        .query_row(
            "SELECT id, name, start_date FROM stage WHERE id = ?1",
            params![stage_id],
            |row| {
                Ok(json!({
                    "id": row.get::<_, i64>(0)?,
                    "name": row.get::<_, String>(1)?,
                    "start_date": row.get::<_, String>(2)?,
                    "user_id": 1
                }))
            },
        )
        .optional()?)
}

pub fn stages_json(conn: &Connection) -> Result<Vec<Value>> {
    let mut stmt =
        conn.prepare("SELECT id, name, start_date FROM stage ORDER BY start_date DESC, id DESC")?;
    let items = stmt
        .query_map([], |row| {
            Ok(json!({
                "id": row.get::<_, i64>(0)?,
                "name": row.get::<_, String>(1)?,
                "start_date": row.get::<_, String>(2)?,
                "user_id": 1
            }))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(items)
}

pub fn categories_json(conn: &Connection, include_subcategories: bool) -> Result<Vec<Value>> {
    let mut stmt = conn.prepare("SELECT id, name FROM category ORDER BY name ASC")?;
    let categories = stmt
        .query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut result = Vec::new();
    for (category_id, name) in categories {
        let mut item = serde_json::Map::from_iter(vec![
            ("id".to_string(), json!(category_id)),
            ("name".to_string(), json!(name)),
            ("user_id".to_string(), json!(1)),
        ]);
        if include_subcategories {
            let mut sub_stmt = conn.prepare(
                "SELECT id, name FROM sub_category WHERE category_id = ?1 ORDER BY name ASC",
            )?;
            let subs = sub_stmt
                .query_map(params![category_id], |row| {
                    Ok(json!({
                        "id": row.get::<_, i64>(0)?,
                        "name": row.get::<_, String>(1)?,
                        "category_id": category_id
                    }))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            item.insert("subcategories".to_string(), Value::Array(subs));
        }
        result.push(Value::Object(item));
    }
    Ok(result)
}

pub fn subcategory_json_by_id(conn: &Connection, subcategory_id: i64) -> Result<Option<Value>> {
    Ok(conn
        .query_row(
            "SELECT sc.id, sc.name, sc.category_id, c.name
             FROM sub_category sc
             JOIN category c ON c.id = sc.category_id
             WHERE sc.id = ?1",
            params![subcategory_id],
            |row| {
                Ok(json!({
                    "id": row.get::<_, i64>(0)?,
                    "name": row.get::<_, String>(1)?,
                    "category_id": row.get::<_, i64>(2)?,
                    "category": {
                        "id": row.get::<_, i64>(2)?,
                        "name": row.get::<_, String>(3)?
                    }
                }))
            },
        )
        .optional()?)
}

pub fn record_json_by_id(conn: &Connection, record_id: i64) -> Result<Option<Value>> {
    Ok(conn
        .query_row(
            "SELECT le.id, le.task, le.log_date, le.time_slot, COALESCE(le.actual_duration, 0), le.mood,
                    COALESCE(le.notes, ''), le.created_at, le.stage_id, le.subcategory_id,
                    st.name, st.start_date, sc.name, c.id, c.name
             FROM log_entry le
             JOIN stage st ON st.id = le.stage_id
             LEFT JOIN sub_category sc ON sc.id = le.subcategory_id
             LEFT JOIN category c ON c.id = sc.category_id
             WHERE le.id = ?1",
            params![record_id],
            |row| {
                let actual_duration = row.get::<_, i64>(4)?;
                let category_id: Option<i64> = row.get(13)?;
                let category_name: Option<String> = row.get(14)?;
                let subcategory_id: Option<i64> = row.get(9)?;
                let subcategory_name: Option<String> = row.get(12)?;
                Ok(json!({
                    "id": row.get::<_, i64>(0)?,
                    "task": row.get::<_, String>(1)?,
                    "log_date": row.get::<_, String>(2)?,
                    "time_slot": row.get::<_, Option<String>>(3)?,
                    "actual_duration": actual_duration,
                    "duration_hours": actual_duration / 60,
                    "duration_minutes": actual_duration % 60,
                    "duration_formatted": db::format_minutes(actual_duration),
                    "mood": row.get::<_, Option<i64>>(5)?,
                    "notes": row.get::<_, String>(6)?,
                    "created_at": row.get::<_, String>(7)?,
                    "stage_id": row.get::<_, i64>(8)?,
                    "stage": {
                        "id": row.get::<_, i64>(8)?,
                        "name": row.get::<_, String>(10)?,
                        "start_date": row.get::<_, String>(11)?
                    },
                    "subcategory_id": subcategory_id,
                    "category_id": category_id,
                    "subcategory": subcategory_id.map(|sid| json!({
                        "id": sid,
                        "name": subcategory_name,
                        "category_id": category_id,
                        "category": category_id.map(|cid| json!({
                            "id": cid,
                            "name": category_name
                        }))
                    }))
                }))
            },
        )
        .optional()?)
}

pub fn active_stage_id(conn: &Connection) -> Result<i64> {
    Ok(db::get_setting(conn, "active_stage_id")?
        .and_then(|item| item.parse::<i64>().ok())
        .unwrap_or(0))
}

pub fn ensure_stage_exists(conn: &Connection, stage_id: i64) -> Result<()> {
    let exists: Option<i64> = conn
        .query_row(
            "SELECT id FROM stage WHERE id = ?1",
            params![stage_id],
            |row| row.get(0),
        )
        .optional()?;
    if exists.is_none() {
        return Err(anyhow!("阶段不存在"));
    }
    Ok(())
}

pub fn moving_average_points(values: &[f64]) -> Vec<Value> {
    db::moving_average(values, 7)
        .into_iter()
        .map(|item| item.map(Value::from).unwrap_or(Value::Null))
        .collect()
}

pub fn recent_records_json(conn: &Connection, limit: i64) -> Result<Vec<Value>> {
    let mut stmt =
        conn.prepare("SELECT id FROM log_entry ORDER BY datetime(created_at) DESC LIMIT ?1")?;
    let ids = stmt
        .query_map(params![limit], |row| row.get::<_, i64>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut items = Vec::new();
    for id in ids {
        if let Some(item) = record_json_by_id(conn, id)? {
            items.push(item);
        }
    }
    Ok(items)
}

pub fn attachment_view_json(conn: &Connection, attachment_id: i64) -> Result<Option<Value>> {
    Ok(conn.query_row(
        "SELECT id, milestone_id, file_path, original_filename, uploaded_at FROM milestone_attachment WHERE id = ?1",
        params![attachment_id],
        |row| {
            Ok(json!({
                "id": row.get::<_, i64>(0)?,
                "milestone_id": row.get::<_, i64>(1)?,
                "file_path": row.get::<_, String>(2)?,
                "original_filename": row.get::<_, String>(3)?,
                "uploaded_at": row.get::<_, String>(4)?
            }))
        },
    ).optional()?)
}

pub fn dashboard_greeting() -> &'static str {
    let current_hour = Local::now().hour();
    if (5..12).contains(&current_hour) {
        "早上好"
    } else if (12..18).contains(&current_hour) {
        "下午好"
    } else {
        "晚上好"
    }
}

pub fn ai_config(conn: &Connection) -> Result<AiConfigView> {
    let api_key = keyring::Entry::new(KEYRING_SERVICE, KEYRING_AI_KEY)
        .ok()
        .and_then(|entry| entry.get_password().ok())
        .unwrap_or_default();
    Ok(AiConfigView {
        configured: !api_key.is_empty(),
        enabled: db::get_setting(conn, "ai_enabled")?.unwrap_or_else(|| "true".to_string())
            == "true",
        model_name: db::get_setting(conn, "ai_model_name")?
            .and_then(|v| serde_json::from_str::<String>(&v).ok().or(Some(v)))
            .unwrap_or_else(|| "qwen-plus-2025-07-28".to_string()),
        base_url: db::get_setting(conn, "ai_base_url")?
            .and_then(|v| serde_json::from_str::<String>(&v).ok().or(Some(v)))
            .unwrap_or_else(|| "https://dashscope.aliyuncs.com/compatible-mode/v1".to_string()),
        has_api_key: !api_key.is_empty(),
    })
}

pub fn labels_for_daily(
    conn: &Connection,
    stage_id: Option<i64>,
) -> Result<(Vec<String>, Vec<f64>, Vec<f64>)> {
    let mut duration_map = BTreeMap::<String, i64>::new();
    let mut efficiency_map = BTreeMap::<String, f64>::new();
    let (sql, params_box): (&str, Vec<i64>) = if let Some(stage_id) = stage_id {
        (
            "SELECT log_date, COALESCE(actual_duration, 0) FROM log_entry WHERE stage_id = ?1 ORDER BY log_date ASC, id ASC",
            vec![stage_id],
        )
    } else {
        (
            "SELECT log_date, COALESCE(actual_duration, 0) FROM log_entry ORDER BY log_date ASC, id ASC",
            vec![],
        )
    };

    let mut stmt = conn.prepare(sql)?;
    let logs = if params_box.is_empty() {
        stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?
    } else {
        stmt.query_map(params![params_box[0]], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?
    };
    for (log_date, duration) in logs {
        *duration_map.entry(log_date).or_insert(0) += duration;
    }
    if duration_map.is_empty() {
        return Ok((Vec::new(), Vec::new(), Vec::new()));
    }

    let (eff_sql, eff_params): (&str, Vec<i64>) = if let Some(stage_id) = stage_id {
        (
            "SELECT log_date, COALESCE(efficiency, 0) FROM daily_data WHERE stage_id = ?1 ORDER BY log_date ASC",
            vec![stage_id],
        )
    } else {
        (
            "SELECT log_date, COALESCE(efficiency, 0) FROM daily_data ORDER BY log_date ASC",
            vec![],
        )
    };
    let mut eff_stmt = conn.prepare(eff_sql)?;
    let rows = if eff_params.is_empty() {
        eff_stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?
    } else {
        eff_stmt
            .query_map(params![eff_params[0]], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?
    };
    let mut grouped = BTreeMap::<String, Vec<f64>>::new();
    for (log_date, score) in rows {
        grouped.entry(log_date).or_default().push(score);
    }
    for (date, scores) in grouped {
        let avg = if scores.is_empty() {
            0.0
        } else {
            scores.iter().sum::<f64>() / scores.len() as f64
        };
        efficiency_map.insert(date, avg);
    }

    let first_date = duration_map
        .keys()
        .next()
        .and_then(|item| db::parse_date(item).ok())
        .ok_or_else(|| anyhow!("无法解析首条学习记录日期"))?;
    let last_date = duration_map
        .keys()
        .next_back()
        .and_then(|item| db::parse_date(item).ok())
        .ok_or_else(|| anyhow!("无法解析末条学习记录日期"))?;
    let labels = (0..=(last_date - first_date).num_days())
        .map(|offset| {
            (first_date + chrono::Duration::days(offset))
                .format("%Y-%m-%d")
                .to_string()
        })
        .collect::<Vec<_>>();
    let duration = labels
        .iter()
        .map(|label| {
            (duration_map.get(label).copied().unwrap_or(0) as f64 / 60.0 * 10.0).round() / 10.0
        })
        .collect::<Vec<_>>();
    let efficiency = labels
        .iter()
        .map(|label| (efficiency_map.get(label).copied().unwrap_or(0.0) * 100.0).round() / 100.0)
        .collect::<Vec<_>>();
    Ok((labels, duration, efficiency))
}

pub fn remove_attachment_file(state: &AppState, relative: &str) {
    let file_path = state.attachments_dir.join(relative);
    if file_path.exists() {
        let _ = fs::remove_file(file_path);
    }
}
