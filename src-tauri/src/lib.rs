mod commands;
mod db;
mod models;

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::Serialize;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, Runtime, Window, WindowEvent,
};

const MAIN_WINDOW_LABEL: &str = "main";
const TRAY_ID: &str = "main-tray";
const TRAY_SHOW_ID: &str = "tray-show-main";
const TRAY_EXIT_ID: &str = "tray-exit-app";

#[derive(Default)]
pub struct AppLifecycleState {
    exit_requested: AtomicBool,
}

impl AppLifecycleState {
    pub fn mark_exit_requested(&self) {
        self.exit_requested.store(true, Ordering::SeqCst);
    }

    pub fn clear_exit_requested(&self) {
        self.exit_requested.store(false, Ordering::SeqCst);
    }

    pub fn is_exit_requested(&self) -> bool {
        self.exit_requested.load(Ordering::SeqCst)
    }
}

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

impl From<zip::result::ZipError> for FrontendError {
    fn from(value: zip::result::ZipError) -> Self {
        Self {
            message: value.to_string(),
        }
    }
}

pub type AppResult<T> = Result<T, FrontendError>;

fn hide_main_window<R: Runtime>(window: &Window<R>) {
    let _ = window.set_skip_taskbar(true);
    let _ = window.hide();
}

fn restore_main_window<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
        let _ = window.set_skip_taskbar(false);
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

fn request_app_exit<R: Runtime>(app: &AppHandle<R>, lifecycle_state: &AppLifecycleState) {
    lifecycle_state.mark_exit_requested();
    if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
        let _ = window.set_skip_taskbar(false);
    }
    app.exit(0);
}

fn build_tray<R: Runtime>(app: &AppHandle<R>) -> anyhow::Result<()> {
    let show_item = MenuItem::with_id(app, TRAY_SHOW_ID, "显示主窗口", true, None::<&str>)?;
    let exit_item = MenuItem::with_id(app, TRAY_EXIT_ID, "退出应用", true, None::<&str>)?;
    let tray_menu = Menu::with_items(app, &[&show_item, &exit_item])?;
    let show_id = show_item.id().clone();
    let exit_id = exit_item.id().clone();

    let mut tray = TrayIconBuilder::with_id(TRAY_ID)
        .menu(&tray_menu)
        .tooltip("学习工作台")
        .show_menu_on_left_click(false)
        .on_menu_event(move |app, event: tauri::menu::MenuEvent| {
            if event.id() == &show_id {
                restore_main_window(app);
            } else if event.id() == &exit_id {
                let lifecycle_state = app.state::<AppLifecycleState>();
                request_app_exit(app, &lifecycle_state);
            }
        })
        .on_tray_icon_event(|tray: &tauri::tray::TrayIcon<R>, event: TrayIconEvent| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                restore_main_window(tray.app_handle());
            }
        });

    if let Some(icon) = app.default_window_icon().cloned() {
        tray = tray.icon(icon);
    }

    tray.build(app)?;
    Ok(())
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(move |app| {
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
            app.manage(AppLifecycleState::default());
            build_tray(app.handle())?;
            Ok(())
        })
        .on_window_event(move |window, event| {
            if window.label() != MAIN_WINDOW_LABEL {
                return;
            }

            if let WindowEvent::CloseRequested { api, .. } = event {
                let lifecycle_state = window.state::<AppLifecycleState>();
                if lifecycle_state.is_exit_requested() {
                    return;
                }
                api.prevent_close();
                hide_main_window(window);
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_prepare_exit_for_update,
            commands::app_cancel_exit_for_update,
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
            commands::charts_overview_forecast_status,
            commands::charts_overview_forecast_retrain,
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
            commands::milestone_attachment_open,
            commands::backup_export_zip,
            commands::backup_import_zip,
            commands::backup_clear_all
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
