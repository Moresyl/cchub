use super::*;
use bytes::Bytes;
use futures_util::{stream, StreamExt};
use std::collections::HashSet;

fn call(id: &str, name: &str) -> Value {
    json!({"type":"tool_use","id":id,"name":name,"input":{"path":"你好.txt"}})
}

fn result(id: &str, text: &str) -> Value {
    json!({"type":"tool_result","tool_use_id":id,"content":text})
}

fn history(blocks: Vec<Value>, results: Vec<Value>) -> Value {
    json!({"model":"fixture","messages":[
        {"role":"assistant","content":blocks},
        {"role":"user","content":results}
    ]})
}

#[test]
fn matches_parallel_calls_by_id_with_reordered_results_and_preserves_errors() {
    let mut failed = result("native-c", "failed");
    failed["is_error"] = json!(true);
    let mut array_result = result("native-a", "");
    array_result["content"] =
        json!([{"type":"text","text":"first"},{"type":"text","text":"second"}]);
    let (body, _) = anthropic_to_gemini(history(
        vec![
            call("native-a", "read_file"),
            call("native-b", "read_file"),
            call("native-c", "run"),
        ],
        vec![result("native-b", "B"), failed, array_result],
    ))
    .unwrap();
    for (index, id) in ["native-a", "native-b", "native-c"].into_iter().enumerate() {
        assert_eq!(
            body["contents"][0]["parts"][index]["functionCall"]["id"],
            id
        );
    }
    let parts = &body["contents"][1]["parts"];
    assert_eq!(
        parts[0]["functionResponse"],
        json!({"id":"native-b","name":"read_file","response":{"result":"B"}})
    );
    assert_eq!(
        parts[1]["functionResponse"],
        json!({"id":"native-c","name":"run","response":{"error":"failed"}})
    );
    assert_eq!(
        parts[2]["functionResponse"],
        json!({"id":"native-a","name":"read_file","response":{"result":"first\nsecond"}})
    );
}

#[test]
fn rejects_orphan_repeated_ambiguous_and_invalid_history_without_echoing_private_content() {
    let cases = vec![
        history(
            vec![],
            vec![result("secret-unmatched-id", "private-result")],
        ),
        history(
            vec![call("id", "run")],
            vec![result("id", "one"), result("id", "private-result")],
        ),
        history(vec![call("id", "run"), call("id", "other")], vec![]),
        history(vec![call(" ", "run")], vec![]),
        history(vec![call("id", " ")], vec![]),
        history(vec![call("id", "run")], vec![result(" ", "private-result")]),
        history(
            vec![json!({"type":"tool_use","id":123,"name":"run"})],
            vec![],
        ),
    ];
    for body in cases {
        let error = anthropic_to_gemini(body).unwrap_err();
        assert!(error.starts_with("Gemini tool"));
        assert!(!error.contains("secret-unmatched-id"));
        assert!(!error.contains("private-result"));
    }
}

#[test]
fn bounds_history_and_does_not_rebind_an_id_after_its_result() {
    let mut messages = Vec::new();
    for index in 0..4096 {
        let id = format!("call-{index}");
        messages.push(json!({"role":"assistant","content":[call(&id, "run")]}));
        messages.push(json!({"role":"user","content":[result(&id, "done")]}));
    }
    let mut body = json!({"messages":messages});
    anthropic_to_gemini(body.clone()).unwrap();
    body["messages"]
        .as_array_mut()
        .unwrap()
        .push(json!({"role":"assistant","content":[call("extra", "run")]}));
    assert!(anthropic_to_gemini(body)
        .unwrap_err()
        .contains("4096-call limit"));
    let body = json!({"messages":[
        {"role":"assistant","content":[call("id", "run")]},
        {"role":"user","content":[result("id", "done")]},
        {"role":"assistant","content":[call("id", "other")]}
    ]});
    assert!(anthropic_to_gemini(body)
        .unwrap_err()
        .contains("duplicate call ID"));
}

#[test]
fn bounds_tool_identity_metadata_and_rejects_duplicate_reply_ids() {
    let oversized = "x".repeat(1025);
    assert!(
        anthropic_to_gemini(history(vec![call(&oversized, "run")], vec![]))
            .unwrap_err()
            .contains("1024-byte")
    );
    assert!(gemini_to_anthropic(
        response(vec![json!({"id":oversized,"name":"run"})]),
        "fixture"
    )
    .unwrap_err()
    .contains("1024-byte"));
    for name in ["private.name", &"x".repeat(129)] {
        assert!(anthropic_to_gemini(history(vec![call("id", name)], vec![]))
            .unwrap_err()
            .contains("invalid function name"));
    }
    assert!(gemini_to_anthropic(
        response(vec![
            json!({"id":"same","name":"run"}),
            json!({"id":"same","name":"read_file"})
        ]),
        "fixture"
    )
    .unwrap_err()
    .contains("duplicate call ID"));
    let mut replies = ToolReplyIds::default();
    for _ in 0..4096 {
        replies.next(&json!({"name":"run"})).unwrap();
    }
    assert!(replies
        .next(&json!({"name":"run"}))
        .unwrap_err()
        .contains("4096-call limit"));
}

