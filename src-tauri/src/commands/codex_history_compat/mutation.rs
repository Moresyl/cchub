//! Shared guarded writes for history migration and selective recovery.
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use rusqlite::{Connection, OptionalExtension};
use serde_json::Value;

use super::migration::{hash, open_state, read_file};
use super::{MAX_BACKUP_BYTES, MAX_SESSION_FILE_BYTES};
use crate::shared::session_archive as archive;

pub(super) struct LogChange {
    pub path: PathBuf,
    pub original: Vec<u8>,
    pub replacement: Vec<u8>,
}

pub(super) struct RowChange {
    pub id: String,
    pub original: String,
    pub replacement: String,
}

pub(super) struct StateChange {
    pub path: PathBuf,
    pub rows: Vec<RowChange>,
}

pub(super) struct MutationPlan {
    pub root: PathBuf,
    pub logs: Vec<LogChange>,
    pub states: Vec<StateChange>,
}

pub(super) fn inactive(path: &Path) -> Result<(), String> {
    let modified = fs::metadata(path)
        .and_then(|meta| meta.modified())
        .map_err(|_| "无法检查会话更新时间")?;
    if modified
        .elapsed()
        .map_or(true, |age| age < Duration::from_secs(60))
    {
        return Err("会话最近仍在更新，请关闭客户端并重新检查".into());
    }
    Ok(())
}

pub(super) fn provider(conn: &Connection, id: &str) -> Result<Option<String>, String> {
    conn.query_row(
        "SELECT substr(model_provider,1,129) FROM threads WHERE id=?1",
        [id],
        |row| row.get::<_, Option<String>>(0),
    )
    .optional()
    .map(Option::flatten)
    .map_err(|_| "无法检查历史状态记录".into())
}

impl MutationPlan {
    fn verify_log(&self, log: &LogChange) -> Result<(), String> {
        if !archive::confined(&log.path, &self.root)
            || read_file(&log.path, MAX_SESSION_FILE_BYTES)? != log.original
        {
            return Err("会话在检查后发生变化，请重新检查".into());
        }
        inactive(&log.path)
    }

    pub(super) fn execute_with(
        self,
        backups: &Path,
        kind: &str,
        mut journal: Value,
        mut write: impl FnMut(&Path, &[u8]) -> Result<(), String>,
    ) -> Result<PathBuf, String> {
        fs::create_dir_all(backups).map_err(|_| "无法创建历史备份目录")?;
        let backup_parent = backups.canonicalize().map_err(|_| "无法确定历史备份目录")?;
        let root = self
            .root
            .canonicalize()
            .map_err(|_| "无法确定历史配置目录")?;
        if backup_parent.starts_with(&root) {
            return Err("历史备份目录必须位于会话目录之外".into());
        }
        let backup = backup_parent.join(format!("codex-history-{kind}-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&backup).map_err(|_| "无法创建历史原始副本目录")?;
        let mut connections = Vec::new();
        let mut touched = Vec::new();
        let mut committed = 0usize;
        let operation: Result<(), String> = (|| {
            for (index, state) in self.states.iter().enumerate() {
                if !archive::confined(&state.path, &self.root) {
                    return Err("历史数据库路径在检查后变化".into());
                }
                let conn = open_state(&state.path, true)?;
                conn.execute_batch("BEGIN IMMEDIATE")
                    .map_err(|_| "历史数据库正在使用，请稍后重试")?;
                for row in &state.rows {
                    if provider(&conn, &row.id)?.as_deref() != Some(row.original.as_str()) {
                        return Err("历史状态在检查后变化，请重新检查".into());
                    }
                }
                let source = open_state(&state.path, false)?;
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
                    .and_then(|file| file.sync_all())
                    .map_err(|_| "无法保存状态备份")?;
                // New journals authenticate their complete snapshot, including WAL data.
                journal["states"][index]["originalHash"] =
                    hash(&read_file(&saved, MAX_SESSION_FILE_BYTES)?).into();
                connections.push(conn);
            }
            for log in &self.logs {
                let saved = backup.join("jsonl").join(
                    log.path
                        .strip_prefix(&self.root)
                        .map_err(|_| "会话备份路径无效")?,
                );
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
            let journal_bytes =
                serde_json::to_vec_pretty(&journal).map_err(|_| "无法创建历史账本")?;
            if journal_bytes.len() as u64 > MAX_BACKUP_BYTES {
                return Err("历史账本超过 64 MiB，写入已停止".into());
            }
            crate::utils::atomic_write(&backup.join(format!("{kind}.json")), &journal_bytes)
                .map_err(|_| "无法保存历史账本")?;
            for log in &self.logs {
                self.verify_log(log)?;
            }
            for (index, log) in self.logs.iter().enumerate() {
                self.verify_log(log)?;
                touched.push(index);
                write(&log.path, &log.replacement)?;
            }
            for (state, conn) in self.states.iter().zip(&connections) {
                update_rows(conn, &state.rows, false)?;
            }
            for conn in &connections {
                conn.execute_batch("COMMIT")
                    .map_err(|_| "无法提交历史修改")?;
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
                let result = conn
                    .execute_batch("BEGIN IMMEDIATE")
                    .map_err(|_| "无法回滚历史状态".into())
                    .and_then(|()| update_rows(conn, &state.rows, true))
                    .and_then(|()| {
                        conn.execute_batch("COMMIT")
                            .map_err(|_| "无法提交历史状态回滚".into())
                    });
                if result.is_err() {
                    let _ = conn.execute_batch("ROLLBACK");
                    rollback_failed = true;
                }
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
        Ok(backup)
    }
}

fn update_rows(conn: &Connection, rows: &[RowChange], reverse: bool) -> Result<(), String> {
    for row in rows {
        let (before, after) = if reverse {
            (&row.replacement, &row.original)
        } else {
            (&row.original, &row.replacement)
        };
        if conn
            .execute(
                "UPDATE threads SET model_provider=?1 WHERE id=?2 AND model_provider=?3",
                rusqlite::params![after, row.id, before],
            )
            .map_err(|_| "无法更新历史状态")?
            != 1
        {
            return Err("历史状态在写入期间变化".into());
        }
    }
    Ok(())
}
