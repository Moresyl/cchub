use super::*;
use crate::provider_proxy::routing::{self, RoutingPolicy};

fn routing_policy(
    app: &App<MockRuntime>,
    enabled: bool,
    quota: bool,
    mode: &str,
    members: &[&str],
) {
    let policy: RoutingPolicy = serde_json::from_value(json!({"enabled":enabled,"quotaAware":quota,
        "defaultGroupId":"g","groups":[{"id":"g","name":"Fixture","mode":mode,
        "pickedProfileId":members[0], "members":members.iter().map(|id| json!({"kind":"profile","profileId":id})).collect::<Vec<_>>()
    }],"rules":[]})).unwrap();
    let state = app.state::<DbState>();
    let conn = state.0.lock().unwrap();
    let previous = routing::load(&conn, "claude").unwrap();
    routing::save(&conn, "claude", previous.revision.as_deref(), policy).unwrap();
}

fn bind(app: &App<MockRuntime>, id: &str, provider: &str, account: &str, alias: Option<&str>) {
    let state = app.state::<DbState>();
    let conn = state.0.lock().unwrap();
    let snapshot: String = conn
        .query_row(
            "SELECT config_snapshot FROM config_profiles WHERE id=?1",
            [id],
            |row| row.get(0),
        )
        .unwrap();
    let mut value: serde_json::Value = serde_json::from_str(&snapshot).unwrap();
    value["metadata"]["providerType"] = json!(provider);
    value["metadata"]["authBinding"] = json!({"authProvider":provider,"accountId":account});
    value["metadata"]["localProxyModelAliases"] = alias
        .map(|model| json!([{"model":"*","upstream":model}]))
        .unwrap_or(json!([]));
    conn.execute(
        "UPDATE config_profiles SET config_snapshot=?1 WHERE id=?2",
        rusqlite::params![value.to_string(), id],
    )
    .unwrap();
}

async fn observed_codex(manager: &CodexOAuthManager) {
    let usage = server(
        StatusCode::OK,
        "application/json",
        r#"{"rate_limit":{"allowed":false,"limit_reached":true}}"#,
    )
    .await;
    let client = reqwest::Client::builder().no_proxy().build().unwrap();
    manager
        .quota_json(Some("one"), |id, token| {
            client
                .get(&usage.url)
                .bearer_auth(token)
                .header("chatgpt-account-id", id)
        })
        .await
        .unwrap();
    assert_eq!(usage.hits.load(Ordering::SeqCst), 1);
}

fn assert_no_quota_side_effects(app: &App<MockRuntime>, ids: &[&str]) {
    let runtime = app.state::<LocalProviderProxyRuntime>();
    let state = runtime.0.lock().unwrap();
    for id in ids {
        assert!(!state
            .profile_circuits
            .contains_key(&profile_circuit_key("claude", id)));
    }
    drop(state);
    let db = app.state::<DbState>();
    let conn = db.0.lock().unwrap();
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM proxy_request_logs", [], |row| row
            .get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        conn.query_row(
            "SELECT value FROM app_settings WHERE key='current_profile_claude'",
            [],
            |row| row.get::<_, String>(0)
        )
        .unwrap(),
        "p1"
    );
}

