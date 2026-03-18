use anyhow::anyhow;
use keyring::Entry;
use reqwest::Client;
use rusqlite::params;
use serde_json::{json, Value};
use tauri::State;

use crate::models::{AiChatSendPayload, AiConfigPayload, AiHistoryQuery};
use crate::{AppResult, AppState};

use super::common::{
    ai_config, connection, invalid, profile_json, recent_records_json, KEYRING_AI_KEY,
    KEYRING_SERVICE,
};
use crate::db;

pub fn ai_history_export_messages(state: State<'_, AppState>) -> AppResult<Value> {
    let conn = connection(&state)?;
    let mut stmt = conn.prepare("SELECT id, session_id, role, content, scope, scope_reference, date_reference, generation_mode, model_name, meta_snapshot, created_at FROM ai_chat_message ORDER BY id ASC")?;
    let items = stmt
        .query_map([], |row| {
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
                "meta": meta.and_then(|item| serde_json::from_str::<Value>(&item).ok()).unwrap_or_else(|| json!({})),
                "created_at": row.get::<_, String>(10)?
            }))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(json!(items))
}

async fn call_ai(
    config: crate::models::AiConfigView,
    username: String,
    email: String,
    recent_summary: String,
    content: &str,
    scope: &str,
    date: Option<&str>,
    stage_id: Option<i64>,
) -> anyhow::Result<(String, String, String)> {
    if !config.enabled {
        return Err(anyhow!("AI 功能已在设置中关闭"));
    }
    let api_key = Entry::new(KEYRING_SERVICE, KEYRING_AI_KEY)?
        .get_password()
        .map_err(|_| anyhow!("未配置 AI API Key"))?;

    let mut scope_lines = vec![format!("scope={scope}")];
    if let Some(date) = date {
        scope_lines.push(format!("date={date}"));
    }
    if let Some(stage_id) = stage_id {
        scope_lines.push(format!("stage_id={stage_id}"));
    }
    let prompt = format!(
        "你是萤火集桌面端里的学习规划助手。请基于本地学习数据，用中文给出直接、具体、可执行的建议。\n\n用户：{}\n邮箱：{}\n上下文：{}\n最近记录：\n{}\n\n用户问题：{}\n\n输出要求：\n1. 先给核心判断。\n2. 再给 3-5 条可执行建议。\n3. 如数据不足要明确指出。\n4. 不要套话。",
        username,
        email,
        scope_lines.join(" | "),
        if recent_summary.is_empty() { "暂无学习记录".to_string() } else { recent_summary },
        content
    );

    let client = Client::new();
    let response = client
        .post(format!(
            "{}/chat/completions",
            config.base_url.trim_end_matches('/')
        ))
        .bearer_auth(api_key)
        .json(&json!({
            "model": config.model_name,
            "messages": [{ "role": "user", "content": prompt }]
        }))
        .send()
        .await?;
    let status = response.status();
    let body: Value = response.json().await.unwrap_or_else(|_| json!({}));
    if !status.is_success() {
        let message = body
            .pointer("/error/message")
            .and_then(Value::as_str)
            .unwrap_or("调用 AI 失败");
        return Err(anyhow!(message.to_string()));
    }
    let content = body
        .pointer("/choices/0/message/content")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("模型未返回有效内容"))?
        .to_string();
    Ok((content, "llm_enhanced".to_string(), config.model_name))
}

#[tauri::command]
pub fn ai_get_config(state: State<'_, AppState>) -> AppResult<Value> {
    let conn = connection(&state)?;
    Ok(json!({ "success": true, "data": ai_config(&conn)? }))
}

