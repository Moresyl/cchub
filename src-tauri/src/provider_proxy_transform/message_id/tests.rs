use super::*;
use crate::provider_proxy_transform::*;
use bytes::Bytes;
use futures_util::{Stream, StreamExt};
use serde_json::{json, Value};
use std::collections::HashSet;

fn source(wire: String) -> impl Stream<Item = Result<Bytes, std::io::Error>> + Send {
    let chunks = wire
        .as_bytes()
        .chunks(7)
        .map(Bytes::copy_from_slice)
        .collect::<Vec<_>>();
    futures_util::stream::iter(chunks.into_iter().map(Ok))
}

async fn events(stream: impl Stream<Item = Result<Bytes, std::io::Error>>) -> Vec<Value> {
    let wire = stream
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .flat_map(|chunk| chunk.unwrap().to_vec())
        .collect::<Vec<_>>();
    String::from_utf8(wire)
        .unwrap()
        .lines()
        .filter_map(|line| line.strip_prefix("data: "))
        .filter(|line| *line != "[DONE]")
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

fn start_id(events: &[Value]) -> &str {
    let starts = events
        .iter()
        .filter(|event| event["type"] == "message_start")
        .collect::<Vec<_>>();
    assert_eq!(
        starts.len(),
        1,
        "each converted response must start once: {events:?}"
    );
    let id = starts[0]["message"]["id"].as_str().unwrap();
    assert!(id.starts_with("msg_") && id.len() > 4);
    id
}

#[test]
fn normalizes_whole_chat_and_responses_messages_without_changing_tool_call_ids() {
    for (input, expected) in [
        ("chatcmpl-123", "msg_123"),
        ("resp_123", "msg_123"),
        ("msg_abc", "msg_abc"),
        ("opaque-123", "msg_opaque-123"),
    ] {
        let chat = openai_to_anthropic(json!({"id":input,"model":"m","choices":[{
            "message":{"role":"assistant","content":"ok","tool_calls":[{
                "id":"call_123","type":"function","function":{"name":"run","arguments":"{}"}
            }]},"finish_reason":"tool_calls"
        }]}))
        .unwrap();
        assert_eq!(chat["id"], expected);
        assert!(chat["content"]
            .as_array()
            .unwrap()
            .iter()
            .any(|block| block["id"] == "call_123"));
        let response = responses_to_anthropic(
            json!({"id":input,"model":"m","status":"completed","output":[{
                "type":"function_call","call_id":"call_123","name":"run","arguments":"{}"
            }]}),
        )
        .unwrap();
        assert_eq!(response["id"], expected);
        assert_eq!(response["content"][0]["id"], "call_123");
    }
}

#[test]
fn missing_and_empty_ids_get_distinct_random_identifiers_including_gemini() {
    let mut seen = HashSet::new();
    for value in [
        None,
        Some(""),
        Some(" "),
        Some("msg_"),
        Some("resp_"),
        Some("chatcmpl-"),
    ] {
        for _ in 0..16 {
            let id = anthropic_message_id(value);
            assert!(uuid::Uuid::parse_str(id.strip_prefix("msg_").unwrap()).is_ok());
            assert!(seen.insert(id));
        }
    }
    for _ in 0..16 {
        let chat = openai_to_anthropic(json!({"choices":[{"message":{"content":"ok"}}]})).unwrap();
        let response = responses_to_anthropic(json!({"output":[],"status":"completed"})).unwrap();
        let gemini = crate::gemini_transform::gemini_to_anthropic(
            json!({"candidates":[{
                "content":{"parts":[{"text":"ok"}]},"finishReason":"STOP"
            }]}),
            "m",
        )
        .unwrap();
        for value in [chat, response, gemini] {
            assert!(seen.insert(value["id"].as_str().unwrap().to_string()));
        }
    }
}

#[tokio::test]
async fn chat_streams_normalize_ids_and_accept_an_absent_id_with_one_stable_start() {
    for id in [Some("chatcmpl-abc"), Some("msg_original"), Some(""), None] {
        let mut first = json!({"model":"m","choices":[{"delta":{"content":"hi"}}]});
        if let Some(id) = id {
            first["id"] = json!(id);
        }
        let second = json!({"id":"chatcmpl-different","model":"m","choices":[{"delta":{},"finish_reason":"stop"}]});
        let wire = format!("data: {first}\n\ndata: {second}\n\ndata: [DONE]\n\n");
        let output = events(create_anthropic_sse_stream(source(wire))).await;
        let observed = start_id(&output);
        match id {
            Some("chatcmpl-abc") => assert_eq!(observed, "msg_abc"),
            Some("msg_original") => assert_eq!(observed, "msg_original"),
            _ => assert!(uuid::Uuid::parse_str(observed.strip_prefix("msg_").unwrap()).is_ok()),
        }
        assert!(output.iter().any(|event| event["type"] == "message_stop"));
    }
}

#[tokio::test]
async fn responses_streams_keep_the_first_start_when_created_is_repeated_or_late() {
    let created =
        "event: response.created\ndata: {\"response\":{\"id\":\"resp_abc\",\"model\":\"m\"}}\n\n";
    let part = "event: response.content_part.added\ndata: {\"part\":{\"type\":\"output_text\"},\"item_id\":\"i\",\"content_index\":0}\n\n";
    let completed = "event: response.completed\ndata: {\"response\":{\"status\":\"completed\",\"usage\":{\"input_tokens\":1,\"output_tokens\":1}}}\n\n";
    for (wire, expected) in [
        (
            format!("{created}{created}{part}{completed}"),
            Some("msg_abc"),
        ),
        (format!("{part}{created}{completed}"), None),
        (format!("{part}{completed}"), None),
    ] {
        let output = events(create_anthropic_sse_stream_from_responses(source(wire))).await;
        let observed = start_id(&output);
        if let Some(expected) = expected {
            assert_eq!(observed, expected);
        } else {
            assert!(uuid::Uuid::parse_str(observed.strip_prefix("msg_").unwrap()).is_ok());
        }
        assert!(output.iter().any(|event| event["type"] == "message_stop"));
    }
}

#[tokio::test]
async fn gemini_streams_do_not_share_millisecond_based_message_ids() {
    let wire = "data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"hi\"}]},\"finishReason\":\"STOP\"}]}\n\n";
    let mut seen = HashSet::new();
    for _ in 0..16 {
        let output = events(create_anthropic_sse_stream_from_gemini(
            source(wire.into()),
            "m".into(),
        ))
        .await;
        let id = start_id(&output);
        assert!(uuid::Uuid::parse_str(id.strip_prefix("msg_").unwrap()).is_ok());
        assert!(seen.insert(id.to_string()));
    }
}

#[tokio::test]
async fn responses_without_created_start_before_content_and_stop_once() {
    let completed = "event: response.completed\ndata: {\"response\":{\"id\":\"resp_final\",\"status\":\"completed\",\"usage\":{\"input_tokens\":1,\"output_tokens\":1}}}\n\n";
    for (name, data) in [
        ("response.output_text.delta", json!({"delta":"hi"})),
        ("response.refusal.delta", json!({"delta":"refused"})),
        ("response.reasoning.delta", json!({"delta":"thinking"})),
        (
            "response.function_call_arguments.delta",
            json!({"delta":"{}","call_id":"call_1","name":"run"}),
        ),
        (
            "response.output_item.added",
            json!({"item":{"type":"function_call","id":"item_1","call_id":"call_1","name":"run"}}),
        ),
        (
            "response.output_item.done",
            json!({"item":{"type":"web_search_call","id":"ws_1","action":{"sources":[]}}}),
        ),
    ] {
        let wire = format!("event: {name}\ndata: {data}\n\n{completed}{completed}");
        let output = events(create_anthropic_sse_stream_from_responses(source(wire))).await;
        start_id(&output);
        assert_eq!(output[0]["type"], "message_start", "{name}");
        assert_eq!(output.last().unwrap()["type"], "message_stop");
        assert_eq!(
            output
                .iter()
                .filter(|event| event["type"] == "message_stop")
                .count(),
            1
        );
        if name.contains("function_call") || name == "response.output_item.added" {
            assert!(output
                .iter()
                .any(|event| event["delta"]["stop_reason"] == "tool_use"));
        }
    }
    let output = events(create_anthropic_sse_stream_from_responses(source(format!(
        "{completed}{completed}"
    ))))
    .await;
    assert_eq!(start_id(&output), "msg_final");
    assert_eq!(output.len(), 3);
}
