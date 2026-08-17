//! Safe recovery for Codex session files found in managed SQL backups.

use base64::Engine;
use serde::Serialize;
use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};
use tauri::State;

use crate::db::DbState;

const MAX_BACKUP_BYTES: u64 = 64 * 1024 * 1024;
const MAX_FILES: usize = 20_000;
const MAX_SESSION_FILE_BYTES: u64 = 64 * 1024 * 1024;
const DEFAULT_HISTORY_PROVIDER: &str = "custom";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexHistoryRestoreResult {
    pub restored_jsonl_files: usize,
    pub restored_state_rows: usize,
    pub skipped_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexHistoryMigrationResult {
    pub source_provider_ids: Vec<String>,
    pub target_provider_id: String,
    pub migrated_jsonl_files: usize,
    pub migrated_state_rows: usize,
    pub backup_path: Option<String>,
    pub skipped_reason: Option<String>,
}

#[derive(Debug)]
struct BackupEntry {
    relative_path: String,
    content_base64: String,
}

fn is_history_file(path: &str) -> bool {
    let normalized = path.replace('\\', "/").to_ascii_lowercase();
    let extension = Path::new(&normalized)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    (normalized.contains("history")
        || normalized.contains("session")
        || normalized.contains("rollout")
        || normalized.contains("thread")
        || normalized
            .rsplit('/')
            .next()
            .is_some_and(|name| name.starts_with("state_")))
        && matches!(extension, "jsonl" | "json" | "sqlite" | "db")
}

fn safe_relative_path(value: &str) -> Option<PathBuf> {
    if value.contains(['\\', ':']) {
        return None;
    }
    let mut result = PathBuf::new();
    for component in Path::new(value).components() {
        match component {
            Component::Normal(part) => result.push(part),
            Component::CurDir => {}
            Component::Prefix(_) | Component::RootDir | Component::ParentDir => return None,
        }
    }
    (!result.as_os_str().is_empty()).then_some(result)
}

fn read_backup_entries(path: &Path) -> Result<Vec<BackupEntry>, String> {
    let metadata = std::fs::metadata(path).map_err(|error| error.to_string())?;
    if metadata.len() > MAX_BACKUP_BYTES {
        return Err(format!(
            "Backup exceeds the 64 MiB limit: {}",
            path.display()
        ));
    }
    let content = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
    let content = crate::commands::extra_commands::validate_sql_backup_content(&content)?;
    let connection = rusqlite::Connection::open_in_memory().map_err(|error| error.to_string())?;
    connection
        .execute_batch(content)
        .map_err(|error| error.to_string())?;
    let mut statement = connection
        .prepare(
            "SELECT relative_path, content_base64 FROM _backup_files
             WHERE root_key = 'tooldir:codex' ORDER BY id LIMIT ?1",
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([MAX_FILES as i64], |row| {
            Ok(BackupEntry {
                relative_path: row.get(0)?,
                content_base64: row.get(1)?,
            })
        })
        .map_err(|error| error.to_string())?;
    Ok(rows
        .filter_map(Result::ok)
        .filter(|entry| is_history_file(&entry.relative_path))
        .collect())
}

fn latest_backup_entries() -> Result<Option<(PathBuf, Vec<BackupEntry>)>, String> {
    let directory = crate::commands::extra_commands::managed_backups_dir()?;
    let read_dir = match std::fs::read_dir(&directory) {
        Ok(value) => value,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.to_string()),
    };
    let mut backups = read_dir
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("sql"))
        .collect::<Vec<_>>();
    backups.sort_by_key(|path| {
        std::fs::metadata(path)
            .and_then(|metadata| metadata.modified())
            .ok()
    });
    backups.reverse();
    for path in backups {
        if let Ok(entries) = read_backup_entries(&path) {
            if !entries.is_empty() {
                return Ok(Some((path, entries)));
            }
        }
    }
    Ok(None)
}

#[tauri::command]
pub fn has_codex_unify_history_backup() -> Result<bool, String> {
    Ok(latest_backup_entries()?.is_some())
}

