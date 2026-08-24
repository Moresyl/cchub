//! Durable Pi session accounting.
//!
//! Pi can rewrite a JSONL when a user branches a session.  A line-number based
//! identity therefore double-counts old turns after the rewrite.  This importer
//! derives stable entry/semantic identities and records them in an append-only
//! ledger that survives normal request-log cleanup.

use chrono::{DateTime, Utc};
use rusqlite::{Connection, OptionalExtension};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use crate::commands::session_usage_compat::SessionSyncResult;

const DATA_SOURCE: &str = "pi_session";
const MAX_FILE_BYTES: u64 = 128 * 1024 * 1024;
const MAX_FILES: usize = 20_000;
const MAX_DEPTH: usize = 8;
const MAX_ENTRIES: usize = 500_000;
const MAX_LABEL_CHARS: usize = 512;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct Costs {
    input: f64,
    output: f64,
    cache_read: f64,
    cache_write: f64,
    total: f64,
}

impl Costs {
    fn reported_total(self) -> Option<f64> {
        let components = self.input + self.output + self.cache_read + self.cache_write;
        let total = if self.total > 0.0 {
            self.total
        } else {
            components
        };
        total
            .is_finite()
            .then_some(total)
            .filter(|value| *value > 0.0)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Tokens {
    input: u64,
    output: u64,
    cache_read: u64,
    cache_write: u64,
}

impl Tokens {
    fn has_values(self) -> bool {
        self.input > 0 || self.output > 0 || self.cache_read > 0 || self.cache_write > 0
    }
}

#[derive(Debug)]
struct PiRecord {
    request_id: String,
    semantic_id: String,
    has_entry_id: bool,
    session_id: String,
    provider: String,
    model: String,
    request_model: String,
    tokens: Tokens,
    costs: Costs,
    status_code: i64,
    error: Option<String>,
    created_at: String,
}

fn number(value: Option<&Value>) -> u64 {
    value
        .and_then(|value| {
            value
                .as_u64()
                .or_else(|| value.as_i64().and_then(|number| u64::try_from(number).ok()))
                .or_else(|| {
                    value
                        .as_f64()
                        .filter(|number| number.is_finite())
                        .map(|number| number.max(0.0) as u64)
                })
        })
        .unwrap_or(0)
}

fn decimal(value: Option<&Value>) -> f64 {
    value
        .and_then(|value| {
            value
                .as_f64()
                .or_else(|| value.as_str()?.parse::<f64>().ok())
        })
        .filter(|value| value.is_finite() && *value >= 0.0)
        .unwrap_or(0.0)
}

fn bounded_text(value: Option<&Value>, fallback: &str) -> String {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(fallback)
        .chars()
        .take(MAX_LABEL_CHARS)
        .collect()
}

fn parse_tokens(usage: &Value) -> Tokens {
    Tokens {
        input: number(usage.get("input")),
        output: number(usage.get("output")),
        cache_read: number(usage.get("cacheRead")),
        cache_write: number(usage.get("cacheWrite")),
    }
}

fn parse_costs(usage: &Value) -> Costs {
    let cost = usage.get("cost");
    Costs {
        input: decimal(cost.and_then(|value| value.get("input"))),
        output: decimal(cost.and_then(|value| value.get("output"))),
        cache_read: decimal(cost.and_then(|value| value.get("cacheRead"))),
        cache_write: decimal(cost.and_then(|value| value.get("cacheWrite"))),
        total: decimal(cost.and_then(|value| value.get("total"))),
    }
}

fn parse_timestamp(value: Option<&Value>, fallback: DateTime<Utc>) -> String {
    let parsed = value.and_then(|value| {
        if let Some(number) = value.as_i64() {
            let seconds = if number.abs() > 100_000_000_000 {
                number / 1000
            } else {
                number
            };
            return DateTime::<Utc>::from_timestamp(seconds, 0);
        }
        value
            .as_str()
            .and_then(|text| DateTime::parse_from_rfc3339(text).ok())
            .map(|time| time.with_timezone(&Utc))
    });
    parsed.unwrap_or(fallback).to_rfc3339()
}

fn hash_field(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
}

fn hash_json(hasher: &mut Sha256, value: &Value) {
    match value {
        Value::Null => hash_field(hasher, b"null"),
        Value::Bool(value) => hash_field(hasher, if *value { b"true" } else { b"false" }),
        Value::Number(value) => hash_field(hasher, value.to_string().as_bytes()),
        Value::String(value) => hash_field(hasher, value.as_bytes()),
        Value::Array(values) => {
            hash_field(hasher, &(values.len() as u64).to_be_bytes());
            for value in values {
                hash_json(hasher, value);
            }
        }
        Value::Object(values) => {
            let mut keys = values.keys().collect::<Vec<_>>();
            keys.sort_unstable();
            for key in keys {
                hash_field(hasher, key.as_bytes());
                hash_json(hasher, &values[key]);
            }
        }
    }
}

fn identities(
    entry: &Value,
    kind: &str,
    usage: &Value,
    message: Option<&Value>,
) -> (String, String, bool) {
    let mut semantic = Sha256::new();
    hash_field(&mut semantic, b"cchub-pi-session-semantic-v1");
    hash_field(&mut semantic, kind.as_bytes());
    for value in [
        entry.get("timestamp"),
        message.and_then(|value| value.get("timestamp")),
    ]
    .into_iter()
    .flatten()
    {
        hash_json(&mut semantic, value);
    }
    if let Some(message) = message {
        for key in [
            "provider",
            "model",
            "responseModel",
            "responseId",
            "api",
            "toolCallId",
            "toolName",
            "stopReason",
            "errorMessage",
            "content",
        ] {
            if let Some(value) = message.get(key) {
                hash_field(&mut semantic, key.as_bytes());
                hash_json(&mut semantic, value);
            }
        }
    } else if let Some(summary) = entry.get("summary") {
        hash_json(&mut semantic, summary);
    }
    hash_json(&mut semantic, usage);
    let semantic_id = format!("pi-semantic:{:x}", semantic.finalize());
    let entry_id = entry
        .get("id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let request_id = match entry_id {
        Some(entry_id) => {
            let mut request = Sha256::new();
            hash_field(&mut request, b"cchub-pi-session-request-v1");
            hash_field(&mut request, kind.as_bytes());
            hash_field(&mut request, entry_id.as_bytes());
            if let Some(timestamp) = entry.get("timestamp") {
                hash_json(&mut request, timestamp);
            }
            format!("pi-session:{:x}", request.finalize())
        }
        None => semantic_id.clone(),
    };
    (request_id, semantic_id, entry_id.is_some())
}

fn parse_record(entry: &Value, session_id: &str, fallback_time: DateTime<Utc>) -> Option<PiRecord> {
    let entry_type = entry.get("type").and_then(Value::as_str)?;
    let (kind, usage, message) = match entry_type {
        "message" => {
            let message = entry.get("message")?;
            match message.get("role").and_then(Value::as_str) {
                Some("assistant") => ("assistant", message.get("usage")?, Some(message)),
                Some("toolResult") => ("tool_result", message.get("usage")?, Some(message)),
                _ => return None,
            }
        }
        "compaction" => ("compaction", entry.get("usage")?, None),
        "branch_summary" => ("branch_summary", entry.get("usage")?, None),
        _ => return None,
    };
    let tokens = parse_tokens(usage);
    let costs = parse_costs(usage);
    let stop_reason = message
        .and_then(|value| value.get("stopReason"))
        .and_then(Value::as_str);
    let failed = matches!(stop_reason, Some("error" | "aborted"));
    if !tokens.has_values() && costs.reported_total().is_none() && !failed {
        return None;
    }

    let provider = message
        .map(|value| bounded_text(value.get("provider"), "Pi"))
        .unwrap_or_else(|| "Pi".to_string());
    let request_model = message
        .map(|value| bounded_text(value.get("model"), "unknown"))
        .unwrap_or_else(|| "unknown".to_string());
    let model = message
        .map(|value| bounded_text(value.get("responseModel"), &request_model))
        .unwrap_or_else(|| "unknown".to_string());
    let created_at = parse_timestamp(
        entry
            .get("timestamp")
            .or_else(|| message.and_then(|value| value.get("timestamp"))),
        fallback_time,
    );
    let (request_id, semantic_id, has_entry_id) = identities(entry, kind, usage, message);
    let error = failed.then(|| {
        message
            .and_then(|value| value.get("errorMessage"))
            .and_then(Value::as_str)
            .unwrap_or(if stop_reason == Some("aborted") {
                "Pi request aborted"
            } else {
                "Pi request failed"
            })
            .chars()
            .take(4096)
            .collect()
    });
    Some(PiRecord {
        request_id,
        semantic_id,
        has_entry_id,
        session_id: session_id.to_string(),
        provider,
        model,
        request_model,
        tokens,
        costs,
        status_code: if stop_reason == Some("aborted") {
            499
        } else if failed {
            500
        } else {
            200
        },
        error,
        created_at,
    })
}

fn collect_files(root: &Path, files: &mut Vec<PathBuf>, depth: usize) {
    if depth > MAX_DEPTH || files.len() >= MAX_FILES {
        return;
    }
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        if files.len() >= MAX_FILES {
            break;
        }
        let path = entry.path();
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_symlink() {
            continue;
        }
        if kind.is_dir() {
            collect_files(&path, files, depth + 1);
        } else if kind.is_file()
            && path.extension().and_then(|value| value.to_str()) == Some("jsonl")
        {
            files.push(path);
        }
    }
}

fn parse_file(path: &Path) -> Result<Vec<PiRecord>, String> {
    let metadata = fs::metadata(path).map_err(|error| error.to_string())?;
    if metadata.len() > MAX_FILE_BYTES {
        return Err("session exceeds the 128 MiB safety limit".to_string());
    }
    let fallback_time = metadata
        .modified()
        .ok()
        .map(DateTime::<Utc>::from)
        .unwrap_or_else(Utc::now);
    let file = fs::File::open(path).map_err(|error| error.to_string())?;
    let mut session_id = None;
    let mut records = Vec::new();
    for (index, line) in BufReader::new(file).lines().enumerate() {
        if index >= MAX_ENTRIES {
            return Err(format!(
                "session exceeds the {MAX_ENTRIES}-entry safety limit"
            ));
        }
        let line = line.map_err(|error| error.to_string())?;
        if line.trim().is_empty() {
            continue;
        }
        let Ok(entry) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        if session_id.is_none() && entry.get("type").and_then(Value::as_str) == Some("session") {
            session_id = entry.get("id").and_then(Value::as_str).map(str::to_string);
            continue;
        }
        let Some(id) = session_id.as_deref() else {
            continue;
        };
        if let Some(record) = parse_record(&entry, id, fallback_time) {
            records.push(record);
        }
    }
    if session_id.is_none() {
        return Err("session header is missing".to_string());
    }
    Ok(records)
}

fn model_cost(conn: &Connection, model: &str, tokens: Tokens) -> f64 {
    let normalized = model.trim().to_ascii_lowercase();
    let rates = conn.query_row(
        "SELECT input_cost_per_million, output_cost_per_million, cache_read_cost_per_million, cache_write_cost_per_million
         FROM model_pricing WHERE model_id = ?1 OR normalized_model_id = ?1
         ORDER BY CASE WHEN model_id = ?1 THEN 0 ELSE 1 END LIMIT 1",
        [&normalized],
        |row| Ok([
            row.get::<_, String>(0)?.parse::<f64>().unwrap_or(0.0),
            row.get::<_, String>(1)?.parse::<f64>().unwrap_or(0.0),
            row.get::<_, String>(2)?.parse::<f64>().unwrap_or(0.0),
            row.get::<_, String>(3)?.parse::<f64>().unwrap_or(0.0),
        ]),
    ).optional().ok().flatten().unwrap_or([0.0; 4]);
    (tokens.input as f64 * rates[0]
        + tokens.output as f64 * rates[1]
        + tokens.cache_read as f64 * rates[2]
        + tokens.cache_write as f64 * rates[3])
        / 1_000_000.0
}

fn insert_record(conn: &Connection, record: &PiRecord) -> Result<bool, String> {
    let request_seen: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM session_usage_dedup WHERE data_source = ?1 AND request_id = ?2)",
        rusqlite::params![DATA_SOURCE, record.request_id], |row| row.get(0),
    ).map_err(|error| error.to_string())?;
    let semantic_seen: bool = conn.query_row(
        if record.has_entry_id {
            "SELECT EXISTS(SELECT 1 FROM session_usage_dedup WHERE data_source = ?1 AND semantic_id = ?2 AND has_entry_id = 0)"
        } else {
            "SELECT EXISTS(SELECT 1 FROM session_usage_dedup WHERE data_source = ?1 AND semantic_id = ?2)"
        },
        rusqlite::params![DATA_SOURCE, record.semantic_id], |row| row.get(0),
    ).map_err(|error| error.to_string())?;
    if request_seen || semantic_seen {
        return Ok(false);
    }

    let total_cost = record
        .costs
        .reported_total()
        .unwrap_or_else(|| model_cost(conn, &record.model, record.tokens));
    conn.execute(
        "INSERT INTO session_usage_dedup (data_source, request_id, semantic_id, has_entry_id, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![DATA_SOURCE, record.request_id, record.semantic_id, i64::from(record.has_entry_id), record.created_at],
    ).map_err(|error| error.to_string())?;
    conn.execute(
        "INSERT OR IGNORE INTO proxy_request_logs (
            request_id, tool_id, profile_id, provider_name, request_model, response_model,
            input_tokens, output_tokens, cache_read_tokens, cache_creation_tokens,
            total_cost_usd, latency_ms, status_code, is_streaming, error_message, created_at
         ) VALUES (?1, 'pi', ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 0, ?11, 1, ?12, ?13)",
        rusqlite::params![
            record.request_id,
            format!("pi-session:{}", record.session_id),
            format!("{} (Session)", record.provider),
            record.request_model,
            record.model,
            record.tokens.input.min(i64::MAX as u64) as i64,
            record.tokens.output.min(i64::MAX as u64) as i64,
            record.tokens.cache_read.min(i64::MAX as u64) as i64,
            record.tokens.cache_write.min(i64::MAX as u64) as i64,
            format!("{total_cost:.9}"),
            record.status_code,
            record.error,
            record.created_at,
        ],
    )
    .map(|changed| changed > 0)
    .map_err(|error| error.to_string())
}

