//! Session usage import for tools that write JSONL transcripts.
//!
//! The importer deliberately extracts only accounting fields.  Prompt and
//! response bodies stay on disk and are never copied into the CCHub database.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter, State};

use crate::db::DbState;

const MAX_FILE_BYTES: u64 = 32 * 1024 * 1024;
const MAX_FILES: usize = 20_000;
const USAGE_INSERT_BATCH_SIZE: usize = 1_000;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSyncResult {
    pub imported: u32,
    pub skipped: u32,
    pub files_scanned: u32,
    pub suspected_duplicates: u32,
    pub deferred_files: u32,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct UsageCounts {
    input: u64,
    output: u64,
    cache_read: u64,
    cache_write: u64,
}

impl UsageCounts {
    fn has_values(self) -> bool {
        self.input > 0 || self.output > 0 || self.cache_read > 0 || self.cache_write > 0
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct CumulativeCounts {
    input: u64,
    output: u64,
    cache_read: u64,
}

impl CumulativeCounts {
    fn delta(self, previous: Option<Self>) -> UsageCounts {
        let previous = previous.unwrap_or_default();
        UsageCounts {
            input: self.input.saturating_sub(previous.input),
            output: self.output.saturating_sub(previous.output),
            cache_read: self.cache_read.saturating_sub(previous.cache_read),
            cache_write: 0,
        }
    }

    fn high_water(&mut self, current: Self) {
        self.input = self.input.max(current.input);
        self.output = self.output.max(current.output);
        self.cache_read = self.cache_read.max(current.cache_read);
    }
}

#[derive(Debug)]
struct UsageRecord {
    id: String,
    tool: &'static str,
    model: String,
    counts: UsageCounts,
    timestamp: String,
    source: String,
}

fn number(value: Option<&Value>) -> u64 {
    value
        .and_then(|value| {
            value
                .as_u64()
                .or_else(|| value.as_i64().and_then(|number| u64::try_from(number).ok()))
                .or_else(|| value.as_f64().map(|number| number.max(0.0) as u64))
        })
        .unwrap_or(0)
}

fn usage_from_object(value: &Value) -> UsageCounts {
    let input = number(
        value
            .get("input_tokens")
            .or_else(|| value.get("prompt_tokens"))
            .or_else(|| value.get("inputTokens")),
    );
    let output = number(
        value
            .get("output_tokens")
            .or_else(|| value.get("completion_tokens"))
            .or_else(|| value.get("outputTokens")),
    );
    let cache_read = number(
        value
            .get("cache_read_input_tokens")
            .or_else(|| value.get("cache_read_tokens"))
            .or_else(|| value.get("cacheReadTokens")),
    );
    let cache_write = number(
        value
            .get("cache_creation_input_tokens")
            .or_else(|| value.get("cache_write_tokens"))
            .or_else(|| value.get("cacheWriteTokens")),
    );
    UsageCounts {
        input,
        output,
        cache_read,
        cache_write,
    }
}

fn cumulative_from_value(value: Option<&Value>) -> Option<CumulativeCounts> {
    let object = value?.as_object()?;
    if !object.keys().any(|key| {
        matches!(
            key.as_str(),
            "input_tokens"
                | "output_tokens"
                | "cached_input_tokens"
                | "cache_read_input_tokens"
                | "total_tokens"
        )
    }) {
        return None;
    }
    Some(CumulativeCounts {
        input: number(value.and_then(|item| item.get("input_tokens"))),
        output: number(value.and_then(|item| item.get("output_tokens"))),
        cache_read: number(
            value
                .and_then(|item| item.get("cached_input_tokens"))
                .or_else(|| value.and_then(|item| item.get("cache_read_input_tokens"))),
        ),
    })
}

#[derive(Default)]
struct CodexTokenState {
    total_high_water: Option<CumulativeCounts>,
    last_signatures_by_source: HashMap<String, String>,
    previous_signature: Option<String>,
}

fn codex_token_counts(value: &Value, state: &mut CodexTokenState) -> Option<(UsageCounts, String)> {
    let payload = value.get("payload")?;
    if payload.get("type").and_then(Value::as_str) != Some("token_count") {
        return None;
    }
    let info = payload.get("info")?;
    let total = cumulative_from_value(info.get("total_token_usage"));
    let last = cumulative_from_value(info.get("last_token_usage"));
    if total.is_none() && last.is_none() {
        return None;
    }
    let signature =
        serde_json::to_string(&(info.get("total_token_usage"), info.get("last_token_usage")))
            .ok()?;
    let source = payload
        .get("rate_limits")
        .and_then(|limits| limits.get("limit_id"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let duplicate = total.is_some()
        && (state.last_signatures_by_source.get(&source) == Some(&signature)
            || state.previous_signature.as_ref() == Some(&signature));
    if total.is_some() {
        state
            .last_signatures_by_source
            .insert(source, signature.clone());
    }
    state.previous_signature = Some(signature);
    let counts = if duplicate {
        UsageCounts::default()
    } else if let Some(last) = last {
        last.delta(None)
    } else {
        let current = total?;
        current.delta(state.total_high_water)
    };
    if let Some(current) = total {
        if let Some(high_water) = state.total_high_water.as_mut() {
            high_water.high_water(current);
        } else {
            state.total_high_water = Some(current);
        }
    }
    let model = find_text(info, &["model", "model_name", "modelName"]).unwrap_or_default();
    Some((counts, model))
}

fn find_usage(value: &Value) -> Option<UsageCounts> {
    if let Some(usage) = value.get("usage") {
        if let Some(object) = usage.as_object() {
            let counts = usage_from_object(&Value::Object(object.clone()));
            if counts.has_values() {
                return Some(counts);
            }
        }
    }
    if value.is_object() {
        let direct = usage_from_object(value);
        if direct.has_values() {
            return Some(direct);
        }
        for child in value
            .as_object()
            .into_iter()
            .flatten()
            .map(|(_, child)| child)
        {
            if let Some(counts) = find_usage(child) {
                return Some(counts);
            }
        }
    } else if let Some(array) = value.as_array() {
        for child in array {
            if let Some(counts) = find_usage(child) {
                return Some(counts);
            }
        }
    }
    None
}

fn find_text(value: &Value, keys: &[&str]) -> Option<String> {
    if let Some(object) = value.as_object() {
        for key in keys {
            if let Some(text) = object.get(*key).and_then(Value::as_str) {
                if !text.trim().is_empty() {
                    return Some(text.trim().to_string());
                }
            }
        }
        for child in object.values() {
            if let Some(text) = find_text(child, keys) {
                return Some(text);
            }
        }
    } else if let Some(array) = value.as_array() {
        for child in array {
            if let Some(text) = find_text(child, keys) {
                return Some(text);
            }
        }
    }
    None
}

fn tool_for_path(path: &Path) -> &'static str {
    let text = path.to_string_lossy().to_ascii_lowercase();
    if text.contains(".claude") {
        "claude"
    } else if text.contains(".codex") {
        "codex"
    } else if text.contains(".gemini") {
        "gemini"
    } else if text.contains(".opencode") {
        "opencode"
    } else if text.contains(".openclaw") {
        "openclaw"
    } else if text.contains(".hermes") {
        "hermes"
    } else if text.contains(".pi") {
        "pi"
    } else {
        "session"
    }
}

fn session_roots(home: &Path) -> Vec<PathBuf> {
    [
        home.join(".claude").join("projects"),
        home.join(".codex").join("sessions"),
        home.join(".gemini"),
        home.join(".opencode"),
        home.join(".openclaw"),
        home.join(".hermes"),
        home.join(".pi").join("agent"),
    ]
    .into_iter()
    .collect()
}

fn collect_jsonl_files(root: &Path, output: &mut Vec<PathBuf>, depth: usize) {
    if output.len() >= MAX_FILES || depth > 6 {
        return;
    }
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        if output.len() >= MAX_FILES {
            return;
        }
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            collect_jsonl_files(&path, output, depth + 1);
        } else if file_type.is_file()
            && matches!(
                path.extension().and_then(|value| value.to_str()),
                Some("jsonl" | "json")
            )
        {
            output.push(path);
        }
    }
}

fn request_id(path: &Path, line_number: usize, model: &str) -> String {
    let key = format!("{}:{line_number}:{model}", path.to_string_lossy());
    format!("session:{:x}", Sha256::digest(key.as_bytes()))
}

fn insert_usage(
    conn: &rusqlite::Connection,
    id: &str,
    tool: &str,
    model: &str,
    counts: UsageCounts,
    timestamp: &str,
) -> Result<bool, String> {
    let changed = conn
        .execute(
            "INSERT OR IGNORE INTO proxy_request_logs (
                request_id, tool_id, profile_id, provider_name, request_model,
                response_model, input_tokens, output_tokens, cache_read_tokens,
                cache_creation_tokens, total_cost_usd, latency_ms, status_code,
                is_streaming, error_message, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?5, ?6, ?7, ?8, ?9, '0', 0, 200, 0, NULL, ?10)",
            rusqlite::params![
                id,
                tool,
                format!("session:{tool}"),
                "Session import",
                if model.is_empty() { None } else { Some(model) },
                counts.input as i64,
                counts.output as i64,
                counts.cache_read as i64,
                counts.cache_write as i64,
                timestamp,
            ],
        )
        .map_err(|error| error.to_string())?;
    Ok(changed > 0)
}

fn scan_file(
    path: &Path,
    result: &mut SessionSyncResult,
    records: &mut Vec<UsageRecord>,
    only_tool: Option<&str>,
) {
    let tool = tool_for_path(path);
    if only_tool.is_some_and(|expected| expected != tool) {
        return;
    }
    let Ok(metadata) = fs::metadata(path) else {
        result.deferred_files = result.deferred_files.saturating_add(1);
        return;
    };
    if metadata.len() > MAX_FILE_BYTES {
        result.errors.push(format!(
            "Skipped oversized session file: {}",
            path.display()
        ));
        result.deferred_files = result.deferred_files.saturating_add(1);
        return;
    }
    result.files_scanned = result.files_scanned.saturating_add(1);
    let file = match fs::File::open(path) {
        Ok(file) => file,
        Err(error) => {
            result.errors.push(format!("{}: {error}", path.display()));
            return;
        }
    };
    let is_json = path.extension().and_then(|value| value.to_str()) == Some("json");
    let mut codex_token_state = CodexTokenState::default();
    let mut process = |line_number: usize, line: &str| {
        let value: Value = match serde_json::from_str(line) {
            Ok(value) => value,
            Err(_) => {
                result.skipped = result.skipped.saturating_add(1);
                return;
            }
        };
        let (counts, model) = if tool == "codex" {
            if let Some((counts, model)) = codex_token_counts(&value, &mut codex_token_state) {
                (counts, model)
            } else {
                let Some(counts) = find_usage(&value) else {
                    return;
                };
                (
                    counts,
                    find_text(&value, &["model", "model_name", "modelName"]).unwrap_or_default(),
                )
            }
        } else {
            let Some(counts) = find_usage(&value) else {
                return;
            };
            (
                counts,
                find_text(&value, &["model", "model_name", "modelName"]).unwrap_or_default(),
            )
        };
        if !counts.has_values() {
            return;
        }
        let id = request_id(path, line_number, &model);
        let timestamp = find_text(&value, &["timestamp", "created_at", "createdAt"])
            .unwrap_or_else(|| Utc::now().to_rfc3339());
        records.push(UsageRecord {
            id,
            tool,
            model,
            counts,
            timestamp,
            source: path.display().to_string(),
        });
    };
    if is_json {
        let mut text = String::new();
        use std::io::Read;
        if file
            .take(MAX_FILE_BYTES + 1)
            .read_to_string(&mut text)
            .is_ok()
        {
            process(1, &text);
        }
        return;
    }
    for (line_number, line) in BufReader::new(file).lines().enumerate() {
        if let Ok(line) = line {
            if !line.trim().is_empty() {
                process(line_number + 1, &line);
            }
        }
    }
}

fn sync_with_filter(only_tool: Option<&str>) -> (SessionSyncResult, Vec<UsageRecord>) {
    let Some(home) = dirs::home_dir() else {
        return (
            SessionSyncResult {
                errors: vec!["Cannot determine the home directory".to_string()],
                ..SessionSyncResult::default()
            },
            Vec::new(),
        );
    };
    let mut files = Vec::new();
    for root in session_roots(&home) {
        collect_jsonl_files(&root, &mut files, 0);
    }
    let mut result = SessionSyncResult::default();
    let mut records = Vec::new();
    for path in files {
        scan_file(&path, &mut result, &mut records, only_tool);
    }
    (result, records)
}

fn persist_records(
    conn: &mut rusqlite::Connection,
    records: Vec<UsageRecord>,
    result: &mut SessionSyncResult,
) {
    for batch in records.chunks(USAGE_INSERT_BATCH_SIZE) {
        let transaction = match conn.transaction() {
            Ok(transaction) => transaction,
            Err(error) => {
                result
                    .errors
                    .push(format!("Failed to start usage import transaction: {error}"));
                continue;
            }
        };
        let mut imported = 0u32;
        let mut suspected_duplicates = 0u32;
        let mut batch_error = None;
        for record in batch {
            match insert_usage(
                &transaction,
                &record.id,
                record.tool,
                &record.model,
                record.counts,
                &record.timestamp,
            ) {
                Ok(true) => imported = imported.saturating_add(1),
                Ok(false) => suspected_duplicates = suspected_duplicates.saturating_add(1),
                Err(error) => {
                    batch_error = Some(format!("{}: {error}", record.source));
                    break;
                }
            }
        }
        if let Some(error) = batch_error {
            result.errors.push(error);
            continue;
        }
        if let Err(error) = transaction.commit() {
            result
                .errors
                .push(format!("Failed to commit usage import batch: {error}"));
            continue;
        }
        result.imported = result.imported.saturating_add(imported);
        result.suspected_duplicates = result
            .suspected_duplicates
            .saturating_add(suspected_duplicates);
    }
}

#[tauri::command]
pub fn sync_session_usage(
    app: AppHandle,
    db: State<'_, DbState>,
) -> Result<SessionSyncResult, String> {
    let (mut result, records) = sync_with_filter(None);
    let mut conn = db.0.lock().map_err(|error| error.to_string())?;
    if !records.is_empty() {
        persist_records(&mut conn, records, &mut result);
    }
    match crate::commands::grok_session_usage::sync_grok_usage(&mut conn) {
        Ok(grok_result) => {
            result.imported = result.imported.saturating_add(grok_result.imported);
            result.skipped = result.skipped.saturating_add(grok_result.skipped);
            result.files_scanned = result
                .files_scanned
                .saturating_add(grok_result.files_scanned);
            result.suspected_duplicates = result
                .suspected_duplicates
                .saturating_add(grok_result.suspected_duplicates);
            result.deferred_files = result
                .deferred_files
                .saturating_add(grok_result.deferred_files);
            result.errors.extend(grok_result.errors);
        }
        Err(error) => result
            .errors
            .push(format!("Grok Build session import failed: {error}")),
    }
    let _ = app.emit(
        "usage-log-recorded",
        serde_json::json!({"source": "session", "imported": result.imported}),
    );
    Ok(result)
}

#[tauri::command]
pub fn rebuild_codex_usage(
    app: AppHandle,
    db: State<'_, DbState>,
) -> Result<SessionSyncResult, String> {
    let (mut result, records) = sync_with_filter(Some("codex"));
    if dirs::home_dir().is_none() {
        let _ = app.emit(
            "usage-log-recorded",
            serde_json::json!({"source": "codex-rebuild", "imported": 0}),
        );
        return Ok(result);
    }
    let mut conn = db.0.lock().map_err(|error| error.to_string())?;
    conn.execute(
        "DELETE FROM proxy_request_logs WHERE request_id LIKE 'session:%' AND tool_id = 'codex'",
        [],
    )
    .map_err(|error| error.to_string())?;
    persist_records(&mut conn, records, &mut result);
    let _ = app.emit(
        "usage-log-recorded",
        serde_json::json!({"source": "codex-rebuild", "imported": result.imported}),
    );
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::{codex_token_counts, find_usage, CumulativeCounts, UsageCounts};
    use serde_json::json;

    #[test]
    fn extracts_common_usage_shapes() {
        let counts = find_usage(&json!({
            "message": {"usage": {"input_tokens": 12, "output_tokens": 7}}
        }))
        .expect("usage should be found");
        assert_eq!(counts.input, 12);
        assert_eq!(counts.output, 7);
    }

    #[test]
    fn ignores_empty_usage() {
        assert!(find_usage(&json!({"usage": {}})).is_none());
    }

    #[test]
    fn cumulative_counts_use_saturating_deltas() {
        let current = CumulativeCounts {
            input: 120,
            output: 30,
            cache_read: 10,
        };
        assert_eq!(
            current.delta(Some(CumulativeCounts {
                input: 100,
                output: 40,
                cache_read: 4,
            })),
            UsageCounts {
                input: 20,
                output: 0,
                cache_read: 6,
                cache_write: 0,
            }
        );
    }

    #[test]
    fn codex_token_snapshots_are_incremental_and_replay_safe() {
        let first = json!({
            "payload": {"type": "token_count", "info": {
                "total_token_usage": {"input_tokens": 100, "output_tokens": 10}
            }}
        });
        let second = json!({
            "payload": {"type": "token_count", "info": {
                "total_token_usage": {"input_tokens": 140, "output_tokens": 15}
            }}
        });
        let mut state = super::CodexTokenState::default();
        assert_eq!(codex_token_counts(&first, &mut state).unwrap().0.input, 100);
        assert_eq!(codex_token_counts(&second, &mut state).unwrap().0.input, 40);
        assert_eq!(
            codex_token_counts(&second, &mut state).unwrap().0,
            UsageCounts::default()
        );
    }

    #[test]
    fn codex_prefers_exact_last_request_usage_over_total_jump() {
        let event = |total: u64, last: u64| {
            json!({"payload": {"type": "token_count", "info": {
                "total_token_usage": {"input_tokens": total},
                "last_token_usage": {"input_tokens": last}
            }}})
        };
        let mut state = super::CodexTokenState::default();
        assert_eq!(
            codex_token_counts(&event(100, 10), &mut state)
                .unwrap()
                .0
                .input,
            10
        );
        assert_eq!(
            codex_token_counts(&event(1_000, 5), &mut state)
                .unwrap()
                .0
                .input,
            5
        );
    }
}
