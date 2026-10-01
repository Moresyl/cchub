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
struct Item<'a> {
    #[serde(rename = "type")]
    kind: Option<String>,
    #[serde(borrow)]
    id: Option<&'a RawValue>,
}

pub(super) fn repair(path: &str, body: Bytes) -> Bytes {
    if !matches!(
        path.trim_matches('/'),
        "responses" | "v1/responses" | "responses/compact" | "v1/responses/compact"
    ) {
        return body;
    }
    repaired(&body).unwrap_or(body)
}

/// Replace only the item ID's JSON string. Opaque history, number spellings,
/// whitespace and call_id links retain their original bytes.
fn repaired(body: &Bytes) -> Option<Bytes> {
    let source = std::str::from_utf8(body).ok()?;
    let history: History<'_> = serde_json::from_str(source).ok()?;
    let entries: Vec<&RawValue> = serde_json::from_str(history.input?.get()).ok()?;
    let mut occupied = HashSet::new();
    let mut candidates = Vec::new();
    for entry in entries {
        let Ok(item) = serde_json::from_str::<Item<'_>>(entry.get()) else {
            continue;
        };
        let Some(raw) = item.id.filter(|raw| raw.get().len() <= 6 * 1028 + 2) else {
            continue;
        };
        let Ok(id) = serde_json::from_str::<String>(raw.get()) else {
            continue;
        };
        if id.starts_with("tsc_") && id.len() <= 1028 {
            occupied.insert(id);
        } else if item.kind.as_deref() == Some("tool_search_call")
            && !id.is_empty()
            && id.len() <= 1024
        {
            candidates.push((raw, id));
        }
    }
    if candidates.is_empty() {
        return None;
    }
    let mut assigned = HashMap::<String, String>::new();
    let mut replacements = Vec::new();
    for (raw, id) in candidates {
        let next = assigned.entry(id.clone()).or_insert_with(|| {
            let suffix = id
                .split_once('_')
                .map(|(_, rest)| rest)
                .filter(|rest| !rest.is_empty())
                .unwrap_or(&id);
            let mut next = format!("tsc_{suffix}");
            if occupied.contains(&next) {
                let stable = format!("tsc_{:x}", Sha256::digest(id.as_bytes()));
                next = stable.clone();
                let mut index = 1usize;
                while occupied.contains(&next) {
                    next = format!("{stable}_{index}");
                    index += 1;
                }
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
#[path = "responses_history/tests.rs"]
mod tests;
