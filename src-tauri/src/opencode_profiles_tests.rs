use super::*;
use serde_json::json;

#[test]
fn flat_profile_becomes_native_and_retains_existing_configuration() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("opencode.jsonc");
    let source = r#"{
  // 自定义设置保留
  "theme": "dark",
  "plugin": ["other-plugin"],
  "mcp": { "server": { "type": "local", "command": ["node"] } },
  "provider": {
    "other": { "options": { "apiKey": "other" } },
    "custom": { "options": { "timeout": 5000 }, "models": { "older": { "limit": { "context": 64000 } } } }
  }
}"#;
    std::fs::write(&path, source).unwrap();
    let profile = json!({"npm":"@ai-sdk/openai-compatible","name":"custom",
        "metadata":{"usageScript":{"enabled":false}},"customEndpoints":["https://backup.test"],
        "options":{"baseURL":"https://api.example.test/v1","apiKey":"fixture-key"},
        "models":{"vendor/new":{"name":"New","contextLimit":128000,"outputLimit":16000,"variants":{"fast":{"reasoningEffort":"low"}}}}
    });
    apply_profile(&path, &profile.to_string()).unwrap();
    let output = std::fs::read_to_string(&path).unwrap();
    assert!(output.contains("// 自定义设置保留\n  \"theme\": \"dark\""));
    assert!(output.contains(r#""mcp": { "server": { "type": "local", "command": ["node"] } }"#));
    let native = crate::json_config::parse_json_object(&output).unwrap();
    assert_eq!(native["model"], "custom/vendor/new");
    assert_eq!(native["provider"]["custom"]["options"]["timeout"], 5000);
    assert_eq!(
        native["provider"]["custom"]["models"]["vendor/new"]["limit"],
        json!({"context":128000,"output":16000})
    );
    assert!(native["provider"]["custom"]["models"]
        .get("older")
        .is_some());
    assert!(native["provider"]["custom"].get("metadata").is_none());
    assert!(native.get("npm").is_none());
    let read: Value = serde_json::from_str(&read_profile(&path).unwrap()).unwrap();
    assert_eq!(read["metadata"]["nativeModelId"], "vendor/new");
    assert_eq!(read["models"]["vendor/new"]["contextLimit"], 128000);
    assert_eq!(read["options"]["apiKey"], "fixture-key");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), output);
}

#[test]
fn built_in_key_only_profiles_do_not_add_sdk_or_replace_native_model() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("opencode.jsonc");
    std::fs::write(&path, "{\"provider\":{\"anthropic\":{\"options\":{\"apiKey\":\"old\"}}},\"model\":\"anthropic/claude-model\"}").unwrap();
    let mut profile: Value = serde_json::from_str(&read_profile(&path).unwrap()).unwrap();
    assert_eq!(profile["metadata"]["nativeProviderId"], "anthropic");
    assert!(profile.get("npm").is_none());
    profile["options"]["apiKey"] = json!("new");
    apply_profile(&path, &profile.to_string()).unwrap();
    let output =
        crate::json_config::parse_json_object(&std::fs::read_to_string(path).unwrap()).unwrap();
    assert_eq!(output["provider"]["anthropic"]["options"]["apiKey"], "new");
    assert!(output["provider"]["anthropic"].get("npm").is_none());
    assert_eq!(output["model"], "anthropic/claude-model");
}

#[test]
fn cleared_form_limits_restore_defaults_without_touching_other_models() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("opencode.json");
    let source = json!({"model":"local/a", "provider":{"local":{"models":{
        "a":{"limit":{"context":10000,"output":1000},"cost":{"input":2}},
        "b":{"limit":{"context":20000}}
    }}}});
    std::fs::write(&path, source.to_string()).unwrap();
    let profile = json!({"metadata":{"nativeProviderId":"local","nativeModelId":"a",
        "modelCatalog":{"toolId":"opencode","models":[{"id":"a","contextWindow":300000}]}},
        "models":{"a":{"contextLimit":null,"outputLimit":null}}});
    apply_profile(&path, &profile.to_string()).unwrap();
    let native =
        crate::json_config::parse_json_object(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let models = &native["provider"]["local"]["models"];
    assert!(models["a"].get("limit").is_none());
    assert_eq!(models["a"]["cost"]["input"], 2);
    assert_eq!(models["b"]["limit"]["context"], 20000);
    assert!(native["provider"]["local"].get("metadata").is_none());
    let read: Value = serde_json::from_str(&read_profile(&path).unwrap()).unwrap();
    assert!(read["models"]["a"].get("contextLimit").is_none());
}

