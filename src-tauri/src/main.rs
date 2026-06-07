// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod downloader;
mod syncthing;

use std::sync::Mutex;
use syncthing::{get_syncthing_status, setup_syncthing, SyncthingState};
use tauri::Manager;

#[tauri::command]
fn minimize_window(window: tauri::Window) -> Result<(), String> {
    window.minimize().map_err(|error| error.to_string())
}

#[tauri::command]
fn toggle_maximize_window(window: tauri::Window) -> Result<(), String> {
    if window.is_maximized().map_err(|error| error.to_string())? {
        window.unmaximize().map_err(|error| error.to_string())
    } else {
        window.maximize().map_err(|error| error.to_string())
    }
}

#[tauri::command]
fn close_window(window: tauri::Window) -> Result<(), String> {
    window.close().map_err(|error| error.to_string())
}

#[tauri::command]
fn start_window_drag(window: tauri::Window) -> Result<(), String> {
    window.start_dragging().map_err(|error| error.to_string())
}

fn main() {
    #[cfg(target_os = "linux")]
    std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");

    let app_dir = downloader::get_app_dir();
    let config_dir = app_dir.join("config");

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(SyncthingState {
            process: Mutex::new(None),
            api_key: Mutex::new(None),
            config_dir,
        })
        .invoke_handler(tauri::generate_handler![
            get_syncthing_status,
            minimize_window,
            toggle_maximize_window,
            close_window,
            start_window_drag,
            syncthing::get_syncthing_api_key,
            syncthing::get_syncthing_overview,
            syncthing::get_syncthing_config,
            syncthing::get_syncthing_system_status,
            syncthing::get_syncthing_connections,
            syncthing::open_syncthing_web,
            syncthing::start_syncthing,
            syncthing::shutdown_syncthing,
            syncthing::restart_syncthing,
            syncthing::add_syncthing_folder,
            syncthing::pause_syncthing_folder,
            syncthing::resume_syncthing_folder,
            syncthing::remove_syncthing_folder,
            syncthing::rescan_syncthing_folder,
            syncthing::rescan_all_syncthing_folders,
            syncthing::add_syncthing_device,
            syncthing::pause_syncthing_device,
            syncthing::resume_syncthing_device,
            syncthing::pause_all_syncthing_devices,
            syncthing::resume_all_syncthing_devices,
            syncthing::remove_syncthing_device
        ])
        .setup(|app| {
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                setup_syncthing(&handle).await;
            });
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::Destroyed = event {
                let state: tauri::State<SyncthingState> = window.state();
                let mut process_guard = state.process.lock().unwrap();
                if let Some(mut child) = process_guard.take() {
                    println!("正在停止 Syncthing 进程...");
                    let _ = child.kill();
                    let _ = child.wait();
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
