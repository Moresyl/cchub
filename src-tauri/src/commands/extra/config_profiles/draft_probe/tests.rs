use super::*;
use std::sync::Mutex;
use tauri::Manager;

fn app() -> tauri::App<tauri::test::MockRuntime> {
    tauri::test::mock_builder()
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .unwrap()
}

#[test]
fn model_selection_validation_is_bounded_and_deduplicates_ids() {
    assert_eq!(
        validate_models(vec![" a ".into(), "a".into(), "b".into()]).unwrap(),
        vec!["a", "b"]
    );
    for models in [
        vec![],
        vec!["".into()],
        vec!["bad\nmodel".into()],
        vec!["x".repeat(257)],
        vec!["a".into(); 33],
    ] {
        assert!(validate_models(models).is_err());
    }
}

#[test]
fn draft_transport_keeps_authoritative_auth_and_a_minimal_request_budget() {
    let parsed = serde_json::json!({"metadata":{
        "customUserAgent":"Draft Agent", "requestHeaders":{"x-fixture":"new","Authorization":"stolen"},
        "localProxyRequestOverrides":{"headers":{"X-Fixture":"override","x-api-key":"stolen"},"body":{"model":"wrong","messages":[],"stream":false,"max_tokens":1000000,"max_completion_tokens":1000000,"temperature":0.2}}
    }});
    let original = StreamCheckRequestSpec {
        endpoint: "https://fixture.test/v1/chat/completions".into(),
        headers: vec![("authorization".into(), "Bearer correct-key".into())],
        body: serde_json::json!({"model":"old","messages":[{"role":"user","content":"Reply with OK."}],"stream":true,"max_tokens":16}),
    };
    let prepared = prepare_model_request(&parsed, &original, "chosen").unwrap();
    assert_eq!(
        prepared
            .headers
            .iter()
            .find(|(name, _)| name == "authorization")
            .unwrap()
            .1,
        "Bearer correct-key"
    );
    assert_eq!(
        prepared
            .headers
            .iter()
            .filter(|(name, _)| name.eq_ignore_ascii_case("x-fixture"))
            .count(),
        1
    );
    assert!(prepared
        .headers
        .contains(&("X-Fixture".into(), "override".into())));
    assert!(prepared
        .headers
        .contains(&("user-agent".into(), "Draft Agent".into())));
    assert_eq!(prepared.body["model"], "chosen");
    assert_eq!(prepared.body["messages"], original.body["messages"]);
    assert_eq!(prepared.body["max_tokens"], 16);
    assert_eq!(prepared.body["stream"], true);
    assert!(prepared.body.get("max_completion_tokens").is_none());
    assert_eq!(prepared.body["temperature"], 0.2);
}

#[test]
fn google_model_paths_are_encoded_and_generation_settings_keep_the_probe_budget() {
    let original = StreamCheckRequestSpec {
        endpoint: "https://fixture.test/v1beta/models/old:streamGenerateContent?alt=sse".into(),
        headers: vec![],
        body: serde_json::json!({"contents":[{"parts":[{"text":"Reply with OK."}]}],"generationConfig":{"maxOutputTokens":16}}),
    };
    let parsed = serde_json::json!({"metadata":{"localProxyRequestOverrides":{"body":{"generationConfig":{"temperature":0.2,"maxOutputTokens":1000000}}}}});
    let prepared = prepare_model_request(&parsed, &original, "model/with space").unwrap();
    assert!(prepared
        .endpoint
        .contains("model%2Fwith%20space:streamGenerateContent"));
    assert!(prepared.endpoint.ends_with("?alt=sse"));
    assert_eq!(prepared.body["generationConfig"]["maxOutputTokens"], 16);
    assert_eq!(prepared.body["generationConfig"]["temperature"], 0.2);
}

#[tokio::test]
async fn invalid_drafts_or_unavailable_oauth_fail_without_using_another_account() {
    let app = app();
    for snapshot in ["{", "[]", "null"] {
        assert!(check_draft(
            app.handle(),
            "claude".into(),
            snapshot.into(),
            vec!["a".into()],
            client()
        )
        .await
        .is_err());
    }
    for provider in ["github_copilot", "codex_oauth", "xai_oauth"] {
        let snapshot = serde_json::json!({"env":{"ANTHROPIC_API_FORMAT": if provider == "codex_oauth" {"openai_responses"} else {"openai_chat"}},"metadata":{"providerType":provider,"authBinding":{"authProvider":provider,"accountId":"unavailable"}}});
        let error = check_draft(
            app.handle(),
            "claude".into(),
            snapshot.to_string(),
            vec!["a".into()],
            client(),
        )
        .await
        .unwrap_err();
        assert!(error.contains("authentication is unavailable"), "{error}");
    }
    let snapshot =
        serde_json::json!({"config":"model = \"broken", "auth":{"OPENAI_API_KEY":"fixture"}});
    assert!(check_draft(
        app.handle(),
        "codex".into(),
        snapshot.to_string(),
        vec!["a".into()],
        client()
    )
    .await
    .unwrap_err()
    .contains("TOML"));
}

