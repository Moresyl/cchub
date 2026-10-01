use super::*;
use crate::provider_proxy_transform::{
    create_anthropic_sse_stream_from_responses, normalize_sse_stream, responses_to_anthropic,
};
use bytes::Bytes;
use futures_util::StreamExt;
use std::collections::HashSet;

fn wire(event: &str, mut value: Value) -> String {
    value["type"] = json!(event);
    format!("event: {event}\ndata: {value}\n\n")
}

async fn convert(input: String) -> Vec<Value> {
    let source = futures_util::stream::iter(
        input
            .into_bytes()
            .into_iter()
            .map(|byte| Ok::<_, std::io::Error>(Bytes::from(vec![byte]))),
    );
    let chunks = create_anthropic_sse_stream_from_responses(normalize_sse_stream(source))
        .collect::<Vec<_>>()
        .await;
    let bytes = chunks
        .into_iter()
        .flat_map(|chunk| chunk.unwrap().to_vec())
        .collect::<Vec<_>>();
    String::from_utf8(bytes)
        .unwrap()
        .lines()
        .filter_map(|line| line.strip_prefix("data: "))
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

fn complete() -> String {
    wire(
        "response.completed",
        json!({"response":{"status":"completed","usage":{"input_tokens":3,"output_tokens":4}}}),
    )
}

fn texts(events: &[Value], field: &str) -> String {
    events
        .iter()
        .filter_map(|event| event["delta"][field].as_str())
        .collect()
}

fn balanced(events: &[Value]) {
    assert_eq!(events.first().unwrap()["type"], "message_start");
    assert_eq!(events.last().unwrap()["type"], "message_stop");
    let mut seen = HashSet::new();
    let mut open = HashSet::new();
    let mut types = HashMap::new();
    for event in events {
        let index = event["index"].as_u64().unwrap_or(0);
        match event["type"].as_str().unwrap() {
            "content_block_start" => {
                assert!(seen.insert(index), "index reopened: {events:?}");
                assert!(open.insert(index));
                types.insert(index, event["content_block"]["type"].as_str().unwrap());
            }
            "content_block_delta" => {
                assert!(open.contains(&index));
                if event["delta"]["type"] == "thinking_delta" {
                    assert_eq!(types[&index], "thinking");
                }
                if event["delta"]["type"] == "text_delta" {
                    assert_eq!(types[&index], "text");
                }
            }
            "content_block_stop" => assert!(open.remove(&index), "duplicate close: {events:?}"),
            _ => {}
        }
    }
    assert!(open.is_empty());
}

#[tokio::test]
async fn summary_parts_are_distinct_and_done_snapshots_do_not_repeat_deltas() {
    let mut input = String::new();
    for index in 0..2 {
        let fields = json!({"item_id":"rs_1","output_index":0,"summary_index":index});
        let mut added = fields.clone();
        added["part"] = json!({"type":"summary_text","text":""});
        input += &wire("response.reasoning_summary_part.added", added.clone());
        input += &wire("response.reasoning_summary_part.added", added);
        let mut delta = fields.clone();
        delta["delta"] = json!(format!("思考{index}🦀"));
        input += &wire("response.reasoning_summary_text.delta", delta);
        let mut done = fields.clone();
        done["text"] = json!(format!("思考{index}🦀"));
        input += &wire("response.reasoning_summary_text.done", done.clone());
        input += &wire("response.reasoning_summary_text.done", done);
        let mut part_done = fields;
        part_done["part"] = json!({"type":"summary_text","text":format!("思考{index}🦀")});
        input += &wire("response.reasoning_summary_part.done", part_done);
    }
    input += &wire(
        "response.output_text.delta",
        json!({"item_id":"msg_1","content_index":0,"delta":"答案"}),
    );
    input += &complete();
    let output = convert(input).await;
    balanced(&output);
    assert_eq!(texts(&output, "thinking"), "思考0🦀思考1🦀");
    assert_eq!(texts(&output, "text"), "答案");
    assert_eq!(
        output
            .iter()
            .filter(|event| event["content_block"]["type"] == "thinking")
            .count(),
        2
    );
}

#[tokio::test]
async fn canonical_legacy_and_data_only_reasoning_start_without_created() {
    for (delta_event, done_event, field) in [
        (
            "response.reasoning_summary_text.delta",
            "response.reasoning_summary_text.done",
            "summary_index",
        ),
        (
            "response.reasoning_text.delta",
            "response.reasoning_text.done",
            "content_index",
        ),
        (
            "response.reasoning.delta",
            "response.reasoning.done",
            "content_index",
        ),
    ] {
        for header in [true, false] {
            let fields = json!({"output_index":2,field:1});
            let mut delta = fields.clone();
            delta["delta"] = json!("plan");
            let first = if header {
                wire(delta_event, delta)
            } else {
                delta["type"] = json!(delta_event);
                format!("data: {delta}\n\n")
            };
            let mut done = fields;
            done["text"] = json!("plan");
            let output = convert(first + &wire(done_event, done) + &complete()).await;
            balanced(&output);
            assert_eq!(texts(&output, "thinking"), "plan");
            assert_eq!(
                output
                    .iter()
                    .filter(|event| event["type"] == "message_start")
                    .count(),
                1
            );
        }
    }
}

#[tokio::test]
async fn reasoning_done_without_deltas_recovers_text_once() {
    for (name, text_field) in [
        (
            "response.reasoning_summary_text.done",
            json!({"text":"only done"}),
        ),
        (
            "response.reasoning_summary_part.done",
            json!({"part":{"type":"summary_text","text":"only done"}}),
        ),
        ("response.reasoning_text.done", json!({"text":"only done"})),
        ("response.reasoning.done", json!({"text":"only done"})),
    ] {
        let done = wire(name, text_field);
        let output = convert(format!("{done}{done}{}", complete())).await;
        balanced(&output);
        assert_eq!(texts(&output, "thinking"), "only done");
    }
}

#[tokio::test]
async fn text_and_tool_transitions_close_reasoning_without_reopening_from_late_done() {
    for tool in [false, true] {
        let initial_text = wire("response.output_text.delta", json!({"delta":"before"}));
        let think = wire(
            "response.reasoning_summary_text.delta",
            json!({"item_id":"rs_1","summary_index":0,"delta":"plan"}),
        );
        let next = if tool {
            wire(
                "response.output_item.added",
                json!({"item":{"type":"function_call","id":"fc_1","call_id":"call_1","name":"run"}}),
            ) + &wire(
                "response.function_call_arguments.delta",
                json!({"item_id":"fc_1","delta":"{}"}),
            )
        } else {
            wire("response.output_text.delta", json!({"delta":"after"}))
        };
        let late_done = wire(
            "response.reasoning_summary_text.done",
            json!({"item_id":"rs_1","summary_index":0,"text":"plan"}),
        );
        let output =
            convert(initial_text + &think + &next + &late_done + &late_done + &complete()).await;
        balanced(&output);
        assert_eq!(texts(&output, "thinking"), "plan");
        assert_eq!(
            texts(&output, "text"),
            if tool { "before" } else { "beforeafter" }
        );
        assert_eq!(
            output
                .iter()
                .find(|event| event["type"] == "message_delta")
                .unwrap()["delta"]["stop_reason"],
            if tool { "tool_use" } else { "end_turn" }
        );
        let start = output
            .iter()
            .position(|event| {
                event["content_block"]["type"] == if tool { "tool_use" } else { "text" }
                    && event["index"] == 2
            })
            .unwrap();
        assert_eq!(output[start - 1]["type"], "content_block_stop");
        assert_eq!(output[start - 1]["index"], 1);
    }
}

#[tokio::test]
async fn equal_summary_and_content_indices_do_not_merge_different_reasoning_parts() {
    let input = wire(
        "response.reasoning_summary_text.delta",
        json!({"item_id":"rs_1","summary_index":0,"delta":"summary"}),
    ) + &wire(
        "response.reasoning_text.delta",
        json!({"item_id":"rs_1","content_index":0,"delta":"content"}),
    ) + &wire(
        "response.reasoning_summary_text.done",
        json!({"item_id":"rs_1","summary_index":0,"text":"summary"}),
    ) + &wire(
        "response.reasoning_text.delta",
        json!({"item_id":"rs_1","content_index":0,"delta":" next"}),
    ) + &complete();
    let output = convert(input).await;
    balanced(&output);
    assert_eq!(texts(&output, "thinking"), "summarycontent next");
    assert_eq!(
        output
            .iter()
            .filter(|event| event["content_block"]["type"] == "thinking")
            .count(),
        2
    );
}

#[tokio::test]
async fn final_reasoning_item_recovers_missing_parts_without_repeating_streamed_parts() {
    for with_delta in [false, true] {
        let mut input = if with_delta {
            wire(
                "response.reasoning_summary_text.delta",
                json!({"item_id":"rs_1","summary_index":0,"delta":"first"}),
            )
        } else {
            String::new()
        };
        let item = wire(
            "response.output_item.done",
            json!({"output_index":0,"item":{"type":"reasoning","id":"rs_1","summary":[
                {"type":"summary_text","text":"first"},{"type":"summary_text","text":"second"}
            ]}}),
        );
        input += &item;
        input += &item;
        input += &complete();
        let output = convert(input).await;
        balanced(&output);
        assert_eq!(texts(&output, "thinking"), "firstsecond");
        assert_eq!(
            output
                .iter()
                .filter(|event| event["content_block"]["type"] == "thinking")
                .count(),
            2
        );
    }
    let item = wire(
        "response.output_item.done",
        json!({"output_index":0,"item":{"type":"reasoning","id":"rs_1","summary":[],"content":[{"type":"reasoning_text","text":"content"}]}}),
    );
    let output = convert(item + &complete()).await;
    balanced(&output);
    assert_eq!(texts(&output, "thinking"), "content");
}

#[test]
fn invalid_parts_and_empty_deltas_do_not_allocate_or_repeat_initial_text() {
    let mut state = ReasoningBlocks::default();
    let mut next = 0;
    for data in [
        json!({"part":{"type":"unknown","text":"skip"}}),
        json!({"part":null}),
    ] {
        assert!(state
            .handle("response.reasoning_summary_part.added", &data, &mut next)
            .unwrap()
            .is_empty());
    }
    for data in [
        json!({"delta":""}),
        json!({"delta":3}),
        json!({"text":null}),
    ] {
        assert!(state
            .handle("response.reasoning_text.delta", &data, &mut next)
            .unwrap()
            .is_empty());
    }
    assert_eq!(next, 0);
    let data =
        json!({"item_id":"rs_1","summary_index":0,"part":{"type":"summary_text","text":"initial"}});
    let output = state
        .handle("response.reasoning_summary_part.added", &data, &mut next)
        .unwrap();
    assert_eq!(texts(&output, "thinking"), "initial");
    assert!(state
        .handle("response.reasoning_summary_part.added", &data, &mut next)
        .unwrap()
        .is_empty());
    let done = state
        .handle("response.reasoning_summary_part.done", &data, &mut next)
        .unwrap();
    assert_eq!(done.len(), 1);
    assert_eq!(done[0]["type"], "content_block_stop");
    assert_eq!(next, 1);
}

#[test]
fn part_and_identity_limits_reject_without_retaining_text_or_evicting_live_parts() {
    let mut state = ReasoningBlocks::default();
    let mut next = 0;
    for index in 0..MAX_PARTS {
        state
            .handle(
                "response.reasoning_summary_text.done",
                &json!({"item_id":"rs","summary_index":index,"text":"done"}),
                &mut next,
            )
            .unwrap();
    }
    assert!(state
        .handle(
            "response.reasoning_summary_text.delta",
            &json!({"item_id":"new","delta":"secret"}),
            &mut next
        )
        .is_err());
    assert!(state
        .handle(
            "response.reasoning_summary_text.delta",
            &json!({"item_id":"x".repeat(MAX_ITEM_ID_BYTES + 1),"delta":"secret"}),
            &mut next
        )
        .is_err());
    assert_eq!(state.blocks.len(), MAX_PARTS);
    assert_eq!(next, MAX_PARTS as u32);
    assert!(state
        .handle(
            "response.reasoning_summary_text.done",
            &json!({"item_id":"rs","summary_index":0,"text":"done"}),
            &mut next
        )
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn adapter_part_limit_emits_one_sanitized_error_without_normal_completion() {
    let mut input = String::new();
    for index in 0..=MAX_PARTS {
        input += &wire(
            "response.reasoning_summary_text.done",
            json!({"item_id":"rs","summary_index":index,"text":"done"}),
        );
    }
    input += &complete();
    let source = futures_util::stream::iter([Ok::<_, std::io::Error>(Bytes::from(input))]);
    let chunks = create_anthropic_sse_stream_from_responses(source)
        .collect::<Vec<_>>()
        .await;
    let output = chunks
        .into_iter()
        .map(|chunk| String::from_utf8(chunk.unwrap().to_vec()).unwrap())
        .collect::<String>();
    assert_eq!(output.matches("event: error\n").count(), 1);
    assert!(output.contains("part limit"));
    assert!(!output.contains("message_stop"));
}

#[test]
fn whole_reasoning_prefers_summary_and_falls_back_to_reasoning_text_without_exposing_seals() {
    for (summary, content, expected) in [
        (
            json!([{"type":"summary_text","text":"summary"}]),
            json!([{"type":"reasoning_text","text":"private content"}]),
            "summary",
        ),
        (
            json!([]),
            json!([{"type":"reasoning_text","text":"a"},{"type":"reasoning_text","text":"b"},{"type":"unknown","text":"skip"}]),
            "ab",
        ),
        (
            Value::Null,
            json!([{"type":"reasoning_text","text":"content"}]),
            "content",
        ),
    ] {
        let output = responses_to_anthropic(json!({"output":[{"type":"reasoning","summary":summary,"content":content,"encrypted_content":"do-not-expose"}]})).unwrap();
        assert_eq!(output["content"][0]["thinking"], expected);
        assert!(!output.to_string().contains("do-not-expose"));
    }
}
