//! Reverse only proven provider changes; never replace a live database or conversation.
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

use serde::Serialize;
use sha2::{Digest, Sha256};

use super::migration::{decode, hash, open_state, read_file, rewrite, MAX_PLAN_BYTES};
use super::mutation::{inactive, provider, LogChange, MutationPlan, RowChange, StateChange};
use super::MAX_SESSION_FILE_BYTES;
use crate::shared::session_archive as archive;

mod catalog;
pub(super) fn list(root: &Path, backups: &Path) -> Result<Vec<BackupSummary>, String> {
    catalog::list(root, backups)
}
pub use catalog::BackupSummary;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreItem {
    pub key: String,
    pub session_id: String,
    pub original_provider_id: String,
    pub status: &'static str,
    pub reason: Option<String>,
    pub log_files: usize,
    pub state_rows: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestorePreview {
    pub backup_key: String,
    pub revision: String,
    pub items: Vec<RestoreItem>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreResult {
    pub restored_jsonl_files: usize,
    pub restored_state_rows: usize,
    pub backup_path: String,
}

struct Group {
    item: RestoreItem,
    logs: Vec<LogChange>,
    states: Vec<(PathBuf, RowChange)>,
}

pub(super) struct RestorePlan {
    root: PathBuf,
    backup_key: String,
    revision: String,
    groups: BTreeMap<String, Group>,
}

fn identity(path: &Path, bytes: &[u8]) -> Result<(String, String), String> {
    let decoded = decode(path, bytes, MAX_SESSION_FILE_BYTES)?;
    let text = std::str::from_utf8(&decoded).map_err(|_| "历史会话不是有效 UTF-8")?;
    let mut result = None;
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        if line.len() as u64 > archive::MAX_LINE_BYTES {
            return Err("历史会话单行超过检查限制".into());
        }
        let value: serde_json::Value =
            serde_json::from_str(line).map_err(|_| "历史会话记录损坏")?;
        if value["type"] != "session_meta" {
            continue;
        }
        if result.is_some() {
            return Err("历史会话包含重复身份记录".into());
        }
        let id = value["payload"]["id"].as_str().ok_or("历史会话缺少 ID")?;
        let provider = value["payload"]["model_provider"]
            .as_str()
            .ok_or("历史会话缺少分桶标识")?;
        if id.is_empty()
            || id.chars().count() > 256
            || id.chars().any(char::is_control)
            || super::normalize_provider_id(provider)? != provider
        {
            return Err("历史会话身份或分桶标识无效".into());
        }
        result = Some((id.into(), provider.into()));
    }
    result.ok_or_else(|| "历史会话缺少身份记录".into())
}

fn saved_file(generation: &Path, kind: &str, relative: &Path) -> Result<PathBuf, String> {
    let saved = generation.join(kind).join(relative);
    if !archive::confined(&saved, generation) {
        return Err("原始备份缺失、包含链接或路径越界".into());
    }
    Ok(saved)
}

fn state_reader(path: &Path) -> Result<rusqlite::Connection, String> {
    let conn = open_state(path, false)?;
    let started = std::time::Instant::now();
    conn.progress_handler(
        1000,
        Some(move || started.elapsed() >= std::time::Duration::from_secs(5)),
    );
    Ok(conn)
}

impl RestorePlan {
    pub(super) fn prepare(root: PathBuf, backups: &Path, key: String) -> Result<Self, String> {
        let (generation, journal, journal_hash) = catalog::read_journal(backups, &key)?;
        if root.canonicalize().map_err(|_| "无法确定当前历史目录")?
            != journal
                .root
                .canonicalize()
                .map_err(|_| "无法确认备份所属配置目录")?
        {
            return Err("该历史备份不属于当前配置目录，请重新选择".into());
        }
        let mut plan = Self {
            root,
            backup_key: key,
            revision: String::new(),
            groups: BTreeMap::new(),
        };
        let mut digest = Sha256::new();
        digest.update(journal_hash);
        digest.update(plan.root.to_string_lossy().as_bytes());
        let mut total = 0usize;
        let mut seen = HashSet::new();
        for (index, entry) in journal.logs.iter().enumerate() {
            let relative = catalog::relative(&entry.path, &journal.root, true)?;
            if !seen.insert(relative.clone()) {
                return Err("历史账本包含重复文件路径".into());
            }
            let saved = saved_file(&generation, "jsonl", &relative)?;
            let bytes = read_file(&saved, MAX_SESSION_FILE_BYTES)?;
            total = total
                .checked_add(bytes.len())
                .ok_or("恢复检查大小超过限制")?;
            if total > MAX_PLAN_BYTES {
                return Err("恢复检查超过 256 MiB，请分批整理备份".into());
            }
            if hash(&bytes) != entry.original_hash {
                return Err("原始会话备份校验失败，恢复已停止".into());
            }
            let (id, original) = match identity(&saved, &bytes) {
                Ok(identity) => identity,
                Err(error) => {
                    plan.conflict(
                        format!("file-{index}"),
                        relative.to_string_lossy().into(),
                        "未知".into(),
                        error,
                    );
                    continue;
                }
            };
            if !journal.source_provider_ids.contains(&original) {
                return Err("原始会话与迁移来源账本不一致".into());
            }
            let group = plan.group(&id, &original);
            let current_path = plan.root.join(&relative);
            let outcome = (|| {
                // A client may pack an old plain log after the migration.
                let path = if current_path.try_exists().map_err(|_| "无法检查当前会话")? {
                    current_path.clone()
                } else {
                    archive::resolve(&current_path).map_err(|_| "当前会话缺失或无法读取")?
                };
                if !archive::confined(&path, &plan.root) {
                    return Err("当前会话包含链接或路径越界".into());
                }
                let current = read_file(&path, MAX_SESSION_FILE_BYTES)?;
                digest.update(
                    serde_json::to_vec(&(&path, hash(&current)))
                        .map_err(|_| "无法创建恢复检查标识")?,
                );
                let (current_id, current_provider) = identity(&path, &current)?;
                if current_id != id {
                    return Err("当前文件属于另一个会话，无法恢复".into());
                }
                if current_provider == original {
                    return Ok(None);
                }
                if current_provider != journal.target_provider_id {
                    return Err("当前分桶已由其他操作修改，无法恢复".into());
                }
                inactive(&path)?;
                let replacement = rewrite(
                    &path,
                    &current,
                    &[journal.target_provider_id.clone()],
                    &original,
                )?
                .ok_or("当前会话无法恢复")?;
                total = total
                    .checked_add(current.len() + replacement.len())
                    .ok_or("恢复检查大小超过限制")?;
                if total > MAX_PLAN_BYTES {
                    return Err("恢复检查超过 256 MiB，请分批整理备份".into());
                }
                Ok(Some(LogChange {
                    path,
                    original: current,
                    replacement,
                }))
            })();
            match outcome {
                Ok(Some(change)) => {
                    let group = plan.groups.get_mut(&group).unwrap();
                    if !group.logs.iter().any(|log| log.path == change.path) {
                        group.logs.push(change);
                    }
                }
                Ok(None) => {}
                Err(error) => plan.mark_conflict(&group, error),
            }
        }
        let mut seen_states = HashSet::new();
        for state in &journal.states {
            let relative = catalog::relative(&state.path, &journal.root, false)?;
            if !seen_states.insert(relative.clone()) {
                return Err("历史账本包含重复数据库路径".into());
            }
            let saved = saved_file(&generation, "state", &relative)?;
            let bytes = read_file(&saved, MAX_SESSION_FILE_BYTES)?;
            total = total
                .checked_add(bytes.len())
                .ok_or("恢复检查大小超过限制")?;
            if total > MAX_PLAN_BYTES {
                return Err("恢复检查超过 256 MiB，请分批整理备份".into());
            }
            let saved_hash = hash(&bytes);
            if state
                .original_hash
                .as_ref()
                .is_some_and(|expected| expected != &saved_hash)
            {
                return Err("原始状态备份校验失败，恢复已停止".into());
            }
            digest.update(saved_hash);
            let source = state_reader(&saved)?;
            let current_path = plan.root.join(relative);
            let current = if archive::confined(&current_path, &plan.root) {
                state_reader(&current_path)
            } else {
                Err("当前状态数据库缺失、包含链接或路径越界".into())
            };
            for (id, original) in &state.rows {
                if provider(&source, id)?.as_deref() != Some(original.as_str()) {
                    return Err("原始状态备份与迁移账本不一致".into());
                }
                let group = plan.group(id, original);
                let outcome = current
                    .as_ref()
                    .map_err(Clone::clone)
                    .and_then(|conn| provider(conn, id));
                digest.update(
                    serde_json::to_vec(&(&current_path, id, &outcome))
                        .map_err(|_| "无法创建状态恢复检查标识")?,
                );
                match outcome {
                    Ok(Some(value)) if value == *original => {}
                    Ok(Some(value)) if value == journal.target_provider_id => {
                        plan.groups.get_mut(&group).unwrap().states.push((
                            current_path.clone(),
                            RowChange {
                                id: id.clone(),
                                original: value,
                                replacement: original.clone(),
                            },
                        ));
                    }
                    Ok(_) => {
                        plan.mark_conflict(&group, "当前状态记录缺失或分桶已修改，无法恢复".into())
                    }
                    Err(error) => plan.mark_conflict(&group, error),
                }
            }
        }
        // An ID with contradictory original ownership must not be partially restored.
        let mut ownership = BTreeMap::<String, HashSet<String>>::new();
        for group in plan.groups.values() {
            ownership
                .entry(group.item.session_id.clone())
                .or_default()
                .insert(group.item.original_provider_id.clone());
        }
        for group in plan.groups.values_mut() {
            if ownership[&group.item.session_id].len() > 1 {
                group.item.status = "conflict";
                group.item.reason = Some("备份中的同一会话有不同原始分桶，请核查后恢复".into());
            }
            group.item.log_files = group.logs.len();
            group.item.state_rows = group.states.len();
            if group.item.status != "conflict" {
                group.item.status = if group.logs.is_empty() && group.states.is_empty() {
                    "restored"
                } else {
                    "ready"
                };
            }
            digest.update(serde_json::to_vec(&group.item).map_err(|_| "无法创建恢复范围标识")?);
        }
        plan.revision = format!("{:x}", digest.finalize());
        Ok(plan)
    }

    fn group(&mut self, id: &str, original: &str) -> String {
        let key = hash(&serde_json::to_vec(&(id, original)).expect("string tuple serialization"));
        self.groups.entry(key.clone()).or_insert_with(|| Group {
            item: RestoreItem {
                key: key.clone(),
                session_id: id.into(),
                original_provider_id: original.into(),
                status: "restored",
                reason: None,
                log_files: 0,
                state_rows: 0,
            },
            logs: Vec::new(),
            states: Vec::new(),
        });
        key
    }

    fn conflict(&mut self, key: String, id: String, original: String, reason: String) {
        self.groups.insert(
            key.clone(),
            Group {
                item: RestoreItem {
                    key,
                    session_id: id,
                    original_provider_id: original,
                    status: "conflict",
                    reason: Some(reason),
                    log_files: 0,
                    state_rows: 0,
                },
                logs: Vec::new(),
                states: Vec::new(),
            },
        );
    }

    fn mark_conflict(&mut self, key: &str, reason: String) {
        let item = &mut self.groups.get_mut(key).unwrap().item;
        item.status = "conflict";
        item.reason = Some(reason);
    }

    pub(super) fn preview(&self) -> RestorePreview {
        RestorePreview {
            backup_key: self.backup_key.clone(),
            revision: self.revision.clone(),
            items: self
                .groups
                .values()
                .map(|group| RestoreItem {
                    key: group.item.key.clone(),
                    session_id: group.item.session_id.clone(),
                    original_provider_id: group.item.original_provider_id.clone(),
                    status: group.item.status,
                    reason: group.item.reason.clone(),
                    log_files: group.item.log_files,
                    state_rows: group.item.state_rows,
                })
                .collect(),
        }
    }

    pub(super) fn execute(
        self,
        backups: &Path,
        expected: &str,
        keys: Vec<String>,
    ) -> Result<RestoreResult, String> {
        self.execute_with(backups, expected, keys, |path, bytes| {
            crate::utils::atomic_write(path, bytes).map_err(|_| "无法恢复会话文件".into())
        })
    }

    fn execute_with(
        self,
        backups: &Path,
        expected: &str,
        keys: Vec<String>,
        write: impl FnMut(&Path, &[u8]) -> Result<(), String>,
    ) -> Result<RestoreResult, String> {
        if self.revision != expected {
            return Err("历史在恢复预览后发生变化，请重新检查".into());
        }
        if keys.is_empty() || keys.len() > self.groups.len() {
            return Err("请选择有效的待恢复会话".into());
        }
        let selected: HashSet<_> = keys.iter().collect();
        if selected.len() != keys.len()
            || keys.iter().any(|key| {
                self.groups
                    .get(key)
                    .is_none_or(|group| group.item.status != "ready")
            })
        {
            return Err("恢复选择包含冲突、已恢复或无效会话，请重新检查".into());
        }
        let mut logs = Vec::new();
        let mut states = BTreeMap::<PathBuf, Vec<RowChange>>::new();
        for (key, group) in self.groups {
            if !selected.contains(&key) {
                continue;
            }
            logs.extend(group.logs);
            for (path, row) in group.states {
                states.entry(path).or_default().push(row);
            }
        }
        let restored_jsonl_files = logs.len();
        let restored_state_rows = states.values().map(Vec::len).sum();
        let mutation = MutationPlan {
            root: self.root,
            logs,
            states: states
                .into_iter()
                .map(|(path, rows)| StateChange { path, rows })
                .collect(),
        };
        let journal = serde_json::json!({"version":1,"root":mutation.root,"sourceBackupKey":self.backup_key,"revision":self.revision,
            "logs":mutation.logs.iter().map(|log| serde_json::json!({"path":log.path,"originalHash":hash(&log.original),"restoredHash":hash(&log.replacement)})).collect::<Vec<_>>(),
            "states":mutation.states.iter().map(|state| serde_json::json!({"path":state.path,"rows":state.rows.iter().map(|row| (&row.id,&row.original,&row.replacement)).collect::<Vec<_>>()})).collect::<Vec<_>>()});
        let backup = mutation.execute_with(backups, "restore", journal, write)?;
        Ok(RestoreResult {
            restored_jsonl_files,
            restored_state_rows,
            backup_path: backup.to_string_lossy().into(),
        })
    }
}

#[cfg(test)]
mod tests;
