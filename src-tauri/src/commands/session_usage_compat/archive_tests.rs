use super::*;

#[test]
fn packing_and_repacking_imports_keep_original_request_ids_and_database_totals() {
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().join(".codex/sessions");
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("rollout.jsonl");
    let bytes = b"{\"payload\":{\"type\":\"token_count\",\"info\":{\"total_token_usage\":{\"input_tokens\":100,\"output_tokens\":10}}}}\n{\"payload\":{\"type\":\"token_count\",\"info\":{\"total_token_usage\":{\"input_tokens\":140,\"output_tokens\":15}}}}\n";
    fs::write(&path, bytes).unwrap();
    let mut conn = rusqlite::Connection::open_in_memory().unwrap();
    crate::db::schema::run_migrations(&conn).unwrap();
    let mut result = SessionSyncResult::default();
    let mut records = Vec::new();
    scan_file(&path, &mut result, &mut records, Some("codex"));
    assert_eq!(records.len(), 2);
    let ids = records
        .iter()
        .map(|record| record.id.clone())
        .collect::<Vec<_>>();
    persist_records(&mut conn, records, &mut result);
    assert_eq!(result.imported, 2);
    let packed = crate::shared::session_archive::twin(&path).unwrap();
    for level in [0, 3] {
        fs::write(
            &packed,
            zstd::stream::encode_all(&bytes[..], level).unwrap(),
        )
        .unwrap();
        if path.exists() {
            fs::remove_file(&path).unwrap();
        }
        let mut records = Vec::new();
        scan_file(&packed, &mut result, &mut records, Some("codex"));
        assert_eq!(
            records
                .iter()
                .map(|record| record.id.clone())
                .collect::<Vec<_>>(),
            ids
        );
        persist_records(&mut conn, records, &mut result);
    }
    let totals: (i64, i64, i64) = conn
        .query_row(
            "SELECT COUNT(*),SUM(input_tokens),SUM(output_tokens) FROM proxy_request_logs",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(totals, (2, 140, 15));
    assert!(result.errors.is_empty());
}

#[test]
fn corrupt_archive_discards_its_pending_records_and_preserves_prior_files() {
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().join(".codex/sessions");
    fs::create_dir_all(&dir).unwrap();
    let first = dir.join("first.jsonl");
    let bytes = b"{\"usage\":{\"input_tokens\":12,\"output_tokens\":7}}\n";
    fs::write(&first, bytes).unwrap();
    let mut result = SessionSyncResult::default();
    let mut records = Vec::new();
    scan_file(&first, &mut result, &mut records, None);
    assert_eq!(records.len(), 1);
    let packed = dir.join("broken.jsonl.zst");
    let mut content = zstd::stream::encode_all(&bytes[..], 0).unwrap();
    content.extend(b"invalid trailing frame");
    fs::write(&packed, content).unwrap();
    scan_file(&packed, &mut result, &mut records, None);
    assert_eq!(records.len(), 1);
    assert_eq!(result.deferred_files, 1);
    assert!(!result.errors.is_empty());
}
