//! Safe recovery for Codex session files found in managed SQL backups.

use base64::Engine;
use serde::Serialize;
use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};
use tauri::State;

use crate::commands::extra_commands::session_file_tasks;
use crate::db::DbState;

mod migration;
mod mutation;
mod recovery;

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
pub async fn migrate_codex_history(
    source_provider_ids: Option<Vec<String>>,
    target_provider_id: Option<String>,
    expected_revision: Option<String>,
    db: State<'_, DbState>,
) -> Result<CodexHistoryMigrationResult, String> {
    let permit = session_file_tasks::mutation_permit().await;
    let root = {
        let conn = db.0.lock().map_err(|_| "配置数据库当前不可用")?;
        crate::commands::extra_commands::resolve_tool_config_dir(&conn, "codex")?
    };
    session_file_tasks::mutate(permit, move || {
        prepare_migration(root, source_provider_ids, target_provider_id)?.execute(
            &crate::commands::extra_commands::managed_backups_dir()?,
            expected_revision.as_deref(),
        )
    })
    .await
}

#[tauri::command(rename_all = "camelCase")]
pub async fn preview_codex_history_migration(
    source_provider_ids: Option<Vec<String>>,
    target_provider_id: Option<String>,
    db: State<'_, DbState>,
) -> Result<migration::MigrationPreview, String> {
    let permit = session_file_tasks::mutation_permit().await;
    let root = {
        let conn = db.0.lock().map_err(|_| "配置数据库当前不可用")?;
        crate::commands::extra_commands::resolve_tool_config_dir(&conn, "codex")?
    };
    session_file_tasks::mutate(permit, move || {
        Ok(prepare_migration(root, source_provider_ids, target_provider_id)?.preview())
    })
    .await
}

fn prepare_migration(
    root: PathBuf,
    sources: Option<Vec<String>>,
    target: Option<String>,
) -> Result<migration::MigrationPlan, String> {
    let target = normalize_provider_id(target.as_deref().unwrap_or(DEFAULT_HISTORY_PROVIDER))?;
    let sources = match sources {
        Some(values) => {
            if values.len() > 128 {
                return Err("单次迁移来源超过限制".into());
            }
            normalize_explicit_provider_ids(values, &target)?
        }
        None => infer_history_provider_ids(&root, &target)?,
    };
    migration::MigrationPlan::prepare(root, sources, target)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn list_codex_history_migration_backups(
    db: State<'_, DbState>,
) -> Result<Vec<recovery::BackupSummary>, String> {
    let permit = session_file_tasks::mutation_permit().await;
    let root = {
        let conn = db.0.lock().map_err(|_| "配置数据库当前不可用")?;
        crate::commands::extra_commands::resolve_tool_config_dir(&conn, "codex")?
    };
    session_file_tasks::mutate(permit, move || {
        recovery::list(
            &root,
            &crate::commands::extra_commands::managed_backups_dir()?,
        )
    })
    .await
}

#[tauri::command(rename_all = "camelCase")]
pub async fn preview_codex_history_restore(
    backup_key: String,
    db: State<'_, DbState>,
) -> Result<recovery::RestorePreview, String> {
    let permit = session_file_tasks::mutation_permit().await;
    let root = {
        let conn = db.0.lock().map_err(|_| "配置数据库当前不可用")?;
        crate::commands::extra_commands::resolve_tool_config_dir(&conn, "codex")?
    };
    session_file_tasks::mutate(permit, move || {
        Ok(recovery::RestorePlan::prepare(
            root,
            &crate::commands::extra_commands::managed_backups_dir()?,
            backup_key,
        )?
        .preview())
    })
    .await
}

#[tauri::command(rename_all = "camelCase")]
pub async fn restore_codex_history_migration(
    backup_key: String,
    expected_revision: String,
    selected_keys: Vec<String>,
    db: State<'_, DbState>,
) -> Result<recovery::RestoreResult, String> {
    let permit = session_file_tasks::mutation_permit().await;
    let root = {
        let conn = db.0.lock().map_err(|_| "配置数据库当前不可用")?;
        crate::commands::extra_commands::resolve_tool_config_dir(&conn, "codex")?
    };
    session_file_tasks::mutate(permit, move || {
        let backups = crate::commands::extra_commands::managed_backups_dir()?;
        recovery::RestorePlan::prepare(root, &backups, backup_key)?.execute(
            &backups,
            &expected_revision,
            selected_keys,
        )
    })
    .await
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

#[cfg(test)]
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

fn infer_history_provider_ids(codex_root: &Path, target: &str) -> Result<Vec<String>, String> {
    let config_path = codex_root.join("config.toml");
    if !config_path
        .try_exists()
        .map_err(|_| "无法检查 Codex 配置")?
    {
        return Ok(Vec::new());
    }
    if !crate::shared::session_archive::confined(&config_path, codex_root) {
        return Err("Codex 配置路径包含链接或越界".into());
    }
    let content = migration::read_file(&config_path, 1024 * 1024)?;
    let content = std::str::from_utf8(&content).map_err(|_| "Codex 配置不是有效 UTF-8")?;
    let document = content
        .parse::<toml_edit::DocumentMut>()
        .map_err(|_| "Codex 配置 TOML 无效，请修复后重试")?;
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
    if ids.len() > 128 {
        return Err("单次迁移来源超过限制".into());
    }
    normalize_explicit_provider_ids(ids, target)
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

        let result = super::migration::MigrationPlan::prepare(
            root.path().to_path_buf(),
            vec!["legacy".into()],
            "custom".into(),
        )
        .expect("prepare state migration")
        .execute(backup.path(), None)
        .expect("migrate state db");
        assert_eq!(result.migrated_state_rows, 1);
        assert!(std::path::Path::new(&result.backup_path.unwrap())
            .join("state/state_1.sqlite")
            .exists());

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
