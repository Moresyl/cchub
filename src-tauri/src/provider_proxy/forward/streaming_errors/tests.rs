use super::*;

#[tokio::test]
async fn each_protocol_gets_one_native_error_without_echoing_transport_secrets() {
    for (protocol, name, error_type) in [
        (Protocol::Messages, Some("error"), "api_error"),
        (Protocol::Responses, Some("response.failed"), "server_error"),
        (Protocol::Chat, None, "api_error"),
        (Protocol::Gemini, None, "UNAVAILABLE"),
    ] {
        let input = futures_util::stream::iter([
            Ok(Bytes::from_static(b"data: partial")),
            Err(std::io::Error::other("https://private.invalid/?key=secret")),
            Ok(Bytes::from_static(b"late bytes")),
        ]);
        let chunks = terminate(input, protocol, "Provider one".into())
            .collect::<Vec<_>>()
            .await;
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].as_ref().unwrap().as_ref(), b"data: partial");
        let failure = std::str::from_utf8(chunks[1].as_ref().unwrap()).unwrap();
        assert!(failure.starts_with("\n\n"));
        assert!(failure.contains(error_type));
        assert!(failure.contains("Provider one"));
        assert!(failure.contains("connection lost"));
        assert!(!failure.contains("private.invalid"));
        assert!(!failure.contains("secret"));
        assert!(!failure.contains("message_stop"));
        assert!(!failure.contains("[DONE]"));
        if let Some(name) = name {
            assert!(failure.contains(&format!("event: {name}\n")));
        } else {
            assert!(!failure.contains("event:"));
        }
        let value: serde_json::Value = serde_json::from_str(
            failure
                .lines()
                .find_map(|line| line.strip_prefix("data: "))
                .unwrap(),
        )
        .unwrap();
        if matches!(protocol, Protocol::Responses) {
            assert_eq!(value["response"]["status"], "failed");
        } else {
            assert!(value.get("error").is_some());
        }
    }
}

#[tokio::test]
async fn whole_streams_are_unchanged_and_unknown_paths_are_not_assigned_a_protocol() {
    let data = Bytes::from_static(b"event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n");
    let input = futures_util::stream::iter([Ok(data.clone())]);
    let output = terminate(input, Protocol::Messages, "Provider".into())
        .collect::<Vec<_>>()
        .await;
    assert_eq!(output.len(), 1);
    assert_eq!(output[0].as_ref().unwrap(), &data);
    for path in [
        "notmessages",
        "some_responses",
        "v1/messages/count_tokens",
        "audio/stream",
    ] {
        assert!(Protocol::for_path(path).is_none(), "{path}");
    }
    for path in [
        "v1/messages",
        "v1/responses",
        "chat/completions",
        "models/gemini:streamGenerateContent",
    ] {
        assert!(Protocol::for_path(path).is_some(), "{path}");
    }
}
