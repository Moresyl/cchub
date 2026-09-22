use axum::http::HeaderMap;
use bytes::Bytes;
use futures_util::{Stream, StreamExt};
use rusqlite::Connection;
use serde_json::{json, Value};

use super::ProfileCandidate;
use crate::commands::claude_desktop_profiles::{active_proxy_provider, gateway_token};

pub(super) fn authorize(conn: &Connection, headers: &HeaderMap) -> Result<(), String> {
    let token = gateway_token(conn)?;
    let bearer = headers
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "));
    let api_key = headers
        .get("x-api-key")
        .and_then(|value| value.to_str().ok());
    if bearer == Some(token.as_str()) || api_key == Some(token.as_str()) {
        Ok(())
    } else {
        Err("Invalid Claude Desktop gateway token".to_string())
    }
}

pub(super) fn profile_candidates(conn: &Connection) -> Result<Vec<ProfileCandidate>, String> {
    let provider = active_proxy_provider(conn)?;
    let snapshot = json!({
        "env": {
            "ANTHROPIC_BASE_URL": provider.base_url,
            "ANTHROPIC_AUTH_TOKEN": provider.api_key,
            "ANTHROPIC_API_FORMAT": provider.api_format,
        },
        "desktopModelRoutes": provider.model_routes,
    });
    Ok(vec![ProfileCandidate {
        profile_id: provider.id,
        profile_name: provider.name,
        snapshot: snapshot.to_string(),
    }])
}

pub(super) fn rewrite_model(body: &[u8], snapshot: &str) -> Result<Vec<u8>, String> {
    let parsed: Value = serde_json::from_str(snapshot).map_err(|error| error.to_string())?;
    let routes = parsed
        .get("desktopModelRoutes")
        .and_then(Value::as_object)
        .ok_or("Claude Desktop model routes are missing")?;
    let mut request: Value = serde_json::from_slice(body)
        .map_err(|_| "Claude Desktop request must contain JSON".to_string())?;
    let model = request
        .get("model")
        .and_then(Value::as_str)
        .ok_or("Claude Desktop request is missing a model")?;
    let upstream = routes
        .get(model)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("No upstream model route for {model}"))?;
    request["model"] = Value::String(upstream.to_string());
    serde_json::to_vec(&request).map_err(|error| error.to_string())
}

