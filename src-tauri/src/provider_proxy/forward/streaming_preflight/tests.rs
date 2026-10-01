use super::*;
use futures_util::stream;

#[test]
fn initialization_is_distinguished_from_outputs_side_effects_and_unknown_events() {
    for frame in [
        ": alive\n\n",
        "event: ping\ndata: {\"type\":\"ping\"}\n\n",
        "data: {\"choices\":[{\"delta\":{\"role\":\"assistant\",\"content\":\"\"}}]}\n\n",
        "event: response.created\ndata: {\"type\":\"response.created\",\"response\":{\"output\":[]}}\n\n",
        "event: message_start\ndata: {\"message\":{\"content\":[],\"usage\":{\"input_tokens\":7}}}\n\n",
        "data: {\"usage\":{\"prompt_tokens\":7}}\n\n",
    ] { assert_eq!(classify(frame), Decision::Lead, "{frame}"); }
    for frame in [
        "data: [DONE]\n\n",
        "data: {\"choices\":[{\"delta\":{\"reasoning_content\":\"think\"}}]}\n\n",
        "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0}]}}]}\n\n",
        "event: content_block_start\ndata: {\"content_block\":{\"type\":\"tool_use\"}}\n\n",
        "event: response.output_item.added\ndata: {\"item\":{\"type\":\"function_call\"}}\n\n",
        "event: response.reasoning_text.delta\ndata: {\"delta\":\"think\"}\n\n",
        "data: {\"candidates\":[{\"content\":{\"parts\":[{\"functionCall\":{}}]}}]}\n\n",
        "event: custom\ndata: {}\n\n",
        "data: {\"choices\":[{\"delta\":{\"unknown_action\":{}}}]}\n\n",
        "data: {\"choices\":[{\"delta\":{\"refusal\":\"declined\"}}]}\n\n",
        "data: malformed\n\n",
        "data: {\"choices\":[{\"delta\":{\"content\":\"visible\"}}],\"error\":{\"code\":429}}\n\n",
    ] {
        assert_eq!(classify(frame), Decision::Commit, "{frame}");
    }
}

#[test]
fn error_status_uses_structured_fields_without_guessing_from_content() {
    for (frame, status) in [
        ("event: error\ndata: {\"error\":{\"type\":\"rate_limit_error\"}}\n\n", 429),
        ("event: response.failed\ndata: {\"response\":{\"error\":{\"code\":\"rate_limit_exceeded\"}}}\n\n", 429),
        ("data: {\"error\":{\"code\":400,\"message\":\"429 secret\"}}\n\n", 400),
        ("data: {\"error\":{\"status\":\"UNAVAILABLE\"}}\n\n", 503),
        ("event: error\n\n", 502),
    ] { assert_eq!(classify(frame), Decision::Failed(StatusCode::from_u16(status).unwrap())); }
}

#[tokio::test]
async fn split_crlf_unicode_errors_retain_usage_and_drop_the_held_source() {
    let frame = "\u{feff}event: message_start\r\ndata: {\"message\":{\"model\":\"模型\",\"usage\":{\"input_tokens\":7,\"cache_read_input_tokens\":2}}}\r\n\r\nevent: error\r\ndata: {\"error\":{\"type\":\"rate_limit_error\",\r\ndata: \"message\":\"秘密\"}}\r\n\r\n";
    let chunks = frame
        .as_bytes()
        .iter()
        .map(|byte| Ok(Bytes::copy_from_slice(&[*byte])))
        .collect::<Vec<Result<Bytes, std::io::Error>>>();
    let result = prepare(
        Box::pin(stream::iter(chunks).chain(stream::pending())),
        "v1/messages",
        None,
    )
    .await;
    let failure = result
        .err()
        .expect("held explicit error should fail immediately");
    assert_eq!(failure.status, StatusCode::TOO_MANY_REQUESTS);
    assert!(!failure.message.contains("秘密"));
    let usage = failure.usage.unwrap();
    assert_eq!(usage.total_input_tokens(), 9);
    assert_eq!(usage.response_model.as_deref(), Some("模型"));
}

#[tokio::test]
async fn output_then_error_in_one_chunk_commits_without_reordering_or_replay() {
    let wire = Bytes::from_static(b"data: {\"choices\":[{\"delta\":{\"content\":\"visible\"}}]}\n\ndata: {\"error\":{\"code\":429}}\n\n");
    let source = stream::iter([Ok(wire.clone())]);
    let output = prepare(Box::pin(source), "v1/chat/completions", None)
        .await
        .ok()
        .unwrap()
        .collect::<Vec<_>>()
        .await;
    assert_eq!(
        output
            .into_iter()
            .flat_map(|chunk| chunk.unwrap().to_vec())
            .collect::<Vec<_>>(),
        wire
    );
}

#[tokio::test]
async fn a_transport_failure_after_only_initialization_can_retry_with_retained_usage() {
    let source = stream::iter([
        Ok(Bytes::from_static(
            b"data: {\"usage\":{\"prompt_tokens\":7}}\n\n",
        )),
        Err(std::io::Error::new(
            std::io::ErrorKind::ConnectionReset,
            "private transport secret",
        )),
    ]);
    let failure = prepare(Box::pin(source), "v1/chat/completions", None)
        .await
        .err()
        .unwrap();
    assert!(failure.retryable());
    assert!(!failure.message.contains("private transport secret"));
    assert_eq!(failure.usage.unwrap().input_tokens, 7);
}

#[tokio::test]
async fn prefix_storage_and_grace_are_bounded_and_timeout_does_not_discard_a_pending_read() {
    let large = Bytes::from(vec![b'x'; MAX_PREFIX_BYTES + 100]);
    let output = prepare(
        Box::pin(stream::iter([Ok(large.clone())])),
        "v1/messages",
        None,
    )
    .await
    .ok()
    .unwrap()
    .collect::<Vec<_>>()
    .await;
    assert_eq!(
        output
            .into_iter()
            .flat_map(|chunk| chunk.unwrap().to_vec())
            .collect::<Vec<_>>(),
        large
    );
    let source = async_stream::stream! {
        yield Ok(Bytes::from_static(b": alive\n\n"));
        tokio::time::sleep(MAX_PREFIX_WAIT + Duration::from_millis(100)).await;
        yield Ok(Bytes::from_static(b"data: [DONE]\n\n"));
    };
    let started = tokio::time::Instant::now();
    let output = prepare(Box::pin(source), "v1/chat/completions", None)
        .await
        .ok()
        .unwrap();
    assert!(started.elapsed() < MAX_PREFIX_WAIT + Duration::from_millis(500));
    let chunks = output.collect::<Vec<_>>().await;
    assert_eq!(
        chunks
            .into_iter()
            .flat_map(|chunk| chunk.unwrap().to_vec())
            .collect::<Vec<_>>(),
        b": alive\n\ndata: [DONE]\n\n"
    );
}

#[tokio::test(start_paused = true)]
async fn grace_wins_when_source_error_and_release_deadline_are_ready_together() {
    let source = async_stream::stream! {
        yield Ok(Bytes::from_static(b": alive\n\n"));
        tokio::time::sleep(MAX_PREFIX_WAIT).await;
        yield Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "idle timeout"));
    };
    let output = prepare(Box::pin(source), "v1/chat/completions", None)
        .await
        .ok()
        .unwrap();
    let chunks = output.collect::<Vec<_>>().await;
    assert_eq!(chunks[0].as_ref().unwrap().as_ref(), b": alive\n\n");
    assert_eq!(
        chunks[1].as_ref().unwrap_err().kind(),
        std::io::ErrorKind::TimedOut
    );
}
