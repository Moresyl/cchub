use super::*;
use crate::provider_proxy_transform::{
    create_anthropic_sse_stream, create_anthropic_sse_stream_from_gemini,
    create_anthropic_sse_stream_from_responses,
};
use bytes::Bytes;
use futures_util::StreamExt;
use serde_json::{json, Value};

fn wire(value: Value) -> String {
    format!("data: {value}\n\n")
}
fn chat(tool: Value) -> String {
    wire(json!({"id":"x","model":"m","choices":[{"delta":{"tool_calls":[tool]}}]}))
}

async fn output(format: &str, input: String) -> String {
    let source = futures_util::stream::iter([Ok::<_, std::io::Error>(Bytes::from(input))]);
    let chunks = match format {
        "chat" => {
            create_anthropic_sse_stream(source)
                .collect::<Vec<_>>()
                .await
        }
        "responses" => {
            create_anthropic_sse_stream_from_responses(source)
                .collect::<Vec<_>>()
                .await
        }
        "gemini" => {
            create_anthropic_sse_stream_from_gemini(source, "fixture".into())
                .collect::<Vec<_>>()
                .await
        }
        _ => unreachable!(),
    };
    chunks
        .into_iter()
        .map(|chunk| String::from_utf8(chunk.unwrap().to_vec()).unwrap())
        .collect()
}

fn rejected(output: &str) {
    assert_eq!(output.matches("event: error\n").count(), 1);
    assert!(!output.contains("event: message_stop\n"));
    assert!(!output.contains("private-secret"));
}

#[tokio::test]
async fn chat_tool_state_accepts_the_boundary_and_rejects_new_slots_before_starting_them() {
    for count in [MAX_BLOCKS, MAX_BLOCKS + 1] {
        let mut input = String::new();
        for index in 0..count {
            input += &chat(
                json!({"index":index,"id":format!("call_{index}"),"function":{"name":"run","arguments":"{}"}}),
            );
        }
        input += "data: [DONE]\n\n";
        let output = output("chat", input).await;
        assert_eq!(
            output.matches("event: content_block_start\n").count(),
            MAX_BLOCKS
        );
        if count > MAX_BLOCKS {
            rejected(&output);
            assert!(output.contains("block limit"));
        } else {
            assert!(!output.contains("event: error\n"));
            assert!(output.contains("message_stop"));
        }
    }
}

#[tokio::test]
async fn chat_tool_identity_rejects_oversize_ids_and_names_without_echoing_them() {
    for (id, name) in [
        (
            format!("private-secret{}", "x".repeat(MAX_ID_BYTES)),
            "run".into(),
        ),
        (
            "call".into(),
            format!("private-secret{}", "x".repeat(MAX_ID_BYTES)),
        ),
    ] {
        let output = output(
            "chat",
            chat(json!({"index":0,"id":id,"function":{"name":name,"arguments":"{}"}}))
                + "data: [DONE]\n\n",
        )
        .await;
        rejected(&output);
        assert!(!output.contains("event: content_block_start\n"));
    }
}

#[tokio::test]
async fn pending_chat_arguments_are_bounded_across_calls_and_released_after_identity_arrives() {
    let piece = "x".repeat(64 * 1024);
    let mut input = String::new();
    for index in 0..=MAX_PENDING_ARGS / piece.len() {
        input += &chat(json!({"index":index % 2,"function":{"arguments":piece}}));
    }
    input += "data: [DONE]\n\n";
    let result = output("chat", input).await;
    rejected(&result);
    assert!(result.contains("pending tool arguments"));
    let mut input = String::new();
    for index in [0, 1] {
        for _ in 0..MAX_PENDING_ARGS / piece.len() {
            input += &chat(json!({"index":index,"function":{"arguments":piece}}));
        }
        input +=
            &chat(json!({"index":index,"id":format!("call_{index}"),"function":{"name":"run"}}));
    }
    input += "data: [DONE]\n\n";
    let result = output("chat", input).await;
    assert!(!result.contains("event: error\n"));
    assert_eq!(result.matches("event: content_block_start\n").count(), 2);
    assert_eq!(result.matches("event: message_stop\n").count(), 1);
}

#[tokio::test]
async fn responses_text_and_reasoning_share_the_content_budget() {
    let mut input = String::new();
    for index in 0..=MAX_BLOCKS / 2 {
        input += &wire(
            json!({"type":"response.reasoning_summary_text.done","item_id":"reason","summary_index":index,"text":"thought"}),
        );
        input += &wire(
            json!({"type":"response.output_text.delta","item_id":format!("message_{index}"),"content_index":0,"delta":"answer"}),
        );
        input += &wire(
            json!({"type":"response.output_text.done","item_id":format!("message_{index}"),"content_index":0}),
        );
    }
    input += &wire(json!({"type":"response.completed","response":{"status":"completed"}}));
    let result = output("responses", input).await;
    rejected(&result);
    assert_eq!(
        result.matches("event: content_block_start\n").count(),
        MAX_BLOCKS
    );
}

#[tokio::test]
async fn responses_tool_slots_are_bounded_and_long_aliases_fail_before_allocation() {
    let mut input = String::new();
    for index in 0..=MAX_BLOCKS {
        input += &wire(
            json!({"type":"response.output_item.added","item":{"type":"function_call","id":format!("item_{index}"),"call_id":format!("call_{index}"),"name":"run"}}),
        );
    }
    input += &wire(json!({"type":"response.completed","response":{"status":"completed"}}));
    let result = output("responses", input).await;
    rejected(&result);
    assert_eq!(
        result.matches("event: content_block_start\n").count(),
        MAX_BLOCKS
    );
    for field in ["id", "call_id", "name"] {
        let mut item = json!({"type":"function_call","id":"item","call_id":"call","name":"run"});
        item[field] = json!(format!("private-secret{}", "x".repeat(MAX_ID_BYTES)));
        let result = output(
            "responses",
            wire(json!({"type":"response.output_item.added","item":item})),
        )
        .await;
        rejected(&result);
        assert!(!result.contains("event: content_block_start\n"));
    }
}

#[tokio::test]
async fn gemini_text_tool_transitions_are_bounded_before_normal_completion() {
    let mut input = String::new();
    for index in 0..=MAX_BLOCKS / 2 {
        input += &wire(
            json!({"candidates":[{"content":{"parts":[{"text":"answer"},{"functionCall":{"id":format!("call_{index}"),"name":"run","args":{}}}]}}]}),
        );
    }
    input += &wire(json!({"candidates":[{"finishReason":"STOP"}]}));
    let result = output("gemini", input).await;
    rejected(&result);
    assert_eq!(
        result.matches("event: content_block_start\n").count(),
        MAX_BLOCKS
    );
}
