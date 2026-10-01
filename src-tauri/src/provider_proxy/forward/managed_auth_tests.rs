use super::*;
use crate::codex_oauth::{CodexOAuthManager, CodexOAuthState};
use crate::copilot_auth::{CopilotAuthManager, CopilotAuthState};
use crate::xai_oauth::{XaiOAuthManager, XaiOAuthState};
use axum::http::HeaderMap;

#[path = "affinity_auth_tests.rs"]
mod affinity_auth_tests;
#[path = "quota_tests.rs"]
mod quota_tests;

enum AccountManager {
    Codex(Arc<CodexOAuthManager>),
    Xai(Arc<XaiOAuthManager>),
    Copilot(Arc<CopilotAuthManager>),
}

impl AccountManager {
    async fn seed(provider: &str, app: &App<MockRuntime>) -> (tempfile::TempDir, Self) {
        match provider {
            "codex_oauth" => {
                let (dir, manager) = crate::codex_oauth::test_support::seeded().await;
                manager.set_default_account("one").await.unwrap();
                assert!(app.manage(CodexOAuthState(manager.clone())));
                (dir, Self::Codex(manager))
            }
            "xai_oauth" => {
                let (dir, manager) = crate::xai_oauth::test_support::seeded().await;
                manager.set_default_account("one").await.unwrap();
                assert!(app.manage(XaiOAuthState(manager.clone())));
                (dir, Self::Xai(manager))
            }
            "github_copilot" => {
                let (dir, manager) = crate::copilot_auth::test_support::seeded().await;
                assert!(app.manage(CopilotAuthState(manager.clone())));
                (dir, Self::Copilot(manager))
            }
            _ => unreachable!(),
        }
    }

    async fn switch_to_second(&self) {
        match self {
            Self::Codex(manager) => manager.set_default_account("two").await.unwrap(),
            Self::Xai(manager) => manager.set_default_account("two").await.unwrap(),
            Self::Copilot(manager) => manager.set_default_account("2").await.unwrap(),
        }
    }
}

