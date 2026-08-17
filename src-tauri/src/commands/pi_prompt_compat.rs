//! Pi instruction files and prompt templates.
//!
//! Pi keeps these files outside the application database.  The commands use
//! content revisions so an editor change cannot be silently overwritten.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

const MAX_FILE_BYTES: u64 = 1024 * 1024;
const MISSING_REVISION: &str = "missing";

fn file_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn pi_agent_dir() -> Result<PathBuf, String> {
    dirs::home_dir()
        .map(|home| home.join(".pi").join("agent"))
        .ok_or_else(|| "Cannot determine the home directory".to_string())
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PiPromptFileKind {
    SystemOverride,
    SystemAppend,
}

impl PiPromptFileKind {
    fn filename(self) -> &'static str {
        match self {
            Self::SystemOverride => "SYSTEM.md",
            Self::SystemAppend => "APPEND_SYSTEM.md",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PiPromptFileSnapshot {
    pub exists: bool,
    pub revision: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PiPromptTemplate {
    pub slug: String,
    pub content: String,
    pub revision: String,
}

fn content_revision(content: &[u8]) -> String {
    format!("{:x}", Sha256::digest(content))
}

fn read_limited(path: &Path) -> Result<Vec<u8>, String> {
    let metadata = fs::metadata(path).map_err(|error| format!("{}: {error}", path.display()))?;
    if metadata.len() > MAX_FILE_BYTES {
        return Err(format!("File exceeds the 1 MiB limit: {}", path.display()));
    }
    let bytes = fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return Err(format!("File exceeds the 1 MiB limit: {}", path.display()));
    }
    Ok(bytes)
}

fn read_optional(path: &Path) -> Result<(bool, String, String), String> {
    match read_limited(path) {
        Ok(bytes) => {
            let revision = content_revision(&bytes);
            let content = String::from_utf8(bytes)
                .map_err(|error| format!("Prompt file must be UTF-8: {error}"))?;
            Ok((true, content, revision))
        }
        Err(_error) if !path.exists() => Ok((false, String::new(), MISSING_REVISION.to_string())),
        Err(error) => Err(error),
    }
}

fn verify_revision(path: &Path, expected: &str) -> Result<(), String> {
    let actual = match read_limited(path) {
        Ok(bytes) => content_revision(&bytes),
        Err(_error) if !path.exists() => MISSING_REVISION.to_string(),
        Err(error) => return Err(error),
    };
    if actual == expected || (actual == MISSING_REVISION && expected.trim().is_empty()) {
        Ok(())
    } else {
        Err(format!(
            "Prompt file changed outside CCHub: {}",
            path.display()
        ))
    }
}

fn validate_content(content: &str, allow_blank: bool) -> Result<(), String> {
    if !allow_blank && content.trim().is_empty() {
        return Err("Pi instruction content cannot be blank".to_string());
    }
    if content.len() as u64 > MAX_FILE_BYTES {
        return Err("Prompt content exceeds the 1 MiB limit".to_string());
    }
    Ok(())
}

fn validate_slug(slug: &str) -> Result<(), String> {
    let reserved = [
        "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
        "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
    ];
    let basename = slug
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    if slug.is_empty()
        || slug.len() > 128
        || slug.starts_with('.')
        || slug.ends_with('.')
        || reserved.iter().any(|item| *item == basename)
        || slug.chars().any(|ch| {
            ch.is_control()
                || ch.is_whitespace()
                || matches!(ch, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*')
        })
    {
        return Err("Prompt template name must be one portable token".to_string());
    }
    Ok(())
}

fn write_atomic(path: &Path, content: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("{}: {error}", parent.display()))?;
    }
    crate::utils::atomic_write_string(path, content)
        .map_err(|error| format!("{}: {error}", path.display()))
}

#[tauri::command(rename_all = "camelCase")]
pub fn get_pi_prompt_file(kind: PiPromptFileKind) -> Result<PiPromptFileSnapshot, String> {
    let _guard = file_lock().lock().map_err(|error| error.to_string())?;
    let path = pi_agent_dir()?.join(kind.filename());
    let (exists, content, revision) = read_optional(&path)?;
    Ok(PiPromptFileSnapshot {
        exists,
        revision,
        content,
    })
}

#[tauri::command(rename_all = "camelCase")]
pub fn replace_pi_prompt_file(
    kind: PiPromptFileKind,
    expected_revision: String,
    content: String,
) -> Result<PiPromptFileSnapshot, String> {
    validate_content(&content, false)?;
    let _guard = file_lock().lock().map_err(|error| error.to_string())?;
    let path = pi_agent_dir()?.join(kind.filename());
    verify_revision(&path, &expected_revision)?;
    write_atomic(&path, &content)?;
    let (exists, content, revision) = read_optional(&path)?;
    Ok(PiPromptFileSnapshot {
        exists,
        revision,
        content,
    })
}

#[tauri::command(rename_all = "camelCase")]
pub fn delete_pi_prompt_file(
    kind: PiPromptFileKind,
    expected_revision: String,
) -> Result<bool, String> {
    let _guard = file_lock().lock().map_err(|error| error.to_string())?;
    let path = pi_agent_dir()?.join(kind.filename());
    verify_revision(&path, &expected_revision)?;
    match fs::remove_file(&path) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(format!("{}: {error}", path.display())),
    }
}

#[tauri::command]
pub fn list_pi_prompt_templates() -> Result<Vec<PiPromptTemplate>, String> {
    let _guard = file_lock().lock().map_err(|error| error.to_string())?;
    let dir = pi_agent_dir()?.join("prompts");
    let entries = match fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(format!("{}: {error}", dir.display())),
    };
    let mut result = Vec::new();
    for entry in entries {
        let path = entry.map_err(|error| error.to_string())?.path();
        if path.extension().and_then(|value| value.to_str()) != Some("md") {
            continue;
        }
        let Some(slug) = path.file_stem().and_then(|value| value.to_str()) else {
            continue;
        };
        if validate_slug(slug).is_err() {
            continue;
        }
        let bytes = read_limited(&path)?;
        let content = String::from_utf8(bytes.clone())
            .map_err(|error| format!("Prompt template must be UTF-8: {error}"))?;
        result.push(PiPromptTemplate {
            slug: slug.to_string(),
            content,
            revision: content_revision(&bytes),
        });
    }
    result.sort_by(|left, right| left.slug.cmp(&right.slug));
    Ok(result)
}

