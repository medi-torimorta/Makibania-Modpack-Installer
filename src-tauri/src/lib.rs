mod config;
mod downloader;
mod installer;
mod launcher;
mod modrinth;
mod state;

use std::{env, path::PathBuf, sync::Mutex};

use serde::Serialize;
use tauri::Manager;
use tauri_plugin_opener::OpenerExt;

use crate::config::Side;
use crate::installer::{Installer, InstallerMode};

pub struct AppState {
    config_path: PathBuf,
    install_dir: PathBuf,
    app_dir: PathBuf,
    state_path: PathBuf,
    log_dir: PathBuf,
    is_running: Mutex<bool>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TitleStatus {
    pub can_install: bool,
    pub can_update: bool,
}

#[tauri::command]
fn initialize_title(state: tauri::State<AppState>) -> TitleStatus {
    log::info!("Called initialize_title.");
    TitleStatus {
        can_install: Installer::can_install(&state.config_path, &state.state_path)
            .inspect_err(|e| log::warn!("Disabled install mode: {:?}", e))
            .is_ok(),
        can_update: Installer::can_update(&state.config_path, &state.state_path)
            .inspect_err(|e| log::warn!("Disabled update mode: {:?}", e))
            .is_ok(),
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModeResult {
    pub is_accept: bool,
    pub error: Option<String>,
}

#[tauri::command]
fn select_mode(state: tauri::State<AppState>, mode: InstallerMode) -> ModeResult {
    log::info!("Selected mode: {mode:?}");
    let result = match mode {
        InstallerMode::Install => Installer::can_install(&state.config_path, &state.state_path),
        InstallerMode::Update => Installer::can_update(&state.config_path, &state.state_path),
    };
    if let Err(ref err) = result {
        log::error!("Failed to start {mode:?}: {err:?}");
    }
    ModeResult {
        is_accept: result.is_ok(),
        error: result.err().map(|e| e.to_string()),
    }
}

#[tauri::command]
fn open_log_folder(app: tauri::AppHandle) -> () {
    let state = app.state::<AppState>();
    if let Err(e) = app
        .opener()
        .open_path(state.log_dir.to_string_lossy().to_string(), None::<&str>)
    {
        log::error!("Failed to open log folder: {e:?}");
    }
}

#[tauri::command]
async fn run_installer(app: tauri::AppHandle, mode: InstallerMode) -> Result<(), String> {
    let state = app.state::<AppState>();
    {
        let mut is_running = state.is_running.lock().unwrap();
        if *is_running {
            log::warn!("Installer is already running, ignoring duplicate call.");
            return Err("Installer is already running".to_string());
        }
        *is_running = true;
    }
    let result = Installer::new(
        mode,
        app.clone(),
        state.config_path.clone(),
        state.install_dir.clone(),
        Side::Client,
        state.app_dir.clone(),
        state.state_path.clone(),
    )
    .map_err(|e| {
        log::error!("Failed to initialize installer: {e:?}");
        format!("{e}")
    })?
    .run()
    .await
    .map_err(|e| {
        log::error!("Failed to {}: {e:?}", mode.to_string().to_lowercase());
        format!("{e}")
    });
    {
        let mut is_running = state.is_running.lock().unwrap();
        *is_running = false;
    }

    result
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let install_dir = env::current_exe().unwrap().parent().unwrap().to_path_buf();
    let config_path = install_dir.join("config.yaml");
    let app_dir = install_dir.join("mm-installer");
    let state_path = app_dir.join("installer-state.json");
    let log_dir = app_dir.join("logs");
    tauri::Builder::default()
        .plugin(tauri_plugin_process::init())
        .plugin(
            tauri_plugin_log::Builder::new()
                .rotation_strategy(tauri_plugin_log::RotationStrategy::KeepAll)
                .timezone_strategy(tauri_plugin_log::TimezoneStrategy::UseLocal)
                .level(log::LevelFilter::Info)
                .targets([tauri_plugin_log::Target::new(
                    tauri_plugin_log::TargetKind::Folder {
                        path: log_dir.clone(),
                        file_name: None,
                    },
                )])
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            initialize_title,
            select_mode,
            run_installer,
            open_log_folder,
        ])
        .setup(|app| {
            app.manage(AppState {
                config_path,
                install_dir,
                app_dir,
                state_path,
                log_dir,
                is_running: false.into(),
            });
            log::info!("{}", "=".repeat(80));
            log::info!("App version: {}", app.package_info().version);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
