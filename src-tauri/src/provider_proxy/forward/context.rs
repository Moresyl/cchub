use crate::db::DbState;
use crate::provider_proxy::{
    build_proxy_error, desktop, read_local_provider_proxy_settings_from_conn,
};
use axum::{
    body::Body,
    http::{Request, Response, StatusCode},
};
use tauri::{AppHandle, Manager};

pub(super) fn setup<R: tauri::Runtime>(
    app: &AppHandle<R>,
    tool: &str,
    request: &mut Request<Body>,
) -> Result<Option<String>, Response<Body>> {
    let db = app.state::<DbState>();
    let conn = db.0.lock().map_err(|_| {
        build_proxy_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Database lock failed".into(),
        )
    })?;
    let settings = read_local_provider_proxy_settings_from_conn(&conn);
    if !settings.enabled_apps.iter().any(|item| item == tool) {
        return Err(build_proxy_error(
            StatusCode::NOT_FOUND,
            format!("Local provider proxy is not enabled for {tool}"),
        ));
    }
    if tool == "claude-desktop" {
        desktop::authorize(&conn, request.headers())
            .map_err(|error| build_proxy_error(StatusCode::UNAUTHORIZED, error))?;
        request.headers_mut().remove("authorization");
        request.headers_mut().remove("x-api-key");
    }
    Ok(conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = 'proxy_url'",
            [],
            |row| row.get::<_, String>(0),
        )
        .ok())
}
