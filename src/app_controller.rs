use crate::backend::models::{
    AddDeviceRequest, AddFolderRequest, FolderVersioning, OperationResult, SyncthingOverview,
};
use crate::backend::syncthing::SyncthingService;
use crate::formatting::{parse_addresses, split_ids};
use crate::view_models::snapshot_from_overview;
use crate::AppWindow;
use anyhow::{bail, Result};
use slint::{ModelRc, SharedString, VecModel, Weak};
use std::future::Future;
use std::rc::Rc;
use std::sync::Arc;
use tokio::runtime::Runtime;

#[derive(Clone)]
pub struct AppController {
    app: Weak<AppWindow>,
    runtime: Arc<Runtime>,
    service: Arc<SyncthingService>,
}

impl AppController {
    pub fn new(
        app: Weak<AppWindow>,
        runtime: Arc<Runtime>,
        service: Arc<SyncthingService>,
    ) -> Self {
        Self {
            app,
            runtime,
            service,
        }
    }

    pub fn bind(&self, app: &AppWindow) {
        let controller = self.clone();
        app.on_download_syncthing_requested(move || {
            let weak = controller.app.clone();
            let _ = weak.upgrade_in_event_loop(move |app| {
                app.set_is_downloading(true);
                app.set_download_progress(0.0);
            });

            let service = controller.service.clone();
            let weak_for_task = controller.app.clone();

            controller.runtime.spawn(async move {
                let result = service
                    .download_core(move |progress| {
                        let weak_inner = weak_for_task.clone();
                        let _ = weak_inner.upgrade_in_event_loop(move |app| {
                            app.set_download_progress(progress);
                        });
                    })
                    .await;

                let _ = weak.upgrade_in_event_loop(move |app| {
                    app.set_is_downloading(false);
                    match result {
                        Ok(_) => {
                            app.set_is_syncthing_downloaded(true);
                            app.set_success_message(shared("Syncthing 下载成功"));
                        }
                        Err(e) => {
                            app.set_error_message(shared(format!("下载失败: {}", e)));
                        }
                    }
                });
            });
        });

        let controller = self.clone();
        app.on_refresh_requested(move || {
            controller.refresh();
        });

        let controller = self.clone();
        app.on_open_web_requested(move || {
            controller.run_mutation("打开 Web UI", |service| async move {
                service.open_web_ui()?;
                Ok(None)
            });
        });

        let controller = self.clone();
        app.on_start_core_requested(move || {
            controller.run_mutation("启动核心", |service| async move {
                service.start().await?;
                Ok(None)
            });
        });

        let controller = self.clone();
        app.on_shutdown_core_requested(move || {
            controller.run_mutation("停止核心", |service| async move {
                service.shutdown().await?;
                Ok(None)
            });
        });

        let controller = self.clone();
        app.on_restart_core_requested(move || {
            controller.run_mutation("重启核心", |service| async move {
                service.restart().await?;
                Ok(None)
            });
        });

        let controller = self.clone();
        app.on_add_folder_requested(move |id, label, path, device_ids, folder_type, rescan_interval, fs_watcher, ignore_perms, ignore_delete, versioning_type| {
            let request = AddFolderRequest {
                id: id.to_string(),
                label: label.to_string(),
                path: path.to_string(),
                device_ids: split_ids(&device_ids),
                folder_type: Some(folder_type.to_string()),
                rescan_interval_s: Some(rescan_interval as i64),
                fs_watcher_enabled: Some(fs_watcher),
                ignore_perms: Some(ignore_perms),
                ignore_delete: Some(ignore_delete),
                versioning: if versioning_type.as_str() == "none" {
                    None
                } else {
                    Some(FolderVersioning {
                        versioning_type: versioning_type.to_string(),
                        params: std::collections::HashMap::new(),
                    })
                },
            };
            controller.run_mutation("添加文件夹", move |service| async move {
                Ok(Some(service.add_folder(request).await?))
            });
        });

        let controller = self.clone();
        app.on_edit_folder_requested(move |id, label, path, device_ids, folder_type, rescan_interval, fs_watcher, ignore_perms, ignore_delete, versioning_type| {
            let request = AddFolderRequest {
                id: id.to_string(),
                label: label.to_string(),
                path: path.to_string(),
                device_ids: split_ids(&device_ids),
                folder_type: Some(folder_type.to_string()),
                rescan_interval_s: Some(rescan_interval as i64),
                fs_watcher_enabled: Some(fs_watcher),
                ignore_perms: Some(ignore_perms),
                ignore_delete: Some(ignore_delete),
                versioning: if versioning_type.as_str() == "none" {
                    None
                } else {
                    Some(FolderVersioning {
                        versioning_type: versioning_type.to_string(),
                        params: std::collections::HashMap::new(),
                    })
                },
            };
            controller.run_mutation("编辑文件夹", move |service| async move {
                Ok(Some(service.edit_folder(request).await?))
            });
        });

        let controller = self.clone();
        app.on_add_device_requested(move |device_id, name, addresses, folder_ids, introducer, auto_accept, compression| {
            let request = AddDeviceRequest {
                device_id: device_id.to_string(),
                name: name.to_string(),
                addresses: parse_addresses(&addresses),
                folder_ids: split_ids(&folder_ids),
                introducer: Some(introducer),
                auto_accept_folders: Some(auto_accept),
                max_send_kbps: None,
                max_recv_kbps: None,
                compression: Some(compression.to_string()),
            };
            controller.run_mutation("添加设备", move |service| async move {
                Ok(Some(service.edit_device(request).await?))
            });
        });

        let controller = self.clone();
        app.on_edit_device_requested(move |device_id, name, addresses, folder_ids, introducer, auto_accept, compression| {
            let request = AddDeviceRequest {
                device_id: device_id.to_string(),
                name: name.to_string(),
                addresses: parse_addresses(&addresses),
                folder_ids: split_ids(&folder_ids),
                introducer: Some(introducer),
                auto_accept_folders: Some(auto_accept),
                max_send_kbps: None,
                max_recv_kbps: None,
                compression: Some(compression.to_string()),
            };
            controller.run_mutation("编辑设备", move |service| async move {
                Ok(Some(service.edit_device(request).await?))
            });
        });

        let controller = self.clone();
        app.on_folder_action_requested(move |action, folder_id| {
            let action = action.to_string();
            let folder_id = folder_id.to_string();
            let action_name = match action.as_str() {
                "pause" => "暂停文件夹",
                "resume" => "恢复文件夹",
                "rescan" => "扫描文件夹",
                _ => "文件夹操作",
            };
            controller.run_mutation(action_name, move |service| async move {
                let result = match action.as_str() {
                    "pause" => service.pause_folder(&folder_id).await?,
                    "resume" => service.resume_folder(&folder_id).await?,
                    "rescan" => service.rescan_folder(&folder_id).await?,
                    _ => bail!("未知文件夹操作: {action}"),
                };
                Ok(Some(result))
            });
        });

        let controller = self.clone();
        app.on_device_action_requested(move |action, device_id| {
            let action = action.to_string();
            let device_id = device_id.to_string();
            let action_name = match action.as_str() {
                "pause" => "暂停设备",
                "resume" => "恢复设备",
                _ => "设备操作",
            };
            controller.run_mutation(action_name, move |service| async move {
                let result = match action.as_str() {
                    "pause" => service.pause_device(&device_id).await?,
                    "resume" => service.resume_device(&device_id).await?,
                    _ => bail!("未知设备操作: {action}"),
                };
                Ok(Some(result))
            });
        });

        let controller = self.clone();
        app.on_global_action_requested(move |action| {
            let action = action.to_string();
            let action_name = match action.as_str() {
                "rescan-all" => "扫描全部文件夹",
                "pause-all" => "暂停全部设备",
                "resume-all" => "恢复全部设备",
                _ => "全局操作",
            };
            controller.run_mutation(action_name, move |service| async move {
                let result = match action.as_str() {
                    "rescan-all" => service.rescan_all_folders().await?,
                    "pause-all" => service.pause_all_devices().await?,
                    "resume-all" => service.resume_all_devices().await?,
                    _ => bail!("未知全局操作: {action}"),
                };
                Ok(Some(result))
            });
        });

        let controller = self.clone();
        app.on_get_folder_ignores_requested(move |folder_id| {
            let folder_id = folder_id.to_string();
            let weak = controller.app.clone();
            let service = controller.service.clone();
            controller.runtime.spawn(async move {
                let ignores_result = service.get_folder_ignores(&folder_id).await;
                let _ = weak.upgrade_in_event_loop(move |app| {
                    if let Ok(ignores) = ignores_result {
                        app.set_ignores_text(shared(ignores));
                    }
                });
            });
        });

        let controller = self.clone();
        app.on_set_folder_ignores_requested(move |folder_id, ignores| {
            let folder_id = folder_id.to_string();
            let ignores = ignores.to_string();
            controller.run_mutation("保存忽略模式", move |service| async move {
                Ok(Some(
                    service.set_folder_ignores(&folder_id, &ignores).await?,
                ))
            });
        });

        let controller = self.clone();
        app.on_set_rate_limits_requested(move |recv, send| {
            let recv_kbps = recv.parse::<i64>().unwrap_or(0);
            let send_kbps = send.parse::<i64>().unwrap_or(0);
            controller.run_mutation("应用网络限速", move |service| async move {
                Ok(Some(service.set_rate_limits(recv_kbps, send_kbps).await?))
            });
        });

        let controller = self.clone();
        app.on_confirmed_action_requested(move |action, target| {
            let action = action.to_string();
            let target = target.to_string();
            let action_name = match action.as_str() {
                "remove-folder" => "删除文件夹",
                "remove-device" => "删除设备",
                "delete-folder-files" => "删除文件夹文件",
                _ => "确认操作",
            };
            controller.run_mutation(action_name, move |service| async move {
                let result = match action.as_str() {
                    "remove-folder" => service.remove_folder(&target).await?,
                    "remove-device" => service.remove_device(&target).await?,
                    "delete-folder-files" => {
                        service.delete_folder_files(&target).await?;
                        OperationResult { restart_required: false }
                    }
                    _ => bail!("未知确认操作: {action}"),
                };
                Ok(Some(result))
            });
        });
    }

