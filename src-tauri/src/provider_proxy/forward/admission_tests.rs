use super::*;
use crate::provider_proxy::admission::{scope_key, Store};

fn configure(app: &App<MockRuntime>, limit: u32, queued: u32, timeout: u64) -> Store {
    let config = OptimizerConfig {
        admission: crate::proxy_optimizer::admission::AdmissionConfig {
            max_concurrent: limit,
            max_queued: queued,
            queue_timeout_secs: timeout,
            ..Default::default()
        },
        ..Default::default()
    };
    crate::provider_proxy::update_optimizer_config_cache(app.handle(), config);
    app.state::<LocalProviderProxyRuntime>()
        .0
        .lock()
        .unwrap()
        .admission
        .clone()
}

async fn count(store: &Store, active: usize, queued: usize) {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let stats = store.stats().unwrap();
            if (stats.active, stats.queued) == (active, queued) {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("admission state must settle");
}

#[tokio::test]
async fn streaming_headers_do_not_release_account_and_never_polled_drop_wakes_the_queue() {
    let primary = streaming_tests::split_server_with_pending("event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":1}}}\n\n", true).await;
    let alternative = server(StatusCode::OK, "application/json", "{}").await;
    let app = app(
        &[
            ("p1", &primary.url, vec![]),
            ("p2", &alternative.url, vec![]),
        ],
        OptimizerConfig::default(),
    );
    let store = configure(&app, 1, 1, 30);
    let first = forward(app.handle().clone(), true).await;
    assert_eq!(first.status(), StatusCode::OK);
    count(&store, 1, 0).await;
    let handle = app.handle().clone();
    let second = tokio::spawn(async move { forward(handle, true).await });
    count(&store, 1, 1).await;
    let third = forward(app.handle().clone(), true).await;
    assert_eq!(third.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(third.headers()["retry-after"], "1");
    let error =
        String::from_utf8(to_bytes(third.into_body(), 4096).await.unwrap().to_vec()).unwrap();
    assert!(error.contains("local account queue is full"));
    assert!(!error.contains("fixture-token"));
    assert_eq!(primary.hits.load(Ordering::SeqCst), 1);
    assert_eq!(alternative.hits.load(Ordering::SeqCst), 0);
    assert_eq!(profile(&app, "p1").consecutive_failures, 0);
    drop(first);
    let second = second.await.unwrap();
    assert_eq!(second.status(), StatusCode::OK);
    assert_eq!(primary.hits.load(Ordering::SeqCst), 2);
    drop(second);
    count(&store, 0, 0).await;
    let stats =
        crate::commands::optimizer_commands::get_proxy_admission_stats(app.handle().clone())
            .unwrap();
    assert_eq!(stats.active, 0);
    assert_eq!(stats.entries.len(), 1);
    assert_eq!(stats.entries[0].active, 0);
}

