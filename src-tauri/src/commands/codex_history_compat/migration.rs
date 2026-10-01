use std::collections::HashSet;
use std::fs;
#[cfg(test)]
use std::fs::OpenOptions;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

use rusqlite::{Connection, OpenFlags};
use serde::Serialize;
use sha2::{Digest, Sha256};

use super::{
    rewrite_history_meta_line, CodexHistoryMigrationResult, MAX_FILES, MAX_SESSION_FILE_BYTES,
};
use crate::shared::session_archive as archive;

pub(super) const MAX_PLAN_BYTES: usize = 256 * 1024 * 1024;
pub(super) const MAX_STATE_ROWS: usize = 50_000;
pub(super) const MAX_STATE_DATABASES: usize = 128;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MigrationPreview {
    pub revision: String,
    pub source_provider_ids: Vec<String>,
    pub target_provider_id: String,
    pub jsonl_files: usize,
    pub compressed_files: usize,
    pub state_rows: usize,
}

use super::mutation::LogChange;

struct StateChange {
    path: PathBuf,
    rows: Vec<(String, String)>,
}

pub(super) struct MigrationPlan {
    root: PathBuf,
    sources: Vec<String>,
    target: String,
    logs: Vec<LogChange>,
    states: Vec<StateChange>,
    revision: String,
}

pub(super) fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub(super) fn read_file(path: &Path, limit: u64) -> Result<Vec<u8>, String> {
    let metadata = fs::symlink_metadata(path).map_err(|_| "无法读取历史文件")?;
    if !metadata.is_file() || archive::is_link(&metadata) {
        return Err("历史文件包含链接或不是普通文件".into());
    }
    if metadata.len() > limit {
        return Err("历史文件超过读取限制".into());
    }
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|_| "无法打开历史文件")?
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "无法读取历史文件内容")?;
    if bytes.len() as u64 > limit {
        return Err("历史文件在读取时超过限制".into());
    }
    Ok(bytes)
}

fn collect(
    root: &Path,
    directory: &Path,
    depth: usize,
    files: &mut Vec<PathBuf>,
) -> Result<(), String> {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(_) => return Err("无法检查历史会话目录".into()),
    };
    if depth > 10 {
        return Err("历史目录层级超过限制".into());
    }
    for entry in entries {
        let path = entry.map_err(|_| "无法读取历史目录项")?.path();
        let meta = fs::symlink_metadata(&path).map_err(|_| "无法检查历史目录项")?;
        if archive::is_link(&meta) {
            return Err("历史目录包含链接或重解析点，请检查后重试".into());
        }
        if meta.is_dir() {
            collect(root, &path, depth + 1, files)?;
        } else if archive::jsonl(&path) {
            if !archive::confined(&path, root) {
                return Err("历史文件不在配置目录内".into());
            }
            files.push(path);
            if files.len() > MAX_FILES {
                return Err("历史文件数量超过限制".into());
            }
        }
    }
    Ok(())
}

pub(super) fn rewrite(
    path: &Path,
    original: &[u8],
    sources: &[String],
    target: &str,
) -> Result<Option<Vec<u8>>, String> {
    rewrite_limited(path, original, sources, target, MAX_SESSION_FILE_BYTES)
}

pub(super) fn decode(path: &Path, original: &[u8], limit: u64) -> Result<Vec<u8>, String> {
    if archive::compressed(path) {
        let mut decoder =
            zstd::stream::read::Decoder::new(original).map_err(|_| "压缩历史文件损坏")?;
        decoder
            .window_log_max(27)
            .map_err(|_| "无法限制压缩历史文件窗口")?;
        let mut bytes = Vec::new();
        decoder
            .take(limit + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "压缩历史文件损坏")?;
        if bytes.len() as u64 > limit {
            return Err("解压历史文件超过读取限制".into());
        }
        Ok(bytes)
    } else {
        if original.len() as u64 > limit {
            return Err("历史文件超过读取限制".into());
        }
        Ok(original.to_vec())
    }
}

