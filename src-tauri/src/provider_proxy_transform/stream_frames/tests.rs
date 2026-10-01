use super::*;
use crate::provider_proxy_transform::{
    create_anthropic_sse_stream, create_anthropic_sse_stream_from_gemini,
    create_anthropic_sse_stream_from_responses,
};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

fn source(bytes: Bytes) -> impl Stream<Item = Result<Bytes, std::io::Error>> + Send {
    futures_util::stream::iter([Ok(bytes)])
}

async fn converted(format: &str, chunks: Vec<Bytes>) -> String {
    let stream = futures_util::stream::iter(chunks.into_iter().map(Ok::<_, std::io::Error>));
    let result = match format {
        "chat" => {
            create_anthropic_sse_stream(stream)
                .collect::<Vec<_>>()
                .await
        }
        "responses" => {
            create_anthropic_sse_stream_from_responses(stream)
                .collect::<Vec<_>>()
                .await
        }
        "gemini" => {
            create_anthropic_sse_stream_from_gemini(stream, "fixture".into())
                .collect::<Vec<_>>()
                .await
        }
        _ => unreachable!(),
    };
    result
        .into_iter()
        .map(|chunk| String::from_utf8(chunk.unwrap().to_vec()).unwrap())
        .collect()
}

#[tokio::test]
async fn event_decoder_preserves_every_byte_split_and_multiline_crlf() {
    let input =
        "event: fixture\r\ndata: 你好🦀\r\ndata: second\r\n\r\n: heartbeat\r\rdata: last\n\n";
    let stream = futures_util::stream::iter(
        input
            .as_bytes()
            .iter()
            .map(|byte| Ok::<_, std::io::Error>(Bytes::copy_from_slice(&[*byte])))
            .collect::<Vec<_>>(),
    );
    let result = frames(stream).collect::<Vec<_>>().await;
    assert_eq!(result.len(), 3);
    let result = result
        .into_iter()
        .flat_map(|chunk| chunk.unwrap().to_vec())
        .collect::<Vec<_>>();
    assert_eq!(
        String::from_utf8(result).unwrap(),
        "event: fixture\ndata: 你好🦀\ndata: second\n\n: heartbeat\n\ndata: last\n\n"
    );
}

#[tokio::test]
async fn event_limit_applies_per_event_and_accepts_exact_boundary() {
    let event = format!("{}\n\n", "x".repeat(MAX_FRAME_BYTES - 2));
    let result = frames(source(Bytes::from(event.repeat(2))))
        .collect::<Vec<_>>()
        .await;
    assert_eq!(result.len(), 2);
    assert!(result
        .iter()
        .all(|event| event.as_ref().unwrap().len() == MAX_FRAME_BYTES));
}

#[tokio::test]
async fn event_overflow_stops_before_reading_the_next_chunk_without_retaining_private_data() {
    for fragmented in [false, true] {
        let bytes = Bytes::from(format!(
            "data: private-credential{}",
            "x".repeat(MAX_FRAME_BYTES)
        ));
        let chunks = if fragmented {
            bytes
                .chunks(1024)
                .map(Bytes::copy_from_slice)
                .collect::<Vec<_>>()
        } else {
            vec![bytes]
        };
        let before = chunks.len();
        let polls = Arc::new(AtomicUsize::new(0));
        let counter = polls.clone();
        let stream = futures_util::stream::iter(
            chunks
                .into_iter()
                .chain([Bytes::from_static(b"data: after\n\n")])
                .map(Ok::<_, std::io::Error>),
        )
        .inspect(move |_| {
            counter.fetch_add(1, Ordering::SeqCst);
        });
        let output = frames(stream).collect::<Vec<_>>().await;
        assert_eq!(output.len(), 1);
        assert!(matches!(
            output[0]
                .as_ref()
                .unwrap_err()
                .get_ref()
                .unwrap()
                .downcast_ref::<FrameError>(),
            Some(FrameError::Limit)
        ));
        assert!(!output[0]
            .as_ref()
            .unwrap_err()
            .to_string()
            .contains("private-credential"));
        assert!(polls.load(Ordering::SeqCst) <= before);
    }
}

#[tokio::test]
async fn invalid_utf8_and_incomplete_events_fail_instead_of_replacing_or_discarding_text() {
    for bytes in [
        Bytes::from_static(b"data: \xff\n\n"),
        Bytes::from_static(b"data: \xe4\xbd\n\n"),
        Bytes::from_static(b"data: partial"),
    ] {
        let result = frames(source(bytes)).collect::<Vec<_>>().await;
        assert_eq!(result.len(), 1);
        assert!(result[0].is_err());
    }
}

