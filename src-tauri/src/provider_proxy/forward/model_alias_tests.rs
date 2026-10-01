use super::*;
use serde_json::Value;

async fn observed_server(
    status: StatusCode,
    streaming: bool,
    served: Option<&str>,
) -> (Upstream, Arc<Mutex<Vec<(String, Value)>>>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let hits = Arc::new(AtomicUsize::new(0));
    let requests = Arc::new(Mutex::new(Vec::new()));
    let (observed, collected) = (hits.clone(), requests.clone());
    let served = served.map(str::to_owned);
    let router = Router::new().fallback(any(move |request: Request<Body>| {
        let (observed, collected, served) = (observed.clone(), collected.clone(), served.clone());
        async move {
            observed.fetch_add(1, Ordering::SeqCst);
            let path = request.uri().to_string();
            let body: Value = serde_json::from_slice(&to_bytes(request.into_body(), 65536).await.unwrap()).unwrap();
            let model = served.unwrap_or_else(|| body["model"].as_str().unwrap_or("wire").to_string());
            let message = if path.contains("models/") {
                json!({"candidates":[{"content":{"parts":[{"text":"ok"}]},"finishReason":"STOP"}],"modelVersion":model,"usageMetadata":{"promptTokenCount":1000000,"candidatesTokenCount":0}})
            } else if path.ends_with("chat/completions") {
                json!({"id":"chatcmpl_alias","model":model,"choices":[{"message":{"content":"ok"},"finish_reason":"stop"}],"usage":{"prompt_tokens":1000000,"completion_tokens":0}})
            } else if path.ends_with("responses") {
                json!({"id":"resp_alias","model":model,"status":"completed","output":[{"type":"message","content":[{"type":"output_text","text":"ok"}]}],"usage":{"input_tokens":1000000,"output_tokens":0}})
            } else {
                json!({"type":"message","id":"msg_alias","model":model,"content":[],"usage":{"input_tokens":1000000,"output_tokens":0}})
            };
            collected.lock().unwrap().push((path, body));
            let wire = if streaming {
                format!("event: message_start\ndata: {}\n\nevent: message_stop\ndata: {{\"type\":\"message_stop\"}}\n\n", json!({"type":"message_start","message":message}))
            } else { message.to_string() };
            Response::builder().status(status).header("content-type", if streaming {"text/event-stream"} else {"application/json"})
                .body(Body::from(wire)).unwrap()
        }
    }));
    let task = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    (Upstream { url, hits, task }, requests)
}

fn edit_snapshot(app: &App<MockRuntime>, id: &str, edit: impl FnOnce(&mut Value)) {
    let db = app.state::<DbState>();
    let conn = db.0.lock().unwrap();
    let raw: String = conn
        .query_row(
            "SELECT config_snapshot FROM config_profiles WHERE id=?1",
            [id],
            |r| r.get(0),
        )
        .unwrap();
    let mut snapshot = serde_json::from_str(&raw).unwrap();
    edit(&mut snapshot);
    conn.execute(
        "UPDATE config_profiles SET config_snapshot=?1 WHERE id=?2",
        rusqlite::params![snapshot.to_string(), id],
    )
    .unwrap();
}

fn rules(app: &App<MockRuntime>, id: &str, upstream: &str) {
    edit_snapshot(app, id, |snapshot| {
        snapshot["metadata"]["localProxyModelAliases"] = json!([{"model":"*","upstream":upstream}])
    });
}

fn pricing(app: &App<MockRuntime>) {
    let db = app.state::<DbState>();
    let conn = db.0.lock().unwrap();
    for (name, price) in [
        ("fixture-model", "2"),
        ("different-served", "9"),
        ("effective-model", "3"),
    ] {
        conn.execute("INSERT INTO model_pricing(model_id,normalized_model_id,input_cost_per_million,created_at,updated_at) VALUES(?1,?1,?2,'now','now')",[name,price]).unwrap();
    }
}

fn outcome(app: &App<MockRuntime>) -> (String, String, String, f64) {
    let db = app.state::<DbState>();
    let conn = db.0.lock().unwrap();
    conn.query_row(
        "SELECT profile_id,request_model,response_model,CAST(total_cost_usd AS REAL) FROM proxy_request_logs",
        [],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
    )
    .unwrap()
}

