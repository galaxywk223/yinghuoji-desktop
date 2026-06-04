use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Cursor;

use anyhow::Result;
use calamine::{open_workbook_auto_from_rs, Data, Reader};
use chrono::Local;
use csv::StringRecord;
use rusqlite::{params, OptionalExtension};
use serde::Deserialize;
use serde_json::{json, Value};
use tauri::State;
use uuid::Uuid;

use crate::{db, AppResult, AppState};

use super::common::{connection, invalid};

#[derive(Debug, Deserialize)]
pub struct CourseProfileSettingsPayload {
    pub source_category_id: Option<i64>,
}

#[derive(Debug, Deserialize, Default)]
pub struct CourseProfileListQuery {
    pub semester: Option<String>,
    pub match_status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CourseProfileImportConfirmPayload {
    pub import_id: Option<String>,
    pub courses: Vec<CourseProfileImportConfirmItem>,
}

#[derive(Debug, Deserialize)]
pub struct CourseProfileImportConfirmItem {
    pub row_index: i64,
    pub semester: String,
    pub course_name: String,
    pub credits: f64,
    pub grade: Option<f64>,
    pub grade_status: Option<String>,
    pub matched_subcategory_id: Option<i64>,
    pub match_status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CourseProfileUpdatePayload {
    pub semester: String,
    pub course_name: String,
    pub credits: f64,
    pub grade: Option<f64>,
    pub matched_subcategory_id: Option<i64>,
    pub match_status: Option<String>,
}

#[derive(Clone)]
struct ImportedCourseRow {
    row_index: usize,
    semester: String,
    course_name: String,
    credits: f64,
    grade: Option<f64>,
    grade_status: String,
}

#[derive(Clone)]
struct CandidateSubcategory {
    id: i64,
    name: String,
    normalized_name: String,
}

fn csv_template_value(value: &Value, key: &str) -> String {
    match value.get(key) {
        Some(Value::String(text)) => text.trim().to_string(),
        Some(Value::Number(number)) => number.to_string(),
        Some(value) if !value.is_null() => value.to_string(),
        _ => String::new(),
    }
}

fn csv_template_number(value: &Value, key: &str, positive_only: bool) -> String {
    let Some(number) = value.get(key).and_then(Value::as_f64) else {
        return String::new();
    };
    if positive_only && number <= 0.0 {
        return String::new();
    }
    if number.fract() == 0.0 {
        format!("{number:.0}")
    } else {
        let text = format!("{number:.2}");
        text.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

fn normalize_name(value: &str) -> String {
    let mut normalized = String::new();
    let mut skip_depth = 0_i32;
    for ch in value.chars() {
        match ch {
            '(' | '（' | '[' | '【' => {
                skip_depth += 1;
            }
            ')' | '）' | ']' | '】' => {
                if skip_depth > 0 {
                    skip_depth -= 1;
                }
            }
            _ if skip_depth > 0 => {}
            _ if ch.is_whitespace() || ch == '-' || ch == '_' || ch == '·' => {}
            _ => {
                for lower in ch.to_lowercase() {
                    normalized.push(lower);
                }
            }
        }
    }
    normalized
}

fn normalize_header(value: &str) -> String {
    value
        .trim()
        .trim_start_matches('\u{feff}')
        .to_ascii_lowercase()
        .replace([' ', '_', '-'], "")
}

fn parse_number(raw: &str, field: &str, row_index: usize) -> AppResult<f64> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(invalid(&format!("第 {row_index} 行缺少{field}")));
    }
    trimmed
        .parse::<f64>()
        .map_err(|_| invalid(&format!("第 {row_index} 行{field}不是有效数字")))
}

fn parse_optional_grade(raw: &str, row_index: usize) -> AppResult<(Option<f64>, String)> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || matches!(trimmed, "未出" | "未出成绩" | "暂无" | "待出") {
        return Ok((None, "pending".to_string()));
    }
    let grade = trimmed
        .parse::<f64>()
        .map_err(|_| invalid(&format!("第 {row_index} 行成绩不是有效数字，可留空表示未出成绩")))?;
    Ok((Some(grade), "graded".to_string()))
}

fn header_index(headers: &[String], candidates: &[&str]) -> Option<usize> {
    headers.iter().position(|item| {
        candidates
            .iter()
            .any(|candidate| item == &normalize_header(candidate))
    })
}

fn required_cell<'a>(
    record: &'a StringRecord,
    index: Option<usize>,
    label: &str,
    row_index: usize,
) -> AppResult<&'a str> {
    let idx = index.ok_or_else(|| invalid(&format!("导入文件缺少{label}列")))?;
    record
        .get(idx)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| invalid(&format!("第 {row_index} 行缺少{label}")))
}

fn parse_csv(file_bytes: Vec<u8>) -> AppResult<Vec<ImportedCourseRow>> {
    let mut reader = csv::ReaderBuilder::new()
        .flexible(true)
        .trim(csv::Trim::All)
        .from_reader(Cursor::new(file_bytes));
    let headers = reader
        .headers()
        .map_err(|e| invalid(&format!("CSV 表头解析失败：{e}")))?
        .iter()
        .map(normalize_header)
        .collect::<Vec<_>>();
    let semester_index = header_index(&headers, &["semester", "学期"]);
    let course_index = header_index(&headers, &["course_name", "course", "课程名称", "课程"]);
    let credits_index = header_index(&headers, &["credits", "credit", "学分"]);
    let grade_index = header_index(&headers, &["grade", "score", "成绩", "分数"]);

    if semester_index.is_none() || course_index.is_none() || credits_index.is_none() {
        return Err(invalid("导入文件必须包含学期、课程名称、学分列"));
    }

    let mut rows = Vec::new();
    for (idx, result) in reader.records().enumerate() {
        let row_index = idx + 2;
        let record = result.map_err(|e| invalid(&format!("第 {row_index} 行解析失败：{e}")))?;
        if record.iter().all(|cell| cell.trim().is_empty()) {
            continue;
        }
        let semester = required_cell(&record, semester_index, "学期", row_index)?.to_string();
        let course_name = required_cell(&record, course_index, "课程名称", row_index)?.to_string();
        let credits = parse_number(
            required_cell(&record, credits_index, "学分", row_index)?,
            "学分",
            row_index,
        )?;
        let grade_raw = grade_index.and_then(|index| record.get(index)).unwrap_or("");
        let (grade, grade_status) = parse_optional_grade(grade_raw, row_index)?;
        rows.push(ImportedCourseRow {
            row_index,
            semester,
            course_name,
            credits,
            grade,
            grade_status,
        });
    }

    Ok(rows)
}

