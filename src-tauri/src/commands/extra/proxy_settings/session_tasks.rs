//! Owned session plans: no application connection travels into file workers.
use std::path::{Path, PathBuf};

use super::super::config_profiles::{resolve_tool_config_dir, session_root_candidates_for_tool};
use super::super::types::SessionDeleteTarget;
use super::{delete_grokbuild_session, delete_opencode_session, session_trash};
use crate::shared::session_archive as archive;

static MUTATIONS: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
static READERS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(4);

/// The permit must enter the blocking closure: dropping an IPC future does not
/// stop spawn_blocking work, so releasing it on the awaiting side would race.
pub(crate) async fn mutation_permit() -> tokio::sync::MutexGuard<'static, ()> {
    MUTATIONS.lock().await
}

pub(crate) async fn read<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    let permit = READERS.acquire().await.map_err(|e| e.to_string())?;
    blocking(move || {
        let _permit = permit;
        work()
    })
    .await
}

pub(crate) async fn mutate<T: Send + 'static>(
    permit: tokio::sync::MutexGuard<'static, ()>,
    work: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    blocking(move || {
        let _permit = permit;
        work()
    })
    .await
}

async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tokio::task::spawn_blocking(work).await.map_err(|error| {
        if error.is_panic() {
            "Session file task failed: worker panicked".to_string()
        } else {
            "Session file task failed: worker cancelled".to_string()
        }
    })?
}

pub(super) struct SessionAccessPlan {
    tool_id: String,
    roots: Vec<PathBuf>,
}

impl SessionAccessPlan {
    pub(super) fn prepare(conn: &rusqlite::Connection, tool_id: &str) -> Result<Self, String> {
        Ok(Self {
            tool_id: tool_id.into(),
            roots: session_root_candidates_for_tool(conn, tool_id)?,
        })
    }

    pub(super) fn allows(&self, source_path: &str) -> bool {
        let source = PathBuf::from(source_path);
        let source = if self.tool_id == "codex" && archive::jsonl(&source) {
            match archive::resolve(&source) {
                Ok(path) => path,
                Err(_) => return false,
            }
        } else {
            source
        };
        self.roots
            .iter()
            .any(|root| archive::confined(&source, root))
    }
}

pub(super) struct SessionDeletePlan {
    access: SessionAccessPlan,
    root: PathBuf,
    target: SessionDeleteTarget,
}

impl SessionDeletePlan {
    pub(super) fn prepare(
        conn: &rusqlite::Connection,
        target: SessionDeleteTarget,
    ) -> Result<Self, String> {
        Ok(Self {
            access: SessionAccessPlan::prepare(conn, &target.tool_id)?,
            root: resolve_tool_config_dir(conn, &target.tool_id)?,
            target,
        })
    }

    pub(super) fn execute(&self) -> Result<(), String> {
        self.execute_with_trash(session_trash::directory)
    }

    fn execute_with_trash(
        &self,
        trash: impl FnOnce() -> Result<PathBuf, String>,
    ) -> Result<(), String> {
        let target = &self.target;
        if !self.access.allows(&target.source_path) {
            return Err("Invalid session source path".into());
        }
        match (target.tool_id.as_str(), target.source_backend.as_str()) {
            (_, "mcode_sqlite") => Err("MiniMax Code sessions are read-only".into()),
            (_, "grokbuild_native") => delete_grokbuild_session(
                &self.root,
                Path::new(&target.source_path),
                &target.session_id,
            ),
            ("opencode", "opencode_sqlite") => {
                delete_opencode_session(Path::new(&target.source_path), &target.session_id)
            }
            ("codex", "jsonl") => {
                session_trash::delete(target, &self.access.roots, &trash()?).map(|_| ())
            }
            (_, "jsonl") => std::fs::remove_file(&target.source_path).map_err(|e| e.to_string()),
            _ => Err("This session backend does not support deletion".into()),
        }
    }
}

pub(super) struct SessionRestorePlan {
    roots: Vec<PathBuf>,
    key: String,
}

impl SessionRestorePlan {
    pub(super) fn prepare(conn: &rusqlite::Connection, key: String) -> Result<Self, String> {
        Ok(Self {
            roots: session_root_candidates_for_tool(conn, "codex")?,
            key,
        })
    }

    pub(super) fn execute(&self, trash: &Path) -> Result<(), String> {
        session_trash::restore(trash, &self.key, &self.roots)
    }
}

#[cfg(test)]
mod tests;
