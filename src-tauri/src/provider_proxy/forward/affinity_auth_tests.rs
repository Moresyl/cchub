use super::super::affinity_tests;
use super::*;

#[tokio::test]
async fn affinity_pins_resolved_account_through_default_switch_and_invalidates_removed_login() {
    for provider in ["codex_oauth", "xai_oauth", "github_copilot"] {
        let (upstream, headers) = recording_server().await;
        let app = app(&[("p1", &upstream.url, vec![])], OptimizerConfig::default());
        let (_dir, manager) = AccountManager::seed(provider, &app).await;
        configure(&app, provider, None);
        affinity_tests::configure(&app, "session", &["p1"]);
        for switched in [false, true] {
            if switched {
                manager.switch_to_second().await;
            }
            let response =
                affinity_tests::send(&app, Some("conversation"), json!({"messages":[]})).await;
            assert_eq!(response.status(), StatusCode::OK);
            to_bytes(response.into_body(), 65536).await.unwrap();
        }
        match manager {
            AccountManager::Codex(manager) => manager.remove_account("one").await.unwrap(),
            AccountManager::Xai(manager) => manager.remove_account("one").await.unwrap(),
            AccountManager::Copilot(manager) => manager.remove_account("1").await.unwrap(),
        }
        let response =
            affinity_tests::send(&app, Some("conversation"), json!({"messages":[]})).await;
        assert_eq!(response.status(), StatusCode::OK);
        to_bytes(response.into_body(), 65536).await.unwrap();
        let observed = headers.lock().unwrap();
        assert_eq!(observed.len(), 3);
        let (one, two) = if provider == "github_copilot" {
            ("1", "2")
        } else {
            ("one", "two")
        };
        for (headers, account) in observed.iter().zip([one, one, two]) {
            assert_eq!(headers["authorization"], format!("Bearer {account}-access"));
            if provider == "codex_oauth" {
                assert_eq!(headers["chatgpt-account-id"], account);
            }
        }
        drop(observed);
        let saved: String = app
            .state::<DbState>()
            .0
            .lock()
            .unwrap()
            .query_row(
                "SELECT config_snapshot FROM config_profiles WHERE id='p1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let saved: serde_json::Value = serde_json::from_str(&saved).unwrap();
        assert!(saved["metadata"]["authBinding"].is_null());
    }
}
