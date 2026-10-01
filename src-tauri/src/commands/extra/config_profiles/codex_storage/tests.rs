use super::super::write_codex_structured_config_to_text;
use super::*;

fn fixture() -> (tempfile::TempDir, std::path::PathBuf, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("config.toml");
    let auth = dir.path().join("auth.json");
    std::fs::write(
        &config,
        "# 保留\nmodel = \"old\"\n[mcp_servers.keep]\ncommand = \"node\"\n",
    )
    .unwrap();
    std::fs::write(&auth, "\u{feff}{\r\n // 保留认证\r\n \"OPENAI_API_KEY\":\"old-key\", \"tokens\":{\"refresh\":\"opaque\"}, \"n\":1.2300e+4,\r\n}\r\n").unwrap();
    (dir, config, auth)
}

#[test]
fn structured_save_preserves_toml_and_auth_fields_and_returns_the_committed_revision() {
    let (_dir, config_path, auth_path) = fixture();
    let initial = read_codex_structured_files(&config_path, &auth_path).unwrap();
    let mut edited = initial.config;
    edited.model = "new-model".into();
    edited.api_key = "new-key".into();
    let draft = initial
        .content
        .replacen("model = \"old\"", "model = \"new-model\"", 1);
    let result = write_codex_structured_files(
        &config_path,
        &auth_path,
        &draft,
        &edited.api_key,
        &initial.file_revision,
    )
    .unwrap();
    assert!(result.content.contains("# 保留"));
    assert!(result.content.contains("[mcp_servers.keep]"));
    let source = std::fs::read_to_string(&auth_path).unwrap();
    for retained in [
        "\u{feff}",
        "// 保留认证\r\n",
        "\"tokens\":{\"refresh\":\"opaque\"}",
        "1.2300e+4",
    ] {
        assert!(source.contains(retained), "{source}");
    }
    assert!(!source.contains("old-key"));
    let next = read_codex_structured_files(&config_path, &auth_path).unwrap();
    assert_eq!(next.config.api_key, "new-key");
    assert_eq!(next.config.model, "new-model");
    assert_eq!(result.content, next.content);
    assert_eq!(result.file_revision, next.file_revision);
    edited.api_key.clear();
    write_codex_structured_files(
        &config_path,
        &auth_path,
        &next.content,
        &edited.api_key,
        &next.file_revision,
    )
    .unwrap();
    let auth = crate::json_config::parse_json_object(&std::fs::read_to_string(auth_path).unwrap())
        .unwrap();
    assert!(auth.get("OPENAI_API_KEY").is_none());
    assert_eq!(auth["tokens"]["refresh"], "opaque");
}

#[test]
fn either_file_changing_after_read_stops_the_save_without_replacing_any_file() {
    for authentication in [false, true] {
        let (_dir, config_path, auth_path) = fixture();
        let read = read_codex_structured_files(&config_path, &auth_path).unwrap();
        let changed = if authentication {
            &auth_path
        } else {
            &config_path
        };
        std::fs::write(changed, b"external update").unwrap();
        let config_before = std::fs::read(&config_path).unwrap();
        let auth_before = std::fs::read(&auth_path).unwrap();
        let error = write_codex_structured_files(
            &config_path,
            &auth_path,
            &read.content,
            &read.config.api_key,
            &read.file_revision,
        )
        .unwrap_err();
        assert!(error.contains("externally"));
        assert_eq!(std::fs::read(config_path).unwrap(), config_before);
        assert_eq!(std::fs::read(auth_path).unwrap(), auth_before);
    }
}

#[test]
fn malformed_auth_and_invalid_toml_never_leave_partial_config_writes() {
    let (_dir, config_path, auth_path) = fixture();
    let config_before = std::fs::read(&config_path).unwrap();
    let edited = read_codex_structured_config_from_content(
        std::str::from_utf8(&config_before).unwrap(),
        "new-key".into(),
    );
    for source in [
        "",
        "[]",
        "null",
        "{\"synthetic-private-secret\":",
        "{\"tokens\":1,\"tokens\":2}",
        "{} {}",
    ] {
        std::fs::write(&auth_path, source).unwrap();
        assert!(read_codex_structured_files(&config_path, &auth_path).is_err());
        let version = revision(Some(&config_before), Some(source.as_bytes()));
        let error = write_codex_structured_files(
            &config_path,
            &auth_path,
            "model = \"new\"",
            &edited.api_key,
            &version,
        )
        .unwrap_err();
        assert!(!error.contains("synthetic-private-secret"));
        assert_eq!(std::fs::read(&config_path).unwrap(), config_before);
        assert_eq!(std::fs::read_to_string(&auth_path).unwrap(), source);
    }
    std::fs::write(&auth_path, "{}").unwrap();
    let version = revision(Some(&config_before), Some(b"{}"));
    for source in [
        "model = [",
        "model_providers = 1",
        "[model_providers]\ncustom = 1",
        "mcp_servers = 1",
    ] {
        assert!(write_codex_structured_files(
            &config_path,
            &auth_path,
            source,
            &edited.api_key,
            &version
        )
        .is_err());
        assert_eq!(std::fs::read(&config_path).unwrap(), config_before);
        assert_eq!(std::fs::read_to_string(&auth_path).unwrap(), "{}");
    }
}

