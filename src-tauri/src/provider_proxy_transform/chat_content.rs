use serde::Deserialize;

const INVALID: &str = "Upstream Chat content contains unsupported or invalid parts";
const LIMIT: &str = "Upstream Chat content exceeded the content limit";

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Text,
    Thinking,
}

impl Kind {
    pub(crate) fn block_type(&self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Thinking => "thinking",
        }
    }

    pub(crate) fn delta_type(&self) -> &'static str {
        match self {
            Self::Text => "text_delta",
            Self::Thinking => "thinking_delta",
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Part {
    pub kind: Kind,
    pub text: String,
}

pub(crate) fn reasoning<'a>(
    first: Option<&'a str>,
    second: Option<&'a str>,
) -> Result<Option<&'a str>, &'static str> {
    match (
        first.filter(|s| !s.is_empty()),
        second.filter(|s| !s.is_empty()),
    ) {
        (Some(first), Some(second)) if first != second => {
            Err("Upstream Chat reasoning aliases are ambiguous")
        }
        (Some(first), _) => Ok(Some(first)),
        (_, second) => Ok(second),
    }
}

#[derive(Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
enum WirePart {
    #[serde(rename = "text", alias = "output_text")]
    Text { text: String },
    #[serde(rename = "refusal")]
    Refusal { refusal: String },
    #[serde(rename = "thinking")]
    Thinking {
        thinking: Thinking,
        #[serde(default, rename = "closed")]
        _closed: Option<bool>,
    },
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Thinking {
    Text(String),
    Parts(Vec<ThinkingText>),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ThinkingText {
    #[serde(rename = "type")]
    kind: String,
    text: String,
}

/// Translate only fully understood parts. Unknown/signed content must not be
/// silently dropped: callers either keep the raw bytes or return a masked error.
/// Limits apply to a single already-bounded body/event, never the entire stream.
pub(crate) fn parse(raw: &str) -> Result<Vec<Part>, &'static str> {
    if raw.len() > super::stream_frames::MAX_FRAME_BYTES {
        return Err(LIMIT);
    }
    let raw = raw.trim();
    if raw == "null" {
        return Ok(Vec::new());
    }
    if raw.starts_with('"') {
        let text = serde_json::from_str(raw).map_err(|_| INVALID)?;
        return Ok(vec![Part {
            kind: Kind::Text,
            text,
        }]);
    }
    let wire: Vec<WirePart> = serde_json::from_str(raw).map_err(|_| INVALID)?;
    let mut count = wire.len();
    if count > super::stream_limits::MAX_BLOCKS {
        return Err(LIMIT);
    }
    let mut parts = Vec::with_capacity(count);
    for part in wire {
        let (kind, text) = match part {
            WirePart::Text { text } => (Kind::Text, text),
            WirePart::Refusal { refusal } => (Kind::Text, refusal),
            WirePart::Thinking { thinking, .. } => {
                let text = match thinking {
                    Thinking::Text(text) => text,
                    Thinking::Parts(inner) => {
                        count = count.saturating_add(inner.len());
                        if count > super::stream_limits::MAX_BLOCKS {
                            return Err(LIMIT);
                        }
                        let mut text = String::new();
                        for item in inner {
                            if item.kind != "text" {
                                return Err(INVALID);
                            }
                            text.push_str(&item.text);
                        }
                        text
                    }
                };
                (Kind::Thinking, text)
            }
        };
        parts.push(Part { kind, text });
    }
    Ok(parts)
}

#[cfg(test)]
mod tests;