fn configure(app: &App<MockRuntime>, provider: &str, binding: Option<&str>) {
    let state = app.state::<DbState>();
    let conn = state.0.lock().unwrap();
    let raw: String = conn
        .query_row(
            "SELECT config_snapshot FROM config_profiles WHERE id='p1'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let mut value: serde_json::Value = serde_json::from_str(&raw).unwrap();
    value["metadata"]["providerType"] = json!(provider);
    value["metadata"]["authBinding"] = binding
        .map(|id| json!({"authProvider":provider,"accountId":id}))
        .unwrap_or(serde_json::Value::Null);
    // Neither profile header extension can replace the resolved managed identity.
    value["metadata"]["requestHeaders"] = json!({
        "authorization":"Bearer injected", "chatgpt-account-id":"injected"
    });
    value["metadata"]["localProxyRequestOverrides"] = json!({"headers":{
        "authorization":"Bearer injected", "chatgpt-account-id":"injected"
    }});
    conn.execute(
        "UPDATE config_profiles SET config_snapshot=?1 WHERE id='p1'",
        [value.to_string()],
    )
    .unwrap();
}

async fn recording_server() -> (Upstream, Arc<Mutex<Vec<HeaderMap>>>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let hits = Arc::new(AtomicUsize::new(0));
    let headers = Arc::new(Mutex::new(Vec::new()));
    let observed = headers.clone();
    let count = hits.clone();
    let router = Router::new().fallback(any(move |request: Request<Body>| {
        observed.lock().unwrap().push(request.headers().clone());
        count.fetch_add(1, Ordering::SeqCst);
        async {
            Response::builder()
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .unwrap()
        }
    }));
    let task = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    (Upstream { url, hits, task }, headers)
}

#[tokio::test]
async fn actual_proxy_uses_resolved_defaults_and_explicit_bindings_for_each_managed_provider() {
    for provider in ["codex_oauth", "xai_oauth", "github_copilot"] {
        let (upstream, headers) = recording_server().await;
        let app = app(&[("p1", &upstream.url, vec![])], OptimizerConfig::default());
        let (_dir, manager) = AccountManager::seed(provider, &app).await;
        configure(&app, provider, None);
        assert_eq!(
            forward(app.handle().clone(), false).await.status(),
            StatusCode::OK
        );
        manager.switch_to_second().await;
        assert_eq!(
            forward(app.handle().clone(), false).await.status(),
            StatusCode::OK
        );
        let first = if provider == "github_copilot" {
            "1"
        } else {
            "one"
        };
        let second = if provider == "github_copilot" {
            "2"
        } else {
            "two"
        };
        configure(&app, provider, Some(first));
        assert_eq!(
            forward(app.handle().clone(), false).await.status(),
            StatusCode::OK
        );
        let observed = headers.lock().unwrap();
        assert_eq!(observed.len(), 3);
        for (headers, account) in observed.iter().zip([first, second, first]) {
            assert_eq!(headers["authorization"], format!("Bearer {account}-access"));
            assert!(!headers.contains_key("x-api-key"));
            if provider == "codex_oauth" {
                assert_eq!(headers["chatgpt-account-id"], account);
            } else {
                assert!(!headers.contains_key("chatgpt-account-id"));
            }
        }
        drop(observed);
        configure(&app, provider, Some("missing"));
        assert_eq!(
            forward(app.handle().clone(), false).await.status(),
            StatusCode::BAD_GATEWAY
        );
        assert_eq!(upstream.hits.load(Ordering::SeqCst), 3);
    }
}

#[tokio::test]
async fn grok_proxy_resolves_the_actual_xai_account_without_an_api_key() {
    let (upstream, headers) = recording_server().await;
    let app = app(&[("p1", &upstream.url, vec![])], OptimizerConfig::default());
    let (_dir, manager) = AccountManager::seed("xai_oauth", &app).await;
    {
        let state = app.state::<DbState>();
        let conn = state.0.lock().unwrap();
        conn.execute(
            "UPDATE config_profiles SET tool_id='grokbuild',config_snapshot=?1 WHERE id='p1'",
            [json!({
                "baseUrl":upstream.url,"metadata":{"providerType":"xai_oauth"}
            })
            .to_string()],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO app_settings(key,value) VALUES('current_profile_grokbuild','p1')",
            [],
        )
        .unwrap();
        conn.execute(
            "UPDATE app_settings SET value=?1 WHERE key='local_provider_proxy_settings'",
            [json!({"enabled_apps":["grokbuild"],"port":34567}).to_string()],
        )
        .unwrap();
    }
    for expected in ["one-access", "two-access"] {
        let request = Request::builder()
            .method("POST")
            .uri("/proxy/grokbuild/v1/responses")
            .header("content-type", "application/json")
            .body(Body::from(
                r#"{"model":"fixture-model","input":"hi","stream":false}"#,
            ))
            .unwrap();
        let response = tokio::time::timeout(
            Duration::from_secs(5),
            forward_proxy_request_with_client(
                app.handle().clone(),
                "grokbuild".into(),
                "v1/responses".into(),
                request,
                Some(reqwest::Client::builder().no_proxy().build().unwrap()),
            ),
        )
        .await
        .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            headers.lock().unwrap().last().unwrap()["authorization"],
            format!("Bearer {expected}")
        );
        manager.switch_to_second().await;
    }
    assert_eq!(upstream.hits.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn removed_account_cannot_promote_an_endpoint_from_a_late_proxy_success() {
    let primary = server(StatusCode::SERVICE_UNAVAILABLE, "application/json", "{}").await;
    let gate = Arc::new(Notify::new());
    let alternate = controlled_server(
        StatusCode::OK,
        "application/json",
        "{}",
        Some(gate.clone()),
        false,
    )
    .await;
    let app = app(
        &[("p1", &primary.url, vec![alternate.url.clone()])],
        OptimizerConfig::default(),
    );
    let (_dir, manager) = crate::copilot_auth::test_support::seeded().await;
    assert!(app.manage(CopilotAuthState(manager.clone())));
    configure(&app, "github_copilot", Some("1"));
    let handle = app.handle().clone();
    let worker = tokio::spawn(async move { forward(handle, false).await });
    tokio::time::timeout(Duration::from_secs(3), async {
        while alternate.hits.load(Ordering::SeqCst) == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    manager.remove_account("1").await.unwrap();
    gate.notify_one();
    let response = worker.await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(!app
        .state::<LocalProviderProxyRuntime>()
        .0
        .lock()
        .unwrap()
        .preferred_base_urls
        .contains_key("p1"));
    assert_eq!(primary.hits.load(Ordering::SeqCst), 1);
    assert_eq!(alternate.hits.load(Ordering::SeqCst), 1);
    assert_eq!(
        forward(app.handle().clone(), false).await.status(),
        StatusCode::BAD_GATEWAY
    );
    assert_eq!(alternate.hits.load(Ordering::SeqCst), 1);
}
