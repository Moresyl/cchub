use super::*;
use crate::provider_proxy::routing::{RoutingDocument, RoutingPolicy};

fn configure(
    app: &App<MockRuntime>,
    mode: &str,
    members: &[&str],
    picked: Option<&str>,
    rules: serde_json::Value,
) -> RoutingDocument {
    let policy: RoutingPolicy = serde_json::from_value(json!({"enabled":true,"defaultGroupId":"g","groups":[{
        "id":"g","name":"Fixture group","mode":mode,"pickedProfileId":picked,
        "members":members.iter().map(|id| json!({"kind":"profile","profileId":id})).collect::<Vec<_>>()
    }],"rules":rules})).unwrap();
    let db = app.state::<DbState>();
    let conn = db.0.lock().unwrap();
    crate::provider_proxy::routing::save(&conn, "claude", None, policy).unwrap()
}

#[tokio::test]
async fn ordered_routing_fails_over_inside_the_group_without_changing_active_profile() {
    let first = server(
        StatusCode::SERVICE_UNAVAILABLE,
        "application/json",
        r#"{"error":"unavailable"}"#,
    )
    .await;
    let second = server(
        StatusCode::OK,
        "application/json",
        r#"{"content":[],"usage":{"input_tokens":7,"output_tokens":3}}"#,
    )
    .await;
    let excluded = server(StatusCode::OK, "application/json", r#"{"excluded":true}"#).await;
    let app = app(
        &[
            ("p1", &second.url, vec![]),
            ("p2", &first.url, vec![]),
            ("p3", &excluded.url, vec![]),
        ],
        OptimizerConfig::default(),
    );
    configure(&app, "ordered", &["p2", "p1"], None, json!([]));
    let response = forward(app.handle().clone(), false).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(first.hits.load(Ordering::SeqCst), 1);
    assert_eq!(second.hits.load(Ordering::SeqCst), 1);
    assert_eq!(excluded.hits.load(Ordering::SeqCst), 0);
    streaming_tests::assert_single_outcome(&app, 200, 1, 7);
    let db = app.state::<DbState>();
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
}

#[tokio::test]
async fn round_robin_changes_starting_profile_without_consuming_failover_candidates() {
    let first = server(StatusCode::OK, "application/json", r#"{"served":"p1"}"#).await;
    let second = server(StatusCode::OK, "application/json", r#"{"served":"p2"}"#).await;
    let app = app(
        &[("p1", &first.url, vec![]), ("p2", &second.url, vec![])],
        OptimizerConfig::default(),
    );
    configure(&app, "roundRobin", &["p1", "p2"], None, json!([]));
    for expected in ["p1", "p2", "p1", "p2"] {
        let response = forward(app.handle().clone(), false).await;
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = to_bytes(response.into_body(), 4096).await.unwrap();
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&bytes).unwrap()["served"],
            expected
        );
    }
    assert_eq!(first.hits.load(Ordering::SeqCst), 2);
    assert_eq!(second.hits.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn manual_selection_does_not_escape_to_other_members_on_failure() {
    let first = server(StatusCode::OK, "application/json", r#"{"served":"p1"}"#).await;
    let second = server(
        StatusCode::SERVICE_UNAVAILABLE,
        "application/json",
        r#"{"error":"unavailable"}"#,
    )
    .await;
    let app = app(
        &[("p1", &first.url, vec![]), ("p2", &second.url, vec![])],
        OptimizerConfig::default(),
    );
    configure(&app, "manual", &["p1", "p2"], Some("p2"), json!([]));
    let response = forward(app.handle().clone(), false).await;
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(first.hits.load(Ordering::SeqCst), 0);
    assert_eq!(second.hits.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn model_rule_precedes_default_and_missing_member_never_sends_to_an_unselected_profile() {
    let first = server(StatusCode::OK, "application/json", r#"{"served":"p1"}"#).await;
    let second = server(StatusCode::OK, "application/json", r#"{"served":"p2"}"#).await;
    let app = app(
        &[("p1", &first.url, vec![]), ("p2", &second.url, vec![])],
        OptimizerConfig::default(),
    );
    let document = configure(&app, "ordered", &["p2"], None, json!([]));
    let mut policy = document.policy;
    policy.default_group_id = None;
    policy.rules = serde_json::from_value(json!([{"id":"r","name":"Model rule","groupId":"g","model":"fixture-model","matchMode":"exact"}])).unwrap();
    {
        let db = app.state::<DbState>();
        let conn = db.0.lock().unwrap();
        crate::provider_proxy::routing::save(&conn, "claude", document.revision.as_deref(), policy)
            .unwrap();
    }
    assert_eq!(
        forward(app.handle().clone(), false).await.status(),
        StatusCode::OK
    );
    assert_eq!(first.hits.load(Ordering::SeqCst), 0);
    assert_eq!(second.hits.load(Ordering::SeqCst), 1);
    app.state::<DbState>()
        .0
        .lock()
        .unwrap()
        .execute("DELETE FROM config_profiles WHERE id='p2'", [])
        .unwrap();
    assert_eq!(
        forward(app.handle().clone(), false).await.status(),
        StatusCode::BAD_GATEWAY
    );
    assert_eq!(first.hits.load(Ordering::SeqCst), 0);
    assert_eq!(second.hits.load(Ordering::SeqCst), 1);
}