#[tokio::test]
async fn quota_skips_do_not_consume_request_budget_or_change_the_active_profile() {
    let exhausted = server(StatusCode::OK, "application/json", "{}").await;
    let available = server(StatusCode::OK, "application/json", "{}").await;
    let app = app(
        &[
            ("p1", &exhausted.url, vec![]),
            ("p2", &exhausted.url, vec![]),
            ("p3", &available.url, vec![]),
        ],
        OptimizerConfig {
            failover_enabled: false,
            ..Default::default()
        },
    );
    let (_dir, manager) = AccountManager::seed("codex_oauth", &app).await;
    let AccountManager::Codex(manager) = manager else {
        unreachable!()
    };
    observed_codex(&manager).await;
    for id in ["p1", "p2"] {
        bind(&app, id, "codex_oauth", "one", None);
    }
    // With no matching group, opt-in quota selection still keeps the active profile intact.
    routing_policy(&app, true, true, "ordered", &["p1", "p2", "p3"]);
    let db = app.state::<DbState>();
    {
        let conn = db.0.lock().unwrap();
        let doc = routing::load(&conn, "claude").unwrap();
        let mut policy = doc.policy;
        policy.default_group_id = None;
        routing::save(&conn, "claude", doc.revision.as_deref(), policy).unwrap();
    }
    assert_eq!(
        forward(app.handle().clone(), false).await.status(),
        StatusCode::OK
    );
    assert_eq!(exhausted.hits.load(Ordering::SeqCst), 0);
    assert_eq!(available.hits.load(Ordering::SeqCst), 1);
    assert_eq!(
        db.0.lock()
            .unwrap()
            .query_row(
                "SELECT value FROM app_settings WHERE key='current_profile_claude'",
                [],
                |row| row.get::<_, String>(0)
            )
            .unwrap(),
        "p1"
    );
    let runtime = app.state::<LocalProviderProxyRuntime>();
    assert!(!runtime
        .0
        .lock()
        .unwrap()
        .profile_circuits
        .contains_key(&profile_circuit_key("claude", "p1")));
}

#[tokio::test]
async fn exhausted_manual_and_ordered_groups_do_not_escape_or_mark_circuit_failures() {
    for mode in ["manual", "ordered"] {
        let selected = server(StatusCode::OK, "application/json", "{}").await;
        let excluded = server(StatusCode::OK, "application/json", "{}").await;
        let app = app(
            &[("p1", &selected.url, vec![]), ("p2", &excluded.url, vec![])],
            Default::default(),
        );
        let (_dir, manager) = AccountManager::seed("codex_oauth", &app).await;
        let AccountManager::Codex(manager) = manager else {
            unreachable!()
        };
        observed_codex(&manager).await;
        bind(&app, "p1", "codex_oauth", "one", None);
        routing_policy(
            &app,
            true,
            true,
            mode,
            if mode == "manual" {
                &["p1", "p2"]
            } else {
                &["p1"]
            },
        );
        let response = forward(app.handle().clone(), false).await;
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        let retry: u64 = response.headers()["retry-after"]
            .to_str()
            .unwrap()
            .parse()
            .unwrap();
        assert!((1..=300).contains(&retry));
        assert_eq!(selected.hits.load(Ordering::SeqCst), 0);
        assert_eq!(excluded.hits.load(Ordering::SeqCst), 0);
        assert_no_quota_side_effects(&app, &["p1", "p2"]);
    }
}

