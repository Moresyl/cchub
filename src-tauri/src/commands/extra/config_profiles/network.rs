#![allow(clippy::too_many_arguments)]
use futures_util::future::join_all;
use tauri::{AppHandle, State};

use crate::db::DbState;

use super::super::types::*;
use super::*;

#[tauri::command]
pub async fn ping_provider_endpoint(
    id: String,
    app_handle: AppHandle,
    db: State<'_, DbState>,
) -> Result<ProviderPingResult, String> {
    let (profile, client) = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        let profile = read_all_config_profiles_from_conn(&conn)?
            .into_iter()
            .find(|profile| profile.id == id)
            .ok_or_else(|| format!("Profile not found: {id}"))?;
        let client = build_provider_probe_client(&conn)?;
        (profile, client)
    };

    let checked_at = chrono::Utc::now().to_rfc3339();
    let (base_url, headers) = match extract_probe_target(&app_handle, &profile).await {
        Ok(value) => value,
        Err(message) => {
            let result = ProviderPingResult {
                profile_id: profile.id,
                tool_id: profile.tool_id,
                provider_name: profile.name,
                base_url: None,
                status: "error".to_string(),
                latency_ms: None,
                http_status: None,
                checked_at,
                message,
            };
            log_provider_result(
                "ping",
                &result.tool_id,
                &result.provider_name,
                result.base_url.as_deref(),
                &result.status,
                &result.message,
            );
            return Ok(result);
        }
    };

    let Some(base_url) = base_url else {
        let result = ProviderPingResult {
            profile_id: profile.id,
            tool_id: profile.tool_id,
            provider_name: profile.name,
            base_url: None,
            status: "error".to_string(),
            latency_ms: None,
            http_status: None,
            checked_at,
            message: "No base URL configured for latency ping".to_string(),
        };
        log_provider_result(
            "ping",
            &result.tool_id,
            &result.provider_name,
            result.base_url.as_deref(),
            &result.status,
            &result.message,
        );
        return Ok(result);
    };

    let send_request = |method: reqwest::Method| {
        let client = client.clone();
        let base_url = base_url.clone();
        let headers = headers.clone();
        async move {
            let started_at = std::time::Instant::now();
            let mut request = client.request(method, &base_url);
            for (name, value) in headers {
                request = request.header(&name, value);
            }
            request
                .send()
                .await
                .map(|response| (response, started_at.elapsed().as_millis() as u64))
        }
    };

    let mut response_result = send_request(reqwest::Method::HEAD).await;
    let should_fallback_to_get = matches!(
        response_result,
        Ok((ref response, _))
            if response.status() == reqwest::StatusCode::METHOD_NOT_ALLOWED
                || response.status() == reqwest::StatusCode::NOT_IMPLEMENTED
    );
    if should_fallback_to_get {
        response_result = send_request(reqwest::Method::GET).await;
    }

    let result = match response_result {
        Ok((response, latency_ms)) => {
            let http_status = response.status().as_u16();
            ProviderPingResult {
                profile_id: profile.id,
                tool_id: profile.tool_id,
                provider_name: profile.name,
                base_url: Some(base_url),
                status: classify_provider_latency_status(latency_ms),
                latency_ms: Some(latency_ms),
                http_status: Some(http_status),
                checked_at,
                message: format!("Endpoint responded with HTTP {http_status}"),
            }
        }
        Err(error) => ProviderPingResult {
            profile_id: profile.id,
            tool_id: profile.tool_id,
            provider_name: profile.name,
            base_url: Some(base_url),
            status: "error".to_string(),
            latency_ms: None,
            http_status: None,
            checked_at,
            message: error.to_string(),
        },
    };

    log_provider_result(
        "ping",
        &result.tool_id,
        &result.provider_name,
        result.base_url.as_deref(),
        &result.status,
        &result.message,
    );
    Ok(result)
}

