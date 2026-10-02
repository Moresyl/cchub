use super::*;
use crate::provider_proxy::{forward::transport, managed_auth::ManagedPrincipal};
use axum::http::{HeaderName, HeaderValue};

fn target() -> UpstreamTarget {
    UpstreamTarget {
        profile_id: "one".into(),
        profile_name: "one".into(),
        base_url: "https://service.invalid/v1".into(),
        use_full_url: false,
        candidate_base_urls: vec![],
        headers: vec![("authorization".into(), "Bearer secret-one".into())],
        managed_principal: None,
        affinity: None,
        request_header_overrides: vec![],
        request_body_override: None,
        claude_api_format: None,
        is_github_copilot: false,
        is_codex_oauth: false,
        cost_multiplier: 1.0,
    }
}

fn key(target: &UpstreamTarget, url: &str) -> String {
    let headers = transport::effective_headers(&[], target, &[]).unwrap();
    scope_key(target, &headers, &url::Url::parse(url).unwrap()).unwrap()
}

#[test]
fn identity_joins_profiles_tools_auth_formats_and_alternative_endpoints() {
    let mut target = target();
    let first = key(&target, "https://service.invalid/v1/messages");
    target.profile_id = "other".into();
    target.profile_name = "other".into();
    target.base_url = "https://service.invalid:443/other".into();
    target.headers = vec![("x-api-key".into(), "secret-one".into())];
    assert_eq!(
        first,
        key(&target, "https://alternative.invalid/v1/responses")
    );
    target.headers[0].1 = "secret-two".into();
    assert_ne!(first, key(&target, "https://service.invalid/v1/messages"));
    target.headers.clear();
    assert_eq!(
        first,
        key(
            &target,
            "https://service.invalid/v1/messages?key=secret-one"
        )
    );
    target.base_url = "https://unrelated.invalid/v1".into();
    assert_ne!(
        first,
        key(
            &target,
            "https://unrelated.invalid/v1/messages?key=secret-one"
        )
    );
    assert_eq!(first.len(), 64);
    assert!(!first.contains("secret"));
}

#[test]
fn managed_refresh_and_revision_changes_share_a_lane_but_different_accounts_do_not() {
    let mut target = target();
    target.managed_principal = Some(ManagedPrincipal {
        provider: AuthProvider::Codex,
        account_id: "account-one".into(),
        revision: "r1".into(),
    });
    let first = key(&target, "https://service.invalid/responses");
    target.headers[0].1 = "Bearer refreshed-secret".into();
    target.managed_principal.as_mut().unwrap().revision = "r2".into();
    assert_eq!(first, key(&target, "https://service.invalid/responses"));
    target.managed_principal.as_mut().unwrap().account_id = "account-two".into();
    assert_ne!(first, key(&target, "https://service.invalid/responses"));
    let principal = target.managed_principal.clone();
    target.request_header_overrides = vec![("x-api-key".into(), "custom-key".into())];
    let extra_credential = key(&target, "https://service.invalid/responses");
    target.managed_principal = None;
    assert_eq!(
        extra_credential,
        key(&target, "https://service.invalid/responses")
    );
    target.managed_principal = principal.clone();
    target.request_header_overrides.clear();
    let query_credential = key(
        &target,
        "https://service.invalid/responses?key=custom-query",
    );
    target.managed_principal = None;
    assert_eq!(
        query_credential,
        key(
            &target,
            "https://service.invalid/responses?key=custom-query"
        )
    );
    target.managed_principal = principal;
    target.request_header_overrides = vec![("authorization".into(), "Bearer custom-secret".into())];
    let overridden = key(&target, "https://service.invalid/responses");
    target.managed_principal = None;
    assert_eq!(
        overridden,
        key(&target, "https://service.invalid/responses")
    );
}

#[test]
fn effective_headers_replace_by_name_keep_repeated_values_and_reject_invalid_values_privately() {
    let mut target = target();
    let original = vec![
        (
            HeaderName::from_static("authorization"),
            HeaderValue::from_static("Bearer local-client"),
        ),
        (
            HeaderName::from_static("x-custom"),
            HeaderValue::from_static("one"),
        ),
        (
            HeaderName::from_static("x-custom"),
            HeaderValue::from_static("two"),
        ),
        (
            HeaderName::from_static("x-api-key"),
            HeaderValue::from_static("local-client-key"),
        ),
    ];
    target.request_header_overrides = vec![("Authorization".into(), "Bearer overridden".into())];
    let merged = transport::effective_headers(&original, &target, &[]).unwrap();
    assert_eq!(merged.get_all("authorization").iter().count(), 1);
    assert_eq!(merged["authorization"], "Bearer overridden");
    assert_eq!(merged.get_all("x-custom").iter().count(), 2);
    assert!(!merged.contains_key("x-api-key"));
    let final_headers = transport::effective_headers(
        &original,
        &target,
        &[("authorization".into(), "Bearer final".into())],
    )
    .unwrap();
    assert_eq!(final_headers["authorization"], "Bearer final");
    target.request_header_overrides[0].1 = "private-secret\ninvalid".into();
    assert_eq!(
        transport::effective_headers(&original, &target, &[]).unwrap_err(),
        "Invalid upstream header value"
    );
    target.headers.clear();
    target.request_header_overrides.clear();
    target.base_url = "https://service.invalid/v1?key=configured".into();
    let merged = transport::effective_headers(&original, &target, &[]).unwrap();
    assert!(!merged.contains_key("authorization"));
    assert!(!merged.contains_key("x-api-key"));
    assert_eq!(merged.get_all("x-custom").iter().count(), 2);
    assert_eq!(
        transport::upstream_query(Some("key=local&trace=keep"), &target).as_deref(),
        Some("trace=keep")
    );
    assert_eq!(transport::upstream_query(Some("key=local"), &target), None);
    target.base_url = "https://service.invalid/v1".into();
    assert_eq!(
        transport::upstream_query(Some("key=local&trace=keep"), &target).as_deref(),
        Some("key=local&trace=keep")
    );
    assert_eq!(
        crate::provider_proxy::build_upstream_request_url(
            "https://service.invalid/v1?key=configured#fragment",
            "v1/messages?key=local&alt=sse",
            Some("key=other&trace=a%2Fb&multi=1&multi=2"),
            false,
        )
        .unwrap(),
        "https://service.invalid/v1/messages?key=configured&alt=sse&trace=a%2Fb&multi=1&multi=2"
    );
}
