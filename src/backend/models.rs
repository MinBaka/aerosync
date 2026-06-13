use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderVersioning {
    #[serde(rename = "type")]
    pub versioning_type: String, // simple, staggered, trashcan, external
    #[serde(default)]
    pub params: std::collections::HashMap<String, String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AddFolderRequest {
    pub id: String,
    pub label: String,
    pub path: String,
    pub device_ids: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub folder_type: Option<String>, // sendreceive, sendonly, receiveonly, receiveencrypted
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rescan_interval_s: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fs_watcher_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ignore_perms: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ignore_delete: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub versioning: Option<FolderVersioning>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AddDeviceRequest {
    pub device_id: String,
    pub name: String,
    pub addresses: Vec<String>,
    pub folder_ids: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub introducer: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_accept_folders: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_send_kbps: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_recv_kbps: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compression: Option<String>, // always, metadata, never
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationResult {
    pub restart_required: bool,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncthingOverview {
    pub is_downloaded: bool,
    pub running: bool,
    pub ready: bool,
    pub config: SyncthingConfig,
    pub system_status: SyncthingSystemStatus,
    pub connections: SyncthingConnections,
    #[serde(default)]
    pub folder_statuses: std::collections::HashMap<String, FolderStatus>,
    #[serde(default)]
    pub device_completions: std::collections::HashMap<String, DeviceCompletion>,
    pub restart_required: bool,
    pub error: Option<String>,
    #[serde(default)]
    pub pending_devices: std::collections::HashMap<String, PendingDevice>,
    #[serde(default)]
    pub pending_folders: std::collections::HashMap<String, PendingFolder>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderStatus {
    #[serde(default)]
    pub state: String,
    #[serde(default)]
    pub need_bytes: u64,
    #[serde(default)]
    pub in_sync_bytes: u64,
    #[serde(default)]
    pub global_bytes: u64,
    #[serde(flatten)]
    pub raw: serde_json::Map<String, Value>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceCompletion {
    #[serde(default)]
    pub completion: f64,
    #[serde(default)]
    pub need_bytes: u64,
    #[serde(flatten)]
    pub raw: serde_json::Map<String, Value>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingDevice {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub address: String,
    #[serde(default)]
    pub time: String,
    #[serde(flatten)]
    pub raw: serde_json::Map<String, Value>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingFolderOffer {
    #[serde(default)]
    pub time: String,
    #[serde(default)]
    pub label: String,
    #[serde(flatten)]
    pub raw: serde_json::Map<String, Value>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingFolder {
    #[serde(default, rename = "offeredBy")]
    pub offered_by: std::collections::HashMap<String, PendingFolderOffer>,
    #[serde(flatten)]
    pub raw: serde_json::Map<String, Value>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncthingConfig {
    #[serde(default)]
    pub folders: Vec<SyncthingFolder>,
    #[serde(default)]
    pub devices: Vec<SyncthingDevice>,
    #[serde(default)]
    pub options: SyncthingOptions,
    #[serde(flatten)]
    pub raw: serde_json::Map<String, Value>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncthingOptions {
    #[serde(default)]
    pub max_recv_kbps: i64,
    #[serde(default)]
    pub max_send_kbps: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub global_announce_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_announce_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nat_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reconnection_interval_s: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connection_limit_max: Option<i32>,
    #[serde(flatten)]
    pub raw: serde_json::Map<String, Value>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncthingFolder {
    pub id: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub paused: bool,
    #[serde(default)]
    pub devices: Vec<SyncthingFolderDevice>,
    #[serde(flatten)]
    pub raw: serde_json::Map<String, Value>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncthingFolderDevice {
    #[serde(default, rename = "deviceID")]
    pub device_id: String,
    #[serde(flatten)]
    pub raw: serde_json::Map<String, Value>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncthingDevice {
    #[serde(default, rename = "deviceID")]
    pub device_id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub addresses: Vec<String>,
    #[serde(default)]
    pub paused: bool,
    #[serde(flatten)]
    pub raw: serde_json::Map<String, Value>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncthingSystemStatus {
    #[serde(default)]
    pub my_id: String,
    #[serde(default)]
    pub discovery_enabled: bool,
    #[serde(default)]
    pub start_time: String,
    #[serde(default)]
    pub uptime: u64,
    #[serde(flatten)]
    pub raw: serde_json::Map<String, Value>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncthingConnections {
    #[serde(default)]
    pub total: SyncthingConnectionTotal,
    #[serde(default)]
    pub connections: std::collections::BTreeMap<String, SyncthingConnection>,
    #[serde(flatten)]
    pub raw: serde_json::Map<String, Value>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncthingConnectionTotal {
    #[serde(default)]
    pub in_bytes_total: u64,
    #[serde(default)]
    pub out_bytes_total: u64,
    #[serde(flatten)]
    pub raw: serde_json::Map<String, Value>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncthingConnection {
    #[serde(default)]
    pub connected: bool,
    #[serde(default)]
    pub address: String,
    #[serde(default, rename = "type")]
    pub connection_type: String,
    #[serde(flatten)]
    pub raw: serde_json::Map<String, Value>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LogEntry {
    pub when: String,
    pub message: String,
    pub level: String,
}