fn data_to_string(cell: &Data) -> String {
    match cell {
        Data::Empty => String::new(),
        Data::String(value) => value.trim().to_string(),
        Data::Float(value) => {
            if value.fract() == 0.0 {
                format!("{value:.0}")
            } else {
                value.to_string()
            }
        }
        Data::Int(value) => value.to_string(),
        Data::Bool(value) => value.to_string(),
        Data::DateTime(value) => value.to_string(),
        Data::DateTimeIso(value) => value.trim().to_string(),
        Data::DurationIso(value) => value.trim().to_string(),
        Data::Error(_) => String::new(),
    }
}

fn parse_xlsx(file_bytes: Vec<u8>) -> AppResult<Vec<ImportedCourseRow>> {
    let mut workbook = open_workbook_auto_from_rs(Cursor::new(file_bytes))
        .map_err(|e| invalid(&format!("XLSX 文件解析失败：{e}")))?;
    let sheet_name = workbook
        .sheet_names()
        .first()
        .cloned()
        .ok_or_else(|| invalid("XLSX 文件没有可读取的工作表"))?;
    let range = workbook
        .worksheet_range(&sheet_name)
        .map_err(|e| invalid(&format!("工作表读取失败：{e}")))?;

    let mut rows_iter = range.rows();
    let headers_row = rows_iter
        .next()
        .ok_or_else(|| invalid("XLSX 文件缺少表头"))?;
    let headers = headers_row
        .iter()
        .map(data_to_string)
        .map(|value| normalize_header(&value))
        .collect::<Vec<_>>();
    let semester_index = header_index(&headers, &["semester", "学期"]);
    let course_index = header_index(&headers, &["course_name", "course", "课程名称", "课程"]);
    let credits_index = header_index(&headers, &["credits", "credit", "学分"]);
    let grade_index = header_index(&headers, &["grade", "score", "成绩", "分数"]);

    if semester_index.is_none() || course_index.is_none() || credits_index.is_none() {
        return Err(invalid("导入文件必须包含学期、课程名称、学分列"));
    }

    let mut rows = Vec::new();
    for (idx, row) in rows_iter.enumerate() {
        let row_index = idx + 2;
        let cells = row.iter().map(data_to_string).collect::<Vec<_>>();
        if cells.iter().all(|cell| cell.trim().is_empty()) {
            continue;
        }
        let get_required = |index: Option<usize>, label: &str| -> AppResult<String> {
            let idx = index.ok_or_else(|| invalid(&format!("导入文件缺少{label}列")))?;
            cells
                .get(idx)
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .ok_or_else(|| invalid(&format!("第 {row_index} 行缺少{label}")))
        };
        let semester = get_required(semester_index, "学期")?;
        let course_name = get_required(course_index, "课程名称")?;
        let credits = parse_number(&get_required(credits_index, "学分")?, "学分", row_index)?;
        let grade_raw = grade_index
            .and_then(|index| cells.get(index))
            .map(String::as_str)
            .unwrap_or("");
        let (grade, grade_status) = parse_optional_grade(grade_raw, row_index)?;
        rows.push(ImportedCourseRow {
            row_index,
            semester,
            course_name,
            credits,
            grade,
            grade_status,
        });
    }

    Ok(rows)
}

fn parse_import_file(file_name: &str, file_bytes: Vec<u8>) -> AppResult<Vec<ImportedCourseRow>> {
    let lower = file_name.to_ascii_lowercase();
    if lower.ends_with(".csv") {
        parse_csv(file_bytes)
    } else if lower.ends_with(".xlsx") {
        parse_xlsx(file_bytes)
    } else {
        Err(invalid("仅支持 CSV 和 XLSX 文件"))
    }
}

fn source_category_id(conn: &rusqlite::Connection) -> Result<Option<i64>> {
    Ok(db::get_setting(conn, "course_profile_source_category_id")?
        .and_then(|value| value.parse::<i64>().ok()))
}

fn source_category_json(conn: &rusqlite::Connection, category_id: i64) -> Result<Option<Value>> {
    Ok(conn
        .query_row(
            "SELECT id, name FROM category WHERE id = ?1",
            params![category_id],
            |row| {
                Ok(json!({
                    "id": row.get::<_, i64>(0)?,
                    "name": row.get::<_, String>(1)?
                }))
            },
        )
        .optional()?)
}

