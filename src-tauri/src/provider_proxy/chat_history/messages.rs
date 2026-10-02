use bytes::Bytes;
use serde::de::{MapAccess, Visitor};
use serde::{Deserialize, Deserializer};
use serde_json::{value::RawValue, Value};
use std::borrow::Cow;
use std::collections::HashSet;
use std::fmt;

use super::FIELDS;

/// Reject duplicate keys rather than guessing which message the server used.
struct Object<'a>(Vec<(String, &'a RawValue)>);

impl<'de> Deserialize<'de> for Object<'de> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ObjectVisitor;
        impl<'de> Visitor<'de> for ObjectVisitor {
            type Value = Object<'de>;
            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("an object with unique keys")
            }
            fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Self::Value, M::Error> {
                let mut seen = HashSet::new();
                let mut entries = Vec::new();
                while let Some((key, value)) = map.next_entry::<String, &RawValue>()? {
                    if !seen.insert(key.clone()) {
                        return Err(serde::de::Error::custom("duplicate key"));
                    }
                    entries.push((key, value));
                }
                Ok(Object(entries))
            }
        }
        deserializer.deserialize_map(ObjectVisitor)
    }
}

impl<'a> Object<'a> {
    fn get(&self, key: &str) -> Option<&'a RawValue> {
        self.0
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| *value)
    }

    fn assistant(&self) -> bool {
        self.get("role")
            .and_then(|v| serde_json::from_str::<String>(v.get()).ok())
            .is_some_and(|role| role == "assistant")
    }

    fn fields(&self) -> u8 {
        FIELDS.iter().enumerate().fold(0, |fields, (index, key)| {
            fields
                | if self.get(key).is_some() {
                    1 << index
                } else {
                    0
                }
        })
    }
}

fn thoughts(message: &Object<'_>) -> Option<String> {
    let mut text = Vec::<String>::new();
    for key in &FIELDS[..2] {
        if let Some(value) = message.get(key).filter(|v| v.get() != "null") {
            let part: String = serde_json::from_str(value.get()).ok()?;
            if !part.is_empty() && !text.contains(&part) {
                text.push(part);
            }
        }
    }
    if let Some(details) = message
        .get("reasoning_details")
        .filter(|v| v.get() != "null")
    {
        for detail in serde_json::from_str::<Vec<Object<'_>>>(details.get()).ok()? {
            // Encrypted/signed/unknown history remains untouched. Plain thinking
            // can be converted without discarding opaque provider metadata.
            if detail
                .0
                .iter()
                .any(|(key, _)| !matches!(key.as_str(), "type" | "text" | "summary"))
            {
                return None;
            }
            if let Some(kind) = detail.get("type") {
                let kind: String = serde_json::from_str(kind.get()).ok()?;
                if !matches!(
                    kind.as_str(),
                    "text" | "reasoning.text" | "reasoning.summary" | "summary"
                ) {
                    return None;
                }
            }
            for key in ["text", "summary"] {
                if let Some(value) = detail.get(key).filter(|v| v.get() != "null") {
                    let part: String = serde_json::from_str(value.get()).ok()?;
                    if !part.is_empty() && !text.contains(&part) {
                        text.push(part);
                    }
                }
            }
        }
    }
    Some(text.join("\n"))
}

fn content(message: &Object<'_>, thinking: &str) -> Option<String> {
    let mut parts = Vec::<Cow<'_, str>>::new();
    if !thinking.is_empty() {
        parts.push(Cow::Owned(
            serde_json::json!({"type":"thinking", "thinking":[{"type":"text","text":thinking}]})
                .to_string(),
        ));
    }
    if let Some(content) = message.get("content").filter(|v| v.get() != "null") {
        if content.get().starts_with('[') {
            let original: Vec<&RawValue> = serde_json::from_str(content.get()).ok()?;
            parts.extend(original.into_iter().map(|part| Cow::Borrowed(part.get())));
        } else {
            let text: String = serde_json::from_str(content.get()).ok()?;
            if !text.is_empty() {
                parts.push(Cow::Owned(
                    serde_json::json!({"type":"text","text":text}).to_string(),
                ));
            }
        }
    }
    Some(format!("[{}]", parts.join(",")))
}

