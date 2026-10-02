use rusqlite::Connection;

/// Return the CREATE TABLE SQL statements as a string (for backup export)
pub fn get_schema_sql() -> String {
    r#"
CREATE TABLE IF NOT EXISTS mcp_servers (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    package_name TEXT,
    version TEXT,
    transport TEXT DEFAULT 'stdio',
    command TEXT,
    args TEXT DEFAULT '[]',
    env TEXT DEFAULT '{}',
    status TEXT DEFAULT 'stopped',
    source TEXT DEFAULT 'local',
    config_path TEXT,
    installed_at TEXT,
    updated_at TEXT
);

CREATE TABLE IF NOT EXISTS plugins (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    source_url TEXT,
    version TEXT,
    installed_at TEXT,
    updated_at TEXT
);

CREATE TABLE IF NOT EXISTS skills (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    plugin_id TEXT,
    trigger_command TEXT,
    file_path TEXT,
    version TEXT,
    installed_at TEXT,
    source_url TEXT,
    baseline_sha256 TEXT,
    latest_sha256 TEXT,
    last_checked_at INTEGER,
    FOREIGN KEY (plugin_id) REFERENCES plugins(id)
);

CREATE TABLE IF NOT EXISTS hooks (
    id TEXT PRIMARY KEY,
    event TEXT NOT NULL,
    matcher TEXT,
    command TEXT NOT NULL,
    scope TEXT DEFAULT 'global',
    project_path TEXT,
    enabled INTEGER DEFAULT 1
);

CREATE TABLE IF NOT EXISTS update_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    item_type TEXT NOT NULL,
    item_id TEXT NOT NULL,
    old_version TEXT,
    new_version TEXT,
    status TEXT,
    updated_at TEXT
);

CREATE TABLE IF NOT EXISTS metrics (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    server_id TEXT NOT NULL,
    request_count INTEGER DEFAULT 0,
    error_count INTEGER DEFAULT 0,
    avg_latency_ms REAL,
    recorded_at TEXT,
    FOREIGN KEY (server_id) REFERENCES mcp_servers(id)
);

CREATE TABLE IF NOT EXISTS mcp_clients (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    config_path TEXT DEFAULT '',
    server_access TEXT DEFAULT '{}',
    created_at TEXT
);

CREATE TABLE IF NOT EXISTS activity_logs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    server_id TEXT NOT NULL,
    request_type TEXT DEFAULT 'request',
    status TEXT DEFAULT 'success',
    latency_ms INTEGER,
    recorded_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_activity_logs_recorded_at
ON activity_logs(recorded_at DESC);

CREATE TABLE IF NOT EXISTS workspaces (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    base_path TEXT,
    is_active INTEGER DEFAULT 0,
    created_at TEXT
);

CREATE TABLE IF NOT EXISTS custom_paths (
    tool_id TEXT PRIMARY KEY,
    config_dir TEXT,
    mcp_config_path TEXT,
    skills_dir TEXT
);

CREATE TABLE IF NOT EXISTS config_profiles (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    tool_id TEXT NOT NULL,
    config_snapshot TEXT NOT NULL,
    sort_order INTEGER DEFAULT 0,
    source_type TEXT DEFAULT 'manual',
    source_key TEXT,
    created_at TEXT,
    updated_at TEXT
);