#[tokio::test]
async fn model_alias_failover_uses_each_profiles_wire_and_canonical_pricing() {
    let (first, first_requests) =
        observed_server(StatusCode::SERVICE_UNAVAILABLE, false, None).await;
    let (second, second_requests) = observed_server(StatusCode::OK, false, None).await;
    let app = app(
        &[("p1", &first.url, vec![]), ("p2", &second.url, vec![])],
        OptimizerConfig::default(),
    );
    rules(&app, "p1", "a/*");
    rules(&app, "p2", "b/*/pro");
    pricing(&app);
    assert_eq!(
        forward(app.handle().clone(), false).await.status(),
        StatusCode::OK
    );
    assert_eq!(
        first_requests.lock().unwrap()[0].1["model"],
        "a/fixture-model"
    );
    assert_eq!(
        second_requests.lock().unwrap()[0].1["model"],
        "b/fixture-model/pro"
    );
    assert_eq!(
        outcome(&app),
        (
            "p2".into(),
            "fixture-model".into(),
            "b/fixture-model/pro".into(),
            2.0
        )
    );
}

#[tokio::test]
async fn model_alias_streaming_and_different_served_identity_are_accounted_correctly() {
    for served in [None, Some("different-served")] {
        let (upstream, requests) = observed_server(StatusCode::OK, true, served).await;
        let app = app(&[("p1", &upstream.url, vec![])], OptimizerConfig::default());
        rules(&app, "p1", "relay/*");
        pricing(&app);
        let response = forward(app.handle().clone(), true).await;
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = to_bytes(response.into_body(), 65536).await.unwrap();
        assert!(String::from_utf8_lossy(&bytes).contains("message_stop"));
        assert_eq!(
            requests.lock().unwrap()[0].1["model"],
            "relay/fixture-model"
        );
        let expected = served.unwrap_or("relay/fixture-model");
        assert_eq!(
            outcome(&app),
            (
                "p1".into(),
                "fixture-model".into(),
                expected.into(),
                if served.is_some() { 9.0 } else { 2.0 }
            )
        );
    }
}

#[tokio::test]
async fn model_alias_routing_and_session_affinity_keep_canonical_identity() {
    let (first, _) = observed_server(StatusCode::OK, false, None).await;
    let (second, requests) = observed_server(StatusCode::OK, false, None).await;
    let app = app(
        &[("p1", &first.url, vec![]), ("p2", &second.url, vec![])],
        OptimizerConfig::default(),
    );
    rules(&app, "p1", "a/*");
    rules(&app, "p2", "b/*");
    affinity_tests::configure(&app, "session", &["p2", "p1"]);
    let db = app.state::<DbState>();
    {
        let conn = db.0.lock().unwrap();
        let document = crate::provider_proxy::routing::load(&conn, "claude").unwrap();
        let mut policy = document.policy;
        policy.default_group_id = None;
        policy.rules = serde_json::from_value(json!([{"id":"r","name":"Canonical","groupId":"g","model":"fixture-model","matchMode":"exact"}])).unwrap();
        crate::provider_proxy::routing::save(&conn, "claude", document.revision.as_deref(), policy)
            .unwrap();
    }
    for _ in 0..3 {
        assert_eq!(
            affinity_tests::send(&app, Some("alias-session"), json!({"messages":[]}))
                .await
                .status(),
            StatusCode::OK
        );
    }
    assert_eq!(first.hits.load(Ordering::SeqCst), 0);
    assert_eq!(requests.lock().unwrap().len(), 3);
    assert!(requests
        .lock()
        .unwrap()
        .iter()
        .all(|(_, body)| body["model"] == "b/fixture-model"));
}

#[tokio::test]
async fn model_alias_reset_and_global_optimizer_use_effective_pre_alias_model() {
    let (upstream, requests) = observed_server(StatusCode::OK, false, None).await;
    let config = OptimizerConfig {
        enabled: true,
        model_mapper: true,
        model_mapper_default: "effective-model".into(),
        ..Default::default()
    };
    let app = app(&[("p1", &upstream.url, vec![])], config);
    rules(&app, "p1", "relay/*");
    pricing(&app);
    assert_eq!(
        forward(app.handle().clone(), false).await.status(),
        StatusCode::OK
    );
    assert_eq!(
        requests.lock().unwrap()[0].1["model"],
        "relay/effective-model"
    );
    assert_eq!(outcome(&app).1, "effective-model");
    assert_eq!(outcome(&app).3, 3.0);
    edit_snapshot(&app, "p1", |snapshot| {
        snapshot["metadata"]["localProxyModelAliases"] = json!([])
    });
    assert_eq!(
        forward(app.handle().clone(), false).await.status(),
        StatusCode::OK
    );
    assert_eq!(requests.lock().unwrap()[1].1["model"], "effective-model");
}

