use super::*;
use serde_json::{json, Value};

fn target() -> UpstreamTarget {
    UpstreamTarget {
        profile_id: "p1".into(),
        profile_name: "fixture".into(),
        base_url: "https://api.mistral.ai/v1".into(),
        use_full_url: false,
        candidate_base_urls: vec![],
        headers: vec![],
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

fn recovery(runtime: Arc<Mutex<LocalProviderProxyRuntimeInner>>, url: &str) -> Recovery {
    Recovery::new(
        runtime,
        "claude",
        &target(),
        "snapshot",
        "v1/chat/completions",
        url,
        Some("fixture"),
        true,
    )
}

fn fault(index: usize, field: &str) -> Bytes {
    Bytes::from(
        json!({"detail":[{"type":"extra_forbidden","loc":["body","messages",index,field]}]})
            .to_string(),
    )
}

#[test]
fn mistral_thinking_preserves_tools_content_and_opaque_number_spellings() {
    let body = Bytes::from_static(br#" {"opaque" : 9007199254740993123456789, "model":"m", "messages" : [ {"role":"user", "content":"hi"}, {"role":"assistant", "reasoning_content":"think", "content":[{"type":"text","text":"answer","number":1.2300e+44}], "tool_calls" : [{"id":"call_1", "function":{"name":"read", "arguments":"{}"}}], "signature":{"raw":1.00000000} }, {"role":"tool", "tool_call_id":"call_1", "content":"ok"} ], "temperature":0.10000000000000000001 } "#);
    let next = recovery(Arc::default(), "https://api.mistral.ai/v1/chat/completions")
        .prepare(body.clone());
    let value: Value = serde_json::from_slice(&next).unwrap();
    assert_eq!(
        value["messages"][1]["content"][0]["thinking"][0]["text"],
        "think"
    );
    assert_eq!(value["messages"][1]["content"][1]["text"], "answer");
    assert_eq!(value["messages"][1]["tool_calls"][0]["id"], "call_1");
    assert!(value["messages"][1].get("reasoning_content").is_none());
    let wire = std::str::from_utf8(&next).unwrap();
    for original in [
        "9007199254740993123456789",
        "1.2300e+44",
        "1.00000000",
        "0.10000000000000000001",
        r#" {"role":"user", "content":"hi"}"#,
        r#" {"role":"tool", "tool_call_id":"call_1", "content":"ok"}"#,
    ] {
        assert!(wire.contains(original), "{original}: {wire}");
    }
    let second = recovery(Arc::default(), "https://api.mistral.ai/v1/chat/completions")
        .prepare(next.clone());
    assert_eq!(second, next);
}

#[test]
fn conversion_handles_aliases_null_tool_content_and_plain_details() {
    for message in [
        json!({"role":"assistant","reasoning":"先想🦀","content":null,"tool_calls":[{"id":"c"}]}),
        json!({"role":"assistant","reasoning_details":[{"type":"reasoning.text","text":"先想🦀"}],"tool_calls":[{"id":"c"}]}),
        json!({"role":"assistant","reasoning_content":"先想🦀","reasoning":"先想🦀","content":"answer","tool_calls":[{"id":"c"}]}),
    ] {
        let body = Bytes::from(json!({"messages":[message]}).to_string());
        let next =
            recovery(Arc::default(), "https://api.mistral.ai/v1/chat/completions").prepare(body);
        let value: Value = serde_json::from_slice(&next).unwrap();
        assert_eq!(
            value["messages"][0]["content"][0]["thinking"][0]["text"],
            "先想🦀"
        );
        assert_eq!(value["messages"][0]["tool_calls"][0]["id"], "c");
    }
}

#[test]
fn unqualified_hosts_paths_methods_and_opaque_history_are_unchanged() {
    let body = Bytes::from_static(
        br#"{"messages":[{"role":"assistant","reasoning":"think","content":null}]}"#,
    );
    for url in [
        "https://api.mistral.ai.evil.test/v1/chat/completions",
        "https://api.mistral.ai@evil.test/v1/chat/completions",
        "https://deepseek.test/v1/chat/completions",
    ] {
        assert_eq!(recovery(Arc::default(), url).prepare(body.clone()), body);
    }
    for path in ["v1/responses", "v1/messages", "foo/chat/completions"] {
        let handler = Recovery::new(
            Arc::default(),
            "claude",
            &target(),
            "s",
            path,
            "https://api.mistral.ai/v1",
            None,
            true,
        );
        assert_eq!(handler.prepare(body.clone()), body);
    }
    let handler = Recovery::new(
        Arc::default(),
        "claude",
        &target(),
        "s",
        "v1/chat/completions",
        "https://api.mistral.ai/v1",
        None,
        false,
    );
    assert_eq!(handler.prepare(body.clone()), body);
    for source in [
        r#"{"messages":[{"role":"assistant","reasoning_details":[{"type":"reasoning.encrypted","data":"opaque"}],"content":null}]}"#,
        r#"{"messages":[{"role":"assistant","reasoning":"t","content":{"opaque":1}}]}"#,
        r#"{"messages":[{"role":"assistant","role":"user","reasoning":"t"}]}"#,
        r#"{"messages":[],"messages":[{"role":"assistant","reasoning":"t"}]}"#,
        r#"{"messages":[{"role":"tool","reasoning":"t","content":"result"}]}"#,
        "broken JSON",
    ] {
        let body = Bytes::copy_from_slice(source.as_bytes());
        assert_eq!(
            recovery(Arc::default(), "https://api.mistral.ai/v1").prepare(body.clone()),
            body
        );
    }
}

#[test]
fn explicit_refusal_repairs_only_the_named_assistant_alias_and_learns_after_success() {
    let runtime: Arc<Mutex<LocalProviderProxyRuntimeInner>> = Arc::default();
    let url = "https://vendor.test/v1/chat/completions";
    let body = Bytes::from_static(br#"{"reasoning":"top-level", "messages":[{"role":"assistant","reasoning_content":"t", "reasoning":"keep", "content":null, "tool_calls":[{"id":"c"}]},{"role":"tool","reasoning_content":"tool-data","tool_call_id":"c"}]}"#);
    let mut handler = recovery(runtime.clone(), url);
    let next = handler
        .retry(
            StatusCode::UNPROCESSABLE_ENTITY,
            &fault(0, "reasoning_content"),
            &body,
        )
        .unwrap();
    let value: Value = serde_json::from_slice(&next).unwrap();
    assert_eq!(value["messages"][0]["reasoning"], "keep");
    assert_eq!(value["messages"][1]["reasoning_content"], "tool-data");
    assert_eq!(value["reasoning"], "top-level");
    assert!(handler
        .retry(
            StatusCode::BAD_REQUEST,
            &fault(0, "reasoning_content"),
            &next
        )
        .is_none());
    assert_eq!(recovery(runtime.clone(), url).prepare(body.clone()), body);
    handler.success();
    assert_eq!(recovery(runtime.clone(), url).prepare(body.clone()), next);
    assert_eq!(
        recovery(
            runtime.clone(),
            "https://alternate.test/v1/chat/completions"
        )
        .prepare(body.clone()),
        body
    );
    for (snapshot, model, profile) in [
        ("updated", "fixture", "p1"),
        ("snapshot", "other", "p1"),
        ("snapshot", "fixture", "p2"),
    ] {
        let mut target = target();
        target.profile_id = profile.into();
        assert_eq!(
            Recovery::new(
                runtime.clone(),
                "claude",
                &target,
                snapshot,
                "v1/chat/completions",
                url,
                Some(model),
                true
            )
            .prepare(body.clone()),
            body
        );
    }
}

#[test]
fn generic_errors_nested_input_bad_indices_and_non_history_fields_cannot_trigger_retry() {
    let body = Bytes::from_static(br#"{"messages":[{"role":"assistant","reasoning_content":"t"},{"role":"tool","reasoning":"t"}]}"#);
    for error in [
        json!({"error":{"message":"reasoning_content is not supported"}}),
        json!({"detail":[{"type":"extra_forbidden","loc":["body","reasoning_content"]}]}),
        json!({"detail":[{"type":"extra_forbidden","loc":["messages",0,"content","reasoning_content"]}]}),
        json!({"detail":[{"type":"extra_forbidden","loc":["messages",0,"tool_calls"]}]}),
        json!({"detail":[{"type":"extra_forbidden","loc":["messages",999,"reasoning_content"]}]}),
        json!({"detail":[{"type":"extra_forbidden","loc":["messages",1,"reasoning"]}]}),
        json!({"input":{"type":"extra_forbidden","loc":["messages",0,"reasoning_content"]}}),
        json!({"detail":[{"type":"missing","loc":["messages",0,"reasoning_content"]}]}),
    ] {
        let mut handler = recovery(Arc::default(), "https://vendor.test/v1");
        assert!(
            handler
                .retry(
                    StatusCode::BAD_REQUEST,
                    &Bytes::from(error.to_string()),
                    &body
                )
                .is_none(),
            "{error}"
        );
    }
    for status in [
        StatusCode::OK,
        StatusCode::UNAUTHORIZED,
        StatusCode::TOO_MANY_REQUESTS,
        StatusCode::INTERNAL_SERVER_ERROR,
    ] {
        assert!(recovery(Arc::default(), "https://vendor.test/v1")
            .retry(status, &fault(0, "reasoning_content"), &body)
            .is_none());
    }
}

#[test]
fn policy_store_expires_and_remains_bounded() {
    let now = Instant::now();
    let mut store = Store::default();
    store.learn("one".into(), 1, now);
    store.learn("one".into(), 2, now);
    assert_eq!(store.get("one", now), 3);
    assert_eq!(store.get("one", now + TTL), 0);
    for index in 0..256 {
        store.learn(index.to_string(), 1, now + Duration::from_millis(index));
    }
    assert_eq!(store.0.len(), MAX_POLICIES);
    assert_eq!(store.get("0", now), 0);
    assert_eq!(store.get("255", now), 1);
}

#[test]
fn managed_account_and_login_revisions_isolate_learned_policies() {
    use super::super::managed_auth::{AuthProvider, ManagedPrincipal};
    let runtime: Arc<Mutex<LocalProviderProxyRuntimeInner>> = Arc::default();
    let mut target = target();
    target.managed_principal = Some(ManagedPrincipal {
        provider: AuthProvider::Xai,
        account_id: "account-a".into(),
        revision: "login-a".into(),
    });
    let url = "https://vendor.test/v1/chat/completions";
    let make = |target: &UpstreamTarget, tool: &str| {
        Recovery::new(
            runtime.clone(),
            tool,
            target,
            "snapshot",
            "v1/chat/completions",
            url,
            Some("fixture"),
            true,
        )
    };
    let body = Bytes::from_static(
        br#"{"messages":[{"role":"assistant","reasoning":"t","content":null}]}"#,
    );
    let mut handler = make(&target, "claude");
    let next = handler
        .retry(StatusCode::BAD_REQUEST, &fault(0, "reasoning"), &body)
        .unwrap();
    handler.success();
    assert_eq!(make(&target, "claude").prepare(body.clone()), next);
    assert_eq!(make(&target, "codex").prepare(body.clone()), body);
    target.managed_principal.as_mut().unwrap().revision = "login-b".into();
    assert_eq!(make(&target, "claude").prepare(body.clone()), body);
    target.managed_principal.as_mut().unwrap().account_id = "account-b".into();
    assert_eq!(make(&target, "claude").prepare(body.clone()), body);
}

#[test]
fn selective_retry_preserves_numbers_outside_value_range_and_accepts_batched_errors() {
    let body = Bytes::from_static(br#"{"messages":[{"role":"assistant","reasoning_content":"a","reasoning":"b","content":null,"opaque":1.23000e+999}],"opaque":9007199254740993123456789}"#);
    let error = Bytes::from(
        json!({"error":{"details":[
            {"type":"extra_forbidden","loc":["messages",0,"reasoning_content"]},
            {"type":"extra_forbidden","loc":["messages",0,"reasoning"]}
        ]}})
        .to_string(),
    );
    let mut handler = recovery(Arc::default(), "https://vendor.test/v1");
    let next = handler
        .retry(StatusCode::BAD_REQUEST, &error, &body)
        .unwrap();
    let wire = std::str::from_utf8(&next).unwrap();
    assert!(wire.contains("1.23000e+999") && wire.contains("9007199254740993123456789"));
    assert!(!wire.contains("reasoning_content") && !wire.contains("\"reasoning\""));
    assert!(handler
        .retry(StatusCode::BAD_REQUEST, &error, &next)
        .is_none());
}

#[test]
fn empty_reasoning_keeps_original_null_content_and_non_reply_json_cannot_be_learned() {
    let body = Bytes::from_static(br#"{"messages":[{"role":"assistant","reasoning_content":null,"reasoning":"","content":null,"tool_calls":[{"id":"c"}]}]}"#);
    let next = recovery(Arc::default(), "https://api.mistral.ai/v1").prepare(body);
    let value: Value = serde_json::from_slice(&next).unwrap();
    assert_eq!(value["messages"][0]["content"], Value::Null);
    assert_eq!(value["messages"][0]["tool_calls"][0]["id"], "c");
    for value in [
        json!({}),
        json!(null),
        json!({"choices":[]}),
        json!({"choices":[{"message":{"role":"user"},"finish_reason":"stop"}]}),
        json!({"choices":[{"message":{"role":"assistant"},"finish_reason":null}]}),
    ] {
        assert!(!is_chat_reply(&value));
    }
    assert!(is_chat_reply(
        &json!({"choices":[{"message":{"role":"assistant","content":null,"tool_calls":[{"id":"c"}]},"finish_reason":"tool_calls"}]})
    ));
}