#[test]
fn explicit_empty_selection_does_not_fall_back_to_the_first_configured_model() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("opencode.json");
    let source = json!({"model":"local/a", "provider":{"local":{"models":{"a":{"limit":{"context":10000}}}}}});
    std::fs::write(&path, source.to_string()).unwrap();
    let profile = json!({"metadata":{"nativeProviderId":"local","nativeModelId":""},"models":{"a":{"contextLimit":10000}}});
    apply_profile(&path, &profile.to_string()).unwrap();
    let native =
        crate::json_config::parse_json_object(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert!(native.get("model").is_none());
    assert_eq!(
        native["provider"]["local"]["models"]["a"]["limit"]["context"],
        10000
    );
    let restored: Value = serde_json::from_str(&read_profile(&path).unwrap()).unwrap();
    assert_eq!(restored["metadata"]["nativeModelId"], "");
    std::fs::write(&path, source.to_string().replace("local/a", "other/a")).unwrap();
    apply_profile(&path, &profile.to_string()).unwrap();
    let native =
        crate::json_config::parse_json_object(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(native["model"], "other/a");
}

#[test]
fn repairs_legacy_flat_files_without_losing_mcp() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("opencode.json");
    let snapshot = r#"{"npm":"@ai-sdk/openai-compatible","options":{"apiKey":"new"},"models":{"model-a":{"name":"A"}}}"#;
    std::fs::write(
        &path,
        r#"{"npm":"old","options":{"apiKey":"old"},"models":{},"mcp":{"keep":{}}}"#,
    )
    .unwrap();
    apply_profile(&path, snapshot).unwrap();
    let value =
        crate::json_config::parse_json_object(&std::fs::read_to_string(path).unwrap()).unwrap();
    assert!(value.get("options").is_none());
    assert_eq!(value["mcp"], json!({"keep":{}}));
    assert_eq!(value["model"], "custom/model-a");
}

#[test]
fn validation_failures_do_not_write_any_native_configuration() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("opencode.json");
    for snapshot in [
        json!({"metadata":{"nativeProviderId":"bad/id"}}),
        json!({"models":[]}),
        json!({"options":false}),
        json!({"models":{"model":{"contextLimit":-2}}}),
        json!({"models":{"model":{"outputLimit":1.5}}}),
    ] {
        assert!(apply_profile(&path, &snapshot.to_string()).is_err());
        assert!(!path.exists());
    }
    let source = "{\"provider\": []}";
    std::fs::write(&path, source).unwrap();
    assert!(apply_profile(&path, r#"{"options":{"apiKey":"new"}}"#).is_err());
    assert_eq!(std::fs::read_to_string(path).unwrap(), source);
}

#[test]
fn native_snapshot_import_selects_only_the_active_provider() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("opencode.jsonc");
    let snapshot = json!({"provider":{"first":{"options":{"apiKey":"unused"}},"selected":{"options":{"apiKey":"selected"}}},"model":"selected/vendor/model","permission":{"bash":"allow"}});
    std::fs::write(&path, "{\"permission\":{\"bash\":\"ask\"}}").unwrap();
    apply_profile(&path, &snapshot.to_string()).unwrap();
    let native =
        crate::json_config::parse_json_object(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(
        native["provider"]["selected"]["options"]["apiKey"],
        "selected"
    );
    assert!(native["provider"].get("first").is_none());
    assert_eq!(native["permission"]["bash"], "ask");
    assert_eq!(native["model"], "selected/vendor/model");
}

#[test]
fn builtin_selection_without_override_does_not_import_a_different_provider() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("opencode.json");
    std::fs::write(&path, "{\"model\":\"anthropic/selected\",\"provider\":{\"other\":{\"options\":{\"apiKey\":\"unrelated\"}}}}").unwrap();
    let profile: Value = serde_json::from_str(&read_profile(&path).unwrap()).unwrap();
    assert_eq!(profile["metadata"]["nativeProviderId"], "anthropic");
    assert_eq!(profile["metadata"]["nativeModelId"], "selected");
    assert!(profile.get("options").is_none());
}
