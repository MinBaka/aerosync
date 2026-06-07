use crate::downloader::{download_syncthing, get_app_dir};
use std::path::PathBuf;
use std::process::{Child, Command};
use std::sync::Mutex;
use tauri::{AppHandle, Manager};

pub struct SyncthingState {
    pub process: Mutex<Option<Child>>,
    pub api_key: Mutex<Option<String>>,
    pub config_dir: PathBuf,
}

#[tauri::command]
pub fn get_syncthing_status(state: tauri::State<SyncthingState>) -> bool {
    let mut process_guard = state.process.lock().unwrap();
    if let Some(child) = process_guard.as_mut() {
        matches!(child.try_wait(), Ok(None))
    } else {
        false
    }
}

#[tauri::command]
pub fn get_syncthing_api_key(state: tauri::State<SyncthingState>) -> String {
    state.api_key.lock().unwrap().clone().unwrap_or_default()
}

pub async fn setup_syncthing(app: &AppHandle) {
    let app_dir = get_app_dir();
    let bin_dir = app_dir.join("bin");
    let config_dir = app_dir.join("config");

    if let Err(error) = std::fs::create_dir_all(&bin_dir) {
        eprintln!("无法创建 Syncthing 二进制目录: {error}");
        return;
    }

    if let Err(error) = std::fs::create_dir_all(&config_dir) {
        eprintln!("无法创建 Syncthing 配置目录: {error}");
        return;
    }

    #[cfg(target_os = "windows")]
    let bin_name = "syncthing.exe";
    #[cfg(not(target_os = "windows"))]
    let bin_name = "syncthing";

    let bin_path = bin_dir.join(bin_name);

    if !bin_path.exists() {
        println!("Syncthing 核心不存在，准备下载...");
        if let Err(error) = download_syncthing(&bin_path).await {
            eprintln!("下载 Syncthing 失败: {error}");
            return;
        }
    }

    println!("正在启动 Syncthing 进程: {:?}", bin_path);
    println!("配置文件目录: {:?}", config_dir);

    let mut command = Command::new(&bin_path);
    command
        .arg(format!("--home={}", config_dir.to_string_lossy()))
        .arg("--no-browser")
        .arg("--no-restart");

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }

    match command.spawn() {
        Ok(child) => {
            println!("Syncthing 已成功启动，PID: {}", child.id());
            let state: tauri::State<SyncthingState> = app.state();
            *state.process.lock().unwrap() = Some(child);

            let config_file = state.config_dir.join("config.xml");
            for _ in 0..30 {
                if config_file.exists() {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }

            if let Ok(content) = std::fs::read_to_string(&config_file) {
                if let Some(api_key) = extract_api_key(&content) {
                    *state.api_key.lock().unwrap() = Some(api_key.clone());
                    println!("成功获取到 API Key: {api_key}");
                }
            }
        }
        Err(error) => eprintln!("无法启动 Syncthing: {error}"),
    }
}

fn extract_api_key(config_xml: &str) -> Option<String> {
    let start = config_xml.find("<apikey>")? + "<apikey>".len();
    let end = config_xml[start..].find("</apikey>")? + start;
    Some(config_xml[start..end].to_string())
}
