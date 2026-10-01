use super::super::types::*;
use super::{normalized_non_empty, read_codex_structured_config_from_content};
use crate::config_write::{self, FileUpdate};
use sha2::{Digest, Sha256};
use std::path::Path;

fn text(bytes: Option<&[u8]>, default: &str) -> Result<String, String> {
    bytes
        .map(|bytes| {
            String::from_utf8(bytes.to_vec())
                .map_err(|_| "Configuration must contain valid UTF-8".into())
        })
        .unwrap_or_else(|| Ok(default.to_string()))
}

fn revision(config: Option<&[u8]>, auth: Option<&[u8]>) -> String {
    let mut hash = Sha256::new();
    hash.update(b"cchub-codex-files-v1");
    for bytes in [config, auth] {
        hash.update([u8::from(bytes.is_some())]);
        if let Some(bytes) = bytes {
            hash.update((bytes.len() as u64).to_le_bytes());
            hash.update(bytes);
        }
    }
    format!("{:x}", hash.finalize())
}

pub(crate) fn read_codex_structured_files(
    config_path: &Path,
    auth_path: &Path,
) -> Result<CodexTomlStructuredRead, String> {
    let _guard = crate::json_config::write_lock()?;
    let config = config_write::read(config_path)?;
    let auth = config_write::read(auth_path)?;
    let content = text(config.as_deref(), "")?;
    let value = crate::json_config::parse_json_object(&text(auth.as_deref(), "{}\n")?)?;
    let api_key = value
        .get("OPENAI_API_KEY")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_string();
    Ok(CodexTomlStructuredRead {
        config: read_codex_structured_config_from_content(&content, api_key),
        content,
        file_revision: revision(config.as_deref(), auth.as_deref()),
    })
}

fn validate_toml(raw: &str) -> Result<(), String> {
    let document = raw
        .parse::<toml_edit::DocumentMut>()
        .map_err(|_| "Invalid TOML; check syntax before saving")?;
    for key in ["model_providers", "mcp_servers"] {
        if document.get(key).is_some_and(|item| !item.is_table_like()) {
            return Err("Provider and MCP configuration must use TOML tables; repair the raw configuration before saving".into());
        }
    }
    let provider = document
        .get("model_provider")
        .and_then(toml_edit::Item::as_str)
        .and_then(normalized_non_empty)
        .unwrap_or_else(|| "custom".into());
    if document
        .get("model_providers")
        .and_then(|item| item.get(&provider))
        .is_some_and(|item| !item.is_table_like())
    {
        return Err("Selected model provider must be a TOML table".into());
    }
    Ok(())
}

pub(crate) fn write_codex_structured_files(
    config_path: &Path,
    auth_path: &Path,
    raw: &str,
    api_key: &str,
    expected_revision: &str,
) -> Result<CodexTomlStructuredWrite, String> {
    let _guard = crate::json_config::write_lock()?;
    let original_config = config_write::read(config_path)?;
    let original_auth = config_write::read(auth_path)?;
    if revision(original_config.as_deref(), original_auth.as_deref()) != expected_revision {
        return Err("Configuration or authentication changed externally; reload before saving. Your draft has been retained".into());
    }
    validate_toml(raw)?;
    let auth =
        crate::json_config::edit_json_text(&text(original_auth.as_deref(), "{}\n")?, |value| {
            let object = value
                .as_object_mut()
                .ok_or("Authentication configuration must be an object")?;
            if let Some(key) = normalized_non_empty(api_key) {
                object.insert("OPENAI_API_KEY".into(), key.into());
            } else {
                object.remove("OPENAI_API_KEY");
            }
            Ok(())
        })?;
    let content = raw.to_string();
    let file_revision = revision(Some(content.as_bytes()), Some(auth.as_bytes()));
    config_write::commit(vec![
        FileUpdate {
            path: config_path.into(),
            original: original_config,
            desired: content.as_bytes().to_vec(),
        },
        FileUpdate {
            path: auth_path.into(),
            original: original_auth,
            desired: auth.into_bytes(),
        },
    ])?;
    Ok(CodexTomlStructuredWrite {
        content,
        file_revision,
    })
}

#[cfg(test)]
mod tests;
