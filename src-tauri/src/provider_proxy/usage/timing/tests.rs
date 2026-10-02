use super::*;
use serde_json::json;

fn frame(event: &str, value: serde_json::Value) -> String {
    format!("event: {event}\ndata: {value}\n\n")
}

#[test]
fn late_usage_lifecycle_roles_and_keepalives_do_not_mark_or_extend_output() {
    for (initial, output, final_usage) in [
        (
            json!({"type":"message_start","message":{"usage":{"input_tokens":2}}}),
            json!({"type":"content_block_delta","delta":{"type":"text_delta","text":"hi"}}),
            json!({"type":"message_delta","usage":{"output_tokens":12}}),
        ),
        (
            json!({"type":"response.created","response":{}}),
            json!({"type":"response.output_text.delta","delta":"hi"}),
            json!({"type":"response.completed","response":{"usage":{"output_tokens":12}}}),
        ),
        (
            json!({"choices":[{"delta":{"role":"assistant","content":""}}]}),
            json!({"choices":[{"delta":{"content":"hi"}}]}),
            json!({"choices":[],"usage":{"completion_tokens":12}}),
        ),
        (
            json!({"candidates":[{"content":{"parts":[{"text":""}]}}]}),
            json!({"candidates":[{"content":{"parts":[{"text":"hi"}]}}]}),
            json!({"usageMetadata":{"candidatesTokenCount":12}}),
        ),
    ] {
        let mut inspector = Inspector::default();
        inspector.observe(frame("", initial).as_bytes(), 10);
        inspector.observe(b": alive\n\nevent: ping\ndata: {}\n\n", 50);
        assert_eq!(inspector.snapshot(), StreamTiming::default());
        inspector.observe(frame("", output.clone()).as_bytes(), 80);
        inspector.observe(frame("", output).as_bytes(), 380);
        inspector.observe(frame("", final_usage).as_bytes(), 900);
        inspector.observe(b"data: [DONE]\n\n", 1000);
        assert_eq!(
            inspector.snapshot(),
            StreamTiming {
                first_output_ms: Some(80),
                generation_ms: Some(300)
            }
        );
    }
}

#[test]
fn reasoning_tools_and_multimodal_payloads_are_output_but_empty_identity_is_not() {
    for value in [
        json!({"type":"content_block_delta","delta":{"type":"thinking_delta","thinking":"reason"}}),
        json!({"type":"content_block_delta","delta":{"type":"input_json_delta","partial_json":"{"}}),
        json!({"type":"content_block_start","content_block":{"type":"tool_use","name":"read"}}),
        json!({"type":"response.reasoning_summary_text.delta","delta":"reason"}),
        json!({"type":"response.refusal.delta","delta":"cannot"}),
        json!({"type":"response.function_call_arguments.delta","delta":"{"}),
        json!({"type":"response.output_item.added","item":{"type":"function_call","name":"read"}}),
        json!({"choices":[{"delta":{"tool_calls":[{"function":{"arguments":"{"}}]}}]}),
        json!({"choices":[{"delta":{"reasoning_content":"reason"}}]}),
        json!({"choices":[{"delta":{"audio":{"data":"YWJj"}}}]}),
        json!({"candidates":[{"content":{"parts":[{"functionCall":{"name":"read"}}]}}]}),
        json!({"candidates":[{"content":{"parts":[{"inlineData":{"data":"YWJj"}}]}}]}),
    ] {
        assert!(output::has_output("", &value), "{value}");
    }
    for value in [
        json!({"type":"content_block_start","content_block":{"type":"thinking","thinking":""}}),
        json!({"type":"content_block_delta","delta":{"type":"signature_delta","signature":"opaque"}}),
        json!({"choices":[{"delta":{"tool_calls":[{"id":"call","function":{"name":"","arguments":""}}]}}]}),
        json!({"type":"response.output_text.done","text":"duplicate"}),
        json!({"type":"error","error":{"message":"failed"}}),
        json!({"unknown":"event"}),
    ] {
        assert!(!output::has_output("", &value), "{value}");
    }
}

#[test]
fn every_byte_unicode_bom_crlf_multiline_and_last_event_field_are_preserved() {
    let wire = "\u{feff}event: ping\r\nevent: response.output_text.delta\r\ndata: {\"delta\":\r\ndata: \"你好🦀\"}\r\n\r\n";
    let mut inspector = Inspector::default();
    for byte in wire.as_bytes() {
        inspector.observe(&[*byte], 120);
    }
    assert_eq!(inspector.snapshot().first_output_ms, Some(120));
    inspector.observe(
        b"event: response.output_text.delta\rdata: {\"delta\":\"bye\"}\r\r",
        320,
    );
    assert_eq!(inspector.snapshot().generation_ms, Some(200));
}

#[test]
fn incomplete_and_oversized_events_cannot_fabricate_late_timing() {
    let mut inspector = Inspector::default();
    inspector.observe(
        b"data: {\"choices\":[{\"delta\":{\"content\":\"hi\"}}]}",
        100,
    );
    assert_eq!(inspector.snapshot(), StreamTiming::default());
    let mut inspector = Inspector::default();
    inspector.observe(
        frame("response.output_text.delta", json!({"delta":"hi"})).as_bytes(),
        10,
    );
    inspector.observe(&vec![b'x'; MAX_FRAME_BYTES + 1], 20);
    inspector.observe(
        frame("response.output_text.delta", json!({"delta":"later"})).as_bytes(),
        800,
    );
    assert_eq!(inspector.snapshot(), StreamTiming::default());
    assert!(inspector.frame.capacity() <= MAX_FRAME_BYTES);
}

#[tokio::test]
async fn capture_is_request_local_and_preserves_wire_and_transport_errors() {
    let bytes = Bytes::from(frame(
        "response.output_text.delta",
        json!({"delta":"你好🦀"}),
    ));
    let source = futures_util::stream::iter([
        Ok(bytes.clone()),
        Err(std::io::Error::new(
            std::io::ErrorKind::ConnectionReset,
            "fixture",
        )),
    ]);
    let capture = StreamTimingCapture::new(Instant::now());
    let other = StreamTimingCapture::new(Instant::now());
    let chunks = observe_stream_timing(source, capture.clone())
        .collect::<Vec<_>>()
        .await;
    assert_eq!(chunks[0].as_ref().unwrap(), &bytes);
    assert_eq!(
        chunks[1].as_ref().unwrap_err().kind(),
        std::io::ErrorKind::ConnectionReset
    );
    assert!(capture.snapshot().first_output_ms.is_some());
    assert_eq!(capture.snapshot().generation_ms, Some(0));
    assert_eq!(other.snapshot(), StreamTiming::default());
}