fn candidate_subcategories(
    conn: &rusqlite::Connection,
    category_id: Option<i64>,
) -> Result<Vec<CandidateSubcategory>> {
    let (sql, params_box): (&str, Vec<Box<dyn rusqlite::ToSql>>) =
        if let Some(category_id) = category_id {
            (
                "SELECT sc.id, sc.name
                 FROM sub_category sc
                 WHERE sc.category_id = ?1
                 ORDER BY sc.name ASC",
                vec![Box::new(category_id)],
            )
        } else {
            (
                "SELECT sc.id, sc.name
                 FROM sub_category sc
                 JOIN category c ON c.id = sc.category_id
                 ORDER BY c.name ASC, sc.name ASC",
                Vec::new(),
            )
        };
    let refs = params_box
        .iter()
        .map(|item| item.as_ref() as &dyn rusqlite::ToSql)
        .collect::<Vec<_>>();
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt
        .query_map(refs.as_slice(), |row| {
            let name: String = row.get(1)?;
            Ok(CandidateSubcategory {
                id: row.get(0)?,
                normalized_name: normalize_name(&name),
                name,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

fn recommend_match(
    course_name: &str,
    candidates: &[CandidateSubcategory],
) -> (Option<i64>, String, f64) {
    let normalized = normalize_name(course_name);
    if normalized.is_empty() {
        return (None, "unmatched".to_string(), 0.0);
    }

    for candidate in candidates {
        if candidate.normalized_name == normalized {
            return (Some(candidate.id), "auto".to_string(), 1.0);
        }
    }

    let mut best: Option<(&CandidateSubcategory, f64)> = None;
    for candidate in candidates {
        if candidate.normalized_name.is_empty() {
            continue;
        }
        let score = if candidate.normalized_name.contains(&normalized)
            || normalized.contains(&candidate.normalized_name)
        {
            let min_len = normalized.len().min(candidate.normalized_name.len()) as f64;
            let max_len = normalized.len().max(candidate.normalized_name.len()) as f64;
            if max_len > 0.0 {
                min_len / max_len
            } else {
                0.0
            }
        } else {
            0.0
        };
        if score > best.map(|(_, value)| value).unwrap_or(0.0) {
            best = Some((candidate, score));
        }
    }

    if let Some((candidate, score)) = best {
        if score >= 0.58 {
            return (Some(candidate.id), "auto".to_string(), score);
        }
    }

    (None, "unmatched".to_string(), 0.0)
}

fn course_json(
    row: &rusqlite::Row<'_>,
    shared_map: &HashMap<i64, i64>,
    efficiency_map: &HashMap<i64, f64>,
) -> rusqlite::Result<Value> {
    let id: i64 = row.get(0)?;
    let semester: String = row.get(1)?;
    let course_name: String = row.get(2)?;
    let credits: f64 = row.get(3)?;
    let grade: Option<f64> = row.get(4)?;
    let grade_status: String = row.get(5)?;
    let match_status: String = row.get(6)?;
    let matched_subcategory_id: Option<i64> = row.get(7)?;
    let matched_subcategory_name: Option<String> = row.get(8)?;
    let matched_category_id: Option<i64> = row.get(9)?;
    let matched_category_name: Option<String> = row.get(10)?;
    let import_batch_id: Option<String> = row.get(11)?;
    let updated_at: String = row.get(12)?;
    let total_minutes: i64 = row.get(13)?;
    let record_count: i64 = row.get(15)?;
    let profile_id: Option<i64> = row.get(16)?;
    let hours = total_minutes as f64 / 60.0;
    let efficiency = matched_subcategory_id
        .and_then(|sub_id| efficiency_map.get(&sub_id).copied())
        .filter(|value| *value > 0.0);
    Ok(json!({
        "id": id,
        "semester": semester,
        "course_name": course_name,
        "credits": credits,
        "grade": grade,
        "grade_status": grade_status,
        "match_status": match_status,
        "matched_subcategory_id": matched_subcategory_id,
        "matched_subcategory_name": matched_subcategory_name,
        "matched_category_id": matched_category_id,
        "matched_category_name": matched_category_name,
        "import_batch_id": import_batch_id,
        "updated_at": updated_at,
        "profile_id": profile_id,
        "is_profile_enriched": profile_id.is_some(),
        "record_count": record_count,
        "learning_minutes": total_minutes,
        "learning_hours": ((hours * 100.0).round() / 100.0),
        "efficiency": efficiency,
        "shared_mapping_count": matched_subcategory_id
            .and_then(|sub_id| shared_map.get(&sub_id).copied())
            .unwrap_or(0),
    }))
}

fn course_efficiency_map(
    conn: &rusqlite::Connection,
    category_id: Option<i64>,
) -> Result<HashMap<i64, f64>> {
    let mut sql = String::from(
        "SELECT le.subcategory_id,
                le.log_date,
                SUM(COALESCE(le.actual_duration, 0)),
                SUM(COALESCE(le.actual_duration, 0) * COALESCE(le.mood, 3))
         FROM log_entry le
         JOIN sub_category sc ON sc.id = le.subcategory_id
         WHERE le.subcategory_id IS NOT NULL",
    );
    let mut params_box: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    if let Some(category_id) = category_id {
        sql.push_str(" AND sc.category_id = ?");
        params_box.push(Box::new(category_id));
    }
    sql.push_str(" GROUP BY le.subcategory_id, le.log_date");
    let refs = params_box
        .iter()
        .map(|item| item.as_ref() as &dyn rusqlite::ToSql)
        .collect::<Vec<_>>();
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map(refs.as_slice(), |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let mut totals = HashMap::<i64, f64>::new();
    for (subcategory_id, duration, weighted_mood) in rows {
        if duration <= 0 {
            continue;
        }
        let hours = duration as f64 / 60.0;
        let avg_mood = weighted_mood as f64 / duration as f64;
        *totals.entry(subcategory_id).or_insert(0.0) += avg_mood * (1.0 + hours).ln();
    }
    Ok(totals
        .into_iter()
        .map(|(subcategory_id, value)| (subcategory_id, round_two(value * 100.0)))
        .collect())
}

fn load_courses(conn: &rusqlite::Connection, query: &CourseProfileListQuery) -> Result<Vec<Value>> {
    let source_category = source_category_id(conn)?;
    let mut sql = String::from(
        "SELECT
                COALESCE(cp.id, -sc.id),
                COALESCE(cp.semester, ''),
                COALESCE(NULLIF(cp.course_name, ''), sc.name),
                COALESCE(cp.credits, 0),
                cp.grade,
                CASE WHEN cp.grade IS NOT NULL THEN 'graded' ELSE COALESCE(cp.grade_status, 'pending') END,
                COALESCE(cp.match_status, 'manual'),
                sc.id,
                sc.name,
                c.id,
                c.name,
                cp.import_batch_id,
                COALESCE(cp.updated_at, ''),
                COALESCE(SUM(COALESCE(le.actual_duration, 0)), 0),
                COALESCE(SUM(COALESCE(le.actual_duration, 0) * COALESCE(le.mood, 3)), 0),
                COUNT(le.id),
                cp.id
         FROM sub_category sc
         JOIN category c ON c.id = sc.category_id
         LEFT JOIN course_profile cp ON cp.id = (
            SELECT cp2.id
            FROM course_profile cp2
            WHERE cp2.matched_subcategory_id = sc.id",
    );
    let mut params_box: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    if let Some(semester) = query
        .semester
        .as_deref()
        .filter(|value| !value.trim().is_empty() && *value != "all")
    {
        sql.push_str(" AND cp2.semester = ?");
        params_box.push(Box::new(semester.trim().to_string()));
    }
    sql.push_str(
        " ORDER BY cp2.semester DESC, cp2.updated_at DESC, cp2.id DESC
          LIMIT 1
         )
         LEFT JOIN log_entry le ON le.subcategory_id = sc.id
         WHERE 1 = 1",
    );
    if let Some(category_id) = source_category {
        sql.push_str(" AND sc.category_id = ?");
        params_box.push(Box::new(category_id));
    }
    if let Some(status) = query
        .match_status
        .as_deref()
        .filter(|value| !value.trim().is_empty() && *value != "all")
    {
        match status.trim() {
            "auto" => {
                sql.push_str(" AND cp.match_status = ?");
                params_box.push(Box::new("auto".to_string()));
            }
            "manual" => {
                sql.push_str(" AND (cp.id IS NULL OR cp.match_status = ?)");
                params_box.push(Box::new("manual".to_string()));
            }
            "unmatched" => {
                sql.push_str(" AND 0 = 1");
            }
            _ => {}
        }
    }
    sql.push_str(
        " GROUP BY sc.id, cp.id
          ORDER BY c.name ASC, sc.name ASC",
    );

    let mut shared_stmt = conn.prepare(
        "SELECT matched_subcategory_id, COUNT(*)
         FROM course_profile
         WHERE matched_subcategory_id IS NOT NULL
         GROUP BY matched_subcategory_id",
    )?;
    let shared_map = shared_stmt
        .query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)))?
        .collect::<rusqlite::Result<HashMap<_, _>>>()?;
    let efficiency_map = course_efficiency_map(conn, source_category)?;

    let refs = params_box
        .iter()
        .map(|item| item.as_ref() as &dyn rusqlite::ToSql)
        .collect::<Vec<_>>();
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map(refs.as_slice(), |row| {
            course_json(row, &shared_map, &efficiency_map)
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

#[tauri::command]
pub fn course_profile_settings_get(state: State<'_, AppState>) -> AppResult<Value> {
    let conn = connection(&state)?;
    let category_id = source_category_id(&conn)?;
    let category = category_id
        .map(|id| source_category_json(&conn, id))
        .transpose()?
        .flatten();
    Ok(json!({
        "success": true,
        "settings": {
            "source_category_id": category_id,
            "source_category": category
        }
    }))
}

#[tauri::command]
pub fn course_profile_settings_set(
    state: State<'_, AppState>,
    payload: CourseProfileSettingsPayload,
) -> AppResult<Value> {
    let conn = connection(&state)?;
    if let Some(category_id) = payload.source_category_id {
        let exists: Option<i64> = conn
            .query_row(
                "SELECT id FROM category WHERE id = ?1",
                params![category_id],
                |row| row.get(0),
            )
            .optional()?;
        if exists.is_none() {
            return Err(invalid("课程来源父分类不存在"));
        }
        db::set_setting(
            &conn,
            "course_profile_source_category_id",
            &category_id.to_string(),
        )?;
    } else {
        db::set_setting(&conn, "course_profile_source_category_id", "")?;
    }
    course_profile_settings_get(state)
}

#[tauri::command]
pub fn course_profile_import_preview(
    state: State<'_, AppState>,
    file_name: String,
    file_bytes: Vec<u8>,
) -> AppResult<Value> {
    let conn = connection(&state)?;
    let source_category_id = source_category_id(&conn)?;
    let candidates = candidate_subcategories(&conn, source_category_id)?;
    let rows = parse_import_file(&file_name, file_bytes)?;
    if rows.is_empty() {
        return Err(invalid("导入文件没有可用课程行"));
    }

    let mut used_matches = HashMap::<i64, i64>::new();
    let items = rows
        .into_iter()
        .map(|row| {
            let (matched_id, status, confidence) = recommend_match(&row.course_name, &candidates);
            if let Some(id) = matched_id {
                *used_matches.entry(id).or_insert(0) += 1;
            }
            let candidate = matched_id.and_then(|id| candidates.iter().find(|item| item.id == id));
            json!({
                "row_index": row.row_index,
                "semester": row.semester,
                "course_name": row.course_name,
                "credits": row.credits,
                "grade": row.grade,
                "grade_status": row.grade_status,
                "match_status": status,
                "matched_subcategory_id": matched_id,
                "matched_subcategory_name": candidate.map(|item| item.name.clone()),
                "confidence": ((confidence * 100.0).round() / 100.0),
            })
        })
        .collect::<Vec<_>>();
    let unmatched_courses = items
        .iter()
        .filter(|item| item["matched_subcategory_id"].is_null())
        .count();

    let matched_ids = used_matches.keys().copied().collect::<HashSet<_>>();
    let unlinked_candidates = candidates
        .iter()
        .filter(|item| !matched_ids.contains(&item.id))
        .map(|item| json!({ "id": item.id, "name": item.name }))
        .collect::<Vec<_>>();
    let duplicate_matches = used_matches
        .iter()
        .filter(|(_, count)| **count > 1)
        .filter_map(|(id, count)| {
            candidates.iter().find(|item| item.id == *id).map(|candidate| {
                json!({
                    "subcategory_id": id,
                    "subcategory_name": candidate.name,
                    "count": count
                })
            })
        })
        .collect::<Vec<_>>();

    Ok(json!({
        "success": true,
        "preview": {
                "import_id": Uuid::new_v4().to_string(),
            "source_category_id": source_category_id,
            "items": items,
            "candidates": candidates.iter().map(|item| json!({
                "id": item.id,
                "name": item.name
            })).collect::<Vec<_>>(),
            "mismatches": {
                "unmatched_courses": unmatched_courses,
                "unlinked_subcategories": unlinked_candidates,
                "duplicate_matches": duplicate_matches
            }
        }
    }))
}

#[tauri::command]
pub fn course_profile_confirm_import(
    state: State<'_, AppState>,
    payload: CourseProfileImportConfirmPayload,
) -> AppResult<Value> {
    if payload.courses.is_empty() {
        return Err(invalid("没有可确认的课程数据"));
    }
    let mut conn = connection(&state)?;
    let tx = conn.transaction()?;
    let import_id = payload
        .import_id
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let now = db::now_local_iso();

    for item in payload.courses {
        if item.semester.trim().is_empty() {
            return Err(invalid(&format!("第 {} 行缺少学期", item.row_index)));
        }
        if item.credits < 0.0 {
            return Err(invalid(&format!("第 {} 行学分不能小于 0", item.row_index)));
        }
        let Some(subcategory_id) = item.matched_subcategory_id else {
            continue;
        };
        let subcategory_name: Option<String> = tx
            .query_row(
                "SELECT name FROM sub_category WHERE id = ?1",
                params![subcategory_id],
                |row| row.get(0),
            )
            .optional()?;
        let Some(subcategory_name) = subcategory_name else {
            return Err(invalid(&format!("第 {} 行匹配的子分类不存在", item.row_index)));
        };
        let grade_status = if item.grade.is_some() {
            "graded"
        } else {
            item.grade_status.as_deref().unwrap_or("pending")
        };
        let match_status = item.match_status.as_deref().unwrap_or("manual");
        let course_name = if item.course_name.trim().is_empty() {
            subcategory_name.as_str()
        } else {
            item.course_name.trim()
        };
        let touched = tx.execute(
            "UPDATE course_profile
             SET course_name = ?1, credits = ?2, grade = ?3, grade_status = ?4,
                 match_status = ?5, import_batch_id = ?6, updated_at = ?7
             WHERE matched_subcategory_id = ?8 AND semester = ?9",
            params![
                course_name,
                item.credits,
                item.grade,
                grade_status,
                match_status,
                import_id,
                now,
                subcategory_id,
                item.semester.trim(),
            ],
        )?;
        if touched > 0 {
            continue;
        }
        tx.execute(
            "INSERT INTO course_profile (
                semester, course_name, credits, grade, grade_status, match_status,
                matched_subcategory_id, import_batch_id, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
             ON CONFLICT(semester, course_name) DO UPDATE SET
                credits = excluded.credits,
                grade = excluded.grade,
                grade_status = excluded.grade_status,
                match_status = excluded.match_status,
                matched_subcategory_id = excluded.matched_subcategory_id,
                import_batch_id = excluded.import_batch_id,
                updated_at = excluded.updated_at",
            params![
                item.semester.trim(),
                course_name,
                item.credits,
                item.grade,
                grade_status,
                match_status,
                subcategory_id,
                import_id,
                now,
                now,
            ],
        )?;
    }

    tx.commit()?;
    Ok(json!({ "success": true, "message": "课程画像导入完成" }))
}

#[tauri::command]
pub fn course_profile_list(
    state: State<'_, AppState>,
    query: CourseProfileListQuery,
) -> AppResult<Value> {
    let conn = connection(&state)?;
    let courses = load_courses(&conn, &query)?;
    let semesters = {
        let mut stmt =
            conn.prepare("SELECT DISTINCT semester FROM course_profile ORDER BY semester DESC")?;
        let values = stmt
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        values
    };
    Ok(json!({
        "success": true,
        "courses": courses,
        "semesters": semesters
    }))
}

#[tauri::command]
pub fn course_profile_download_template(state: State<'_, AppState>) -> AppResult<Value> {
    let conn = connection(&state)?;
    let courses = load_courses(&conn, &CourseProfileListQuery::default())?;
    if courses.is_empty() {
        return Err(invalid("当前课程来源下没有可生成模板的已有科目"));
    }

    fs::create_dir_all(&state.exports_dir)?;
    let file_name = format!(
        "课程画像导入模板-{}.csv",
        Local::now().format("%Y%m%d-%H%M%S")
    );
    let path = state.exports_dir.join(file_name);
    let mut writer = csv::WriterBuilder::new()
        .has_headers(true)
        .from_writer(Vec::<u8>::new());
    writer
        .write_record(["学期", "课程名称", "学分", "成绩"])
        .map_err(|error| invalid(&error.to_string()))?;
    for course in &courses {
        let semester = csv_template_value(course, "semester");
        let course_name = csv_template_value(course, "course_name");
        let credits = csv_template_number(course, "credits", true);
        let grade = csv_template_number(course, "grade", false);
        writer
            .write_record([semester, course_name, credits, grade])
            .map_err(|error| invalid(&error.to_string()))?;
    }
    let bytes = writer.into_inner().map_err(|error| invalid(&error.to_string()))?;
    let mut content = Vec::with_capacity(bytes.len() + 3);
    content.extend_from_slice(&[0xEF, 0xBB, 0xBF]);
    content.extend(bytes);
    fs::write(&path, content)?;

    let reveal_result = tauri_plugin_opener::reveal_item_in_dir(&path)
        .map(|_| true)
        .unwrap_or(false);

    Ok(json!({
        "success": true,
        "file_path": path.to_string_lossy(),
        "file_name": path.file_name().and_then(|item| item.to_str()).unwrap_or("课程画像导入模板.csv"),
        "row_count": courses.len(),
        "revealed": reveal_result
    }))
}

#[tauri::command]
pub fn course_profile_update_course(
    state: State<'_, AppState>,
    course_id: i64,
    payload: CourseProfileUpdatePayload,
) -> AppResult<Value> {
    if payload.semester.trim().is_empty() || payload.course_name.trim().is_empty() {
        return Err(invalid("学期和课程名称不能为空"));
    }
    if payload.credits < 0.0 {
        return Err(invalid("学分不能小于 0"));
    }
    let conn = connection(&state)?;
    let matched_subcategory_id = if course_id < 0 {
        Some(-course_id)
    } else {
        payload.matched_subcategory_id
    };
    let Some(subcategory_id) = matched_subcategory_id else {
        return Err(invalid("课程画像必须关联已有科目"));
    };
    let exists: Option<i64> = conn
        .query_row(
            "SELECT id FROM sub_category WHERE id = ?1",
            params![subcategory_id],
            |row| row.get(0),
        )
        .optional()?;
    if exists.is_none() {
        return Err(invalid("关联的已有科目不存在"));
    }
    let grade_status = if payload.grade.is_some() {
        "graded"
    } else {
        "pending"
    };
    let match_status = payload.match_status.as_deref().unwrap_or("manual");
    let now = db::now_local_iso();
    if course_id > 0 {
        conn.execute(
            "UPDATE course_profile
             SET semester = ?1, course_name = ?2, credits = ?3, grade = ?4,
                 grade_status = ?5, match_status = ?6, matched_subcategory_id = ?7,
                 updated_at = ?8
             WHERE id = ?9",
            params![
                payload.semester.trim(),
                payload.course_name.trim(),
                payload.credits,
                payload.grade,
                grade_status,
                match_status,
                subcategory_id,
                now,
                course_id,
            ],
        )?;
    } else {
        conn.execute(
            "INSERT INTO course_profile (
                semester, course_name, credits, grade, grade_status, match_status,
                matched_subcategory_id, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(semester, course_name) DO UPDATE SET
                credits = excluded.credits,
                grade = excluded.grade,
                grade_status = excluded.grade_status,
                match_status = excluded.match_status,
                matched_subcategory_id = excluded.matched_subcategory_id,
                updated_at = excluded.updated_at",
            params![
                payload.semester.trim(),
                payload.course_name.trim(),
                payload.credits,
                payload.grade,
                grade_status,
                match_status,
                subcategory_id,
                now,
                now,
            ],
        )?;
    }
    Ok(json!({ "success": true, "message": "课程已更新" }))
}

#[tauri::command]
pub fn course_profile_delete_course(
    state: State<'_, AppState>,
    course_id: i64,
) -> AppResult<Value> {
    let conn = connection(&state)?;
    if course_id > 0 {
        conn.execute("DELETE FROM course_profile WHERE id = ?1", params![course_id])?;
    }
    Ok(json!({ "success": true, "message": "课程已删除" }))
}

fn round_two(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

fn round_one(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

fn percent_share(value: f64, total: f64) -> f64 {
    if total > 0.0 {
        round_one(value / total * 100.0)
    } else {
        0.0
    }
}

fn ratio_label(ratio: Option<f64>, low: &str, matched: &str, high: &str, missing: &str) -> String {
    match ratio {
        Some(value) if value < 0.75 => low.to_string(),
        Some(value) if value > 1.35 => high.to_string(),
        Some(_) => matched.to_string(),
        None => missing.to_string(),
    }
}

fn grade_effect_label(grade_delta: Option<f64>) -> String {
    match grade_delta {
        Some(value) if value >= 8.0 => "高于均分".to_string(),
        Some(value) if value <= -8.0 => "低于均分".to_string(),
        Some(_) => "接近均分".to_string(),
        None => "待观察".to_string(),
    }
}

fn course_name_from_row(row: &Value) -> String {
    row["name"].as_str().unwrap_or("").to_string()
}

fn numeric_items(
    rows: &[Value],
    key: &str,
    positive_only: bool,
    sort_by_ratio_deviation: bool,
) -> Vec<(String, f64)> {
    let mut items = rows
        .iter()
        .filter_map(|row| {
            let value = row.get(key)?.as_f64()?;
            if !value.is_finite() || (positive_only && value <= 0.0) {
                return None;
            }
            Some((course_name_from_row(row), round_two(value)))
        })
        .collect::<Vec<_>>();

    items.sort_by(|a, b| {
        let left = if sort_by_ratio_deviation {
            (a.1 - 1.0).abs()
        } else {
            a.1
        };
        let right = if sort_by_ratio_deviation {
            (b.1 - 1.0).abs()
        } else {
            b.1
        };
        right
            .partial_cmp(&left)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    items
}

fn series_from_items(items: &[(String, f64)]) -> Value {
    json!({
        "labels": items.iter().map(|item| item.0.clone()).collect::<Vec<_>>(),
        "data": items.iter().map(|item| item.1).collect::<Vec<_>>(),
        "items": items.iter().map(|item| json!({
            "name": item.0,
            "value": item.1
        })).collect::<Vec<_>>()
    })
}

fn count_series(labels: Vec<String>) -> Value {
    let mut totals = HashMap::<String, f64>::new();
    for label in labels {
        *totals.entry(label).or_insert(0.0) += 1.0;
    }
    let mut items = totals.into_iter().collect::<Vec<_>>();
    items.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.0.cmp(&b.0))
    });
    series_from_items(&items)
}

fn metric_view(
    label: &str,
    description: &str,
    unit: &str,
    bar_title: &str,
    pie_title: &str,
    bar: Value,
    pie: Value,
) -> Value {
    metric_view_with_units(label, description, unit, unit, bar_title, pie_title, bar, pie)
}

fn metric_view_with_units(
    label: &str,
    description: &str,
    bar_unit: &str,
    pie_unit: &str,
    bar_title: &str,
    pie_title: &str,
    bar: Value,
    pie: Value,
) -> Value {
    json!({
        "label": label,
        "description": description,
        "unit": bar_unit,
        "bar_unit": bar_unit,
        "pie_unit": pie_unit,
        "bar_title": bar_title,
        "pie_title": pie_title,
        "bar": bar,
        "pie": pie
    })
}

fn build_course_profile_summary(courses: &[Value]) -> Value {
    let mut total_credits = 0.0_f64;
    let mut graded_credits = 0.0_f64;
    let mut pending_credits = 0.0_f64;
    let mut grade_sum = 0.0_f64;
    let mut grade_count = 0.0_f64;
    let mut weighted_sum = 0.0_f64;
    let mut matched_count = 0_i64;
    let mut unmatched_count = 0_i64;
    let mut shared_count = 0_i64;
    let mut total_hours = 0.0_f64;
    let mut total_positive_efficiency = 0.0_f64;

    let mut chart_rows = Vec::new();
    for item in courses {
        let credits = item["credits"].as_f64().unwrap_or(0.0);
        total_credits += credits;
        if item["grade_status"].as_str() == Some("graded") {
            if let Some(grade) = item["grade"].as_f64() {
                graded_credits += credits;
                grade_sum += grade;
                grade_count += 1.0;
                weighted_sum += grade * credits;
            }
        } else {
            pending_credits += credits;
        }
        if item["matched_subcategory_id"].is_null() {
            unmatched_count += 1;
        } else {
            matched_count += 1;
        }
        if item["shared_mapping_count"].as_i64().unwrap_or(0) > 1 {
            shared_count += 1;
        }
        let hours = item["learning_hours"].as_f64().unwrap_or(0.0);
        total_hours += hours;
        total_positive_efficiency += item["efficiency"]
            .as_f64()
            .filter(|value| *value > 0.0)
            .unwrap_or(0.0);
        chart_rows.push(json!({
            "name": item["course_name"].as_str().unwrap_or(""),
            "credits": credits,
            "grade": item["grade"].as_f64(),
            "learning_hours": hours,
            "efficiency": item["efficiency"].as_f64()
        }));
    }

    let average_grade = if grade_count > 0.0 {
        Some(((grade_sum / grade_count) * 100.0).round() / 100.0)
    } else {
        None
    };
    let weighted_grade = if graded_credits > 0.0 {
        Some(((weighted_sum / graded_credits) * 100.0).round() / 100.0)
    } else {
        None
    };
    let grade_coverage = if total_credits > 0.0 {
        ((graded_credits / total_credits) * 1000.0).round() / 10.0
    } else {
        0.0
    };

    let mut analysis_rows = Vec::new();
    for item in courses {
        let name = item["course_name"].as_str().unwrap_or("").to_string();
        let credits = item["credits"].as_f64().unwrap_or(0.0);
        let hours = item["learning_hours"].as_f64().unwrap_or(0.0);
        let efficiency = item["efficiency"].as_f64();
        let grade = item["grade"].as_f64();
        let credit_share = if total_credits > 0.0 {
            credits / total_credits
        } else {
            0.0
        };
        let expected_hours = total_hours * credit_share;
        let duration_credit_ratio = if expected_hours > 0.0 && hours > 0.0 {
            Some(hours / expected_hours)
        } else {
            None
        };
        let efficiency_share = efficiency
            .filter(|value| *value > 0.0)
            .map(|value| {
                if total_positive_efficiency > 0.0 {
                    value / total_positive_efficiency
                } else {
                    0.0
                }
            })
            .unwrap_or(0.0);
        let efficiency_credit_ratio = if credit_share > 0.0 && efficiency_share > 0.0 {
            Some(efficiency_share / credit_share)
        } else {
            None
        };
        let grade_delta = grade.and_then(|value| weighted_grade.map(|avg| value - avg));
        let grade_credit_contribution = grade.map(|value| value * credits);
        let grade_credit_delta = grade_delta.map(|value| value * credits);
        let grade_return_index = match (grade, hours > 0.0) {
            (Some(value), true) => Some(value * credits / hours),
            _ => None,
        };

        let has_credit = credits > 0.0;
        let duration_fit_label = if !has_credit {
            "待补学分".to_string()
        } else {
            ratio_label(
                duration_credit_ratio,
                "投入偏少",
                "投入匹配",
                "投入偏多",
                "缺少学习记录",
            )
        };
        let efficiency_fit_label = if !has_credit {
            "待补学分".to_string()
        } else {
            ratio_label(
                efficiency_credit_ratio,
                "效率偏低",
                "效率匹配",
                "效率突出",
                "缺少效率数据",
            )
        };
        let grade_label = grade_effect_label(grade_delta);
        let shared_mapping_count = item["shared_mapping_count"].as_i64().unwrap_or(0);
        let matched = !item["matched_subcategory_id"].is_null();
        let high_credit = credit_share >= 0.25
            || (courses.len() > 0 && credits >= total_credits / courses.len() as f64 * 1.25);

        let mut data_quality_flags = Vec::<String>::new();
        if !matched {
            data_quality_flags.push("未匹配".to_string());
        }
        if shared_mapping_count > 1 {
            data_quality_flags.push("共享映射".to_string());
        }
        if !has_credit {
            data_quality_flags.push("待补学分".to_string());
        }
        if hours <= 0.0 {
            data_quality_flags.push("缺少学习记录".to_string());
        }

        let mut diagnosis_tags = Vec::<String>::new();
        if !matched {
            diagnosis_tags.push("映射不确定".to_string());
        }
        if hours <= 0.0 {
            diagnosis_tags.push("缺少学习记录".to_string());
        }
        if !has_credit {
            diagnosis_tags.push("待补学分".to_string());
        }
        if grade.is_none() && has_credit && high_credit {
            diagnosis_tags.push("高学分成绩待出".to_string());
        } else if grade.is_none() {
            diagnosis_tags.push("成绩待观察".to_string());
        }
        if matches!(duration_credit_ratio, Some(value) if value < 0.75) && high_credit {
            diagnosis_tags.push("高学分低投入".to_string());
        } else if matches!(duration_credit_ratio, Some(value) if value < 0.75) {
            diagnosis_tags.push("投入偏少".to_string());
        }
        if matches!(duration_credit_ratio, Some(value) if value > 1.35)
            && matches!(grade_delta, Some(value) if value < -5.0)
        {
            diagnosis_tags.push("高投入低成绩".to_string());
        }
        if matches!(duration_credit_ratio, Some(value) if value >= 1.0)
            && matches!(grade_delta, Some(value) if value >= 5.0)
        {
            diagnosis_tags.push("重投入高回报".to_string());
        }
        if matches!(duration_credit_ratio, Some(value) if value < 1.0)
            && matches!(grade_delta, Some(value) if value >= 5.0)
        {
            diagnosis_tags.push("低投入高成绩".to_string());
        }
        if matches!(efficiency_credit_ratio, Some(value) if value > 1.35) {
            diagnosis_tags.push("高效优势".to_string());
        }
        if matches!(efficiency_credit_ratio, Some(value) if value < 0.75) && high_credit {
            diagnosis_tags.push("效率偏低".to_string());
        }
        if matches!(duration_credit_ratio, Some(value) if (0.75..=1.35).contains(&value))
            && matches!(efficiency_credit_ratio, Some(value) if value < 0.75)
        {
            diagnosis_tags.push("投入匹配但效率偏低".to_string());
        }
        if diagnosis_tags.is_empty() {
            diagnosis_tags.push("状态稳定".to_string());
        }

        analysis_rows.push(json!({
            "id": item["id"].as_i64(),
            "name": name,
            "semester": item["semester"].as_str(),
            "credits": round_two(credits),
            "grade": grade,
            "grade_status": item["grade_status"].as_str(),
            "learning_hours": round_two(hours),
            "process_efficiency": efficiency.map(round_two),
            "credit_share": percent_share(credits, total_credits),
            "duration_share": percent_share(hours, total_hours),
            "efficiency_share": percent_share(efficiency.unwrap_or(0.0).max(0.0), total_positive_efficiency),
            "expected_hours": round_two(expected_hours),
            "duration_credit_ratio": duration_credit_ratio.map(round_two),
            "duration_fit_label": duration_fit_label,
            "efficiency_credit_ratio": efficiency_credit_ratio.map(round_two),
            "efficiency_fit_label": efficiency_fit_label,
            "grade_delta": grade_delta.map(round_two),
            "grade_credit_contribution": grade_credit_contribution.map(round_two),
            "grade_credit_delta": grade_credit_delta.map(round_two),
            "grade_return_index": grade_return_index.map(round_two),
            "grade_effect_label": grade_label,
            "diagnosis_tags": diagnosis_tags,
            "data_quality_flags": data_quality_flags
        }));
    }

    let duration_items = numeric_items(&analysis_rows, "learning_hours", true, false);
    let efficiency_items = numeric_items(&analysis_rows, "process_efficiency", true, false);
    let credit_items = numeric_items(&analysis_rows, "credits", true, false);
    let duration_credit_items = numeric_items(&analysis_rows, "duration_credit_ratio", false, true);
    let efficiency_credit_items =
        numeric_items(&analysis_rows, "efficiency_credit_ratio", false, true);
    let grade_performance_items = numeric_items(&analysis_rows, "grade_delta", false, false);
    let grade_contribution_items =
        numeric_items(&analysis_rows, "grade_credit_contribution", true, false);
    let grade_return_items = numeric_items(&analysis_rows, "grade_return_index", true, false);
    let mut diagnosis_items = analysis_rows
        .iter()
        .filter_map(|row| {
            let count = row["diagnosis_tags"].as_array()?.len() as f64;
            Some((course_name_from_row(row), count))
        })
        .collect::<Vec<_>>();
    diagnosis_items.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let duration_fit_labels = analysis_rows
        .iter()
        .filter_map(|row| row["duration_fit_label"].as_str().map(str::to_string))
        .collect::<Vec<_>>();
    let efficiency_fit_labels = analysis_rows
        .iter()
        .filter_map(|row| row["efficiency_fit_label"].as_str().map(str::to_string))
        .collect::<Vec<_>>();
    let grade_effect_labels = analysis_rows
        .iter()
        .filter_map(|row| row["grade_effect_label"].as_str().map(str::to_string))
        .collect::<Vec<_>>();
    let diagnosis_labels = analysis_rows
        .iter()
        .flat_map(|row| {
            row["diagnosis_tags"]
                .as_array()
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .filter_map(|value| value.as_str().map(str::to_string))
        })
        .collect::<Vec<_>>();

    let metric_views = json!({
        "duration": metric_view(
            "时长",
            "按课程累计学习时长排名，回答时间主要投向了哪些课。",
            "h",
            "课程时长排名",
            "时长占比",
            series_from_items(&duration_items),
            series_from_items(&duration_items),
        ),
        "efficiency": metric_view(
            "效率",
            "沿用学习记录效率公式，观察过程状态和投入规模形成的效率排名。",
            "",
            "课程效率排名",
            "效率占比",
            series_from_items(&efficiency_items),
            series_from_items(&efficiency_items),
        ),
        "credits": metric_view(
            "学分",
            "按课程学分衡量重要程度，作为投入期望的基准。",
            "学分",
            "课程学分排名",
            "学分占比",
            series_from_items(&credit_items),
            series_from_items(&credit_items),
        ),
        "duration_credit": metric_view_with_units(
            "投入匹配",
            "用实际时长占比对比学分占比，1 表示投入与学分基本匹配。",
            "x",
            "门",
            "投入匹配偏离",
            "投入匹配分布",
            series_from_items(&duration_credit_items),
            count_series(duration_fit_labels),
        ),
        "efficiency_credit": metric_view_with_units(
            "效率匹配",
            "用效率占比对比学分占比，1 表示效率贡献与课程重要度基本匹配。",
            "x",
            "门",
            "效率匹配偏离",
            "效率匹配分布",
            series_from_items(&efficiency_credit_items),
            count_series(efficiency_fit_labels),
        ),
        "grade": metric_view_with_units(
            "成绩表现",
            "按课程成绩相对加权均分的差值观察结果表现，未出成绩归入待观察。",
            "分",
            "门",
            "成绩相对均分",
            "成绩状态分布",
            series_from_items(&grade_performance_items),
            count_series(grade_effect_labels),
        ),
        "grade_contribution": metric_view_with_units(
            "成绩贡献",
            "用成绩乘以学分衡量课程对整体结果的贡献，高学分课程的成绩影响会被放大。",
            "分·学分",
            "门",
            "成绩贡献排名",
            "成绩状态分布",
            series_from_items(&grade_contribution_items),
            count_series(
                analysis_rows
                    .iter()
                    .filter_map(|row| {
                        if row["grade_credit_contribution"].as_f64().is_some() {
                            Some("已出成绩".to_string())
                        } else {
                            row["grade_effect_label"].as_str().map(str::to_string)
                        }
                    })
                    .collect::<Vec<_>>(),
            ),
        ),
        "grade_return": metric_view_with_units(
            "成绩回报",
            "用成绩乘以学分再除以学习时长，观察结果与投入之间的回报关系。",
            "分·学分/h",
            "门",
            "成绩回报排名",
            "成绩状态分布",
            series_from_items(&grade_return_items),
            count_series(
                analysis_rows
                    .iter()
                    .filter_map(|row| {
                        if row["grade_return_index"].as_f64().is_some() {
                            Some("可计算回报".to_string())
                        } else if row["grade"].as_f64().is_none() {
                            Some("成绩待观察".to_string())
                        } else if row["learning_hours"].as_f64().unwrap_or(0.0) <= 0.0 {
                            Some("缺少学习记录".to_string())
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>(),
            ),
        ),
        "diagnosis": metric_view(
            "诊断",
            "汇总投入、效率、学分、成绩和数据质量产生的诊断标签。",
            "项",
            "课程诊断项数量",
            "诊断标签分布",
            series_from_items(&diagnosis_items),
            count_series(diagnosis_labels),
        )
    });

    json!({
        "total_courses": courses.len(),
        "total_credits": ((total_credits * 100.0).round() / 100.0),
        "graded_credits": ((graded_credits * 100.0).round() / 100.0),
        "pending_credits": ((pending_credits * 100.0).round() / 100.0),
        "average_grade": average_grade,
        "weighted_grade": weighted_grade,
        "grade_coverage": grade_coverage,
        "matched_count": matched_count,
        "unmatched_count": unmatched_count,
        "shared_mapping_count": shared_count,
        "total_learning_hours": ((total_hours * 100.0).round() / 100.0),
        "chart_rows": chart_rows,
        "analysis_rows": analysis_rows,
        "metric_views": metric_views
    })
}

#[tauri::command]
pub fn course_profile_summary(
    state: State<'_, AppState>,
    query: CourseProfileListQuery,
) -> AppResult<Value> {
    let conn = connection(&state)?;
    let courses = load_courses(&conn, &query)?;
    Ok(json!({
        "success": true,
        "summary": build_course_profile_summary(&courses)
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn course(
        id: i64,
        name: &str,
        credits: f64,
        grade: Option<f64>,
        hours: f64,
        efficiency: Option<f64>,
    ) -> Value {
        json!({
            "id": id,
            "semester": "测试学期",
            "course_name": name,
            "credits": credits,
            "grade": grade,
            "grade_status": if grade.is_some() { "graded" } else { "pending" },
            "learning_hours": hours,
            "efficiency": efficiency,
            "matched_subcategory_id": id,
            "shared_mapping_count": 0
        })
    }

    fn analysis_row<'a>(summary: &'a Value, name: &str) -> &'a Value {
        summary["analysis_rows"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["name"].as_str() == Some(name))
            .unwrap()
    }

    fn metric_items<'a>(summary: &'a Value, metric: &str) -> &'a Vec<Value> {
        summary["metric_views"][metric]["bar"]["items"]
            .as_array()
            .unwrap()
    }

    #[test]
    fn investment_match_corrects_raw_duration_for_credit_weight() {
        let summary = build_course_profile_summary(&[
            course(1, "三学分课", 3.0, Some(90.0), 101.0, Some(1.0)),
            course(2, "一学分课", 1.0, Some(90.0), 100.0, Some(1.0)),
        ]);

        let duration_items = metric_items(&summary, "duration");
        assert_eq!(duration_items[0]["name"].as_str(), Some("三学分课"));

        let investment_row = analysis_row(&summary, "三学分课");
        assert_eq!(
            investment_row["duration_fit_label"].as_str(),
            Some("投入偏少")
        );

        let investment_items = metric_items(&summary, "duration_credit");
        assert_eq!(investment_items[0]["name"].as_str(), Some("一学分课"));
    }

    #[test]
    fn grade_views_separate_performance_contribution_and_return() {
        let summary = build_course_profile_summary(&[
            course(1, "几分钟一百分", 1.0, Some(100.0), 0.1, Some(2.0)),
            course(2, "几百小时九十五", 4.0, Some(95.0), 300.0, Some(1.0)),
        ]);

        assert_eq!(
            metric_items(&summary, "grade")[0]["name"].as_str(),
            Some("几分钟一百分")
        );
        assert_eq!(
            metric_items(&summary, "grade_contribution")[0]["name"].as_str(),
            Some("几百小时九十五")
        );
        assert_eq!(
            metric_items(&summary, "grade_return")[0]["name"].as_str(),
            Some("几分钟一百分")
        );
    }

    #[test]
    fn pending_grade_stays_in_input_metrics_and_out_of_grade_rankings() {
        let summary = build_course_profile_summary(&[
            course(1, "已出成绩", 2.0, Some(90.0), 10.0, Some(1.0)),
            course(2, "成绩未出", 4.0, None, 20.0, Some(1.0)),
        ]);

        assert!(metric_items(&summary, "duration")
            .iter()
            .any(|item| item["name"].as_str() == Some("成绩未出")));
        assert!(!metric_items(&summary, "grade")
            .iter()
            .any(|item| item["name"].as_str() == Some("成绩未出")));
        assert_eq!(
            analysis_row(&summary, "成绩未出")["grade_effect_label"].as_str(),
            Some("待观察")
        );
    }

    #[test]
    fn zero_credit_and_zero_hours_do_not_create_ratio_values() {
        let summary = build_course_profile_summary(&[course(
            1,
            "空资料课程",
            0.0,
            Some(80.0),
            0.0,
            None,
        )]);
        let row = analysis_row(&summary, "空资料课程");

        assert!(row["duration_credit_ratio"].is_null());
        assert!(row["efficiency_credit_ratio"].is_null());
        assert!(row["grade_return_index"].is_null());
        assert_eq!(row["duration_fit_label"].as_str(), Some("待补学分"));
        assert!(row["diagnosis_tags"]
            .as_array()
            .unwrap()
            .iter()
            .any(|tag| tag.as_str() == Some("缺少学习记录")));
    }
}
