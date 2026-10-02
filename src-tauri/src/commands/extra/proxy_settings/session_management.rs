use super::super::types::SessionDeleteTarget;
use super::{
    session_tasks::{self, SessionDeletePlan, SessionRestorePlan},
    session_trash,
};
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
    mut delete: impl FnMut(usize, &SessionDeleteTarget) -> Result<(), String>,
) -> SessionBatchDeleteResult {
    let mut result = SessionBatchDeleteResult::default();
    let mut seen = std::collections::HashSet::new();
    for (index, target) in targets.into_iter().enumerate() {
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
        match delete(index, &target) {
            Ok(()) => result.deleted.push(target),
            Err(error) => result.failed.push(SessionDeleteFailure { target, error }),
        }
    }
    result
}

#[tauri::command]
pub async fn delete_sessions_checked(
    sessions: Vec<SessionDeleteTarget>,
    db: State<'_, DbState>,
) -> Result<SessionBatchDeleteResult, String> {
    let permit = session_tasks::mutation_permit().await;
    let plans = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        sessions
            .iter()
            .cloned()
            .map(|target| SessionDeletePlan::prepare(&conn, target))
            .collect::<Vec<_>>()
    };
    session_tasks::mutate(permit, move || {
        Ok(delete_batch(sessions, |index, _| {
            plans[index].as_ref().map_err(Clone::clone)?.execute()
        }))
    })
    .await
}

#[tauri::command]
pub async fn list_session_trash() -> Result<Vec<session_trash::TrashedSession>, String> {
    let permit = session_tasks::mutation_permit().await;
    session_tasks::mutate(permit, || session_trash::list(&session_trash::directory()?)).await
}

#[tauri::command]
pub async fn restore_session_trash(key: String, db: State<'_, DbState>) -> Result<(), String> {
    let permit = session_tasks::mutation_permit().await;
    let plan = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        SessionRestorePlan::prepare(&conn, key)?
    };
    session_tasks::mutate(permit, move || plan.execute(&session_trash::directory()?)).await
}

#[tauri::command]
pub async fn purge_session_trash(
    targets: Vec<session_trash::SessionPurgeTarget>,
) -> Result<session_trash::SessionPurgeResult, String> {
    let permit = session_tasks::mutation_permit().await;
    session_tasks::mutate(permit, move || {
        session_trash::purge(&session_trash::directory()?, targets)
    })
    .await
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
            |_, t| {
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