#[tokio::test]
async fn local_wait_timeout_and_cancel_never_send_or_change_circuit_health() {
    let gate = Arc::new(Notify::new());
    let primary = controlled_server(
        StatusCode::OK,
        "application/json",
        "{}",
        Some(gate.clone()),
        false,
    )
    .await;
    let app = app(&[("p1", &primary.url, vec![])], OptimizerConfig::default());
    let store = configure(&app, 1, 2, 1);
    let handle = app.handle().clone();
    let first = tokio::spawn(async move { forward(handle, false).await });
    count(&store, 1, 0).await;
    let handle = app.handle().clone();
    let cancelled = tokio::spawn(async move { forward(handle, false).await });
    count(&store, 1, 1).await;
    cancelled.abort();
    assert!(cancelled.await.unwrap_err().is_cancelled());
    count(&store, 1, 0).await;
    let timed_out = forward(app.handle().clone(), false).await;
    assert_eq!(timed_out.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(primary.hits.load(Ordering::SeqCst), 1);
    assert_eq!(profile(&app, "p1").consecutive_failures, 0);
    count(&store, 1, 0).await;
    let conn = app.state::<DbState>();
    assert_eq!(
        conn.0
            .lock()
            .unwrap()
            .query_row("SELECT count(*) FROM proxy_request_logs", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
    gate.notify_one();
    assert_eq!(first.await.unwrap().status(), StatusCode::OK);
    count(&store, 0, 0).await;
}

#[tokio::test]
async fn same_account_across_profiles_queues_and_live_settings_wake_without_stale_request_override()
{
    let primary = streaming_tests::split_server_with_pending(
        "data: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":1}}}\n\n",
        true,
    )
    .await;
    let app = app(
        &[("p1", &primary.url, vec![]), ("p2", &primary.url, vec![])],
        OptimizerConfig::default(),
    );
    let store = configure(&app, 1, 2, 30);
    let first = forward(app.handle().clone(), true).await;
    app.state::<DbState>()
        .0
        .lock()
        .unwrap()
        .execute(
            "UPDATE app_settings SET value='p2' WHERE key='current_profile_claude'",
            [],
        )
        .unwrap();
    let handle = app.handle().clone();
    let second = tokio::spawn(async move { forward(handle, true).await });
    count(&store, 1, 1).await;
    let stats = store.stats().unwrap();
    assert_eq!(stats.entries.len(), 1);
    assert_eq!(stats.entries[0].profiles, ["p1", "p2"]);
    let mut config = OptimizerConfig::default();
    config
        .admission
        .account_limits
        .insert(stats.entries[0].key.clone(), 2);
    crate::provider_proxy::update_optimizer_config_cache(app.handle(), config);
    let second = second.await.unwrap();
    count(&store, 2, 0).await;
    assert_eq!(primary.hits.load(Ordering::SeqCst), 2);
    drop((first, second));
    count(&store, 0, 0).await;
}

#[tokio::test]
async fn queued_managed_credentials_are_rechecked_before_network_send() {
    use crate::codex_oauth::{test_support, CodexOAuthState};
    let primary = server(StatusCode::OK, "application/json", "{}").await;
    let app = app(&[("p1", &primary.url, vec![])], OptimizerConfig::default());
    let (_directory, manager) = test_support::seeded().await;
    assert!(app.manage(CodexOAuthState(manager.clone())));
    let snapshot = json!({"env":{"ANTHROPIC_BASE_URL":primary.url}, "metadata":{
        "providerType":"codex_oauth", "authBinding":{"authProvider":"codex_oauth","accountId":"one"}
    }})
    .to_string();
    app.state::<DbState>()
        .0
        .lock()
        .unwrap()
        .execute(
            "UPDATE config_profiles SET config_snapshot=?1 WHERE id='p1'",
            [&snapshot],
        )
        .unwrap();
    let store = configure(&app, 1, 1, 30);
    let target = crate::provider_proxy::extract_upstream_target(
        app.handle(),
        "claude",
        "p1".into(),
        "p1".into(),
        &snapshot,
    )
    .await
    .unwrap();
    let headers =
        crate::provider_proxy::forward::transport::effective_headers(&[], &target, &[]).unwrap();
    let key = scope_key(&target, &headers, &url::Url::parse(&primary.url).unwrap()).unwrap();
    let blocker = store.acquire(key, "blocker").await.unwrap();
    let handle = app.handle().clone();
    let queued = tokio::spawn(async move { forward(handle, false).await });
    count(&store, 1, 1).await;
    manager.remove_account("one").await.unwrap();
    drop(blocker);
    assert_eq!(queued.await.unwrap().status(), StatusCode::CONFLICT);
    assert_eq!(primary.hits.load(Ordering::SeqCst), 0);
    count(&store, 0, 0).await;
}

