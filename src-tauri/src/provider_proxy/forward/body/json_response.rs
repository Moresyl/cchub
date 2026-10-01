use serde::Deserialize;
use serde_json::{value::RawValue, Value};

#[derive(Deserialize)]
struct Envelope<'a> {
    #[serde(default, borrow)]
    error: Option<&'a RawValue>,
    #[serde(default, borrow, rename = "type")]
    kind: Option<&'a RawValue>,
}

/// A native JSON response may contain valid numeric spellings beyond Value's
/// floating-point range. Validate its syntax without rewriting opaque data,
/// while still rejecting explicit errors and ambiguous duplicate error fields.
pub(crate) fn json_success_error(
    parsed: Option<&Value>,
    bytes: &[u8],
    translated: bool,
) -> Option<&'static str> {
    if translated && parsed.is_none() {
        return Some("Upstream returned invalid JSON");
    }
    let raw: &RawValue = match serde_json::from_slice(bytes) {
        Ok(raw) => raw,
        Err(_) => return Some("Upstream returned invalid JSON"),
    };
    if !raw.get().starts_with('{') {
        return None;
    }
    let envelope: Envelope<'_> = match serde_json::from_str(raw.get()) {
        Ok(envelope) => envelope,
        Err(_) => return Some("Upstream returned ambiguous JSON error fields"),
    };
    let is_error = envelope.error.is_some_and(|error| error.get() != "null")
        || envelope
            .kind
            .and_then(|kind| serde_json::from_str::<String>(kind.get()).ok())
            .as_deref()
            == Some("error");
    is_error.then_some("Upstream returned an in-band error")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_validates_large_opaque_numbers_without_requiring_value_conversion() {
        let raw = br#"{"choices":[],"opaque":1.2300e+999}"#;
        assert!(serde_json::from_slice::<Value>(raw).is_err());
        assert_eq!(json_success_error(None, raw, false), None);
        assert_eq!(
            json_success_error(None, raw, true),
            Some("Upstream returned invalid JSON")
        );
        for raw in [b"null".as_slice(), b"[]", b"\"text\"", b"123"] {
            assert_eq!(json_success_error(None, raw, false), None);
        }
    }

    #[test]
    fn native_explicit_and_duplicate_errors_cannot_hide_behind_opaque_fields() {
        for raw in [
            r#"{"error":{"message":"secret"},"opaque":1.2300e+999}"#,
            r#"{"type":"error","opaque":1.2300e+999}"#,
            r#"{"error":"secret","error":null}"#,
            r#"{"error":null,"error":null}"#,
            r#"{"type":"message","type":"error"}"#,
            "{invalid",
        ] {
            let parsed = serde_json::from_str(raw).ok();
            let message = json_success_error(parsed.as_ref(), raw.as_bytes(), false).unwrap();
            assert!(!message.contains("secret"));
        }
        assert_eq!(
            json_success_error(None, br#"{"error":null,"opaque":1.2300e+999}"#, false),
            None
        );
    }
}
