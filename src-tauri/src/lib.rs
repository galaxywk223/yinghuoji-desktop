mod commands;
mod db;
mod models;

use std::fs;
use std::path::PathBuf;

use serde::Serialize;
use tauri::Manager;

#[derive(Clone)]
pub struct AppState {
    pub base_dir: PathBuf,
    pub db_path: PathBuf,
    pub attachments_dir: PathBuf,
    pub exports_dir: PathBuf,
}

#[derive(Debug, Serialize)]
pub struct FrontendError {
    pub message: String,
}

impl From<anyhow::Error> for FrontendError {
    fn from(value: anyhow::Error) -> Self {
        Self {
            message: value.to_string(),
        }
    }
}

impl From<rusqlite::Error> for FrontendError {
    fn from(value: rusqlite::Error) -> Self {
        Self {
            message: value.to_string(),
        }
    }
}

impl From<serde_json::Error> for FrontendError {
    fn from(value: serde_json::Error) -> Self {
        Self {
            message: value.to_string(),
        }
    }
}

impl From<std::io::Error> for FrontendError {
    fn from(value: std::io::Error) -> Self {
        Self {
            message: value.to_string(),
        }
    }
}

impl From<keyring::Error> for FrontendError {
    fn from(value: keyring::Error) -> Self {
        Self {
            message: value.to_string(),
        }
    }
}

impl From<zip::result::ZipError> for FrontendError {
    fn from(value: zip::result::ZipError) -> Self {
        Self {
            message: value.to_string(),
        }
    }
}

pub type AppResult<T> = Result<T, FrontendError>;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let base_dir = app
                .path()
                .app_data_dir()
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;
            fs::create_dir_all(&base_dir)?;
            let attachments_dir = base_dir.join("attachments");
            let exports_dir = base_dir.join("exports");
            fs::create_dir_all(&attachments_dir)?;
            fs::create_dir_all(&exports_dir)?;

            let state = AppState {
                db_path: base_dir.join("app.db"),
                base_dir,
                attachments_dir,
                exports_dir,
            };

            db::initialize_database(&state)?;
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_initialize,
            commands::profile_get,
            commands::profile_update,
            commands::settings_get,
            commands::settings_set,
            commands::stages_list,
            commands::stage_get,
            commands::stage_create,
            commands::stage_update,
            commands::stage_delete,
            commands::categories_list,
            commands::category_get,
            commands::category_create,
            commands::category_update,
            commands::category_delete,
            commands::subcategory_create,
            commands::subcategory_update,
            commands::subcategory_delete,
            commands::subcategory_merge,
            commands::records_structured,
            commands::records_list,
            commands::records_recent,
            commands::record_get,
            commands::record_create,
            commands::record_update,
            commands::record_delete,
            commands::record_statistics,
            commands::dashboard_summary,
            commands::charts_overview,
            commands::charts_categories,
            commands::charts_category_trend,
            commands::charts_stages,
            commands::countdowns_list,
            commands::countdown_get,
            commands::countdown_create,
            commands::countdown_update,
            commands::countdown_delete,
            commands::mottos_list,
            commands::motto_get,
            commands::motto_random,
            commands::motto_create,
            commands::motto_update,
            commands::motto_delete,
            commands::milestones_list,
            commands::milestone_get,
            commands::milestone_create,
            commands::milestone_update,
            commands::milestone_delete,
            commands::milestone_categories_list,
            commands::milestone_category_create,
            commands::milestone_category_update,
            commands::milestone_category_delete,
            commands::milestone_attachment_upload,
            commands::milestone_attachment_delete,
            commands::milestone_attachment_get,
            commands::backup_export_zip,
            commands::backup_import_zip,
            commands::backup_clear_all,
            commands::ai_get_config,
            commands::ai_update_config,
            commands::ai_chat_sessions,
            commands::ai_chat_messages,
            commands::ai_chat_send,
            commands::ai_history_list
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
