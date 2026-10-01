use crate::db::DbState;
use crate::provider_proxy::{ProxyRequestInsights, ProxyUsageMetrics, UpstreamTarget};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

const MAX_ATTEMPTS: usize = 256;
const MAX_LEDGER_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamAttempt {
    pub attempt_id: String,
    pub profile_id: String,
    pub provider_name: String,
    pub model: Option<String>,
    pub response_model: Option<String>,
    pub status_code: u16,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_creation_tokens: u64,
    pub total_cost_usd: String,
}

#[allow(clippy::too_many_arguments)]
pub(in crate::provider_proxy) fn record_stream_attempt<R: tauri::Runtime>(
    app: &AppHandle<R>,
    request_id: &str,
    upstream: &UpstreamTarget,
    insights: &ProxyRequestInsights,
    status: u16,
    usage: Option<&ProxyUsageMetrics>,
) -> Result<(), String> {
    let Some(usage) = usage else {
        return Ok(());
    };
    let db = app.state::<DbState>();
    let conn =
        db.0.lock()
            .map_err(|_| "Could not preserve failed stream accounting")?;
    let result = (|| -> Result<(), String> {
        let tx = conn
            .unchecked_transaction()
            .map_err(|_| "Could not begin stream accounting")?;
        let source: String = tx
            .query_row(
                "SELECT stream_attempts_json FROM proxy_request_logs WHERE request_id=?1",
                [request_id],
                |row| row.get(0),
            )
            .map_err(|_| "Could not read stream accounting")?;
        if source.len() > MAX_LEDGER_BYTES {
            return Err("Stream accounting limit reached".into());
        }
        let mut attempts: Vec<StreamAttempt> =
            serde_json::from_str(&source).map_err(|_| "Invalid stream accounting")?;
        if attempts.len() >= MAX_ATTEMPTS {
            return Err("Stream attempt limit reached".into());
        }
        let cost = super::calculate_proxy_total_cost(&tx, upstream, insights, usage);
        if !cost.is_finite() || cost < 0.0 {
            return Err("Invalid stream attempt cost".into());
        }
        attempts.push(StreamAttempt {
            attempt_id: uuid::Uuid::new_v4().to_string(),
            profile_id: upstream.profile_id.clone(),
            provider_name: upstream.profile_name.clone(),
            model: insights.sent_model().map(str::to_owned),
            response_model: usage.response_model.clone(),
            status_code: status,
            input_tokens: usage.total_input_tokens(),
            output_tokens: usage.output_tokens,
            cache_read_tokens: usage.cache_read_tokens,
            cache_creation_tokens: usage.cache_creation_tokens,
            total_cost_usd: format!("{cost:.6}"),
        });
        let serialized =
            serde_json::to_string(&attempts).map_err(|_| "Could not encode stream accounting")?;
        if serialized.len() > MAX_LEDGER_BYTES {
            return Err("Stream accounting limit reached".into());
        }
        tx.execute(
            "UPDATE proxy_request_logs SET stream_attempts_json=?2 WHERE request_id=?1",
            rusqlite::params![request_id, serialized],
        )
        .map_err(|_| "Could not preserve stream accounting")?;
        tx.commit()
            .map_err(|_| "Could not commit stream accounting".to_string())
    })();
    result.map_err(|_| {
        "Failed stream accounting could not be retained; no further provider was requested".into()
    })
}