#[tokio::test]
async fn disabled_policy_or_quota_option_preserves_existing_forwarding() {
    let upstream = server(StatusCode::OK, "application/json", "{}").await;
    let app = app(&[("p1", &upstream.url, vec![])], Default::default());
    let (_dir, manager) = AccountManager::seed("codex_oauth", &app).await;
    let AccountManager::Codex(manager) = manager else {
        unreachable!()
    };
    observed_codex(&manager).await;
    bind(&app, "p1", "codex_oauth", "one", None);
    for (enabled, quota) in [(false, true), (true, false)] {
        routing_policy(&app, enabled, quota, "ordered", &["p1"]);
        assert_eq!(
            forward(app.handle().clone(), false).await.status(),
            StatusCode::OK
        );
    }
    assert_eq!(upstream.hits.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn actual_copilot_resources_gate_the_final_alias_without_blocking_free_or_unknown_models() {
    let upstream = server(StatusCode::OK, "application/json", "{}").await;
    let usage = server(StatusCode::OK, "application/json", r#"{"copilot_plan":"individual_pro","quota_snapshots":{"premium_interactions":{"entitlement":300,"quota_remaining":0}}}"#).await;
    let models = server(StatusCode::OK, "application/json", r#"{"data":[{"id":"paid","name":"Paid","vendor":"fixture","model_picker_enabled":true,"billing":{"is_premium":true,"multiplier":1}},{"id":"free","name":"Free","vendor":"fixture","model_picker_enabled":true,"billing":{"is_premium":false,"multiplier":0}}]}"#).await;
    let app = app(&[("p1", &upstream.url, vec![])], Default::default());
    let (_dir, manager) = AccountManager::seed("github_copilot", &app).await;
    let AccountManager::Copilot(manager) = manager else {
        unreachable!()
    };
    crate::copilot_auth::test_support::query_resources(&manager, "1", &usage.url, &models.url)
        .await
        .unwrap();
    routing_policy(&app, true, true, "ordered", &["p1"]);
    bind(&app, "p1", "github_copilot", "1", Some("paid"));
    assert_eq!(
        forward(app.handle().clone(), false).await.status(),
        StatusCode::TOO_MANY_REQUESTS
    );
    assert_no_quota_side_effects(&app, &["p1"]);
    for model in [Some("free"), None] {
        bind(&app, "p1", "github_copilot", "1", model);
        assert_eq!(
            forward(app.handle().clone(), false).await.status(),
            StatusCode::OK
        );
    }
    bind(&app, "p1", "github_copilot", "2", Some("paid"));
    assert_eq!(
        forward(app.handle().clone(), false).await.status(),
        StatusCode::OK
    );
    let failed = server(StatusCode::SERVICE_UNAVAILABLE, "application/json", "{}").await;
    crate::copilot_auth::test_support::query_resources(&manager, "1", &usage.url, &failed.url)
        .await
        .unwrap();
    bind(&app, "p1", "github_copilot", "1", Some("paid"));
    assert_eq!(
        forward(app.handle().clone(), false).await.status(),
        StatusCode::OK
    );
    assert_eq!(upstream.hits.load(Ordering::SeqCst), 4);
}

#[tokio::test]
async fn quota_does_not_filter_catalog_requests_and_failure_budget_stays_bounded() {
    let upstream = server(
        StatusCode::SERVICE_UNAVAILABLE,
        "application/json",
        r#"{"error":"vendor failure"}"#,
    )
    .await;
    let skipped = server(StatusCode::OK, "application/json", "{}").await;
    let excluded = server(StatusCode::OK, "application/json", "{}").await;
    let app = app(
        &[
            ("p1", &upstream.url, vec![]),
            ("p2", &skipped.url, vec![]),
            ("p3", &upstream.url, vec![]),
            ("p4", &excluded.url, vec![]),
        ],
        OptimizerConfig {
            max_profile_retries: 1,
            ..Default::default()
        },
    );
    let (_dir, manager) = AccountManager::seed("codex_oauth", &app).await;
    let AccountManager::Codex(manager) = manager else {
        unreachable!()
    };
    observed_codex(&manager).await;
    bind(&app, "p2", "codex_oauth", "one", None);
    routing_policy(&app, true, true, "ordered", &["p1", "p2", "p3", "p4"]);
    let response = forward(app.handle().clone(), false).await;
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(upstream.hits.load(Ordering::SeqCst), 2);
    assert_eq!(skipped.hits.load(Ordering::SeqCst), 0);
    assert_eq!(excluded.hits.load(Ordering::SeqCst), 0);
    routing_policy(&app, true, true, "manual", &["p2"]);
    let request = Request::builder()
        .method("GET")
        .uri("/proxy/claude/v1/models")
        .body(Body::empty())
        .unwrap();
    let response = forward_proxy_request_with_client(
        app.handle().clone(),
        "claude".into(),
        "v1/models".into(),
        request,
        Some(reqwest::Client::builder().no_proxy().build().unwrap()),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(skipped.hits.load(Ordering::SeqCst), 1);
}
