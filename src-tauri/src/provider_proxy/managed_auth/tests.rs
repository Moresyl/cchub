use super::*;

#[test]
fn credential_headers_use_the_resolved_account_and_preserve_its_revision() {
    for provider in [
        AuthProvider::Codex,
        AuthProvider::Xai,
        AuthProvider::Copilot,
    ] {
        let (headers, principal) = ManagedCredentials {
            provider,
            lease: TokenLease {
                account_id: "resolved-account".into(),
                revision: "sign-in-revision".into(),
                token: "fixture-token".into(),
            },
        }
        .into_parts();
        assert_eq!(principal.provider, provider);
        assert_eq!(principal.account_id, "resolved-account");
        assert_eq!(principal.revision, "sign-in-revision");
        assert!(headers
            .iter()
            .any(|(name, value)| name == "authorization" && value == "Bearer fixture-token"));
        let account_headers = headers
            .iter()
            .filter(|(name, _)| name == "chatgpt-account-id")
            .collect::<Vec<_>>();
        if provider == AuthProvider::Codex {
            assert_eq!(account_headers.len(), 1);
            assert_eq!(account_headers[0].1, "resolved-account");
        } else {
            assert!(account_headers.is_empty());
        }
        let debug = format!("{principal:?}");
        assert!(!debug.contains("resolved-account"));
        assert!(!debug.contains("fixture-token"));
    }
}

#[tokio::test]
async fn unavailable_managers_fail_cleanly_and_plain_api_keys_need_no_manager() {
    let app = tauri::test::mock_builder()
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .unwrap();
    for provider in ["codex_oauth", "xai_oauth", "github_copilot"] {
        assert!(resolve(
            app.handle(),
            Some(provider),
            &serde_json::json!({}),
            "Fixture"
        )
        .await
        .is_err());
        let principal = ManagedPrincipal {
            provider: AuthProvider::Codex,
            account_id: "one".into(),
            revision: "old".into(),
        };
        assert!(!principal.if_current(app.handle(), || panic!("unavailable account cannot commit")));
    }
    assert!(
        resolve(app.handle(), None, &serde_json::json!({}), "Fixture")
            .await
            .unwrap()
            .is_none()
    );
}
