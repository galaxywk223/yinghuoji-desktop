use serde::{Deserialize, Deserializer};

fn deserialize_optional_string<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    match value {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(serde_json::Value::String(text)) => Ok(Some(text)),
        Some(serde_json::Value::Number(number)) => Ok(Some(number.to_string())),
        Some(serde_json::Value::Bool(flag)) => Ok(Some(flag.to_string())),
        Some(other) => Err(serde::de::Error::custom(format!(
            "expected string or number, got {other}"
        ))),
    }
}

#[derive(Debug, Deserialize)]
pub struct ProfileUpdatePayload {
    pub username: String,
    pub email: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SettingItemPayload {
    pub key: String,
    pub value: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct StagePayload {
    pub name: String,
    pub start_date: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CategoryPayload {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct SubcategoryUpdatePayload {
    pub name: String,
    pub category_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct SubcategoryMergePayload {
    pub target_subcategory_id: i64,
}

#[derive(Debug, Deserialize)]
pub struct StructuredRecordsQuery {
    pub stage_id: i64,
    pub sort: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct RecordsListQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub stage_id: Option<i64>,
    pub category_id: Option<i64>,
    pub subcategory_id: Option<i64>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RecordPayload {
    pub task: String,
    pub subcategory_id: i64,
    pub log_date: String,
    pub actual_duration: f64,
    pub time_slot: Option<String>,
    pub notes: Option<String>,
    pub mood: Option<i64>,
    pub stage_id: Option<i64>,
}

#[derive(Debug, Deserialize, Default)]
pub struct StatsQuery {
    pub stage_id: Option<i64>,
    pub days: Option<i64>,
}

#[derive(Debug, Deserialize, Default)]
pub struct RecentRecordsQuery {
    pub limit: Option<i64>,
}

#[derive(Debug, Deserialize, Default)]
pub struct ChartsOverviewQuery {
    pub view: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_string")]
    pub stage_id: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct ChartsCategoryQuery {
    #[serde(default, deserialize_with = "deserialize_optional_string")]
    pub stage_id: Option<String>,
    pub range_mode: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub metric_mode: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct CategoryTrendQuery {
    pub category_id: Option<i64>,
    pub subcategory_id: Option<i64>,
    #[serde(default, deserialize_with = "deserialize_optional_string")]
    pub stage_id: Option<String>,
    pub range_mode: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub granularity: Option<String>,
    pub metric_mode: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CountdownPayload {
    pub title: String,
    pub target_datetime_utc: String,
}

#[derive(Debug, Deserialize)]
pub struct MottoPayload {
    pub content: String,
}

#[derive(Debug, Deserialize, Default)]
pub struct MilestonesListQuery {
    pub category_id: Option<i64>,
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct MilestonePayload {
    pub title: String,
    pub event_date: Option<String>,
    pub description: Option<String>,
    pub category_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct MilestoneCategoryPayload {
    pub name: String,
}
