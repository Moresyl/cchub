use super::*;
use serde_json::Value;

pub(super) fn configure(app: &App<MockRuntime>, affinity: &str, members: &[&str]) {
    let db = app.state::<DbState>();
    let conn = db.0.lock().unwrap();
    let old = crate::provider_proxy::routing::load(&conn, "claude").unwrap();
    let policy = serde_json::from_value(json!({"enabled":true,"affinity":affinity,"defaultGroupId":"g","groups":[{
        "id":"g","name":"Fixture","mode":"roundRobin","members":members.iter().map(|id| json!({"kind":"profile","profileId":id})).collect::<Vec<_>>()
    }]})).unwrap();
    crate::provider_proxy::routing::save(&conn, "claude", old.revision.as_deref(), policy).unwrap();
}

pub(super) async fn send(
    app: &App<MockRuntime>,
    session: Option<&str>,
    mut body: Value,
) -> Response<Body> {
    if body.get("model").is_none() {
        body["model"] = json!("fixture-model");
    }
    let mut builder = Request::builder()
        .method("POST")
        .uri("/proxy/claude/v1/messages")
        .header("content-type", "application/json");
    if let Some(session) = session {
        builder = builder.header("x-session-id", session);
    }
    let request = builder.body(Body::from(body.to_string())).unwrap();
    tokio::time::timeout(
        Duration::from_secs(5),
        forward_proxy_request_with_client(
            app.handle().clone(),
            "claude".into(),
            "v1/messages".into(),
            request,
            Some(reqwest::Client::builder().no_proxy().build().unwrap()),
        ),
    )
    .await
    .unwrap()
}

async fn served(response: Response<Body>) -> String {
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), 65536).await.unwrap();
    serde_json::from_slice::<Value>(&bytes).unwrap()["served"]
        .as_str()
        .unwrap()
        .to_string()
}

fn prompt(text: &str) -> Value {
    json!({"messages":[{"role":"user","content":text}]})
}

