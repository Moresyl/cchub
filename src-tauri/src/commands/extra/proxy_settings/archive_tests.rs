use super::super::config_profiles::*;
use super::*;
use crate::shared::session_archive as archive;
use std::path::{Path, PathBuf};

fn rollout(root: &Path) -> PathBuf {
    let dir = root.join("sessions");
    std::fs::create_dir(&dir).unwrap();
    let path = dir.join("rollout-same.jsonl");
    let mut data = concat!(
        "{\"type\":\"session_meta\",\"payload\":{\"id\":\"same\",\"title\":\"Archive title\"}}\n",
        "{\"type\":\"response_item\",\"payload\":{\"type\":\"message\",\"role\":\"user\",\"content\":[{\"type\":\"input_text\",\"text\":\"归档问题\"}]}}\n",
        "{\"type\":\"response_item\",\"payload\":{\"type\":\"function_call\",\"name\":\"shell\",\"arguments\":\"pwd\"}}\n"
    ).to_owned();
    data.push_str(&"{\"type\":\"empty\"}\n".repeat(2200));
    data.push_str("{\"type\":\"event_msg\",\"payload\":{\"type\":\"token_count\",\"info\":{\"total_token_usage\":{\"input_tokens\":180,\"output_tokens\":40,\"total_tokens\":220},\"last_token_usage\":{\"input_tokens\":50,\"output_tokens\":10}}}}\n");
    std::fs::write(&path, data).unwrap();
    path
}

fn pack(path: &Path) -> PathBuf {
    let packed = archive::twin(path).unwrap();
    std::fs::write(
        &packed,
        zstd::stream::encode_all(std::fs::read(path).unwrap().as_slice(), 0).unwrap(),
    )
    .unwrap();
    packed
}

#[test]
fn archive_list_detail_and_latest_totals_survive_coexistence_and_packing() {
    let root = tempfile::tempdir().unwrap();
    let path = rollout(root.path());
    let roots = vec![root.path().to_path_buf()];
    let before = scan_codex_sessions_from_plan(Some(root.path()), &[], &roots, "");
    assert_eq!(before.len(), 1);
    assert_eq!(before[0].tokens_used, Some(220));
    let detail = load_session_detail(&before[0]).unwrap();
    assert_eq!(detail.entries.len(), 2);
    let packed = pack(&path);
    let both = scan_codex_sessions_from_plan(Some(root.path()), &[], &roots, "");
    assert_eq!(both.len(), 1);
    assert_eq!(both[0].source_path, path.to_string_lossy());
    std::fs::remove_file(&path).unwrap();
    let after = scan_codex_sessions_from_plan(Some(root.path()), &[], &roots, "");
    assert_eq!(after.len(), 1);
    assert_eq!(after[0].id, before[0].id);
    assert_eq!(after[0].source_path, packed.to_string_lossy());
    assert_eq!(after[0].tokens_used, Some(220));
    assert_eq!(
        serde_json::to_value(load_session_detail(&before[0]).unwrap().entries).unwrap(),
        serde_json::to_value(detail.entries).unwrap()
    );
}

#[test]
fn native_relative_paths_resolve_archives_and_missing_files_do_not_create_phantom_rows() {
    let root = tempfile::tempdir().unwrap();
    let path = rollout(root.path());
    let db = root.path().join("state_5.sqlite");
    let conn = rusqlite::Connection::open(&db).unwrap();
    conn.execute_batch("CREATE TABLE threads(id TEXT,rollout_path TEXT,created_at INTEGER,updated_at INTEGER,cwd TEXT,title TEXT,first_user_message TEXT);").unwrap();
    conn.execute("INSERT INTO threads VALUES('same','sessions/rollout-same.jsonl',1700000000,1700000001,'C:/fixture','Database title','first')",[]).unwrap();
    drop(conn);
    let original_db = std::fs::read(&db).unwrap();
    let packed = pack(&path);
    std::fs::remove_file(&path).unwrap();
    let roots = vec![root.path().to_path_buf()];
    let sessions = scan_codex_sessions_from_plan(Some(root.path()), &[db.clone()], &roots, "");
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].source_path, packed.to_string_lossy());
    assert_eq!(sessions[0].title, "Database title");
    assert_eq!(sessions[0].tokens_used, Some(220));
    assert_eq!(std::fs::read(&db).unwrap(), original_db);
    std::fs::remove_file(packed).unwrap();
    assert!(scan_codex_sessions_from_plan(Some(root.path()), &[db], &roots, "").is_empty());
}

#[test]
fn compressed_indexes_and_non_codex_archives_are_not_deletable_sessions() {
    let root = tempfile::tempdir().unwrap();
    let bytes = zstd::stream::encode_all(&b"{}\n"[..], 0).unwrap();
    for name in ["history.jsonl.zst", "session_index.jsonl.zst"] {
        let path = root.path().join(name);
        std::fs::write(&path, &bytes).unwrap();
        assert!(!is_session_candidate_path("codex", &path, root.path()));
    }
    let path = root.path().join("sessions-rollout.jsonl.zst");
    std::fs::write(&path, &bytes).unwrap();
    assert!(!is_session_candidate_path("claude", &path, root.path()));
    assert!(is_session_candidate_path("codex", &path, root.path()));
    assert!(parse_codex_session_entries(&path).is_ok());
    std::fs::write(&path, b"not zstd").unwrap();
    assert!(parse_codex_session_entries(&path).is_err());
    assert!(parse_generic_jsonl_session_summary("codex", &path, "").is_none());
}

#[test]
fn detail_authorization_resolves_missing_twins_without_lexical_path_escape() {
    let root = tempfile::tempdir().unwrap();
    let path = rollout(root.path());
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    crate::db::schema::run_migrations(&conn).unwrap();
    conn.execute(
        "INSERT INTO custom_paths(tool_id,config_dir) VALUES('codex',?1)",
        [root.path().to_string_lossy().to_string()],
    )
    .unwrap();
    pack(&path);
    std::fs::remove_file(&path).unwrap();
    assert!(is_valid_session_source_path(
        &conn,
        "codex",
        &path.to_string_lossy()
    ));
    assert!(!is_valid_session_source_path(
        &conn,
        "codex",
        &root.path().join("../outside.jsonl").to_string_lossy()
    ));
}
