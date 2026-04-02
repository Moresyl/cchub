use crate::db::DbState;
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tauri::State;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyUsageSummary {
    pub total_requests: u64,
    pub success_requests: u64,
    pub success_rate: f64,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_cache_read_tokens: u64,
    pub total_cache_creation_tokens: u64,
    pub total_cost_usd: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyRequestLogRow {
    pub request_id: String,
    pub tool_id: String,
    pub profile_id: String,
    pub provider_name: String,
    pub request_model: Option<String>,
    pub response_model: Option<String>,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_creation_tokens: u64,
    pub total_cost_usd: String,
    pub latency_ms: u64,
    pub status_code: u16,
    pub is_streaming: bool,
    pub error_message: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ProxyRequestLogFilters {
    pub limit: Option<u32>,
    pub tool_id: Option<String>,
    pub provider_query: String,
    pub model_query: String,
    pub status: String,
    pub stream_mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyUsageTrendPoint {
    pub date: String,
    pub requests: u64,
    pub success_requests: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_cost_usd: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPricingRow {
    pub model_id: String,
    pub normalized_model_id: String,
    pub input_cost_per_million: String,
    pub output_cost_per_million: String,
    pub cache_read_cost_per_million: String,
    pub cache_write_cost_per_million: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ModelPricingInput {
    pub model_id: String,
    pub input_cost_per_million: String,
    pub output_cost_per_million: String,
    pub cache_read_cost_per_million: String,
    pub cache_write_cost_per_million: String,
}

fn map_proxy_request_log_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ProxyRequestLogRow> {
    Ok(ProxyRequestLogRow {
        request_id: row.get(0)?,
        tool_id: row.get(1)?,
        profile_id: row.get(2)?,
        provider_name: row.get(3)?,
        request_model: row.get(4)?,
        response_model: row.get(5)?,
        input_tokens: row.get::<_, i64>(6)?.max(0) as u64,
        output_tokens: row.get::<_, i64>(7)?.max(0) as u64,
        cache_read_tokens: row.get::<_, i64>(8)?.max(0) as u64,
        cache_creation_tokens: row.get::<_, i64>(9)?.max(0) as u64,
        total_cost_usd: row.get(10)?,
        latency_ms: row.get::<_, i64>(11)?.max(0) as u64,
        status_code: row.get::<_, i64>(12)?.clamp(0, u16::MAX as i64) as u16,
        is_streaming: row.get::<_, i64>(13)? != 0,
        error_message: row.get(14)?,
        created_at: row.get(15)?,
    })
}

fn map_model_pricing_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ModelPricingRow> {
    Ok(ModelPricingRow {
        model_id: row.get(0)?,
        normalized_model_id: row.get(1)?,
        input_cost_per_million: row.get(2)?,
        output_cost_per_million: row.get(3)?,
        cache_read_cost_per_million: row.get(4)?,
        cache_write_cost_per_million: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

fn normalize_model_pricing_id(model_id: &str) -> String {
    let trimmed = model_id.trim().to_ascii_lowercase();
    if trimmed.is_empty() {
        return String::new();
    }

    let normalized = trimmed.replace('@', "-");
    let without_prefix = normalized.strip_prefix("models/").unwrap_or(&normalized);
    let preferred = without_prefix
        .rsplit('/')
        .next()
        .unwrap_or(without_prefix)
        .split(':')
        .next()
        .unwrap_or(without_prefix)
        .trim();

    if preferred.is_empty() {
        without_prefix.to_string()
    } else {
        preferred.to_string()
    }
}

fn normalize_cost_text(value: &str) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok("0".to_string());
    }
    let parsed = trimmed
        .parse::<f64>()
        .map_err(|_| format!("Invalid cost value: {trimmed}"))?;
    if !parsed.is_finite() || parsed < 0.0 {
        return Err(format!("Invalid cost value: {trimmed}"));
    }
    Ok(format!("{parsed:.6}"))
}

#[tauri::command]
pub fn get_proxy_usage_summary(db: State<'_, DbState>) -> Result<ProxyUsageSummary, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;

    conn.query_row(
        "SELECT
            COUNT(*) AS total_requests,
            COALESCE(SUM(CASE WHEN status_code >= 200 AND status_code < 300 THEN 1 ELSE 0 END), 0) AS success_requests,
            COALESCE(SUM(input_tokens), 0) AS total_input_tokens,
            COALESCE(SUM(output_tokens), 0) AS total_output_tokens,
            COALESCE(SUM(cache_read_tokens), 0) AS total_cache_read_tokens,
            COALESCE(SUM(cache_creation_tokens), 0) AS total_cache_creation_tokens,
            COALESCE(SUM(CAST(total_cost_usd AS REAL)), 0) AS total_cost_usd
         FROM proxy_request_logs",
        [],
        |row| {
            let total_requests: i64 = row.get(0)?;
            let success_requests: i64 = row.get(1)?;
            let total_input_tokens: i64 = row.get(2)?;
            let total_output_tokens: i64 = row.get(3)?;
            let total_cache_read_tokens: i64 = row.get(4)?;
            let total_cache_creation_tokens: i64 = row.get(5)?;
            let total_cost_usd: f64 = row.get(6)?;

            let success_rate = if total_requests > 0 {
                success_requests as f64 * 100.0 / total_requests as f64
            } else {
                0.0
            };

            Ok(ProxyUsageSummary {
                total_requests: total_requests.max(0) as u64,
                success_requests: success_requests.max(0) as u64,
                success_rate,
                total_input_tokens: total_input_tokens.max(0) as u64,
                total_output_tokens: total_output_tokens.max(0) as u64,
                total_cache_read_tokens: total_cache_read_tokens.max(0) as u64,
                total_cache_creation_tokens: total_cache_creation_tokens.max(0) as u64,
                total_cost_usd: format!("{total_cost_usd:.6}"),
            })
        },
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_recent_proxy_request_logs(
    limit: Option<u32>,
    db: State<'_, DbState>,
) -> Result<Vec<ProxyRequestLogRow>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let limit = limit.unwrap_or(12).clamp(1, 100) as i64;
    let mut stmt = conn
        .prepare(
            "SELECT
                request_id,
                tool_id,
                profile_id,
                provider_name,
                request_model,
                response_model,
                input_tokens,
                output_tokens,
                cache_read_tokens,
                cache_creation_tokens,
                total_cost_usd,
                latency_ms,
                status_code,
                is_streaming,
                error_message,
                created_at
             FROM proxy_request_logs
             ORDER BY created_at DESC
             LIMIT ?1",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map(rusqlite::params![limit], map_proxy_request_log_row)
        .map_err(|e| e.to_string())?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn search_proxy_request_logs(
    filters: Option<ProxyRequestLogFilters>,
    db: State<'_, DbState>,
) -> Result<Vec<ProxyRequestLogRow>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let filters = filters.unwrap_or_default();
    let limit = filters.limit.unwrap_or(80).clamp(1, 200) as i64;
    let tool_id = filters
        .tool_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);
    let provider_query = {
        let value = filters.provider_query.trim().to_ascii_lowercase();
        if value.is_empty() {
            None
        } else {
            Some(format!("%{value}%"))
        }
    };
    let model_query = {
        let value = filters.model_query.trim().to_ascii_lowercase();
        if value.is_empty() {
            None
        } else {
            Some(format!("%{value}%"))
        }
    };
    let status = match filters.status.as_str() {
        "success" | "error" => Some(filters.status),
        _ => None,
    };
    let stream_mode = match filters.stream_mode.as_str() {
        "streaming" | "non_streaming" => Some(filters.stream_mode),
        _ => None,
    };

    let mut stmt = conn
        .prepare(
            "SELECT
                request_id,
                tool_id,
                profile_id,
                provider_name,
                request_model,
                response_model,
                input_tokens,
                output_tokens,
                cache_read_tokens,
                cache_creation_tokens,
                total_cost_usd,
                latency_ms,
                status_code,
                is_streaming,
                error_message,
                created_at
             FROM proxy_request_logs
             WHERE (?1 IS NULL OR tool_id = ?1)
               AND (?2 IS NULL OR LOWER(provider_name) LIKE ?2)
               AND (
                    ?3 IS NULL
                    OR LOWER(COALESCE(request_model, '')) LIKE ?3
                    OR LOWER(COALESCE(response_model, '')) LIKE ?3
               )
               AND (
                    ?4 IS NULL
                    OR (?4 = 'success' AND status_code >= 200 AND status_code < 300)
                    OR (?4 = 'error' AND (status_code < 200 OR status_code >= 300))
               )
               AND (
                    ?5 IS NULL
                    OR (?5 = 'streaming' AND is_streaming = 1)
                    OR (?5 = 'non_streaming' AND is_streaming = 0)
               )
             ORDER BY created_at DESC
             LIMIT ?6",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map(
            rusqlite::params![
                tool_id,
                provider_query,
                model_query,
                status,
                stream_mode,
                limit,
            ],
            map_proxy_request_log_row,
        )
        .map_err(|e| e.to_string())?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_proxy_usage_trend(
    days: Option<u32>,
    db: State<'_, DbState>,
) -> Result<Vec<ProxyUsageTrendPoint>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let days = days.unwrap_or(7).clamp(3, 30) as i64;
    let end = Utc::now().date_naive();
    let start = end - Duration::days(days - 1);
    let start_key = start.format("%Y-%m-%d").to_string();

    let mut stmt = conn
        .prepare(
            "SELECT
                day,
                COALESCE(SUM(total_requests), 0) AS request_count,
                COALESCE(SUM(success_requests), 0) AS success_count,
                COALESCE(SUM(total_input_tokens), 0) AS input_tokens,
                COALESCE(SUM(total_output_tokens), 0) AS output_tokens,
                COALESCE(SUM(CAST(total_cost_usd AS REAL)), 0) AS total_cost_usd
             FROM proxy_usage_daily_rollups
             WHERE day >= ?1
             GROUP BY day
             ORDER BY day ASC",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map(rusqlite::params![start_key], |row| {
            let total_cost_usd: f64 = row.get(5)?;
            Ok(ProxyUsageTrendPoint {
                date: row.get(0)?,
                requests: row.get::<_, i64>(1)?.max(0) as u64,
                success_requests: row.get::<_, i64>(2)?.max(0) as u64,
                input_tokens: row.get::<_, i64>(3)?.max(0) as u64,
                output_tokens: row.get::<_, i64>(4)?.max(0) as u64,
                total_cost_usd: format!("{total_cost_usd:.6}"),
            })
        })
        .map_err(|e| e.to_string())?;

    let mut by_date = HashMap::new();
    for item in rows {
        let point = item.map_err(|e| e.to_string())?;
        by_date.insert(point.date.clone(), point);
    }

    let mut trend = Vec::with_capacity(days as usize);
    for offset in 0..days {
        let date = (start + Duration::days(offset))
            .format("%Y-%m-%d")
            .to_string();
        trend.push(by_date.remove(&date).unwrap_or(ProxyUsageTrendPoint {
            date,
            requests: 0,
            success_requests: 0,
            input_tokens: 0,
            output_tokens: 0,
            total_cost_usd: "0.000000".to_string(),
        }));
    }

    Ok(trend)
}

#[tauri::command]
pub fn list_model_pricing(db: State<'_, DbState>) -> Result<Vec<ModelPricingRow>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT
                model_id,
                normalized_model_id,
                input_cost_per_million,
                output_cost_per_million,
                cache_read_cost_per_million,
                cache_write_cost_per_million,
                created_at,
                updated_at
             FROM model_pricing
             ORDER BY updated_at DESC, model_id ASC",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([], map_model_pricing_row)
        .map_err(|e| e.to_string())?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_model_pricing(
    entry: ModelPricingInput,
    db: State<'_, DbState>,
) -> Result<ModelPricingRow, String> {
    let model_id = entry.model_id.trim();
    if model_id.is_empty() {
        return Err("Model ID is required".to_string());
    }

    let normalized_model_id = normalize_model_pricing_id(model_id);
    if normalized_model_id.is_empty() {
        return Err("Model ID is required".to_string());
    }

    let input_cost_per_million = normalize_cost_text(&entry.input_cost_per_million)?;
    let output_cost_per_million = normalize_cost_text(&entry.output_cost_per_million)?;
    let cache_read_cost_per_million = normalize_cost_text(&entry.cache_read_cost_per_million)?;
    let cache_write_cost_per_million = normalize_cost_text(&entry.cache_write_cost_per_million)?;
    let now = Utc::now().to_rfc3339();

    let conn = db.0.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO model_pricing (
            model_id,
            normalized_model_id,
            input_cost_per_million,
            output_cost_per_million,
            cache_read_cost_per_million,
            cache_write_cost_per_million,
            created_at,
            updated_at
        ) VALUES (
            ?1,
            ?2,
            ?3,
            ?4,
            ?5,
            ?6,
            COALESCE((SELECT created_at FROM model_pricing WHERE model_id = ?1), ?7),
            ?7
        )
        ON CONFLICT(model_id) DO UPDATE SET
            normalized_model_id = excluded.normalized_model_id,
            input_cost_per_million = excluded.input_cost_per_million,
            output_cost_per_million = excluded.output_cost_per_million,
            cache_read_cost_per_million = excluded.cache_read_cost_per_million,
            cache_write_cost_per_million = excluded.cache_write_cost_per_million,
            updated_at = excluded.updated_at",
        rusqlite::params![
            model_id,
            normalized_model_id,
            input_cost_per_million,
            output_cost_per_million,
            cache_read_cost_per_million,
            cache_write_cost_per_million,
            now,
        ],
    )
    .map_err(|e| e.to_string())?;

    conn.query_row(
        "SELECT
            model_id,
            normalized_model_id,
            input_cost_per_million,
            output_cost_per_million,
            cache_read_cost_per_million,
            cache_write_cost_per_million,
            created_at,
            updated_at
         FROM model_pricing
         WHERE model_id = ?1",
        rusqlite::params![model_id],
        map_model_pricing_row,
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_model_pricing(model_id: String, db: State<'_, DbState>) -> Result<(), String> {
    let trimmed = model_id.trim();
    if trimmed.is_empty() {
        return Err("Model ID is required".to_string());
    }

    let conn = db.0.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "DELETE FROM model_pricing WHERE model_id = ?1",
        rusqlite::params![trimmed],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
