#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app_controller;
mod backend;
mod formatting;
mod view_models;

use app_controller::AppController;
use backend::syncthing::SyncthingService;
use std::sync::Arc;
use tokio::runtime::Builder;

slint::include_modules!();

fn main() -> anyhow::Result<()> {
    let app = AppWindow::new()?;
    let runtime = Arc::new(
        Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()?,
    );
    let service = Arc::new(SyncthingService::new());
    let controller = AppController::new(app.as_weak(), runtime, service);

    controller.bind(&app);
    controller.start_initial_setup();

    app.run()?;
    Ok(())
}