#[tauri::command]
pub fn restore_codex_unified_history(
    db: State<'_, DbState>,
) -> Result<CodexHistoryRestoreResult, String> {
    let Some((backup_path, entries)) = latest_backup_entries()? else {
        return Ok(CodexHistoryRestoreResult {
            restored_jsonl_files: 0,
            restored_state_rows: 0,
            skipped_reason: Some(
                "No Codex session files were found in managed backups".to_string(),
            ),
        });
    };
    let root = {
        let conn = db.0.lock().map_err(|error| error.to_string())?;
        crate::commands::extra_commands::resolve_tool_config_dir(&conn, "codex")?
    };
    let safety_root = crate::commands::extra_commands::ensure_managed_backups_dir()?.join(format!(
        "codex-history-safety-{}",
        chrono::Utc::now().timestamp_millis()
    ));
    let mut restored_jsonl_files = 0usize;
    let mut restored_state_rows = 0usize;
    for entry in entries {
        let Some(relative) = safe_relative_path(&entry.relative_path) else {
            continue;
        };
        let target = root.join(&relative);
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(entry.content_base64)
            .map_err(|error| {
                format!(
                    "Invalid backup content in {}: {error}",
                    backup_path.display()
                )
            })?;
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        if target.exists() {
            let safety_target = safety_root.join(&relative);
            if let Some(parent) = safety_target.parent() {
                std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
            }
            std::fs::copy(&target, safety_target).map_err(|error| error.to_string())?;
        }
        crate::utils::atomic_write(&target, &bytes).map_err(|error| error.to_string())?;
        if target.extension().and_then(|value| value.to_str()) == Some("jsonl") {
            restored_jsonl_files += 1;
        } else {
            restored_state_rows += 1;
        }
    }
    Ok(CodexHistoryRestoreResult {
        restored_jsonl_files,
        restored_state_rows,
        skipped_reason: None,
    })
}

/// Re-bucket existing Codex sessions after a provider id was renamed or consolidated.
/// Every changed file/database is copied into a unique managed backup before mutation.
#[tauri::command(rename_all = "camelCase")]
pub fn migrate_codex_history(
    source_provider_ids: Option<Vec<String>>,
    target_provider_id: Option<String>,
    db: State<'_, DbState>,
) -> Result<CodexHistoryMigrationResult, String> {
    let target = normalize_provider_id(
        target_provider_id
            .as_deref()
            .unwrap_or(DEFAULT_HISTORY_PROVIDER),
    )?;
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    let codex_root = crate::commands::extra_commands::resolve_tool_config_dir(&conn, "codex")?;
    drop(conn);

    let sources = match source_provider_ids {
        Some(values) => normalize_explicit_provider_ids(values, &target)?,
        None => infer_history_provider_ids(&codex_root, &target),
    };
    if sources.is_empty() {
        return Ok(CodexHistoryMigrationResult {
            source_provider_ids: Vec::new(),
            target_provider_id: target,
            migrated_jsonl_files: 0,
            migrated_state_rows: 0,
            backup_path: None,
            skipped_reason: Some("no_source_provider_ids".to_string()),
        });
    }

    let backup_root = crate::commands::extra_commands::ensure_managed_backups_dir()?
        .join(format!("codex-history-migration-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&backup_root).map_err(|error| error.to_string())?;
    let migrated_jsonl_files =
        migrate_codex_jsonl_files(&codex_root, &sources, &target, &backup_root)?;
    let migrated_state_rows =
        migrate_codex_state_databases(&codex_root, &sources, &target, &backup_root)?;

    if migrated_jsonl_files == 0 && migrated_state_rows == 0 {
        let _ = std::fs::remove_dir_all(&backup_root);
        return Ok(CodexHistoryMigrationResult {
            source_provider_ids: sources,
            target_provider_id: target,
            migrated_jsonl_files: 0,
            migrated_state_rows: 0,
            backup_path: None,
            skipped_reason: Some("nothing_to_migrate".to_string()),
        });
    }

    Ok(CodexHistoryMigrationResult {
        source_provider_ids: sources,
        target_provider_id: target,
        migrated_jsonl_files,
        migrated_state_rows,
        backup_path: Some(backup_root.to_string_lossy().to_string()),
        skipped_reason: None,
    })
}

fn normalize_provider_id(value: &str) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty() || value.len() > 128 || value == "." || value == ".." {
        return Err("Invalid Codex provider id".to_string());
    }
    if value
        .chars()
        .any(|character| character.is_control() || matches!(character, '/' | '\\' | ':'))
    {
        return Err("Invalid Codex provider id".to_string());
    }
    Ok(value.to_string())
}

fn normalize_provider_ids(values: Vec<String>, target: &str) -> Vec<String> {
    let mut ids = BTreeSet::new();
    for value in values {
        if let Ok(value) = normalize_provider_id(&value) {
            if value != target {
                ids.insert(value);
            }
        }
    }
    ids.into_iter().collect()
}

fn normalize_explicit_provider_ids(
    values: Vec<String>,
    target: &str,
) -> Result<Vec<String>, String> {
    let mut ids = BTreeSet::new();
    for value in values {
        let value = normalize_provider_id(&value)?;
        if value != target {
            ids.insert(value);
        }
    }
    Ok(ids.into_iter().collect())
}

fn infer_history_provider_ids(codex_root: &Path, target: &str) -> Vec<String> {
    let config_path = codex_root.join("config.toml");
    let Ok(content) = std::fs::read_to_string(config_path) else {
        return Vec::new();
    };
    let Ok(document) = content.parse::<toml_edit::DocumentMut>() else {
        return Vec::new();
    };
    let mut ids = Vec::new();
    if let Some(table) = document
        .get("model_providers")
        .and_then(|item| item.as_table_like())
    {
        for (id, _) in table.iter() {
            let id = id.trim();
            if !id.eq_ignore_ascii_case("openai") && !id.eq_ignore_ascii_case(target) {
                ids.push(id.to_string());
            }
        }
    }
    normalize_provider_ids(ids, target)
}

fn collect_codex_jsonl_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for directory in [root.join("sessions"), root.join("archived_sessions")] {
        collect_codex_files(&directory, &mut files, 0, 10);
    }
    files
}

