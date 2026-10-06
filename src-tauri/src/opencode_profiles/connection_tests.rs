use super::connection::*;
use serde_json::json;

#[test]
fn native_body_overlays_keep_probe_prompt_model_and_budget_small() {
    let connection = from_profile(&json!({"settings":{},"body":{"temperature":0.2,"model":"wrong","stream":false,"messages":[],"max_tokens":1000000,"max_completion_tokens":1000000,"generationConfig":{"temperature":0.3,"maxOutputTokens":1000000}}})).unwrap();
    let mut body = json!({"model":"selected","stream":true,"messages":[{"role":"user","content":"OK"}],"max_tokens":16,"generationConfig":{"maxOutputTokens":16}});
    connection.apply_probe_body(&mut body);
    assert_eq!(body["model"], "selected");
    assert_eq!(body["stream"], true);
    assert_eq!(body["max_tokens"], 16);
    assert_eq!(body["generationConfig"]["maxOutputTokens"], 16);
    assert_eq!(body["generationConfig"]["temperature"], 0.3);
    assert_eq!(body["temperature"], 0.2);
    assert!(body.get("max_completion_tokens").is_none());
    assert_eq!(
        connection
            .body_override(Some(json!({"temperature":0.1})))
            .unwrap()["temperature"],
        0.1
    );
}

#[test]
fn native_model_and_variant_overlays_select_matching_credentials_and_package() {
    let profile = json!({"package":"@opencode/ai/openai-compatible","settings":{"baseURL":"https://provider.test/v1","apiKey":"provider"},"headers":{"x-custom":"provider"},"body":{"temperature":0.5},"metadata":{"nativeFormat":"providers","nativeProviderId":"local","nativeModelId":"alias#fast"},"models":{"first":{"settings":{"apiKey":"unrelated"}},"alias":{"modelID":"vendor/model","package":"@opencode/ai/anthropic-compatible","settings":{"baseURL":"https://model.test/v1"},"headers":{"x-custom":"model"},"variants":[{"id":"fast","settings":{"apiKey":"selected"},"headers":{"x-custom":"variant"},"body":{"temperature":0.1}}]}}});
    let connection = from_profile(&profile).unwrap();
    assert_eq!(connection.model, "vendor/model");
    assert_eq!(connection.base_url().unwrap(), "https://model.test/v1");
    assert_eq!(connection.text("apiKey").unwrap(), "selected");
    assert_eq!(connection.body, json!({"temperature":0.1}));
    assert_eq!(
        connection.auth_headers().unwrap(),
        vec![
            ("x-api-key".into(), "selected".into()),
            ("anthropic-version".into(), "2023-06-01".into()),
            ("x-custom".into(), "variant".into())
        ]
    );
}

#[test]
fn builtin_native_and_legacy_profiles_use_the_correct_protocol_defaults() {
    for (id, base, responses) in [
        ("anthropic", "https://api.anthropic.com", false),
        (
            "google",
            "https://generativelanguage.googleapis.com/v1beta",
            false,
        ),
        ("openai", "https://api.openai.com/v1", true),
    ] {
        let profile = json!({"metadata":{"nativeFormat":"providers","nativeProviderId":id},"settings":{"apiKey":"fixture"}});
        let connection = from_profile(&profile).unwrap();
        assert_eq!(connection.default_base_url(), base);
        assert_eq!(connection.responses(), responses);
        assert!(connection.auth_headers().is_ok());
    }
    let legacy = from_profile(&json!({"npm":"@ai-sdk/openai-compatible","options":{"baseURL":"https://legacy.test","apiKey":"legacy"}})).unwrap();
    assert_eq!(legacy.base_url().unwrap(), "https://legacy.test");
    assert!(!legacy.responses());
    assert_eq!(
        legacy.auth_headers().unwrap(),
        vec![("authorization".into(), "Bearer legacy".into())]
    );
}

#[test]
fn native_headers_can_authenticate_without_a_key_but_transport_or_injection_headers_are_rejected() {
    let profile = json!({"settings":{},"headers":{"Authorization":"Bearer fixture"}});
    assert!(from_profile(&profile).unwrap().auth_headers().is_ok());
    for header in [
        json!({"host":"secret-header-sentinel"}),
        json!({"x-custom":"bad\r\nvalue"}),
        json!({"bad name":"value"}),
    ] {
        let profile = json!({"settings":{"apiKey":"fixture"},"headers":header});
        let error = from_profile(&profile).unwrap().auth_headers().unwrap_err();
        assert!(!error.contains("fixture"));
        assert!(!error.contains("secret-header-sentinel"));
    }
}

#[test]
fn credential_status_matches_effective_native_priority_and_selected_provider() {
    for document in [
        json!({"provider":{"same":{"options":{"apiKey":"legacy"}}},"providers":{"same":{}}}),
        json!({"providers":{"other":{"settings":{"apiKey":"unrelated"}},"selected":{}},"model":"selected/m"}),
        json!({"settings":{"apiKey":"{env:CCHUB_NATIVE_MISSING_CREDENTIAL_TEST}"}}),
        json!({"settings":{"apiKey":"{file:unread}"}}),
    ] {
        assert!(!has_credentials(&document));
    }
    assert!(has_credentials(
        &json!({"provider":{"same":{"options":{"apiKey":"legacy"}}},"providers":{"same":{"settings":false}}})
    ));
    assert!(has_credentials(
        &json!({"providers":{"a":{},"b":{"settings":{"apiKey":"valid"}}}})
    ));
}
