#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! Island desktop companion — system tray app with local API server.
//!
//! Runs silently in the system tray, exposing a local HTTP/WebSocket API
//! that the browser extension uses to queue and monitor downloads.

mod config;
mod downloader;
mod error;
mod formats;
mod models;
mod queue;
mod server;
mod platform;

use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};
use tracing::{error, info};

use crate::config::AppConfig;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const SETTINGS_FILE_NAME: &str = "settings.json";
const API_PORT: u16 = 49152;

// ---------------------------------------------------------------------------
// Persisted settings
// ---------------------------------------------------------------------------

/// Settings persisted to disk in the app config directory.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct AppSettings {
    download_dir: Option<String>,
}

/// Response shape for settings-related Tauri commands.
#[derive(Debug, Clone, Serialize)]
struct DownloadSettingsResponse {
    current_download_dir: String,
    default_download_dir: String,
}

// ---------------------------------------------------------------------------
// Settings file I/O
// ---------------------------------------------------------------------------

fn settings_path(app: &AppHandle) -> anyhow::Result<PathBuf> {
    let dir = app.path().app_config_dir()?;
    fs::create_dir_all(&dir)?;
    Ok(dir.join(SETTINGS_FILE_NAME))
}

fn load_settings(app: &AppHandle) -> anyhow::Result<AppSettings> {
    let path = settings_path(app)?;
    if !path.exists() {
        return Ok(AppSettings::default());
    }
    let content = fs::read_to_string(path)?;
    let parsed = serde_json::from_str::<AppSettings>(&content)?;
    Ok(parsed)
}

fn save_settings(app: &AppHandle, settings: &AppSettings) -> anyhow::Result<()> {
    let path = settings_path(app)?;
    let data = serde_json::to_string_pretty(settings)?;
    fs::write(path, data)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Settings window
// ---------------------------------------------------------------------------

pub fn open_settings_window(app: &AppHandle) -> anyhow::Result<()> {
    if let Some(win) = app.get_webview_window("settings") {
        let _ = win.show();
        let _ = win.set_focus();
        return Ok(());
    }

    let win = WebviewWindowBuilder::new(app, "settings", WebviewUrl::App("settings.html".into()))
        .title("Island Settings")
        .inner_size(690.0, 420.0)
        .resizable(false)
        .minimizable(false)
        .maximizable(false)
        .visible(true)
        .build()?;

    let _ = win.set_focus();
    Ok(())
}

// ---------------------------------------------------------------------------
// Tauri commands
// ---------------------------------------------------------------------------

#[tauri::command]
fn get_download_settings(app: AppHandle) -> Result<DownloadSettingsResponse, String> {
    let settings = load_settings(&app).map_err(|e| e.to_string())?;
    let default_download_dir = config::default_download_dir().display().to_string();
    let current_download_dir = settings
        .download_dir
        .unwrap_or_else(|| default_download_dir.clone());
    Ok(DownloadSettingsResponse {
        current_download_dir,
        default_download_dir,
    })
}

#[tauri::command]
async fn set_download_directory(
    app: AppHandle,
    state: tauri::State<'_, AppConfig>,
    path: String,
) -> Result<DownloadSettingsResponse, String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err("Download path cannot be empty".to_string());
    }

    let target = PathBuf::from(trimmed);
    fs::create_dir_all(&target).map_err(|e| format!("Failed to create download path: {e}"))?;
    let normalized = target
        .canonicalize()
        .unwrap_or_else(|_| target.clone())
        .display()
        .to_string();

    // Persist to disk
    let mut settings = load_settings(&app).map_err(|e| e.to_string())?;
    settings.download_dir = Some(normalized.clone());
    save_settings(&app, &settings).map_err(|e| e.to_string())?;

    // Update runtime config
    state.set_download_dir(Some(normalized.clone())).await;
    info!(path = %normalized, "download directory updated");

    let default_download_dir = config::default_download_dir().display().to_string();

    Ok(DownloadSettingsResponse {
        current_download_dir: normalized,
        default_download_dir,
    })
}

#[tauri::command]
async fn reset_download_directory(
    app: AppHandle,
    state: tauri::State<'_, AppConfig>,
) -> Result<DownloadSettingsResponse, String> {
    let mut settings = load_settings(&app).map_err(|e| e.to_string())?;
    settings.download_dir = None;
    save_settings(&app, &settings).map_err(|e| e.to_string())?;

    state.set_download_dir(None).await;
    info!("download directory reset to default");

    let default_download_dir = config::default_download_dir().display().to_string();
    Ok(DownloadSettingsResponse {
        current_download_dir: default_download_dir.clone(),
        default_download_dir,
    })
}

#[tauri::command]
fn browse_download_directory(current_path: Option<String>) -> Result<Option<String>, String> {
    let mut dialog = rfd::FileDialog::new();
    if let Some(path) = current_path {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            dialog = dialog.set_directory(trimmed);
        }
    }
    let selected = dialog.pick_folder().map(|p| p.display().to_string());
    Ok(selected)
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() {
    // Initialize structured logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .compact()
        .init();

    info!(version = server::VERSION, "starting Island desktop");

    // Create shared config
    let app_config = AppConfig::new(None);

    // Start download queue
    let (queue_state, receiver) = queue::QueueState::new(128);
    let worker_queue = queue_state.clone();
    let worker_config = app_config.clone();
    tauri::async_runtime::spawn(queue::run_worker(worker_queue, worker_config, receiver));

    let tray_config = app_config.clone();

    tauri::Builder::default()
        .manage(app_config.clone())
        .invoke_handler(tauri::generate_handler![
            get_download_settings,
            set_download_directory,
            reset_download_directory,
            browse_download_directory
        ])
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--hidden"]),
        ))
        .setup(move |app| {
            // Start API server
            let api_queue = queue_state.clone();
            let api_config = app_config.clone();
            let api_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                if let Err(err) = server::run(api_queue, api_config, api_handle, API_PORT).await {
                    error!(error = %err, "Island API server crashed");
                }
            });

            // Load persisted settings into runtime config
            if let Ok(settings) = load_settings(app.handle()) {
                if let Some(dir) = &settings.download_dir {
                    let config = app_config.clone();
                    let dir = dir.clone();
                    tauri::async_runtime::spawn(async move {
                        config.set_download_dir(Some(dir)).await;
                    });
                    info!(
                        download_dir = settings.download_dir.as_deref().unwrap_or("(default)"),
                        "loaded persisted settings"
                    );
                }
            }

            // Build system tray
            let settings_item = MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)?;
            let open_downloads = MenuItem::with_id(
                app,
                "open_downloads",
                "Open Downloads Folder",
                true,
                None::<&str>,
            )?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&settings_item, &open_downloads, &quit])?;

            let _ = TrayIconBuilder::with_id("island")
                .menu(&menu)
                .on_menu_event({
                    let config = tray_config.clone();
                    move |app, event| match event.id.as_ref() {
                        "settings" => {
                            if let Err(err) = open_settings_window(app.app_handle()) {
                                error!(error = %err, "failed to open settings window");
                            }
                        }
                        "open_downloads" => {
                            let cfg = config.clone();
                            tauri::async_runtime::spawn(async move {
                                if let Err(err) = platform::open_downloads_folder(&cfg).await {
                                    error!(error = %err, "failed to open downloads folder");
                                }
                            });
                        }
                        "quit" => {
                            info!("user quit from tray menu");
                            app.exit(0);
                        }
                        _ => {}
                    }
                })
                .build(app)?;

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Island Tauri app");
}
