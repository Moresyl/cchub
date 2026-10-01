use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use rusqlite::{Connection, OpenFlags};
use serde::Serialize;
use sha2::{Digest, Sha256};

use super::{
    rewrite_history_meta_line, CodexHistoryMigrationResult, MAX_FILES, MAX_SESSION_FILE_BYTES,
};
use crate::shared::session_archive as archive;

const MAX_PLAN_BYTES: usize = 256 * 1024 * 1024;
const MAX_STATE_ROWS: usize = 50_000;

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

struct LogChange {
    path: PathBuf,
    original: Vec<u8>,
    replacement: Vec<u8>,
}

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

fn hash(bytes: &[u8]) -> String {
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

fn rewrite(
    path: &Path,
    original: &[u8],
    sources: &[String],
    target: &str,
) -> Result<Option<Vec<u8>>, String> {
    rewrite_limited(path, original, sources, target, MAX_SESSION_FILE_BYTES)
}

fn rewrite_limited(
    path: &Path,
    original: &[u8],
    sources: &[String],
    target: &str,
    limit: u64,
) -> Result<Option<Vec<u8>>, String> {
    let decoded = if archive::compressed(path) {
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
        bytes
    } else {
        original.to_vec()
    };
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

fn open_state(path: &Path, writable: bool) -> Result<Connection, String> {
    let meta = fs::symlink_metadata(path).map_err(|_| "无法检查历史状态数据库")?;
    if !meta.is_file() || archive::is_link(&meta) || meta.len() > MAX_SESSION_FILE_BYTES {
        return Err("历史状态数据库不是普通文件或超过读取限制".into());
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
        for path in if sources.is_empty() {
            Vec::new()
        } else {
            crate::commands::extra_commands::codex_state_databases(&root)
        } {
            if !archive::confined(&path, &root) {
                return Err("历史状态数据库不在配置目录内".into());
            }
            let rows = selected_rows(&open_state(&path, false)?, &sources)?;
            if !rows.is_empty() {
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
        fs::create_dir_all(backups).map_err(|_| "无法创建历史备份目录")?;
        let backup_parent = backups.canonicalize().map_err(|_| "无法确定历史备份目录")?;
        if backup_parent.starts_with(
            self.root
                .canonicalize()
                .map_err(|_| "无法确定历史配置目录")?,
        ) {
            return Err("历史备份目录必须位于会话目录之外".into());
        }
        let backup =
            backup_parent.join(format!("codex-history-migration-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&backup).map_err(|_| "无法创建迁移原始副本目录")?;
        let mut connections = Vec::new();
        let mut touched = Vec::new();
        let mut committed = 0usize;
        let operation: Result<(), String> = (|| {
            for state in &self.states {
                if !archive::confined(&state.path, &self.root) {
                    return Err("历史数据库路径在检查后变化".into());
                }
                let conn = open_state(&state.path, true)?;
                conn.execute_batch("BEGIN IMMEDIATE")
                    .map_err(|_| "历史数据库正在使用，请稍后重试")?;
                if selected_rows(&conn, &self.sources)? != state.rows {
                    return Err("历史状态在检查后变化，请重新检查".into());
                }
                let source = open_state(&state.path, false)?;
                let pages: u64 = source
                    .query_row("PRAGMA page_count", [], |r| r.get(0))
                    .map_err(|_| "无法检查状态备份大小")?;
                let size: u64 = source
                    .query_row("PRAGMA page_size", [], |r| r.get(0))
                    .map_err(|_| "无法检查状态备份大小")?;
                if pages.saturating_mul(size) > MAX_SESSION_FILE_BYTES {
                    return Err("历史状态备份超过读取限制".into());
                }
                let saved = backup.join("state").join(
                    state
                        .path
                        .strip_prefix(&self.root)
                        .map_err(|_| "状态备份路径无效")?,
                );
                fs::create_dir_all(saved.parent().ok_or("状态备份路径无效")?)
                    .map_err(|_| "无法创建状态备份目录")?;
                let mut destination = Connection::open(&saved).map_err(|_| "无法创建状态备份")?;
                let task = rusqlite::backup::Backup::new(&source, &mut destination)
                    .map_err(|_| "无法创建状态备份")?;
                if !matches!(
                    task.step(-1).map_err(|_| "状态数据库备份失败")?,
                    rusqlite::backup::StepResult::Done
                ) {
                    return Err("状态数据库备份繁忙，请稍后重试".into());
                }
                drop(task);
                drop(destination);
                OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open(&saved)
                    .and_then(|f| f.sync_all())
                    .map_err(|_| "无法保存状态备份")?;
                connections.push(conn);
            }
            for log in &self.logs {
                let relative = log
                    .path
                    .strip_prefix(&self.root)
                    .map_err(|_| "会话备份路径无效")?;
                let saved = backup.join("jsonl").join(relative);
                fs::create_dir_all(saved.parent().ok_or("会话备份路径无效")?)
                    .map_err(|_| "无法创建会话备份目录")?;
                let mut file = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(saved)
                    .map_err(|_| "无法保存原始会话")?;
                file.write_all(&log.original)
                    .and_then(|()| file.sync_all())
                    .map_err(|_| "无法保存原始会话")?;
            }
            let journal = serde_json::json!({"version":1,"root":self.root,"sourceProviderIds":self.sources,"targetProviderId":self.target,"revision":self.revision,
                "logs":self.logs.iter().map(|log| serde_json::json!({"path":log.path,"originalHash":hash(&log.original),"migratedHash":hash(&log.replacement)})).collect::<Vec<_>>(),
                "states":self.states.iter().map(|state| serde_json::json!({"path":state.path,"rows":state.rows})).collect::<Vec<_>>()});
            crate::utils::atomic_write(
                &backup.join("migration.json"),
                &serde_json::to_vec_pretty(&journal).map_err(|_| "无法创建迁移账本")?,
            )
            .map_err(|_| "无法保存迁移账本")?;
            for log in &self.logs {
                self.verify_log(log)?;
            }
            for (index, log) in self.logs.iter().enumerate() {
                self.verify_log(log)?;
                touched.push(index);
                write(&log.path, &log.replacement)?;
            }
            for (state, conn) in self.states.iter().zip(&connections) {
                for (id, provider) in &state.rows {
                    if conn.execute("UPDATE threads SET model_provider=?1 WHERE id=?2 AND model_provider=?3", rusqlite::params![self.target, id, provider]).map_err(|_| "无法迁移历史状态")? != 1 { return Err("历史状态在迁移期间变化".into()); }
                }
            }
            for conn in &connections {
                conn.execute_batch("COMMIT")
                    .map_err(|_| "无法提交历史迁移")?;
                committed += 1;
            }
            Ok(())
        })();
        if let Err(error) = operation {
            for conn in connections.iter().skip(committed) {
                let _ = conn.execute_batch("ROLLBACK");
            }
            let mut rollback_failed = false;
            for (state, conn) in self.states.iter().zip(&connections).take(committed) {
                rollback_failed |= restore_rows(conn, &state.rows, &self.target).is_err();
            }
            for index in touched.into_iter().rev() {
                let log = &self.logs[index];
                let current = if archive::confined(&log.path, &self.root) {
                    read_file(&log.path, MAX_SESSION_FILE_BYTES)
                } else {
                    Err("会话路径已变化".into())
                };
                match current {
                    Ok(bytes) if bytes == log.original => {}
                    Ok(bytes) if bytes == log.replacement => {
                        rollback_failed |=
                            crate::utils::atomic_write(&log.path, &log.original).is_err();
                    }
                    _ => rollback_failed = true,
                }
            }
            return Err(format!(
                "{error}；{}，原始副本保留在 {}",
                if rollback_failed {
                    "部分内容未能回滚"
                } else {
                    "已回滚本次修改"
                },
                backup.display()
            ));
        }
        Ok(CodexHistoryMigrationResult {
            source_provider_ids: self.sources,
            target_provider_id: self.target,
            migrated_jsonl_files: preview.jsonl_files,
            migrated_state_rows: preview.state_rows,
            backup_path: Some(backup.to_string_lossy().into()),
            skipped_reason: None,
        })
    }

    fn verify_log(&self, log: &LogChange) -> Result<(), String> {
        if !archive::confined(&log.path, &self.root)
            || read_file(&log.path, MAX_SESSION_FILE_BYTES)? != log.original
        {
            return Err("会话在检查后发生变化，请重新检查".into());
        }
        let modified = fs::metadata(&log.path)
            .and_then(|m| m.modified())
            .map_err(|_| "无法检查会话更新时间")?;
        if modified
            .elapsed()
            .map_or(true, |age| age < Duration::from_secs(60))
        {
            return Err("会话最近仍在更新，请关闭客户端并重新检查".into());
        }
        Ok(())
    }
}

fn restore_rows(conn: &Connection, rows: &[(String, String)], target: &str) -> Result<(), String> {
    conn.execute_batch("BEGIN IMMEDIATE")
        .map_err(|_| "无法回滚历史状态")?;
    let result = (|| {
        for (id, original) in rows {
            if conn
                .execute(
                    "UPDATE threads SET model_provider=?1 WHERE id=?2 AND model_provider=?3",
                    rusqlite::params![original, id, target],
                )
                .map_err(|_| "无法回滚历史状态")?
                != 1
            {
                return Err("历史状态已变化，保留原始备份".into());
            }
        }
        conn.execute_batch("COMMIT")
            .map_err(|_| "无法提交历史状态回滚".to_string())
    })();
    if result.is_err() {
        let _ = conn.execute_batch("ROLLBACK");
    }
    result
}

#[cfg(test)]
mod tests;
