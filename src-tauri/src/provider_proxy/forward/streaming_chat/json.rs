use serde::de::{MapAccess, Visitor};
use serde::{Deserialize, Deserializer};
use serde_json::value::RawValue;
use std::collections::HashSet;
use std::ops::Range;

// Keep ordered, borrowed JSON values. Re-serializing a Value would change opaque
// history, signatures, escaped strings and arbitrary-precision number spellings.
struct Object<'a>(Vec<(String, &'a RawValue)>);

impl<'de> Deserialize<'de> for Object<'de> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ObjectVisitor;
        impl<'de> Visitor<'de> for ObjectVisitor {
            type Value = Object<'de>;

            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("an object with unambiguous fields")
            }

            fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Self::Value, M::Error> {
                let mut fields = Vec::new();
                let mut seen = HashSet::new();
                while let Some((key, raw)) = map.next_entry::<String, &'de RawValue>()? {
                    if !seen.insert(key.clone()) {
                        return Err(serde::de::Error::custom("duplicate field"));
                    }
                    fields.push((key, raw));
                }
                Ok(Object(fields))
            }
        }
        deserializer.deserialize_map(ObjectVisitor)
    }
}

impl<'a> Object<'a> {
    fn get(&self, name: &str) -> Option<&'a RawValue> {
        self.0
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, raw)| *raw)
    }
}

struct Edit {
    range: Range<usize>,
    replacement: String,
}

/// Repair empty compatibility shapes and fully understood typed Chat content.
/// Malformed or ambiguous objects pass through; unknown fields remain opaque.
pub(super) fn repair(source: &str) -> Option<String> {
    repair_inner(source, false)
}

pub(super) fn repair_whole(source: &str) -> Option<String> {
    repair_inner(source, true)
}

fn repair_inner(source: &str, whole: bool) -> Option<String> {
    // Ordinary content chunks need no extra JSON deserialization. This cheap
    // hint accepts false positives and recognizes whitespace inside [] without
    // relying on literal field names (which may themselves use JSON escapes).
    let empty_array = source
        .as_bytes()
        .split(|byte| *byte == b'[')
        .skip(1)
        .any(|tail| tail.iter().find(|byte| !byte.is_ascii_whitespace()) == Some(&b']'));
    let nested_array = source.bytes().filter(|byte| *byte == b'[').nth(1).is_some();
    if !source.contains("\"\"") && !empty_array && !nested_array {
        return None;
    }
    let root: Object<'_> = serde_json::from_str(source).ok()?;
    let choices: Vec<&RawValue> = serde_json::from_str(root.get("choices")?.get()).ok()?;
    let mut edits = Vec::new();
    for choice in choices {
        let fields: Object<'_> = serde_json::from_str(choice.get()).ok()?;
        if let Some(reason) = fields
            .get("finish_reason")
            .filter(|raw| !whole && raw.get() == "\"\"")
        {
            edits.push(Edit {
                range: range(source, reason)?,
                replacement: "null".into(),
            });
        }
        if let Some(delta) = fields.get(if whole { "message" } else { "delta" }) {
            repair_delta(source, delta, whole, &mut edits)?;
        }
    }
    if edits.is_empty() {
        return None;
    }
    edits.sort_by_key(|edit| edit.range.start);
    let mut output = String::with_capacity(source.len());
    let mut cursor = 0;
    for edit in edits {
        // Checked ranges also prevent accidental overlapping deletions.
        output.push_str(source.get(cursor..edit.range.start)?);
        output.push_str(&edit.replacement);
        cursor = edit.range.end;
    }
    output.push_str(source.get(cursor..)?);
    Some(output)
}

fn repair_delta(source: &str, delta: &RawValue, whole: bool, edits: &mut Vec<Edit>) -> Option<()> {
    let fields: Object<'_> = serde_json::from_str(delta.get()).ok()?;
    let mut removed = Vec::new();
    let flattened_thinking = flatten_content(source, delta, &fields, &mut removed, edits)?;
    for (index, (key, raw)) in fields.0.iter().enumerate() {
        if !whole && !flattened_thinking && key == "reasoning_content" && raw.get() == "\"\"" {
            removed.push(index);
        } else if !whole && key == "tool_calls" {
            let calls: Vec<&RawValue> = serde_json::from_str(raw.get()).ok()?;
            if calls.is_empty() {
                removed.push(index);
            } else {
                for call in calls {
                    let call: Object<'_> = serde_json::from_str(call.get()).ok()?;
                    if let Some(function) = call.get("function") {
                        let fields: Object<'_> = serde_json::from_str(function.get()).ok()?;
                        let empty: Vec<usize> = fields
                            .0
                            .iter()
                            .enumerate()
                            .filter(|(_, (key, raw))| key == "name" && raw.get() == "\"\"")
                            .map(|(index, _)| index)
                            .collect();
                        remove_fields(source, function, &fields, &empty, edits)?;
                    }
                }
            }
        }
    }
    removed.sort_unstable();
    remove_fields(source, delta, &fields, &removed, edits)
}

