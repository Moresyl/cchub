use super::*;
use serde_json::json;
use tokio::io::AsyncWriteExt;

#[test]
fn catalog_preserves_capabilities_and_enriches_duplicate_rows() {
    let rows = parse(&json!({"models":[
        {"slug":" a ","display_name":"Alpha","context_window":200000,
         "supported_reasoning_levels":[{"effort":"high"},"max","high"],
         "default_reasoning_level":"high","input_modalities":["text","image"]},
        {"id":"a","max_output_tokens":32000,"context_window":"bad"},
        "b", {"id":"a"}
    ]}))
    .unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].model.context_window, Some(200000));
    assert_eq!(rows[0].model.max_output_tokens, Some(32000));
    let wire = serde_json::to_value(&rows[0]).unwrap();
    assert_eq!(wire["id"], "a");
    assert_eq!(wire["displayName"], "Alpha");
    assert_eq!(wire["ownedBy"], "Codex");
    assert_eq!(wire["supportedReasoningLevels"], json!(["high", "max"]));
    assert_eq!(wire["defaultReasoningEffort"], "high");
    assert_eq!(wire["inputModalities"], json!(["text", "image"]));
    assert!(wire.get("model").is_none());
}

#[test]
fn camel_case_model_capabilities_and_explicit_empty_levels_are_preserved() {
    let rows = parse(&json!({"data":[
        {"model":"camel","displayName":"Camel","supportedReasoningEfforts":[
            {"reasoningEffort":"low"},{"reasoningEffort":"future-effort"}],
         "defaultReasoningEffort":"low","contextWindow":128000},
        {"id":"empty","supported_reasoning_efforts":[]},
        {"id":"unknown","supportedReasoningEfforts":[{},null],"defaultReasoningEffort":false}
    ]}))
    .unwrap();
    assert_eq!(
        rows[0].model.supported_reasoning_levels,
        Some(vec!["low".into(), "future-effort".into()])
    );
    assert_eq!(
        rows[0].model.default_reasoning_effort.as_deref(),
        Some("low")
    );
    assert_eq!(rows[1].model.supported_reasoning_levels, Some(vec![]));
    assert_eq!(rows[2].model.supported_reasoning_levels, None);
    assert_eq!(rows[2].model.default_reasoning_effort, None);
}

#[test]
fn catalog_accepts_legacy_containers_without_inventing_models_from_invalid_values() {
    for value in [
        json!(["legacy"]),
        json!({"items":["legacy"]}),
        json!({"models":{"legacy":{"label":"Legacy"},"invalid":null}}),
    ] {
        let rows = parse(&value).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].model.id, "legacy");
        assert_eq!(rows[0].model.supported_reasoning_levels, None);
        assert_eq!(rows[0].model.input_modalities, None);
    }
    for value in [
        json!({"error":"private"}),
        json!({"models":null}),
        json!(null),
        json!({"data":{}}),
    ] {
        assert_eq!(
            parse(&value).unwrap_err(),
            "Invalid Codex model catalog container"
        );
    }
    assert!(parse(&json!({"models":[]})).unwrap().is_empty());
    assert!(parse(&json!({"models":{}})).unwrap().is_empty());
}

#[test]
fn hidden_models_are_not_offered_as_picker_choices() {
    let rows = parse(&json!({"models":[
        {"id":"old","hidden":true}, {"id":"internal","visibility":"hide"},
        {"id":"visible","hidden":false}, {"id":"other","visibility":"list"}
    ]}))
    .unwrap();
    assert_eq!(
        rows.iter()
            .map(|row| row.model.id.as_str())
            .collect::<Vec<_>>(),
        vec!["other", "visible"]
    );
}

#[test]
fn model_requests_use_the_same_contract_for_cli_and_managed_credentials() {
    let client = reqwest::Client::new();
    for account in [None, Some("fixture-account")] {
        let request = request(&client, "fixture-token", account).build().unwrap();
        assert_eq!(
            request.url().as_str().split('?').next().unwrap(),
            MODELS_URL
        );
        assert_eq!(
            request.url().query_pairs().collect::<Vec<_>>(),
            vec![("client_version".into(), env!("CARGO_PKG_VERSION").into())]
        );
        assert_eq!(request.headers()["originator"], "cchub");
        assert_eq!(request.headers()["accept"], "application/json");
        assert_eq!(request.headers()["authorization"], "Bearer fixture-token");
        assert_eq!(
            request
                .headers()
                .get("chatgpt-account-id")
                .map(|value| value.to_str().unwrap()),
            account
        );
    }
}

async fn reply(status: u16, body: String) -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/models", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        crate::shared::usage_http::test_support::read_headers(&mut socket).await;
        let headers = format!(
            "HTTP/1.1 {status} Test\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        socket.write_all(headers.as_bytes()).await.unwrap();
        // A bounded reader may close before the full body has been delivered.
        let _ = socket.write_all(body.as_bytes()).await;
        let _ = socket.shutdown().await;
    });
    (url, task)
}

#[tokio::test]
async fn cli_response_is_bounded_and_errors_do_not_expose_upstream_content() {
    let client = reqwest::Client::builder().no_proxy().build().unwrap();
    for (status, body, expected) in [
        (
            403,
            "fixture-token private challenge".into(),
            "Codex model API returned HTTP 403",
        ),
        (
            200,
            "not-json fixture-token".into(),
            "Invalid Codex model response",
        ),
        (
            200,
            "x".repeat(2 * 1024 * 1024 + 1),
            "Codex model response exceeds the size limit",
        ),
        (
            200,
            r#"{"error":"fixture-token"}"#.into(),
            "Invalid Codex model catalog container",
        ),
    ] {
        let (url, task) = reply(status, body).await;
        assert_eq!(
            fetch_cli_models(client.get(url)).await.unwrap_err(),
            expected
        );
        task.await.unwrap();
    }
    let (url, task) = reply(
        200,
        r#"{"models":[{"slug":"custom","supported_reasoning_levels":["max"]}]}"#.into(),
    )
    .await;
    let rows = fetch_cli_models(client.get(url)).await.unwrap();
    assert_eq!(
        rows[0].model.supported_reasoning_levels,
        Some(vec!["max".into()])
    );
    task.await.unwrap();
}