fn rewrite_limited(
    path: &Path,
    original: &[u8],
    sources: &[String],
    target: &str,
    limit: u64,
) -> Result<Option<Vec<u8>>, String> {
    let decoded = decode(path, original, limit)?;
    let content = std::str::from_utf8(&decoded).map_err(|_| "历史文件不是有效 UTF-8")?;
    let source_ids: HashSet<_> = sources.iter().collect();
    let mut rewritten = String::with_capacity(content.len());
    let mut changed = false;
    for segment in content.split_inclusive('\n') {
        if segment.len() as u64 > archive::MAX_LINE_BYTES {
            return Err("历史文件单行超过读取限制".into());
        }
        let (line, newline) = if let Some(line) = segment.strip_suffix("\r\n") {
            (line, "\r\n")
        } else if let Some(line) = segment.strip_suffix('\n') {
            (line, "\n")
        } else {
            (segment, "")
        };
        if line.contains("\"session_meta\"") && line.contains("\"model_provider\"") {
            serde_json::from_str::<serde_json::Value>(line)
                .map_err(|_| "历史会话元数据损坏，请检查后重试")?;
        }
        if let Some(next) = rewrite_history_meta_line(line, &source_ids, target) {
            if next.len() as u64 + newline.len() as u64 > archive::MAX_LINE_BYTES {
                return Err("迁移后的历史记录单行超过限制".into());
            }
            rewritten.push_str(&next);
            changed = true;
        } else {
            rewritten.push_str(line);
        }
        rewritten.push_str(newline);
        if rewritten.len() as u64 > limit {
            return Err("迁移后的历史文件超过读取限制".into());
        }
    }
    if !changed {
        return Ok(None);
    }
    let replacement = if archive::compressed(path) {
        zstd::stream::encode_all(rewritten.as_bytes(), 3).map_err(|_| "无法编码压缩历史文件")?
    } else {
        rewritten.into_bytes()
    };
    if replacement.len() as u64 > limit {
        return Err("迁移后的压缩历史文件超过读取限制".into());
    }
    Ok(Some(replacement))
}

pub(super) fn open_state(path: &Path, writable: bool) -> Result<Connection, String> {
    let meta = fs::symlink_metadata(path).map_err(|_| "无法检查历史状态数据库")?;
    if !meta.is_file() || archive::is_link(&meta) || meta.len() > MAX_SESSION_FILE_BYTES {
        return Err("历史状态数据库不是普通文件或超过读取限制".into());
    }
    for suffix in ["-wal", "-shm", "-journal"] {
        let mut sidecar = path.as_os_str().to_os_string();
        sidecar.push(suffix);
        match fs::symlink_metadata(Path::new(&sidecar)) {
            Ok(meta)
                if !meta.is_file()
                    || archive::is_link(&meta)
                    || meta.len() > MAX_SESSION_FILE_BYTES =>
            {
                return Err("历史数据库附属文件包含链接、类型无效或超过限制".into());
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err("无法检查历史数据库附属文件".into()),
        }
    }
    let conn = Connection::open_with_flags(
        path,
        if writable {
            OpenFlags::SQLITE_OPEN_READ_WRITE
        } else {
            OpenFlags::SQLITE_OPEN_READ_ONLY
        },
    )
    .map_err(|_| "无法打开历史状态数据库")?;
    conn.busy_timeout(Duration::from_secs(1))
        .map_err(|_| "无法设置历史数据库等待限制")?;
    conn.execute_batch("PRAGMA trusted_schema=OFF; PRAGMA foreign_keys=ON;")
        .map_err(|_| "无法设置历史数据库保护")?;
    let pages: u64 = conn
        .query_row("PRAGMA page_count", [], |r| r.get(0))
        .map_err(|_| "无法检查历史状态大小")?;
    let size: u64 = conn
        .query_row("PRAGMA page_size", [], |r| r.get(0))
        .map_err(|_| "无法检查历史状态大小")?;
    if pages.saturating_mul(size) > MAX_SESSION_FILE_BYTES {
        return Err("历史状态数据库及 WAL 数据超过读取限制".into());
    }
    Ok(conn)
}

fn selected_rows(conn: &Connection, sources: &[String]) -> Result<Vec<(String, String)>, String> {
    let start = std::time::Instant::now();
    conn.progress_handler(
        1000,
        Some(move || start.elapsed() >= Duration::from_secs(5)),
    );
    let result = selected_rows_inner(conn, sources);
    conn.progress_handler(0, None::<fn() -> bool>);
    result
}

fn selected_rows_inner(
    conn: &Connection,
    sources: &[String],
) -> Result<Vec<(String, String)>, String> {
    let mut stmt = conn
        .prepare("PRAGMA table_info(threads)")
        .map_err(|_| "无法检查历史状态字段")?;
    let columns = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|_| "无法检查历史状态字段")?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| "历史状态字段无效")?;
    if !columns.iter().any(|s| s == "model_provider") {
        return Ok(Vec::new());
    }
    let mut stmt = conn
        .prepare("SELECT substr(id,1,257),substr(model_provider,1,129) FROM threads ORDER BY id")
        .map_err(|_| "无法读取历史状态记录")?;
    let mut rows = stmt.query([]).map_err(|_| "无法读取历史状态记录")?;
    let mut selected = Vec::new();
    let mut scanned = 0;
    while let Some(row) = rows.next().map_err(|_| "历史状态记录无效")? {
        scanned += 1;
        if scanned > MAX_STATE_ROWS {
            return Err("历史状态记录超过检查限制".into());
        }
        let provider: Option<String> = row.get(1).map_err(|_| "历史状态记录无效")?;
        if let Some(provider) = provider.filter(|p| sources.contains(p)) {
            let id: String = row.get(0).map_err(|_| "历史会话 ID 无效")?;
            if id.chars().count() > 256 {
                return Err("历史会话 ID 超过读取限制".into());
            }
            selected.push((id, provider));
        }
    }
    Ok(selected)
}

