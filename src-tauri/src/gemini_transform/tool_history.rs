use serde_json::Value;
use std::collections::{HashMap, HashSet};

const MAX_CALLS: usize = 4096;
const MAX_ID_BYTES: usize = 1024;

// Request-local association: names cannot identify parallel calls to the same
// function. Completed IDs also remain reserved for the lifetime of the history.
#[derive(Default)]
pub(super) struct ToolHistory {
    pending: HashMap<String, String>,
    seen: HashSet<String>,
}

impl ToolHistory {
    pub(super) fn register<'a>(&mut self, block: &'a Value) -> Result<(&'a str, &'a str), String> {
        let id = required_string(
            block,
            "id",
            "Gemini tool history requires a nonempty call ID",
        )?;
        let name = required_string(
            block,
            "name",
            "Gemini tool history requires a nonempty function name",
        )?;
        validate_id(id)?;
        if name.len() > 128
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        {
            return Err("Gemini tool history contains an invalid function name".into());
        }
        if self.seen.len() >= MAX_CALLS {
            return Err("Gemini tool history exceeds the 4096-call limit".into());
        }
        if !self.seen.insert(id.to_owned()) {
            return Err("Gemini tool history contains a duplicate call ID".into());
        }
        self.pending.insert(id.to_owned(), name.to_owned());
        Ok((id, name))
    }

    pub(super) fn resolve<'a>(&mut self, block: &'a Value) -> Result<(&'a str, String), String> {
        let id = required_string(
            block,
            "tool_use_id",
            "Gemini tool result requires a nonempty call ID",
        )?;
        validate_id(id)?;
        let name = self.pending.remove(id).ok_or_else(|| {
            "Gemini tool result has no preceding unresolved call with the same ID".to_string()
        })?;
        Ok((id, name))
    }
}

fn required_string<'a>(block: &'a Value, key: &str, error: &str) -> Result<&'a str, String> {
    block
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| error.to_string())
}

fn gemini_tool_id(call: &Value) -> String {
    call.get("id")
        .and_then(Value::as_str)
        .filter(|id| !id.trim().is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| format!("toolu_{}", uuid::Uuid::new_v4().simple()))
}

fn validate_id(id: &str) -> Result<(), String> {
    if id.len() > MAX_ID_BYTES {
        Err("Gemini tool call ID exceeds the 1024-byte limit".into())
    } else {
        Ok(())
    }
}

#[derive(Default)]
pub(crate) struct ToolReplyIds {
    seen: HashSet<String>,
}

impl ToolReplyIds {
    pub(crate) fn next(&mut self, call: &Value) -> Result<String, String> {
        if self.seen.len() >= MAX_CALLS {
            return Err("Gemini tool response exceeds the 4096-call limit".into());
        }
        if let Some(id) = call.get("id").and_then(Value::as_str) {
            validate_id(id)?;
        }
        let id = gemini_tool_id(call);
        if !self.seen.insert(id.clone()) {
            return Err("Gemini tool response contains a duplicate call ID".into());
        }
        Ok(id)
    }
}
