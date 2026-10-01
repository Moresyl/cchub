use super::*;
use sha2::{Digest, Sha256};
use std::io::Read;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveSnapshot {
    pub content: Option<String>,
    pub revision: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibrarySnapshot {
    pub prompts: BTreeMap<String, PromptRecord>,
    pub library_revision: String,
    pub live: Option<LiveSnapshot>,
    pub live_error: Option<String>,
}

pub(super) fn library_revision(
    app: &str,
    prompts: &BTreeMap<String, PromptRecord>,
) -> Result<String, String> {
    let bytes =
        serde_json::to_vec(&(app, prompts)).map_err(|_| "Cannot inspect prompt versions")?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

pub(super) fn check_library(
    conn: &Connection,
    app: &str,
    expected: Option<&str>,
) -> Result<(), String> {
    if let Some(expected) = expected {
        if library_revision(app, &load_prompts(conn, app)?)? != expected {
            return Err(
                "Prompt library changed; reload and review before saving. Your draft is retained"
                    .into(),
            );
        }
    }
    Ok(())
}

pub(super) fn read_live(path: &Path) -> Result<LiveSnapshot, String> {
    let bytes = match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_file() => {
            if metadata.len() > MAX_PROMPT_BYTES as u64 {
                return Err("Prompt file exceeds the 1 MiB limit".into());
            }
            let file = std::fs::File::open(path).map_err(|_| "Cannot read prompt file")?;
            let mut bytes = Vec::new();
            file.take(MAX_PROMPT_BYTES as u64 + 1)
                .read_to_end(&mut bytes)
                .map_err(|_| "Cannot read prompt file")?;
            if bytes.len() > MAX_PROMPT_BYTES {
                return Err("Prompt file exceeds the 1 MiB limit".into());
            }
            Some(bytes)
        }
        Ok(_) => return Err("Prompt target must be a regular file".into()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(_) => return Err("Cannot inspect prompt file".into()),
    };
    let revision = match &bytes {
        Some(bytes) => format!("sha256:{:x}", Sha256::digest(bytes)),
        None => "missing".into(),
    };
    let content = bytes
        .map(String::from_utf8)
        .transpose()
        .map_err(|_| "Prompt file must contain valid UTF-8")?;
    Ok(LiveSnapshot { content, revision })
}

pub(super) fn check_live(live: &LiveSnapshot, expected: Option<&str>) -> Result<(), String> {
    if expected.is_some_and(|expected| expected != live.revision) {
        return Err("Prompt file changed externally; reload and review before writing. Your draft is retained".into());
    }
    Ok(())
}

pub(super) fn snapshot_at(
    conn: &Connection,
    app: &str,
    path: &Path,
) -> Result<LibrarySnapshot, String> {
    let prompts = load_prompts(conn, app)?;
    let library_revision = library_revision(app, &prompts)?;
    let (live, live_error) = match read_live(path) {
        Ok(live) => (Some(live), None),
        Err(error) => (None, Some(error)),
    };
    Ok(LibrarySnapshot {
        prompts,
        library_revision,
        live,
        live_error,
    })
}
