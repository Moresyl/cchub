pub mod models;
pub mod schema;

use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::AppHandle;
use tauri::Manager;

pub struct DbState(pub Mutex<Connection>);

/// Record an activity log entry
pub fn record_activity(
    conn: &Connection,
    server_id: &str,
    request_type: &str,
    status: &str,
    latency_ms: Option<i64>,
) {
    let now = chrono::Utc::now().to_rfc3339();
    let _ = conn.execute(
        "INSERT INTO activity_logs (server_id, request_type, status, latency_ms, recorded_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![server_id, request_type, status, latency_ms, now],
    );
}

pub fn get_db_path(app_handle: &AppHandle) -> PathBuf {
    let app_dir = app_handle
        .path()
        .app_data_dir()
        .expect("failed to get app data dir");
    std::fs::create_dir_all(&app_dir).expect("failed to create app data dir");
    app_dir.join("cchub.db")
}

fn ensure_incremental_auto_vacuum(conn: &Connection) -> Result<(), rusqlite::Error> {
    let mode: i64 = conn.query_row("PRAGMA auto_vacuum;", [], |row| row.get(0))?;
    if mode != 2 {
        conn.execute_batch("PRAGMA auto_vacuum = INCREMENTAL; VACUUM;")?;
    }
    conn.execute_batch("PRAGMA incremental_vacuum;")?;
    Ok(())
}

pub fn init_db(app_handle: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let db_path = get_db_path(app_handle);
    let db_exists = db_path.exists();
    let conn = Connection::open(&db_path)?;

    // SQLite PRAGMA optimizations for local desktop usage
    conn.execute_batch("PRAGMA journal_mode = WAL;")?; // Write-Ahead Logging for better concurrency
    conn.execute_batch("PRAGMA foreign_keys = ON;")?; // Enforce foreign key constraints
    conn.execute_batch("PRAGMA busy_timeout = 5000;")?; // Wait up to 5s on locked DB instead of failing immediately
    conn.execute_batch("PRAGMA synchronous = NORMAL;")?; // Good balance of safety vs performance with WAL

    if !db_exists {
        conn.execute_batch("PRAGMA auto_vacuum = INCREMENTAL;")?; // Enable incremental vacuum for new DBs
    }

    schema::run_migrations(&conn)?;
    ensure_incremental_auto_vacuum(&conn)?;

    // Restore proxy setting from database
    if let Ok(proxy) = conn.query_row(
        "SELECT value FROM app_settings WHERE key = 'proxy_url'",
        [],
        |row| row.get::<_, String>(0),
    ) {
        if !proxy.trim().is_empty() {
            std::env::set_var("HTTP_PROXY", &proxy);
            std::env::set_var("HTTPS_PROXY", &proxy);
            std::env::set_var("http_proxy", &proxy);
            std::env::set_var("https_proxy", &proxy);
        }
    }

    app_handle.manage(DbState(Mutex::new(conn)));
    Ok(())
}