#[tokio::test]
async fn actual_draft_command_uses_new_credentials_and_leaves_the_database_unchanged() {
    let app = app();
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute_batch("CREATE TABLE app_settings (key TEXT PRIMARY KEY, value TEXT); CREATE TABLE config_profiles (id TEXT, config_snapshot TEXT); INSERT INTO config_profiles VALUES ('saved', '{\"key\":\"old\"}');").unwrap();
    app.manage(DbState(Mutex::new(conn)));
    let (base, task) = server().await;
    let snapshot = serde_json::json!({"env":{"ANTHROPIC_AUTH_TOKEN":"new-draft-key","ANTHROPIC_BASE_URL":base,"ANTHROPIC_MODEL":"old"},"metadata":{"customUserAgent":"Draft Agent","requestHeaders":{"x-fixture":"draft-header"},"localProxyRequestOverrides":{"body":{"temperature":0.2}}}}).to_string();
    let result = test_profile_draft(
        "claude".into(),
        snapshot.clone(),
        vec!["one".into(), "two".into()],
        app.handle().clone(),
        app.state::<DbState>(),
    )
    .await
    .unwrap();
    task.abort();
    assert_eq!(
        result
            .iter()
            .map(|item| item.model.as_str())
            .collect::<Vec<_>>(),
        vec!["one", "two"]
    );
    assert!(result.iter().all(|item| item.status == "healthy"));
    let db = app.state::<DbState>();
    let conn = db.0.lock().unwrap();
    let saved: String = conn
        .query_row(
            "SELECT config_snapshot FROM config_profiles WHERE id = 'saved'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(saved, r#"{"key":"old"}"#);
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM config_profiles", [], |row| row
            .get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert!(snapshot.contains("new-draft-key"));
}

#[tokio::test]
async fn draft_requests_follow_the_selected_protocol_for_each_supported_tool() {
    let app = app();
    let (base, task) = server().await;
    let metadata = serde_json::json!({"customUserAgent":"Draft Agent","requestHeaders":{"x-fixture":"draft-header"},"localProxyRequestOverrides":{"body":{"temperature":0.2}}});
    let snapshots = [
        (
            "claude",
            serde_json::json!({"env":{"ANTHROPIC_AUTH_TOKEN":"new-draft-key","ANTHROPIC_BASE_URL":base,"ANTHROPIC_API_FORMAT":"openai_chat"},"metadata":metadata}),
        ),
        (
            "codex",
            serde_json::json!({"config":format!("model_provider = 'chosen'\nmodel = 'old'\n[model_providers.unused]\nbase_url = 'http://127.0.0.1:1/wrong'\nwire_api = 'chat'\n[model_providers.chosen]\nbase_url = '{base}/v1'\nwire_api = 'responses'"),"auth":{"OPENAI_API_KEY":"new-draft-key"},"metadata":metadata}),
        ),
        (
            "gemini",
            serde_json::json!({"env":{"GEMINI_API_KEY":"new-draft-key","GOOGLE_GEMINI_BASE_URL":format!("{base}/v1beta")},"metadata":metadata}),
        ),
        (
            "openclaw",
            serde_json::json!({"baseUrl":format!("{base}/v1"),"apiKey":"new-draft-key","api":"openai-completions","models":[{"id":"old"}],"metadata":metadata}),
        ),
        (
            "hermes",
            serde_json::json!({"config":{"model":{"provider":"custom","base_url":format!("{base}/v1"),"default":"old"}},"env":{"HERMES_API_KEY":"new-draft-key"},"metadata":{"customUserAgent":"Draft Agent","requestHeaders":{"x-fixture":"draft-header"},"hermesApiKeyEnv":"HERMES_API_KEY","localProxyRequestOverrides":{"body":{"temperature":0.2}}}}),
        ),
        (
            "opencode",
            serde_json::json!({"npm":"@ai-sdk/openai-compatible","options":{"apiKey":"new-draft-key","baseURL":format!("{base}/v1")},"models":{"old":{}},"metadata":metadata}),
        ),
    ];
    for (tool, snapshot) in snapshots {
        let result = check_draft(
            app.handle(),
            tool.into(),
            snapshot.to_string(),
            vec!["one".into()],
            client(),
        )
        .await
        .unwrap();
        assert_eq!(result[0].status, "healthy", "{tool}: {}", result[0].message);
    }
    task.abort();
}

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .no_proxy()
        .timeout(std::time::Duration::from_secs(2))
        .build()
        .unwrap()
}

