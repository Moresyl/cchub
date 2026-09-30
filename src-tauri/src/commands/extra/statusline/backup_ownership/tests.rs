use super::*;

fn fixture() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    crate::db::schema::run_migrations(&conn).unwrap();
    conn
}

#[test]
fn cloud_settings_preserve_exact_device_state_and_import_only_shared_libraries() {
    let live = fixture();
    let imported = fixture();
    for (key, local, remote) in [
        (
            "webdav_sync_settings",
            "local DAV scope",
            "foreign DAV scope",
        ),
        ("s3_sync_settings", "local S3 scope", "foreign S3 scope"),
        ("proxy_url", "local proxy", "foreign proxy"),
        (
            "usage_alert_state",
            "local rules and history",
            "foreign alerts",
        ),
        ("hermes.rootOverride", "local Hermes root", "foreign root"),
        (
            "claude_desktop_gateway_token",
            "local gateway secret",
            "foreign secret",
        ),
        (
            "future_device_setting",
            "local future value",
            "foreign future value",
        ),
        ("universal_providers", "local library", "remote library"),
        (
            "common_config_snippets",
            "local snippets",
            "remote snippets",
        ),
    ] {
        live.execute("INSERT INTO app_settings VALUES (?1,?2)", (key, local))
            .unwrap();
        imported
            .execute("INSERT INTO app_settings VALUES (?1,?2)", (key, remote))
            .unwrap();
    }
    imported
        .execute(
            "INSERT INTO app_settings VALUES ('new_device_secret','foreign secret')",
            [],
        )
        .unwrap();
    assert_eq!(preserve_device_state(&live, &imported).unwrap(), 7);
    for (key, expected) in [
        ("webdav_sync_settings", "local DAV scope"),
        ("s3_sync_settings", "local S3 scope"),
        ("proxy_url", "local proxy"),
        ("usage_alert_state", "local rules and history"),
        ("hermes.rootOverride", "local Hermes root"),
        ("claude_desktop_gateway_token", "local gateway secret"),
        ("future_device_setting", "local future value"),
        ("universal_providers", "remote library"),
        ("common_config_snippets", "remote snippets"),
    ] {
        assert_eq!(
            imported
                .query_row("SELECT value FROM app_settings WHERE key=?1", [key], |r| {
                    r.get::<_, String>(0)
                })
                .unwrap(),
            expected
        );
    }
    assert_eq!(
        imported
            .query_row(
                "SELECT COUNT(*) FROM app_settings WHERE key='new_device_secret'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        0
    );
}

#[test]
fn custom_paths_and_workspace_bindings_remain_local_with_nulls_preserved() {
    let live = fixture();
    let imported = fixture();
    live.execute_batch("INSERT INTO custom_paths VALUES ('codex','local config',NULL,'local skills'); INSERT INTO workspaces VALUES ('local','本机',NULL,'local project',1,NULL);").unwrap();
    imported.execute_batch("INSERT INTO custom_paths VALUES ('codex','foreign config','foreign mcp','foreign skills'); INSERT INTO custom_paths VALUES ('claude','foreign config',NULL,NULL); INSERT INTO workspaces VALUES ('local','远端','remote','foreign project',1,'created'); INSERT INTO workspaces VALUES ('remote','远端新项目',NULL,'foreign project',1,NULL);").unwrap();
    assert_eq!(preserve_device_state(&live, &imported).unwrap(), 2);
    assert_eq!(imported.query_row("SELECT config_dir,mcp_config_path,skills_dir FROM custom_paths WHERE tool_id='codex'",[],|r|Ok((r.get::<_,String>(0)?,r.get::<_,Option<String>>(1)?,r.get::<_,String>(2)?))).unwrap(),("local config".into(),None,"local skills".into()));
    assert_eq!(
        imported
            .query_row("SELECT COUNT(*) FROM custom_paths", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        imported
            .query_row(
                "SELECT base_path,is_active FROM workspaces WHERE id='local'",
                [],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
            )
            .unwrap(),
        ("local project".into(), 1)
    );
    assert_eq!(
        imported
            .query_row(
                "SELECT base_path,is_active FROM workspaces WHERE id='remote'",
                [],
                |r| Ok((r.get::<_, Option<String>>(0)?, r.get::<_, i64>(1)?))
            )
            .unwrap(),
        (None, 0)
    );
}

#[test]
fn a_new_device_does_not_import_remote_cloud_accounts_or_paths() {
    let live = fixture();
    let imported = fixture();
    imported.execute_batch("INSERT INTO app_settings VALUES ('s3_sync_settings','foreign secret'); INSERT INTO app_settings VALUES ('provider_config_fragments','shared fragments'); INSERT INTO custom_paths VALUES ('claude','foreign',NULL,NULL);").unwrap();
    assert_eq!(preserve_device_state(&live, &imported).unwrap(), 0);
    assert_eq!(
        imported
            .query_row("SELECT COUNT(*) FROM custom_paths", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        imported
            .query_row("SELECT COUNT(*) FROM app_settings", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        1
    );
}
