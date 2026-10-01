use super::*;
use futures_util::stream;

async fn collect(stream: impl Stream<Item = Result<Bytes, std::io::Error>>) -> Bytes {
    tokio::pin!(stream);
    let mut output = Vec::new();
    while let Some(chunk) = stream.next().await {
        output.extend_from_slice(&chunk.unwrap());
    }
    Bytes::from(output)
}

#[tokio::test]
async fn fragmented_unicode_crlf_and_multiple_events_are_repaired() {
    let input = "data: {\"choices\":[{\"delta\":{\"content\":\"你好🦀\",\"reasoning_content\":\"\",\"tool_calls\":[]},\"finish_reason\":\"\"}]}\r\n\r\ndata: [DONE]\r\n\r\n";
    let chunks = input
        .as_bytes()
        .iter()
        .map(|byte| Ok::<_, std::io::Error>(Bytes::copy_from_slice(&[*byte])))
        .collect::<Vec<_>>();
    let source = crate::provider_proxy_transform::normalize_sse_stream(stream::iter(chunks));
    let output = collect(normalize(source)).await;
    assert_eq!(output, "data: {\"choices\":[{\"delta\":{\"content\":\"你好🦀\"},\"finish_reason\":null}]}\n\ndata: [DONE]\n\n");
}

#[test]
fn multiline_data_keeps_comments_ids_retries_and_opaque_payloads() {
    let source = Bytes::from_static(b"\xef\xbb\xbf: heartbeat\nevent: message\nid: abc\ndata: {\"choices\": [\n: middle comment\ndata: {\"delta\":{\"reasoning_content\":\"\",\"content\":\"\\u4f60\"},\"finish_reason\":\"\"}\ndata: ],\"opaque\":999999999999999999999999999999}\nretry: 1000\n\n");
    let output = repair_frame(source);
    let text = std::str::from_utf8(&output).unwrap();
    assert!(text.starts_with("\u{feff}: heartbeat\nevent: message\nid: abc\n"));
    assert!(text.ends_with(": middle comment\nretry: 1000\n\n"));
    let data = crate::provider_proxy_transform::stream_frames::event_data(text).unwrap();
    assert_eq!(data, "{\"choices\": [\n{\"delta\":{\"content\":\"\\u4f60\"},\"finish_reason\":null}\n],\"opaque\":999999999999999999999999999999}");
}

#[test]
fn untouched_frames_preserve_bytes_and_allocation() {
    for frame in [
        ": keepalive\n\n", "data: [DONE]\n\n", "data: malformed {\n\n",
        "event: error\ndata: {\"error\":{\"message\":\"no\"}}\n\n",
        "data: {\"choices\":[{\"delta\":{\"content\":\"a\"},\"finish_reason\":null}],  \"extra\":1}\n\n",
    ] {
        let bytes = Bytes::copy_from_slice(frame.as_bytes());
        let pointer = bytes.as_ptr();
        let output = repair_frame(bytes);
        assert_eq!(output, frame);
        assert_eq!(output.as_ptr(), pointer);
    }
}

#[tokio::test]
async fn oversized_frames_pass_through_and_the_next_frame_is_still_repaired() {
    let large = format!("data: {{\"choices\":[{{\"delta\":{{\"reasoning_content\":\"\",\"content\":\"{}\"}}}}]}}\n\n", "a".repeat(40000));
    let next = "data: {\"choices\":[{\"delta\":{\"tool_calls\":[]}}]}\n\n";
    for split in [1, 17, 16385, 50000] {
        let input = format!("{large}{next}");
        let chunks = input
            .as_bytes()
            .chunks(split)
            .map(|bytes| Ok::<_, std::io::Error>(Bytes::copy_from_slice(bytes)))
            .collect::<Vec<_>>();
        let output = collect(normalize_limited(stream::iter(chunks), 1024)).await;
        assert_eq!(
            output,
            format!("{large}data: {{\"choices\":[{{\"delta\":{{}}}}]}}\n\n")
        );
    }
}

