use futures_util::future::join_all;
use std::time::Duration;
use tauri::{AppHandle, State};

use crate::db::DbState;

use super::super::types::ProviderStreamCheckResult;
use super::*;

async fn check_stream_profile(
    app_handle: AppHandle,
    profile: ConfigProfile,
    client: reqwest::Client,
) -> ProviderStreamCheckResult {
    let checked_at = chrono::Utc::now().to_rfc3339();
    let request = match extract_stream_check_request(&app_handle, &profile).await {
        Ok(request) => request,
        Err(message) => {
            let status =
                if message.contains("not supported") || message.contains("not yet supported") {
                    "unsupported"
                } else {
                    "unconfigured"
                };
            return ProviderStreamCheckResult {
                profile_id: profile.id,
                tool_id: profile.tool_id,
                provider_name: profile.name,
                base_url: None,
                status: status.to_string(),
                latency_ms: None,
                http_status: None,
                checked_at,
                message,
            };
        }
    };

    let started_at = std::time::Instant::now();
    let mut request_builder = client
        .post(&request.endpoint)
        .header("content-type", "application/json")
        .header("accept", "text/event-stream, application/json")
        .json(&request.body);
    for (name, value) in &request.headers {
        request_builder = request_builder.header(name, value);
    }

    let response = request_builder.send().await;
    let latency_ms = started_at.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
    let result = match response {
        Ok(mut response) => {
            let http_status = response.status().as_u16();
            if !response.status().is_success() {
                ProviderStreamCheckResult {
                    profile_id: profile.id,
                    tool_id: profile.tool_id,
                    provider_name: profile.name,
                    base_url: Some(request.endpoint),
                    status: "reachable".to_string(),
                    latency_ms: Some(latency_ms),
                    http_status: Some(http_status),
                    checked_at,
                    message: format!("Endpoint responded with HTTP {http_status}"),
                }
            } else {
                let chunk = tokio::time::timeout(Duration::from_secs(15), response.chunk()).await;
                let (status, message) = match chunk {
                    Ok(Ok(Some(chunk))) => (
                        "healthy",
                        format!("Received first stream chunk ({} bytes)", chunk.len()),
                    ),
                    Ok(Ok(None)) => (
                        "reachable",
                        "Stream endpoint closed without returning chunks".to_string(),
                    ),
                    Ok(Err(error)) => ("error", error.to_string()),
                    Err(_) => (
                        "reachable",
                        "Connected but no stream chunk arrived within 15 seconds".to_string(),
                    ),
                };
                ProviderStreamCheckResult {
                    profile_id: profile.id,
                    tool_id: profile.tool_id,
                    provider_name: profile.name,
                    base_url: Some(request.endpoint),
                    status: status.to_string(),
                    latency_ms: Some(latency_ms),
                    http_status: Some(http_status),
                    checked_at,
                    message,
                }
            }
        }
        Err(error) => ProviderStreamCheckResult {
            profile_id: profile.id,
            tool_id: profile.tool_id,
            provider_name: profile.name,
            base_url: Some(request.endpoint),
            status: "error".to_string(),
            latency_ms: None,
            http_status: None,
            checked_at,
            message: error.to_string(),
        },
    };
    log_provider_result(
        "stream-check-all",
        &result.tool_id,
        &result.provider_name,
        result.base_url.as_deref(),
        &result.status,
        &result.message,
    );
    result
}

#[tauri::command]
pub async fn stream_check_all_config_profiles(
    app_handle: AppHandle,
    db: State<'_, DbState>,
) -> Result<Vec<ProviderStreamCheckResult>, String> {
    let (profiles, client) = {
        let conn = db.0.lock().map_err(|error| error.to_string())?;
        let profiles = read_all_config_profiles_from_conn(&conn)?;
        let client = build_provider_probe_client(&conn)?;
        (profiles, client)
    };
    let profiles = profiles.into_iter().take(32).collect::<Vec<_>>();
    let checks = profiles
        .into_iter()
        .map(|profile| check_stream_profile(app_handle.clone(), profile, client.clone()));
    let results = join_all(checks).await;
    crate::utils::append_runtime_log(
        "info",
        "profiles",
        &format!(
            "Completed batch stream check for {} profiles",
            results.len()
        ),
    );
    Ok(results)
}
