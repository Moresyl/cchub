use crate::provider_proxy::ProxyRequestInsights;
use serde_json::Value;

fn parse_json_bytes(bytes: &[u8]) -> Option<Value> {
    serde_json::from_slice(bytes).ok()
}

fn extract_gemini_model_from_path(relative_path: &str) -> Option<String> {
    let (_, suffix) = relative_path.split_once("models/")?;
    let model = suffix
        .split(':')
        .next()
        .unwrap_or_default()
        .trim()
        .trim_start_matches('/');
    if model.is_empty() {
        None
    } else {
        decode_model_component(model)
    }
}

fn decode_model_component(model: &str) -> Option<String> {
    let mut bytes = Vec::with_capacity(model.len());
    let mut chars = model.bytes();
    while let Some(byte) = chars.next() {
        if byte == b'%' {
            let high = char::from(chars.next()?).to_digit(16)?;
            let low = char::from(chars.next()?).to_digit(16)?;
            bytes.push((high * 16 + low) as u8);
        } else {
            bytes.push(byte);
        }
    }
    String::from_utf8(bytes).ok()
}

pub(crate) fn extract_request_insights(
    tool_id: &str,
    relative_path: &str,
    body_bytes: &[u8],
) -> ProxyRequestInsights {
    let parsed = parse_json_bytes(body_bytes);
    let request_model = parsed
        .as_ref()
        .and_then(|value| value.get("model"))
        .and_then(|value| value.as_str())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .or_else(|| {
            if tool_id == "gemini"
                || relative_path.contains("GenerateContent")
                || relative_path.ends_with(":generateContent")
            {
                extract_gemini_model_from_path(relative_path)
            } else {
                None
            }
        });

    let is_streaming = parsed
        .as_ref()
        .and_then(|value| value.get("stream"))
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
        || relative_path.contains("streamGenerateContent");

    ProxyRequestInsights {
        request_model,
        upstream_model: None,
        is_streaming,
    }
}