fn collect_codex_files(directory: &Path, files: &mut Vec<PathBuf>, depth: u8, max_depth: u8) {
    if depth > max_depth {
        return;
    }
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_codex_files(&path, files, depth + 1, max_depth);
        } else if path.extension().and_then(|value| value.to_str()) == Some("jsonl") {
            files.push(path);
        }
    }
}

fn migrate_codex_jsonl_files(
    root: &Path,
    sources: &[String],
    target: &str,
    backup_root: &Path,
) -> Result<usize, String> {
    let source_ids = sources.iter().collect::<std::collections::HashSet<_>>();
    let mut changed_files = 0;
    for path in collect_codex_jsonl_files(root) {
        let metadata = std::fs::metadata(&path).map_err(|error| error.to_string())?;
        if metadata.len() > MAX_SESSION_FILE_BYTES {
            continue;
        }
        let content = std::fs::read_to_string(&path).map_err(|error| error.to_string())?;
        let mut rewritten = String::with_capacity(content.len());
        let mut changed = false;
        for segment in content.split_inclusive('\n') {
            let (line, newline) = segment
                .strip_suffix('\n')
                .map(|line| (line, "\n"))
                .unwrap_or((segment, ""));
            let next = rewrite_history_meta_line(line, &source_ids, target);
            if next.is_some() {
                changed = true;
                rewritten.push_str(&next.unwrap_or_default());
            } else {
                rewritten.push_str(line);
            }
            rewritten.push_str(newline);
        }
        if !changed {
            continue;
        }
        let relative = path
            .strip_prefix(root)
            .map_err(|_| "Codex session path escaped config directory".to_string())?;
        let backup_path = backup_root.join("jsonl").join(relative);
        if let Some(parent) = backup_path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        std::fs::copy(&path, &backup_path).map_err(|error| error.to_string())?;
        let current = std::fs::metadata(&path).map_err(|error| error.to_string())?;
        if current.len() != metadata.len() || current.modified().ok() != metadata.modified().ok() {
            return Err(format!(
                "Codex session changed during migration: {}",
                path.display()
            ));
        }
        crate::utils::atomic_write(&path, rewritten.as_bytes())
            .map_err(|error| error.to_string())?;
        changed_files += 1;
    }
    Ok(changed_files)
}

fn rewrite_history_meta_line(
    line: &str,
    source_ids: &std::collections::HashSet<&String>,
    target: &str,
) -> Option<String> {
    if !line.contains("\"session_meta\"") || !line.contains("\"model_provider\"") {
        return None;
    }
    let mut value = serde_json::from_str::<serde_json::Value>(line).ok()?;
    if value.get("type").and_then(serde_json::Value::as_str) != Some("session_meta") {
        return None;
    }
    let payload = value.get_mut("payload")?.as_object_mut()?;
    let provider = payload.get("model_provider")?.as_str()?;
    if !source_ids.iter().any(|source| source.as_str() == provider) {
        return None;
    }
    payload.insert(
        "model_provider".to_string(),
        serde_json::Value::String(target.to_string()),
    );
    serde_json::to_string(&value).ok()
}

