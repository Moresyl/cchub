use std::collections::HashSet;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};

use super::super::migration::{hash, read_file, MAX_STATE_DATABASES, MAX_STATE_ROWS};
use super::super::{normalize_provider_id, MAX_BACKUP_BYTES, MAX_FILES};
use crate::shared::session_archive as archive;

const PREFIX: &str = "codex-history-migration-";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Journal {
    pub version: u32,
    pub root: PathBuf,
    pub source_provider_ids: Vec<String>,
    pub target_provider_id: String,
    pub logs: Vec<LogEntry>,
    pub states: Vec<StateEntry>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct LogEntry {
    pub path: PathBuf,
    pub original_hash: String,
    pub migrated_hash: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct StateEntry {
    pub path: PathBuf,
    pub rows: Vec<(String, String)>,
    pub original_hash: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupSummary {
    pub key: String,
    pub created_at_seconds: u64,
    pub target_provider_id: Option<String>,
    pub log_files: usize,
    pub state_rows: usize,
    pub problem: Option<String>,
}

pub(super) fn valid_hash(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn valid_key(key: &str) -> bool {
    key.strip_prefix(PREFIX)
        .is_some_and(|id| uuid::Uuid::parse_str(id).is_ok_and(|parsed| parsed.to_string() == id))
}

pub(super) fn read_journal(
    backups: &Path,
    key: &str,
) -> Result<(PathBuf, Journal, String), String> {
    if !valid_key(key) {
        return Err("历史备份标识无效".into());
    }
    let generation = backups.join(key);
    let meta = fs::symlink_metadata(&generation).map_err(|_| "历史备份不存在")?;
    if !meta.is_dir() || archive::is_link(&meta) {
        return Err("历史备份目录包含链接或不是目录".into());
    }
    let path = generation.join("migration.json");
    if !archive::confined(&path, backups) {
        return Err("历史备份账本路径越界或包含链接".into());
    }
    let bytes = read_file(&path, MAX_BACKUP_BYTES)?;
    let journal: Journal = serde_json::from_slice(&bytes).map_err(|_| "历史备份账本损坏")?;
    if journal.version != 1
        || !journal.root.is_absolute()
        || journal.logs.len() > MAX_FILES
        || journal.states.len() > MAX_STATE_DATABASES
        || journal.source_provider_ids.len() > 128
        || journal
            .states
            .iter()
            .map(|state| state.rows.len())
            .sum::<usize>()
            > MAX_STATE_ROWS
    {
        return Err("历史备份格式或范围不受支持".into());
    }
    if normalize_provider_id(&journal.target_provider_id)? != journal.target_provider_id {
        return Err("历史备份分桶标识无效".into());
    }
    let mut sources = HashSet::new();
    for source in &journal.source_provider_ids {
        if normalize_provider_id(source)? != *source
            || source == &journal.target_provider_id
            || !sources.insert(source)
        {
            return Err("历史备份来源标识无效".into());
        }
    }
    for entry in &journal.logs {
        if !valid_hash(&entry.original_hash) || !valid_hash(&entry.migrated_hash) {
            return Err("历史备份校验标识无效".into());
        }
    }
    for state in &journal.states {
        if state
            .original_hash
            .as_ref()
            .is_some_and(|value| !valid_hash(value))
        {
            return Err("历史状态备份校验标识无效".into());
        }
        let mut ids = HashSet::new();
        for (id, original) in &state.rows {
            if id.is_empty()
                || id.chars().count() > 256
                || id.chars().any(char::is_control)
                || !sources.contains(original)
                || !ids.insert(id)
            {
                return Err("历史状态账本包含无效或重复记录".into());
            }
        }
    }
    Ok((generation, journal, hash(&bytes)))
}

pub(super) fn relative(path: &Path, root: &Path, log: bool) -> Result<PathBuf, String> {
    let relative = path
        .strip_prefix(root)
        .map_err(|_| "历史账本路径不属于原配置目录")?;
    let parts = relative.components().collect::<Vec<_>>();
    if parts.is_empty()
        || parts.iter().any(|part| match part {
            Component::Normal(value) => value.to_str().is_none_or(|name| {
                name.is_empty()
                    || name.ends_with(['.', ' '])
                    || name
                        .chars()
                        .any(|c| c.is_control() || "\\:*?\"<>|".contains(c))
            }),
            _ => true,
        })
    {
        return Err("历史账本相对路径无效".into());
    }
    let first = parts[0].as_os_str().to_str().unwrap_or_default();
    let valid = if log {
        matches!(first, "sessions" | "archived_sessions")
            && parts.len() > 1
            && archive::jsonl(relative)
    } else {
        parts.len() == 1
            && (first == "state.sqlite"
                || (first.starts_with("state_") && first.ends_with(".sqlite")))
    };
    if !valid {
        return Err("历史账本包含不支持的文件类型".into());
    }
    Ok(relative.into())
}

pub(super) fn list(root: &Path, backups: &Path) -> Result<Vec<BackupSummary>, String> {
    let entries = match fs::read_dir(backups) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(_) => return Err("无法检查历史备份目录".into()),
    };
    let root = root.canonicalize().map_err(|_| "无法确定当前历史目录")?;
    let mut summaries = Vec::new();
    let mut scanned = 0;
    let mut generations = 0;
    let mut journal_bytes = 0u64;
    for entry in entries {
        scanned += 1;
        if scanned > MAX_FILES {
            return Err("备份目录数量超过检查限制".into());
        }
        let entry = entry.map_err(|_| "无法读取备份目录项")?;
        let key = entry.file_name().to_string_lossy().into_owned();
        if !valid_key(&key) {
            continue;
        }
        generations += 1;
        if generations > 200 {
            return Err("迁移备份超过 200 个，请先整理备份目录再检查".into());
        }
        if let Ok(meta) = fs::symlink_metadata(entry.path().join("migration.json")) {
            journal_bytes = journal_bytes.saturating_add(meta.len());
            if journal_bytes > 64 * 1024 * 1024 {
                return Err("迁移账本总大小超过 64 MiB，请先整理备份目录".into());
            }
        }
        let created_at_seconds = fs::symlink_metadata(entry.path())
            .ok()
            .and_then(|meta| meta.modified().ok())
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map_or(0, |duration| duration.as_secs());
        let mut summary = BackupSummary {
            key: key.clone(),
            created_at_seconds,
            target_provider_id: None,
            log_files: 0,
            state_rows: 0,
            problem: None,
        };
        match read_journal(backups, &key) {
            Ok((_, journal, _)) => match journal.root.canonicalize() {
                Ok(original_root) if original_root == root => {
                    summary.target_provider_id = Some(journal.target_provider_id);
                    summary.log_files = journal.logs.len();
                    summary.state_rows = journal.states.iter().map(|state| state.rows.len()).sum();
                }
                Ok(_) => continue,
                Err(_) => summary.problem = Some("无法确认备份所属配置目录".into()),
            },
            Err(error) => summary.problem = Some(error),
        }
        summaries.push(summary);
    }
    summaries.sort_by(|a, b| {
        b.created_at_seconds
            .cmp(&a.created_at_seconds)
            .then_with(|| a.key.cmp(&b.key))
    });
    Ok(summaries)
}
