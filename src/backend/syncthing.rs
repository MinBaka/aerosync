use super::downloader::{download_syncthing, get_app_dir};
use super::models::{
    AddDeviceRequest, AddFolderRequest, DeviceCompletion, FolderStatus, LogEntry, OperationResult,
    SyncthingConfig, SyncthingConnections, SyncthingOverview, SyncthingSystemStatus,
};
use anyhow::{anyhow, bail, Context, Result};
use reqwest::{Method, StatusCode, Url};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::process::{Child, Command};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use sysinfo::{ProcessesToUpdate, Signal, System};

const SYNCTHING_API_URL: &str = "http://127.0.0.1:58384/rest";
const SYNCTHING_GUI_CONFIG_ADDRESS: &str = "127.0.0.1:58384";
const SYNCTHING_GUI_CLI_ADDRESS: &str = "http://127.0.0.1:58384";

pub struct SyncthingService {
    process: Mutex<Option<Child>>,
    api_key: Mutex<Option<String>>,
    bin_dir: PathBuf,
    config_dir: PathBuf,
}

impl SyncthingService {
    pub fn new() -> Self {
        let app_dir = get_app_dir();
        Self {
            process: Mutex::new(None),
            api_key: Mutex::new(None),
            bin_dir: app_dir.join("bin"),
            config_dir: app_dir.join("config"),
        }
    }

    pub async fn setup(&self) -> Result<()> {
        self.start().await
    }

