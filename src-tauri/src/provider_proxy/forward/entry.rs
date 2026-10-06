use axum::{
    body::Body,
    http::{Request, Response, StatusCode},
};
use tauri::AppHandle;

pub(super) async fn forward_proxy_request_with_client<R: tauri::Runtime>(
    app_handle: AppHandle<R>,
    tool_id: String,
    relative_path: String,
    request: Request<Body>,
    client: Option<reqwest::Client>,
) -> Response<Body> {
    let stopped = || {
        crate::provider_proxy::build_proxy_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "Proxy is shutting down".into(),
        )
    };
    crate::provider_proxy::cost::lifecycle()
        .until_closing(super::forward_proxy_request_inner(
            app_handle,
            tool_id,
            relative_path,
            request,
            client,
        ))
        .await
        .unwrap_or_else(stopped)
}