pub fn sync_pi_usage(conn: &mut Connection) -> Result<SessionSyncResult, String> {
    let root = crate::commands::pi_compat::session_root()??;
    let mut files = Vec::new();
    collect_files(&root, &mut files, 0);
    files.sort();
    let mut result = SessionSyncResult {
        files_scanned: files.len().min(u32::MAX as usize) as u32,
        ..Default::default()
    };
    for file in files {
        let records = match parse_file(&file) {
            Ok(records) => records,
            Err(error) => {
                result.errors.push(format!("{}: {error}", file.display()));
                continue;
            }
        };
        let transaction = conn.transaction().map_err(|error| error.to_string())?;
        let mut imported = 0u32;
        let mut skipped = 0u32;
        let mut file_error = None;
        for record in &records {
            match insert_record(&transaction, record) {
                Ok(true) => imported = imported.saturating_add(1),
                Ok(false) => skipped = skipped.saturating_add(1),
                Err(error) => {
                    file_error = Some(error);
                    break;
                }
            }
        }
        if let Some(error) = file_error {
            result.errors.push(format!("{}: {error}", file.display()));
            continue;
        }
        transaction.commit().map_err(|error| error.to_string())?;
        result.imported = result.imported.saturating_add(imported);
        result.skipped = result.skipped.saturating_add(skipped);
        result.suspected_duplicates = result.suspected_duplicates.saturating_add(skipped);
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(input: u64) -> Value {
        serde_json::json!({
            "type": "message", "id": "assistant-1", "timestamp": "2026-08-24T10:00:00Z",
            "message": { "role": "assistant", "provider": "vendor", "model": "requested",
                "responseModel": "actual", "content": [{"type":"text","text":"ok"}],
                "usage": { "input": input, "output": 7, "cacheRead": 5, "cacheWrite": 2,
                    "cost": { "input": 0.1, "output": 0.2, "cacheRead": 0.01, "cacheWrite": 0.02, "total": 0.33 } },
                "stopReason": "stop" }
        })
    }

    #[test]
    fn parses_pi_native_tokens_cost_and_models() {
        let record = parse_record(&fixture(10), "session-a", Utc::now()).expect("record");
        assert_eq!(
            record.tokens,
            Tokens {
                input: 10,
                output: 7,
                cache_read: 5,
                cache_write: 2
            }
        );
        assert_eq!(record.model, "actual");
        assert_eq!(record.request_model, "requested");
        assert_eq!(record.costs.reported_total(), Some(0.33));
    }

    #[test]
    fn durable_ledger_rejects_replayed_entry() {
        let mut conn = Connection::open_in_memory().expect("database");
        crate::db::schema::run_migrations(&conn).expect("schema");
        let record = parse_record(&fixture(10), "session-a", Utc::now()).expect("record");
        let tx = conn.transaction().expect("transaction");
        assert!(insert_record(&tx, &record).expect("first insert"));
        tx.commit().expect("commit");
        assert!(!insert_record(&conn, &record).expect("replay"));
    }

    #[test]
    fn distinct_entry_ids_do_not_collapse_on_semantic_hash() {
        let first = fixture(10);
        let mut second = first.clone();
        second["id"] = Value::String("assistant-2".to_string());
        let first = parse_record(&first, "session-a", Utc::now()).expect("first");
        let second = parse_record(&second, "session-a", Utc::now()).expect("second");
        assert_ne!(first.request_id, second.request_id);
        assert_eq!(first.semantic_id, second.semantic_id);

        let mut conn = Connection::open_in_memory().expect("database");
        crate::db::schema::run_migrations(&conn).expect("schema");
        let tx = conn.transaction().expect("transaction");
        assert!(insert_record(&tx, &first).expect("first insert"));
        assert!(insert_record(&tx, &second).expect("second insert"));
        tx.commit().expect("commit");
    }

    #[test]
    fn entry_id_upgrade_rejects_a_legacy_semantic_duplicate() {
        let legacy_value = fixture(10);
        let mut legacy_value = legacy_value;
        legacy_value.as_object_mut().expect("entry").remove("id");
        let legacy = parse_record(&legacy_value, "session-a", Utc::now()).expect("legacy");
        let modern = parse_record(&fixture(10), "session-a", Utc::now()).expect("modern");
        assert_eq!(legacy.semantic_id, modern.semantic_id);

        let mut conn = Connection::open_in_memory().expect("database");
        crate::db::schema::run_migrations(&conn).expect("schema");
        let tx = conn.transaction().expect("transaction");
        assert!(insert_record(&tx, &legacy).expect("legacy insert"));
        tx.commit().expect("commit");
        assert!(!insert_record(&conn, &modern).expect("modern duplicate"));
    }
}
