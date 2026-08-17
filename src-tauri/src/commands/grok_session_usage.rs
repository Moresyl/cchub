use chrono::{DateTime, Utc};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

use crate::commands::session_usage_compat::SessionSyncResult;
use rusqlite::Connection;

const MAX_FILE_BYTES: u64 = 50 * 1024 * 1024;
const MAX_DEPTH: usize = 16;
const SETTLE_WINDOW_SECONDS: i64 = 60;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Counters {
    input: u64,
    output: u64,
    cached: u64,
    latency_ms: u64,
    cost_ticks: u64,
    cost_partial: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct UsageEvent {
    timestamp: i64,
    prompt_id: String,
    cost_partial: bool,
    models: Vec<(String, Counters)>,
}

fn number(value: Option<&Value>) -> u64 {
    value
        .and_then(|value| {
            value
                .as_u64()
                .or_else(|| value.as_i64().and_then(|number| u64::try_from(number).ok()))
        })
        .unwrap_or(0)
}

fn parse_timestamp(value: Option<&Value>) -> Option<i64> {
    let value = value?;
    if let Some(number) = value.as_i64() {
        return Some(if number > 100_000_000_000 {
            number / 1000
        } else {
            number
        });
    }
    value
        .as_str()
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .map(|value| value.timestamp())
}

fn parse_counters(value: &Value) -> Counters {
    Counters {
        input: number(value.get("inputTokens")),
        output: number(value.get("outputTokens")),
        cached: number(value.get("cachedReadTokens")),
        latency_ms: number(value.get("apiDurationMs")),
        cost_ticks: number(value.get("costUsdTicks")),
        cost_partial: value
            .get("costIsPartial")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    }
}

fn parse_events(content: &str) -> Vec<UsageEvent> {
    let mut events = Vec::new();
    for line in content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        let Ok(record) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if record.get("method").and_then(Value::as_str) != Some("_x.ai/session/update") {
            continue;
        }
        let Some(update) = record.get("params").and_then(|params| params.get("update")) else {
            continue;
        };
        let kind = update.get("sessionUpdate").and_then(Value::as_str);
        if kind.is_some() && kind != Some("turn_completed") {
            continue;
        }
        let Some(usage) = update.get("usage").filter(|value| value.is_object()) else {
            continue;
        };
        let Some(timestamp) = parse_timestamp(record.get("timestamp")) else {
            continue;
        };
        let prompt_id = update
            .get("prompt_id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim()
            .to_string();
        let mut models = usage
            .get("modelUsage")
            .and_then(Value::as_object)
            .map(|models| {
                models
                    .iter()
                    .map(|(model, value)| (model.clone(), parse_counters(value)))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if models.is_empty() {
            models.push(("unknown".to_string(), parse_counters(usage)));
        }
        models.sort_by(|left, right| left.0.cmp(&right.0));
        events.push(UsageEvent {
            timestamp,
            prompt_id,
            cost_partial: usage
                .get("costIsPartial")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            models,
        });
    }
    events
}

fn collect_files(root: &Path, files: &mut Vec<PathBuf>, depth: usize) {
    if depth > MAX_DEPTH {
        return;
    }
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            collect_files(&path, files, depth + 1);
        } else if file_type.is_file()
            && path.file_name().and_then(|value| value.to_str()) == Some("updates.jsonl")
        {
            files.push(path);
        }
    }
}

fn session_files() -> Vec<PathBuf> {
    let Some(home) = dirs::home_dir() else {
        return Vec::new();
    };
    let mut files = Vec::new();
    for root in [
        home.join(".grok").join("sessions"),
        home.join(".grok").join("archived_sessions"),
    ] {
        collect_files(&root, &mut files, 0);
    }
    files.sort();
    files
}

fn model_costs(conn: &Connection, model: &str) -> Option<[f64; 4]> {
    let normalized = model.trim().to_ascii_lowercase();
    conn.query_row(
        "SELECT input_cost_per_million, output_cost_per_million,
                cache_read_cost_per_million, cache_write_cost_per_million
         FROM model_pricing
         WHERE model_id = ?1 OR normalized_model_id = ?1
         ORDER BY CASE WHEN model_id = ?1 THEN 0 ELSE 1 END
         LIMIT 1",
        rusqlite::params![normalized],
        |row| {
            Ok([
                row.get::<_, String>(0)?.parse::<f64>().unwrap_or(0.0),
                row.get::<_, String>(1)?.parse::<f64>().unwrap_or(0.0),
                row.get::<_, String>(2)?.parse::<f64>().unwrap_or(0.0),
                row.get::<_, String>(3)?.parse::<f64>().unwrap_or(0.0),
            ])
        },
    )
    .ok()
}

fn cost_for(conn: &Connection, model: &str, counters: Counters) -> (f64, f64, f64, f64, f64) {
    let reported =
        (counters.cost_ticks > 0).then_some(counters.cost_ticks as f64 / 10_000_000_000.0);
    if let Some([input_rate, output_rate, cache_rate, write_rate]) = model_costs(conn, model) {
        let uncached_input = counters.input.saturating_sub(counters.cached) as f64;
        let input = uncached_input * input_rate / 1_000_000.0;
        let output = counters.output as f64 * output_rate / 1_000_000.0;
        let cache = counters.cached as f64 * cache_rate / 1_000_000.0;
        let write = 0.0_f64 * write_rate;
        let total = if reported.is_some() && !(counters.cost_partial) {
            reported.unwrap_or(0.0)
        } else {
            input + output + cache + write
        };
        return (input, output, cache, write, total);
    }
    (0.0, 0.0, 0.0, 0.0, reported.unwrap_or(0.0))
}

fn session_id(path: &Path) -> String {
    path.parent()
        .and_then(Path::file_name)
        .and_then(|value| value.to_str())
        .unwrap_or("unknown")
        .to_string()
}

fn insert_event(
    conn: &Connection,
    request_id: &str,
    session: &str,
    model: &str,
    counters: Counters,
    timestamp: i64,
) -> Result<bool, String> {
    if counters.input == 0 && counters.output == 0 && counters.cached == 0 {
        return Ok(false);
    }
    let (input_cost, output_cost, cache_cost, write_cost, total_cost) =
        cost_for(conn, model, counters);
    let created_at = DateTime::<Utc>::from_timestamp(timestamp, 0)
        .map(|value| value.to_rfc3339())
        .unwrap_or_else(|| Utc::now().to_rfc3339());
    let changed = conn
        .execute(
            "INSERT INTO proxy_request_logs (
                request_id, tool_id, profile_id, provider_name, request_model,
                response_model, input_tokens, output_tokens, cache_read_tokens,
                cache_creation_tokens, total_cost_usd, latency_ms, status_code,
                is_streaming, error_message, created_at
             ) VALUES (?1, 'grokbuild', ?2, 'Grok Build (Session)', ?3, ?3,
                       ?4, ?5, ?6, 0, ?7, ?8, 200, 1, NULL, ?9)
             ON CONFLICT(request_id) DO UPDATE SET
                request_model = excluded.request_model,
                response_model = excluded.response_model,
                input_tokens = excluded.input_tokens,
                output_tokens = excluded.output_tokens,
                cache_read_tokens = excluded.cache_read_tokens,
                total_cost_usd = excluded.total_cost_usd,
                latency_ms = excluded.latency_ms
             WHERE tool_id = 'grokbuild'
               AND provider_name = 'Grok Build (Session)'
               AND (request_model <> excluded.request_model
                 OR response_model <> excluded.response_model
                 OR input_tokens <> excluded.input_tokens
                 OR output_tokens <> excluded.output_tokens
                 OR cache_read_tokens <> excluded.cache_read_tokens
                 OR total_cost_usd <> excluded.total_cost_usd
                 OR latency_ms <> excluded.latency_ms)",
            rusqlite::params![
                request_id,
                format!("grok-session:{session}"),
                model,
                counters.input as i64,
                counters.output as i64,
                counters.cached as i64,
                format!("{total_cost:.6}"),
                counters.latency_ms.min(i64::MAX as u64) as i64,
                created_at,
            ],
        )
        .map_err(|error| error.to_string())?;
    let _ = (input_cost, output_cost, cache_cost, write_cost);
    Ok(changed > 0)
}

