use tauri::{AppHandle, WebviewWindow, WebviewWindowBuilder};

pub(crate) fn create_main_window(
    app_handle: &AppHandle,
    visible: bool,
) -> Result<WebviewWindow, String> {
    let window_config = app_handle
        .config()
        .app
        .windows
        .iter()
        .find(|window| window.label == "main")
        .or_else(|| app_handle.config().app.windows.first())
        .ok_or_else(|| "Main window config not found".to_string())?;

    let builder = WebviewWindowBuilder::from_config(app_handle, window_config)
        .map_err(|error| format!("Failed to load main window config: {error}"))?;
    let builder = if visible {
        builder.visible(true)
    } else {
        builder
    };

    builder
        .build()
        .map_err(|error| format!("Failed to create main window: {error}"))
}