    pub fn start_initial_setup(&self) {
        self.post_to_ui(|app| {
            app.set_is_loading(true);
            app.set_is_running(true);
            app.set_status_text(shared("启动中"));
            app.set_error_message(shared(""));
            app.set_success_message(shared(""));
        });

        let service = self.service.clone();
        let weak = self.app.clone();
        self.runtime.spawn(async move {
            let setup_result = service.setup().await;
            let overview_result = service.overview().await;
            let logs_result = service.get_logs(None).await;
            post_result(
                weak,
                setup_result.err().map(|error| error.to_string()),
                overview_result,
                logs_result,
            );
        });
    }

    fn refresh(&self) {
        self.post_to_ui(|app| {
            app.set_is_loading(true);
            app.set_error_message(shared(""));
        });

        let service = self.service.clone();
        let weak = self.app.clone();
        self.runtime.spawn(async move {
            let overview_result = service.overview().await;
            let logs_result = service.get_logs(None).await;
            post_result(weak, None, overview_result, logs_result);
        });
    }

    fn run_mutation<F, Fut>(&self, name: &'static str, operation: F)
    where
        F: FnOnce(Arc<SyncthingService>) -> Fut + Send + 'static,
        Fut: Future<Output = Result<Option<OperationResult>>> + Send + 'static,
    {
        self.post_to_ui(|app| {
            app.set_is_mutating(true);
            app.set_error_message(shared(""));
            app.set_success_message(shared(""));
        });

        let service = self.service.clone();
        let weak = self.app.clone();
        self.runtime.spawn(async move {
            let result = operation(service.clone()).await;
            let overview_result = if result.is_ok() {
                Some(service.overview().await)
            } else {
                None
            };

            let _ = weak.upgrade_in_event_loop(move |app| {
                app.set_is_mutating(false);
                match result {
                    Ok(operation_result) => {
                        if let Some(Ok(overview)) = overview_result {
                            apply_overview(&app, overview);
                        }
                        if operation_result.is_some_and(|result| result.restart_required) {
                            app.set_restart_required(true);
                            app.set_success_message(shared(
                                "操作已完成，Syncthing 提示需要重启核心。",
                            ));
                        } else {
                            app.set_success_message(shared(format!("{name}完成")));
                        }
                    }
                    Err(error) => {
                        app.set_error_message(shared(error.to_string()));
                    }
                }
            });
        });
    }

    fn post_to_ui(&self, update: impl FnOnce(AppWindow) + Send + 'static) {
        let weak = self.app.clone();
        let _ = weak.upgrade_in_event_loop(update);
    }
}

fn post_result(
    weak: Weak<AppWindow>,
    setup_error: Option<String>,
    overview_result: Result<SyncthingOverview>,
    logs_result: Result<Vec<crate::backend::models::LogEntry>>,
) {
    let _ = weak.upgrade_in_event_loop(move |app| {
        app.set_is_loading(false);
        match overview_result {
            Ok(overview) => apply_overview(&app, overview),
            Err(error) => app.set_error_message(shared(error.to_string())),
        }
        if let Ok(logs) = logs_result {
            apply_logs(&app, logs);
        }
        if let Some(error) = setup_error {
            app.set_error_message(shared(format!("无法启动 Syncthing: {error}")));
        }
    });
}

fn apply_overview(app: &AppWindow, overview: SyncthingOverview) {
    let snapshot = snapshot_from_overview(overview);
    app.set_is_syncthing_downloaded(snapshot.is_downloaded);
    app.set_is_running(snapshot.is_running);
    app.set_is_ready(snapshot.is_ready);
    app.set_restart_required(snapshot.restart_required);
    app.set_status_text(shared(snapshot.status_text));
    app.set_last_updated(shared("刚刚刷新"));
    app.set_error_message(shared(snapshot.error_message));
    app.set_folder_count_text(shared(snapshot.folder_count_text));
    app.set_folder_detail_text(shared(snapshot.folder_detail_text));
    app.set_device_count_text(shared(snapshot.device_count_text));
    app.set_uptime_text(shared(snapshot.uptime_text));
    app.set_traffic_text(shared(snapshot.traffic_text));
    app.set_local_device_id(shared(snapshot.local_device_id));
    app.set_discovery_text(shared(snapshot.discovery_text));
    app.set_start_time(shared(snapshot.start_time));
    app.set_device_choices(shared(snapshot.device_choices));
    app.set_folder_choices(shared(snapshot.folder_choices));
    app.set_config_max_recv_kbps(shared(snapshot.config_max_recv_kbps));
    app.set_config_max_send_kbps(shared(snapshot.config_max_send_kbps));
    app.set_folders(model(snapshot.folders));
    app.set_devices(model(snapshot.devices));
    app.set_transfers(model(snapshot.transfers));
}

fn apply_logs(app: &AppWindow, logs: Vec<crate::backend::models::LogEntry>) {
    use crate::LogRow;
    let log_rows: Vec<LogRow> = logs
        .into_iter()
        .map(|log| LogRow {
            when: log.when.into(),
            level: log.level.to_uppercase().into(),
            message: log.message.into(),
        })
        .collect();
    app.set_logs(model(log_rows));
}

fn model<T: Clone + 'static>(items: Vec<T>) -> ModelRc<T> {
    Rc::new(VecModel::from(items)).into()
}

fn shared(value: impl Into<SharedString>) -> SharedString {
    value.into()
}
