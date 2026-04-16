use crate::db::models::Skill;
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, USER_AGENT};
use rusqlite::Connection;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, serde::Serialize)]
pub struct SkillUpdateStatus {
    pub id: String,
    pub update_available: bool,
    pub latest_sha256: Option<String>,
    pub current_sha256: Option<String>,
    pub last_checked_at: Option<i64>,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SkillSourceMetadata {
    pub source_url: Option<String>,
    pub baseline_sha256: Option<String>,
    pub latest_sha256: Option<String>,
    pub last_checked_at: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum GitHubContentResponse {
    File(GitHubEntry),
    Directory(Vec<GitHubEntry>),
}

#[derive(Debug, Deserialize)]
struct GitHubEntry {
    #[serde(rename = "type")]
    entry_type: String,
    name: String,
    download_url: Option<String>,
}

fn reqwest_client() -> Result<reqwest::Client, String> {
    let mut headers = HeaderMap::new();
    headers.insert(USER_AGENT, HeaderValue::from_static("CCHub Skill Updater"));
    headers.insert(
        ACCEPT,
        HeaderValue::from_static("application/vnd.github+json"),
    );
    reqwest::Client::builder()
        .default_headers(headers)
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|e| e.to_string())
}

pub fn sha256_hex(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}

pub fn file_sha256(path: &Path) -> Result<String, String> {
    let content = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    Ok(sha256_hex(&content))
}

pub fn load_skill_metadata_map(conn: &Connection) -> Result<HashMap<String, SkillSourceMetadata>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT COALESCE(file_path, id), source_url, baseline_sha256, latest_sha256, last_checked_at
             FROM skills",
        )
        .map_err(|e| e.to_string())?;

    let mut rows = stmt.query([]).map_err(|e| e.to_string())?;
    let mut metadata = HashMap::new();
    while let Some(row) = rows.next().map_err(|e| e.to_string())? {
        let key: String = row.get(0).map_err(|e| e.to_string())?;
        metadata.insert(
            key,
            SkillSourceMetadata {
                source_url: row.get(1).ok(),
                baseline_sha256: row.get(2).ok(),
                latest_sha256: row.get(3).ok(),
                last_checked_at: row.get(4).ok(),
            },
        );
    }
    Ok(metadata)
}

