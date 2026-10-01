use futures_util::future::join_all;
use tauri::{AppHandle, State};

use crate::db::DbState;

use super::super::types::ProviderStreamCheckResult;
use super::*;

pub(super) async fn check_stream_profile(
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

    let endpoint = request.endpoint.clone();
    let outcome = super::stream_probe::execute(client, request).await;
    let result = ProviderStreamCheckResult {
        profile_id: profile.id,
        tool_id: profile.tool_id,
        provider_name: profile.name,
        base_url: Some(endpoint),
        status: outcome.status.to_string(),
        latency_ms: outcome.latency_ms,
        http_status: outcome.http_status,
        checked_at,
        message: outcome.message,
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
