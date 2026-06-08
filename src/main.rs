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
    menu::{Menu, MenuItem, PredefinedMenuItem},
    Icon, MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent,
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
    let show_i = MenuItem::new("显示主窗口", true, None);
    let separator = PredefinedMenuItem::separator();
    let quit_i = MenuItem::new("退出", true, None);
    let _ = tray_menu.append(&show_i);
    let _ = tray_menu.append(&separator);
    let _ = tray_menu.append(&quit_i);

    let tray_icon_image = load_tray_icon(include_bytes!("../assets/icons/32x32.png"))?;
    let _tray_icon = TrayIconBuilder::new()
        .with_menu(Box::new(tray_menu))
        .with_tooltip("AeroSync")
        .with_icon(tray_icon_image)
        .with_menu_on_left_click(false)
        .build()?;

    app.window().on_close_requested({
        let app_weak = app.as_weak();
        move || {
            if app_weak
                .upgrade()
                .is_some_and(|app| app.get_close_to_tray())
            {
                slint::CloseRequestResponse::HideWindow
            } else {
                let _ = slint::quit_event_loop();
                slint::CloseRequestResponse::HideWindow
            }
        }
    });

    let show_id = show_i.id().clone();
    let quit_id = quit_i.id().clone();
    let app_weak_menu = app.as_weak();
    std::thread::spawn(move || {
        while let Ok(event) = tray_icon::menu::MenuEvent::receiver().recv() {
            if event.id == quit_id {
                let _ = slint::invoke_from_event_loop(|| {
                    let _ = slint::quit_event_loop();
                });
            } else if event.id == show_id {
                show_main_window(&app_weak_menu);
            }
        }
    });

    let app_weak_icon = app.as_weak();
    std::thread::spawn(move || {
        while let Ok(event) = TrayIconEvent::receiver().recv() {
            let should_show = matches!(
                event,
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                } | TrayIconEvent::DoubleClick {
                    button: MouseButton::Left,
                    ..
                }
            );

            if should_show {
                show_main_window(&app_weak_icon);
            }
        }
    });

    #[cfg(target_os = "linux")]
    let _gtk_event_pump = start_gtk_event_pump();

    let controller = AppController::new(app.as_weak(), runtime, service);

    controller.bind(&app);
    controller.start_initial_setup();

    app.show()?;
    slint::run_event_loop_until_quit()?;
    Ok(())
}

fn load_tray_icon(bytes: &[u8]) -> anyhow::Result<Icon> {
    let icon_image = image::load_from_memory(bytes)?.into_rgba8();
    let (icon_width, icon_height) = icon_image.dimensions();
    Ok(Icon::from_rgba(
        icon_image.into_raw(),
        icon_width,
        icon_height,
    )?)
}

fn show_main_window(app_weak: &slint::Weak<AppWindow>) {
    let app_weak = app_weak.clone();
    let _ = slint::invoke_from_event_loop(move || {
        if let Some(app) = app_weak.upgrade() {
            let _ = app.window().show();
        }
    });
}

#[cfg(target_os = "linux")]
fn start_gtk_event_pump() -> slint::Timer {
    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        std::time::Duration::from_millis(50),
        || while gtk::glib::MainContext::default().iteration(false) {},
    );
    timer
}
