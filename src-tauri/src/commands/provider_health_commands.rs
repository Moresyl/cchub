use serde::Serialize;
use tauri::{AppHandle, State};

use crate::commands::extra_commands::{
    build_provider_probe_client, extract_probe_target, read_all_config_profiles_from_conn,
};
use crate::db::DbState;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderHealthItem {
    pub profile_id: String,
    pub provider_name: String,
    pub tool_id: String,
    pub endpoint: Option<String>,
    pub status: String,
    pub latency_ms: Option<u64>,
    pub http_status: Option<u16>,
    pub checked_at: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderStatsItem {
    pub provider_name: String,
    pub tool_id: String,
    pub requests: u64,
    pub successful_requests: u64,
    pub success_rate: f64,
    pub average_latency_ms: f64,
    pub last_request_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelStatsItem {
    pub model_id: String,
    pub requests: u64,
    pub successful_requests: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_cost_usd: String,
}

#[tauri::command]
pub async fn get_provider_health(
    provider_id: Option<String>,
    app_handle: AppHandle,
    db: State<'_, DbState>,
) -> Result<Vec<ProviderHealthItem>, String> {
    let (profiles, client) = {
        let conn = db.0.lock().map_err(|error| error.to_string())?;
        let mut profiles = read_all_config_profiles_from_conn(&conn)?;
        if let Some(provider_id) = provider_id
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
        {
            profiles.retain(|profile| profile.id == provider_id || profile.name == provider_id);
        }
        (profiles, build_provider_probe_client(&conn)?)
    };

    let mut results = Vec::with_capacity(profiles.len());
    for profile in profiles {
        let checked_at = chrono::Utc::now().to_rfc3339();
        let (endpoint, headers) = match extract_probe_target(&app_handle, &profile).await {
            Ok(value) => value,
            Err(message) => {
                results.push(ProviderHealthItem {
                    profile_id: profile.id,
                    provider_name: profile.name,
                    tool_id: profile.tool_id,
                    endpoint: None,
                    status: "unconfigured".to_string(),
                    latency_ms: None,
                    http_status: None,
                    checked_at,
                    message,
                });
                continue;
            }
        };
        let Some(endpoint) = endpoint else {
            results.push(ProviderHealthItem {
                profile_id: profile.id,
                provider_name: profile.name,
                tool_id: profile.tool_id,
                endpoint: None,
                status: "unconfigured".to_string(),
                latency_ms: None,
                http_status: None,
                checked_at,
                message: "No provider endpoint configured".to_string(),
            });
            continue;
        };

        let started_at = std::time::Instant::now();
        let mut request = client.head(&endpoint);
        for (name, value) in headers {
            request = request.header(name, value);
        }
        let response = request.send().await;
        let latency_ms = started_at.elapsed().as_millis() as u64;
        match response {
            Ok(response) => {
                let http_status = response.status().as_u16();
                let status = if response.status().is_success() {
                    "healthy"
                } else if response.status().is_client_error() {
                    "reachable"
                } else {
                    "degraded"
                };
                results.push(ProviderHealthItem {
                    profile_id: profile.id,
                    provider_name: profile.name,
                    tool_id: profile.tool_id,
                    endpoint: Some(endpoint),
                    status: status.to_string(),
                    latency_ms: Some(latency_ms),
                    http_status: Some(http_status),
                    checked_at,
                    message: format!("Provider endpoint returned HTTP {http_status}"),
                });
            }
            Err(error) => results.push(ProviderHealthItem {
                profile_id: profile.id,
                provider_name: profile.name,
                tool_id: profile.tool_id,
                endpoint: Some(endpoint),
                status: "offline".to_string(),
                latency_ms: Some(latency_ms),
                http_status: None,
                checked_at,
                message: error.to_string(),
            }),
        }
    }
    Ok(results)
}

#[tauri::command]
pub fn get_provider_stats(db: State<'_, DbState>) -> Result<Vec<ProviderStatsItem>, String> {
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    let mut statement = conn
        .prepare(
            "SELECT provider_name, tool_id, COUNT(*),
                    SUM(CASE WHEN status_code BETWEEN 200 AND 299 THEN 1 ELSE 0 END),
                    AVG(latency_ms), MAX(created_at)
             FROM proxy_request_logs
             GROUP BY provider_name, tool_id
             ORDER BY COUNT(*) DESC, provider_name ASC",
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], |row| {
            let requests = row.get::<_, i64>(2)?.max(0) as u64;
            let successful_requests = row.get::<_, i64>(3)?.max(0) as u64;
            Ok(ProviderStatsItem {
                provider_name: row.get(0)?,
                tool_id: row.get(1)?,
                requests,
                successful_requests,
                success_rate: if requests == 0 {
                    0.0
                } else {
                    successful_requests as f64 * 100.0 / requests as f64
                },
                average_latency_ms: row.get::<_, Option<f64>>(4)?.unwrap_or(0.0),
                last_request_at: row.get(5)?,
            })
        })
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn get_model_stats(db: State<'_, DbState>) -> Result<Vec<ModelStatsItem>, String> {
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    let mut statement = conn
        .prepare(
            "SELECT COALESCE(request_model, response_model, 'unknown'), COUNT(*),
                    SUM(CASE WHEN status_code BETWEEN 200 AND 299 THEN 1 ELSE 0 END),
                    SUM(input_tokens), SUM(output_tokens), SUM(CAST(total_cost_usd AS REAL))
             FROM proxy_request_logs
             GROUP BY COALESCE(request_model, response_model, 'unknown')
             ORDER BY COUNT(*) DESC, 1 ASC",
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], |row| {
            Ok(ModelStatsItem {
                model_id: row.get(0)?,
                requests: row.get::<_, i64>(1)?.max(0) as u64,
                successful_requests: row.get::<_, i64>(2)?.max(0) as u64,
                input_tokens: row.get::<_, i64>(3)?.max(0) as u64,
                output_tokens: row.get::<_, i64>(4)?.max(0) as u64,
                total_cost_usd: format!("{:.6}", row.get::<_, Option<f64>>(5)?.unwrap_or(0.0)),
            })
        })
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}