pub fn persist_skill_metadata(conn: &Connection, skill: &Skill) -> Result<(), String> {
    conn.execute(
        "INSERT INTO skills (
            id, name, description, plugin_id, trigger_command, file_path, version, installed_at,
            source_url, baseline_sha256, latest_sha256, last_checked_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
        ON CONFLICT(id) DO UPDATE SET
            name = excluded.name,
            description = excluded.description,
            plugin_id = excluded.plugin_id,
            trigger_command = excluded.trigger_command,
            file_path = excluded.file_path,
            version = excluded.version,
            source_url = COALESCE(excluded.source_url, skills.source_url),
            baseline_sha256 = COALESCE(excluded.baseline_sha256, skills.baseline_sha256),
            latest_sha256 = COALESCE(excluded.latest_sha256, skills.latest_sha256),
            last_checked_at = COALESCE(excluded.last_checked_at, skills.last_checked_at)",
        rusqlite::params![
            skill.id,
            skill.name,
            skill.description,
            skill.plugin_id,
            skill.trigger_command,
            skill.file_path,
            skill.version,
            skill.installed_at,
            skill.source_url,
            skill.baseline_sha256,
            skill.latest_sha256,
            skill.last_checked_at,
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn persist_marketplace_skill_install(
    conn: &Connection,
    file_path: &str,
    name: &str,
    description: Option<&str>,
    trigger_command: Option<&str>,
    source_url: Option<&str>,
    content: &str,
) -> Result<(), String> {
    let hash = sha256_hex(content);
    persist_skill_metadata(
        conn,
        &Skill {
            id: file_path.to_string(),
            name: name.to_string(),
            description: description.map(str::to_string),
            tool_id: Some("claude".to_string()),
            plugin_id: None,
            trigger_command: trigger_command.map(str::to_string),
            file_path: Some(file_path.to_string()),
            version: None,
            installed_at: Some(chrono::Utc::now().to_rfc3339()),
            source_url: source_url.map(str::to_string),
            baseline_sha256: Some(hash),
            latest_sha256: None,
            last_checked_at: None,
            current_sha256: None,
        },
    )
}

pub fn rename_skill_metadata(conn: &Connection, old_path: &str, new_path: &str) -> Result<(), String> {
    conn.execute(
        "UPDATE skills SET id = ?1, file_path = ?1 WHERE id = ?2 OR file_path = ?2",
        rusqlite::params![new_path, old_path],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn remove_skill_metadata(conn: &Connection, file_path: &str) -> Result<(), String> {
    conn.execute(
        "DELETE FROM skills WHERE id = ?1 OR file_path = ?1",
        rusqlite::params![file_path],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn sync_skill_baseline_to_current(conn: &Connection, file_path: &str) -> Result<(), String> {
    let path = Path::new(file_path);
    if !path.exists() {
        return Ok(());
    }
    let current = file_sha256(path)?;
    conn.execute(
        "UPDATE skills SET baseline_sha256 = ?1 WHERE id = ?2 OR file_path = ?2",
        rusqlite::params![current, file_path],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn parse_github_tree_url(url: &str) -> Option<(String, String, String, String)> {
    let trimmed = url.trim().trim_end_matches('/');
    let marker = "https://github.com/";
    if !trimmed.starts_with(marker) {
        return None;
    }
    let rest = &trimmed[marker.len()..];
    let parts: Vec<&str> = rest.split('/').collect();
    if parts.len() < 4 || parts[2] != "tree" {
        return None;
    }
    Some((
        parts[0].to_string(),
        parts[1].to_string(),
        parts[3].to_string(),
        if parts.len() > 4 {
            parts[4..].join("/")
        } else {
            String::new()
        },
    ))
}

async fn fetch_text(client: &reqwest::Client, url: &str) -> Result<String, String> {
    client
        .get(url)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .text()
        .await
        .map_err(|e| e.to_string())
}

async fn fetch_github_tree_content(
    client: &reqwest::Client,
    owner: &str,
    repo: &str,
    branch: &str,
    path: &str,
) -> Result<String, String> {
    let api_url = if path.is_empty() {
        format!("https://api.github.com/repos/{owner}/{repo}/contents?ref={branch}")
    } else {
        format!("https://api.github.com/repos/{owner}/{repo}/contents/{path}?ref={branch}")
    };
    let payload = client
        .get(&api_url)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .json::<GitHubContentResponse>()
        .await
        .map_err(|e| e.to_string())?;

    match payload {
        GitHubContentResponse::File(entry) => {
            let download_url = entry
                .download_url
                .ok_or_else(|| "GitHub file does not expose a download URL".to_string())?;
            fetch_text(client, &download_url).await
        }
        GitHubContentResponse::Directory(entries) => {
            let mut markdown_entries = entries
                .into_iter()
                .filter(|entry| {
                    entry.entry_type == "file"
                        && entry.name.to_ascii_lowercase().ends_with(".md")
                        && entry.download_url.is_some()
                })
                .collect::<Vec<_>>();

            markdown_entries.sort_by(|a, b| {
                let a_key = if a.name.eq_ignore_ascii_case("SKILL.md") {
                    format!("0-{}", a.name.to_ascii_lowercase())
                } else {
                    format!("1-{}", a.name.to_ascii_lowercase())
                };
                let b_key = if b.name.eq_ignore_ascii_case("SKILL.md") {
                    format!("0-{}", b.name.to_ascii_lowercase())
                } else {
                    format!("1-{}", b.name.to_ascii_lowercase())
                };
                a_key.cmp(&b_key)
            });

            if markdown_entries.is_empty() {
                return Err("No markdown files were found in the GitHub skill directory".to_string());
            }

            let mut merged = String::new();
            for (index, entry) in markdown_entries.iter().enumerate() {
                let download_url = entry
                    .download_url
                    .as_deref()
                    .ok_or_else(|| "GitHub entry is missing a download URL".to_string())?;
                let content = fetch_text(client, download_url).await?;
                if index > 0 {
                    merged.push_str("\n\n---\n\n");
                }
                merged.push_str(&content);
            }
            Ok(merged)
        }
    }
}

pub async fn fetch_remote_skill_content(source_url: &str) -> Result<String, String> {
    let client = reqwest_client()?;
    let trimmed = source_url.trim();
    if let Some((owner, repo, branch, path)) = parse_github_tree_url(trimmed) {
        return fetch_github_tree_content(&client, &owner, &repo, &branch, &path).await;
    }
    fetch_text(&client, trimmed).await
}
