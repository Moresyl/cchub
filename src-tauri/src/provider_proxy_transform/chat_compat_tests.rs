use super::{create_anthropic_sse_stream, normalize_sse_stream};
use bytes::Bytes;
use futures_util::StreamExt;
use serde_json::{json, Value};

async fn convert(wire: String) -> Vec<Value> {
    let chunks = wire
        .as_bytes()
        .iter()
        .map(|byte| Ok::<_, std::io::Error>(Bytes::copy_from_slice(&[*byte])))
        .collect::<Vec<_>>();
    let output =
        create_anthropic_sse_stream(normalize_sse_stream(futures_util::stream::iter(chunks)))
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .map(|chunk| String::from_utf8(chunk.unwrap().to_vec()).unwrap())
            .collect::<String>();
    output
        .lines()
        .filter_map(|line| line.strip_prefix("data: "))
        .map(|data| serde_json::from_str(data).unwrap())
        .collect()
}

fn chunk(delta: Value, finish: Value) -> String {
    format!(
        "data: {}\r\n\r\n",
        json!({"id":"fixture","model":"fixture-model","choices":[{"index":0,"delta":delta,"finish_reason":finish}]})
    )
}

#[tokio::test]
async fn empty_tool_lists_reasoning_and_finish_reasons_do_not_split_text() {
    for reasoning_key in ["reasoning", "reasoning_content"] {
        let mut wire = String::new();
        for text in ["我来", "看", "一下🦀"] {
            let mut delta = json!({"content":text,"tool_calls":[]});
            delta[reasoning_key] = json!("");
            wire += &chunk(delta, json!(""));
        }
        wire += &chunk(
            json!({"tool_calls":[],"reasoning_content":""}),
            json!("stop"),
        );
        wire += "data: [DONE]\r\n\r\n";
        let events = convert(wire).await;
        let starts: Vec<_> = events
            .iter()
            .filter(|event| event["type"] == "content_block_start")
            .collect();
        assert_eq!(starts.len(), 1, "{reasoning_key}: {events:?}");
        assert_eq!(starts[0]["content_block"]["type"], "text");
        let text = events
            .iter()
            .filter_map(|event| event.pointer("/delta/text").and_then(Value::as_str))
            .collect::<String>();
        assert_eq!(text, "我来看一下🦀");
        assert_eq!(
            events
                .iter()
                .filter(|event| event["type"] == "content_block_stop")
                .count(),
            1
        );
        assert_eq!(
            events
                .iter()
                .filter(|event| event["type"] == "message_delta")
                .count(),
            1
        );
        assert_eq!(events.last().unwrap()["type"], "message_stop");
    }
}

#[tokio::test]
async fn real_reasoning_then_text_has_one_block_for_each_phase() {
    for key in ["reasoning", "reasoning_content"] {
        let mut thought = json!({"tool_calls":[]});
        thought[key] = json!("先思考");
        let mut wire = chunk(thought, json!(""));
        // Providers can include both names; an empty primary must not mask real content.
        wire += &chunk(
            json!({"reasoning":"","reasoning_content":"再确认","tool_calls":[]}),
            json!(""),
        );
        wire += &chunk(
            json!({"reasoning_content":"","content":"答案","tool_calls":[]}),
            json!(null),
        );
        wire += &chunk(json!({"content":"完成","tool_calls":[]}), json!("stop"));
        wire += "data: [DONE]\n\n";
        let events = convert(wire).await;
        let starts: Vec<_> = events
            .iter()
            .filter(|event| event["type"] == "content_block_start")
            .collect();
        assert_eq!(starts.len(), 2);
        assert_eq!(starts[0]["content_block"]["type"], "thinking");
        assert_eq!(starts[1]["content_block"]["type"], "text");
        assert_eq!(
            events
                .iter()
                .filter_map(|event| event.pointer("/delta/thinking").and_then(Value::as_str))
                .collect::<String>(),
            "先思考再确认"
        );
        assert_eq!(
            events
                .iter()
                .filter_map(|event| event.pointer("/delta/text").and_then(Value::as_str))
                .collect::<String>(),
            "答案完成"
        );
    }
}

#[tokio::test]
async fn empty_finish_fragments_do_not_start_fallback_tools_or_end_the_message() {
    let mut wire = chunk(
        json!({"tool_calls":[{"index":0,"function":{"arguments":"{\"path\":"}}]}),
        json!(""),
    );
    wire += &chunk(
        json!({"tool_calls":[{"index":0,"id":"call_one","function":{"name":"read_file","arguments":"\"文件\"}"}}]}),
        json!(""),
    );
    wire += &chunk(
        json!({"tool_calls":[{"index":0,"function":{"name":"","arguments":""}}]}),
        json!("tool_calls"),
    );
    wire += "data: [DONE]\n\n";
    let events = convert(wire).await;
    let starts: Vec<_> = events
        .iter()
        .filter(|event| event["type"] == "content_block_start")
        .collect();
    assert_eq!(starts.len(), 1);
    assert_eq!(starts[0]["content_block"]["name"], "read_file");
    assert_eq!(starts[0]["content_block"]["id"], "call_one");
    assert_eq!(
        events
            .iter()
            .filter_map(|event| event.pointer("/delta/partial_json").and_then(Value::as_str))
            .collect::<String>(),
        r#"{"path":"文件"}"#
    );
    assert_eq!(
        events
            .iter()
            .filter(|event| event["type"] == "message_delta")
            .count(),
        1
    );
    assert_eq!(events.last().unwrap()["type"], "message_stop");
}

#[tokio::test]
async fn empty_finish_without_terminal_is_an_error_instead_of_success() {
    let wire = chunk(json!({"content":"partial","tool_calls":[]}), json!(""));
    let events = convert(wire).await;
    assert!(events
        .iter()
        .all(|event| event["type"] != "message_delta" && event["type"] != "message_stop"));
    assert_eq!(events.last().unwrap()["type"], "error");
}
