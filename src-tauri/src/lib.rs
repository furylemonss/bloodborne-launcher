mod commands;
mod constants;
mod download;
mod extract;
mod notify;
mod pipeline;
mod power;
mod state;
mod tray;

use tauri::image::Image;
use tauri::Manager;

const WINDOW_ICON_SIZE: u32 = 32;

fn last_root_path() -> Option<std::path::PathBuf> {
    dirs::data_dir().map(|d| d.join("bloodborne-installer").join("last-root.txt"))
}

fn load_last_root(_app: &tauri::AppHandle) -> Option<std::path::PathBuf> {
    let path = last_root_path()?;
    let text = std::fs::read_to_string(path).ok()?;
    let root = std::path::PathBuf::from(text.trim());
    if root.is_dir() {
        Some(root)
    } else {
        None
    }
}

/// Görev çubuğu + alt+tab + pencere başlığı ikonu: gömülü logo.
fn apply_window_icon(app: &tauri::AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    #[cfg(windows)]
    {
        if let Ok(icon) = Image::from_app_icon_resource(WINDOW_ICON_SIZE) {
            let _ = window.set_icon(icon);
            return;
        }
    }
    if let Some(icon) = app.default_window_icon() {
        let _ = window.set_icon(icon.clone());
    }
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .manage(commands::Shared::new())
        .invoke_handler(tauri::generate_handler![
            commands::get_state,
            commands::set_root,
            commands::disk_info,
            commands::start_pipeline,
            commands::pause,
            commands::resume,
            commands::hide_to_tray,
            commands::show_window,
            commands::clean_leftovers,
            commands::clean_temp,
            commands::get_port_settings,
            commands::set_port_settings,
            commands::play,
            commands::stop_game,
            commands::open_advanced,
            commands::shutdown_now,
            commands::restart_now,
            commands::quit_app,
            commands::ping
        ])
        .setup(|app| {
            apply_window_icon(app.handle());
            tray::setup_tray(app.handle())?;
            let handle = app.handle().clone();
            if let Some(window) = app.get_webview_window("main") {
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        commands::quit_app(handle.clone());
                    }
                });
            }

            if let Some(root) = load_last_root(app.handle()) {
                let _ = commands::set_root(app.handle().clone(), root.to_string_lossy().to_string());
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("tauri calistirilamadi");
}
