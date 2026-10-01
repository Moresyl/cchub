use super::*;
use axum::body::to_bytes;
use reqwest::header::{HeaderMap, HeaderValue};

const TYPED: &str = r#"{"choices":[{"message":{"content":[{"type":"thinking","thinking":"想"},{"type":"text","text":"答"}]},"finish_reason":"stop"}],"opaque":1.2300e+999}"#;

fn headers(mime: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert("content-type", HeaderValue::from_str(mime).unwrap());
    for name in [
        "etag",
        "content-md5",
        "digest",
        "content-digest",
        "repr-digest",
        "x-request-id",
    ] {
        headers.insert(name, HeaderValue::from_static("fixture-original"));
    }
    headers
}

#[tokio::test]
async fn repaired_raw_json_invalidates_only_original_body_validators() {
    for mime in ["application/json; charset=UTF-8", "application/vendor+json"] {
        let response = raw_response(
            StatusCode::OK,
            &headers(mime),
            Bytes::from_static(TYPED.as_bytes()),
            "v1/chat/completions",
        );
        for name in [
            "etag",
            "content-md5",
            "digest",
            "content-digest",
            "repr-digest",
        ] {
            assert!(!response.headers().contains_key(name));
        }
        assert_eq!(response.headers()["x-request-id"], "fixture-original");
        assert_eq!(response.headers()["content-type"], mime);
        let bytes = to_bytes(response.into_body(), 65536).await.unwrap();
        let text = std::str::from_utf8(&bytes).unwrap();
        assert!(text.contains(r#""content":"答""#));
        assert!(text.contains(r#""reasoning_content":"想""#));
        assert!(text.contains(r#""opaque":1.2300e+999"#));
    }
}

#[tokio::test]
async fn other_paths_mime_errors_and_opaque_json_keep_body_and_validators() {
    let opaque = TYPED.replace(
        r#""text","text":"答""#,
        r#""image_url","image_url":{"url":"opaque://image"}"#,
    );
    for (status, mime, path, body) in [
        (StatusCode::OK, "text/plain", "v1/chat/completions", TYPED),
        (
            StatusCode::OK,
            "text/event-stream",
            "v1/chat/completions",
            TYPED,
        ),
        (StatusCode::OK, "application/json", "v1/messages", TYPED),
        (StatusCode::OK, "application/json", "v1/responses", TYPED),
        (
            StatusCode::BAD_REQUEST,
            "application/json",
            "v1/chat/completions",
            TYPED,
        ),
        (
            StatusCode::OK,
            "application/json",
            "v1/chat/completions",
            opaque.as_str(),
        ),
        (
            StatusCode::OK,
            "application/json",
            "v1/chat/completions",
            "{malformed",
        ),
        (
            StatusCode::OK,
            "application/json",
            "v1/chat/completions",
            r#"{"choices":[{"message":{"content":"ordinary"}}]}"#,
        ),
    ] {
        let response = raw_response(
            status,
            &headers(mime),
            Bytes::copy_from_slice(body.as_bytes()),
            path,
        );
        assert_eq!(response.status(), status);
        assert_eq!(response.headers()["etag"], "fixture-original");
        assert_eq!(to_bytes(response.into_body(), 65536).await.unwrap(), body);
    }
}

#[test]
fn whole_repair_above_cap_preserves_original_allocation_and_bytes() {
    let body = format!(
        r#"{{"opaque":"{}","choices":[{{"message":{{"content":[{{"type":"text","text":"答"}}]}}}}]}}"#,
        "x".repeat(8 * 1024 * 1024)
    );
    let bytes = Bytes::from(body);
    let pointer = bytes.as_ptr();
    let result = super::super::streaming_chat::repair_whole(bytes.clone());
    assert_eq!(result, bytes);
    assert_eq!(result.as_ptr(), pointer);
}

#[tokio::test]
async fn native_desktop_response_restores_its_public_model_and_keeps_usage() {
    let converted = serde_json::json!({"model":"upstream-private","content":[{"type":"text","text":"答案"}],"usage":{"input_tokens":7}});
    let response = finish_json_response(
        StatusCode::OK,
        &headers("application/json"),
        Bytes::new(),
        Some(converted),
        "v1/messages",
        true,
        br#"{"model":"desktop-public"}"#,
    );
    let value: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 65536).await.unwrap()).unwrap();
    assert_eq!(value["model"], "desktop-public");
    assert_eq!(value["usage"]["input_tokens"], 7);
    assert_eq!(value["content"][0]["text"], "答案");
}
