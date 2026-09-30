//! Extract accounting fields only; conversation bodies never leave SQLite.
use chrono::{DateTime, Utc};
use rusqlite::Connection;
use serde::Serialize;
use serde_json::Value;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub(crate) struct Tokens {
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_write: u64,
}

impl Tokens {
    pub(crate) fn input_total(self) -> u64 {
        self.input
            .saturating_add(self.cache_read)
            .saturating_add(self.cache_write)
    }

    pub(crate) fn total(self) -> u64 {
        self.input_total().saturating_add(self.output)
    }

    pub(crate) fn accumulate(&mut self, next: Self) {
        self.input = self.input.saturating_add(next.input);
        self.output = self.output.saturating_add(next.output);
        self.cache_read = self.cache_read.saturating_add(next.cache_read);
        self.cache_write = self.cache_write.saturating_add(next.cache_write);
    }
}

#[derive(Debug, Serialize)]
pub(crate) struct UsageRecord {
    pub session: String,
    pub message: String,
    pub model: String,
    pub provider: String,
    pub tokens: Tokens,
    pub reported_cost: Option<f64>,
    pub failed: bool,
    pub created_at: String,
}

#[derive(Default)]
pub(crate) struct UsageBatch {
    pub records: Vec<UsageRecord>,
    pub pending: u32,
    pub malformed: u32,
}

fn label(value: Option<&Value>, fallback: &str) -> String {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .unwrap_or(fallback)
        .chars()
        .take(512)
        .collect()
}

fn count(value: Option<&Value>) -> u64 {
    value.and_then(Value::as_u64).unwrap_or(0)
}

fn parse(
    value: &Value,
    session: String,
    message: String,
    created: i64,
    kind: &str,
) -> Option<UsageRecord> {
    let usage = value.get("tokens")?.as_object()?;
    let tokens = Tokens {
        input: count(usage.get("input")),
        // Native reasoning usage is separate from visible output and billed as output.
        output: count(usage.get("output")).saturating_add(count(usage.get("reasoning"))),
        cache_read: count(usage.get("cache").and_then(|cache| cache.get("read"))),
        cache_write: count(usage.get("cache").and_then(|cache| cache.get("write"))),
    };
    if tokens.total() == 0 {
        return None;
    }
    let model = value
        .get("modelID")
        .filter(|model| model.is_string())
        .or_else(|| value.get("model").filter(|model| model.is_string()))
        .or_else(|| value.pointer("/model/id"));
    let provider = value
        .get("providerID")
        .filter(|provider| provider.is_string())
        .or_else(|| value.pointer("/model/providerID"));
    let time = value
        .pointer("/time/created")
        .and_then(Value::as_i64)
        .unwrap_or(created);
    let timestamp = DateTime::<Utc>::from_timestamp_millis(time)
        .or_else(|| DateTime::<Utc>::from_timestamp_millis(created))?;
    Some(UsageRecord {
        session,
        message,
        model: label(model, "unknown"),
        provider: label(provider, "OpenCode"),
        tokens,
        reported_cost: value
            .get("cost")
            .and_then(Value::as_f64)
            .filter(|cost| cost.is_finite() && *cost > 0.0),
        failed: value.get("error").is_some_and(|error| !error.is_null())
            || (kind == "compaction"
                && value.get("status").and_then(Value::as_str) == Some("failed")),
        created_at: timestamp.to_rfc3339(),
    })
}

pub(crate) fn read_usage(conn: &Connection, session: Option<&str>) -> Result<UsageBatch, String> {
    let mut result = UsageBatch::default();
    for layout in crate::opencode_storage::layouts(conn)? {
        let kind = if layout.v2 {
            "m.type"
        } else {
            "json_extract(m.data, '$.role')"
        };
        // Project known accounting fields inside SQLite instead of loading the
        // full data object (which can contain prompts, output and tool secrets).
        let sql = format!("SELECT s.id, m.id, m.time_created, CASE WHEN json_valid(m.data) THEN {kind} END,
            CASE WHEN json_valid(m.data) THEN json_object(
                'tokens', json_object('input', json_extract(m.data, '$.tokens.input'),
                    'output', json_extract(m.data, '$.tokens.output'),
                    'reasoning', json_extract(m.data, '$.tokens.reasoning'),
                    'cache', json_object('read', json_extract(m.data, '$.tokens.cache.read'),
                        'write', json_extract(m.data, '$.tokens.cache.write'))),
                'time', json_object('created', json_extract(m.data, '$.time.created'),
                    'completed', json_extract(m.data, '$.time.completed')),
                'modelID', json_extract(m.data, '$.modelID'),
                'model', CASE WHEN json_type(m.data, '$.model') = 'text' THEN json_extract(m.data, '$.model')
                    ELSE json_object('id', json_extract(m.data, '$.model.id'),
                        'providerID', json_extract(m.data, '$.model.providerID')) END,
                'providerID', json_extract(m.data, '$.providerID'),
                'cost', json_extract(m.data, '$.cost'),
                'status', json_extract(m.data, '$.status'),
                'error', CASE WHEN json_type(m.data, '$.error') IS NOT NULL AND json_type(m.data, '$.error') != 'null' THEN 1 END
            ) ELSE NULL END
            FROM {messages} m JOIN {sessions} s ON s.id = m.session_id
            WHERE {predicate} AND (?2 IS NULL OR s.id = ?2)
            ORDER BY m.time_created, m.id", messages = layout.messages, sessions = layout.sessions, predicate = layout.predicate);
        let mut statement = conn.prepare(&sql).map_err(|error| error.to_string())?;
        let rows = statement
            .query_map(rusqlite::params![layout.migration, session], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                ))
            })
            .map_err(|error| error.to_string())?;
        for row in rows {
            let (session, message, created, kind, data) = row.map_err(|error| error.to_string())?;
            let Some(data) = data else {
                result.malformed = result.malformed.saturating_add(1);
                continue;
            };
            let kind = kind.as_deref().unwrap_or("");
            if kind != "assistant" && !(layout.v2 && kind == "compaction") {
                continue;
            }
            let value: Value =
                serde_json::from_str(&data).map_err(|_| "Invalid accounting projection")?;
            let completed = if kind == "compaction" {
                matches!(
                    value.get("status").and_then(Value::as_str),
                    Some("completed" | "failed")
                )
            } else {
                value
                    .pointer("/time/completed")
                    .and_then(Value::as_i64)
                    .is_some_and(|time| time > 0)
            };
            if !completed {
                result.pending = result.pending.saturating_add(1);
                continue;
            }
            if let Some(record) = parse(&value, session, message, created, kind) {
                result.records.push(record);
            }
        }
    }
    Ok(result)
}
