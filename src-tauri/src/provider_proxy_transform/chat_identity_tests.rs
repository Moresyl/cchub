use super::create_anthropic_sse_stream;
use bytes::Bytes;
use futures_util::StreamExt;
use serde_json::{json, Value};

fn frame(tool: Value) -> String {
    format!(
        "data: {}\n\n",
        json!({"id":"fixture","model":"fixture-model","choices":[{"delta":{"tool_calls":[tool]}}]})
    )
}
async fn convert(calls: Vec<Value>) -> (String, Vec<Value>) {
    let mut wire = calls.into_iter().map(frame).collect::<String>();
    wire += "data: {\"id\":\"fixture\",\"model\":\"fixture-model\",\"choices\":[{\"delta\":{},\"finish_reason\":\"tool_calls\"}]}\n\ndata: [DONE]\n\n";
    // Split into small byte chunks so identity and UTF-8 processing do not rely
    // on one complete upstream chunk.
    let chunks = wire
        .as_bytes()
        .chunks(17)
        .map(|chunk| Ok::<_, std::io::Error>(Bytes::copy_from_slice(chunk)))
        .collect::<Vec<_>>();
    let output = create_anthropic_sse_stream(futures_util::stream::iter(chunks))
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .map(|chunk| String::from_utf8(chunk.unwrap().to_vec()).unwrap())
        .collect::<String>();
    let events = output
        .lines()
        .filter_map(|line| line.strip_prefix("data: "))
        .map(|data| serde_json::from_str(data).unwrap())
        .collect();
    (output, events)
}

#[tokio::test]
async fn repeated_chat_identity_and_empty_identity_fragments_continue_one_call() {
    let (output, events) = convert(vec![
        json!({"index":0,"function":{"arguments":"{\"path\":"}}),
        json!({"index":0,"id":"call_one","function":{"name":"read_file","arguments":"\""}}),
        json!({"index":0,"id":"call_one","function":{"name":"read_file","arguments":"文件"}}),
        json!({"index":0,"id":"","function":{"name":"","arguments":"\"}"}}),
    ])
    .await;
    let starts = events
        .iter()
        .filter(|event| event["type"] == "content_block_start")
        .collect::<Vec<_>>();
    assert_eq!(starts.len(), 1);
    assert_eq!(starts[0]["content_block"]["id"], "call_one");
    assert_eq!(starts[0]["content_block"]["name"], "read_file");
    let arguments = events
        .iter()
        .filter_map(|event| event.pointer("/delta/partial_json").and_then(Value::as_str))
        .collect::<String>();
    assert_eq!(
        serde_json::from_str::<Value>(&arguments).unwrap(),
        json!({"path":"文件"})
    );
    assert_eq!(output.matches("event: content_block_stop\n").count(), 1);
    assert_eq!(output.matches("event: message_stop\n").count(), 1);
    assert!(!output.contains("event: error\n"));
}

#[tokio::test]
async fn conflicting_chat_identity_stops_without_mixing_arguments_or_echoing_identity() {
    for conflict in [
        json!({"index":0,"id":"private-conflict","function":{"name":"read_file","arguments":"private-arguments"}}),
        json!({"index":0,"id":"call_one","function":{"name":"private-conflict","arguments":"private-arguments"}}),
        json!({"index":1,"id":"call_one","function":{"name":"read_file","arguments":"private-arguments"}}),
    ] {
        let (output, _) = convert(vec![
            json!({"index":0,"id":"call_one","function":{"name":"read_file","arguments":"{"}}),
            conflict,
        ])
        .await;
        assert_eq!(output.matches("event: content_block_start\n").count(), 1);
        assert_eq!(output.matches("event: error\n").count(), 1);
        assert!(!output.contains("event: message_stop\n"));
        assert!(!output.contains("private-conflict"));
        assert!(!output.contains("private-arguments"));
    }
}

#[tokio::test]
async fn anonymous_chat_fallback_avoids_an_explicit_call_id_collision() {
    let (output, events) = convert(vec![
        json!({"index":0,"id":"tool_call_1","function":{"name":"run","arguments":"{}"}}),
        json!({"index":1,"function":{"name":"other","arguments":"{}"}}),
    ])
    .await;
    let ids = events
        .iter()
        .filter_map(|event| event.pointer("/content_block/id").and_then(Value::as_str))
        .collect::<Vec<_>>();
    assert_eq!(ids, ["tool_call_1", "tool_call_1_1"]);
    assert!(!output.contains("event: error\n"));
    assert_eq!(output.matches("event: message_stop\n").count(), 1);
}