#[tokio::test]
async fn affinity_session_prefers_verified_profile_and_partitions_session_model_or_missing_identity(
) {
    let first = server(StatusCode::OK, "application/json", r#"{"served":"p1"}"#).await;
    let second = server(StatusCode::OK, "application/json", r#"{"served":"p2"}"#).await;
    let app = app(
        &[("p1", &first.url, vec![]), ("p2", &second.url, vec![])],
        OptimizerConfig::default(),
    );
    configure(&app, "session", &["p1", "p2"]);
    for _ in 0..3 {
        assert_eq!(
            served(send(&app, Some("same"), prompt("hello")).await).await,
            "p1"
        );
    }
    assert_eq!(
        served(send(&app, Some("different"), prompt("hello")).await).await,
        "p2"
    );
    assert_eq!(served(send(&app, None, prompt("hello")).await).await, "p1");
    assert_eq!(
        served(
            send(
                &app,
                Some("same"),
                json!({"model":"different-model","messages":[]})
            )
            .await
        )
        .await,
        "p2"
    );
    assert_eq!(
        served(send(&app, Some("same"), prompt("hello")).await).await,
        "p1"
    );
    assert_eq!(first.hits.load(Ordering::SeqCst), 5);
    assert_eq!(second.hits.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn affinity_turn_keeps_tool_results_but_new_user_turn_uses_group_order() {
    let first = server(StatusCode::OK, "application/json", r#"{"served":"p1"}"#).await;
    let second = server(StatusCode::OK, "application/json", r#"{"served":"p2"}"#).await;
    let app = app(
        &[("p1", &first.url, vec![]), ("p2", &second.url, vec![])],
        OptimizerConfig::default(),
    );
    configure(&app, "turn", &["p1", "p2"]);
    assert_eq!(
        served(send(&app, Some("same"), prompt("hello")).await).await,
        "p1"
    );
    let continuation = json!({"messages":[{"role":"user","content":"hello"},
        {"role":"assistant","content":[{"type":"tool_use","id":"call","name":"run","input":{}}]},
        {"role":"user","content":[{"type":"tool_result","tool_use_id":"call","content":"ok"}]}]});
    for _ in 0..2 {
        assert_eq!(
            served(send(&app, Some("same"), continuation.clone()).await).await,
            "p1"
        );
    }
    assert_eq!(
        served(send(&app, Some("same"), prompt("next turn")).await).await,
        "p2"
    );
}

#[tokio::test]
async fn affinity_auto_retains_recent_cache_only_when_vendor_reports_the_threshold() {
    for cached in [0, 1023, 1024] {
        let wire: &'static str = match cached {
            0 => r#"{"served":"p1","usage":{"input_tokens":2000,"output_tokens":1}}"#,
            1023 => {
                r#"{"served":"p1","usage":{"input_tokens":2000,"output_tokens":1,"cache_read_input_tokens":1023}}"#
            }
            _ => {
                r#"{"served":"p1","usage":{"input_tokens":2000,"output_tokens":1,"cache_read_input_tokens":1024}}"#
            }
        };
        let first = server(StatusCode::OK, "application/json", wire).await;
        let second = server(StatusCode::OK, "application/json", r#"{"served":"p2"}"#).await;
        let app = app(
            &[("p1", &first.url, vec![]), ("p2", &second.url, vec![])],
            OptimizerConfig::default(),
        );
        configure(&app, "auto", &["p1", "p2"]);
        assert_eq!(
            served(send(&app, Some("same"), prompt("first")).await).await,
            "p1"
        );
        assert_eq!(
            served(send(&app, Some("same"), prompt("next")).await).await,
            if cached >= 1024 { "p1" } else { "p2" }
        );
    }
}

#[tokio::test]
async fn affinity_cancelled_or_incomplete_stream_never_becomes_the_successful_profile() {
    for cancelled in [true, false] {
        let first = server(StatusCode::OK,"text/event-stream",
            "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"model\":\"fixture-model\",\"usage\":{\"input_tokens\":7}}}\n\n").await;
        let second = server(StatusCode::OK, "application/json", r#"{"served":"p2"}"#).await;
        let app = app(
            &[("p1", &first.url, vec![]), ("p2", &second.url, vec![])],
            OptimizerConfig::default(),
        );
        configure(&app, "session", &["p1", "p2"]);
        let response = send(&app, Some("same"), json!({"stream":true,"messages":[]})).await;
        if cancelled {
            drop(response);
        } else {
            let bytes = to_bytes(response.into_body(), 65536).await.unwrap();
            assert!(std::str::from_utf8(&bytes).unwrap().contains("error"));
        }
        assert_eq!(
            served(send(&app, Some("same"), prompt("hello")).await).await,
            "p2"
        );
        assert_eq!(
            served(send(&app, Some("same"), prompt("hello")).await).await,
            "p2"
        );
        assert_eq!(first.hits.load(Ordering::SeqCst), 1);
    }
}

#[tokio::test]
async fn affinity_successful_stream_binds_only_after_client_observes_completion() {
    let first = server(StatusCode::OK,"text/event-stream",
        "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"model\":\"fixture-model\",\"usage\":{\"input_tokens\":7}}}\n\nevent: message_stop\ndata: {\"type\":\"message_stop\"}\n\n").await;
    let second = server(StatusCode::OK, "application/json", r#"{"served":"p2"}"#).await;
    let app = app(
        &[("p1", &first.url, vec![]), ("p2", &second.url, vec![])],
        OptimizerConfig::default(),
    );
    configure(&app, "session", &["p1", "p2"]);
    for _ in 0..2 {
        let response = send(&app, Some("same"), json!({"stream":true,"messages":[]})).await;
        let bytes = to_bytes(response.into_body(), 65536).await.unwrap();
        assert!(std::str::from_utf8(&bytes)
            .unwrap()
            .contains("event: message_stop"));
    }
    assert_eq!(first.hits.load(Ordering::SeqCst), 2);
    assert_eq!(second.hits.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn affinity_late_whole_reply_cannot_replace_a_newer_success() {
    let release = Arc::new(Notify::new());
    let first = controlled_server(
        StatusCode::OK,
        "application/json",
        r#"{"served":"p1"}"#,
        Some(release.clone()),
        false,
    )
    .await;
    let second = server(StatusCode::OK, "application/json", r#"{"served":"p2"}"#).await;
    let app = app(
        &[("p1", &first.url, vec![]), ("p2", &second.url, vec![])],
        OptimizerConfig::default(),
    );
    configure(&app, "session", &["p1", "p2"]);
    let first_request = send(&app, Some("same"), prompt("hello"));
    tokio::pin!(first_request);
    tokio::select! {
        response = &mut first_request => panic!("unexpected early response: {}",response.status()),
        _ = async { while first.hits.load(Ordering::SeqCst)==0 { tokio::task::yield_now().await; } } => {}
    }
    assert_eq!(
        served(send(&app, Some("same"), prompt("hello")).await).await,
        "p2"
    );
    release.notify_one();
    assert_eq!(served(first_request.await).await, "p1");
    assert_eq!(
        served(send(&app, Some("same"), prompt("hello")).await).await,
        "p2"
    );
}

#[tokio::test]
async fn affinity_changed_profile_or_policy_rejects_late_binding_even_if_restored_later() {
    for policy in [true, false] {
        let release = Arc::new(Notify::new());
        let first = controlled_server(
            StatusCode::OK,
            "application/json",
            r#"{"served":"p1"}"#,
            Some(release.clone()),
            false,
        )
        .await;
        let second = server(StatusCode::OK, "application/json", r#"{"served":"p2"}"#).await;
        let app = app(
            &[("p1", &first.url, vec![]), ("p2", &second.url, vec![])],
            OptimizerConfig::default(),
        );
        configure(&app, "session", &["p1", "p2"]);
        let pending = send(&app, Some("same"), prompt("hello"));
        tokio::pin!(pending);
        tokio::select! {
            response = &mut pending => panic!("unexpected early response: {}",response.status()),
            _ = async { while first.hits.load(Ordering::SeqCst)==0 { tokio::task::yield_now().await; } } => {}
        }
        let sql = if policy {
            "SELECT value FROM app_settings WHERE key='provider_routing:claude'"
        } else {
            "SELECT config_snapshot FROM config_profiles WHERE id='p1'"
        };
        let update = if policy {
            "UPDATE app_settings SET value=?1 WHERE key='provider_routing:claude'"
        } else {
            "UPDATE config_profiles SET config_snapshot=?1 WHERE id='p1'"
        };
        let original: String = app
            .state::<DbState>()
            .0
            .lock()
            .unwrap()
            .query_row(sql, [], |row| row.get(0))
            .unwrap();
        let mut changed: Value = serde_json::from_str(&original).unwrap();
        if policy {
            changed["revision"] = json!("new-policy");
        } else {
            changed["env"]["ANTHROPIC_AUTH_TOKEN"] = json!("new-key");
        }
        app.state::<DbState>()
            .0
            .lock()
            .unwrap()
            .execute(update, [changed.to_string()])
            .unwrap();
        release.notify_one();
        assert_eq!(served(pending.await).await, "p1");
        app.state::<DbState>()
            .0
            .lock()
            .unwrap()
            .execute(update, [original])
            .unwrap();
        assert_eq!(
            served(send(&app, Some("same"), prompt("hello")).await).await,
            "p2"
        );
    }
}

#[tokio::test]
async fn affinity_unavailable_profile_respects_circuits_and_promotes_the_actual_fallback() {
    let first = server(StatusCode::OK, "application/json", r#"{"served":"p1"}"#).await;
    let second = server(StatusCode::OK, "application/json", r#"{"served":"p2"}"#).await;
    let app = app(
        &[("p1", &first.url, vec![]), ("p2", &second.url, vec![])],
        OptimizerConfig::default(),
    );
    configure(&app, "session", &["p1", "p2"]);
    assert_eq!(
        served(send(&app, Some("same"), prompt("first")).await).await,
        "p1"
    );
    open_profile(&app, "p1", false);
    assert_eq!(
        served(send(&app, Some("same"), prompt("next")).await).await,
        "p2"
    );
    app.state::<LocalProviderProxyRuntime>()
        .0
        .lock()
        .unwrap()
        .profile_circuits
        .clear();
    assert_eq!(
        served(send(&app, Some("same"), prompt("next")).await).await,
        "p2"
    );
    assert_eq!(first.hits.load(Ordering::SeqCst), 1);
    assert_eq!(second.hits.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn affinity_failure_retries_group_members_only_within_the_configured_budget() {
    for failover in [true, false] {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let hits = Arc::new(AtomicUsize::new(0));
        let observed = hits.clone();
        let router = Router::new().fallback(any(move || {
            let first = observed.fetch_add(1, Ordering::SeqCst) == 0;
            async move {
                Response::builder()
                    .status(if first {
                        StatusCode::OK
                    } else {
                        StatusCode::SERVICE_UNAVAILABLE
                    })
                    .header("content-type", "application/json")
                    .header("retry-after", "17")
                    .body(Body::from(if first {
                        r#"{"served":"p1"}"#
                    } else {
                        r#"{"error":"unavailable"}"#
                    }))
                    .unwrap()
            }
        }));
        let task = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        let first = Upstream { url, hits, task };
        let second = server(StatusCode::OK, "application/json", r#"{"served":"p2"}"#).await;
        let app = app(
            &[("p1", &first.url, vec![]), ("p2", &second.url, vec![])],
            OptimizerConfig {
                failover_enabled: failover,
                ..Default::default()
            },
        );
        configure(&app, "session", &["p1", "p2"]);
        assert_eq!(
            served(send(&app, Some("same"), prompt("first")).await).await,
            "p1"
        );
        let response = send(&app, Some("same"), prompt("next")).await;
        if failover {
            assert_eq!(served(response).await, "p2");
            assert_eq!(
                served(send(&app, Some("same"), prompt("next")).await).await,
                "p2"
            );
        } else {
            assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
            assert_eq!(response.headers()["retry-after"], "17");
            to_bytes(response.into_body(), 65536).await.unwrap();
        }
        assert_eq!(first.hits.load(Ordering::SeqCst), 2);
        assert_eq!(
            second.hits.load(Ordering::SeqCst),
            if failover { 2 } else { 0 }
        );
    }
}
