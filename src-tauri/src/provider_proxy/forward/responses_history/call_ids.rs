use bytes::Bytes;
use serde::Deserialize;
use serde_json::value::RawValue;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};

#[derive(Deserialize)]
struct History<'a> {
    #[serde(borrow)]
    input: Option<&'a RawValue>,
}

#[derive(Deserialize)]
struct Call<'a> {
    #[serde(rename = "type")]
    kind: Option<String>,
    #[serde(borrow)]
    call_id: Option<&'a RawValue>,
}

pub(super) fn repair(body: Bytes) -> Bytes {
    bounded(&body).unwrap_or(body)
}

// Replace only oversized link strings; preserve all other history bytes,
// including encrypted reasoning and numeric spellings from other providers.
fn bounded(body: &Bytes) -> Option<Bytes> {
    let source = std::str::from_utf8(body).ok()?;
    let history: History<'_> = serde_json::from_str(source).ok()?;
    let entries: Vec<&RawValue> = serde_json::from_str(history.input?.get()).ok()?;
    let mut candidates = Vec::new();
    let mut occupied = HashSet::new();
    for entry in entries {
        let Ok(call) = serde_json::from_str::<Call<'_>>(entry.get()) else {
            continue;
        };
        if !matches!(
            call.kind.as_deref(),
            Some(
                "function_call"
                    | "function_call_output"
                    | "tool_search_call"
                    | "tool_search_output"
                    | "custom_tool_call"
                    | "custom_tool_call_output"
            )
        ) {
            continue;
        }
        let Some(raw) = call.call_id else { continue };
        let Ok(id) = serde_json::from_str::<String>(raw.get()) else {
            continue;
        };
        if id.chars().count() > 64 {
            candidates.push((raw, id));
        } else {
            occupied.insert(id);
        }
    }
    if candidates.is_empty() {
        return None;
    }
    let mut assigned = HashMap::new();
    let mut replacements = Vec::new();
    for (raw, id) in candidates {
        let next = assigned.entry(id.clone()).or_insert_with(|| {
            let mut next = format!("{:x}", Sha256::digest(id.as_bytes()));
            let mut nonce = 0usize;
            while occupied.contains(&next) {
                nonce += 1;
                next = format!("{:x}", Sha256::digest(format!("{id}\0{nonce}").as_bytes()));
            }
            occupied.insert(next.clone());
            next
        });
        let start = raw
            .get()
            .as_ptr()
            .addr()
            .checked_sub(source.as_ptr().addr())?;
        let end = start.checked_add(raw.get().len())?;
        if source.get(start..end) != Some(raw.get()) {
            return None;
        }
        replacements.push((start..end, serde_json::to_vec(next).ok()?));
    }
    let mut output = Vec::with_capacity(body.len());
    let mut cursor = 0;
    for (range, replacement) in replacements {
        output.extend_from_slice(body.get(cursor..range.start)?);
        output.extend_from_slice(&replacement);
        cursor = range.end;
    }
    output.extend_from_slice(body.get(cursor..)?);
    Some(Bytes::from(output))
}

#[cfg(test)]
#[path = "call_ids/tests.rs"]
mod tests;