#[tokio::test]
async fn repair_limit_boundaries_do_not_drop_blank_line_bytes() {
    let frame = "data: {\"choices\":[{\"delta\":{\"tool_calls\":[]}}]}\n\n";
    for limit in [frame.len() - 1, frame.len(), frame.len() + 1] {
        let input = stream::iter([Ok(Bytes::from_static(frame.as_bytes()))]);
        let output = collect(normalize_limited(input, limit)).await;
        let expected = if limit < frame.len() {
            frame
        } else {
            "data: {\"choices\":[{\"delta\":{}}]}\n\n"
        };
        assert_eq!(output, expected);
    }
}

#[tokio::test]
async fn eof_and_transport_errors_keep_partial_bytes_without_success_markers() {
    let partial = "data: {\"choices\":[{\"delta\":{\"reasoning_content\":\"\"}";
    let output = collect(normalize(stream::iter([Ok(Bytes::from_static(
        partial.as_bytes(),
    ))])))
    .await;
    assert_eq!(output, partial);
    let input = stream::iter([
        Ok(Bytes::from_static(partial.as_bytes())),
        Err(std::io::Error::new(
            std::io::ErrorKind::ConnectionReset,
            "fixture reset",
        )),
    ]);
    let output = normalize(input).collect::<Vec<_>>().await;
    assert_eq!(output.len(), 2);
    assert_eq!(output[0].as_ref().unwrap().as_ref(), partial.as_bytes());
    assert_eq!(
        output[1].as_ref().unwrap_err().kind(),
        std::io::ErrorKind::ConnectionReset
    );
}

#[tokio::test]
async fn complete_events_and_oversized_prefixes_do_not_wait_for_the_next_network_read() {
    for input in [": heartbeat\n\n".to_owned(), "x".repeat(2048)] {
        let source = stream::iter([Ok(Bytes::from(input.clone()))]).chain(stream::pending());
        let output = normalize_limited(source, 1024);
        tokio::pin!(output);
        let first = tokio::time::timeout(std::time::Duration::from_secs(1), output.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert_eq!(first.as_ref(), &input.as_bytes()[..first.len()]);
    }
}

#[tokio::test]
async fn comment_heartbeats_without_blank_lines_are_delivered_immediately() {
    for header in [
        ": heartbeat\n",
        "\u{feff}: heartbeat\n",
        "retry: 1000\n",
        "id: abc\n",
    ] {
        let source =
            stream::iter([Ok(Bytes::copy_from_slice(header.as_bytes()))]).chain(stream::pending());
        let output = normalize(source);
        tokio::pin!(output);
        let first = tokio::time::timeout(std::time::Duration::from_secs(1), output.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert_eq!(first, header);
    }
    let source = stream::iter([Ok(Bytes::from_static(
        b": heartbeat\nid: abc\ndata: {\"choices\":[{\"delta\":{\"tool_calls\":[]}}]}\n\n",
    ))]);
    assert_eq!(
        collect(normalize(source)).await,
        ": heartbeat\nid: abc\ndata: {\"choices\":[{\"delta\":{}}]}\n\n"
    );
}

#[tokio::test]
async fn events_above_the_production_limit_keep_every_byte_and_release_bounded_pieces() {
    let input = format!("data: {{\"opaque\":\"{}\",\"choices\":[{{\"delta\":{{\"tool_calls\":[]}}}}]}}\n\ndata: [DONE]\n\n", "x".repeat(MAX_REPAIR_BYTES));
    let source = stream::iter([Ok(Bytes::from(input.clone()))]);
    let output = normalize(source).collect::<Vec<_>>().await;
    assert!(output
        .iter()
        .all(|chunk| chunk.as_ref().unwrap().len() <= MAX_REPAIR_BYTES));
    assert!(output.len() > 2);
    let combined: Vec<u8> = output
        .into_iter()
        .flat_map(|chunk| chunk.unwrap().to_vec())
        .collect();
    assert_eq!(combined, input.as_bytes());
}
