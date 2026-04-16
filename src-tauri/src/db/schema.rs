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

CREATE TABLE IF NOT EXISTS app_settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

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
    input_tokens INTEGER DEFAULT 0,
    output_tokens INTEGER DEFAULT 0,
    cache_read_tokens INTEGER DEFAULT 0,
    cache_creation_tokens INTEGER DEFAULT 0,
    total_cost_usd TEXT DEFAULT '0',
    latency_ms INTEGER DEFAULT 0,
    status_code INTEGER DEFAULT 0,
    is_streaming INTEGER DEFAULT 0,
    error_message TEXT,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_proxy_request_logs_created_at
ON proxy_request_logs(created_at DESC);

CREATE INDEX IF NOT EXISTS idx_proxy_request_logs_tool_id_created_at
ON proxy_request_logs(tool_id, created_at DESC);

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

    Ok(())
}
