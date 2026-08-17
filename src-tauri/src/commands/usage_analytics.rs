//! Filterable usage analytics for the dashboard.
//!
//! The existing logs page is intentionally request-oriented. This command
//! provides one compact, read-only aggregate for date, app, provider, and
//! model filters so the UI does not need to issue several expensive queries.

use chrono::{Duration, Utc};
use rusqlite::params;
use serde::Serialize;
use std::collections::BTreeMap;
use tauri::State;

use crate::db::DbState;

#[derive(Debug, Clone, Serialize, Default)]
pub struct UsageAnalyticsSummary {
    pub total_requests: u64,
    pub success_requests: u64,
    pub success_rate: f64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_creation_tokens: u64,
    pub total_cost_usd: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct UsageAnalyticsTrendPoint {
    pub date: String,
    pub requests: u64,
    pub success_requests: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_cost_usd: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct UsageAnalyticsProviderRow {
    pub provider_name: String,
    pub app_id: String,
    pub requests: u64,
    pub success_rate: f64,
    pub total_tokens: u64,
    pub total_cost_usd: String,
    pub avg_latency_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct UsageAnalyticsModelRow {
    pub model: String,
    pub requests: u64,
    pub success_rate: f64,
    pub total_tokens: u64,
    pub total_cost_usd: String,
    pub avg_latency_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct UsageAnalytics {
    pub days: u32,
    pub start_date: String,
    pub end_date: String,
    pub summary: UsageAnalyticsSummary,
    pub trends: Vec<UsageAnalyticsTrendPoint>,
    pub providers: Vec<UsageAnalyticsProviderRow>,
    pub models: Vec<UsageAnalyticsModelRow>,
}

#[derive(Debug, Clone)]
struct RequestSample {
    date: String,
    app_id: String,
    provider_name: String,
    model: String,
    input_tokens: u64,
    output_tokens: u64,
    cache_read_tokens: u64,
    cache_creation_tokens: u64,
    cost: f64,
    latency_ms: u64,
    success: bool,
}

#[derive(Debug, Default)]
struct Aggregate {
    requests: u64,
    success_requests: u64,
    input_tokens: u64,
    output_tokens: u64,
    cache_read_tokens: u64,
    cache_creation_tokens: u64,
    total_cost: f64,
    total_latency_ms: u128,
}

impl Aggregate {
    fn add(&mut self, row: &RequestSample) {
        self.requests += 1;
        if row.success {
            self.success_requests += 1;
        }
        self.input_tokens = self.input_tokens.saturating_add(row.input_tokens);
        self.output_tokens = self.output_tokens.saturating_add(row.output_tokens);
        self.cache_read_tokens = self.cache_read_tokens.saturating_add(row.cache_read_tokens);
        self.cache_creation_tokens = self
            .cache_creation_tokens
            .saturating_add(row.cache_creation_tokens);
        self.total_cost += row.cost;
        self.total_latency_ms += row.latency_ms as u128;
    }

    fn success_rate(&self) -> f64 {
        if self.requests == 0 {
            0.0
        } else {
            self.success_requests as f64 * 100.0 / self.requests as f64
        }
    }

    fn total_tokens(&self) -> u64 {
        self.input_tokens
            .saturating_add(self.output_tokens)
            .saturating_add(self.cache_read_tokens)
            .saturating_add(self.cache_creation_tokens)
    }

    fn avg_latency_ms(&self) -> u64 {
        if self.requests == 0 {
            0
        } else {
            (self.total_latency_ms / self.requests as u128) as u64
        }
    }
}

fn date_bounds(days: Option<u32>) -> (u32, String, String) {
    let days = days.unwrap_or(7).clamp(1, 90);
    let end = Utc::now().date_naive();
    let start = end - Duration::days(i64::from(days.saturating_sub(1)));
    (
        days,
        start.format("%Y-%m-%d").to_string(),
        end.format("%Y-%m-%d").to_string(),
    )
}

fn clean_filter(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
}

fn sample_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<RequestSample> {
    let request_model: Option<String> = row.get(3)?;
    let response_model: Option<String> = row.get(4)?;
    let model = response_model
        .or(request_model)
        .unwrap_or_else(|| "(unknown)".to_string());
    Ok(RequestSample {
        date: row.get(0)?,
        app_id: row.get(1)?,
        provider_name: row.get(2)?,
        model,
        input_tokens: row.get::<_, i64>(5)?.max(0) as u64,
        output_tokens: row.get::<_, i64>(6)?.max(0) as u64,
        cache_read_tokens: row.get::<_, i64>(7)?.max(0) as u64,
        cache_creation_tokens: row.get::<_, i64>(8)?.max(0) as u64,
        cost: row
            .get::<_, String>(9)?
            .parse::<f64>()
            .unwrap_or(0.0)
            .max(0.0),
        latency_ms: row.get::<_, i64>(10)?.max(0) as u64,
        success: row.get::<_, i64>(11)? >= 200 && row.get::<_, i64>(11)? < 300,
    })
}

fn format_cost(value: f64) -> String {
    if value.is_finite() {
        format!("{value:.6}")
    } else {
        "0.000000".to_string()
    }
}

fn trend_from_samples(
    samples: &[RequestSample],
    start_date: &str,
    end_date: &str,
    days: u32,
) -> Vec<UsageAnalyticsTrendPoint> {
    let mut grouped: BTreeMap<String, Aggregate> = BTreeMap::new();
    for sample in samples {
        grouped.entry(sample.date.clone()).or_default().add(sample);
    }
    let start = chrono::NaiveDate::parse_from_str(start_date, "%Y-%m-%d")
        .unwrap_or_else(|_| Utc::now().date_naive());
    let _ = end_date;
    (0..days)
        .map(|offset| {
            let date = (start + Duration::days(i64::from(offset)))
                .format("%Y-%m-%d")
                .to_string();
            let aggregate = grouped.remove(&date).unwrap_or_default();
            UsageAnalyticsTrendPoint {
                date,
                requests: aggregate.requests,
                success_requests: aggregate.success_requests,
                input_tokens: aggregate.input_tokens,
                output_tokens: aggregate.output_tokens,
                total_cost_usd: format_cost(aggregate.total_cost),
            }
        })
        .collect()
}

#[tauri::command(rename_all = "camelCase")]
pub fn get_usage_analytics(
    days: Option<u32>,
    app_id: Option<String>,
    provider_name: Option<String>,
    model: Option<String>,
    db: State<'_, DbState>,
) -> Result<UsageAnalytics, String> {
    let (days, start_date, end_date) = date_bounds(days);
    let start_key = format!("{start_date}T00:00:00+00:00");
    let end_date_value = chrono::NaiveDate::parse_from_str(&end_date, "%Y-%m-%d")
        .map_err(|error| error.to_string())?
        + Duration::days(1);
    let end_key = format!("{}T00:00:00+00:00", end_date_value.format("%Y-%m-%d"));
    let app_filter = clean_filter(app_id);
    let provider_filter = clean_filter(provider_name);
    let model_filter = clean_filter(model);
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    let mut statement = conn
        .prepare(
            "SELECT substr(created_at, 1, 10), tool_id, provider_name, request_model,
                    response_model, input_tokens, output_tokens, cache_read_tokens,
                    cache_creation_tokens, total_cost_usd, latency_ms, status_code
             FROM proxy_request_logs
             WHERE created_at >= ?1
               AND created_at < ?2
               AND (?3 IS NULL OR LOWER(tool_id) = ?3)
               AND (?4 IS NULL OR LOWER(provider_name) = ?4)
               AND (?5 IS NULL OR LOWER(COALESCE(response_model, request_model, '')) = ?5)",
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map(
            params![
                start_key,
                end_key,
                app_filter,
                provider_filter,
                model_filter
            ],
            sample_from_row,
        )
        .map_err(|error| error.to_string())?;
    let samples = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;

    let mut summary = Aggregate::default();
    let mut providers: BTreeMap<(String, String), Aggregate> = BTreeMap::new();
    let mut models: BTreeMap<String, Aggregate> = BTreeMap::new();
    for sample in &samples {
        summary.add(sample);
        providers
            .entry((sample.provider_name.clone(), sample.app_id.clone()))
            .or_default()
            .add(sample);
        models.entry(sample.model.clone()).or_default().add(sample);
    }
    let summary = UsageAnalyticsSummary {
        total_requests: summary.requests,
        success_requests: summary.success_requests,
        success_rate: summary.success_rate(),
        input_tokens: summary.input_tokens,
        output_tokens: summary.output_tokens,
        cache_read_tokens: summary.cache_read_tokens,
        cache_creation_tokens: summary.cache_creation_tokens,
        total_cost_usd: format_cost(summary.total_cost),
    };
    let mut providers = providers
        .into_iter()
        .map(
            |((provider_name, app_id), aggregate)| UsageAnalyticsProviderRow {
                provider_name,
                app_id,
                requests: aggregate.requests,
                success_rate: aggregate.success_rate(),
                total_tokens: aggregate.total_tokens(),
                total_cost_usd: format_cost(aggregate.total_cost),
                avg_latency_ms: aggregate.avg_latency_ms(),
            },
        )
        .collect::<Vec<_>>();
    providers.sort_by(|left, right| {
        right
            .requests
            .cmp(&left.requests)
            .then_with(|| left.provider_name.cmp(&right.provider_name))
    });
    providers.truncate(50);
    let mut models = models
        .into_iter()
        .map(|(model, aggregate)| UsageAnalyticsModelRow {
            model,
            requests: aggregate.requests,
            success_rate: aggregate.success_rate(),
            total_tokens: aggregate.total_tokens(),
            total_cost_usd: format_cost(aggregate.total_cost),
            avg_latency_ms: aggregate.avg_latency_ms(),
        })
        .collect::<Vec<_>>();
    models.sort_by(|left, right| {
        right
            .requests
            .cmp(&left.requests)
            .then_with(|| left.model.cmp(&right.model))
    });
    models.truncate(50);

    Ok(UsageAnalytics {
        days,
        start_date: start_date.clone(),
        end_date: end_date.clone(),
        summary,
        trends: trend_from_samples(&samples, &start_date, &end_date, days),
        providers,
        models,
    })
}

#[cfg(test)]
mod tests {
    use super::{date_bounds, format_cost};

    #[test]
    fn clamps_analytics_range() {
        assert_eq!(date_bounds(Some(0)).0, 1);
        assert_eq!(date_bounds(Some(365)).0, 90);
        assert_eq!(date_bounds(None).0, 7);
    }

    #[test]
    fn formats_non_finite_cost_as_zero() {
        assert_eq!(format_cost(f64::NAN), "0.000000");
        assert_eq!(format_cost(1.25), "1.250000");
    }
}