#[tokio::test]
async fn model_alias_malformed_import_fails_before_any_upstream_request() {
    let (upstream, _) = observed_server(StatusCode::OK, false, None).await;
    let app = app(&[("p1", &upstream.url, vec![])], OptimizerConfig::default());
    edit_snapshot(&app, "p1", |snapshot| {
        snapshot["metadata"]["localProxyModelAliases"] = json!({"secret":"PRIVATE"})
    });
    let response = forward(app.handle().clone(), false).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = to_bytes(response.into_body(), 4096).await.unwrap();
    assert!(!String::from_utf8_lossy(&body).contains("PRIVATE"));
    assert_eq!(upstream.hits.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn model_alias_gemini_native_and_transformed_requests_encode_model_and_keep_query() {
    for (tool, streaming, full_url) in [
        ("gemini", false, false),
        ("gemini", true, true),
        ("claude", false, false),
        ("claude", true, false),
    ] {
        let (upstream, requests) = observed_server(StatusCode::OK, false, None).await;
        let app = app(&[("p1", &upstream.url, vec![])], OptimizerConfig::default());
        let action = if streaming {
            "streamGenerateContent"
        } else {
            "generateContent"
        };
        let endpoint = if full_url {
            format!("{}/v1beta/models/old:{action}?alt=sse", upstream.url)
        } else {
            upstream.url.clone()
        };
        edit_snapshot(&app, "p1", |snapshot| {
            if tool == "gemini" {
                snapshot["env"] =
                    json!({"GOOGLE_GEMINI_BASE_URL":endpoint,"GEMINI_API_KEY":"fixture-token"});
            } else {
                snapshot["env"]["ANTHROPIC_API_FORMAT"] = json!("gemini_native");
            }
            snapshot["metadata"]["useFullUrl"] = json!(full_url);
            snapshot["metadata"]["localProxyModelAliases"] =
                json!([{"model":"fixture-model","upstream":"vendor/中:model"}]);
        });
        if tool == "gemini" {
            let db = app.state::<DbState>();
            let conn = db.0.lock().unwrap();
            conn.execute(
                "UPDATE config_profiles SET tool_id='gemini' WHERE id='p1'",
                [],
            )
            .unwrap();
            conn.execute(
                "UPDATE app_settings SET value=?1 WHERE key='local_provider_proxy_settings'",
                [json!({"enabled_apps":["gemini"],"port":34567}).to_string()],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO app_settings(key,value) VALUES('current_profile_gemini','p1')",
                [],
            )
            .unwrap();
        }
        let path = if tool == "gemini" {
            format!("v1beta/models/fixture-model:{action}")
        } else {
            "v1/messages".into()
        };
        let body = if tool == "gemini" {
            json!({"contents":[]})
        } else {
            json!({"model":"fixture-model","stream":streaming,"messages":[]})
        };
        let request = Request::builder()
            .method("POST")
            .uri(format!("/proxy/{tool}/{path}?alt=sse"))
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap();
        let response = forward_proxy_request_with_client(
            app.handle().clone(),
            tool.into(),
            path,
            request,
            Some(reqwest::Client::builder().no_proxy().build().unwrap()),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        let _ = to_bytes(response.into_body(), 65536).await.unwrap();
        let requests = requests.lock().unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(
            requests[0].0,
            format!("/v1beta/models/vendor%2F%E4%B8%AD%3Amodel:{action}?alt=sse")
        );
        assert!(requests[0].1.get("model").is_none());
    }
}

#[test]
fn model_alias_profile_save_rejects_invalid_rules_without_changing_database() {
    let app = app(
        &[("p1", "http://127.0.0.1:1", vec![])],
        OptimizerConfig::default(),
    );
    let invalid =
        json!({"metadata":{"localProxyModelAliases":[{"model":"core","upstream":""}]}}).to_string();
    assert!(crate::commands::extra::save_config_profile(
        "Invalid".into(),
        "claude".into(),
        invalid.clone(),
        app.state::<DbState>()
    )
    .is_err());
    assert!(crate::commands::extra::update_config_profile(
        "p1".into(),
        "Invalid".into(),
        invalid,
        app.state::<DbState>()
    )
    .is_err());
    let db = app.state::<DbState>();
    let conn = db.0.lock().unwrap();
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM config_profiles", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        conn.query_row("SELECT name FROM config_profiles WHERE id='p1'", [], |r| {
            r.get::<_, String>(0)
        })
        .unwrap(),
        "p1"
    );
}

#[tokio::test]
async fn model_alias_chat_and_responses_rewrite_after_protocol_conversion() {
    for (format, path) in [
        ("openai_chat", "/v1/chat/completions"),
        ("openai_responses", "/v1/responses"),
    ] {
        let (upstream, requests) = observed_server(StatusCode::OK, false, None).await;
        let app = app(&[("p1", &upstream.url, vec![])], OptimizerConfig::default());
        set_format(&app, "p1", format);
        rules(&app, "p1", "vendor/*");
        pricing(&app);
        let response = forward(app.handle().clone(), false).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body: Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 65536).await.unwrap()).unwrap();
        assert_eq!(body["model"], "vendor/fixture-model");
        let requests = requests.lock().unwrap();
        assert_eq!(requests[0].0, path);
        assert_eq!(requests[0].1["model"], "vendor/fixture-model");
        assert_eq!(outcome(&app).3, 2.0);
    }
}

