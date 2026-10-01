use super::*;
use crate::db::schema;
use rusqlite::Connection;

fn local_settings() -> (tempfile::TempDir, Connection, PathBuf) {
    let directory = tempfile::tempdir().unwrap();
    let conn = Connection::open_in_memory().unwrap();
    schema::run_migrations(&conn).unwrap();
    conn.execute(
        "INSERT INTO custom_paths(tool_id,config_dir) VALUES('claude',?1)",
        [directory.path().to_str().unwrap()],
    )
    .unwrap();
    let path = resolve_claude_settings_local_path(&conn).unwrap();
    assert_eq!(path, directory.path().join("settings.local.json"));
    (directory, conn, path)
}

#[test]
fn claude_toggle_updates_preserve_jsonc_bom_crlf_comments_and_unrelated_fields() {
    let (_directory, conn, path) = local_settings();
    let source = "\u{feff}{\r\n  // 保留设置 😀\r\n  \"env\": { \"OTHER\": \"keep\", /* 保留 */ \"CLAUDE_CODE_MAX_THINKING_TOKENS\": \"64000\", },\r\n  \"permissions\": { \"allow\": [\"Read\"], },\r\n  \"unrelated\": 1.2300e+4,\r\n}\r\n";
    std::fs::write(&path, source).unwrap();
    let result = write_claude_config_toggle_to_conn(&conn, "hideAttribution", true).unwrap();
    assert!(result.hide_attribution);
    assert!(result.max_thinking_tokens);
    assert_eq!(result.max_thinking_tokens_value, "64000");
    let output = std::fs::read_to_string(&path).unwrap();
    assert!(output.starts_with('\u{feff}'));
    for retained in [
        "// 保留设置 😀\r\n",
        "/* 保留 */",
        "\"OTHER\": \"keep\"",
        "\"permissions\": { \"allow\": [\"Read\"], }",
        "1.2300e+4",
    ] {
        assert!(output.contains(retained), "{retained}: {output}");
    }
    let result = write_claude_config_toggle_to_conn(&conn, "hideAttribution", false).unwrap();
    assert!(!result.hide_attribution);
    assert_eq!(result.max_thinking_tokens_value, "64000");
    let value =
        crate::json_config::parse_json_object(&std::fs::read_to_string(path).unwrap()).unwrap();
    assert_eq!(value["env"]["OTHER"], "keep");
    assert_eq!(value["permissions"]["allow"][0], "Read");
}

#[test]
fn all_claude_toggles_round_trip_without_rewriting_other_settings() {
    let (_directory, conn, path) = local_settings();
    for (key, name, value) in [
        ("hideAttribution", "ANTHROPIC_HIDE_ATTRIBUTION", "true"),
        ("enableTeammates", "CLAUDE_CODE_ENABLE_TEAMMATES", "true"),
        (
            "maxThinkingTokens",
            "CLAUDE_CODE_MAX_THINKING_TOKENS",
            "32000",
        ),
        ("enableToolSearch", "ENABLE_TOOL_SEARCH", "true"),
    ] {
        let source = "{\"permissions\":{\"allow\":[\"Read\"]},\"unrelated\":1e3}";
        std::fs::write(&path, source).unwrap();
        write_claude_config_toggle_to_conn(&conn, key, true).unwrap();
        let enabled =
            crate::json_config::parse_json_object(&std::fs::read_to_string(&path).unwrap())
                .unwrap();
        assert_eq!(enabled["env"][name], value);
        let result = write_claude_config_toggle_to_conn(&conn, key, false).unwrap();
        assert!(
            !result.hide_attribution
                && !result.enable_teammates
                && !result.max_thinking_tokens
                && !result.enable_tool_search
        );
        let output = std::fs::read_to_string(&path).unwrap();
        assert!(output.contains("\"unrelated\":1e3"));
        assert!(crate::json_config::parse_json_object(&output)
            .unwrap()
            .get("env")
            .is_none());
    }
}

#[test]
fn malformed_claude_local_settings_are_never_replaced_or_exposed_in_errors() {
    let (_directory, conn, path) = local_settings();
    for source in [
        "",
        "[]",
        "null",
        "\"synthetic-private-secret\"",
        "{\"synthetic-private-secret\":",
        "{\"env\":{\"key\":1},\"env\":{\"key\":2}}",
        "{} {}",
        "{\"env\":[\"synthetic-private-secret\"]}",
        "{\"env\":\"synthetic-private-secret\"}",
        "{\"env\":true}",
    ] {
        std::fs::write(&path, source).unwrap();
        for enabled in [false, true] {
            let error =
                write_claude_config_toggle_to_conn(&conn, "enableToolSearch", enabled).unwrap_err();
            assert!(!error.contains("synthetic-private-secret"));
            assert_eq!(std::fs::read_to_string(&path).unwrap(), source);
        }
        assert!(read_claude_config_toggles_from_conn(&conn).is_err());
    }
}

#[test]
fn missing_null_and_unchanged_claude_env_have_no_unnecessary_writes() {
    let (_directory, conn, path) = local_settings();
    let result = write_claude_config_toggle_to_conn(&conn, "enableToolSearch", false).unwrap();
    assert!(!result.enable_tool_search);
    assert!(!path.exists());
    for (source, enabled) in [
        ("{ /* keep */ \"env\":null, \"other\":2, }", false),
        ("{ /* keep */ \"env\":{}, \"other\":2, }", false),
        ("{\"env\":{\"OTHER\":\"keep\"},\"other\":1e3}", false),
        (
            "{\"env\":{\"ENABLE_TOOL_SEARCH\":\"true\"},\"other\":1e3}",
            true,
        ),
    ] {
        std::fs::write(&path, source).unwrap();
        let before = std::fs::metadata(&path).unwrap().modified().unwrap();
        write_claude_config_toggle_to_conn(&conn, "enableToolSearch", enabled).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), source);
        assert_eq!(
            std::fs::metadata(&path).unwrap().modified().unwrap(),
            before
        );
    }
}

#[test]
fn claude_toggle_unknown_keys_and_io_errors_leave_files_untouched() {
    let (_directory, conn, path) = local_settings();
    assert!(
        write_claude_config_toggle_to_conn(&conn, "synthetic-private-secret", true)
            .unwrap_err()
            .contains("Unknown")
    );
    assert!(!path.exists());
    std::fs::create_dir(&path).unwrap();
    assert!(write_claude_config_toggle_to_conn(&conn, "enableToolSearch", true).is_err());
    assert!(path.is_dir());
    assert!(read_claude_config_toggles_from_conn(&conn).is_err());
}