/// Probe the primary endpoint and every metadata.endpointCandidates entry in parallel.
///
/// This intentionally performs a lightweight HEAD/GET request only. It never sends a
/// model prompt, so scanning failover endpoints is safe and does not consume provider quota.
#[tauri::command]
pub async fn scan_provider_endpoints(
    id: String,
    app_handle: AppHandle,
    db: State<'_, DbState>,
) -> Result<Vec<ProviderEndpointCheckResult>, String> {
    let (profile, client) = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        let profile = read_all_config_profiles_from_conn(&conn)?
            .into_iter()
            .find(|profile| profile.id == id)
            .ok_or_else(|| format!("Profile not found: {id}"))?;
        let client = build_provider_probe_client(&conn)?;
        (profile, client)
    };

    let (primary, headers) = extract_probe_target(&app_handle, &profile).await?;
    let parsed: serde_json::Value = serde_json::from_str(&profile.config_snapshot)
        .map_err(|error| format!("Invalid profile config: {error}"))?;
    let mut endpoints = Vec::new();
    if let Some(value) = primary {
        push_probe_endpoint(&mut endpoints, value);
    }
    if let Some(values) = parsed
        .get("metadata")
        .and_then(|value| value.get("endpointCandidates"))
        .and_then(serde_json::Value::as_array)
    {
        for value in values.iter().filter_map(serde_json::Value::as_str) {
            push_probe_endpoint(&mut endpoints, value.to_string());
        }
    }
    if endpoints.is_empty() {
        return Err("No endpoint is configured for this profile".to_string());
    }
    endpoints.truncate(8);

    let checks = endpoints.into_iter().map(|endpoint| {
        let client = client.clone();
        let headers = headers.clone();
        async move { probe_endpoint(client, endpoint, headers).await }
    });
    let results = join_all(checks).await;
    let successful = results
        .iter()
        .filter(|result| result.status != "error")
        .count();
    crate::utils::append_runtime_log(
        "info",
        "profiles",
        &format!(
            "Scanned {} endpoints for profile {}: {successful} reachable",
            results.len(),
            profile.name
        ),
    );
    Ok(results)
}

fn push_probe_endpoint(endpoints: &mut Vec<String>, value: String) {
    let endpoint = value.trim().trim_end_matches('/').to_string();
    if (endpoint.starts_with("https://") || endpoint.starts_with("http://"))
        && !endpoint.is_empty()
        && !endpoints.iter().any(|existing| existing == &endpoint)
    {
        endpoints.push(endpoint);
    }
}

async fn probe_endpoint(
    client: reqwest::Client,
    endpoint: String,
    headers: Vec<(String, String)>,
) -> ProviderEndpointCheckResult {
    let started_at = std::time::Instant::now();
    let mut request = client.head(&endpoint);
    for (name, value) in &headers {
        request = request.header(name, value);
    }
    let mut response = request.send().await;
    if matches!(response, Ok(ref value) if value.status() == reqwest::StatusCode::METHOD_NOT_ALLOWED
        || value.status() == reqwest::StatusCode::NOT_IMPLEMENTED)
    {
        let mut fallback = client.get(&endpoint);
        for (name, value) in &headers {
            fallback = fallback.header(name, value);
        }
        response = fallback.send().await;
    }

    let latency_ms = started_at.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
    match response {
        Ok(response) => {
            let status = response.status().as_u16();
            ProviderEndpointCheckResult {
                endpoint,
                status: classify_provider_latency_status(latency_ms),
                latency_ms: Some(latency_ms),
                http_status: Some(status),
                message: format!("Endpoint responded with HTTP {status}"),
            }
        }
        Err(error) => ProviderEndpointCheckResult {
            endpoint,
            status: "error".to_string(),
            latency_ms: None,
            http_status: None,
            message: error.to_string(),
        },
    }
}