impl MigrationPlan {
    pub(super) fn prepare(
        root: PathBuf,
        sources: Vec<String>,
        target: String,
    ) -> Result<Self, String> {
        let mut paths = Vec::new();
        for dir in if sources.is_empty() {
            Vec::new()
        } else {
            vec!["sessions", "archived_sessions"]
        } {
            let directory = root.join(dir);
            if directory.exists()
                && archive::is_link(
                    &fs::symlink_metadata(&directory).map_err(|_| "无法检查历史目录")?,
                )
            {
                return Err("历史目录包含链接或重解析点".into());
            }
            collect(&root, &directory, 0, &mut paths)?;
        }
        paths.sort();
        let mut logs = Vec::new();
        let mut total_bytes = 0usize;
        for path in paths {
            let original = read_file(&path, MAX_SESSION_FILE_BYTES)?;
            if let Some(replacement) = rewrite(&path, &original, &sources, &target)? {
                let age = fs::metadata(&path)
                    .and_then(|m| m.modified())
                    .map_err(|_| "无法检查会话更新时间")?
                    .elapsed()
                    .map_err(|_| "会话更新时间在未来，迁移已停止")?;
                if age < Duration::from_secs(60) {
                    return Err("会话最近仍在更新，请关闭客户端并稍后重新检查".into());
                }
                total_bytes = total_bytes
                    .checked_add(original.len() + replacement.len())
                    .ok_or("历史迁移大小超过限制")?;
                if total_bytes > MAX_PLAN_BYTES {
                    return Err("本次历史迁移大小超过 256 MiB，请分批处理".into());
                }
                logs.push(LogChange {
                    path,
                    original,
                    replacement,
                });
            }
        }
        let mut states = Vec::new();
        let mut state_rows = 0usize;
        for path in if sources.is_empty() {
            Vec::new()
        } else {
            crate::commands::extra_commands::codex_state_databases(&root)
        } {
            if !archive::confined(&path, &root) {
                return Err("历史状态数据库不在配置目录内".into());
            }
            let rows = selected_rows(&open_state(&path, false)?, &sources)?;
            state_rows = state_rows.saturating_add(rows.len());
            if state_rows > MAX_STATE_ROWS {
                return Err("本次迁移状态记录超过 50000 条，请分批处理".into());
            }
            if !rows.is_empty() {
                if states.len() >= MAX_STATE_DATABASES {
                    return Err("本次迁移状态数据库超过 128 个，请分批处理".into());
                }
                states.push(StateChange { path, rows });
            }
        }
        let mut digest = Sha256::new();
        digest.update(
            serde_json::to_vec(&(&root, &sources, &target)).map_err(|_| "无法创建迁移检查标识")?,
        );
        for log in &logs {
            digest.update(
                serde_json::to_vec(&(&log.path, hash(&log.original), hash(&log.replacement)))
                    .map_err(|_| "无法创建会话检查标识")?,
            );
        }
        for state in &states {
            digest.update(
                serde_json::to_vec(&(&state.path, &state.rows))
                    .map_err(|_| "无法创建状态检查标识")?,
            );
        }
        Ok(Self {
            root,
            sources,
            target,
            logs,
            states,
            revision: format!("{:x}", digest.finalize()),
        })
    }

