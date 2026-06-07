use crate::downloader::{download_syncthing, get_app_dir};
use reqwest::{Method, Url};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::process::{Child, Command};
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Manager};

const SYNCTHING_API_URL: &str = "http://127.0.0.1:58384/rest";
const SYNCTHING_WEB_URL: &str = "http://127.0.0.1:58384";
const SYNCTHING_GUI_ADDRESS: &str = "127.0.0.1:58384";

pub struct SyncthingState {
    pub process: Mutex<Option<Child>>,
    pub api_key: Mutex<Option<String>>,
    pub config_dir: PathBuf,
}

impl Drop for SyncthingState {
    fn drop(&mut self) {
        if let Ok(mut process_guard) = self.process.lock() {
            if let Some(mut child) = process_guard.take() {
                println!("正在停止 Syncthing 进程...");
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddFolderRequest {
    id: String,
    label: String,
    path: String,
    device_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddDeviceRequest {
    device_id: String,
    name: String,
    addresses: Vec<String>,
    folder_ids: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationResult {
    restart_required: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncthingOverview {
    running: bool,
    ready: bool,
    config: Value,
    system_status: Value,
    connections: Value,
    restart_required: bool,
    error: Option<String>,
}

#[tauri::command]
pub fn get_syncthing_status(state: tauri::State<'_, SyncthingState>) -> bool {
    is_process_running(&state) || has_existing_aerosync_syncthing_process(&state)
}

#[tauri::command]
pub fn get_syncthing_api_key(state: tauri::State<'_, SyncthingState>) -> String {
    state.api_key.lock().unwrap().clone().unwrap_or_default()
}

#[tauri::command]
pub async fn get_syncthing_overview(
    state: tauri::State<'_, SyncthingState>,
) -> Result<SyncthingOverview, String> {
    let running = is_process_running(&state);
    if !running && !detect_existing_syncthing_api(&state).await && !has_existing_aerosync_syncthing_process(&state) {
        return Ok(empty_overview(false, false, None));
    }

    if let Err(error) = wait_for_syncthing_api(&state, Duration::from_secs(2)).await {
        return Ok(empty_overview(true, false, Some(error)));
    }

    let config = match syncthing_get(&state, &["config"], &[]).await {
        Ok(config) => config,
        Err(error) => return Ok(empty_overview(true, false, Some(error))),
    };
    let system_status = match syncthing_get(&state, &["system", "status"], &[]).await {
        Ok(status) => status,
        Err(error) => return Ok(empty_overview(true, false, Some(error))),
    };
    let connections = match syncthing_get(&state, &["system", "connections"], &[]).await {
        Ok(connections) => connections,
        Err(error) => return Ok(empty_overview(true, false, Some(error))),
    };
    let restart_required = get_restart_required(&state).await.unwrap_or(false);

    Ok(SyncthingOverview {
        running: true,
        ready: true,
        config,
        system_status,
        connections,
        restart_required,
        error: None,
    })
}

#[tauri::command]
pub async fn get_syncthing_config(
    state: tauri::State<'_, SyncthingState>,
) -> Result<Value, String> {
    syncthing_get(&state, &["config"], &[]).await
}

#[tauri::command]
pub async fn get_syncthing_system_status(
    state: tauri::State<'_, SyncthingState>,
) -> Result<Value, String> {
    syncthing_get(&state, &["system", "status"], &[]).await
}

#[tauri::command]
pub async fn get_syncthing_connections(
    state: tauri::State<'_, SyncthingState>,
) -> Result<Value, String> {
    syncthing_get(&state, &["system", "connections"], &[]).await
}

#[tauri::command]
pub fn open_syncthing_web() -> Result<(), String> {
    open_url(SYNCTHING_WEB_URL)
}

#[tauri::command]
pub async fn start_syncthing(state: tauri::State<'_, SyncthingState>) -> Result<(), String> {
    start_syncthing_process(&state).await
}

#[tauri::command]
pub async fn shutdown_syncthing(state: tauri::State<'_, SyncthingState>) -> Result<(), String> {
    shutdown_syncthing_process(&state).await
}

#[tauri::command]
pub async fn restart_syncthing(state: tauri::State<'_, SyncthingState>) -> Result<(), String> {
    shutdown_syncthing_process(&state).await?;
    start_syncthing_process(&state).await
}

#[tauri::command]
pub async fn add_syncthing_folder(
    state: tauri::State<'_, SyncthingState>,
    request: AddFolderRequest,
) -> Result<OperationResult, String> {
    let folder_id = request.id.trim();
    let path = request.path.trim();

    if folder_id.is_empty() {
        return Err("文件夹 ID 不能为空".to_string());
    }
    if path.is_empty() {
        return Err("本地路径不能为空".to_string());
    }

    wait_for_syncthing_api(&state, Duration::from_secs(10)).await?;

    let mut folder = syncthing_get(&state, &["config", "defaults", "folder"], &[]).await?;
    let my_id = get_my_device_id(&state).await.unwrap_or_default();
    let mut device_ids = Vec::new();

    if !my_id.is_empty() {
        push_unique(&mut device_ids, my_id);
    }
    for device_id in request.device_ids {
        push_unique(&mut device_ids, device_id);
    }

    folder["id"] = json!(folder_id);
    folder["label"] = json!(request.label.trim());
    folder["path"] = json!(path);
    folder["paused"] = json!(false);
    folder["devices"] = json!(
        device_ids
            .into_iter()
            .map(|device_id| json!({ "deviceID": device_id }))
            .collect::<Vec<_>>()
    );

    syncthing_request_empty(
        &state,
        Method::POST,
        &["config", "folders"],
        &[],
        Some(folder),
    )
    .await?;

    operation_result(&state).await
}

#[tauri::command]
pub async fn pause_syncthing_folder(
    state: tauri::State<'_, SyncthingState>,
    folder_id: String,
) -> Result<OperationResult, String> {
    patch_folder_paused(&state, &folder_id, true).await
}

#[tauri::command]
pub async fn resume_syncthing_folder(
    state: tauri::State<'_, SyncthingState>,
    folder_id: String,
) -> Result<OperationResult, String> {
    patch_folder_paused(&state, &folder_id, false).await
}

#[tauri::command]
pub async fn remove_syncthing_folder(
    state: tauri::State<'_, SyncthingState>,
    folder_id: String,
) -> Result<OperationResult, String> {
    let folder_id = normalize_required(&folder_id, "文件夹 ID 不能为空")?;
    wait_for_syncthing_api(&state, Duration::from_secs(10)).await?;
    syncthing_request_empty(
        &state,
        Method::DELETE,
        &["config", "folders", &folder_id],
        &[],
        None,
    )
    .await?;
    operation_result(&state).await
}

#[tauri::command]
pub async fn rescan_syncthing_folder(
    state: tauri::State<'_, SyncthingState>,
    folder_id: String,
) -> Result<OperationResult, String> {
    let folder_id = normalize_required(&folder_id, "文件夹 ID 不能为空")?;
    wait_for_syncthing_api(&state, Duration::from_secs(10)).await?;
    syncthing_request_empty(
        &state,
        Method::POST,
        &["db", "scan"],
        &[("folder", folder_id)],
        None,
    )
    .await?;
    Ok(OperationResult {
        restart_required: false,
    })
}

#[tauri::command]
pub async fn rescan_all_syncthing_folders(
    state: tauri::State<'_, SyncthingState>,
) -> Result<OperationResult, String> {
    wait_for_syncthing_api(&state, Duration::from_secs(10)).await?;
    syncthing_request_empty(&state, Method::POST, &["db", "scan"], &[], None).await?;
    Ok(OperationResult {
        restart_required: false,
    })
}

#[tauri::command]
pub async fn add_syncthing_device(
    state: tauri::State<'_, SyncthingState>,
    request: AddDeviceRequest,
) -> Result<OperationResult, String> {
    let device_id = request.device_id.trim();
    if device_id.is_empty() {
        return Err("设备 ID 不能为空".to_string());
    }

    wait_for_syncthing_api(&state, Duration::from_secs(10)).await?;

    let mut device = syncthing_get(&state, &["config", "defaults", "device"], &[]).await?;
    let mut addresses = request
        .addresses
        .into_iter()
        .map(|address| address.trim().to_string())
        .filter(|address| !address.is_empty())
        .collect::<Vec<_>>();

    if addresses.is_empty() {
        addresses.push("dynamic".to_string());
    }

    device["deviceID"] = json!(device_id);
    device["name"] = json!(request.name.trim());
    device["addresses"] = json!(addresses);
    device["paused"] = json!(false);

    syncthing_request_empty(
        &state,
        Method::POST,
        &["config", "devices"],
        &[],
        Some(device),
    )
    .await?;

    for folder_id in request.folder_ids {
        let folder_id = folder_id.trim().to_string();
        if folder_id.is_empty() {
            continue;
        }

        let mut folder = syncthing_get(&state, &["config", "folders", &folder_id], &[]).await?;
        add_device_to_folder(&mut folder, device_id);
        syncthing_request_empty(
            &state,
            Method::PUT,
            &["config", "folders", &folder_id],
            &[],
            Some(folder),
        )
        .await?;
    }

    operation_result(&state).await
}

#[tauri::command]
pub async fn pause_syncthing_device(
    state: tauri::State<'_, SyncthingState>,
    device_id: String,
) -> Result<OperationResult, String> {
    system_device_action(&state, "pause", Some(device_id)).await
}

#[tauri::command]
pub async fn resume_syncthing_device(
    state: tauri::State<'_, SyncthingState>,
    device_id: String,
) -> Result<OperationResult, String> {
    system_device_action(&state, "resume", Some(device_id)).await
}

#[tauri::command]
pub async fn pause_all_syncthing_devices(
    state: tauri::State<'_, SyncthingState>,
) -> Result<OperationResult, String> {
    system_device_action(&state, "pause", None).await
}

#[tauri::command]
pub async fn resume_all_syncthing_devices(
    state: tauri::State<'_, SyncthingState>,
) -> Result<OperationResult, String> {
    system_device_action(&state, "resume", None).await
}

#[tauri::command]
pub async fn remove_syncthing_device(
    state: tauri::State<'_, SyncthingState>,
    device_id: String,
) -> Result<OperationResult, String> {
    let device_id = normalize_required(&device_id, "设备 ID 不能为空")?;
    wait_for_syncthing_api(&state, Duration::from_secs(10)).await?;

    let mut config = syncthing_get(&state, &["config"], &[]).await?;

    if let Some(devices) = config.get_mut("devices").and_then(Value::as_array_mut) {
        devices.retain(|device| device.get("deviceID").and_then(Value::as_str) != Some(&device_id));
    }

    if let Some(folders) = config.get_mut("folders").and_then(Value::as_array_mut) {
        for folder in folders {
            if let Some(devices) = folder.get_mut("devices").and_then(Value::as_array_mut) {
                devices.retain(|device| device.get("deviceID").and_then(Value::as_str) != Some(&device_id));
            }
        }
    }

    syncthing_request_empty(&state, Method::PUT, &["config"], &[], Some(config)).await?;
    operation_result(&state).await
}

pub async fn setup_syncthing(app: &AppHandle) {
    let state: tauri::State<SyncthingState> = app.state();
    if let Err(error) = start_syncthing_process(&state).await {
        eprintln!("无法启动 Syncthing: {error}");
    }
}

async fn start_syncthing_process(state: &SyncthingState) -> Result<(), String> {
    if is_process_running(state) {
        wait_for_syncthing_api(state, Duration::from_secs(10)).await?;
        return Ok(());
    }

    if detect_existing_syncthing_api(state).await {
        println!("检测到 AeroSync Syncthing API 已在运行，复用现有实例");
        return Ok(());
    }

    if has_existing_aerosync_syncthing_process(state) {
        match wait_for_syncthing_api(state, Duration::from_secs(10)).await {
            Ok(()) => return Ok(()),
            Err(error) => {
                eprintln!("检测到不可用的 AeroSync Syncthing 旧进程，准备清理后重启: {error}");
                kill_existing_aerosync_syncthing_processes(state);
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }

    *state.api_key.lock().unwrap() = None;

    let app_dir = get_app_dir();
    let bin_dir = app_dir.join("bin");
    let config_dir = app_dir.join("config");

    std::fs::create_dir_all(&bin_dir)
        .map_err(|error| format!("无法创建 Syncthing 二进制目录: {error}"))?;
    std::fs::create_dir_all(&config_dir)
        .map_err(|error| format!("无法创建 Syncthing 配置目录: {error}"))?;

    #[cfg(target_os = "windows")]
    let bin_name = "syncthing.exe";
    #[cfg(not(target_os = "windows"))]
    let bin_name = "syncthing";

    let bin_path = bin_dir.join(bin_name);

    if !bin_path.exists() {
        println!("Syncthing 核心不存在，准备下载...");
        download_syncthing(&bin_path)
            .await
            .map_err(|error| format!("下载 Syncthing 失败: {error}"))?;
    }

    println!("正在启动 Syncthing 进程: {:?}", bin_path);
    println!("配置文件目录: {:?}", config_dir);

    let mut command = Command::new(&bin_path);
    command
        .arg(format!("--home={}", config_dir.to_string_lossy()))
        .arg(format!("--gui-address={SYNCTHING_GUI_ADDRESS}"))
        .arg("--no-browser")
        .arg("--no-restart");

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }

    let child = command
        .spawn()
        .map_err(|error| format!("无法启动 Syncthing: {error}"))?;

    println!("Syncthing 已成功启动，PID: {}", child.id());
    *state.process.lock().unwrap() = Some(child);

    wait_for_api_key(state, Duration::from_secs(30)).await?;
    wait_for_syncthing_api(state, Duration::from_secs(30)).await?;

    Ok(())
}

async fn shutdown_syncthing_process(state: &SyncthingState) -> Result<(), String> {
    if !is_process_running(state) && !detect_existing_syncthing_api(state).await {
        *state.api_key.lock().unwrap() = None;
        return Ok(());
    }

    let _ = syncthing_request_empty(state, Method::POST, &["system", "shutdown"], &[], None).await;

    if wait_for_syncthing_process_absent(state, Duration::from_secs(10)).await {
        *state.api_key.lock().unwrap() = None;
        return Ok(());
    }

    if is_process_running(state) {
        kill_syncthing_child(state);
        return Ok(());
    }

    *state.api_key.lock().unwrap() = None;
    Err("Syncthing 仍在运行，但不是 AeroSync 本轮启动的进程，请稍后再试或手动结束旧进程".to_string())
}

async fn patch_folder_paused(
    state: &SyncthingState,
    folder_id: &str,
    paused: bool,
) -> Result<OperationResult, String> {
    let folder_id = normalize_required(folder_id, "文件夹 ID 不能为空")?;
    wait_for_syncthing_api(state, Duration::from_secs(10)).await?;
    syncthing_request_empty(
        state,
        Method::PATCH,
        &["config", "folders", &folder_id],
        &[],
        Some(json!({ "paused": paused })),
    )
    .await?;
    operation_result(state).await
}

async fn system_device_action(
    state: &SyncthingState,
    action: &str,
    device_id: Option<String>,
) -> Result<OperationResult, String> {
    wait_for_syncthing_api(state, Duration::from_secs(10)).await?;

    let query = device_id
        .map(|device_id| normalize_required(&device_id, "设备 ID 不能为空"))
        .transpose()?
        .map(|device_id| vec![("device", device_id)])
        .unwrap_or_default();

    syncthing_request_empty(state, Method::POST, &["system", action], &query, None).await?;
    Ok(OperationResult {
        restart_required: false,
    })
}

async fn operation_result(state: &SyncthingState) -> Result<OperationResult, String> {
    Ok(OperationResult {
        restart_required: get_restart_required(state).await.unwrap_or(false),
    })
}

async fn get_restart_required(state: &SyncthingState) -> Result<bool, String> {
    let value = syncthing_get(state, &["config", "restart-required"], &[]).await?;

    if let Some(requires_restart) = value.as_bool() {
        return Ok(requires_restart);
    }

    Ok(value
        .get("requiresRestart")
        .or_else(|| value.get("restartRequired"))
        .and_then(Value::as_bool)
        .unwrap_or(false))
}

async fn get_my_device_id(state: &SyncthingState) -> Result<String, String> {
    let status = syncthing_get(state, &["system", "status"], &[]).await?;
    Ok(status
        .get("myID")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string())
}

async fn wait_for_api_key(state: &SyncthingState, timeout: Duration) -> Result<String, String> {
    let start = std::time::Instant::now();
    let mut last_error = "Syncthing API Key 尚未就绪".to_string();

    while start.elapsed() < timeout {
        match ensure_api_key(state) {
            Ok(api_key) => return Ok(api_key),
            Err(error) => last_error = error,
        }
        tokio::time::sleep(Duration::from_millis(300)).await;
    }

    Err(last_error)
}

async fn wait_for_syncthing_api(state: &SyncthingState, timeout: Duration) -> Result<(), String> {
    let start = std::time::Instant::now();
    let mut last_error = "Syncthing API 尚未就绪".to_string();

    while start.elapsed() < timeout {
        let process_present = is_process_running(state) || has_existing_aerosync_syncthing_process(state);

        match syncthing_request_empty(state, Method::GET, &["system", "ping"], &[], None).await {
            Ok(()) => return Ok(()),
            Err(error) => {
                if !process_present {
                    return Err("Syncthing 核心未运行".to_string());
                }
                last_error = error;
            }
        }

        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    Err(last_error)
}

fn ensure_api_key(state: &SyncthingState) -> Result<String, String> {
    if let Some(api_key) = state.api_key.lock().unwrap().clone() {
        if !api_key.is_empty() {
            return Ok(api_key);
        }
    }

    let config_file = state.config_dir.join("config.xml");
    let content = std::fs::read_to_string(&config_file)
        .map_err(|error| format!("读取 Syncthing 配置失败: {error}"))?;
    let api_key = extract_api_key(&content).ok_or_else(|| "Syncthing API Key 尚未生成".to_string())?;

    *state.api_key.lock().unwrap() = Some(api_key.clone());
    println!("成功获取到 API Key: {api_key}");
    Ok(api_key)
}

async fn syncthing_get(
    state: &SyncthingState,
    path_segments: &[&str],
    query: &[(&str, String)],
) -> Result<Value, String> {
    syncthing_request_json(state, Method::GET, path_segments, query, None).await
}

async fn syncthing_request_json(
    state: &SyncthingState,
    method: Method,
    path_segments: &[&str],
    query: &[(&str, String)],
    body: Option<Value>,
) -> Result<Value, String> {
    let response = send_syncthing_request(state, method, path_segments, query, body).await?;
    let text = response
        .text()
        .await
        .map_err(|error| format!("读取 Syncthing API 响应失败: {error}"))?;

    if text.trim().is_empty() {
        return Ok(Value::Null);
    }

    serde_json::from_str(&text).map_err(|error| format!("解析 Syncthing API 响应失败: {error}"))
}

async fn syncthing_request_empty(
    state: &SyncthingState,
    method: Method,
    path_segments: &[&str],
    query: &[(&str, String)],
    body: Option<Value>,
) -> Result<(), String> {
    send_syncthing_request(state, method, path_segments, query, body)
        .await
        .map(|_| ())
}

async fn send_syncthing_request(
    state: &SyncthingState,
    method: Method,
    path_segments: &[&str],
    query: &[(&str, String)],
    body: Option<Value>,
) -> Result<reqwest::Response, String> {
    let api_key = ensure_api_key(state)?;
    let url = build_api_url(path_segments, query)?;
    let mut request = syncthing_client()?
        .request(method, url)
        .header("X-API-Key", api_key);

    if let Some(body) = body {
        request = request.json(&body);
    }

    let response = request
        .send()
        .await
        .map_err(|error| format!("请求 Syncthing API 失败: {error}"))?;
    let status = response.status();

    if !status.is_success() {
        let message = response.text().await.unwrap_or_default();
        return Err(if message.trim().is_empty() {
            format!("Syncthing API 返回错误状态: {status}")
        } else {
            format!("Syncthing API 返回错误状态: {status}，{message}")
        });
    }

    Ok(response)
}

fn syncthing_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .no_proxy()
        .build()
        .map_err(|error| format!("创建 Syncthing API 客户端失败: {error}"))
}

fn build_api_url(
    path_segments: &[&str],
    query: &[(&str, String)],
) -> Result<Url, String> {
    let mut url = Url::parse(SYNCTHING_API_URL)
        .map_err(|error| format!("Syncthing API 地址无效: {error}"))?;

    {
        let mut segments = url
            .path_segments_mut()
            .map_err(|_| "Syncthing API 地址不能作为 base URL".to_string())?;
        for segment in path_segments {
            segments.push(segment);
        }
    }

    if !query.is_empty() {
        let mut pairs = url.query_pairs_mut();
        for (key, value) in query {
            pairs.append_pair(key, value);
        }
    }

    Ok(url)
}

fn is_process_running(state: &SyncthingState) -> bool {
    let mut process_guard = state.process.lock().unwrap();

    if let Some(child) = process_guard.as_mut() {
        match child.try_wait() {
            Ok(None) => true,
            Ok(Some(_)) => {
                *process_guard = None;
                *state.api_key.lock().unwrap() = None;
                false
            }
            Err(error) => {
                eprintln!("检查 Syncthing 进程状态失败: {error}");
                false
            }
        }
    } else {
        false
    }
}

async fn wait_for_syncthing_process_absent(state: &SyncthingState, timeout: Duration) -> bool {
    let start = std::time::Instant::now();

    while start.elapsed() < timeout {
        if !is_process_running(state) && !has_existing_aerosync_syncthing_process(state) {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }

    false
}

pub fn kill_syncthing_child(state: &SyncthingState) {
    let mut process_guard = state.process.lock().unwrap();
    if let Some(mut child) = process_guard.take() {
        println!("正在停止 Syncthing 进程...");
        let _ = child.kill();
        let _ = child.wait();
    }
    *state.api_key.lock().unwrap() = None;
}

fn has_existing_aerosync_syncthing_process(state: &SyncthingState) -> bool {
    #[cfg(target_os = "linux")]
    {
        if let Ok(output) = Command::new("pgrep")
            .args(["-f", &format!("syncthing.*--home={}", state.config_dir.to_string_lossy())])
            .output()
        {
            return output.status.success() && !String::from_utf8_lossy(&output.stdout).trim().is_empty();
        }
    }

    false
}

fn kill_existing_aerosync_syncthing_processes(state: &SyncthingState) {
    #[cfg(target_os = "linux")]
    {
        let pattern = format!("syncthing.*--home={}", state.config_dir.to_string_lossy());
        let _ = Command::new("pkill").args(["-f", &pattern]).status();
    }
}

async fn detect_existing_syncthing_api(state: &SyncthingState) -> bool {
    ensure_api_key(state).is_ok()
        && syncthing_request_empty(state, Method::GET, &["system", "ping"], &[], None)
            .await
            .is_ok()
}

fn add_device_to_folder(folder: &mut Value, device_id: &str) {
    if !folder.get("devices").is_some_and(Value::is_array) {
        folder["devices"] = json!([]);
    }

    let devices = folder
        .get_mut("devices")
        .and_then(Value::as_array_mut)
        .expect("devices was initialized as an array");

    let exists = devices
        .iter()
        .any(|device| device.get("deviceID").and_then(Value::as_str) == Some(device_id));

    if !exists {
        devices.push(json!({ "deviceID": device_id }));
    }
}

fn push_unique(values: &mut Vec<String>, value: String) {
    let value = value.trim().to_string();
    if !value.is_empty() && !values.iter().any(|existing| existing == &value) {
        values.push(value);
    }
}

fn normalize_required(value: &str, message: &str) -> Result<String, String> {
    let value = value.trim().to_string();
    if value.is_empty() {
        Err(message.to_string())
    } else {
        Ok(value)
    }
}

fn empty_overview(running: bool, ready: bool, error: Option<String>) -> SyncthingOverview {
    SyncthingOverview {
        running,
        ready,
        config: json!({ "folders": [], "devices": [] }),
        system_status: json!({}),
        connections: json!({ "connections": {} }),
        restart_required: false,
        error,
    }
}

fn open_url(url: &str) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut command = Command::new("cmd");
        command.args(["/C", "start", "", url]);
        command
    };

    #[cfg(target_os = "macos")]
    let mut command = {
        let mut command = Command::new("open");
        command.arg(url);
        command
    };

    #[cfg(all(unix, not(target_os = "macos")))]
    let mut command = {
        let mut command = Command::new("xdg-open");
        command.arg(url);
        command
    };

    command
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("打开 Syncthing Web UI 失败: {error}"))
}

fn extract_api_key(config_xml: &str) -> Option<String> {
    let start = config_xml.find("<apikey>")? + "<apikey>".len();
    let end = config_xml[start..].find("</apikey>")? + start;
    Some(config_xml[start..end].to_string())
}