#[tokio::test]
async fn model_alias_analytics_keep_saved_wire_identity_after_profile_rules_change() {
    for served in [None, Some("different-served")] {
        let (upstream, _) = observed_server(StatusCode::OK, false, served).await;
        let app = app(&[("p1", &upstream.url, vec![])], OptimizerConfig::default());
        rules(&app, "p1", "relay/*");
        pricing(&app);
        assert_eq!(
            forward(app.handle().clone(), false).await.status(),
            StatusCode::OK
        );
        edit_snapshot(&app, "p1", |snapshot| {
            snapshot["metadata"]["localProxyModelAliases"] = json!([])
        });
        let filter = served.unwrap_or("fixture-model");
        let analytics = crate::commands::usage_analytics::get_usage_analytics(
            Some(7),
            Some("claude".into()),
            None,
            Some(filter.into()),
            app.state::<DbState>(),
        )
        .unwrap();
        assert_eq!(analytics.summary.total_requests, 1);
        assert_eq!(analytics.models.len(), 1);
        assert_eq!(analytics.models[0].model, filter);
        let db = app.state::<DbState>();
        let conn = db.0.lock().unwrap();
        assert_eq!(
            conn.query_row("SELECT upstream_model FROM proxy_request_logs", [], |r| {
                r.get::<_, String>(0)
            })
            .unwrap(),
            "relay/fixture-model"
        );
    }
}

#[test]
fn model_alias_shared_save_validates_all_members_before_mutation() {
    let app = app(
        &[("p1", "http://127.0.0.1:1", vec![])],
        OptimizerConfig::default(),
    );
    let valid =
        json!({"metadata":{"localProxyModelAliases":[{"model":"core","upstream":"vendor/*"}]}})
            .to_string();
    let invalid =
        json!({"metadata":{"localProxyModelAliases":[{"model":"core","upstream":""}]}}).to_string();
    let inputs = vec![
        crate::commands::extra::SharedConfigProfileInput {
            tool_id: "claude".into(),
            config_snapshot: valid.clone(),
        },
        crate::commands::extra::SharedConfigProfileInput {
            tool_id: "gemini".into(),
            config_snapshot: invalid,
        },
    ];
    assert!(crate::commands::extra::save_shared_config_profiles(
        "Shared".into(),
        inputs,
        None,
        None,
        app.state::<DbState>()
    )
    .is_err());
    {
        let db = app.state::<DbState>();
        let conn = db.0.lock().unwrap();
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM config_profiles", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            1
        );
    }
    let id = crate::commands::extra::save_config_profile(
        "Valid".into(),
        "claude".into(),
        valid.clone(),
        app.state::<DbState>(),
    )
    .unwrap();
    let db = app.state::<DbState>();
    let conn = db.0.lock().unwrap();
    assert_eq!(
        conn.query_row(
            "SELECT config_snapshot FROM config_profiles WHERE id=?1",
            [id],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        valid
    );
}

#[test]
fn model_alias_opencode_jsonc_is_validated_after_normalization() {
    let app = app(
        &[("p1", "http://127.0.0.1:1", vec![])],
        OptimizerConfig::default(),
    );
    let valid="// Native profile\n{\"metadata\":{\"localProxyModelAliases\":[{\"model\":\"core\",\"upstream\":\"vendor/*\"}]}}";
    let invalid = valid.replace("vendor/*", "");
    assert!(crate::commands::extra::save_config_profile(
        "Invalid".into(),
        "opencode".into(),
        invalid.clone(),
        app.state::<DbState>()
    )
    .is_err());
    let id = crate::commands::extra::save_config_profile(
        "Valid".into(),
        "opencode".into(),
        valid.into(),
        app.state::<DbState>(),
    )
    .unwrap();
    assert!(crate::commands::extra::update_config_profile(
        id.clone(),
        "Invalid".into(),
        invalid.clone(),
        app.state::<DbState>()
    )
    .is_err());
    let inputs = vec![crate::commands::extra::SharedConfigProfileInput {
        tool_id: "opencode".into(),
        config_snapshot: invalid,
    }];
    assert!(crate::commands::extra::save_shared_config_profiles(
        "Invalid".into(),
        inputs,
        None,
        None,
        app.state::<DbState>()
    )
    .is_err());
    let db = app.state::<DbState>();
    let conn = db.0.lock().unwrap();
    let snapshot: String = conn
        .query_row(
            "SELECT config_snapshot FROM config_profiles WHERE id=?1",
            [id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&snapshot).unwrap()["metadata"]["localProxyModelAliases"][0]
            ["upstream"],
        "vendor/*"
    );
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM config_profiles", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        2
    );
}