#[tokio::test]
async fn draft_batches_check_every_model_with_at_most_four_simultaneous_requests() {
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    let app = app();
    let started = Arc::new(AtomicUsize::new(0));
    let permits = Arc::new(tokio::sync::Semaphore::new(0));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let seen = started.clone();
    let gate = permits.clone();
    let route = axum::Router::new().fallback(axum::routing::post(move || {
        let seen = seen.clone();
        let gate = gate.clone();
        async move {
            seen.fetch_add(1, Ordering::SeqCst);
            gate.acquire().await.unwrap().forget();
            axum::Json(serde_json::json!({"choices":[{"message":{"content":"OK"},"finish_reason":"stop"}]}))
        }
    }));
    let task = tokio::spawn(async move {
        axum::serve(listener, route).await.unwrap();
    });
    let snapshot = serde_json::json!({"env":{"ANTHROPIC_AUTH_TOKEN":"fixture","ANTHROPIC_BASE_URL":base,"ANTHROPIC_API_FORMAT":"openai_chat"}}).to_string();
    let models = (0..10).map(|i| format!("model-{i}")).collect::<Vec<_>>();
    let handle = app.handle().clone();
    let expected = models.clone();
    let checking = tokio::spawn(async move {
        check_draft(&handle, "claude".into(), snapshot, models, client()).await
    });
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while started.load(Ordering::SeqCst) < 4 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    tokio::task::yield_now().await;
    assert_eq!(started.load(Ordering::SeqCst), 4);
    permits.add_permits(10);
    let results = checking.await.unwrap().unwrap();
    task.abort();
    assert_eq!(
        results
            .iter()
            .map(|result| &result.model)
            .collect::<Vec<_>>(),
        expected.iter().collect::<Vec<_>>()
    );
    assert!(results.iter().all(|result| result.status == "healthy"));
    assert_eq!(started.load(Ordering::SeqCst), 10);
}

async fn server() -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let router = axum::Router::new().fallback(axum::routing::post(|uri: axum::http::Uri, headers: axum::http::HeaderMap, axum::Json(body): axum::Json<Value>| async move {
        let key = headers.get("x-api-key").or_else(|| headers.get("x-goog-api-key")).or_else(|| headers.get("authorization")).unwrap().to_str().unwrap();
        assert!(key.ends_with("new-draft-key"));
        assert_eq!(headers.get("x-fixture").unwrap(), "draft-header");
        assert_eq!(headers.get("user-agent").unwrap(), "Draft Agent");
        assert_eq!(body["temperature"], 0.2);
        if body.get("contents").is_some() {
            assert!(uri.path().ends_with("/models/one:streamGenerateContent"));
            assert_eq!(body["generationConfig"]["maxOutputTokens"], 16);
            return axum::Json(serde_json::json!({"candidates":[{"content":{"parts":[{"text":"OK"}]},"finishReason":"STOP"}]}));
        }
        assert!(matches!(body["model"].as_str(), Some("one" | "two")));
        if body.get("input").is_some() {
            assert!(uri.path().ends_with("/responses"));
            assert_eq!(body["max_output_tokens"], 16);
            return axum::Json(serde_json::json!({"type":"response","id":"fixture","status":"completed","output":[]}));
        }
        assert_eq!(body["max_tokens"], 16);
        if headers.contains_key("x-api-key") {
            assert!(uri.path().ends_with("/messages"));
            axum::Json(serde_json::json!({"type":"message","id":"fixture","stop_reason":"end_turn","content":[]}))
        } else {
            assert!(uri.path().ends_with("/chat/completions"));
            axum::Json(serde_json::json!({"choices":[{"message":{"content":"OK"},"finish_reason":"stop"}]}))
        }
    }));
    let task = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    (base, task)
}
