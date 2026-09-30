use super::*;

fn fixture() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    crate::db::schema::run_migrations(&conn).unwrap();
    conn
}

fn dump(body: &str) -> String {
    format!("-- CCHub Database Backup (.sql)\n{}\n{}\nINSERT INTO _backup_meta VALUES ('version', '1.6.10');\n{body}", crate::db::schema::get_schema_sql(), BACKUP_TABLE_SCHEMA)
}

#[test]
fn exported_business_data_and_artifacts_round_trip() {
    let source = fixture();
    source
        .execute(
            "INSERT INTO app_settings VALUES ('sample', ?1)",
            ["中文 ' quoted\n-- not SQL"],
        )
        .unwrap();
    source
        .execute(
            "INSERT INTO workspaces (id,name,base_path) VALUES ('w','项目','C:/开发')",
            [],
        )
        .unwrap();
    source.execute("INSERT INTO config_profiles (id,name,tool_id,config_snapshot) VALUES ('p','配置','claude',?1)", ["{\"env\":{\"KEY\":\"secret\"}}"] ).unwrap();
    let mut body = String::new();
    super::super::backups_restore::append_backup_database_rows(&source, &mut body);
    body.push_str("INSERT INTO _tool_configs VALUES ('claude', 'ignored', '{}');\nINSERT INTO _skill_files (tool_id,name,content) VALUES ('pi','SKILL.md','中文');\nINSERT INTO _backup_files (root_key,relative_path,content_base64) VALUES ('skillsdir:pi','SKILL.md','YQ==');");
    let target = fixture();
    load_backup_sql(&target, &dump(&body)).unwrap();
    let actual: String = target
        .query_row(
            "SELECT value FROM app_settings WHERE key='sample'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(actual, "中文 ' quoted\n-- not SQL");
    assert_eq!(
        target
            .query_row("SELECT COUNT(*) FROM _backup_files", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        1
    );
    super::super::backup_paths::validate_artifact_paths(&target).unwrap();
}

#[test]
fn dangerous_instructions_roll_back_all_imported_data_and_remove_hooks() {
    for instruction in [
        "ATTACH ':memory:' AS other;",
        "PRAGMA writable_schema=ON;",
        "COMMIT;",
        "ROLLBACK;",
        "SAVEPOINT escape;",
        "CREATE TRIGGER attack AFTER INSERT ON app_settings BEGIN DELETE FROM app_settings; END;",
        "CREATE VIEW attack AS SELECT * FROM app_settings;",
        "CREATE VIRTUAL TABLE attack USING fts5(value);",
        "CREATE TABLE unknown (value TEXT);",
        "CREATE INDEX unknown ON app_settings(value);",
        "DROP TABLE app_settings;",
        "ALTER TABLE app_settings ADD COLUMN attack TEXT;",
        "SELECT load_extension('secret-value');",
        "INSERT INTO app_settings VALUES ('function', randomblob(1000000000));",
    ] {
        let target = fixture();
        let sql = dump(&format!(
            "INSERT INTO app_settings VALUES ('before','secret-value'); {instruction}"
        ));
        let error = load_backup_sql(&target, &sql).unwrap_err();
        assert!(!error.contains("secret-value"));
        assert_eq!(
            target
                .query_row("SELECT COUNT(*) FROM app_settings", [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            0,
            "{instruction}"
        );
        assert!(target.is_autocommit());
        // Trusted migrations must work after both success and rejected SQL.
        crate::db::schema::run_migrations(&target).unwrap();
    }
}

#[test]
fn external_databases_cannot_be_created_or_changed() {
    let dir = tempfile::tempdir().unwrap();
    let existing = dir.path().join("existing.db");
    let output = dir.path().join("outside.db");
    let outside = Connection::open(&existing).unwrap();
    outside
        .execute_batch("CREATE TABLE sentinel(value); INSERT INTO sentinel VALUES ('original');")
        .unwrap();
    for command in [
        format!(
            "ATTACH '{}' AS outside; DELETE FROM outside.sentinel;",
            existing.to_string_lossy().replace('\'', "''")
        ),
        format!(
            "VACUUM main INTO '{}';",
            output.to_string_lossy().replace('\'', "''")
        ),
    ] {
        assert!(load_backup_sql(&fixture(), &dump(&command)).is_err());
        assert!(!output.exists());
        assert_eq!(
            outside
                .query_row("SELECT value FROM sentinel", [], |r| r.get::<_, String>(0))
                .unwrap(),
            "original"
        );
    }
}

#[test]
fn missing_metadata_and_invalid_data_are_rejected_without_secret_errors() {
    let target = fixture();
    assert!(load_backup_sql(
        &target,
        "-- CCHub Database Backup (.sql)\nINSERT INTO app_settings VALUES ('before','secret');"
    )
    .is_err());
    assert_eq!(
        target
            .query_row("SELECT COUNT(*) FROM app_settings", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
    let error = load_backup_sql(
        &target,
        &dump("INSERT INTO app_settings VALUES ('secret-value');"),
    )
    .unwrap_err();
    assert!(!error.contains("secret-value"));
    assert!(load_backup_sql(&target, "-- arbitrary SQL\nSELECT 1;").is_err());
}

#[test]
fn foreign_schema_declarations_cannot_change_current_columns() {
    let target = fixture();
    let sql = "-- CCHub Database Backup (.sql)\nCREATE TABLE IF NOT EXISTS app_settings (attack TEXT);\nINSERT INTO _backup_meta VALUES ('version','1.0');\nINSERT INTO app_settings (key,value) VALUES ('safe','value');";
    load_backup_sql(&target, sql).unwrap();
    assert_eq!(
        target
            .query_row("SELECT value FROM app_settings WHERE key='safe'", [], |r| r
                .get::<_, String>(0))
            .unwrap(),
        "value"
    );
    crate::db::schema::run_migrations(&target).unwrap();
}

#[test]
fn expensive_queries_are_interrupted_and_the_transaction_is_rolled_back() {
    let target = fixture();
    // Enough VM instructions to exercise the actual SQLite progress hook.
    let values = (0..200)
        .map(|n| format!("('{n}','value')"))
        .collect::<Vec<_>>()
        .join(",");
    let content = dump(&format!("INSERT INTO app_settings VALUES {values}; SELECT a.key FROM app_settings a, app_settings b, app_settings c;"));
    assert!(load_backup_sql_with_limit(&target, &content, Duration::ZERO).is_err());
    assert_eq!(
        target
            .query_row("SELECT COUNT(*) FROM app_settings", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert!(target.is_autocommit());
    crate::db::schema::run_migrations(&target).unwrap();
}
