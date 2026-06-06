// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod downloader;
mod syncthing;

use std::sync::Mutex;
use tauri::Manager;
use syncthing::{SyncthingState, get_syncthing_status, setup_syncthing};

#[tauri::command]
fn win_minimize(window: tauri::Window) {
    let _ = window.minimize();
}

#[tauri::command]
fn win_toggle_maximize(window: tauri::Window) {
    if let Ok(maximized) = window.is_maximized() {
        if maximized {
            let _ = window.unmaximize();
        } else {
            let _ = window.maximize();
        }
    }
}

#[tauri::command]
fn win_close(window: tauri::Window) {
    let _ = window.close();
}

#[tauri::command]
fn win_start_drag(window: tauri::Window) {
    let _ = window.start_dragging();
}

fn main() {
    // 修复 Linux/Wayland 下 WebKitGTK 的协议崩溃问题
    std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    std::env::set_var("WEBKIT_DISABLE_COMPOSITING_MODE", "1");

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
            syncthing::get_syncthing_api_key,
            win_minimize,
            win_toggle_maximize,
            win_close,
            win_start_drag
        ])
        .setup(|app| {
            let handle = app.handle().clone();

            // 异步执行下载和启动逻辑
            tauri::async_runtime::spawn(async move {
                setup_syncthing(&handle).await;
            });

            Ok(())
        })
        .on_window_event(|app, event| match event {
            tauri::WindowEvent::Destroyed => {
                // 窗口销毁时杀死后台的 syncthing 进程
                let state: tauri::State<SyncthingState> = app.state();
                let mut process_guard = state.process.lock().unwrap();
                if let Some(mut child) = process_guard.take() {
                    println!("正在停止 Syncthing 进程...");
                    let _ = child.kill();
                    let _ = child.wait();
                }
            }
            _ => {}
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
