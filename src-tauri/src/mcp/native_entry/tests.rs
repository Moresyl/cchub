use super::*;
use crate::mcp::sources::{SourceRole, SourceSnapshot};

fn change(tool: &str, path: &std::path::Path, name: &str, spec: Option<NativeSpec>) -> Change {
    Change {
        binding: SourceBinding {
            tool: tool.into(),
            path: path.into(),
            role: SourceRole::Primary,
        },
        canonical_path: crate::config_write::target_key(path).unwrap(),
        container: if tool == "opencode" {
            "mcp"
        } else if matches!(tool, "codex" | "grokbuild" | "hermes") {
            "mcp_servers"
        } else {
            "mcpServers"
        }
        .into(),
        name: name.into(),
        spec,
        original: crate::config_write::read(path).unwrap(),
        revision: crate::config_write::FileRevision::capture(path).unwrap().0,
        aliases: Vec::new(),
    }
}

#[test]
fn full_native_changes_retain_options_comments_and_unrelated_same_names() {
    let root = tempfile::tempdir().unwrap();
    for tool in ["claude", "codex", "hermes"] {
        let file = root.path().join(tool);
        let (source, spec) = match tool {
            "codex" => ("\u{feff}# head\r\nweight=nan\r\n[mcp_servers.selected]\r\ncommand='node' # retained\r\ntimeout=91\r\nenabled=false\r\n[mcp_servers.other]\r\ncommand='other'\r\n", NativeSpec::Toml("command='edited'\ntimeout=91\nenabled=false\ncustom='new'\n".into())),
            "hermes" => ("\u{feff}# head\r\nweight: .nan\r\nmcp_servers:\r\n  selected:\r\n    command: node # retained\r\n    timeout: 91\r\n    enabled: false\r\n  other:\r\n    command: other\r\n# tail\r\n", NativeSpec::Yaml("command: edited\ntimeout: 91\nenabled: false\ncustom: new\n".into())),
            _ => ("\u{feff}{// head\r\n\"mcpServers\":{\"selected\":{\"command\":\"node\",/* retained */\"timeout\":91,\"enabled\":false},\"other\":{\"command\":\"other\"}}}\r\n", NativeSpec::Json(r#"{"command":"edited","timeout":91,"enabled":false,"custom":"new"}"#.into())),
        };
        std::fs::write(&file, source).unwrap();
        prepare(&[change(tool, &file, "selected", Some(spec))])
            .unwrap()
            .commit_then(|| Ok(()))
            .unwrap();
        let text = std::fs::read_to_string(&file).unwrap();
        assert!(text.starts_with('\u{feff}'));
        assert!(text.contains("retained"));
        assert!(text.contains("head"));
        assert!(!text.replace("\r\n", "").contains('\n'));
        let snapshot = SourceSnapshot::read_bindings(&[SourceBinding {
            tool: tool.into(),
            path: file,
            role: SourceRole::Primary,
        }])
        .unwrap();
        assert_eq!(snapshot.origins.len(), 2);
        assert!(snapshot
            .origins
            .iter()
            .any(|origin| origin.native_name == "other" && origin.connection.command == "other"));
        assert!(snapshot
            .origins
            .iter()
            .any(|origin| origin.native_name == "selected"
                && origin.disabled
                && origin.connection.command == "edited"));
    }
}

#[test]
fn aliases_compose_distinct_containers_once_and_conflicting_entries_refuse() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("shared.json");
    std::fs::write(&file, "{}\n").unwrap();
    let first = change(
        "claude",
        &file,
        "same",
        Some(NativeSpec::Json(r#"{"command":"node"}"#.into())),
    );
    let second = change(
        "opencode",
        &file,
        "same",
        Some(NativeSpec::Json(
            r#"{"type":"local","command":["other","--mcp"]}"#.into(),
        )),
    );
    let prepared = prepare(&[first, second]).unwrap();
    assert_eq!(prepared.plan.updates.len(), 1);
    prepared.commit_then(|| Ok(())).unwrap();
    let original = std::fs::read(&file).unwrap();
    let first = change(
        "claude",
        &file,
        "same",
        Some(NativeSpec::Json(r#"{"command":"one"}"#.into())),
    );
    let second = change(
        "mcode",
        &file,
        "same",
        Some(NativeSpec::Json(r#"{"command":"two"}"#.into())),
    );
    assert!(prepare(&[first, second]).is_err());
    assert_eq!(std::fs::read(file).unwrap(), original);
}

#[test]
fn noops_and_missing_removals_preserve_exact_bytes_and_missing_parents() {
    let root = tempfile::tempdir().unwrap();
    let missing = root.path().join("not-created/missing.json");
    let prepared = prepare(&[change("claude", &missing, "absent", None)]).unwrap();
    assert!(prepared.plan.updates.is_empty());
    prepared.commit_then(|| Ok(())).unwrap();
    assert!(!missing.parent().unwrap().exists());
    let file = root.path().join("no-op.toml");
    let source = "\u{feff}# exact\r\n[mcp_servers.same]\r\ncommand='node'\r\nweight=nan\r\n";
    std::fs::write(&file, source).unwrap();
    let prepared = prepare(&[change(
        "codex",
        &file,
        "same",
        Some(NativeSpec::Toml("command='node'\nweight=nan\n".into())),
    )])
    .unwrap();
    assert!(prepared.plan.updates.is_empty());
    prepared.commit_then(|| Ok(())).unwrap();
    assert_eq!(std::fs::read_to_string(file).unwrap(), source);
}

#[test]
fn yaml_merged_container_removal_preserves_other_effective_entries_and_source_trivia() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("merged.yaml");
    let source = "# head\nbase: &base\n  same: {command: node, enabled: false, timeout: 91}\n  sibling: {command: other}\nmcp_servers:\n  <<: *base\n  direct: {command: direct}\n# tail\n";
    std::fs::write(&file, source).unwrap();
    prepare(&[change("hermes", &file, "same", None)])
        .unwrap()
        .commit_then(|| Ok(()))
        .unwrap();
    let snapshot = SourceSnapshot::read_bindings(&[SourceBinding {
        tool: "hermes".into(),
        path: file.clone(),
        role: SourceRole::Primary,
    }])
    .unwrap();
    assert_eq!(snapshot.origins.len(), 2);
    assert!(snapshot
        .origins
        .iter()
        .any(|origin| origin.native_name == "sibling"));
    assert!(std::fs::read_to_string(file).unwrap().contains("# tail"));
}

#[test]
fn real_sql_failure_recovers_native_group_and_external_guard_prevents_any_write() {
    let root = tempfile::tempdir().unwrap();
    let first = root.path().join("first.json");
    let second = root.path().join("second.toml");
    let third = root.path().join("new/config.yaml");
    std::fs::write(&first, "{ /* exact */ \"mcpServers\":{} }\n").unwrap();
    std::fs::write(&second, "# exact\nmodel='keep'\n").unwrap();
    let original_first = std::fs::read(&first).unwrap();
    let original_second = std::fs::read(&second).unwrap();
    let changes = vec![
        change(
            "claude",
            &first,
            "same",
            Some(NativeSpec::Json(r#"{"command":"node"}"#.into())),
        ),
        change(
            "codex",
            &second,
            "same",
            Some(NativeSpec::Toml("command='node'\n".into())),
        ),
        change(
            "hermes",
            &third,
            "same",
            Some(NativeSpec::Yaml("command: node\n".into())),
        ),
    ];
    let prepared = prepare(&changes).unwrap();
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE catalog(value INTEGER CHECK(value=1)); INSERT INTO catalog VALUES(1);",
    )
    .unwrap();
    let tx = rusqlite::Transaction::new_unchecked(&conn, rusqlite::TransactionBehavior::Immediate)
        .unwrap();
    let result = prepared.commit_then(|| {
        assert!(third.exists());
        tx.execute("INSERT INTO catalog VALUES(2)", [])
            .map_err(|_| "SQL fixture refused".to_string())?;
        tx.commit().map_err(|_| "SQL fixture refused".to_string())
    });
    assert!(result.is_err());
    assert_eq!(std::fs::read(&first).unwrap(), original_first);
    assert_eq!(std::fs::read(&second).unwrap(), original_second);
    assert!(!third.exists());
    assert!(!third.parent().unwrap().exists());
    // Recovery replaces the files, so a retry must capture fresh identities.
    assert!(prepare(&changes).is_err());
    let refreshed: Vec<_> = changes
        .iter()
        .map(|old| {
            change(
                &old.binding.tool,
                &old.binding.path,
                &old.name,
                old.spec.clone(),
            )
        })
        .collect();
    let prepared = prepare(&refreshed).unwrap();
    std::fs::write(&second, "# external\n").unwrap();
    assert!(prepared
        .commit_then(|| panic!("finalizer must not run"))
        .is_err());
    assert_eq!(std::fs::read(first).unwrap(), original_first);
    assert_eq!(std::fs::read_to_string(second).unwrap(), "# external\n");
    assert!(!third.exists());
}

#[test]
fn malformed_siblings_and_invalid_definition_types_never_write() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("invalid.json");
    let source = r#"{"mcpServers":{"bad":{"command":"private-fixture","args":[1]}}}"#;
    std::fs::write(&file, source).unwrap();
    let result = prepare(&[change(
        "claude",
        &file,
        "new",
        Some(NativeSpec::Json(r#"{"command":"node"}"#.into())),
    )]);
    assert!(result.is_err());
    assert_eq!(std::fs::read_to_string(&file).unwrap(), source);
    let result = prepare(&[change(
        "claude",
        &file,
        "new",
        Some(NativeSpec::Json(r#"{"command":1}"#.into())),
    )]);
    assert!(result.is_err());
    assert_eq!(std::fs::read_to_string(file).unwrap(), source);
}

#[test]
fn source_revision_captured_before_prepare_refuses_intervening_edits() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("changed.json");
    std::fs::write(&file, r#"{"mcpServers":{"same":{"command":"old"}}}"#).unwrap();
    let intended = change(
        "claude",
        &file,
        "same",
        Some(NativeSpec::Json(r#"{"command":"new"}"#.into())),
    );
    let external = r#"{"mcpServers":{"same":{"command":"external"}},"unrelated":"keep"}"#;
    std::fs::write(&file, external).unwrap();
    assert!(prepare(&[intended]).is_err());
    assert_eq!(std::fs::read_to_string(file).unwrap(), external);
}

#[test]
fn equal_byte_replacements_are_rejected_before_prepare_and_commit() {
    for before_prepare in [true, false] {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("native.json");
        let bytes = br#"{"mcpServers":{"same":{"command":"old"}}}"#;
        std::fs::write(&file, bytes).unwrap();
        let changes = vec![change(
            "claude",
            &file,
            "same",
            Some(NativeSpec::Json(r#"{"command":"new"}"#.into())),
        )];
        let prepared = if before_prepare {
            None
        } else {
            Some(prepare(&changes).unwrap())
        };
        crate::utils::atomic_write(&file, bytes).unwrap();
        if let Some(prepared) = prepared {
            assert!(prepared
                .commit_then(|| panic!("finalizer must not run"))
                .is_err());
        } else {
            assert!(prepare(&changes).is_err());
        }
        assert_eq!(std::fs::read(file).unwrap(), bytes);
    }
}

#[cfg(windows)]
#[test]
fn junction_retarget_is_rejected_before_prepare_and_before_commit() {
    fn junction(path: &std::path::Path, target: &std::path::Path) {
        let mut process = std::process::Command::new("powershell");
        crate::utils::configure_background_command(&mut process);
        let output = process
            .args(["-NoProfile", "-NonInteractive", "-Command", "New-Item -ItemType Junction -Path $env:CCHUB_TEST_LINK -Target $env:CCHUB_TEST_TARGET -ErrorAction Stop | Out-Null"])
            .env("CCHUB_TEST_LINK", path).env("CCHUB_TEST_TARGET", target)
            .output().unwrap();
        assert!(
            output.status.success(),
            "owned fixture junction creation failed"
        );
    }
    for before_prepare in [true, false] {
        let root = tempfile::tempdir().unwrap();
        let first = root.path().join("first");
        let second = root.path().join("second");
        let link = root.path().join("alias");
        let bytes = br#"{"mcpServers":{"same":{"command":"old"}}}"#;
        for dir in [&first, &second] {
            std::fs::create_dir(dir).unwrap();
            std::fs::write(dir.join("native.json"), bytes).unwrap();
        }
        junction(&link, &first);
        let changes = vec![change(
            "claude",
            &link.join("native.json"),
            "same",
            Some(NativeSpec::Json(r#"{"command":"new"}"#.into())),
        )];
        let prepared = if before_prepare {
            None
        } else {
            Some(prepare(&changes).unwrap())
        };
        std::fs::remove_dir(&link).unwrap();
        junction(&link, &second);
        if let Some(prepared) = prepared {
            assert!(prepared
                .commit_then(|| panic!("finalizer must not run"))
                .is_err());
        } else {
            assert!(prepare(&changes).is_err());
        }
        for dir in [&first, &second] {
            assert_eq!(std::fs::read(dir.join("native.json")).unwrap(), bytes);
        }
        std::fs::remove_dir(&link).unwrap();
    }
}