#[tauri::command]
pub async fn probe_config_profile(
    id: String,
    app_handle: AppHandle,
    db: State<'_, DbState>,
) -> Result<ProviderProbeResult, String> {
    let (profile, client) = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        let profile = read_all_config_profiles_from_conn(&conn)?
            .into_iter()
            .find(|profile| profile.id == id)
            .ok_or_else(|| format!("Profile not found: {id}"))?;
        let client = build_provider_probe_client(&conn)?;
        (profile, client)
    };

    let checked_at = chrono::Utc::now().to_rfc3339();
    let (base_url, headers) = match extract_probe_target(&app_handle, &profile).await {
        Ok(value) => value,
        Err(message) => {
            let result = ProviderProbeResult {
                profile_id: profile.id,
                tool_id: profile.tool_id,
                provider_name: profile.name,
                base_url: None,
                status: "unconfigured".to_string(),
                latency_ms: None,
                http_status: None,
                checked_at,
                message,
            };
            log_provider_result(
                "probe",
                &result.tool_id,
                &result.provider_name,
                result.base_url.as_deref(),
                &result.status,
                &result.message,
            );
            return Ok(result);
        }
    };

    let result = if let Some(base_url) = base_url {
        let started_at = std::time::Instant::now();
        let mut request = client.get(&base_url);
        for (name, value) in headers {
            request = request.header(&name, value);
        }

        match request.send().await {
            Ok(response) => {
                let latency_ms = started_at.elapsed().as_millis() as u64;
                let http_status = response.status().as_u16();
                let status = if response.status().is_success() {
                    "healthy"
                } else if response.status().is_client_error() || response.status().is_server_error()
                {
                    "reachable"
                } else {
                    "unknown"
                };

                ProviderProbeResult {
                    profile_id: profile.id,
                    tool_id: profile.tool_id,
                    provider_name: profile.name,
                    base_url: Some(base_url),
                    status: status.to_string(),
                    latency_ms: Some(latency_ms),
                    http_status: Some(http_status),
                    checked_at,
                    message: format!("Endpoint responded with HTTP {http_status}"),
                }
            }
            Err(error) => ProviderProbeResult {
                profile_id: profile.id,
                tool_id: profile.tool_id,
                provider_name: profile.name,
                base_url: Some(base_url),
                status: "error".to_string(),
                latency_ms: None,
                http_status: None,
                checked_at,
                message: error.to_string(),
            },
        }
    } else {
        ProviderProbeResult {
            profile_id: profile.id,
            tool_id: profile.tool_id,
            provider_name: profile.name,
            base_url: None,
            status: "unconfigured".to_string(),
            latency_ms: None,
            http_status: None,
            checked_at,
            message: "No base URL configured for probing".to_string(),
        }
    };

    log_provider_result(
        "probe",
        &result.tool_id,
        &result.provider_name,
        result.base_url.as_deref(),
        &result.status,
        &result.message,
    );
    Ok(result)
}