#[test]
fn first_use_writes_both_files_and_a_repeated_save_keeps_timestamps() {
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("new/nested/config.toml");
    let auth = dir.path().join("new/nested/auth.json");
    let initial = read_codex_structured_files(&config, &auth).unwrap();
    let written = write_codex_structured_files(
        &config,
        &auth,
        "",
        &initial.config.api_key,
        &initial.file_revision,
    )
    .unwrap();
    let next = read_codex_structured_files(&config, &auth).unwrap();
    let before = [
        std::fs::metadata(&config).unwrap().modified().unwrap(),
        std::fs::metadata(&auth).unwrap().modified().unwrap(),
    ];
    assert_eq!(written.file_revision, next.file_revision);
    write_codex_structured_files(
        &config,
        &auth,
        &next.content,
        &next.config.api_key,
        &next.file_revision,
    )
    .unwrap();
    assert_eq!(
        std::fs::metadata(config).unwrap().modified().unwrap(),
        before[0]
    );
    assert_eq!(
        std::fs::metadata(auth).unwrap().modified().unwrap(),
        before[1]
    );
}

#[test]
fn api_key_only_edit_preserves_every_toml_byte_and_timestamp() {
    let (_dir, config_path, auth_path) = fixture();
    let source = "model_provider = 'vendor.name' # owner\nmodel = '' # empty\nmodel_providers = { 'vendor.name' = { base_url = 'https://fixture.test/a=b', name = 'Relay # 1' } }\n";
    std::fs::write(&config_path, source).unwrap();
    let initial = read_codex_structured_files(&config_path, &auth_path).unwrap();
    let before = std::fs::metadata(&config_path).unwrap().modified().unwrap();
    let saved = write_codex_structured_files(
        &config_path,
        &auth_path,
        source,
        "new-key",
        &initial.file_revision,
    )
    .unwrap();
    assert_eq!(saved.content, source);
    assert_eq!(std::fs::read_to_string(&config_path).unwrap(), source);
    assert_eq!(
        std::fs::metadata(&config_path).unwrap().modified().unwrap(),
        before
    );
    assert_eq!(
        read_codex_structured_files(&config_path, &auth_path)
            .unwrap()
            .config
            .api_key,
        "new-key"
    );
}

#[test]
fn codex_reader_uses_exact_root_keys_and_decodes_quoted_provider_keys_and_inline_tables() {
    let source = "model_provider = 'vendor.name' # owner\nmodel = '' # empty model\nmodel_context_window = 0x100000 # budget\nmodel_providers = { 'vendor.name' = { name = 'Relay # 1', base_url = 'https://fixture.test/a=b', wire_api = 'responses' } }\nmcp_servers = { keep = { command = 'node' } }\n[other]\nmodel = 'unrelated-model'\n";
    let read = read_codex_structured_config_from_content(source, String::new());
    assert_eq!(read.model_provider, "vendor.name");
    assert_eq!(read.model, "");
    assert_eq!(read.provider_label, "Relay # 1");
    assert_eq!(read.base_url, "https://fixture.test/a=b");
    assert_eq!(read.model_context_window, "1048576");
    assert_eq!(read.mcp_servers, ["keep"]);
    assert!(!read.malformed_mcp_servers);
    let written = write_codex_structured_config_to_text(source, &read);
    for retained in [
        "# owner",
        "# empty model",
        "0x100000 # budget",
        "command = 'node'",
        "model = 'unrelated-model'",
    ] {
        assert!(written.contains(retained), "{written}");
    }
    assert_eq!(
        super::super::parse_toml_assignment("model_provider = 'custom'\nmodel = ''", "model"),
        None
    );
}