CREATE TABLE IF NOT EXISTS project_profiles (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    snapshot TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    last_applied_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_project_profiles_updated_at
ON project_profiles(updated_at DESC, name COLLATE NOCASE ASC);

CREATE TABLE IF NOT EXISTS app_settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS prompt_library (
    app_id TEXT NOT NULL,
    id TEXT NOT NULL,
    name TEXT NOT NULL,
    content TEXT NOT NULL,
    description TEXT,
    enabled INTEGER NOT NULL DEFAULT 0 CHECK (enabled IN (0, 1)),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (app_id, id)
);

CREATE INDEX IF NOT EXISTS idx_prompt_library_app_updated_at
ON prompt_library(app_id, updated_at DESC);

CREATE UNIQUE INDEX IF NOT EXISTS idx_prompt_library_one_enabled_per_app
ON prompt_library(app_id) WHERE enabled = 1;

CREATE TABLE IF NOT EXISTS imported_project_files (
    project_root TEXT NOT NULL,
    relative_path TEXT NOT NULL,
    content_base64 TEXT NOT NULL,
    PRIMARY KEY (project_root, relative_path)
);

CREATE TABLE IF NOT EXISTS proxy_request_logs (
    request_id TEXT PRIMARY KEY,
    tool_id TEXT NOT NULL,
    profile_id TEXT NOT NULL,
    provider_name TEXT NOT NULL,
    request_model TEXT,
    response_model TEXT,
    upstream_model TEXT,
    input_tokens INTEGER DEFAULT 0,
    input_tokens_is_total INTEGER NOT NULL DEFAULT 0,
    output_tokens INTEGER DEFAULT 0,
    cache_read_tokens INTEGER DEFAULT 0,
    cache_creation_tokens INTEGER DEFAULT 0,
    total_cost_usd TEXT DEFAULT '0',
    latency_ms INTEGER DEFAULT 0,
    first_output_ms INTEGER,
    generation_ms INTEGER,
    status_code INTEGER DEFAULT 0,
    is_streaming INTEGER DEFAULT 0,
    error_message TEXT,
    stream_attempts_json TEXT NOT NULL DEFAULT '[]',
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_proxy_request_logs_created_at
ON proxy_request_logs(created_at DESC);

CREATE INDEX IF NOT EXISTS idx_proxy_request_logs_tool_id_created_at
ON proxy_request_logs(tool_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_proxy_request_logs_provider_created_at
ON proxy_request_logs(provider_name, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_proxy_request_logs_request_model_created_at
ON proxy_request_logs(request_model, created_at DESC);

-- 会话导入的持久化去重账本独立于明细日志。即使明细被清理，重写或分叉的
-- JSONL 也不会在下一次同步时重复计费。
CREATE TABLE IF NOT EXISTS session_usage_dedup (
    data_source TEXT NOT NULL,
    request_id TEXT NOT NULL,
    semantic_id TEXT NOT NULL,
    has_entry_id INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    PRIMARY KEY (data_source, request_id)
);

CREATE INDEX IF NOT EXISTS idx_session_usage_dedup_semantic
ON session_usage_dedup(data_source, semantic_id, has_entry_id);

CREATE TABLE IF NOT EXISTS model_pricing (
    model_id TEXT PRIMARY KEY,
    normalized_model_id TEXT NOT NULL,
    input_cost_per_million TEXT NOT NULL DEFAULT '0',
    output_cost_per_million TEXT NOT NULL DEFAULT '0',
    cache_read_cost_per_million TEXT NOT NULL DEFAULT '0',
    cache_write_cost_per_million TEXT NOT NULL DEFAULT '0',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_model_pricing_normalized_model_id
ON model_pricing(normalized_model_id);

CREATE TABLE IF NOT EXISTS proxy_usage_daily_rollups (
    day TEXT NOT NULL,
    tool_id TEXT NOT NULL,
    total_requests INTEGER NOT NULL DEFAULT 0,
    success_requests INTEGER NOT NULL DEFAULT 0,
    total_input_tokens INTEGER NOT NULL DEFAULT 0,
    total_output_tokens INTEGER NOT NULL DEFAULT 0,
    total_cache_read_tokens INTEGER NOT NULL DEFAULT 0,
    total_cache_creation_tokens INTEGER NOT NULL DEFAULT 0,
    total_cost_usd TEXT NOT NULL DEFAULT '0',
    avg_latency_ms REAL NOT NULL DEFAULT 0,
    updated_at TEXT NOT NULL,
    PRIMARY KEY (day, tool_id)
);

CREATE INDEX IF NOT EXISTS idx_proxy_usage_daily_rollups_day
ON proxy_usage_daily_rollups(day DESC, tool_id ASC);

-- tray menu refresh + Profiles 页都按 tool_id 过滤后再按 sort_order 排序，
-- 这个复合索引让"取某工具的 profiles 列表"完全走索引，避免全表扫。
CREATE INDEX IF NOT EXISTS idx_config_profiles_tool_id_sort_order
ON config_profiles(tool_id, sort_order);

-- activity_logs 在 mcp 健康检查、卸载 server 时会按 server_id 删除/统计
CREATE INDEX IF NOT EXISTS idx_activity_logs_server_id
ON activity_logs(server_id);

-- skills 表按 plugin_id 过滤插件下的 skill 列表
CREATE INDEX IF NOT EXISTS idx_skills_plugin_id
ON skills(plugin_id);

-- proxy_request_logs 按 profile_id 过滤（profile 详情页 / 删除 profile 级联清理）
CREATE INDEX IF NOT EXISTS idx_proxy_request_logs_profile_id
ON proxy_request_logs(profile_id);
"#
    .to_string()
}

pub fn run_migrations(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.execute_batch(&get_schema_sql())?;

    // Migration: add config_path column if not exists
    let _ = conn.execute_batch("ALTER TABLE mcp_servers ADD COLUMN config_path TEXT;");
    let _ =
        conn.execute_batch("ALTER TABLE config_profiles ADD COLUMN sort_order INTEGER DEFAULT 0;");
    let _ = conn
        .execute_batch("ALTER TABLE config_profiles ADD COLUMN source_type TEXT DEFAULT 'manual';");
    let _ = conn.execute_batch("ALTER TABLE config_profiles ADD COLUMN source_key TEXT;");
    let _ = conn.execute_batch("ALTER TABLE skills ADD COLUMN source_url TEXT;");
    let _ = conn.execute_batch("ALTER TABLE skills ADD COLUMN baseline_sha256 TEXT;");
    let _ = conn.execute_batch("ALTER TABLE skills ADD COLUMN latest_sha256 TEXT;");
    let _ = conn.execute_batch("ALTER TABLE skills ADD COLUMN last_checked_at INTEGER;");

    let log_columns = {
        let mut statement = conn.prepare("PRAGMA table_info(proxy_request_logs)")?;
        let columns = statement
            .query_map([], |row| row.get::<_, String>(1))?
            .collect::<Result<Vec<_>, _>>()?;
        columns
    };
    if !log_columns.iter().any(|column| column == "upstream_model") {
        conn.execute_batch("ALTER TABLE proxy_request_logs ADD COLUMN upstream_model TEXT;")?;
    }
    if !log_columns
        .iter()
        .any(|column| column == "input_tokens_is_total")
    {
        conn.execute_batch("ALTER TABLE proxy_request_logs ADD COLUMN input_tokens_is_total INTEGER NOT NULL DEFAULT 0;")?;
    }

    if !log_columns
        .iter()
        .any(|column| column == "stream_attempts_json")
    {
        conn.execute_batch("ALTER TABLE proxy_request_logs ADD COLUMN stream_attempts_json TEXT NOT NULL DEFAULT '[]';")?;
    }
    for column in ["first_output_ms", "generation_ms"] {
        if !log_columns.iter().any(|existing| existing == column) {
            conn.execute_batch(&format!(
                "ALTER TABLE proxy_request_logs ADD COLUMN {column} INTEGER;"
            ))?;
        }
    }
    seed_builtin_model_pricing(conn)?;

    Ok(())
}

fn seed_builtin_model_pricing(conn: &Connection) -> Result<(), rusqlite::Error> {
    let now = chrono::Utc::now().to_rfc3339();
    let builtins = [
        (
            "qwen3.8-max",
            "2.000000",
            "6.000000",
            "0.250000",
            "2.500000",
        ),
        (
            "claude-opus-5",
            "5.000000",
            "25.000000",
            "0.500000",
            "6.250000",
        ),
        (
            "gemini-3.6-flash",
            "1.500000",
            "7.500000",
            "0.150000",
            "0.000000",
        ),
        (
            "grok-4.5-build",
            "2.000000",
            "6.000000",
            "0.300000",
            "0.000000",
        ),
    ];
    for (model_id, input, output, cache_read, cache_write) in builtins {
        conn.execute(
            "INSERT OR IGNORE INTO model_pricing (
                model_id,
                normalized_model_id,
                input_cost_per_million,
                output_cost_per_million,
                cache_read_cost_per_million,
                cache_write_cost_per_million,
                created_at,
                updated_at
            ) VALUES (?1, ?1, ?2, ?3, ?4, ?5, ?6, ?6)",
            rusqlite::params![model_id, input, output, cache_read, cache_write, now],
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table_exists(conn: &Connection, table_name: &str) -> bool {
        conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
            [table_name],
            |row| row.get::<_, bool>(0),
        )
        .unwrap()
    }

    fn column_exists(conn: &Connection, table_name: &str, column_name: &str) -> bool {
        let mut stmt = conn
            .prepare(&format!("PRAGMA table_info({table_name})"))
            .unwrap();
        let columns = stmt
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        columns.iter().any(|column| column == column_name)
    }

    #[test]
    fn model_alias_column_migrates_old_logs_without_rewriting_their_identity() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(&get_schema_sql().replace("    upstream_model TEXT,\n", ""))
            .unwrap();
        conn.execute("INSERT INTO proxy_request_logs(request_id,tool_id,profile_id,provider_name,request_model,response_model,total_cost_usd,created_at) VALUES('old','claude','p1','Provider','core','actual','2.500000','2026-10-01')",[]).unwrap();
        run_migrations(&conn).unwrap();
        run_migrations(&conn).unwrap();
        let row:(String,String,String,Option<String>)=conn.query_row("SELECT request_model,response_model,total_cost_usd,upstream_model FROM proxy_request_logs WHERE request_id='old'",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).unwrap();
        assert_eq!(
            row,
            ("core".into(), "actual".into(), "2.500000".into(), None)
        );
    }

    #[test]
    fn cache_usage_marker_migrates_without_reinterpreting_old_counts_or_charges() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(&get_schema_sql().replace(
            "    input_tokens_is_total INTEGER NOT NULL DEFAULT 0,\n",
            "",
        ))
        .unwrap();
        conn.execute("INSERT INTO proxy_request_logs(request_id,tool_id,profile_id,provider_name,input_tokens,cache_read_tokens,cache_creation_tokens,total_cost_usd,created_at) VALUES('old','claude','p1','Provider',100,800,100,'2.500000','2026-10-01')",[]).unwrap();
        run_migrations(&conn).unwrap();
        run_migrations(&conn).unwrap();
        let row: (i64,i64,i64,i64,String) = conn.query_row("SELECT input_tokens,cache_read_tokens,cache_creation_tokens,input_tokens_is_total,total_cost_usd FROM proxy_request_logs WHERE request_id='old'",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))).unwrap();
        assert_eq!(row, (100, 800, 100, 0, "2.500000".into()));
    }

    #[test]
    fn stream_attempts_migrate_existing_logs_without_erasing_costs_or_later_details() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            &get_schema_sql().replace("    stream_attempts_json TEXT NOT NULL DEFAULT '[]',\n", ""),
        )
        .unwrap();
        conn.execute("INSERT INTO proxy_request_logs(request_id,tool_id,profile_id,provider_name,input_tokens,total_cost_usd,created_at) VALUES('old','claude','p1','Provider',100,'2.500000','2026-10-01')", []).unwrap();
        run_migrations(&conn).unwrap();
        let row: (i64, String, String) = conn.query_row("SELECT input_tokens,total_cost_usd,stream_attempts_json FROM proxy_request_logs WHERE request_id='old'", [], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap();
        assert_eq!(row, (100, "2.500000".into(), "[]".into()));
        conn.execute("UPDATE proxy_request_logs SET stream_attempts_json='[{\"attempt_id\":\"retained\"}]' WHERE request_id='old'", []).unwrap();
        run_migrations(&conn).unwrap();
        let ledger: String = conn
            .query_row(
                "SELECT stream_attempts_json FROM proxy_request_logs WHERE request_id='old'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(ledger, "[{\"attempt_id\":\"retained\"}]");
    }

    #[test]
    fn run_migrations_is_idempotent() {
        let conn = Connection::open_in_memory().unwrap();

        run_migrations(&conn).unwrap();
        run_migrations(&conn).unwrap();
        run_migrations(&conn).unwrap();

        assert!(table_exists(&conn, "app_settings"));
        assert!(table_exists(&conn, "config_profiles"));
        assert!(table_exists(&conn, "project_profiles"));
        assert!(table_exists(&conn, "proxy_request_logs"));
        assert!(column_exists(&conn, "proxy_request_logs", "upstream_model"));
        assert!(column_exists(
            &conn,
            "proxy_request_logs",
            "stream_attempts_json"
        ));
        assert!(table_exists(&conn, "session_usage_dedup"));
        assert!(column_exists(&conn, "mcp_servers", "config_path"));
        assert!(column_exists(&conn, "config_profiles", "source_type"));
        assert!(column_exists(&conn, "skills", "last_checked_at"));
    }

    #[test]
    fn stream_timing_migrates_without_fabricating_historical_measurements() {
        let conn = Connection::open_in_memory().unwrap();
        let old = get_schema_sql()
            .replace("    first_output_ms INTEGER,\n", "")
            .replace("    generation_ms INTEGER,\n", "");
        conn.execute_batch(&old).unwrap();
        conn.execute("INSERT INTO proxy_request_logs(request_id,tool_id,profile_id,provider_name,latency_ms,total_cost_usd,created_at) VALUES('old','claude','p1','Provider',1500,'2.500000','2026-10-01')", []).unwrap();
        run_migrations(&conn).unwrap();
        run_migrations(&conn).unwrap();
        let row: (i64, String, Option<i64>, Option<i64>) = conn.query_row(
            "SELECT latency_ms,total_cost_usd,first_output_ms,generation_ms FROM proxy_request_logs WHERE request_id='old'", [],
            |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?)),
        ).unwrap();
        assert_eq!(row, (1500, "2.500000".into(), None, None));
        conn.execute("UPDATE proxy_request_logs SET first_output_ms=120,generation_ms=320 WHERE request_id='old'", []).unwrap();
        run_migrations(&conn).unwrap();
        let timing: (i64, i64) = conn.query_row("SELECT first_output_ms,generation_ms FROM proxy_request_logs WHERE request_id='old'", [], |row| Ok((row.get(0)?,row.get(1)?))).unwrap();
        assert_eq!(timing, (120, 320));
    }

    #[test]
    fn builtin_pricing_is_seeded_without_overwriting_custom_values() {
        let conn = Connection::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();

        let initial: (String, String, String, String) = conn
            .query_row(
                "SELECT input_cost_per_million, output_cost_per_million, cache_read_cost_per_million, cache_write_cost_per_million FROM model_pricing WHERE model_id = 'qwen3.8-max'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(
            initial,
            (
                "2.000000".into(),
                "6.000000".into(),
                "0.250000".into(),
                "2.500000".into()
            )
        );

        for (model_id, input, output) in [
            ("claude-opus-5", "5.000000", "25.000000"),
            ("gemini-3.6-flash", "1.500000", "7.500000"),
            ("grok-4.5-build", "2.000000", "6.000000"),
        ] {
            let costs: (String, String) = conn
                .query_row(
                    "SELECT input_cost_per_million, output_cost_per_million FROM model_pricing WHERE model_id = ?1",
                    [model_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .unwrap();
            assert_eq!(costs, (input.to_string(), output.to_string()));
        }

        conn.execute(
            "UPDATE model_pricing SET input_cost_per_million = '9.000000' WHERE model_id = 'qwen3.8-max'",
            [],
        )
        .unwrap();
        run_migrations(&conn).unwrap();
        let custom: String = conn
            .query_row(
                "SELECT input_cost_per_million FROM model_pricing WHERE model_id = 'qwen3.8-max'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(custom, "9.000000");
    }
}
