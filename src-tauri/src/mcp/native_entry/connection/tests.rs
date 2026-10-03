use super::*;
use crate::mcp::sources::{SourceBinding, SourceRole};

fn origin(tool: &str, spec: NativeSpec) -> (tempfile::TempDir, NativeOrigin) {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("native.conf");
    let binding = SourceBinding {
        tool: tool.into(),
        path: path.clone(),
        role: SourceRole::Primary,
    };
    let canonical = crate::config_write::target_key(&path).unwrap();
    let origin = NativeOrigin::new(
        binding,
        canonical,
        Format::for_tool(tool).unwrap().container().into(),
        "same".into(),
        spec,
    )
    .unwrap();
    (root, origin)
}

#[test]
fn projected_remote_connections_retain_headers_policy_and_extensions_in_every_format() {
    let (_root, source) = origin("claude", NativeSpec::Json(r#"{"type":"http","url":"https://fixture.invalid/mcp","headers":{"Authorization":"private-fixture"},"enabled":false,"disabled":true,"timeout":91,"extension":{"keep":true}}"#.into()));
    for tool in [
        "claude",
        "claude-desktop",
        "codex",
        "gemini",
        "grokbuild",
        "opencode",
        "hermes",
        "mcode",
    ] {
        let spec = project_connection(&source, tool).unwrap();
        let (_root, projected) = origin(tool, spec.clone());
        assert_eq!(
            projected.connection, source.connection,
            "connection changed for {tool}"
        );
        assert!(projected.disabled, "disabled policy lost for {tool}");
        let value = spec.to_json().unwrap();
        assert_eq!(value["timeout"], 91);
        assert_eq!(value["extension"]["keep"], true);
        assert_eq!(value["disabled"], true);
        assert!(value.get("command").is_none());
        assert!(value.get("env").is_none());
        if tool == "codex" {
            assert_eq!(value["http_headers"]["Authorization"], "private-fixture");
        } else {
            assert_eq!(value["headers"]["Authorization"], "private-fixture");
        }
    }
}

#[test]
fn native_typed_extensions_survive_same_format_edits_and_lossy_targets_are_refused() {
    let (_root, source) = origin("codex", NativeSpec::Toml("command='old'\nenabled=false\nweight=nan\ndate=1979-05-27T07:32:00Z\n[extension]\nkeep=true\n".into()));
    let config = McpServerConfig {
        command: "new".into(),
        args: vec!["one".into()],
        env: std::collections::HashMap::new(),
        transport_type: Some("stdio".into()),
    };
    let changed = patch_connection(Some(&source.spec), "same", "codex", &config).unwrap();
    let Entry::Toml(fields) = changed.entry().unwrap() else {
        panic!("native format lost")
    };
    assert!(fields["weight"].as_float().unwrap().is_nan());
    assert!(matches!(fields["date"], ::toml::Value::Datetime(_)));
    assert_eq!(fields["command"].as_str(), Some("new"));
    assert_eq!(fields["enabled"].as_bool(), Some(false));
    assert!(project_connection(&source, "claude").is_err());
    assert!(project_connection(&source, "hermes").is_err());
    let (_root, date) = origin(
        "codex",
        NativeSpec::Toml("command='old'\ndate=1979-05-27\n".into()),
    );
    assert!(project_connection(&date, "claude").is_err());
}

#[test]
fn native_authentication_is_preserved_locally_and_never_silently_mapped_to_another_tool() {
    let (_root, source) = origin("codex", NativeSpec::Toml("url='https://fixture.invalid/mcp'\nbearer_token_env_var='TOKEN'\n[env_http_headers]\nAuthorization='AUTH'\n".into()));
    let config = McpServerConfig {
        command: "https://fixture.invalid/new".into(),
        args: vec![],
        env: std::collections::HashMap::new(),
        transport_type: Some("http".into()),
    };
    let value = patch_connection(Some(&source.spec), "same", "codex", &config)
        .unwrap()
        .to_json()
        .unwrap();
    assert_eq!(value["bearer_token_env_var"], "TOKEN");
    assert_eq!(value["env_http_headers"]["Authorization"], "AUTH");
    for tool in [
        "claude",
        "grokbuild",
        "gemini",
        "opencode",
        "hermes",
        "mcode",
    ] {
        let error = project_connection(&source, tool).unwrap_err();
        assert!(!error.contains("TOKEN"));
        assert!(!error.contains("AUTH"));
    }
}

#[test]
fn unsupported_json_extension_values_do_not_disappear_during_toml_projection() {
    let (_root, source) = origin(
        "claude",
        NativeSpec::Json(r#"{"command":"node","extension":null}"#.into()),
    );
    assert!(project_connection(&source, "codex").is_err());
    assert!(project_connection(&source, "grokbuild").is_err());
    assert_eq!(
        project_connection(&source, "gemini")
            .unwrap()
            .to_json()
            .unwrap()["extension"],
        serde_json::Value::Null
    );
}
