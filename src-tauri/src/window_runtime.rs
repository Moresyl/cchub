use tauri::{AppHandle, Manager, WebviewWindow};

use crate::{lightweight, utils, window_preferences::load_window_preferences};

const WINDOW_ICON: tauri::image::Image<'_> = tauri::include_image!("./icons/icon.png");

pub(crate) fn configure_main_window(
    app_handle: &AppHandle,
    window: &WebviewWindow,
) -> Result<(), tauri::Error> {
    window.set_icon(WINDOW_ICON.clone())?;
    let handle = app_handle.clone();
    window.on_window_event(move |event| {
        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
            let preferences = load_window_preferences(&handle);
            if preferences.lightweight_mode {
                api.prevent_close();
                if let Some(window) = handle.get_webview_window("main") {
                    let _ = window.destroy();
                }
                return;
            }
            if preferences.close_to_tray {
                api.prevent_close();
                if let Some(window) = handle.get_webview_window("main") {
                    let _ = window.hide();
                }
            } else {
                handle.exit(0);
            }
        }
    });
    Ok(())
}

pub(crate) fn show_or_create_main_window(
    app_handle: &AppHandle,
    force_visible: bool,
) -> Option<WebviewWindow> {
    let window = match app_handle.get_webview_window("main") {
        Some(window) => window,
        None => match lightweight::create_main_window(app_handle, force_visible) {
            Ok(window) => {
                if let Err(error) = configure_main_window(app_handle, &window) {
                    utils::append_runtime_log(
                        "error",
                        "window",
                        &format!("Failed to configure main window: {error}"),
                    );
                }
                window
            }
            Err(error) => {
                utils::append_runtime_log(
                    "error",
                    "window",
                    &format!("Failed to recreate main window: {error}"),
                );
                return None;
            }
        },
    };
    if force_visible {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
    Some(window)
}
