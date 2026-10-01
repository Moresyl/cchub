use super::*;
use serde_json::json;

fn aliases(rows: Value) -> Result<ModelAliases, String> {
    ModelAliases::from_snapshot(&json!({"metadata":{"localProxyModelAliases":rows}}).to_string())
}

#[test]
fn exact_alias_overrides_provider_default_and_replaces_every_star() {
    let rules = aliases(json!([
        {"model":" * ","upstream":" relay/* "},
        {"model":" core ","upstream":"vendor/*/pro-*"}
    ]))
    .unwrap();
    assert_eq!(
        rules.resolve("core").unwrap().as_deref(),
        Some("vendor/core/pro-core")
    );
    assert_eq!(
        rules.resolve("other").unwrap().as_deref(),
        Some("relay/other")
    );
    assert_eq!(
        rules.resolve("Core").unwrap().as_deref(),
        Some("relay/Core")
    );
}

#[test]
fn absent_removed_and_identity_aliases_do_not_modify_request_bytes() {
    for snapshot in [
        "native = true",
        "{}",
        r#"{"metadata":{"localProxyModelAliases":[]}}"#,
        r#"{"metadata":{"localProxyModelAliases":[{"model":"core","upstream":"*"}]}}"#,
    ] {
        let rules = ModelAliases::from_snapshot(snapshot).unwrap();
        let body = Bytes::from_static(b"{ \"model\": \"core\", \"messages\": [] }");
        let insights = super::super::extract_request_insights("claude", "v1/messages", &body);
        let (path, result, insights) = rules
            .apply("v1/messages".into(), body.clone(), insights)
            .unwrap();
        assert_eq!(result, body);
        assert_eq!(path, "v1/messages");
        assert!(insights.upstream_model.is_none());
    }
}

#[test]
fn malformed_partial_duplicate_and_unsafe_names_fail_with_sanitized_errors() {
    for rows in [
        json!(null),
        json!({"secret":"must-not-leak"}),
        json!([null]),
        json!([{"model":"core"}]),
        json!([{"model":"core","upstream":""}]),
        json!([{"model":"core*","upstream":"relay"}]),
        json!([{"model":"core","upstream":"secret?must-not-leak"}]),
        json!([{"model":"core","upstream":"a"},{"model":" core ","upstream":"b"}]),
        json!([{"model":"core","upstream":"a\nb"}]),
        json!([{"model":"core","upstream":"x".repeat(1025)}]),
        json!([{"model":7,"upstream":"a"}]),
    ] {
        let error = aliases(rows).unwrap_err();
        assert!(!error.contains("must-not-leak"));
    }
    assert!(aliases(json!(vec![json!({"model":"","upstream":""}); 129])).is_err());
    assert!(aliases(json!([{"model":"","upstream":""}]))
        .unwrap()
        .0
        .is_empty());
}

#[test]
fn expanded_alias_and_unicode_limits_are_bounded_before_allocation() {
    let rules = aliases(json!([{"model":"*","upstream":"*".repeat(1024)}])).unwrap();
    assert!(rules.resolve("ab").is_err());
    assert!(aliases(json!([{"model":"x","upstream":"中".repeat(342)}])).is_err());
    let rules = aliases(json!([{"model":"*","upstream":"中/*"}])).unwrap();
    assert_eq!(rules.resolve("core").unwrap().as_deref(), Some("中/core"));
}

#[test]
fn gemini_path_alias_is_encoded_as_one_component_and_preserves_action() {
    let rules = aliases(json!([{"model":"core","upstream":"vendor/中:model"}])).unwrap();
    for action in ["generateContent", "streamGenerateContent"] {
        let path = format!("v1beta/models/core:{action}");
        let body = Bytes::from_static(b"{\"contents\":[]}");
        let insights = super::super::extract_request_insights("claude", &path, &body);
        let (path, body, insights) = rules.apply(path, body, insights).unwrap();
        assert_eq!(
            path,
            format!("v1beta/models/vendor%2F%E4%B8%AD%3Amodel:{action}")
        );
        assert!(serde_json::from_slice::<Value>(&body)
            .unwrap()
            .get("model")
            .is_none());
        assert_eq!(insights.request_model.as_deref(), Some("core"));
        assert_eq!(insights.sent_model(), Some("vendor/中:model"));
    }
}

#[test]
fn expected_wire_model_uses_canonical_price_but_different_served_model_keeps_its_price() {
    let insights = ProxyRequestInsights {
        request_model: Some("core".into()),
        upstream_model: Some("wire".into()),
        is_streaming: false,
    };
    assert_eq!(insights.pricing_model(None), Some("core"));
    assert_eq!(insights.pricing_model(Some("wire")), Some("core"));
    assert_eq!(insights.pricing_model(Some("other")), Some("other"));
    let plain = ProxyRequestInsights {
        upstream_model: None,
        ..insights
    };
    assert_eq!(plain.pricing_model(Some("wire")), Some("wire"));
}

#[test]
fn gemini_stream_query_is_preserved_without_duplicate_delimiters_or_keys() {
    let url = super::super::upstream::build_upstream_request_url(
        "https://example.com/v1beta",
        "v1beta/models/core:streamGenerateContent?alt=sse",
        Some("alt=sse&trace=a%2Fb&multi=1&multi=2"),
        false,
    )
    .unwrap();
    assert_eq!(url, "https://example.com/v1beta/models/core:streamGenerateContent?alt=sse&trace=a%2Fb&multi=1&multi=2");
    let rules = aliases(json!([{"model":"core","upstream":"vendor/*"}])).unwrap();
    let body = Bytes::from_static(b"{\"contents\":[]}");
    let path = "v1beta/models/core:streamGenerateContent?alt=sse";
    let insights = super::super::extract_request_insights("gemini", path, &body);
    let (path, _, insights) = rules.apply(path.into(), body, insights).unwrap();
    assert_eq!(
        path,
        "v1beta/models/vendor%2Fcore:streamGenerateContent?alt=sse"
    );
    let base = alias_base_url(
        "https://example.com/v1beta/models/old:streamGenerateContent?alt=sse&token=a:b",
        true,
        &path,
        &insights,
    )
    .unwrap();
    assert_eq!(
        base,
        "https://example.com/v1beta/models/vendor%2Fcore:streamGenerateContent?alt=sse&token=a:b"
    );
    assert!(alias_base_url("https://example.com/custom", true, &path, &insights).is_err());
}

#[test]
fn encoded_gemini_request_model_is_decoded_for_alias_matching() {
    let body = Bytes::from_static(b"{\"contents\":[]}");
    let insights = super::super::extract_request_insights(
        "gemini",
        "v1beta/models/vendor%2F%E4%B8%AD%3Amodel:generateContent",
        &body,
    );
    assert_eq!(insights.request_model.as_deref(), Some("vendor/中:model"));
    let rules = aliases(json!([{"model":"vendor/中:model","upstream":"replacement"}])).unwrap();
    let (path, _, _) = rules
        .apply(
            "v1beta/models/vendor%2F%E4%B8%AD%3Amodel:generateContent".into(),
            body,
            insights,
        )
        .unwrap();
    assert_eq!(path, "v1beta/models/replacement:generateContent");
}
