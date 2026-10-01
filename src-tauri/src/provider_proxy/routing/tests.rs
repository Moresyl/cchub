use super::*;
use rusqlite::Connection;
use serde_json::json;

fn group(id: &str, members: &[&str]) -> RoutingGroup {
    RoutingGroup {
        id: id.into(),
        name: id.into(),
        mode: RoutingMode::Ordered,
        members: members
            .iter()
            .map(|id| RoutingMember::Profile {
                profile_id: (*id).into(),
            })
            .collect(),
        picked_profile_id: None,
    }
}

fn policy() -> RoutingPolicy {
    RoutingPolicy {
        enabled: true,
        affinity: AffinityMode::Off,
        default_group_id: Some("g".into()),
        groups: vec![group("g", &["p2", "p1"])],
        rules: vec![],
    }
}

fn resolve(policy: RoutingPolicy, body: serde_json::Value) -> Result<RoutingPreview, String> {
    plan::resolve(
        &RoutingDocument {
            revision: None,
            policy,
        },
        &serde_json::to_vec(&body).unwrap(),
        "",
        &["p1".into(), "p2".into(), "p3".into()],
        None,
        |_, _| 0,
    )
}

#[test]
fn disabled_and_unmatched_rules_keep_existing_order() {
    let mut config = policy();
    config.enabled = false;
    assert_eq!(
        resolve(config, json!({})).unwrap().profile_ids,
        ["p1", "p2", "p3"]
    );
    let mut config = policy();
    config.default_group_id = None;
    config.rules.push(RoutingRule {
        id: "r".into(),
        name: "r".into(),
        group_id: "g".into(),
        model: "chosen".into(),
        ..Default::default()
    });
    assert_eq!(
        resolve(config, json!({"model":"other"})).unwrap().reason,
        "activeProfile"
    );
}

#[test]
fn first_rule_requires_all_conditions_and_ignores_tool_argument_images() {
    let mut config = policy();
    config.groups.push(group("fast", &["p3"]));
    config.rules = vec![RoutingRule {
        id: "r".into(),
        name: "r".into(),
        group_id: "fast".into(),
        model: "model-".into(),
        match_mode: ModelMatch::Prefix,
        images: true,
        thinking: true,
        min_request_bytes: 1,
    }];
    let request = json!({"model":"model-a","thinking":{"type":"adaptive"},"messages":[{"role":"user","content":[{"type":"image","source":{}}]}]});
    let result = resolve(config.clone(), request.clone()).unwrap();
    assert_eq!(result.rule_id.as_deref(), Some("r"));
    assert_eq!(result.profile_ids, ["p3"]);
    for request in [
        json!({"model":"model-a","thinking":{"type":"disabled"},"messages":[{"content":[{"type":"image"}]}]}),
        json!({"model":"other","thinking":{"type":"adaptive"},"messages":[{"content":[{"type":"image"}]}]}),
        json!({"model":"model-a","thinking":{"type":"adaptive"},"messages":[{"content":[{"type":"tool_use","input":{"type":"image"}}]}]}),
    ] {
        assert_eq!(
            resolve(config.clone(), request).unwrap().reason,
            "defaultGroup"
        );
    }
    config.rules.push(RoutingRule {
        id: "second".into(),
        name: "second".into(),
        group_id: "g".into(),
        images: true,
        ..Default::default()
    });
    assert_eq!(
        resolve(config, request).unwrap().rule_id.as_deref(),
        Some("r")
    );
}

#[test]
fn reasoning_effort_recognizes_minimal_and_skips_non_string_aliases() {
    let rule = RoutingRule {
        thinking: true,
        ..Default::default()
    };
    for request in [
        json!({"reasoning_effort":"minimal"}),
        json!({"reasoning_effort":null,"reasoning":{"effort":"low"}}),
        json!({"reasoning":{"effort":5},"output_config":{"effort":"high"}}),
    ] {
        let bytes = serde_json::to_vec(&request).unwrap();
        assert!(facts::Facts::from_body(&bytes, "").matches(&rule));
    }
    for request in [
        json!({}),
        json!({"reasoning_effort":"none"}),
        json!({"reasoning":{"effort":""}}),
    ] {
        let bytes = serde_json::to_vec(&request).unwrap();
        assert!(!facts::Facts::from_body(&bytes, "").matches(&rule));
    }
}

