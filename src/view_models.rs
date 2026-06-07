use crate::backend::models::SyncthingOverview;
use crate::formatting::{format_bytes, format_duration, short_id};
use crate::{DeviceRow, FolderRow, TransferRow};
use slint::SharedString;

pub struct UiSnapshot {
    pub is_running: bool,
    pub is_ready: bool,
    pub restart_required: bool,
    pub error_message: String,
    pub status_text: String,
    pub folder_count_text: String,
    pub folder_detail_text: String,
    pub device_count_text: String,
    pub uptime_text: String,
    pub traffic_text: String,
    pub local_device_id: String,
    pub discovery_text: String,
    pub start_time: String,
    pub device_choices: String,
    pub folder_choices: String,
    pub folders: Vec<FolderRow>,
    pub devices: Vec<DeviceRow>,
    pub transfers: Vec<TransferRow>,
}

pub fn snapshot_from_overview(overview: SyncthingOverview) -> UiSnapshot {
    let local_device_id = overview.system_status.my_id.clone();
    let folders = overview
        .config
        .folders
        .iter()
        .map(|folder| {
            let remote_shared = folder
                .devices
                .iter()
                .filter(|device| device.device_id != local_device_id)
                .count();
            FolderRow {
                id: shared(folder.id.clone()),
                title: shared(if folder.label.trim().is_empty() {
                    folder.id.clone()
                } else {
                    folder.label.clone()
                }),
                subtitle: shared(if folder.path.trim().is_empty() {
                    "未设置路径".to_string()
                } else {
                    folder.path.clone()
                }),
                shared_text: shared(format!("{remote_shared} 台")),
                status: shared(if folder.paused {
                    "已暂停"
                } else {
                    "同步中"
                }),
                paused: folder.paused,
            }
        })
        .collect::<Vec<_>>();

    let remote_devices = overview
        .config
        .devices
        .iter()
        .filter(|device| device.device_id != local_device_id)
        .collect::<Vec<_>>();

    let connected_devices = remote_devices
        .iter()
        .filter(|device| {
            overview
                .connections
                .connections
                .get(&device.device_id)
                .is_some_and(|connection| connection.connected)
        })
        .count();

    let devices = remote_devices
        .iter()
        .map(|device| {
            let connected = overview
                .connections
                .connections
                .get(&device.device_id)
                .is_some_and(|connection| connection.connected);
            let name = if device.name.trim().is_empty() {
                short_id(&device.device_id)
            } else {
                device.name.clone()
            };
            DeviceRow {
                id: shared(device.device_id.clone()),
                name: shared(name),
                short_id: shared(short_id(&device.device_id)),
                status: shared(if connected {
                    "在线"
                } else if device.paused {
                    "已暂停"
                } else {
                    "离线"
                }),
                paused: device.paused,
                connected,
            }
        })
        .collect::<Vec<_>>();

    let transfers = overview
        .connections
        .connections
        .iter()
        .map(|(device_id, connection)| TransferRow {
            device_id: shared(device_id.clone()),
            short_id: shared(short_id(device_id)),
            address: shared(if connection.address.trim().is_empty() {
                "未知".to_string()
            } else {
                connection.address.clone()
            }),
            connection_type: shared(if connection.connection_type.trim().is_empty() {
                "未知".to_string()
            } else {
                connection.connection_type.clone()
            }),
            status: shared(if connection.connected {
                "已连接"
            } else {
                "未连接"
            }),
            connected: connection.connected,
        })
        .collect::<Vec<_>>();

    let active_folders = overview
        .config
        .folders
        .iter()
        .filter(|folder| !folder.paused)
        .count();
    let traffic_text = format!(
        "{} ↓ / {} ↑",
        format_bytes(overview.connections.total.in_bytes_total),
        format_bytes(overview.connections.total.out_bytes_total)
    );

    UiSnapshot {
        is_running: overview.running,
        is_ready: overview.ready,
        restart_required: overview.restart_required,
        error_message: overview.error.unwrap_or_default(),
        status_text: if !overview.running {
            "未连接".to_string()
        } else if overview.ready {
            "运行中".to_string()
        } else {
            "启动中".to_string()
        },
        folder_count_text: overview.config.folders.len().to_string(),
        folder_detail_text: format!("{active_folders} 个正在同步"),
        device_count_text: format!("{connected_devices} / {}", remote_devices.len()),
        uptime_text: format_duration(overview.system_status.uptime, overview.ready),
        traffic_text,
        local_device_id: if overview.system_status.my_id.trim().is_empty() {
            "未知设备".to_string()
        } else {
            short_id(&overview.system_status.my_id)
        },
        discovery_text: if overview.system_status.discovery_enabled {
            "已启用".to_string()
        } else {
            "未启用".to_string()
        },
        start_time: if overview.system_status.start_time.trim().is_empty() {
            "等待数据".to_string()
        } else {
            overview.system_status.start_time
        },
        device_choices: remote_devices
            .iter()
            .map(|device| device.device_id.as_str())
            .collect::<Vec<_>>()
            .join(", "),
        folder_choices: overview
            .config
            .folders
            .iter()
            .map(|folder| folder.id.as_str())
            .collect::<Vec<_>>()
            .join(", "),
        folders,
        devices,
        transfers,
    }
}

fn shared(value: impl Into<SharedString>) -> SharedString {
    value.into()
}