fn migrate_codex_state_databases(
    root: &Path,
    sources: &[String],
    target: &str,
    backup_root: &Path,
) -> Result<usize, String> {
    let mut paths = Vec::new();
    if let Ok(entries) = std::fs::read_dir(root) {
        paths.extend(entries.flatten().map(|entry| entry.path()).filter(|path| {
            let name = path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or_default();
            path.is_file() && name.starts_with("state_") && name.ends_with(".sqlite")
        }));
    }
    let fallback = root.join("state.sqlite");
    if fallback.is_file() {
        paths.push(fallback);
    }
    let mut migrated = 0;
    for path in paths {
        let mut connection =
            rusqlite::Connection::open(&path).map_err(|error| error.to_string())?;
        connection
            .busy_timeout(std::time::Duration::from_secs(5))
            .map_err(|error| error.to_string())?;
        if !sqlite_has_column(&connection, "threads", "model_provider")? {
            continue;
        }
        let placeholders = std::iter::repeat_n("?", sources.len())
            .collect::<Vec<_>>()
            .join(",");
        let count_sql =
            format!("SELECT COUNT(*) FROM threads WHERE model_provider IN ({placeholders})");
        let count: i64 = connection
            .query_row(
                &count_sql,
                rusqlite::params_from_iter(sources.iter()),
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        if count == 0 {
            continue;
        }
        let relative = path
            .strip_prefix(root)
            .map_err(|_| "Codex state DB path escaped config directory".to_string())?;
        let backup_path = backup_root.join("state").join(relative);
        if let Some(parent) = backup_path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        std::fs::copy(&path, &backup_path).map_err(|error| error.to_string())?;
        let mut values = Vec::with_capacity(sources.len() + 1);
        values.push(target.to_string());
        values.extend(sources.iter().cloned());
        let update_sql = format!(
            "UPDATE threads SET model_provider = ? WHERE model_provider IN ({placeholders})"
        );
        let transaction = connection
            .transaction()
            .map_err(|error| error.to_string())?;
        migrated += transaction
            .execute(&update_sql, rusqlite::params_from_iter(values.iter()))
            .map_err(|error| error.to_string())?;
        transaction.commit().map_err(|error| error.to_string())?;
    }
    Ok(migrated)
}

fn sqlite_has_column(
    connection: &rusqlite::Connection,
    table: &str,
    column: &str,
) -> Result<bool, String> {
    let mut statement = connection
        .prepare(&format!("PRAGMA table_info({table})"))
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|error| error.to_string())?;
    let has_column = rows.flatten().any(|value| value == column);
    Ok(has_column)
}

#[cfg(test)]
mod tests {
    use super::{
        is_history_file, normalize_provider_ids, rewrite_history_meta_line, safe_relative_path,
    };
    use std::collections::HashSet;
    use tempfile::tempdir;

    #[test]
    fn history_filter_requires_a_supported_file() {
        assert!(is_history_file("sessions/rollout.jsonl"));
        assert!(is_history_file("state_123.sqlite"));
        assert!(!is_history_file("config.toml"));
    }

    #[test]
    fn backup_paths_cannot_escape_root() {
        assert!(safe_relative_path("sessions/a.jsonl").is_some());
        assert!(safe_relative_path("../outside.jsonl").is_none());
        assert!(safe_relative_path("C:\\outside.jsonl").is_none());
    }

    #[test]
    fn history_meta_rewrites_only_selected_provider() {
        let source = "legacy".to_string();
        let source_ids = HashSet::from([&source]);
        let line = r#"{"type":"session_meta","payload":{"id":"s1","model_provider":"legacy"}}"#;
        let rewritten = rewrite_history_meta_line(line, &source_ids, "custom").expect("rewrite");
        assert!(rewritten.contains("\"model_provider\":\"custom\""));
        assert!(rewrite_history_meta_line(line, &HashSet::new(), "custom").is_none());
    }

    #[test]
    fn provider_ids_are_deduplicated_and_target_is_excluded() {
        assert_eq!(
            normalize_provider_ids(
                vec![
                    "legacy".to_string(),
                    "custom".to_string(),
                    "legacy".to_string()
                ],
                "custom",
            ),
            vec!["legacy".to_string()]
        );
    }

    #[test]
    fn state_database_migration_is_transactional_and_backed_up() {
        let root = tempdir().expect("codex root");
        let backup = tempdir().expect("backup root");
        let state_path = root.path().join("state_1.sqlite");
        let connection = rusqlite::Connection::open(&state_path).expect("state db");
        connection
            .execute_batch(
                "CREATE TABLE threads (id TEXT PRIMARY KEY, model_provider TEXT);
                 INSERT INTO threads VALUES ('legacy-thread', 'legacy');
                 INSERT INTO threads VALUES ('official-thread', 'openai');",
            )
            .expect("seed state db");
        drop(connection);

        let migrated = super::migrate_codex_state_databases(
            root.path(),
            &["legacy".to_string()],
            "custom",
            backup.path(),
        )
        .expect("migrate state db");
        assert_eq!(migrated, 1);
        assert!(backup.path().join("state/state_1.sqlite").exists());

        let connection = rusqlite::Connection::open(state_path).expect("reopen state db");
        let provider: String = connection
            .query_row(
                "SELECT model_provider FROM threads WHERE id = 'legacy-thread'",
                [],
                |row| row.get(0),
            )
            .expect("read migrated row");
        assert_eq!(provider, "custom");
    }
}
