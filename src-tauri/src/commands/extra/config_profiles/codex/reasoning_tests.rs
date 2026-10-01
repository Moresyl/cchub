use super::*;

#[test]
fn reasoning_default_is_omitted_in_read_and_both_native_write_paths() {
    for source in [
        "model = 'keep'\n",
        "model_reasoning_effort = 'high'\nmodel = 'keep'\n",
    ] {
        let mut config = read_codex_structured_config_from_content(source, String::new());
        config.reasoning_effort.clear();
        let written = write_codex_structured_config_to_text(source, &config);
        let doc = written.parse::<toml_edit::DocumentMut>().unwrap();
        assert!(doc.get("model_reasoning_effort").is_none());
        assert_eq!(doc["model"].as_str(), Some("keep"));
        assert_eq!(
            read_codex_structured_config_from_content(&written, String::new()).reasoning_effort,
            ""
        );
        let mut settings = source.parse::<toml_edit::DocumentMut>().unwrap();
        set_codex_reasoning_effort(&mut settings, " ");
        assert!(settings.get("model_reasoning_effort").is_none());
        assert_eq!(settings["model"].as_str(), Some("keep"));
    }
    assert_eq!(
        read_codex_structured_config_from_content("", String::new()).reasoning_effort,
        ""
    );
}

#[test]
fn custom_and_explicit_none_levels_preserve_comments_and_never_mean_omission() {
    for level in ["none", "minimal", "max", "future-effort", "custom\"level"] {
        let source =
            "model_reasoning_effort = 'high' # keep\n[mcp_servers.keep]\ncommand = 'node'\n";
        let mut config = read_codex_structured_config_from_content(source, String::new());
        config.reasoning_effort = level.into();
        let written = write_codex_structured_config_to_text(source, &config);
        assert_eq!(
            read_codex_structured_config_from_content(&written, String::new()).reasoning_effort,
            level
        );
        assert!(written.contains("# keep"));
        assert!(written.contains("command = 'node'"));
        let mut settings = source.parse::<toml_edit::DocumentMut>().unwrap();
        set_codex_reasoning_effort(&mut settings, level);
        assert_eq!(settings["model_reasoning_effort"].as_str(), Some(level));
    }
}