fn edited_message(message: Object<'_>, mistral: bool, refused: u8) -> Option<Vec<u8>> {
    if !message.assistant() {
        return None;
    }
    let mut fields = message.fields() & refused;
    let converted = if mistral && message.fields() != 0 {
        let thinking = thoughts(&message)?;
        // Empty aliases do not justify changing null/absent/string content.
        let next = if thinking.is_empty() {
            None
        } else {
            Some(content(&message, &thinking)?)
        };
        fields |= message.fields();
        next
    } else {
        None
    };
    if fields == 0 {
        return None;
    }
    let mut entries = Vec::new();
    let mut had_content = false;
    for (key, value) in message.0 {
        if FIELDS
            .iter()
            .enumerate()
            .any(|(index, name)| fields & (1 << index) != 0 && name == &key)
        {
            continue;
        }
        if key == "content" {
            had_content = true;
            entries.push((key, converted.as_deref().unwrap_or(value.get())));
        } else {
            entries.push((key, value.get()));
        }
    }
    if !had_content {
        if let Some(content) = &converted {
            entries.push(("content".into(), content));
        }
    }
    let mut output = Vec::new();
    output.push(b'{');
    for (index, (key, value)) in entries.into_iter().enumerate() {
        if index != 0 {
            output.push(b',');
        }
        output.extend(serde_json::to_vec(&key).ok()?);
        output.push(b':');
        output.extend(value.as_bytes());
    }
    output.push(b'}');
    Some(output)
}

pub(super) fn edit(body: &Bytes, mistral: bool, refused: u8) -> Option<Bytes> {
    if !mistral && refused == 0 {
        return None;
    }
    let source = std::str::from_utf8(body).ok()?;
    let root: Object<'_> = serde_json::from_str(source).ok()?;
    let messages: Vec<&RawValue> = serde_json::from_str(root.get("messages")?.get()).ok()?;
    let mut output = Vec::with_capacity(body.len());
    let mut cursor = 0;
    for raw in messages {
        let Ok(message) = serde_json::from_str::<Object<'_>>(raw.get()) else {
            continue;
        };
        let Some(next) = edited_message(message, mistral, refused) else {
            continue;
        };
        let start = raw
            .get()
            .as_ptr()
            .addr()
            .checked_sub(source.as_ptr().addr())?;
        let end = start.checked_add(raw.get().len())?;
        if source.get(start..end) != Some(raw.get()) {
            return None;
        }
        output.extend_from_slice(body.get(cursor..start)?);
        output.extend(next);
        cursor = end;
    }
    if cursor == 0 {
        return None;
    }
    output.extend_from_slice(body.get(cursor..)?);
    Some(Bytes::from(output))
}

/// Only structured validation errors identifying an existing assistant field
/// are evidence of a schema mismatch. Strings mentioning a field are not.
pub(super) fn refused(error: &Bytes, body: &Bytes) -> u8 {
    if error.len() > 64 * 1024 {
        return 0;
    }
    let Ok(root) = serde_json::from_slice::<Object<'_>>(body) else {
        return 0;
    };
    let Some(messages) = root.get("messages") else {
        return 0;
    };
    let Ok(messages) = serde_json::from_str::<Vec<&RawValue>>(messages.get()) else {
        return 0;
    };
    let Ok(error) = serde_json::from_slice::<Value>(error) else {
        return 0;
    };
    let mut pending = vec![&error];
    let mut fields = 0;
    let mut visited = 0;
    while let Some(value) = pending.pop() {
        visited += 1;
        if visited > 4096 {
            break;
        }
        match value {
            Value::Array(values) => pending.extend(values),
            Value::Object(object) => {
                if object.get("type").and_then(Value::as_str) == Some("extra_forbidden") {
                    if let Some(location) = object.get("loc").and_then(Value::as_array) {
                        let location = if location.first().and_then(Value::as_str) == Some("body") {
                            &location[1..]
                        } else {
                            location.as_slice()
                        };
                        if location.len() == 3 && location[0].as_str() == Some("messages") {
                            let message = location[1]
                                .as_u64()
                                .and_then(|index| usize::try_from(index).ok())
                                .and_then(|index| messages.get(index))
                                .and_then(|raw| serde_json::from_str::<Object<'_>>(raw.get()).ok());
                            if let Some(message) = message.filter(Object::assistant) {
                                if let Some(index) = FIELDS
                                    .iter()
                                    .position(|key| Some(*key) == location[2].as_str())
                                {
                                    fields |= message.fields() & (1 << index);
                                }
                            }
                        }
                    }
                }
                // 'input' often repeats the rejected message. It is data, not
                // a second validation error; do not inspect it recursively.
                pending.extend(
                    object
                        .iter()
                        .filter(|(key, _)| {
                            matches!(key.as_str(), "error" | "detail" | "details" | "errors")
                        })
                        .map(|(_, value)| value),
                );
            }
            _ => {}
        }
    }
    fields
}