#[test]
fn gemini_levels_match_reasoning_without_requiring_an_integer_budget() {
    let rule = RoutingRule {
        thinking: true,
        ..Default::default()
    };
    for level in ["minimal", "low", "medium", "high", " HIGH "] {
        let bytes = serde_json::to_vec(
            &json!({"generationConfig":{"thinkingConfig":{"thinkingLevel":level}}}),
        )
        .unwrap();
        assert!(facts::Facts::from_body(&bytes, "").matches(&rule));
    }
    for config in [
        json!({"thinkingLevel":"THINKING_LEVEL_UNSPECIFIED"}),
        json!({"thinkingLevel":"unknown"}),
        json!({"thinkingLevel":null,"thinkingBudget":0}),
        json!({"includeThoughts":true}),
    ] {
        let bytes =
            serde_json::to_vec(&json!({"generationConfig":{"thinkingConfig":config}})).unwrap();
        assert!(!facts::Facts::from_body(&bytes, "").matches(&rule));
    }
}

#[test]
fn native_gemini_path_images_and_thinking_are_recognized() {
    let body = serde_json::to_vec(
        &json!({"contents":[{"parts":[{"inlineData":{"mimeType":"image/png","data":"fixture"}}]}],
        "generationConfig":{"thinkingConfig":{"thinkingBudget":-1}}}),
    )
    .unwrap();
    let facts = facts::Facts::from_body(&body, "v1beta/models/model-a:streamGenerateContent");
    assert!(facts.matches(&RoutingRule {
        model: "model-a".into(),
        images: true,
        thinking: true,
        ..Default::default()
    }));
    assert!(!facts.matches(&RoutingRule {
        model: "model-A".into(),
        ..Default::default()
    }));
}

#[test]
fn nested_groups_preserve_child_order_and_deduplicate_profiles() {
    let mut config = policy();
    config.groups[0].members = vec![
        RoutingMember::Group {
            group_id: "child".into(),
        },
        RoutingMember::Profile {
            profile_id: "p2".into(),
        },
    ];
    config.groups.push(group("child", &["p3", "p2", "p1"]));
    assert_eq!(
        resolve(config.clone(), json!({})).unwrap().profile_ids,
        ["p3", "p2", "p1"]
    );
    config.groups[1].mode = RoutingMode::Manual;
    config.groups[1].picked_profile_id = Some("p1".into());
    assert_eq!(
        resolve(config, json!({})).unwrap().profile_ids,
        ["p1", "p2"]
    );
}

#[test]
fn invalid_graphs_rules_manual_picks_and_selected_stale_profiles_fail() {
    let mut cycle = policy();
    cycle.groups[0].members.push(RoutingMember::Group {
        group_id: "g".into(),
    });
    assert!(validation::validate(&cycle).is_err());
    let mut missing_group = policy();
    missing_group.groups[0].members.push(RoutingMember::Group {
        group_id: "gone".into(),
    });
    assert!(validation::validate(&missing_group).is_err());
    let mut missing_profile = policy();
    missing_profile.groups[0]
        .members
        .push(RoutingMember::Profile {
            profile_id: "gone".into(),
        });
    assert!(resolve(missing_profile, json!({})).is_err());
    let mut manual = policy();
    manual.groups[0].mode = RoutingMode::Manual;
    assert!(validation::validate(&manual).is_err());
    let mut rule = policy();
    rule.rules.push(RoutingRule {
        id: "r".into(),
        name: "r".into(),
        group_id: "g".into(),
        ..Default::default()
    });
    assert!(validation::validate(&rule).is_err());
    let mut duplicate = policy();
    duplicate.groups.push(duplicate.groups[0].clone());
    assert!(validation::validate(&duplicate).is_err());
}

#[test]
fn nesting_and_graph_expansion_are_bounded() {
    let mut config = policy();
    for i in 0..9 {
        let mut next = group(&format!("g{i}"), &["p1"]);
        if i > 0 {
            next.members.push(RoutingMember::Group {
                group_id: format!("g{}", i - 1),
            });
        }
        config.groups.push(next);
    }
    assert!(validation::validate(&config).is_err());
    let mut config = policy();
    for i in 0..24 {
        let mut next = group(&format!("g{i}"), &["p1"]);
        for j in 0..i {
            next.members.push(RoutingMember::Group {
                group_id: format!("g{j}"),
            });
        }
        config.groups.push(next);
    }
    assert!(validation::validate(&config).is_err());
    let mut wide = policy();
    for level in 0..5 {
        for node in 0..6 {
            let mut next = group(&format!("level-{level}-{node}"), &["p1"]);
            if level > 0 {
                for child in 0..6 {
                    next.members.push(RoutingMember::Group {
                        group_id: format!("level-{}-{child}", level - 1),
                    });
                }
            }
            wide.groups.push(next);
        }
    }
    assert!(validation::validate(&wide)
        .unwrap_err()
        .contains("expansion"));
}