fn has_recent_proxy_activity(conn: &Connection, timestamp: i64) -> Result<bool, String> {
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM proxy_request_logs
             WHERE tool_id = 'grokbuild'
               AND provider_name <> 'Grok Build (Session)'
               AND ABS(COALESCE(CAST(strftime('%s', created_at) AS INTEGER), 0) - ?1) <= ?2",
            rusqlite::params![timestamp, SETTLE_WINDOW_SECONDS],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    Ok(count > 0)
}

pub fn sync_grok_usage(conn: &mut Connection) -> Result<SessionSyncResult, String> {
    let files = session_files();
    let mut result = SessionSyncResult {
        files_scanned: files.len() as u32,
        ..Default::default()
    };
    let now = Utc::now().timestamp();
    for path in files {
        let metadata = match fs::metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) => {
                result.errors.push(format!("{}: {error}", path.display()));
                continue;
            }
        };
        if metadata.len() > MAX_FILE_BYTES {
            result.deferred_files = result.deferred_files.saturating_add(1);
            continue;
        }
        let content = match fs::read_to_string(&path) {
            Ok(content) => content,
            Err(error) => {
                result.errors.push(format!("{}: {error}", path.display()));
                continue;
            }
        };
        let session = session_id(&path);
        let mut deferred = false;
        for (index, event) in parse_events(&content).into_iter().enumerate() {
            if now.saturating_sub(event.timestamp) < SETTLE_WINDOW_SECONDS {
                deferred = true;
                break;
            }
            if has_recent_proxy_activity(conn, event.timestamp)? {
                result.skipped = result.skipped.saturating_add(event.models.len() as u32);
                continue;
            }
            let turn = if event.prompt_id.is_empty() {
                format!("index-{index}")
            } else {
                event.prompt_id.clone()
            };
            for (model, mut counters) in event.models {
                counters.cost_partial |= event.cost_partial;
                let request_id = format!("grok-session:{session}:{turn}:{model}");
                match insert_event(
                    conn,
                    &request_id,
                    &session,
                    &model,
                    counters,
                    event.timestamp,
                ) {
                    Ok(true) => result.imported = result.imported.saturating_add(1),
                    Ok(false) => result.skipped = result.skipped.saturating_add(1),
                    Err(error) => result.errors.push(format!("{request_id}: {error}")),
                }
            }
        }
        if deferred {
            result.deferred_files = result.deferred_files.saturating_add(1);
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::{insert_event, parse_events, parse_timestamp, Counters};
    use rusqlite::Connection;
    use serde_json::json;

    #[test]
    fn parses_turn_completed_model_usage_and_ignores_noise() {
        let content = concat!(
            "not json\n",
            "{\"method\":\"_x.ai/session/update\",\"params\":{\"update\":{\"sessionUpdate\":\"usage_snapshot\",\"usage\":{\"inputTokens\":99}}}}\n",
            "{\"timestamp\":1720000000,\"method\":\"_x.ai/session/update\",\"params\":{\"update\":{\"sessionUpdate\":\"turn_completed\",\"prompt_id\":\"p1\",\"usage\":{\"modelUsage\":{\"grok-4.5-build\":{\"inputTokens\":10,\"outputTokens\":2,\"cachedReadTokens\":3,\"costUsdTicks\":1000}}}}}}\n"
        );
        let events = parse_events(content);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].prompt_id, "p1");
        assert_eq!(events[0].models[0].1.input, 10);
        assert_eq!(events[0].models[0].1.cached, 3);
    }

    #[test]
    fn accepts_epoch_millis_and_rfc3339_timestamps() {
        assert_eq!(
            parse_timestamp(Some(&json!(1_720_000_000_000_i64))),
            Some(1_720_000_000)
        );
        assert!(parse_timestamp(Some(&json!("2026-01-01T00:00:00Z"))).is_some());
    }

    #[test]
    fn repeated_session_event_is_idempotent() {
        let conn = Connection::open_in_memory().expect("open test database");
        crate::db::schema::run_migrations(&conn).expect("create test schema");
        let counters = Counters {
            input: 100,
            output: 20,
            cached: 5,
            latency_ms: 12,
            ..Default::default()
        };
        assert!(insert_event(
            &conn,
            "grok-session:test:p1:model",
            "test",
            "grok-4.5-build",
            counters,
            1_720_000_000
        )
        .expect("insert event"));
        assert!(!insert_event(
            &conn,
            "grok-session:test:p1:model",
            "test",
            "grok-4.5-build",
            counters,
            1_720_000_000
        )
        .expect("repeat event"));
    }
}
