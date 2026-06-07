#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app_controller;
mod backend;
mod formatting;
mod view_models;

use app_controller::AppController;
use backend::syncthing::SyncthingService;
use std::sync::Arc;
use tokio::runtime::Builder;
use tray_icon::{
    menu::{Menu, MenuItem},
    TrayIconBuilder,
};

slint::include_modules!();

fn main() -> anyhow::Result<()> {
    #[cfg(target_os = "linux")]
    if let Err(err) = gtk::init() {
        eprintln!("Failed to initialize GTK: {}", err);
    }

    let app = AppWindow::new()?;
    let runtime = Arc::new(
        Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()?,
    );
    let service = Arc::new(SyncthingService::new());
    let tray_menu = Menu::new();
    let quit_i = MenuItem::new("退出", true, None);
    let _ = tray_menu.append(&quit_i);

    let _tray_icon = TrayIconBuilder::new()
        .with_menu(Box::new(tray_menu))
        .with_tooltip("AeroSync")
        .build()?;

    let quit_id = quit_i.id().clone();
    std::thread::spawn(move || {
        while let Ok(event) = tray_icon::menu::MenuEvent::receiver().recv() {
            if event.id == quit_id {
                let _ = slint::invoke_from_event_loop(|| {
                    let _ = slint::quit_event_loop();
                });
            }
        }
    });

    let controller = AppController::new(app.as_weak(), runtime, service);

    controller.bind(&app);
    controller.start_initial_setup();

    app.run()?;
    Ok(())
}