    pub async fn overview(&self) -> Result<SyncthingOverview> {
        let running = self.is_child_running();
        let is_downloaded = self.is_core_downloaded();
        if !running
            && !self.has_existing_aerosync_syncthing_process()
            && !self.detect_existing_syncthing_api().await
        {
            return Ok(empty_overview(is_downloaded, false, false, None));
        }

        if let Err(error) = self.wait_for_syncthing_api(Duration::from_secs(10)).await {
            return Ok(empty_overview(
                is_downloaded,
                true,
                false,
                Some(error.to_string()),
            ));
        }

        let start = Instant::now();
        let mut last_error = anyhow!("Syncthing API 数据尚未就绪");
        while start.elapsed() < Duration::from_secs(10) {
            match self.fetch_ready_overview(is_downloaded).await {
                Ok(overview) => return Ok(overview),
                Err(error) => last_error = error,
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }

        Ok(empty_overview(
            is_downloaded,
            true,
            false,
            Some(last_error.to_string()),
        ))
    }

    async fn fetch_ready_overview(&self, is_downloaded: bool) -> Result<SyncthingOverview> {
        let config =
            serde_json::from_value::<SyncthingConfig>(self.syncthing_get(&["config"], &[]).await?)
                .unwrap_or_default();
        let mut system_status = serde_json::from_value::<SyncthingSystemStatus>(
            self.syncthing_get(&["system", "status"], &[]).await?,
        )
        .unwrap_or_default();

        // Ensure my_id is populated even if parsing fails for some reason
        if system_status.my_id.is_empty() {
            system_status.my_id = self.get_my_device_id().await.unwrap_or_default();
        }

        let connections = serde_json::from_value::<SyncthingConnections>(
            self.syncthing_get(&["system", "connections"], &[]).await?,
        )
        .unwrap_or_default();
        let mut folder_statuses = std::collections::HashMap::new();
        for folder in &config.folders {
            let status = serde_json::from_value::<FolderStatus>(
                self.syncthing_get(&["db", "status"], &[("folder", folder.id.clone())])
                    .await
                    .unwrap_or_else(|_| json!({})),
            )
            .unwrap_or_default();
            folder_statuses.insert(folder.id.clone(), status);
        }

        let mut device_completions = std::collections::HashMap::new();
        let my_id = system_status.my_id.clone();
        for device in &config.devices {
            if device.device_id == my_id {
                continue;
            }
            let completion = serde_json::from_value::<DeviceCompletion>(
                self.syncthing_get(
                    &["db", "completion"],
                    &[("device", device.device_id.clone())],
                )
                .await
                .unwrap_or_else(|_| json!({})),
            )
            .unwrap_or_default();
            device_completions.insert(device.device_id.clone(), completion);
        }

        let pending_devices = self.get_pending_devices().await.unwrap_or_default();
        let pending_folders = self.get_pending_folders().await.unwrap_or_default();

        let upgrade_status = self.syncthing_get(&["system", "upgrade"], &[]).await
            .ok()
            .and_then(|v| serde_json::from_value::<crate::backend::models::SystemUpgradeStatus>(v).ok());

        let restart_required = self.get_restart_required().await.unwrap_or(false);

        Ok(SyncthingOverview {
            is_downloaded,
            running: true,
            ready: true,
            config,
            system_status,
            connections,
            folder_statuses,
            device_completions,
            restart_required,
            error: None,
            pending_devices,
            pending_folders,
            upgrade_status,
        })
    }

    pub async fn download_core<F>(&self, progress_callback: F) -> Result<()>
    where
        F: FnMut(f32) + Send + 'static,
    {
        #[cfg(target_os = "windows")]
        let bin_name = "syncthing.exe";
        #[cfg(not(target_os = "windows"))]
        let bin_name = "syncthing";

        let bin_path = self.bin_dir.join(bin_name);
        if !bin_path.exists() {
            println!("Syncthing 核心不存在，准备下载...");
            download_syncthing(&bin_path, progress_callback)
                .await
                .map_err(|error| anyhow!("下载 Syncthing 失败: {error}"))?;
        }
        Ok(())
    }

    pub fn is_core_downloaded(&self) -> bool {
        #[cfg(target_os = "windows")]
        let bin_name = "syncthing.exe";
        #[cfg(not(target_os = "windows"))]
        let bin_name = "syncthing";

        self.bin_dir.join(bin_name).exists()
    }

    pub async fn start(&self) -> Result<()> {
        if self.is_child_running() {
            self.wait_for_syncthing_api(Duration::from_secs(15)).await?;
            return Ok(());
        }

        if self.detect_existing_syncthing_api().await {
            println!("检测到 AeroSync Syncthing API 已在运行，复用现有实例");
            return Ok(());
        }

        if self.has_existing_aerosync_syncthing_process() {
            match self.wait_for_syncthing_api(Duration::from_secs(8)).await {
                Ok(()) => return Ok(()),
                Err(error) => {
                    eprintln!("检测到不可用的 AeroSync Syncthing 旧进程，准备清理后重启: {error}");
                    self.kill_existing_aerosync_syncthing_processes();
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            }
        }

        *self.api_key.lock().unwrap() = None;
        std::fs::create_dir_all(&self.bin_dir).context("无法创建 Syncthing 二进制目录")?;
        std::fs::create_dir_all(&self.config_dir).context("无法创建 Syncthing 配置目录")?;
        self.ensure_gui_address()?;

        #[cfg(target_os = "windows")]
        let bin_name = "syncthing.exe";
        #[cfg(not(target_os = "windows"))]
        let bin_name = "syncthing";

        let bin_path = self.bin_dir.join(bin_name);
        if !bin_path.exists() {
            bail!("Syncthing 核心尚未下载");
        }

        println!("正在启动 Syncthing 进程: {:?}", bin_path);
        println!("配置文件目录: {:?}", self.config_dir);

        let mut command = Command::new(&bin_path);
        command
            .arg("serve")
            .arg(format!("--home={}", self.config_dir.to_string_lossy()))
            .arg(format!("--gui-address={SYNCTHING_GUI_CLI_ADDRESS}"))
            .arg("--skip-port-probing")
            .arg("--no-browser");

        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }

        let child = command.spawn().context("无法启动 Syncthing")?;
        println!("Syncthing 已成功启动，PID: {}", child.id());
        *self.process.lock().unwrap() = Some(child);

        self.wait_for_api_key(Duration::from_secs(60)).await?;
        self.wait_for_syncthing_api(Duration::from_secs(20)).await?;
        Ok(())
    }

    pub async fn shutdown(&self) -> Result<()> {
        if !self.is_child_running() && !self.detect_existing_syncthing_api().await {
            *self.api_key.lock().unwrap() = None;
            return Ok(());
        }

        let _ = self
            .syncthing_request_empty(Method::POST, &["system", "shutdown"], &[], None)
            .await;

        if self
            .wait_for_syncthing_process_absent(Duration::from_secs(10))
            .await
        {
            *self.api_key.lock().unwrap() = None;
            return Ok(());
        }

        if self.is_child_running() {
            self.kill_owned_child();
            return Ok(());
        }

        *self.api_key.lock().unwrap() = None;
        bail!("Syncthing 仍在运行，但不是 AeroSync 本轮启动的进程，请稍后再试或手动结束旧进程")
    }

    pub async fn set_rate_limits(&self, recv_kbps: i64, send_kbps: i64) -> Result<OperationResult> {
        self.wait_for_syncthing_api(Duration::from_secs(10)).await?;
        self.syncthing_request_empty(
            Method::PATCH,
            &["config", "options"],
            &[],
            Some(json!({ "maxRecvKbps": recv_kbps, "maxSendKbps": send_kbps })),
        )
        .await?;
        self.operation_result().await
    }

    pub async fn restart(&self) -> Result<()> {
        if !self.is_child_running() && !self.detect_existing_syncthing_api().await {
            return self.start().await;
        }

        let _ = self
            .syncthing_request_empty(Method::POST, &["system", "restart"], &[], None)
            .await;

        tokio::time::sleep(Duration::from_secs(2)).await;
        self.wait_for_syncthing_api(Duration::from_secs(30)).await?;
        Ok(())
    }

    pub async fn check_upgrade(&self) -> Result<OperationResult> {
        self.wait_for_syncthing_api(Duration::from_secs(10)).await?;
        self.syncthing_request_empty(Method::GET, &["system", "upgrade"], &[], None).await?;
        self.operation_result().await
    }

    pub async fn perform_upgrade(&self) -> Result<OperationResult> {
        self.wait_for_syncthing_api(Duration::from_secs(10)).await?;
        let _ = self.syncthing_request_empty(Method::POST, &["system", "upgrade"], &[], None).await;

        tokio::time::sleep(Duration::from_secs(5)).await;
        self.wait_for_syncthing_api(Duration::from_secs(60)).await?;
        Ok(OperationResult { restart_required: false })
    }

    pub async fn set_auto_upgrade(&self, enabled: bool) -> Result<OperationResult> {
        self.wait_for_syncthing_api(Duration::from_secs(10)).await?;
        let interval = if enabled { 24 } else { 0 };
        self.syncthing_request_empty(
            Method::PATCH,
            &["config", "options"],
            &[],
            Some(json!({ "autoUpgradeIntervalH": interval })),
        )
        .await?;
        self.operation_result().await
    }

    pub fn kill_owned_child(&self) {
        let mut process_guard = self.process.lock().unwrap();
        if let Some(mut child) = process_guard.take() {
            println!("正在停止 Syncthing 进程...");
            let _ = child.kill();
            let _ = child.wait();
        }
        *self.api_key.lock().unwrap() = None;
    }

    pub async fn add_folder(&self, request: AddFolderRequest) -> Result<OperationResult> {
        let folder_id = normalize_required(&request.id, "文件夹 ID 不能为空")?;
        let path = normalize_required(&request.path, "本地路径不能为空")?;

        self.wait_for_syncthing_api(Duration::from_secs(10)).await?;

        let mut folder = self
            .syncthing_get(&["config", "defaults", "folder"], &[])
            .await?;
        let my_id = self.get_my_device_id().await.unwrap_or_default();
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
        folder["devices"] = json!(device_ids
            .into_iter()
            .map(|device_id| json!({ "deviceID": device_id }))
            .collect::<Vec<_>>());

        // Apply advanced settings
        if let Some(folder_type) = request.folder_type {
            folder["type"] = json!(folder_type);
        }
        if let Some(rescan_interval) = request.rescan_interval_s {
            folder["rescanIntervalS"] = json!(rescan_interval);
        }
        if let Some(fs_watcher) = request.fs_watcher_enabled {
            folder["fsWatcherEnabled"] = json!(fs_watcher);
        }
        if let Some(ignore_perms) = request.ignore_perms {
            folder["ignorePerms"] = json!(ignore_perms);
        }
        if let Some(ignore_delete) = request.ignore_delete {
            folder["ignoreDelete"] = json!(ignore_delete);
        }
        if let Some(versioning) = request.versioning {
            folder["versioning"] = json!({
                "type": versioning.versioning_type,
                "params": versioning.params,
            });
        }

        self.syncthing_request_empty(Method::POST, &["config", "folders"], &[], Some(folder))
            .await?;

        self.operation_result().await
    }

    pub async fn get_folder_ignores(&self, folder_id: &str) -> Result<String> {
        let folder_id = normalize_required(folder_id, "文件夹 ID 不能为空")?;
        self.wait_for_syncthing_api(Duration::from_secs(10)).await?;
        let value = self
            .syncthing_get(&["db", "ignores"], &[("folder", folder_id)])
            .await?;

        let ignore_lines = value
            .get("ignore")
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default();

        Ok(ignore_lines)
    }

    pub async fn set_folder_ignores(
        &self,
        folder_id: &str,
        ignores: &str,
    ) -> Result<OperationResult> {
        let folder_id = normalize_required(folder_id, "文件夹 ID 不能为空")?;
        self.wait_for_syncthing_api(Duration::from_secs(10)).await?;

        let lines: Vec<String> = ignores
            .lines()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        self.syncthing_request_empty(
            Method::POST,
            &["db", "ignores"],
            &[("folder", folder_id)],
            Some(json!({ "ignore": lines })),
        )
        .await?;

        self.operation_result().await
    }

    pub async fn edit_folder(&self, request: AddFolderRequest) -> Result<OperationResult> {
        let folder_id = normalize_required(&request.id, "文件夹 ID 不能为空")?;
        self.wait_for_syncthing_api(Duration::from_secs(10)).await?;

        let mut folder = self
            .syncthing_get(&["config", "folders", &folder_id], &[])
            .await?;

        let my_id = self.get_my_device_id().await.unwrap_or_default();
        let mut device_ids = Vec::new();

        if !my_id.is_empty() {
            push_unique(&mut device_ids, my_id);
        }
        for device_id in request.device_ids {
            push_unique(&mut device_ids, device_id);
        }

        folder["label"] = json!(request.label.trim());
        folder["devices"] = json!(device_ids
            .into_iter()
            .map(|device_id| json!({ "deviceID": device_id }))
            .collect::<Vec<_>>());

        // Apply advanced settings
        if let Some(folder_type) = request.folder_type {
            folder["type"] = json!(folder_type);
        }
        if let Some(rescan_interval) = request.rescan_interval_s {
            folder["rescanIntervalS"] = json!(rescan_interval);
        }
        if let Some(fs_watcher) = request.fs_watcher_enabled {
            folder["fsWatcherEnabled"] = json!(fs_watcher);
        }
        if let Some(ignore_perms) = request.ignore_perms {
            folder["ignorePerms"] = json!(ignore_perms);
        }
        if let Some(ignore_delete) = request.ignore_delete {
            folder["ignoreDelete"] = json!(ignore_delete);
        }
        if let Some(versioning) = request.versioning {
            folder["versioning"] = json!({
                "type": versioning.versioning_type,
                "params": versioning.params,
            });
        } else {
            // If versioning is None, disable it
            folder["versioning"] = json!({
                "type": "",
                "params": {},
            });
        }

        self.syncthing_request_empty(
            Method::PUT,
            &["config", "folders", &folder_id],
            &[],
            Some(folder),
        )
        .await?;

        self.operation_result().await
    }

    pub async fn pause_folder(&self, folder_id: &str) -> Result<OperationResult> {
        self.patch_folder_paused(folder_id, true).await
    }

    pub async fn resume_folder(&self, folder_id: &str) -> Result<OperationResult> {
        self.patch_folder_paused(folder_id, false).await
    }

    pub async fn remove_folder(&self, folder_id: &str) -> Result<OperationResult> {
        let folder_id = normalize_required(folder_id, "文件夹 ID 不能为空")?;
        self.wait_for_syncthing_api(Duration::from_secs(10)).await?;
        self.syncthing_request_empty(
            Method::DELETE,
            &["config", "folders", &folder_id],
            &[],
            None,
        )
        .await?;
        self.operation_result().await
    }

    pub async fn delete_folder_files(&self, folder_id: &str) -> Result<()> {
        let folder_id = normalize_required(folder_id, "文件夹 ID 不能为空")?;
        self.wait_for_syncthing_api(Duration::from_secs(10)).await?;

        // 获取文件夹配置以得到路径
        let folder = self
            .syncthing_get(&["config", "folders", &folder_id], &[])
            .await?;

        let path = folder["path"]
            .as_str()
            .context("文件夹配置中没有 path 字段")?;

        // 删除文件夹
        std::fs::remove_dir_all(path)
            .with_context(|| format!("删除文件夹失败: {}", path))?;

        Ok(())
    }

    pub async fn rescan_folder(&self, folder_id: &str) -> Result<OperationResult> {
        let folder_id = normalize_required(folder_id, "文件夹 ID 不能为空")?;
        self.wait_for_syncthing_api(Duration::from_secs(10)).await?;
        self.syncthing_request_empty(
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

    pub async fn rescan_all_folders(&self) -> Result<OperationResult> {
        self.wait_for_syncthing_api(Duration::from_secs(10)).await?;
        self.syncthing_request_empty(Method::POST, &["db", "scan"], &[], None)
            .await?;
        Ok(OperationResult {
            restart_required: false,
        })
    }

    pub async fn add_device(&self, request: AddDeviceRequest) -> Result<OperationResult> {
        let device_id = normalize_required(&request.device_id, "设备 ID 不能为空")?;
        self.wait_for_syncthing_api(Duration::from_secs(10)).await?;

        let mut device = self
            .syncthing_get(&["config", "defaults", "device"], &[])
            .await?;
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

        // Apply advanced settings
        if let Some(introducer) = request.introducer {
            device["introducer"] = json!(introducer);
        }
        if let Some(auto_accept) = request.auto_accept_folders {
            device["autoAcceptFolders"] = json!(auto_accept);
        }
        if let Some(compression) = &request.compression {
            device["compression"] = json!(compression);
        }
        if let Some(max_send) = request.max_send_kbps {
            device["maxSendKbps"] = json!(max_send);
        }
        if let Some(max_recv) = request.max_recv_kbps {
            device["maxRecvKbps"] = json!(max_recv);
        }

        self.syncthing_request_empty(Method::POST, &["config", "devices"], &[], Some(device))
            .await?;

        for folder_id in request.folder_ids {
            let folder_id = folder_id.trim().to_string();
            if folder_id.is_empty() {
                continue;
            }

            let mut folder = self
                .syncthing_get(&["config", "folders", &folder_id], &[])
                .await?;
            add_device_to_folder(&mut folder, &device_id);
            self.syncthing_request_empty(
                Method::PUT,
                &["config", "folders", &folder_id],
                &[],
                Some(folder),
            )
            .await?;
        }

        self.operation_result().await
    }

    pub async fn edit_device(&self, request: AddDeviceRequest) -> Result<OperationResult> {
        let device_id = normalize_required(&request.device_id, "设备 ID 不能为空")?;
        self.wait_for_syncthing_api(Duration::from_secs(10)).await?;

        let mut config = self.syncthing_get(&["config"], &[]).await?;

        let mut addresses = request
            .addresses
            .into_iter()
            .map(|address| address.trim().to_string())
            .filter(|address| !address.is_empty())
            .collect::<Vec<_>>();

        if addresses.is_empty() {
            addresses.push("dynamic".to_string());
        }

        if let Some(devices) = config.get_mut("devices").and_then(Value::as_array_mut) {
            for device in devices {
                if device.get("deviceID").and_then(Value::as_str) == Some(&device_id) {
                    device["name"] = json!(request.name.trim());
                    device["addresses"] = json!(addresses);

                    // Apply advanced settings
                    if let Some(introducer) = request.introducer {
                        device["introducer"] = json!(introducer);
                    }
                    if let Some(auto_accept) = request.auto_accept_folders {
                        device["autoAcceptFolders"] = json!(auto_accept);
                    }
                    if let Some(compression) = &request.compression {
                        device["compression"] = json!(compression);
                    }
                    if let Some(max_send) = request.max_send_kbps {
                        device["maxSendKbps"] = json!(max_send);
                    }
                    if let Some(max_recv) = request.max_recv_kbps {
                        device["maxRecvKbps"] = json!(max_recv);
                    }
                }
            }
        }

        if let Some(folders) = config.get_mut("folders").and_then(Value::as_array_mut) {
            for folder in folders {
                let folder_id_str = folder.get("id").and_then(Value::as_str).unwrap_or_default();
                let should_share = request.folder_ids.iter().any(|id| id == folder_id_str);

                if let Some(devices) = folder.get_mut("devices").and_then(Value::as_array_mut) {
                    let has_device = devices
                        .iter()
                        .any(|d| d.get("deviceID").and_then(Value::as_str) == Some(&device_id));

                    if should_share && !has_device {
                        devices.push(json!({ "deviceID": device_id }));
                    } else if !should_share && has_device {
                        devices.retain(|d| {
                            d.get("deviceID").and_then(Value::as_str) != Some(&device_id)
                        });
                    }
                }
            }
        }

        self.syncthing_request_empty(Method::PUT, &["config"], &[], Some(config))
            .await?;

        self.operation_result().await
    }

    pub async fn pause_device(&self, device_id: &str) -> Result<OperationResult> {
        self.system_device_action("pause", Some(device_id.to_string()))
            .await
    }

    pub async fn resume_device(&self, device_id: &str) -> Result<OperationResult> {
        self.system_device_action("resume", Some(device_id.to_string()))
            .await
    }

    pub async fn pause_all_devices(&self) -> Result<OperationResult> {
        self.system_device_action("pause", None).await
    }

    pub async fn resume_all_devices(&self) -> Result<OperationResult> {
        self.system_device_action("resume", None).await
    }

    pub async fn remove_device(&self, device_id: &str) -> Result<OperationResult> {
        let device_id = normalize_required(device_id, "设备 ID 不能为空")?;
        self.wait_for_syncthing_api(Duration::from_secs(10)).await?;

        let mut config = self.syncthing_get(&["config"], &[]).await?;

        if let Some(devices) = config.get_mut("devices").and_then(Value::as_array_mut) {
            devices.retain(|device| {
                device.get("deviceID").and_then(Value::as_str) != Some(&device_id)
            });
        }

        if let Some(folders) = config.get_mut("folders").and_then(Value::as_array_mut) {
            for folder in folders {
                if let Some(devices) = folder.get_mut("devices").and_then(Value::as_array_mut) {
                    devices.retain(|device| {
                        device.get("deviceID").and_then(Value::as_str) != Some(&device_id)
                    });
                }
            }
        }

        self.syncthing_request_empty(Method::PUT, &["config"], &[], Some(config))
            .await?;
        self.operation_result().await
    }

    async fn patch_folder_paused(&self, folder_id: &str, paused: bool) -> Result<OperationResult> {
        let folder_id = normalize_required(folder_id, "文件夹 ID 不能为空")?;
        self.wait_for_syncthing_api(Duration::from_secs(10)).await?;
        self.syncthing_request_empty(
            Method::PATCH,
            &["config", "folders", &folder_id],
            &[],
            Some(json!({ "paused": paused })),
        )
        .await?;
        self.operation_result().await
    }

    async fn system_device_action(
        &self,
        action: &str,
        device_id: Option<String>,
    ) -> Result<OperationResult> {
        self.wait_for_syncthing_api(Duration::from_secs(10)).await?;

        let query = device_id
            .map(|device_id| normalize_required(&device_id, "设备 ID 不能为空"))
            .transpose()?
            .map(|device_id| vec![("device", device_id)])
            .unwrap_or_default();

        self.syncthing_request_empty(Method::POST, &["system", action], &query, None)
            .await?;
        Ok(OperationResult {
            restart_required: false,
        })
    }

    async fn operation_result(&self) -> Result<OperationResult> {
        Ok(OperationResult {
            restart_required: self.get_restart_required().await.unwrap_or(false),
        })
    }

    async fn get_restart_required(&self) -> Result<bool> {
        let value = self
            .syncthing_get(&["config", "restart-required"], &[])
            .await?;

        if let Some(requires_restart) = value.as_bool() {
            return Ok(requires_restart);
        }

        Ok(value
            .get("requiresRestart")
            .or_else(|| value.get("restartRequired"))
            .and_then(Value::as_bool)
            .unwrap_or(false))
    }

    async fn get_my_device_id(&self) -> Result<String> {
        let status = self.syncthing_get(&["system", "status"], &[]).await?;
        Ok(status
            .get("myID")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string())
    }

    async fn wait_for_api_key(&self, timeout: Duration) -> Result<String> {
        let start = Instant::now();
        let mut last_error = anyhow!("Syncthing API Key 尚未就绪");

        while start.elapsed() < timeout {
            match self.ensure_api_key() {
                Ok(api_key) => return Ok(api_key),
                Err(error) => last_error = error,
            }
            tokio::time::sleep(Duration::from_millis(300)).await;
        }

        Err(last_error)
    }

    async fn wait_for_syncthing_api(&self, timeout: Duration) -> Result<()> {
        let start = Instant::now();
        let mut last_error = anyhow!("Syncthing API 尚未就绪");

        while start.elapsed() < timeout {
            let process_present =
                self.is_child_running() || self.has_existing_aerosync_syncthing_process();

            match self
                .syncthing_request_empty(Method::GET, &["system", "ping"], &[], None)
                .await
            {
                Ok(()) => return Ok(()),
                Err(error) => {
                    if !process_present {
                        bail!("Syncthing 核心未运行");
                    }
                    last_error = error;
                }
            }

            tokio::time::sleep(Duration::from_millis(500)).await;
        }

        Err(last_error)
    }

    async fn wait_for_syncthing_process_absent(&self, timeout: Duration) -> bool {
        let start = Instant::now();

        while start.elapsed() < timeout {
            if !self.is_child_running() && !self.has_existing_aerosync_syncthing_process() {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }

        false
    }

    fn ensure_gui_address(&self) -> Result<()> {
        let config_file = self.config_dir.join("config.xml");
        let Ok(content) = std::fs::read_to_string(&config_file) else {
            return Ok(());
        };

        let Some(gui_start) = content.find("<gui") else {
            return Ok(());
        };
        let Some(gui_end) = content[gui_start..]
            .find("</gui>")
            .map(|offset| gui_start + offset)
        else {
            return Ok(());
        };
        let gui_section = &content[gui_start..gui_end];

        let updated = if let Some(address_open_rel) = gui_section.find("<address>") {
            let value_start = gui_start + address_open_rel + "<address>".len();
            let Some(address_close_rel) = content[value_start..gui_end].find("</address>") else {
                return Ok(());
            };
            let value_end = value_start + address_close_rel;
            if content[value_start..value_end].trim() == SYNCTHING_GUI_CONFIG_ADDRESS {
                return Ok(());
            }

            let mut updated = String::with_capacity(content.len());
            updated.push_str(&content[..value_start]);
            updated.push_str(SYNCTHING_GUI_CONFIG_ADDRESS);
            updated.push_str(&content[value_end..]);
            updated
        } else {
            let Some(open_end_rel) = gui_section.find('>') else {
                return Ok(());
            };
            let insert_at = gui_start + open_end_rel + 1;
            let mut updated = String::with_capacity(content.len() + 40);
            updated.push_str(&content[..insert_at]);
            updated.push_str("\n        <address>");
            updated.push_str(SYNCTHING_GUI_CONFIG_ADDRESS);
            updated.push_str("</address>");
            updated.push_str(&content[insert_at..]);
            updated
        };

        std::fs::write(&config_file, updated)
            .with_context(|| format!("更新 Syncthing GUI 地址失败: {}", config_file.display()))?;
        println!("已设置 Syncthing GUI 地址: {SYNCTHING_GUI_CONFIG_ADDRESS}");
        Ok(())
    }

    fn ensure_api_key(&self) -> Result<String> {
        if let Some(api_key) = self.api_key.lock().unwrap().clone() {
            if !api_key.is_empty() {
                return Ok(api_key);
            }
        }

        let config_file = self.config_dir.join("config.xml");
        let content = std::fs::read_to_string(&config_file)
            .with_context(|| format!("读取 Syncthing 配置失败: {}", config_file.display()))?;
        let api_key =
            extract_api_key(&content).ok_or_else(|| anyhow!("Syncthing API Key 尚未生成"))?;

        *self.api_key.lock().unwrap() = Some(api_key.clone());
        println!("成功获取到 API Key: {api_key}");
        Ok(api_key)
    }

    async fn syncthing_get(
        &self,
        path_segments: &[&str],
        query: &[(&str, String)],
    ) -> Result<Value> {
        self.syncthing_request_json(Method::GET, path_segments, query, None)
            .await
    }

    async fn syncthing_request_json(
        &self,
        method: Method,
        path_segments: &[&str],
        query: &[(&str, String)],
        body: Option<Value>,
    ) -> Result<Value> {
        let response = self
            .send_syncthing_request(method, path_segments, query, body)
            .await?;
        let text = response
            .text()
            .await
            .context("读取 Syncthing API 响应失败")?;

        if text.trim().is_empty() {
            return Ok(Value::Null);
        }

        serde_json::from_str(&text).context("解析 Syncthing API 响应失败")
    }

    async fn syncthing_request_empty(
        &self,
        method: Method,
        path_segments: &[&str],
        query: &[(&str, String)],
        body: Option<Value>,
    ) -> Result<()> {
        self.send_syncthing_request(method, path_segments, query, body)
            .await
            .map(|_| ())
    }

    async fn send_syncthing_request(
        &self,
        method: Method,
        path_segments: &[&str],
        query: &[(&str, String)],
        body: Option<Value>,
    ) -> Result<reqwest::Response> {
        let api_key = self.ensure_api_key()?;
        let url = build_api_url(path_segments, query)?;
        let mut request = syncthing_client()?
            .request(method, url)
            .header("X-API-Key", api_key);

        if let Some(body) = body {
            request = request.json(&body);
        }

        let response = request.send().await.context("请求 Syncthing API 失败")?;
        let status = response.status();

        if !status.is_success() {
            let message = response.text().await.unwrap_or_default();
            if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
                *self.api_key.lock().unwrap() = None;
            }
            if message.trim().is_empty() {
                bail!("Syncthing API 返回错误状态: {status}");
            }
            bail!("Syncthing API 返回错误状态: {status}，{message}");
        }

        Ok(response)
    }

    fn is_child_running(&self) -> bool {
        let mut process_guard = self.process.lock().unwrap();

        if let Some(child) = process_guard.as_mut() {
            match child.try_wait() {
                Ok(None) => true,
                Ok(Some(_)) => {
                    *process_guard = None;
                    *self.api_key.lock().unwrap() = None;
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

    fn has_existing_aerosync_syncthing_process(&self) -> bool {
        let mut system = System::new_all();
        system.refresh_processes(ProcessesToUpdate::All, true);
        system
            .processes()
            .values()
            .any(|process| self.is_aerosync_syncthing_process(process))
    }

    fn kill_existing_aerosync_syncthing_processes(&self) {
        let mut system = System::new_all();
        system.refresh_processes(ProcessesToUpdate::All, true);
        for process in system.processes().values() {
            if self.is_aerosync_syncthing_process(process) {
                let _ = process.kill_with(Signal::Kill);
            }
        }
    }

    fn is_aerosync_syncthing_process(&self, process: &sysinfo::Process) -> bool {
        let config_dir = self.config_dir.to_string_lossy();
        let home_arg = format!("--home={config_dir}");
        let cmdline = process
            .cmd()
            .iter()
            .map(|part| part.to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join(" ");
        let name = process.name().to_string_lossy().to_ascii_lowercase();
        let cmdline_lower = cmdline.to_ascii_lowercase();

        (name.contains("syncthing") || cmdline_lower.contains("syncthing"))
            && (cmdline.contains(&home_arg) || cmdline.contains(config_dir.as_ref()))
    }

    async fn detect_existing_syncthing_api(&self) -> bool {
        self.ensure_api_key().is_ok()
            && self
                .syncthing_request_empty(Method::GET, &["system", "ping"], &[], None)
                .await
                .is_ok()
    }

    pub async fn get_logs(&self, since: Option<i64>) -> Result<Vec<LogEntry>> {
        self.wait_for_syncthing_api(Duration::from_secs(10)).await?;

        let params: Vec<(&str, String)> = if let Some(since_val) = since {
            vec![("since", since_val.to_string())]
        } else {
            vec![]
        };

        let value = self
            .syncthing_get(&["system", "log"], &params)
            .await?;

        let messages = value["messages"]
            .as_array()
            .context("日志响应中没有 messages 数组")?;

        let logs: Vec<LogEntry> = messages
            .iter()
            .filter_map(|msg| {
                Some(LogEntry {
                    when: msg["when"].as_str()?.to_string(),
                    message: msg["message"].as_str()?.to_string(),
                    level: msg["level"].as_str().unwrap_or("info").to_string(),
                })
            })
            .collect();

        Ok(logs)
    }

    async fn get_pending_devices(&self) -> Result<std::collections::HashMap<String, crate::backend::models::PendingDevice>> {
        let value = self.syncthing_get(&["cluster", "pending", "devices"], &[]).await?;
        serde_json::from_value(value).map_err(Into::into)
    }

    async fn get_pending_folders(&self) -> Result<std::collections::HashMap<String, crate::backend::models::PendingFolder>> {
        let value = self.syncthing_get(&["cluster", "pending", "folders"], &[]).await?;
        serde_json::from_value(value).map_err(Into::into)
    }

    pub async fn accept_device(&self, device_id: &str) -> Result<OperationResult> {
        let device_id = normalize_required(device_id, "设备 ID 不能为空")?;
        self.wait_for_syncthing_api(Duration::from_secs(10)).await?;

        let pending = self.get_pending_devices().await?;
        let device = pending.get(&device_id).context("找不到该待处理设备")?;

        let request = AddDeviceRequest {
            device_id: device_id.clone(),
            name: device.name.clone(),
            addresses: vec!["dynamic".to_string()],
            folder_ids: vec![],
            introducer: Some(false),
            auto_accept_folders: Some(false),
            max_send_kbps: None,
            max_recv_kbps: None,
            compression: Some("metadata".to_string()),
        };

        self.add_device(request).await?;

        // 移除待处理状态
        self.syncthing_request_empty(
            Method::DELETE,
            &["cluster", "pending", "devices"],
            &[("device", device_id)],
            None,
        ).await?;

        Ok(OperationResult { restart_required: false })
    }

    pub async fn ignore_device(&self, device_id: &str) -> Result<OperationResult> {
        let device_id = normalize_required(device_id, "设备 ID 不能为空")?;
        self.wait_for_syncthing_api(Duration::from_secs(10)).await?;

        self.syncthing_request_empty(
            Method::DELETE,
            &["cluster", "pending", "devices"],
            &[("device", device_id)],
            None,
        ).await?;

        Ok(OperationResult { restart_required: false })
    }

    pub async fn accept_folder(&self, folder_id: &str, device_id: &str, label: &str) -> Result<OperationResult> {
        let folder_id = normalize_required(folder_id, "文件夹 ID 不能为空")?;
        let device_id = normalize_required(device_id, "设备 ID 不能为空")?;
        self.wait_for_syncthing_api(Duration::from_secs(10)).await?;

        let home_dir = dirs::home_dir().unwrap_or_else(|| PathBuf::from("~"));
        let path = home_dir.join("Sync").join(&folder_id);

        let request = AddFolderRequest {
            id: folder_id.clone(),
            label: label.to_string(),
            path: path.to_string_lossy().to_string(),
            device_ids: vec![device_id.clone()],
            folder_type: Some("sendreceive".to_string()),
            rescan_interval_s: Some(3600),
            fs_watcher_enabled: Some(true),
            ignore_perms: Some(false),
            ignore_delete: Some(false),
            versioning: None,
        };

        self.add_folder(request).await?;

        // 移除待处理状态
        self.syncthing_request_empty(
            Method::DELETE,
            &["cluster", "pending", "folders"],
            &[("folder", folder_id), ("device", device_id)],
            None,
        ).await?;

        Ok(OperationResult { restart_required: false })
    }

    pub async fn ignore_folder(&self, folder_id: &str, device_id: &str) -> Result<OperationResult> {
        let folder_id = normalize_required(folder_id, "文件夹 ID 不能为空")?;
        let device_id = normalize_required(device_id, "设备 ID 不能为空")?;
        self.wait_for_syncthing_api(Duration::from_secs(10)).await?;

        self.syncthing_request_empty(
            Method::DELETE,
            &["cluster", "pending", "folders"],
            &[("folder", folder_id), ("device", device_id)],
            None,
        ).await?;

        Ok(OperationResult { restart_required: false })
    }

    pub async fn save_global_settings(
        &self,
        global_discovery: bool,
        local_discovery: bool,
        global_announce: bool,
        nat_enabled: bool,
        reconnection_interval: i32,
        max_connections: i32,
    ) -> Result<OperationResult> {
        self.wait_for_syncthing_api(Duration::from_secs(10)).await?;

        let mut config = self.syncthing_get(&["config"], &[]).await?;

        // Update options
        if let Some(options) = config.get_mut("options").and_then(Value::as_object_mut) {
            options.insert("globalAnnounceEnabled".to_string(), json!(global_announce));
            options.insert("localAnnounceEnabled".to_string(), json!(local_discovery));
            options.insert("natEnabled".to_string(), json!(nat_enabled));
            options.insert("reconnectionIntervalS".to_string(), json!(reconnection_interval));

            if max_connections > 0 {
                options.insert("connectionLimitMax".to_string(), json!(max_connections));
            }

            // Global discovery servers
            if global_discovery {
                if !options.contains_key("globalAnnounceServers") {
                    options.insert(
                        "globalAnnounceServers".to_string(),
                        json!(["default"]),
                    );
                }
            } else {
                options.insert("globalAnnounceServers".to_string(), json!([]));
            }
        }

        self.syncthing_request_empty(Method::PUT, &["config"], &[], Some(config))
            .await?;

        self.operation_result().await
    }
}

impl Drop for SyncthingService {
    fn drop(&mut self) {
        self.kill_owned_child();
    }
}

fn syncthing_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .no_proxy()
        .build()
        .context("创建 Syncthing API 客户端失败")
}

fn build_api_url(path_segments: &[&str], query: &[(&str, String)]) -> Result<Url> {
    let mut url = Url::parse(SYNCTHING_API_URL).context("Syncthing API 地址无效")?;

    {
        let mut segments = url
            .path_segments_mut()
            .map_err(|_| anyhow!("Syncthing API 地址不能作为 base URL"))?;
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

fn normalize_required(value: &str, message: &str) -> Result<String> {
    let value = value.trim().to_string();
    if value.is_empty() {
        bail!(message.to_string());
    }
    Ok(value)
}

fn empty_overview(
    is_downloaded: bool,
    running: bool,
    ready: bool,
    error: Option<String>,
) -> SyncthingOverview {
    SyncthingOverview {
        is_downloaded,
        running,
        ready,
        config: SyncthingConfig::default(),
        system_status: SyncthingSystemStatus::default(),
        connections: SyncthingConnections::default(),
        folder_statuses: std::collections::HashMap::new(),
        device_completions: std::collections::HashMap::new(),
        restart_required: false,
        error,
        pending_devices: std::collections::HashMap::new(),
        pending_folders: std::collections::HashMap::new(),
        upgrade_status: None,
    }
}

fn extract_api_key(config_xml: &str) -> Option<String> {
    let start = config_xml.find("<apikey>")? + "<apikey>".len();
    let end = config_xml[start..].find("</apikey>")? + start;
    let api_key = config_xml[start..end].trim().to_string();
    (!api_key.is_empty()).then_some(api_key)
}
