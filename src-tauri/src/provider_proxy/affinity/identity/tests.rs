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
