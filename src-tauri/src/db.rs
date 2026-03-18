use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use chrono::{DateTime, Datelike, Duration, Local, NaiveDate, Utc};
use rusqlite::{params, Connection, OptionalExtension};

use crate::AppState;

pub fn open_connection(state: &AppState) -> Result<Connection> {
    let conn = Connection::open(&state.db_path)?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    Ok(conn)
}

pub fn initialize_database(state: &AppState) -> Result<()> {
    let conn = open_connection(state)?;
    conn.execute_batch(
        r#"
        PRAGMA journal_mode = WAL;
        CREATE TABLE IF NOT EXISTS local_profile (
          id INTEGER PRIMARY KEY CHECK (id = 1),
          username TEXT NOT NULL,
          email TEXT,
          created_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS app_setting (
          key TEXT PRIMARY KEY,
          value TEXT NOT NULL,
          updated_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS stage (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          name TEXT NOT NULL,
          start_date TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS category (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          name TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS sub_category (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          name TEXT NOT NULL,
          category_id INTEGER NOT NULL REFERENCES category(id) ON DELETE CASCADE
        );
        CREATE TABLE IF NOT EXISTS log_entry (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          log_date TEXT NOT NULL,
          time_slot TEXT,
          task TEXT NOT NULL,
          actual_duration INTEGER,
          legacy_category TEXT,
          mood INTEGER,
          notes TEXT,
          stage_id INTEGER NOT NULL REFERENCES stage(id),
          subcategory_id INTEGER REFERENCES sub_category(id),
          created_at TEXT NOT NULL,
          updated_at TEXT
        );
        CREATE TABLE IF NOT EXISTS daily_data (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          log_date TEXT NOT NULL,
          efficiency REAL,
          stage_id INTEGER NOT NULL REFERENCES stage(id) ON DELETE CASCADE,
          UNIQUE(log_date, stage_id)
        );
        CREATE TABLE IF NOT EXISTS weekly_data (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          year INTEGER NOT NULL,
          week_num INTEGER NOT NULL,
          efficiency REAL,
          stage_id INTEGER NOT NULL REFERENCES stage(id) ON DELETE CASCADE,
          UNIQUE(year, week_num, stage_id)
        );
        CREATE TABLE IF NOT EXISTS motto (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          content TEXT NOT NULL,
          created_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS milestone_category (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          name TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS milestone (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          title TEXT NOT NULL,
          event_date TEXT NOT NULL,
          description TEXT,
          category_id INTEGER REFERENCES milestone_category(id),
          created_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS milestone_attachment (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          milestone_id INTEGER NOT NULL REFERENCES milestone(id) ON DELETE CASCADE,
          file_path TEXT NOT NULL,
          original_filename TEXT NOT NULL,
          uploaded_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS countdown_event (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          title TEXT NOT NULL,
          target_datetime_utc TEXT NOT NULL,
          created_at_utc TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS ai_insight (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          insight_type TEXT NOT NULL,
          scope TEXT NOT NULL,
          scope_reference INTEGER,
          start_date TEXT,
          end_date TEXT,
          next_start_date TEXT,
          next_end_date TEXT,
          input_snapshot TEXT,
          output_text TEXT NOT NULL,
          created_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS ai_chat_session (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          title TEXT NOT NULL,
          scope TEXT NOT NULL DEFAULT 'global',
          scope_reference INTEGER,
          date_reference TEXT,
          created_at TEXT NOT NULL,
          updated_at TEXT NOT NULL,
          last_message_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS ai_chat_message (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          session_id INTEGER NOT NULL REFERENCES ai_chat_session(id) ON DELETE CASCADE,
          role TEXT NOT NULL,
          content TEXT NOT NULL,
          scope TEXT NOT NULL DEFAULT 'global',
          scope_reference INTEGER,
          date_reference TEXT,
          generation_mode TEXT,
          model_name TEXT,
          meta_snapshot TEXT,
          created_at TEXT NOT NULL
        );
        "#,
    )?;

    let now = now_local_iso();
    conn.execute(
        "INSERT OR IGNORE INTO local_profile (id, username, email, created_at) VALUES (1, ?, ?, ?)",
        params!["学习者", "", now],
    )?;

    if conn.query_row("SELECT COUNT(*) FROM motto", [], |row| row.get::<_, i64>(0))? == 0 {
        for content in default_mottos() {
            conn.execute(
                "INSERT INTO motto (content, created_at) VALUES (?, ?)",
                params![content, now_local_iso()],
            )?;
        }
    }

    conn.execute(
        "INSERT OR IGNORE INTO app_setting (key, value, updated_at) VALUES ('active_stage_id', '0', ?)",
        params![now_local_iso()],
    )?;
    conn.execute(
        "INSERT OR IGNORE INTO app_setting (key, value, updated_at) VALUES ('ai_enabled', 'true', ?)",
        params![now_local_iso()],
    )?;
    conn.execute(
        "INSERT OR IGNORE INTO app_setting (key, value, updated_at) VALUES ('ai_model_name', 'qwen-plus-2025-07-28', ?)",
        params![now_local_iso()],
    )?;
    conn.execute(
        "INSERT OR IGNORE INTO app_setting (key, value, updated_at) VALUES ('ai_base_url', 'https://dashscope.aliyuncs.com/compatible-mode/v1', ?)",
        params![now_local_iso()],
    )?;
    Ok(())
}

pub fn default_mottos() -> [&'static str; 7] {
    [
        "书山有路勤为径,学海无涯苦作舟。",
        "业精于勤,荒于嬉;行成于思,毁于随。",
        "不积跬步,无以至千里;不积小流,无以成江海。",
        "少壮不努力,老大徒伤悲。",
        "吾生也有涯,而知也无涯。",
        "天行健,君子以自强不息。",
        "明日复明日,明日何其多。我生待明日,万事成蹉跎。"
    ]
}

pub fn now_local_iso() -> String {
    Local::now().to_rfc3339()
}

pub fn now_utc_iso() -> String {
    Utc::now().to_rfc3339()
}

pub fn parse_date(value: &str) -> Result<NaiveDate> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|e| anyhow!(e.to_string()))
}

pub fn normalize_duration_minutes(value: f64) -> i64 {
    if value <= 0.0 {
        return 0;
    }
    if value < 10.0 && value.fract() != 0.0 {
        (value * 60.0).round() as i64
    } else {
        value.round() as i64
    }
}

pub fn format_minutes(minutes: i64) -> String {
    let hours = minutes / 60;
    let mins = minutes % 60;
    if hours > 0 && mins > 0 {
        format!("{hours}h {mins}m")
    } else if hours > 0 {
        format!("{hours}h")
    } else {
        format!("{mins}m")
    }
}

pub fn get_setting(conn: &Connection, key: &str) -> Result<Option<String>> {
    conn.query_row(
        "SELECT value FROM app_setting WHERE key = ?1",
        params![key],
        |row| row.get(0),
    )
    .optional()
    .map_err(Into::into)
}

pub fn set_setting(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO app_setting (key, value, updated_at) VALUES (?1, ?2, ?3)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
        params![key, value, now_local_iso()],
    )?;
    Ok(())
}

pub fn stage_for_date(conn: &Connection, log_date: NaiveDate) -> Result<Option<(i64, String, NaiveDate)>> {
    conn.query_row(
        "SELECT id, name, start_date FROM stage WHERE start_date <= ?1 ORDER BY start_date DESC LIMIT 1",
        params![log_date.format("%Y-%m-%d").to_string()],
        |row| {
            let start_date: String = row.get(2)?;
            Ok((
                row.get(0)?,
                row.get(1)?,
                parse_date(&start_date).map_err(|_| rusqlite::Error::InvalidQuery)?,
            ))
        },
    )
    .optional()
    .map_err(Into::into)
}

pub fn stage_start_date(conn: &Connection, stage_id: i64) -> Result<NaiveDate> {
    let start_date: String = conn.query_row(
        "SELECT start_date FROM stage WHERE id = ?1",
        params![stage_id],
        |row| row.get(0),
    )?;
    parse_date(&start_date)
}

pub fn next_stage_start(conn: &Connection, stage_id: i64) -> Result<Option<NaiveDate>> {
    let current = stage_start_date(conn, stage_id)?;
    let value: Option<String> = conn
        .query_row(
            "SELECT start_date FROM stage WHERE start_date > ?1 ORDER BY start_date ASC LIMIT 1",
            params![current.format("%Y-%m-%d").to_string()],
            |row| row.get(0),
        )
        .optional()?;
    value.map(|item| parse_date(&item)).transpose()
}

pub fn get_custom_week_window(log_date: NaiveDate, start_date: NaiveDate) -> (NaiveDate, NaiveDate, i32, i32) {
    if log_date < start_date {
        let first_week_end = start_date + Duration::days(((6 - start_date.weekday().num_days_from_monday()) % 7) as i64);
        return (start_date, first_week_end, start_date.year(), 1);
    }

    if start_date.weekday().num_days_from_monday() == 0 {
        let days_diff = (log_date - start_date).num_days();
        let week_num = (days_diff / 7) as i32 + 1;
        let week_start = start_date + Duration::weeks((week_num - 1) as i64);
        let week_end = week_start + Duration::days(6);
        return (week_start, week_end, week_start.year(), week_num);
    }

    let first_week_end = start_date + Duration::days((6 - start_date.weekday().num_days_from_monday()) as i64);
    if log_date <= first_week_end {
        return (start_date, first_week_end, start_date.year(), 1);
    }

    let first_full_week_start = first_week_end + Duration::days(1);
    let weeks_after_first = (log_date - first_full_week_start).num_days() / 7;
    let week_num = weeks_after_first as i32 + 2;
    let week_start = first_full_week_start + Duration::weeks(weeks_after_first);
    let week_end = week_start + Duration::days(6);
    (week_start, week_end, week_start.year(), week_num)
}

pub fn ensure_log_stage_consistency(conn: &Connection) -> Result<()> {
    let mut stages_stmt =
        conn.prepare("SELECT id, start_date FROM stage ORDER BY start_date DESC")?;
    let stages = stages_stmt
        .query_map([], |row| {
            let start_date: String = row.get(1)?;
            Ok((row.get::<_, i64>(0)?, start_date))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    if stages.is_empty() {
        return Ok(());
    }

    let mut log_stmt = conn.prepare("SELECT id, log_date FROM log_entry")?;
    let logs = log_stmt
        .query_map([], |row| {
            let log_date: String = row.get(1)?;
            Ok((row.get::<_, i64>(0)?, log_date))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let mut touched = Vec::new();
    for (log_id, log_date_str) in logs {
        let log_date = parse_date(&log_date_str)?;
        let target_stage = stages.iter().find(|(_, start)| parse_date(start).map(|d| d <= log_date).unwrap_or(false));
        if let Some((stage_id, _)) = target_stage {
            conn.execute(
                "UPDATE log_entry SET stage_id = ?1 WHERE id = ?2 AND stage_id != ?1",
                params![stage_id, log_id],
            )?;
            touched.push(*stage_id);
        }
    }

    touched.sort_unstable();
    touched.dedup();
    for stage_id in touched {
        recalculate_efficiency_for_stage(conn, stage_id)?;
    }
    Ok(())
}

pub fn recalculate_efficiency_for_stage(conn: &Connection, stage_id: i64) -> Result<()> {
    let stage_start = stage_start_date(conn, stage_id)?;
    let next_stage = next_stage_start(conn, stage_id)?;
    let stage_end = next_stage
        .map(|d| d - Duration::days(1))
        .unwrap_or_else(|| Local::now().date_naive());

    conn.execute("DELETE FROM daily_data WHERE stage_id = ?1", params![stage_id])?;
    conn.execute("DELETE FROM weekly_data WHERE stage_id = ?1", params![stage_id])?;

    let mut stmt = conn.prepare(
        "SELECT DISTINCT log_date FROM log_entry WHERE stage_id = ?1 ORDER BY log_date ASC",
    )?;
    let log_dates = stmt
        .query_map(params![stage_id], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    for log_date_str in &log_dates {
        let log_date = parse_date(log_date_str)?;
        let score = calculate_daily_efficiency(conn, stage_id, log_date)?;
        conn.execute(
            "INSERT INTO daily_data (log_date, efficiency, stage_id) VALUES (?1, ?2, ?3)
             ON CONFLICT(log_date, stage_id) DO UPDATE SET efficiency = excluded.efficiency",
            params![log_date_str, score, stage_id],
        )?;
    }

    let mut seen_weeks = Vec::<(i32, i32, NaiveDate, NaiveDate)>::new();
    for log_date_str in log_dates {
        let log_date = parse_date(&log_date_str)?;
        let (week_start, week_end, year, week_num) = get_custom_week_window(log_date, stage_start);
        if !seen_weeks
            .iter()
            .any(|item| item.0 == year && item.1 == week_num)
        {
            seen_weeks.push((year, week_num, week_start, week_end));
        }
    }

    for (year, week_num, week_start, week_end) in seen_weeks {
        let effective_start = if week_start < stage_start { stage_start } else { week_start };
        let today = Local::now().date_naive();
        let mut effective_end = if week_end > stage_end { stage_end } else { week_end };
        if effective_end > today {
            effective_end = today;
        }
        let days_in_week = (effective_end - effective_start).num_days() + 1;
        let mut total_score = 0.0_f64;
        for day_offset in 0..days_in_week {
            let day = effective_start + Duration::days(day_offset);
            let day_key = day.format("%Y-%m-%d").to_string();
            let value: Option<f64> = conn
                .query_row(
                    "SELECT efficiency FROM daily_data WHERE stage_id = ?1 AND log_date = ?2",
                    params![stage_id, day_key],
                    |row| row.get(0),
                )
                .optional()?;
            total_score += value.unwrap_or(0.0);
        }
        let avg = if days_in_week > 0 {
            total_score / days_in_week as f64
        } else {
            0.0
        };
        conn.execute(
            "INSERT INTO weekly_data (year, week_num, efficiency, stage_id) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(year, week_num, stage_id) DO UPDATE SET efficiency = excluded.efficiency",
            params![year, week_num, avg, stage_id],
        )?;
    }

    Ok(())
}

pub fn update_efficiency_for_date(conn: &Connection, stage_id: i64, log_date: NaiveDate) -> Result<()> {
    let score = calculate_daily_efficiency(conn, stage_id, log_date)?;
    conn.execute(
        "INSERT INTO daily_data (log_date, efficiency, stage_id) VALUES (?1, ?2, ?3)
         ON CONFLICT(log_date, stage_id) DO UPDATE SET efficiency = excluded.efficiency",
        params![log_date.format("%Y-%m-%d").to_string(), score, stage_id],
    )?;

    let stage_start = stage_start_date(conn, stage_id)?;
    let next_stage = next_stage_start(conn, stage_id)?;
    let stage_end = next_stage
        .map(|d| d - Duration::days(1))
        .unwrap_or_else(|| Local::now().date_naive());
    let (week_start, week_end, year, week_num) = get_custom_week_window(log_date, stage_start);
    let effective_start = if week_start < stage_start { stage_start } else { week_start };
    let effective_end = {
        let mut end = if week_end > stage_end { stage_end } else { week_end };
        let today = Local::now().date_naive();
        if end > today {
            end = today;
        }
        end
    };
    let days_in_week = (effective_end - effective_start).num_days() + 1;
    let mut total_score = 0.0_f64;
    for offset in 0..days_in_week {
        let day = effective_start + Duration::days(offset);
        let key = day.format("%Y-%m-%d").to_string();
        let value: Option<f64> = conn
            .query_row(
                "SELECT efficiency FROM daily_data WHERE stage_id = ?1 AND log_date = ?2",
                params![stage_id, key],
                |row| row.get(0),
            )
            .optional()?;
        total_score += value.unwrap_or(0.0);
    }
    let avg = if days_in_week > 0 {
        total_score / days_in_week as f64
    } else {
        0.0
    };
    conn.execute(
        "INSERT INTO weekly_data (year, week_num, efficiency, stage_id) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(year, week_num, stage_id) DO UPDATE SET efficiency = excluded.efficiency",
        params![year, week_num, avg, stage_id],
    )?;
    Ok(())
}

pub fn calculate_daily_efficiency(conn: &Connection, stage_id: i64, log_date: NaiveDate) -> Result<f64> {
    let mut stmt = conn.prepare(
        "SELECT actual_duration, COALESCE(mood, 3) FROM log_entry WHERE stage_id = ?1 AND log_date = ?2",
    )?;
    let rows = stmt
        .query_map(
            params![stage_id, log_date.format("%Y-%m-%d").to_string()],
            |row| Ok((row.get::<_, Option<i64>>(0)?.unwrap_or(0), row.get::<_, i64>(1)?)),
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    if rows.is_empty() {
        return Ok(0.0);
    }

    let total_duration: i64 = rows.iter().map(|item| item.0).sum();
    if total_duration <= 0 {
        return Ok(0.0);
    }

    let weighted_sum: i64 = rows.iter().map(|item| item.0 * item.1).sum();
    let avg_mood = weighted_sum as f64 / total_duration as f64;
    let total_hours = total_duration as f64 / 60.0;
    Ok(avg_mood * (1.0 + total_hours).ln())
}

pub fn moving_average(data: &[f64], window_size: usize) -> Vec<Option<f64>> {
    if data.is_empty() {
        return Vec::new();
    }
    let mut result = Vec::with_capacity(data.len());
    for idx in 0..data.len() {
        let start = idx.saturating_sub(window_size.saturating_sub(1));
        let window = &data[start..=idx];
        let sum: f64 = window.iter().copied().sum();
        result.push(Some(sum / window.len() as f64));
    }
    result
}

pub fn parse_rfc3339(value: &str) -> Result<DateTime<Utc>> {
    Ok(DateTime::parse_from_rfc3339(value)?.with_timezone(&Utc))
}

pub fn remove_dir_contents(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            fs::remove_dir_all(entry.path())?;
        } else {
            fs::remove_file(entry.path())?;
        }
    }
    Ok(())
}

pub fn attachment_path(base_dir: &Path, relative: &str) -> PathBuf {
    base_dir.join(relative.replace('/', std::path::MAIN_SEPARATOR_STR))
}
