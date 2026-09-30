use super::*;
use serde_json::json;

fn config_file(source: &str) -> (tempfile::TempDir, std::path::PathBuf) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("config.jsonc");
    std::fs::write(&path, source).unwrap();
    (directory, path)
}

#[test]
fn updates_only_changed_values_and_keeps_unicode_comments_crlf_bom() {
    let source = "\u{feff}{\r\n\t// 保留 😀\r\n\t\"model\": \"old\", /* 模型 */\r\n\t\"other\": { \"raw\": 1e3, \"array\": [1, 2,], },\r\n}\r\n";
    let (_directory, path) = config_file(source);
    update_json_file(&path, |value| {
        value["model"] = json!("new");
        Ok(())
    })
    .unwrap();
    let output = std::fs::read_to_string(path).unwrap();
    assert_eq!(output, source.replace("\"old\"", "\"new\""));
}

#[test]
fn nested_edits_keep_sibling_order_comments_and_escaped_property_names() {
    let source = r#"{
  "mcp": {
    // 外部配置
    "other": { "command": ["keep"], },
    "server.a": {
      "u\u0072l": "old", // URL 注释
      "oauth": { "clientId": "custom", "scopes": ["read", "write"] }
    }
  },
  "theme": "dark"
}"#;
    let (_directory, path) = config_file(source);
    update_json_file(&path, |value| {
        value["mcp"]["server.a"]["url"] = json!("https://example.test");
        value["mcp"]["server.a"]["enabled"] = json!(true);
        Ok(())
    })
    .unwrap();
    let output = std::fs::read_to_string(path).unwrap();
    assert!(output.contains(r#""other": { "command": ["keep"], },"#));
    assert!(output.contains(r#""u\u0072l": "https://example.test", // URL 注释"#));
    assert!(output.contains(r#""oauth": { "clientId": "custom", "scopes": ["read", "write"] }"#));
    assert_eq!(
        parse_json_object(&output).unwrap()["mcp"]["server.a"]["enabled"],
        true
    );
}

#[test]
fn removes_first_middle_last_and_only_properties_without_invalid_commas() {
    for key in ["first", "middle", "last"] {
        let (_directory, path) =
            config_file("{\n  \"first\": 1,\n  \"middle\": 2, // note\n  \"last\": 3,\n}\n");
        update_json_file(&path, |value| {
            value.as_object_mut().unwrap().remove(key);
            Ok(())
        })
        .unwrap();
        let output = parse_json_object(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(output.as_object().unwrap().len(), 2);
        assert!(output.get(key).is_none());
    }
    let (_directory, path) = config_file("{\"only\": 1, /* 尾部 */}");
    update_json_file(&path, |value| {
        *value = json!({});
        Ok(())
    })
    .unwrap();
    assert_eq!(
        parse_json_object(&std::fs::read_to_string(path).unwrap()).unwrap(),
        json!({})
    );
}

#[test]
fn no_op_preserves_exact_bytes_and_never_creates_missing_file() {
    let source = "// 😀\n{ \"number\": 1e3, \"model\": \"unchanged\", }\n";
    let (directory, path) = config_file(source);
    let modified = std::fs::metadata(&path).unwrap().modified().unwrap();
    update_json_file(&path, |_| Ok(())).unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), source);
    assert_eq!(
        std::fs::metadata(&path).unwrap().modified().unwrap(),
        modified
    );
    let absent = directory.path().join("missing/config.jsonc");
    update_json_file(&absent, |_| Ok(())).unwrap();
    assert!(!absent.parent().unwrap().exists());
}

#[test]
fn creates_missing_parents_and_replaces_arrays_scalars_with_valid_values() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("nested/config.jsonc");
    update_json_file(&path, |value| {
        *value = json!({"items": ["引号\"", 3, false, null], "object": {"field": true}});
        Ok(())
    })
    .unwrap();
    update_json_file(&path, |value| {
        value["items"] = json!({"new": "value"});
        value["object"] = json!(17);
        Ok(())
    })
    .unwrap();
    let parsed = parse_json_object(&std::fs::read_to_string(path).unwrap()).unwrap();
    assert_eq!(parsed, json!({"items": {"new": "value"}, "object": 17}));
}

#[test]
fn rejects_invalid_ambiguous_and_json5_inputs_without_replacing_them_or_exposing_secrets() {
    for source in [
        "",
        "[]",
        "null",
        "{\"key\": \"private-secret\",}",
        "{\"a\": 1 \"b\": 2}",
        "{a: 1}",
        "{\"a\": 'value'}",
        "{\"a\": NaN}",
        "{\"a\": 1, \"a\": 2}",
        "{\"a\": {\"x\": 1, \"\\u0078\": 2}}",
    ] {
        // The secret-bearing input here is valid JSONC; make it invalid below.
        let source = if source.contains("private-secret") {
            "{\"key\": \"private-secret\", broken"
        } else {
            source
        };
        let (_directory, path) = config_file(source);
        let error = update_json_file(&path, |value| {
            value["new"] = json!(true);
            Ok(())
        })
        .unwrap_err();
        assert!(!error.contains("private-secret"));
        assert_eq!(std::fs::read_to_string(path).unwrap(), source);
    }
}

#[test]
fn failed_edits_and_external_changes_never_overwrite_current_bytes() {
    let source = "{\"model\": \"before\"}";
    let (_directory, path) = config_file(source);
    assert!(update_json_file(&path, |value| {
        value["model"] = json!("partial");
        Err("validation failed".into())
    })
    .is_err());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), source);
    let error = update_json_file(&path, |value| {
        value["model"] = json!("ours");
        std::fs::write(&path, "{\"model\": \"external\"}").unwrap();
        Ok(())
    })
    .unwrap_err();
    assert!(error.contains("changed externally"));
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "{\"model\": \"external\"}"
    );
}

#[test]
fn rejects_non_object_update_without_touching_file() {
    let source = "{}\n";
    let (_directory, path) = config_file(source);
    assert!(update_json_file(&path, |value| {
        *value = json!([]);
        Ok(())
    })
    .is_err());
    assert_eq!(std::fs::read_to_string(path).unwrap(), source);
}
