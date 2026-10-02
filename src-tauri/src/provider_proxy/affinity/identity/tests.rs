use super::*;
use serde_json::json;

fn key(tool: &str, headers: &[(&str, &str)], body: Value) -> Option<String> {
    let headers = headers
        .iter()
        .map(|(name, value)| (name.parse().unwrap(), value.parse().unwrap()))
        .collect::<Vec<_>>();
    request_key(
        tool,
        "v1/messages",
        "policy",
        Some("group"),
        &headers,
        &body,
    )
}

#[test]
fn only_explicit_bounded_client_sessions_can_create_affinity() {
    let body = json!({"model":"m","messages":[{"role":"user","content":"identical prompt"}]});
    assert!(key("claude", &[], body.clone()).is_none());
    assert!(key("claude", &[], json!({"previous_response_id":"resp_cursor"})).is_none());
    assert!(key("claude", &[("x-session-id", " ")], body.clone()).is_none());
    assert!(key(
        "claude",
        &[("x-session-id", "a"), ("x-session-id", "b")],
        body.clone()
    )
    .is_none());
    assert!(key("claude", &[("x-session-id", &"x".repeat(513))], body).is_none());
    for body in [
        json!({"metadata":{"session_id":"explicit"}}),
        json!({"metadata":{"user_id":"user_account_session_explicit"}}),
        json!({"metadata":{"user_id":"{\"session_id\":\"explicit\"}"}}),
    ] {
        assert_eq!(
            key("claude", &[], body),
            key("claude", &[("x-session-id", "explicit")], json!({}))
        );
    }
    assert!(key(
        "claude",
        &[],
        json!({"metadata":{"user_id":"ordinary-user-id"}})
    )
    .is_none());
}

#[test]
fn scopes_partition_tools_credentials_models_effort_and_policy_without_retaining_secrets() {
    let baseline = key(
        "claude",
        &[
            ("x-session-id", "session-secret"),
            ("authorization", "credential-secret"),
        ],
        json!({"model":"m"}),
    )
    .unwrap();
    assert_eq!(baseline.len(), 64);
    assert!(!baseline.contains("secret"));
    for next in [
        key(
            "codex",
            &[
                ("x-session-id", "session-secret"),
                ("authorization", "credential-secret"),
            ],
            json!({"model":"m"}),
        ),
        key(
            "claude",
            &[
                ("x-session-id", "different"),
                ("authorization", "credential-secret"),
            ],
            json!({"model":"m"}),
        ),
        key(
            "claude",
            &[
                ("x-session-id", "session-secret"),
                ("authorization", "other"),
            ],
            json!({"model":"m"}),
        ),
        key(
            "claude",
            &[
                ("x-session-id", "session-secret"),
                ("x-api-key", "credential-secret"),
            ],
            json!({"model":"m"}),
        ),
        key(
            "claude",
            &[
                ("x-session-id", "session-secret"),
                ("authorization", "credential-secret"),
            ],
            json!({"model":"n"}),
        ),
        key(
            "claude",
            &[
                ("x-session-id", "session-secret"),
                ("authorization", "credential-secret"),
            ],
            json!({"model":"m","thinking":{"type":"adaptive"}}),
        ),
    ] {
        assert_ne!(Some(&baseline), next.as_ref());
    }
    let headers = vec![
        (
            "x-session-id".parse().unwrap(),
            "session-secret".parse().unwrap(),
        ),
        (
            "authorization".parse().unwrap(),
            "credential-secret".parse().unwrap(),
        ),
    ];
    for (policy, group) in [("other", Some("group")), ("policy", Some("other"))] {
        assert_ne!(
            Some(baseline.clone()),
            request_key(
                "claude",
                "v1/messages",
                policy,
                group,
                &headers,
                &json!({"model":"m"})
            )
        );
    }
}

#[test]
fn tool_results_keep_the_turn_while_new_user_prompts_change_it() {
    let prompt = json!({"role":"user","content":"hello"});
    let initial = turn(&json!({"messages":[prompt]}));
    assert!(!initial.within);
    for tool in [
        json!({"role":"user","content":[{"type":"tool_result","tool_use_id":"call"}]}),
        json!({"role":"tool","content":"result"}),
    ] {
        let next = turn(
            &json!({"messages":[prompt, {"role":"assistant","content":[]}, tool, {"role":"system","content":"notes"}]}),
        );
        assert!(next.within);
        assert_eq!(next.marker, initial.marker);
    }
    let new_turn = turn(&json!({"messages":[prompt,prompt]}));
    assert!(!new_turn.within);
    assert_ne!(new_turn.marker, initial.marker);
    assert!(
        turn(&json!({"input":[{"type":"function_call_output","call_id":"call","output":"ok"}]}))
            .within
    );
    assert!(
        turn(&json!({"contents":[{"role":"user","parts":[{"functionResponse":{"name":"run"}}]}]}))
            .within
    );
}

#[test]
fn native_results_link_partial_histories_but_notifications_begin_another_turn() {
    let prompt =
        json!({"type":"message","role":"user","content":[{"type":"input_text","text":"hello"}]});
    let initial = turn(&json!({"input":[prompt]}));
    for kind in [
        "function_call_output",
        "custom_tool_call_output",
        "tool_search_output",
    ] {
        let result = json!({"type":kind,"call_id":"call","output":"ok","tools":[]});
        let continued = turn(&json!({"input":[prompt, result]}));
        assert!(continued.within, "{kind}");
        assert_eq!(continued.marker, initial.marker);
        let partial = turn(&json!({"previous_response_id":"resp_old","input":[result]}));
        assert!(partial.within, "{kind}");
        assert_eq!(partial.marker, None);
        let next = turn(&json!({"input":[prompt,result,prompt]}));
        assert!(!next.within);
        assert_ne!(next.marker, initial.marker);
    }
    for kind in ["function_call_output", "custom_tool_call_output"] {
        for call_id in [
            Value::Null,
            json!(""),
            json!("  "),
            json!(12),
            json!("bad\ncall"),
        ] {
            let notice = json!({"type":kind,"call_id":call_id,"output":"independent notification"});
            let next = turn(&json!({"input":[prompt,
                {"type":"custom_tool_call_output","call_id":"call","output":"paired"},notice]}));
            assert!(!next.within, "{notice}");
            assert_ne!(next.marker, initial.marker);
            assert!(next.marker.is_some());
        }
        let next = turn(&json!({"input":[{"type":kind,"output":"notice"}]}));
        assert!(!next.within);
        assert!(next.marker.is_some());
    }
    // Hosted search is internal history, not a client result or a notification.
    let hosted =
        json!({"type":"tool_search_output","execution":"server","call_id":null,"tools":[]});
    assert_eq!(turn(&json!({"input":[prompt,hosted]})), initial);
    assert_eq!(turn(&json!({"input":[hosted]})), Turn::default());
}