#[tokio::test]
async fn duplicate_streamed_call_ids_emit_one_error_without_duplicate_tools_or_completion() {
    let data = format!(
        "data: {}\n\n",
        response(vec![
            json!({"id":"same","name":"run"}),
            json!({"id":"same","name":"read_file"})
        ])
    );
    let input = stream::iter([Ok::<_, std::io::Error>(Bytes::from(data))]);
    let output = crate::provider_proxy_transform::create_anthropic_sse_stream_from_gemini(
        input,
        "fixture".into(),
    );
    let chunks: Vec<_> = output.collect().await;
    let bytes: Vec<_> = chunks
        .into_iter()
        .flat_map(|chunk| chunk.unwrap().to_vec())
        .collect();
    let text = String::from_utf8(bytes).unwrap();
    assert_eq!(text.matches("event: content_block_start").count(), 1);
    assert_eq!(text.matches("event: error").count(), 1);
    assert!(!text.contains("event: message_stop"));
    assert!(!text.contains("\"name\":\"read_file\""));
}

fn response(calls: Vec<Value>) -> Value {
    json!({"candidates":[{"content":{"parts":calls.into_iter().map(|call| json!({"functionCall":call})).collect::<Vec<_>>()},"finishReason":"STOP"}]})
}

#[test]
fn preserves_native_response_ids_and_generates_unique_uuid_fallbacks() {
    let mut ids = HashSet::new();
    for _ in 0..32 {
        let body = gemini_to_anthropic(
            response(vec![
                json!({"id":"native-call","name":"run","args":{"path":"你好.txt"}}),
                json!({"id":" ","name":"run"}),
                json!({"id":false,"name":"run"}),
                json!({"name":"run"}),
            ]),
            "fixture",
        )
        .unwrap();
        assert_eq!(body["content"][0]["id"], "native-call");
        for call in body["content"].as_array().unwrap().iter().skip(1) {
            let id = call["id"].as_str().unwrap();
            assert!(uuid::Uuid::parse_str(id.strip_prefix("toolu_").unwrap()).is_ok());
            assert!(ids.insert(id.to_string()));
        }
        let (history, _) = anthropic_to_gemini(history(
            body["content"].as_array().unwrap().clone(),
            vec![result("native-call", "done")],
        ))
        .unwrap();
        assert_eq!(
            history["contents"][1]["parts"][0]["functionResponse"]["id"],
            "native-call"
        );
    }
}

#[tokio::test]
async fn fragmented_stream_retains_parallel_tool_ids_and_produces_balanced_blocks() {
    let mut ids = HashSet::new();
    for _ in 0..16 {
        let payload = format!(
            "data: {}\r\n\r\n",
            response(vec![
                json!({"id":"native-a","name":"read_file","args":{"path":"你好.txt"}}),
                json!({"id":"native-b","name":"read_file","args":{"path":"another.txt"}}),
                json!({"name":"run","args":{}}),
            ])
        );
        let input = stream::iter(
            payload
                .into_bytes()
                .into_iter()
                .map(|byte| Ok::<_, std::io::Error>(Bytes::from(vec![byte]))),
        );
        let input = crate::provider_proxy_transform::normalize_sse_stream(input);
        let output = crate::provider_proxy_transform::create_anthropic_sse_stream_from_gemini(
            input,
            "fixture".into(),
        );
        let output: Vec<_> = output.collect().await;
        let bytes: Vec<_> = output
            .into_iter()
            .flat_map(|chunk| chunk.unwrap().to_vec())
            .collect();
        let text = String::from_utf8(bytes).unwrap();
        let events: Vec<Value> = text
            .lines()
            .filter_map(|line| line.strip_prefix("data: "))
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        let starts: Vec<_> = events
            .iter()
            .filter(|event| event["type"] == "content_block_start")
            .collect();
        assert_eq!(starts.len(), 3);
        assert_eq!(starts[0]["content_block"]["id"], "native-a");
        assert_eq!(starts[1]["content_block"]["id"], "native-b");
        let generated = starts[2]["content_block"]["id"].as_str().unwrap();
        assert!(uuid::Uuid::parse_str(generated.strip_prefix("toolu_").unwrap()).is_ok());
        assert!(ids.insert(generated.to_owned()));
        for start in starts {
            let index = &start["index"];
            assert_eq!(
                events
                    .iter()
                    .filter(
                        |event| event["type"] == "content_block_stop" && event["index"] == *index
                    )
                    .count(),
                1
            );
        }
        let arguments = events
            .iter()
            .find(|event| event["type"] == "content_block_delta" && event["index"] == 0)
            .unwrap();
        let args: Value =
            serde_json::from_str(arguments["delta"]["partial_json"].as_str().unwrap()).unwrap();
        assert_eq!(args["path"], "你好.txt");
        assert_eq!(
            events
                .iter()
                .filter(|event| event["type"] == "message_stop")
                .count(),
            1
        );
        assert!(events
            .iter()
            .any(|event| event["delta"]["stop_reason"] == "tool_use"));
    }
}