pub(super) fn restore_response_model(response: &mut Value, original_request: &[u8]) {
    let Some(model) = serde_json::from_slice::<Value>(original_request)
        .ok()
        .and_then(|value| {
            value
                .get("model")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
    else {
        return;
    };
    if response.get("model").and_then(Value::as_str).is_some() {
        response["model"] = Value::String(model);
    }
}

fn restore_sse_event(event: &[u8], model: &str) -> Vec<u8> {
    let Ok(text) = std::str::from_utf8(event) else {
        return event.to_vec();
    };
    let mut lines = text
        .lines()
        .map(|line| line.trim_end_matches('\r').to_string())
        .collect::<Vec<_>>();
    let data_lines = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| line.starts_with("data:"))
        .map(|(index, line)| {
            (
                index,
                line.trim_start_matches("data:").trim_start().to_string(),
            )
        })
        .collect::<Vec<_>>();
    if data_lines.len() != 1 {
        return event.to_vec();
    }
    let (index, payload) = &data_lines[0];
    let Ok(mut parsed) = serde_json::from_str::<Value>(payload) else {
        return event.to_vec();
    };
    if parsed
        .pointer("/message/model")
        .and_then(Value::as_str)
        .is_some()
    {
        parsed["message"]["model"] = json!(model);
    } else if parsed.get("model").and_then(Value::as_str).is_some() {
        parsed["model"] = json!(model);
    } else {
        return event.to_vec();
    }
    lines[*index] = format!("data: {}", parsed);
    lines.join("\n").into_bytes()
}

fn event_boundary(buffer: &[u8]) -> Option<(usize, usize)> {
    let lf = buffer
        .windows(2)
        .position(|part| part == b"\n\n")
        .map(|at| (at, 2));
    let crlf = buffer
        .windows(4)
        .position(|part| part == b"\r\n\r\n")
        .map(|at| (at, 4));
    match (lf, crlf) {
        (Some(left), Some(right)) => Some(if left.0 < right.0 { left } else { right }),
        (left, right) => left.or(right),
    }
}

pub(super) fn restore_stream_model<S, E>(
    stream: S,
    model: Option<String>,
) -> impl Stream<Item = Result<Bytes, std::io::Error>> + Send
where
    S: Stream<Item = Result<Bytes, E>> + Send + 'static,
    E: std::error::Error + Send + 'static,
{
    async_stream::stream! {
        let mut pending = Vec::new();
        tokio::pin!(stream);
        while let Some(chunk) = stream.next().await {
            let bytes = match chunk { Ok(bytes) => bytes, Err(error) => { yield Err(std::io::Error::other(error.to_string())); break; } };
            let Some(model) = model.as_deref() else { yield Ok(bytes); continue; };
            pending.extend_from_slice(&bytes);
            while let Some((index, delimiter_len)) = event_boundary(&pending) {
                let event = restore_sse_event(&pending[..index], model);
                let mut output = event;
                output.extend_from_slice(&pending[index..index + delimiter_len]);
                pending.drain(..index + delimiter_len);
                yield Ok(Bytes::from(output));
            }
            if pending.len() > 8 * 1024 * 1024 {
                yield Err(std::io::Error::other("Claude Desktop stream event exceeded 8 MiB"));
                break;
            }
        }
        if !pending.is_empty() { yield Ok(Bytes::from(pending)); }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_only_explicit_models_without_dropping_other_request_fields() {
        let snapshot =
            json!({"desktopModelRoutes": {"claude-sonnet-4-6": "upstream-model"}}).to_string();
        let body = br#"{"model":"claude-sonnet-4-6","messages":[{"role":"user","content":"hello"}],"stream":true}"#;
        let mapped: Value =
            serde_json::from_slice(&rewrite_model(body, &snapshot).unwrap()).unwrap();
        assert_eq!(mapped["model"], "upstream-model");
        assert_eq!(mapped["messages"][0]["content"], "hello");
        assert!(rewrite_model(br#"{"model":"claude-opus-4-6"}"#, &snapshot).is_err());
    }

    #[test]
    fn response_model_is_restored_without_overwriting_usage() {
        let mut response = json!({"model":"upstream-model","usage":{"input_tokens":2}});
        restore_response_model(&mut response, br#"{"model":"claude-sonnet-4-6"}"#);
        assert_eq!(response["model"], "claude-sonnet-4-6");
        assert_eq!(response["usage"]["input_tokens"], 2);
    }

    #[test]
    fn requires_the_gateway_token() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute(
            "CREATE TABLE app_settings (key TEXT PRIMARY KEY, value TEXT)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO app_settings VALUES ('claude_desktop_gateway_token', 'random-token')",
            [],
        )
        .unwrap();
        let mut headers = HeaderMap::new();
        assert!(authorize(&conn, &headers).is_err());
        headers.insert("authorization", "Bearer wrong".parse().unwrap());
        assert!(authorize(&conn, &headers).is_err());
        headers.insert("authorization", "Bearer random-token".parse().unwrap());
        assert!(authorize(&conn, &headers).is_ok());
    }

    #[tokio::test]
    async fn rewrites_a_streamed_message_model_across_chunk_boundaries() {
        let input = futures_util::stream::iter(vec![
            Ok::<Bytes, std::io::Error>(Bytes::from_static(
                b"event: message_start\ndata: {\"message\":{\"model\":\"upstream\"}}\n",
            )),
            Ok::<Bytes, std::io::Error>(Bytes::from_static(b"\nevent: ping\ndata: {}\n\n")),
        ]);
        let chunks = restore_stream_model(input, Some("claude-sonnet-4-6".into()))
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        let output = chunks.concat();
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("\"model\":\"claude-sonnet-4-6\""));
        assert!(output.contains("event: ping\ndata: {}"));
    }
}