#[tauri::command(rename_all = "camelCase")]
pub fn upsert_pi_prompt_template(
    slug: String,
    original_slug: Option<String>,
    expected_revision: String,
    content: String,
) -> Result<PiPromptTemplate, String> {
    validate_slug(&slug)?;
    if let Some(original) = original_slug.as_deref() {
        validate_slug(original)?;
    }
    validate_content(&content, true)?;
    let _guard = file_lock().lock().map_err(|error| error.to_string())?;
    let dir = pi_agent_dir()?.join("prompts");
    let target = dir.join(format!("{slug}.md"));
    if let Some(original) = original_slug
        .as_deref()
        .filter(|value| *value != slug.as_str())
    {
        let source = dir.join(format!("{original}.md"));
        verify_revision(&source, &expected_revision)?;
        verify_revision(&target, MISSING_REVISION)?;
        fs::create_dir_all(&dir).map_err(|error| format!("{}: {error}", dir.display()))?;
        fs::rename(&source, &target).map_err(|error| format!("{}: {error}", source.display()))?;
    } else {
        verify_revision(&target, &expected_revision)?;
    }
    if let Err(error) = write_atomic(&target, &content) {
        if let Some(original) = original_slug
            .as_deref()
            .filter(|value| *value != slug.as_str())
        {
            let source = dir.join(format!("{original}.md"));
            let _ = fs::rename(&target, &source);
        }
        return Err(error);
    }
    Ok(PiPromptTemplate {
        slug,
        revision: content_revision(content.as_bytes()),
        content,
    })
}

#[tauri::command(rename_all = "camelCase")]
pub fn delete_pi_prompt_template(slug: String, expected_revision: String) -> Result<bool, String> {
    validate_slug(&slug)?;
    let _guard = file_lock().lock().map_err(|error| error.to_string())?;
    let path = pi_agent_dir()?.join("prompts").join(format!("{slug}.md"));
    verify_revision(&path, &expected_revision)?;
    match fs::remove_file(&path) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(format!("{}: {error}", path.display())),
    }
}

#[cfg(test)]
mod tests {
    use super::{content_revision, validate_slug};

    #[test]
    fn template_names_are_portable() {
        assert!(validate_slug("review-pr").is_ok());
        assert!(validate_slug("release.v2").is_ok());
        assert!(validate_slug("a/b").is_err());
        assert!(validate_slug("CON").is_err());
        assert!(validate_slug("with space").is_err());
    }

    #[test]
    fn revisions_are_stable() {
        assert_eq!(content_revision(b"hello"), content_revision(b"hello"));
        assert_ne!(content_revision(b"hello"), content_revision(b"world"));
    }
}