#[tokio::test]
async fn all_adapters_preserve_split_unicode_and_crlf_through_real_conversion() {
    for (format, event) in [
        ("chat", "data: {\"id\":\"x\",\"model\":\"m\",\"choices\":[{\"delta\":{\"content\":\"你好🦀\"},\"finish_reason\":\"stop\"}]}\r\n\r\ndata: [DONE]\r\n\r\n"),
        ("responses", "data: {\"type\":\"response.output_text.delta\",\"delta\":\"你好🦀\"}\r\n\r\ndata: {\"type\":\"response.completed\",\"response\":{\"status\":\"completed\"}}\r\n\r\n"),
        ("gemini", "data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"你好🦀\"}]},\"finishReason\":\"STOP\"}]}\r\n\r\n"),
    ] {
        let output = converted(format, event.as_bytes().iter().map(|byte| Bytes::copy_from_slice(&[*byte])).collect()).await;
        assert!(output.contains("你好🦀"), "{format}: {output}");
        assert!(!output.contains('�'));
        assert_eq!(output.matches("event: message_stop\n").count(), 1);
        assert!(!output.contains("event: error\n"));
    }
}

#[tokio::test]
async fn all_adapters_report_one_bounded_decoder_error_without_normal_completion() {
    for format in ["chat", "responses", "gemini"] {
        for bytes in [
            Bytes::from(format!(
                "data: private-secret{}",
                "x".repeat(MAX_FRAME_BYTES)
            )),
            Bytes::from_static(b"data: \xff\n\n"),
            Bytes::from_static(b"data: incomplete"),
        ] {
            let output = converted(format, vec![bytes]).await;
            assert_eq!(output.matches("event: error\n").count(), 1, "{format}");
            assert!(output.contains("api_error"));
            assert!(!output.contains("message_stop"));
            assert!(!output.contains("private-secret"));
        }
    }
}

#[tokio::test]
async fn missing_semantic_completion_never_becomes_a_normal_reply() {
    for (format, event) in [
        ("chat", "data: {\"id\":\"x\",\"model\":\"m\",\"choices\":[{\"delta\":{\"content\":\"partial\"}}]}\n\n"),
        ("responses", "data: {\"type\":\"response.output_text.delta\",\"delta\":\"partial\"}\n\n"),
        ("gemini", "data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"partial\"}]}}]}\n\n"),
    ] {
        let output = converted(format, vec![Bytes::from_static(event.as_bytes())]).await;
        assert!(output.contains("partial"));
        assert_eq!(output.matches("event: error\n").count(), 1);
        assert!(!output.contains("message_stop"));
    }
}

#[tokio::test]
async fn chat_finish_reason_without_optional_done_sentinel_is_valid_completion() {
    let event = b"data: {\"id\":\"x\",\"model\":\"m\",\"choices\":[{\"delta\":{\"content\":\"answer\"},\"finish_reason\":\"stop\"}]}\n\n";
    let output = converted("chat", vec![Bytes::from_static(event)]).await;
    assert_eq!(output.matches("event: message_stop\n").count(), 1);
    assert!(!output.contains("event: error\n"));
}

#[tokio::test]
async fn all_adapters_join_multiline_event_data_and_ignore_bom_and_heartbeats() {
    for (format, event, terminal) in [
        ("chat", "{\"id\":\"x\",\"model\":\"m\",\"choices\":[\n{\"delta\":{\"content\":\"你好🦀\"},\"finish_reason\":\"stop\"}]}", "data: [DONE]\n\n"),
        ("responses", "{\"type\":\"response.output_text.delta\",\n\"delta\":\"你好🦀\"}", "data: {\"type\":\"response.completed\",\"response\":{\"status\":\"completed\"}}\n\n"),
        ("gemini", "{\"candidates\":[{\"content\":{\"parts\":[\n{\"text\":\"你好🦀\"}]},\"finishReason\":\"STOP\"}]}", ""),
    ] {
        let input = format!("\u{feff}: heartbeat\r\n\r\ndata: {}\r\n\r\n{terminal}", event.replace('\n', "\r\ndata: "));
        let result = converted(format, input.as_bytes().iter().map(|byte| Bytes::copy_from_slice(&[*byte])).collect()).await;
        assert!(result.contains("你好🦀"), "{format}: {result}");
        assert!(!result.contains("event: error\n"));
        assert_eq!(result.matches("event: message_stop\n").count(), 1);
    }
}
