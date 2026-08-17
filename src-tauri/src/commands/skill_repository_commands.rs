use std::collections::HashSet;
use std::path::Path;

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::commands::extra_commands::{
    get_json_app_setting, resolve_tool_skills_dir, set_json_app_setting,
};
use crate::db::{record_activity, DbState};
use crate::mcp::registry::{self, SkillRegistryEntry};
use crate::skills::{installer, updater};

const SKILL_REPOS_SETTING_KEY: &str = "skill_repositories";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillRepo {
    pub owner: String,
    pub name: String,
    #[serde(default = "default_branch")]
    pub branch: String,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_branch() -> String {
    "main".to_string()
}

fn default_enabled() -> bool {
    true
}

fn validate_repo(repo: &SkillRepo) -> Result<(), String> {
    let valid_segment = |value: &str| {
        !value.trim().is_empty()
            && value.len() <= 128
            && !value.contains(['/', '\\', ':', '?', '#', '%'])
            && value != "."
            && value != ".."
    };
    if !valid_segment(&repo.owner) || !valid_segment(&repo.name) {
        return Err("Skill repository owner/name is invalid".to_string());
    }
    if !valid_segment(&repo.branch) {
        return Err("Skill repository branch is invalid".to_string());
    }
    Ok(())
}

fn read_repos(conn: &rusqlite::Connection) -> Result<Vec<SkillRepo>, String> {
    Ok(get_json_app_setting(conn, SKILL_REPOS_SETTING_KEY)?.unwrap_or_default())
}

fn write_repos(conn: &rusqlite::Connection, repos: &[SkillRepo]) -> Result<(), String> {
    set_json_app_setting(conn, SKILL_REPOS_SETTING_KEY, &repos.to_vec())
}

fn proxy_url(conn: &rusqlite::Connection) -> String {
    conn.query_row(
        "SELECT value FROM app_settings WHERE key = 'proxy_url'",
        [],
        |row| row.get::<_, String>(0),
    )
    .unwrap_or_default()
}

#[tauri::command]
pub fn get_skill_repos(db: State<'_, DbState>) -> Result<Vec<SkillRepo>, String> {
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    read_repos(&conn)
}

#[tauri::command]
pub fn add_skill_repo(repo: SkillRepo, db: State<'_, DbState>) -> Result<bool, String> {
    let mut repo = repo;
    repo.owner = repo.owner.trim().to_string();
    repo.name = repo.name.trim().trim_end_matches(".git").to_string();
    repo.branch = repo.branch.trim().to_string();
    validate_repo(&repo)?;
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    let mut repos = read_repos(&conn)?;
    repos.retain(|item| {
        !(item.owner.eq_ignore_ascii_case(&repo.owner)
            && item.name.eq_ignore_ascii_case(&repo.name))
    });
    repos.push(repo);
    write_repos(&conn, &repos)?;
    Ok(true)
}

#[tauri::command]
pub fn remove_skill_repo(
    owner: String,
    name: String,
    db: State<'_, DbState>,
) -> Result<bool, String> {
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    let mut repos = read_repos(&conn)?;
    let before = repos.len();
    repos.retain(|item| {
        !(item.owner.eq_ignore_ascii_case(owner.trim())
            && item.name.eq_ignore_ascii_case(name.trim()))
    });
    if before == repos.len() {
        return Ok(false);
    }
    write_repos(&conn, &repos)?;
    Ok(true)
}

async fn discover_available_skills_for_db(
    repo_ids: Option<Vec<String>>,
    db: &DbState,
) -> Result<Vec<SkillRegistryEntry>, String> {
    let (repos, proxy) = {
        let conn = db.0.lock().map_err(|error| error.to_string())?;
        (read_repos(&conn)?, proxy_url(&conn))
    };
    let selected = repo_ids.map(|ids| ids.into_iter().collect::<HashSet<_>>());
    let mut result = Vec::new();
    let mut seen = HashSet::new();
    for repo in repos.into_iter().filter(|repo| repo.enabled) {
        let key = format!("{}/{}", repo.owner, repo.name);
        if selected.as_ref().is_some_and(|ids| !ids.contains(&key)) {
            continue;
        }
        let entries = registry::fetch_skills_from_github_repo(
            &repo.owner,
            &repo.name,
            &repo.branch,
            Some(proxy.as_str()),
        )
        .await?;
        for entry in entries {
            if seen.insert(entry.id.clone()) {
                result.push(entry);
            }
        }
    }
    Ok(result)
}

#[tauri::command]
pub async fn discover_available_skills(
    repo_ids: Option<Vec<String>>,
    db: State<'_, DbState>,
) -> Result<Vec<SkillRegistryEntry>, String> {
    discover_available_skills_for_db(repo_ids, db.inner()).await
}

fn safe_skill_file_name(name: &str) -> Result<String, String> {
    let trimmed = name.trim();
    let path = Path::new(trimmed);
    if path.components().count() > 1 || trimmed.contains(['/', '\\']) {
        return Err("Skill name must not contain a path".to_string());
    }
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or(trimmed)
        .trim();
    if stem.is_empty() || stem == "." || stem == ".." {
        return Err("Skill name is empty".to_string());
    }
    let safe = stem
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>();
    Ok(format!("{safe}.md"))
}

fn install_entry(
    entry: &SkillRegistryEntry,
    target_dir: &Path,
    conn: &rusqlite::Connection,
) -> Result<String, String> {
    std::fs::create_dir_all(target_dir).map_err(|error| error.to_string())?;
    let file_path = target_dir.join(safe_skill_file_name(&entry.name)?);
    crate::utils::atomic_write_string(&file_path, &entry.content)
        .map_err(|error| format!("Failed to write skill: {error}"))?;
    let file_path_text = file_path.to_string_lossy().to_string();
    updater::persist_marketplace_skill_install(
        conn,
        &file_path_text,
        &entry.name,
        Some(&entry.description),
        None,
        entry.github_url.as_deref(),
        &entry.content,
    )?;
    record_activity(conn, &entry.name, "skill_install", "success", None);
    Ok(file_path_text)
}

async fn find_available_skill(
    directory: &str,
    db: &State<'_, DbState>,
) -> Result<SkillRegistryEntry, String> {
    let entries = discover_available_skills_for_db(None, db.inner()).await?;
    entries
        .into_iter()
        .find(|entry| {
            entry.id.eq_ignore_ascii_case(directory)
                || entry.name.eq_ignore_ascii_case(directory)
                || entry
                    .github_url
                    .as_deref()
                    .is_some_and(|url| url.ends_with(directory))
        })
        .ok_or_else(|| format!("Skill not found in configured repositories: {directory}"))
}

#[tauri::command]
pub async fn install_skill(directory: String, db: State<'_, DbState>) -> Result<bool, String> {
    install_skill_for_app("claude".to_string(), directory, db).await
}

#[tauri::command]
pub async fn install_skill_for_app(
    app: String,
    directory: String,
    db: State<'_, DbState>,
) -> Result<bool, String> {
    let entry = find_available_skill(&directory, &db).await?;
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    let target = resolve_tool_skills_dir(&conn, &app)?;
    install_entry(&entry, &target, &conn)?;
    Ok(true)
}

#[tauri::command]
pub fn uninstall_skill(directory: String, db: State<'_, DbState>) -> Result<bool, String> {
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    let path = conn
        .query_row(
            "SELECT file_path FROM skills WHERE id = ?1 OR name = ?1 OR file_path = ?1 LIMIT 1",
            rusqlite::params![directory.trim()],
            |row| row.get::<_, Option<String>>(0),
        )
        .map_err(|error| error.to_string())?
        .ok_or_else(|| format!("Installed skill not found: {directory}"))?;
    installer::uninstall_skill_file(&path)?;
    let _ = updater::remove_skill_metadata(&conn, &path);
    Ok(true)
}

#[tauri::command]
pub fn toggle_skill_app(
    id: String,
    app: String,
    enabled: bool,
    db: State<'_, DbState>,
) -> Result<bool, String> {
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    let (path, name): (String, String) = conn
        .query_row(
            "SELECT file_path, name FROM skills WHERE id = ?1 OR file_path = ?1 LIMIT 1",
            rusqlite::params![id.trim()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|error| error.to_string())?;
    let target_dir = resolve_tool_skills_dir(&conn, &app)?;
    if enabled {
        installer::copy_skill_between_tools(&path, &target_dir.to_string_lossy(), "copy")?;
    } else {
        let target = target_dir.join(safe_skill_file_name(&name)?);
        if target.exists() {
            installer::uninstall_skill_file(&target.to_string_lossy())?;
        }
    }
    Ok(true)
}

#[tauri::command]
pub fn get_installed_skills(
    db: State<'_, DbState>,
) -> Result<Vec<crate::db::models::Skill>, String> {
    let plan = {
        let conn = db.0.lock().map_err(|error| error.to_string())?;
        crate::skills::scanner::prepare_local_skill_scan(&conn)
    };
    let scanned = crate::skills::scanner::scan_local_skills_from_plan(&plan);
    if !scanned.is_empty() {
        return Ok(scanned);
    }
    crate::commands::skill_commands::get_skills(db)
}

#[tauri::command]
pub fn get_skills_for_app(
    app: String,
    db: State<'_, DbState>,
) -> Result<Vec<crate::db::models::Skill>, String> {
    let app = app.trim().to_ascii_lowercase();
    if app.is_empty() {
        return Err("Application id is required".to_string());
    }
    let plan = {
        let conn = db.0.lock().map_err(|error| error.to_string())?;
        crate::skills::scanner::prepare_local_skill_scan(&conn)
    };
    let scanned = crate::skills::scanner::scan_local_skills_from_plan(&plan);
    Ok(scanned
        .into_iter()
        .filter(|skill| skill.tool_id.as_deref() == Some(app.as_str()))
        .collect())
}

#[tauri::command]
pub async fn update_skill(id: String, db: State<'_, DbState>) -> Result<bool, String> {
    Ok(crate::commands::skill_commands::batch_update_skills(vec![id], db).await? > 0)
}

#[tauri::command]
pub fn install_skills_from_zip(
    file_path: String,
    current_app: String,
    db: State<'_, DbState>,
) -> Result<Vec<String>, String> {
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    let target_dir = resolve_tool_skills_dir(&conn, &current_app)?;
    let installed =
        installer::install_skill_file(&file_path, &target_dir.to_string_lossy(), "copy")?;
    Ok(vec![installed])
}

#[tauri::command]
pub async fn search_skills_sh(
    query: String,
    limit: usize,
    _offset: usize,
    db: State<'_, DbState>,
) -> Result<Vec<SkillRegistryEntry>, String> {
    let proxy = {
        let conn = db.0.lock().map_err(|error| error.to_string())?;
        proxy_url(&conn)
    };
    registry::search_skillhub(query.trim(), limit as u32, Some(proxy.as_str())).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo(owner: &str, name: &str) -> SkillRepo {
        SkillRepo {
            owner: owner.to_string(),
            name: name.to_string(),
            branch: "main".to_string(),
            enabled: true,
        }
    }

    #[test]
    fn validates_repository_segments() {
        assert!(validate_repo(&repo("acme", "skills")).is_ok());
        assert!(validate_repo(&repo("acme/org", "skills")).is_err());
        assert!(validate_repo(&repo("", "skills")).is_err());
        assert!(validate_repo(&repo("acme", "..")).is_err());
    }

    #[test]
    fn sanitizes_skill_file_names_without_accepting_paths() {
        assert_eq!(
            safe_skill_file_name("code-review"),
            Ok("code-review.md".to_string())
        );
        assert_eq!(
            safe_skill_file_name("my skill"),
            Ok("my-skill.md".to_string())
        );
        assert!(safe_skill_file_name("../secrets").is_err());
        assert!(safe_skill_file_name("/").is_err());
    }
}
