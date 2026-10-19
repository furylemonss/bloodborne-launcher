use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};

use crate::commands::{pause_pipeline, resume_pipeline};

const TRAY_SIZE: u32 = 32;

pub fn setup_tray(app: &AppHandle) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "Pencereye dön", true, None::<&str>)?;
    let minimize = MenuItem::with_id(app, "minimize", "Küçült", true, None::<&str>)?;
    let reset = MenuItem::with_id(app, "reset", "Pencereyi sıfırla", true, None::<&str>)?;
    let pause = MenuItem::with_id(app, "pause", "Dondur", true, None::<&str>)?;
    let resume = MenuItem::with_id(app, "resume", "Devam", true, None::<&str>)?;
    let hide = MenuItem::with_id(app, "hide", "Tray'e al", true, None::<&str>)?;
    let sep = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "Çıkış", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[&show, &minimize, &reset, &hide, &pause, &resume, &sep, &quit],
    )?;

    let mut builder = TrayIconBuilder::with_id("main")
        .tooltip("Bloodborne Launcher")
        .icon_as_template(false)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => show_main(app),
            "minimize" => minimize_main(app),
            "reset" => reset_window(app),
            "hide" => hide_main(app),
            "pause" => pause_pipeline(app.clone()),
            "resume" => resume_pipeline(app.clone()),
            "quit" => crate::commands::quit_app(app.clone()),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main(tray.app_handle());
            }
        });

    #[cfg(windows)]
    {
        if let Ok(icon) = Image::from_app_icon_resource(TRAY_SIZE) {
            builder = builder.icon(icon);
        } else if let Some(icon) = app.default_window_icon() {
            builder = builder.icon(icon.clone());
        }
    }
    #[cfg(not(windows))]
    {
        if let Some(icon) = app.default_window_icon() {
            builder = builder.icon(icon.clone());
        }
    }

    builder.build(app)?;
    Ok(())
}

pub fn show_main(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

pub fn minimize_main(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.minimize();
    }
}

pub fn reset_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
        let _ = window.set_size(tauri::LogicalSize::new(1120.0, 720.0));
        let _ = window.center();
    }
}

pub fn hide_main(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }
}
