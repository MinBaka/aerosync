// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod downloader;
mod syncthing;

use std::sync::Mutex;
use syncthing::{get_syncthing_status, setup_syncthing, SyncthingState};
use tauri::Manager;

fn main() {
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
            syncthing::get_syncthing_api_key
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
