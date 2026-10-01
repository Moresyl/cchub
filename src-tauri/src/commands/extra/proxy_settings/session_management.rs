use super::super::config_profiles::session_roots_for_tool;
use super::super::types::SessionDeleteTarget;
use super::{delete_session_impl, session_trash};
use crate::db::DbState;
use serde::Serialize;
use tauri::State;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionDeleteFailure {
    pub target: SessionDeleteTarget,
    pub error: String,
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SessionBatchDeleteResult {
    pub deleted: Vec<SessionDeleteTarget>,
    pub failed: Vec<SessionDeleteFailure>,
}

fn delete_batch(
    targets: Vec<SessionDeleteTarget>,
    mut delete: impl FnMut(&SessionDeleteTarget) -> Result<(), String>,
) -> SessionBatchDeleteResult {
    let mut result = SessionBatchDeleteResult::default();
    let mut seen = std::collections::HashSet::new();
    for target in targets {
        let path = if target.tool_id == "codex" {
            crate::shared::session_archive::logical_path(std::path::Path::new(&target.source_path))
        } else {
            std::path::PathBuf::from(&target.source_path)
        };
        if !seen.insert((
            target.tool_id.clone(),
            target.session_id.clone(),
            path,
            target.source_backend.clone(),
        )) {
            continue;
        }
        match delete(&target) {
            Ok(()) => result.deleted.push(target),
            Err(error) => result.failed.push(SessionDeleteFailure { target, error }),
        }
    }
    result
}

#[tauri::command]
pub fn delete_sessions_checked(
    sessions: Vec<SessionDeleteTarget>,
    db: State<'_, DbState>,
) -> Result<SessionBatchDeleteResult, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    Ok(delete_batch(sessions, |target| {
        delete_session_impl(
            &conn,
            &target.tool_id,
            &target.session_id,
            &target.source_path,
            &target.source_backend,
        )
    }))
}

#[tauri::command]
pub fn list_session_trash() -> Result<Vec<session_trash::TrashedSession>, String> {
    session_trash::list(&session_trash::directory()?)
}

#[tauri::command]
pub fn restore_session_trash(key: String, db: State<'_, DbState>) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let roots = session_roots_for_tool(&conn, "codex")?;
    session_trash::restore(&session_trash::directory()?, &key, &roots)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn continues_after_failure_and_deduplicates_only_exact_sources() {
        let target = |path: &str| SessionDeleteTarget {
            tool_id: "codex".into(),
            session_id: "same".into(),
            source_backend: "jsonl".into(),
            source_path: path.into(),
        };
        let result = delete_batch(
            vec![
                target("one.jsonl"),
                target("bad.jsonl"),
                target("two.jsonl"),
                target("one.jsonl.zst"),
            ],
            |t| {
                if t.source_path.starts_with("bad") {
                    Err("fixture failure".into())
                } else {
                    Ok(())
                }
            },
        );
        assert_eq!(result.deleted.len(), 2);
        assert_eq!(result.failed.len(), 1);
        assert_eq!(result.failed[0].target.source_path, "bad.jsonl");
    }
    #[test]
    fn accepts_existing_snake_case_and_camel_case_delete_targets() {
        for value in [
            serde_json::json!({"tool_id":"codex","session_id":"s","source_path":"one.jsonl","source_backend":"jsonl"}),
            serde_json::json!({"toolId":"codex","sessionId":"s","sourcePath":"one.jsonl","sourceBackend":"jsonl"}),
        ] {
            let target: SessionDeleteTarget = serde_json::from_value(value).unwrap();
            assert_eq!(target.session_id, "s");
        }
    }
}