#[tauri::command]
pub fn ai_update_config(state: State<'_, AppState>, payload: AiConfigPayload) -> AppResult<Value> {
    let conn = connection(&state)?;
    if let Some(enabled) = payload.enabled {
        db::set_setting(&conn, "ai_enabled", if enabled { "true" } else { "false" })?;
    }
    if let Some(model_name) = payload.model_name {
        db::set_setting(&conn, "ai_model_name", &serde_json::to_string(&model_name)?)?;
    }
    if let Some(base_url) = payload.base_url {
        db::set_setting(&conn, "ai_base_url", &serde_json::to_string(&base_url)?)?;
    }
    if let Some(api_key) = payload.api_key {
        let entry = Entry::new(KEYRING_SERVICE, KEYRING_AI_KEY)?;
        if api_key.trim().is_empty() {
            let _ = entry.delete_credential();
        } else {
            entry.set_password(api_key.trim())?;
        }
    }
    Ok(json!({ "success": true, "message": "AI 设置已更新", "data": ai_config(&conn)? }))
}

#[tauri::command]
pub fn ai_chat_sessions(state: State<'_, AppState>) -> AppResult<Value> {
    let conn = connection(&state)?;
    let mut stmt = conn.prepare("SELECT id, title, scope, scope_reference, date_reference, created_at, updated_at, last_message_at FROM ai_chat_session ORDER BY datetime(last_message_at) DESC, id DESC")?;
    let items = stmt
        .query_map([], |row| {
            Ok(json!({
                "id": row.get::<_, i64>(0)?,
                "user_id": 1,
                "title": row.get::<_, String>(1)?,
                "scope": row.get::<_, String>(2)?,
                "scope_reference": row.get::<_, Option<i64>>(3)?,
                "date_reference": row.get::<_, Option<String>>(4)?,
                "created_at": row.get::<_, String>(5)?,
                "updated_at": row.get::<_, String>(6)?,
                "last_message_at": row.get::<_, String>(7)?
            }))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(json!({ "success": true, "data": items }))
}

#[tauri::command]
pub fn ai_chat_messages(state: State<'_, AppState>, session_id: i64) -> AppResult<Value> {
    let conn = connection(&state)?;
    let mut stmt = conn.prepare("SELECT id, session_id, role, content, scope, scope_reference, date_reference, generation_mode, model_name, meta_snapshot, created_at FROM ai_chat_message WHERE session_id = ?1 ORDER BY datetime(created_at) ASC, id ASC")?;
    let items = stmt
        .query_map(params![session_id], |row| {
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
                "meta": meta.and_then(|item| serde_json::from_str::<Value>(&item).ok()).unwrap_or_else(|| json!({})),
                "created_at": row.get::<_, String>(10)?
            }))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(json!({ "success": true, "data": { "messages": items } }))
}

#[tauri::command]
pub async fn ai_chat_send(
    state: State<'_, AppState>,
    payload: AiChatSendPayload,
) -> AppResult<Value> {
    let conn = connection(&state)?;
    let scope = if payload.scope.trim().is_empty() {
        "global".to_string()
    } else {
        payload.scope.clone()
    };
    let config = ai_config(&conn)?;
    let profile = profile_json(&conn)?;
    let recent = recent_records_json(&conn, 12)?;
    let recent_summary = recent
        .iter()
        .take(8)
        .map(|item| {
            format!(
                "- {} | {} | {} 分钟",
                item["log_date"].as_str().unwrap_or(""),
                item["task"].as_str().unwrap_or(""),
                item["actual_duration"].as_i64().unwrap_or(0)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let now = db::now_local_iso();
    let session_id = if let Some(session_id) = payload.session_id {
        session_id
    } else {
        conn.execute(
            "INSERT INTO ai_chat_session (title, scope, scope_reference, date_reference, created_at, updated_at, last_message_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                payload.content.chars().take(24).collect::<String>(),
                scope,
                payload.stage_id,
                payload.date,
                now,
                now,
                now
            ],
        )?;
        conn.last_insert_rowid()
    };
    conn.execute(
        "INSERT INTO ai_chat_message (session_id, role, content, scope, scope_reference, date_reference, created_at)
         VALUES (?1, 'user', ?2, ?3, ?4, ?5, ?6)",
        params![session_id, payload.content, scope, payload.stage_id, payload.date, db::now_local_iso()],
    )?;
    let user_message_id = conn.last_insert_rowid();

    drop(conn);
    let ai_result = call_ai(
        config,
        profile["username"].as_str().unwrap_or("学习者").to_string(),
        profile["email"].as_str().unwrap_or("").to_string(),
        recent_summary,
        &payload.content,
        &scope,
        payload.date.as_deref(),
        payload.stage_id,
    )
    .await;
    let conn = connection(&state)?;
    let (assistant_content, generation_mode, model_name) = match ai_result {
        Ok(result) => result,
        Err(error) => {
            let text = error.to_string();
            let fallback = if text.contains("未配置") {
                "AI 尚未配置。请到设置中心填写 API Key。"
            } else if text.contains("quota") || text.contains("429") {
                "AI 调用失败：当前配额不足或被限流，请稍后重试。"
            } else {
                "AI 调用失败：网络异常或服务不可用，请稍后重试。"
            };
            (
                fallback.to_string(),
                "rule_fallback".to_string(),
                String::new(),
            )
        }
    };

    let meta = json!({
        "generation_label": if generation_mode == "llm_enhanced" { "LLM增强" } else { "规则兜底" },
        "used_modules": ["records", "charts", "local_profile"],
        "scope": scope,
        "period_label": payload.date
    });
    conn.execute(
        "INSERT INTO ai_chat_message (session_id, role, content, scope, scope_reference, date_reference, generation_mode, model_name, meta_snapshot, created_at)
         VALUES (?1, 'assistant', ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            session_id,
            assistant_content,
            payload.scope,
            payload.stage_id,
            payload.date,
            generation_mode,
            if model_name.is_empty() { None::<String> } else { Some(model_name.clone()) },
            meta.to_string(),
            db::now_local_iso()
        ],
    )?;
    let assistant_message_id = conn.last_insert_rowid();
    conn.execute(
        "UPDATE ai_chat_session SET updated_at = ?1, last_message_at = ?1 WHERE id = ?2",
        params![db::now_local_iso(), session_id],
    )?;

    let session = ai_chat_sessions(state.clone())?["data"]
        .as_array()
        .and_then(|items| {
            items
                .iter()
                .find(|item| item["id"].as_i64() == Some(session_id))
                .cloned()
        })
        .ok_or_else(|| invalid("会话不存在"))?;
    let messages = ai_chat_messages(state.clone(), session_id)?["data"]["messages"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let user_message = messages
        .iter()
        .find(|item| item["id"].as_i64() == Some(user_message_id))
        .cloned()
        .ok_or_else(|| invalid("用户消息保存失败"))?;
    let assistant_message = messages
        .iter()
        .find(|item| item["id"].as_i64() == Some(assistant_message_id))
        .cloned()
        .ok_or_else(|| invalid("AI 消息保存失败"))?;

    Ok(json!({
        "success": true,
        "data": {
            "session": session,
            "user_message": user_message,
            "assistant_message": assistant_message,
            "meta": meta
        }
    }))
}

#[tauri::command]
pub fn ai_history_list(state: State<'_, AppState>, query: AiHistoryQuery) -> AppResult<Value> {
    let conn = connection(&state)?;
    let limit = query.limit.unwrap_or(20).clamp(1, 100);
    let offset = query.offset.unwrap_or(0).max(0);
    let mut sql = "SELECT id, insight_type, scope, scope_reference, start_date, end_date, next_start_date, next_end_date, input_snapshot, output_text, created_at FROM ai_insight WHERE 1=1".to_string();
    if let Some(scope) = query.scope {
        sql.push_str(&format!(" AND scope = '{}'", scope));
    }
    if let Some(kind) = query.r#type {
        sql.push_str(&format!(" AND insight_type = '{}'", kind));
    }
    sql.push_str(&format!(
        " ORDER BY datetime(created_at) DESC LIMIT {} OFFSET {}",
        limit, offset
    ));
    let mut stmt = conn.prepare(&sql)?;
    let items = stmt
        .query_map([], |row| {
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
                "created_at": row.get::<_, String>(10)?
            }))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(json!({ "success": true, "data": items }))
}