#[test]
fn rotations_are_bounded_and_previews_do_not_advance_them() {
    let mut counters = std::collections::VecDeque::new();
    assert_eq!(plan::rotation(&mut counters, "g".into(), 3, true), 0);
    assert_eq!(plan::rotation(&mut counters, "g".into(), 3, false), 1);
    assert_eq!(plan::rotation(&mut counters, "g".into(), 3, true), 1);
    for i in 0..2000 {
        plan::rotation(&mut counters, format!("revision-{i}"), 2, true);
    }
    assert_eq!(counters.len(), 512);
}

fn db() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    crate::db::schema::run_migrations(&conn).unwrap();
    for id in ["p1", "p2", "p3"] {
        conn.execute("INSERT INTO config_profiles(id,name,tool_id,config_snapshot) VALUES(?1,?1,'claude','{}')",[id]).unwrap();
    }
    conn.execute(
        "INSERT INTO app_settings(key,value) VALUES('current_profile_claude','p1')",
        [],
    )
    .unwrap();
    conn
}

#[test]
fn save_uses_exact_revision_and_rejections_leave_previous_policy_intact() {
    let conn = db();
    assert!(load(&conn, "claude").unwrap().revision.is_none());
    let first = save(&conn, "claude", None, policy()).unwrap();
    assert!(save(&conn, "claude", None, RoutingPolicy::default()).is_err());
    assert_eq!(load(&conn, "claude").unwrap().revision, first.revision);
    assert!(save(&conn, "codex", None, policy()).is_err());
    assert!(load(&conn, "codex").unwrap().revision.is_none());
    let second = save(
        &conn,
        "claude",
        first.revision.as_deref(),
        RoutingPolicy::default(),
    )
    .unwrap();
    assert_ne!(first.revision, second.revision);
    assert!(save(&conn, "claude", first.revision.as_deref(), policy()).is_err());
    assert_eq!(load(&conn, "claude").unwrap().revision, second.revision);
}

#[test]
fn missing_members_can_be_disabled_and_corrupt_saved_data_is_not_silently_reset() {
    let conn = db();
    let first = save(&conn, "claude", None, policy()).unwrap();
    conn.execute("DELETE FROM config_profiles WHERE id='p2'", [])
        .unwrap();
    assert!(save(&conn, "claude", first.revision.as_deref(), policy()).is_err());
    let mut disabled = policy();
    disabled.enabled = false;
    let second = save(&conn, "claude", first.revision.as_deref(), disabled).unwrap();
    assert!(!second.policy.enabled);
    conn.execute(
        "UPDATE app_settings SET value='bad secret value' WHERE key='provider_routing:claude'",
        [],
    )
    .unwrap();
    let error = load(&conn, "claude").unwrap_err();
    assert!(!error.contains("secret"));
    assert!(load(&conn, "unsupported").is_err());
}

#[test]
fn preview_simulates_byte_thresholds_without_allocating_or_saving_a_large_body() {
    let conn = db();
    let mut config = policy();
    config.groups.push(group("large", &["p3"]));
    config.rules.push(RoutingRule {
        id: "bytes".into(),
        name: "bytes".into(),
        group_id: "large".into(),
        min_request_bytes: 100,
        ..Default::default()
    });
    for (size, selected) in [
        (Some(99), None),
        (Some(100), Some("bytes")),
        (None, None),
        (Some(67108864), Some("bytes")),
    ] {
        let result = preview(&conn, "claude", config.clone(), json!({}), "", size).unwrap();
        assert_eq!(result.rule_id.as_deref(), selected);
    }
    assert!(preview(&conn, "claude", config, json!({}), "", Some(67108865)).is_err());
    assert!(load(&conn, "claude").unwrap().revision.is_none());
}

#[test]
fn preview_uses_active_order_and_never_persists_drafts() {
    let conn = db();
    let plan = preview(&conn, "claude", policy(), json!({}), "", None).unwrap();
    assert_eq!(plan.profile_ids, ["p2", "p1"]);
    assert!(load(&conn, "claude").unwrap().revision.is_none());
    let plan = preview(
        &conn,
        "claude",
        RoutingPolicy::default(),
        json!({}),
        "",
        None,
    )
    .unwrap();
    assert_eq!(plan.profile_ids[0], "p1");
}
