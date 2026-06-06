use std::process::{Child, Command, Stdio};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use tauri::{AppHandle, Manager};
use crate::downloader::{download_syncthing, get_app_dir};

pub struct SyncthingState {
    pub process: Mutex<Option<Child>>,
    pub api_key: Mutex<Option<String>>,
    pub config_dir: PathBuf,
}

#[tauri::command]
pub fn get_syncthing_status(state: tauri::State<SyncthingState>) -> bool {
    let mut process_guard = state.process.lock().unwrap();
    if let Some(child) = process_guard.as_mut() {
        match child.try_wait() {
            Ok(Some(_status)) => false, // Exited
            Ok(None) => true,           // Running
            Err(_) => false,            // Error querying
        }
    } else {
        false
    }
}

#[tauri::command]
pub fn get_syncthing_api_key(state: tauri::State<SyncthingState>) -> String {
    let api_key_guard = state.api_key.lock().unwrap();
    api_key_guard.clone().unwrap_or_default()
}

pub async fn setup_syncthing(app: &AppHandle) {
    let app_dir = get_app_dir();
    let bin_dir = app_dir.join("bin");
    let config_dir = app_dir.join("config");

    if !bin_dir.exists() {
        std::fs::create_dir_all(&bin_dir).unwrap();
    }
    if !config_dir.exists() {
        std::fs::create_dir_all(&config_dir).unwrap();
    }

    #[cfg(target_os = "windows")]
    let bin_name = "syncthing.exe";
    #[cfg(not(target_os = "windows"))]
    let bin_name = "syncthing";

    let bin_path = bin_dir.join(bin_name);

    if !bin_path.exists() {
        println!("Syncthing 核心不存在，准备下载...");
        if let Err(e) = download_syncthing(&bin_path).await {
            eprintln!("下载 Syncthing 失败: {}", e);
            return;
        }
    }

    // 启动进程
    println!("正在启动 Syncthing 进程: {:?}", bin_path);
    println!("配置文件目录: {:?}", config_dir);

    let mut cmd = Command::new(&bin_path);
    cmd.arg(format!("--home={}", config_dir.to_string_lossy()))
       .arg("--no-browser")
       .arg("--no-restart");

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW
        cmd.creation_flags(0x08000000);
    }

    match cmd.spawn() {
        Ok(child) => {
            println!("Syncthing 已成功启动，PID: {}", child.id());
            let state: tauri::State<SyncthingState> = app.state();
            *state.process.lock().unwrap() = Some(child);

            // 等待 config.xml 生成并读取 API Key
            let config_file = config_dir.join("config.xml");
            for _ in 0..30 {
                if config_file.exists() {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }

            if config_file.exists() {
                if let Ok(content) = std::fs::read_to_string(&config_file) {
                    if let Some(start) = content.find("<apikey>") {
                        if let Some(end) = content[start..].find("</apikey>") {
                            let key = &content[start + 8..start + end];
                            *state.api_key.lock().unwrap() = Some(key.to_string());
                            println!("成功获取到 API Key: {}", key);
                        }
                    }
                }
            }
        },
        Err(e) => eprintln!("无法启动 Syncthing: {}", e),
    }
}
