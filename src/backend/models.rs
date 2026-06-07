use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AddFolderRequest {
    pub id: String,
    pub label: String,
    pub path: String,
    pub device_ids: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AddDeviceRequest {
    pub device_id: String,
    pub name: String,
    pub addresses: Vec<String>,
    pub folder_ids: Vec<String>,
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
    pub restart_required: bool,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncthingConfig {
    #[serde(default)]
    pub folders: Vec<SyncthingFolder>,
    #[serde(default)]
    pub devices: Vec<SyncthingDevice>,
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