fn flatten_content(
    source: &str,
    delta: &RawValue,
    fields: &Object<'_>,
    removed: &mut Vec<usize>,
    edits: &mut Vec<Edit>,
) -> Option<bool> {
    let Some(raw) = fields
        .get("content")
        .filter(|raw| raw.get().starts_with('['))
    else {
        return Some(false);
    };
    let Ok(parts) = crate::provider_proxy_transform::chat_content::parse(raw.get()) else {
        // Unsupported extensions/signed parts retain every original byte.
        return Some(false);
    };
    let mut text = String::new();
    let mut thinking = String::new();
    for part in parts {
        match part.kind {
            crate::provider_proxy_transform::chat_content::Kind::Text => text.push_str(&part.text),
            crate::provider_proxy_transform::chat_content::Kind::Thinking => {
                thinking.push_str(&part.text)
            }
        }
    }
    if !thinking.is_empty() {
        let existing = string_field(fields.get("reasoning_content"))?;
        let alias = string_field(fields.get("reasoning"))?;
        // Distinct nonempty aliases are ambiguous. Never pick one and drop the other.
        if !existing.is_empty() && !alias.is_empty() && existing != alias {
            return None;
        }
        let combined = format!(
            "{}{thinking}",
            if existing.is_empty() {
                &alias
            } else {
                &existing
            }
        );
        let encoded = serde_json::to_string(&combined).ok()?;
        if let Some(raw) = fields.get("reasoning_content") {
            edits.push(Edit {
                range: range(source, raw)?,
                replacement: encoded,
            });
        } else {
            let end = range(source, delta)?.end.checked_sub(1)?;
            edits.push(Edit {
                range: end..end,
                replacement: format!(",\"reasoning_content\":{encoded}"),
            });
        }
        if let Some(index) = fields.0.iter().position(|(key, _)| key == "reasoning") {
            removed.push(index);
        }
    }
    edits.push(Edit {
        range: range(source, raw)?,
        replacement: serde_json::to_string(&text).ok()?,
    });
    Some(!thinking.is_empty())
}

fn string_field(raw: Option<&RawValue>) -> Option<String> {
    match raw {
        None => Some(String::new()),
        Some(raw) if raw.get() == "null" => Some(String::new()),
        Some(raw) => serde_json::from_str(raw.get()).ok(),
    }
}

fn range(source: &str, raw: &RawValue) -> Option<Range<usize>> {
    let start = raw
        .get()
        .as_ptr()
        .addr()
        .checked_sub(source.as_ptr().addr())?;
    let end = start.checked_add(raw.get().len())?;
    (source.get(start..end) == Some(raw.get())).then_some(start..end)
}

fn comma_before(source: &str, fields: &Object<'_>, index: usize) -> Option<usize> {
    let mut position = range(source, fields.0.get(index.checked_sub(1)?)?.1)?.end;
    while source
        .as_bytes()
        .get(position)
        .is_some_and(u8::is_ascii_whitespace)
    {
        position += 1;
    }
    (source.as_bytes().get(position) == Some(&b',')).then_some(position)
}

fn field_start(source: &str, raw: &RawValue, fields: &Object<'_>, index: usize) -> Option<usize> {
    if index == 0 {
        Some(range(source, raw)?.start + 1)
    } else {
        Some(comma_before(source, fields, index)? + 1)
    }
}

fn remove_fields(
    source: &str,
    raw: &RawValue,
    fields: &Object<'_>,
    removed: &[usize],
    edits: &mut Vec<Edit>,
) -> Option<()> {
    let mut cursor = 0;
    while let Some(&first) = removed.get(cursor) {
        let mut last = first;
        cursor += 1;
        while removed.get(cursor) == Some(&(last + 1)) {
            last += 1;
            cursor += 1;
        }
        let (start, end) = if last + 1 < fields.0.len() {
            (
                field_start(source, raw, fields, first)?,
                field_start(source, raw, fields, last + 1)?,
            )
        } else {
            let start = if first == 0 {
                range(source, raw)?.start + 1
            } else {
                comma_before(source, fields, first)?
            };
            (start, range(source, fields.0.get(last)?.1)?.end)
        };
        edits.push(Edit {
            range: start..end,
            replacement: String::new(),
        });
    }
    Some(())
}

#[cfg(test)]
#[path = "json/tests.rs"]
mod tests;
