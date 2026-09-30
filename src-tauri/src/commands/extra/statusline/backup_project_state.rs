use super::super::config_profiles::{get_json_app_setting, set_json_app_setting};
use rusqlite::Connection;
use std::collections::BTreeSet;

const DEFERRED_ROOTS_KEY: &str = "deferred_imported_project_roots";

pub(super) fn deferred_roots(conn: &Connection) -> Result<BTreeSet<String>, String> {
    get_json_app_setting(conn, DEFERRED_ROOTS_KEY).map(|roots| roots.unwrap_or_default())
}

#[cfg(test)]
pub(super) fn defer_project_root(conn: &Connection, root: &str) -> Result<(), String> {
    defer_project_roots(conn, std::iter::once(root))
}

pub(super) fn defer_project_roots<'a>(
    conn: &Connection,
    incoming: impl IntoIterator<Item = &'a str>,
) -> Result<(), String> {
    let mut roots = deferred_roots(conn)?;
    for root in incoming {
        let root = super::normalize_project_root_path(root).ok_or("无效的项目备份路径")?;
        roots.insert(root.to_string());
    }
    set_json_app_setting(conn, DEFERRED_ROOTS_KEY, &roots)
}

pub(super) fn release_project_root(conn: &Connection, root: &str) -> Result<(), String> {
    let root = super::normalize_project_root_path(root).ok_or("无效的项目备份路径")?;
    let mut roots = deferred_roots(conn)?;
    roots.remove(root);
    set_json_app_setting(conn, DEFERRED_ROOTS_KEY, &roots)
}