    pub(super) fn preview(&self) -> MigrationPreview {
        MigrationPreview {
            revision: self.revision.clone(),
            source_provider_ids: self.sources.clone(),
            target_provider_id: self.target.clone(),
            jsonl_files: self.logs.len(),
            compressed_files: self
                .logs
                .iter()
                .filter(|log| archive::compressed(&log.path))
                .count(),
            state_rows: self.states.iter().map(|state| state.rows.len()).sum(),
        }
    }

    pub(super) fn execute(
        self,
        backups: &Path,
        expected: Option<&str>,
    ) -> Result<CodexHistoryMigrationResult, String> {
        self.execute_with(backups, expected, |path, bytes| {
            crate::utils::atomic_write(path, bytes).map_err(|_| "无法写入迁移会话文件".into())
        })
    }

    fn execute_with(
        self,
        backups: &Path,
        expected: Option<&str>,
        mut write: impl FnMut(&Path, &[u8]) -> Result<(), String>,
    ) -> Result<CodexHistoryMigrationResult, String> {
        if expected.is_some_and(|revision| revision != self.revision) {
            return Err("历史在预览后发生变化，请重新检查再迁移".into());
        }
        let preview = self.preview();
        if self.logs.is_empty() && self.states.is_empty() {
            return Ok(CodexHistoryMigrationResult {
                source_provider_ids: self.sources,
                target_provider_id: self.target,
                migrated_jsonl_files: 0,
                migrated_state_rows: 0,
                backup_path: None,
                skipped_reason: Some("nothing_to_migrate".into()),
            });
        }
        let journal = serde_json::json!({"version":1,"root":self.root,"sourceProviderIds":self.sources,"targetProviderId":self.target,"revision":self.revision,
            "logs":self.logs.iter().map(|log| serde_json::json!({"path":log.path,"originalHash":hash(&log.original),"migratedHash":hash(&log.replacement)})).collect::<Vec<_>>(),
            "states":self.states.iter().map(|state| serde_json::json!({"path":state.path,"rows":state.rows})).collect::<Vec<_>>()});
        let mutation = super::mutation::MutationPlan {
            root: self.root,
            logs: self.logs,
            states: self
                .states
                .into_iter()
                .map(|state| super::mutation::StateChange {
                    path: state.path,
                    rows: state
                        .rows
                        .into_iter()
                        .map(|(id, original)| super::mutation::RowChange {
                            id,
                            original,
                            replacement: self.target.clone(),
                        })
                        .collect(),
                })
                .collect(),
        };
        let backup = mutation.execute_with(backups, "migration", journal, &mut write)?;
        Ok(CodexHistoryMigrationResult {
            source_provider_ids: self.sources,
            target_provider_id: self.target,
            migrated_jsonl_files: preview.jsonl_files,
            migrated_state_rows: preview.state_rows,
            backup_path: Some(backup.to_string_lossy().into()),
            skipped_reason: None,
        })
    }
}

#[cfg(test)]
mod tests;