#[tokio::test]
async fn corrupt_persisted_policy_cannot_silently_remove_limits_or_contact_upstream() {
    let primary = server(StatusCode::OK, "application/json", "{}").await;
    let app = app(&[("p1", &primary.url, vec![])], OptimizerConfig::default());
    for invalid in ["private-setting-sentinel", "null"] {
        app.state::<LocalProviderProxyRuntime>()
            .0
            .lock()
            .unwrap()
            .optimizer_config = None;
        app.state::<DbState>().0.lock().unwrap().execute("INSERT OR REPLACE INTO app_settings(key,value) VALUES('proxy_optimizer_config',?1)", [invalid]).unwrap();
        let response = forward(app.handle().clone(), false).await;
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let error = String::from_utf8(to_bytes(response.into_body(), 4096).await.unwrap().to_vec())
            .unwrap();
        assert!(!error.contains("private-setting-sentinel"));
        assert_eq!(primary.hits.load(Ordering::SeqCst), 0);
        assert!(app
            .state::<LocalProviderProxyRuntime>()
            .0
            .lock()
            .unwrap()
            .optimizer_config
            .is_none());
    }
}

#[tokio::test]
async fn real_proxy_connection_cancel_releases_queue_without_an_upstream_request() {
    use tokio::io::AsyncWriteExt;
    let gate = Arc::new(Notify::new());
    let primary = controlled_server(
        StatusCode::OK,
        "application/json",
        "{}",
        Some(gate.clone()),
        false,
    )
    .await;
    let app = app(&[("p1", &primary.url, vec![])], OptimizerConfig::default());
    let store = configure(&app, 1, 2, 30);
    let handle = app.handle().clone();
    let router = Router::new().fallback(any(move |request: Request<Body>| {
        let app = handle.clone();
        async move {
            forward_proxy_request_with_client(
                app,
                "claude".into(),
                "v1/messages".into(),
                request,
                Some(reqwest::Client::builder().no_proxy().build().unwrap()),
            )
            .await
        }
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    let proxy = Upstream {
        url: address.to_string(),
        hits: Arc::new(AtomicUsize::new(0)),
        task,
    };
    let payload = r#"{"model":"fixture-model","messages":[],"stream":false}"#;
    let request = format!("POST /proxy/claude/v1/messages HTTP/1.1\r\nHost: {address}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{payload}", payload.len());
    let mut first = tokio::net::TcpStream::connect(address).await.unwrap();
    first.write_all(request.as_bytes()).await.unwrap();
    count(&store, 1, 0).await;
    let mut second = tokio::net::TcpStream::connect(address).await.unwrap();
    second.write_all(request.as_bytes()).await.unwrap();
    count(&store, 1, 1).await;
    drop(second);
    count(&store, 1, 0).await;
    assert_eq!(primary.hits.load(Ordering::SeqCst), 1);
    gate.notify_one();
    count(&store, 0, 0).await;
    drop((first, proxy));
}

#[tokio::test]
async fn queued_managed_account_uses_its_new_token_without_following_a_changed_default() {
    use crate::codex_oauth::{test_support, CodexOAuthState};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let headers = Arc::new(Mutex::new(Vec::new()));
    let observed = headers.clone();
    let router = Router::new().fallback(any(move |request: Request<Body>| {
        observed.lock().unwrap().push(request.headers().clone());
        async {
            Response::builder()
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .unwrap()
        }
    }));
    let primary = Upstream {
        url,
        hits: Arc::new(AtomicUsize::new(0)),
        task: tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        }),
    };
    let app = app(&[("p1", &primary.url, vec![])], OptimizerConfig::default());
    let (_directory, manager) = test_support::seeded().await;
    manager.set_default_account("one").await.unwrap();
    assert!(app.manage(CodexOAuthState(manager.clone())));
    let snapshot = json!({"env":{"ANTHROPIC_BASE_URL":primary.url}, "metadata":{"providerType":"codex_oauth"}}).to_string();
    app.state::<DbState>()
        .0
        .lock()
        .unwrap()
        .execute(
            "UPDATE config_profiles SET config_snapshot=?1 WHERE id='p1'",
            [&snapshot],
        )
        .unwrap();
    let store = configure(&app, 1, 1, 30);
    let target = crate::provider_proxy::extract_upstream_target(
        app.handle(),
        "claude",
        "p1".into(),
        "p1".into(),
        &snapshot,
    )
    .await
    .unwrap();
    let old_headers =
        crate::provider_proxy::forward::transport::effective_headers(&[], &target, &[]).unwrap();
    let key = scope_key(
        &target,
        &old_headers,
        &url::Url::parse(&primary.url).unwrap(),
    )
    .unwrap();
    let blocker = store.acquire(key, "blocker").await.unwrap();
    let handle = app.handle().clone();
    let queued = tokio::spawn(async move { forward(handle, false).await });
    count(&store, 1, 1).await;
    test_support::replace_cached_token(&manager, "one", "fresh-one-access").await;
    manager.set_default_account("two").await.unwrap();
    drop(blocker);
    assert_eq!(queued.await.unwrap().status(), StatusCode::OK);
    let headers = headers.lock().unwrap();
    assert_eq!(headers.len(), 1);
    assert_eq!(headers[0]["authorization"], "Bearer fresh-one-access");
    assert_eq!(headers[0]["chatgpt-account-id"], "one");
    count(&store, 0, 0).await;
}

#[tokio::test]
async fn actual_transport_uses_only_configured_credentials_and_preserves_other_query_fields() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let recorded = Arc::new(Mutex::new(None));
    let observed = recorded.clone();
    let router = Router::new().fallback(any(move |request: Request<Body>| {
        *observed.lock().unwrap() = Some((request.headers().clone(), request.uri().clone()));
        async {
            Response::builder()
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .unwrap()
        }
    }));
    let primary = Upstream {
        url,
        hits: Arc::new(AtomicUsize::new(0)),
        task: tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        }),
    };
    let app = app(&[("p1", &primary.url, vec![])], OptimizerConfig::default());
    for query_credential in [false, true] {
        if query_credential {
            let snapshot = json!({"env": {
                "ANTHROPIC_BASE_URL": format!("{}/v1?key=configured-query", primary.url),
                "ANTHROPIC_AUTH_TOKEN": "fixture-token"
            }})
            .to_string();
            app.state::<DbState>()
                .0
                .lock()
                .unwrap()
                .execute(
                    "UPDATE config_profiles SET config_snapshot=?1 WHERE id='p1'",
                    [&snapshot],
                )
                .unwrap();
        }
        let request = Request::builder().method("POST")
        .uri("/proxy/claude/v1/messages?key=private-local&access_token=private-local&trace=keep%20value")
        .header("content-type", "application/json").header("authorization", "Bearer private-local")
        .header("x-api-key", "private-local")
        .body(Body::from(r#"{"model":"fixture-model","messages":[]}"#)).unwrap();
        let response = forward_proxy_request_with_client(
            app.handle().clone(),
            "claude".into(),
            "v1/messages".into(),
            request,
            Some(reqwest::Client::builder().no_proxy().build().unwrap()),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        let record = recorded.lock().unwrap();
        let (headers, uri) = record.as_ref().unwrap();
        assert_eq!(headers.get_all("x-api-key").iter().count(), 1);
        assert_eq!(headers["x-api-key"], "fixture-token");
        assert!(!headers.contains_key("authorization"));
        assert_eq!(uri.path(), "/v1/messages");
        let mut expected = vec![("trace".into(), "keep value".into())];
        if query_credential {
            expected.insert(0, ("key".into(), "configured-query".into()));
        }
        assert_eq!(
            url::form_urlencoded::parse(uri.query().unwrap().as_bytes()).collect::<Vec<_>>(),
            expected
        );
    }
}