#[tauri::command]
pub async fn stream_check_config_profile(
    id: String,
    app_handle: AppHandle,
    db: State<'_, DbState>,
) -> Result<ProviderStreamCheckResult, String> {
    let (profile, client) = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        let profile = read_all_config_profiles_from_conn(&conn)?
            .into_iter()
            .find(|profile| profile.id == id)
            .ok_or_else(|| format!("Profile not found: {id}"))?;
        let client = build_provider_probe_client(&conn)?;
        (profile, client)
    };

    let checked_at = chrono::Utc::now().to_rfc3339();
    let request = match extract_stream_check_request(&app_handle, &profile).await {
        Ok(request) => request,
        Err(message) => {
            let status =
                if message.contains("not yet supported") || message.contains("not supported") {
                    "unsupported"
                } else {
                    "unconfigured"
                };
            let result = ProviderStreamCheckResult {
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
            log_provider_result(
                "stream-check",
                &result.tool_id,
                &result.provider_name,
                result.base_url.as_deref(),
                &result.status,
                &result.message,
            );
            return Ok(result);
        }
    };
    let StreamCheckRequestSpec {
        endpoint,
        headers,
        body,
    } = request;

    let started_at = std::time::Instant::now();
    let mut request_builder = client
        .post(&endpoint)
        .header("content-type", "application/json")
        .header("accept", "text/event-stream, application/json");
    for (name, value) in headers {
        request_builder = request_builder.header(&name, value);
    }

    let result = match request_builder.json(&body).send().await {
        Ok(mut response) => {
            let latency_ms = started_at.elapsed().as_millis() as u64;
            let http_status = response.status().as_u16();

            if !response.status().is_success() {
                let detail = response.text().await.unwrap_or_default();
                ProviderStreamCheckResult {
                    profile_id: profile.id,
                    tool_id: profile.tool_id,
                    provider_name: profile.name,
                    base_url: Some(endpoint.clone()),
                    status: "reachable".to_string(),
                    latency_ms: Some(latency_ms),
                    http_status: Some(http_status),
                    checked_at,
                    message: if detail.trim().is_empty() {
                        format!("Endpoint responded with HTTP {http_status}")
                    } else {
                        format!(
                            "HTTP {http_status}: {}",
                            detail.chars().take(160).collect::<String>()
                        )
                    },
                }
            } else {
                match tokio::time::timeout(std::time::Duration::from_secs(15), response.chunk()).await {
                    Ok(Ok(Some(chunk))) => ProviderStreamCheckResult {
                        profile_id: profile.id,
                        tool_id: profile.tool_id,
                        provider_name: profile.name,
                        base_url: Some(endpoint.clone()),
                        status: "healthy".to_string(),
                        latency_ms: Some(latency_ms),
                        http_status: Some(http_status),
                        checked_at,
                        message: format!("Received first stream chunk ({} bytes)", chunk.len()),
                    },
                    Ok(Ok(None)) => ProviderStreamCheckResult {
                        profile_id: profile.id,
                        tool_id: profile.tool_id,
                        provider_name: profile.name,
                        base_url: Some(endpoint.clone()),
                        status: "reachable".to_string(),
                        latency_ms: Some(latency_ms),
                        http_status: Some(http_status),
                        checked_at,
                        message: "Stream endpoint closed without returning chunks".to_string(),
                    },
                    Ok(Err(error)) => ProviderStreamCheckResult {
                        profile_id: profile.id,
                        tool_id: profile.tool_id,
                        provider_name: profile.name,
                        base_url: Some(endpoint.clone()),
                        status: "error".to_string(),
                        latency_ms: Some(latency_ms),
                        http_status: Some(http_status),
                        checked_at,
                        message: error.to_string(),
                    },
                    Err(_) => ProviderStreamCheckResult {
                        profile_id: profile.id,
                        tool_id: profile.tool_id,
                        provider_name: profile.name,
                        base_url: Some(endpoint.clone()),
                        status: "reachable".to_string(),
                        latency_ms: Some(latency_ms),
                        http_status: Some(http_status),
                        checked_at,
                        message: "Connected successfully but did not receive a stream chunk within 15 seconds".to_string(),
                    },
                }
            }
        }
        Err(error) => ProviderStreamCheckResult {
            profile_id: profile.id,
            tool_id: profile.tool_id,
            provider_name: profile.name,
            base_url: Some(endpoint),
            status: "error".to_string(),
            latency_ms: None,
            http_status: None,
            checked_at,
            message: error.to_string(),
        },
    };

    log_provider_result(
        "stream-check",
        &result.tool_id,
        &result.provider_name,
        result.base_url.as_deref(),
        &result.status,
        &result.message,
    );
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::push_probe_endpoint;

    #[test]
    fn probe_endpoint_candidates_are_normalized_and_deduplicated() {
        let mut endpoints = Vec::new();
        push_probe_endpoint(&mut endpoints, " https://example.com/ ".to_string());
        push_probe_endpoint(&mut endpoints, "https://example.com".to_string());
        push_probe_endpoint(&mut endpoints, "ftp://example.com".to_string());
        assert_eq!(endpoints, vec!["https://example.com"]);
    }
}
