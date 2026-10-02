use super::storage;
use serde::Serialize;
use std::path::Path;
use toml_edit::{DocumentMut, Item};

#[derive(Debug, Serialize)]
pub struct CodexSettings {
    pub approval_mode: String,
    pub approval_policy: String,
    pub sandbox_mode: String,
    pub permission_profile: String,
    pub reasoning_effort: String,
    pub disable_response_storage: bool,
    pub context_window_1m: bool,
    pub context_window: Option<i64>,
    pub legacy_personality: bool,
    pub profile_selected: bool,
    pub config_revision: String,
}

fn parse(source: &str) -> Result<DocumentMut, String> {
    source
        .parse()
        .map_err(|_| "Codex configuration is invalid TOML; repair it before saving".into())
}

fn string(doc: &DocumentMut, key: &str) -> Result<String, String> {
    match doc.get(key) {
        None => Ok(String::new()),
        Some(item) => item
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| "Codex settings contain an invalid field type".into()),
    }
}

fn settings(path: &Path, bytes: Option<&[u8]>) -> Result<CodexSettings, String> {
    let doc = parse(storage::text(bytes, "")?)?;
    let approval_policy = match doc.get("approval_policy") {
        None => String::new(),
        Some(item) if item.is_table_like() => "granular".into(),
        Some(item) => item
            .as_str()
            .map(str::to_owned)
            .ok_or("Codex approval policy has an invalid field type")?,
    };
    let sandbox_mode = string(&doc, "sandbox_mode")?;
    let permission_profile = string(&doc, "default_permissions")?;
    let effective_sandbox = match permission_profile.as_str() {
        "" => sandbox_mode.as_str(),
        ":read-only" => "read-only",
        ":workspace" => "workspace-write",
        ":danger-full-access" => "danger-full-access",
        _ => "custom",
    };
    let profile_selected = doc.contains_key("profile");
    let approval_mode = if profile_selected {
        "custom"
    } else if approval_policy.is_empty() && effective_sandbox.is_empty() {
        "default"
    } else if !permission_profile.is_empty()
        && (!sandbox_mode.is_empty() || doc.contains_key("sandbox_workspace_write"))
    {
        "custom"
    } else {
        match (approval_policy.as_str(), effective_sandbox) {
            ("on-request", "read-only") => "read-only",
            ("on-request", "workspace-write") => "workspace-write",
            ("never", "danger-full-access") => "danger-full-access",
            _ => "custom",
        }
    };
    let disable_response_storage = match doc.get("disable_response_storage") {
        None => false,
        Some(item) => item
            .as_bool()
            .ok_or("Codex response storage setting must be a boolean")?,
    };
    let context_window = match doc.get("model_context_window") {
        None => None,
        Some(item) => Some(
            item.as_integer()
                .filter(|value| *value > 0)
                .ok_or("Codex context window must be a positive integer")?,
        ),
    };
    Ok(CodexSettings {
        approval_mode: approval_mode.into(),
        approval_policy,
        sandbox_mode,
        permission_profile,
        reasoning_effort: string(&doc, "model_reasoning_effort")?,
        disable_response_storage,
        context_window_1m: context_window == Some(1_000_000),
        context_window,
        legacy_personality: matches!(
            string(&doc, "personality")?.as_str(),
            "suggest" | "auto-edit" | "full-auto"
        ),
        profile_selected,
        config_revision: storage::revision(path, bytes),
    })
}

pub(super) fn read(path: &Path) -> Result<CodexSettings, String> {
    let _guard = crate::json_config::write_lock()?;
    settings(path, storage::read(path)?.as_deref())
}

fn set_scalar(doc: &mut DocumentMut, key: &str, value: Item) {
    crate::commands::extra::config_profiles::set_scalar(&mut doc[key], value);
}

fn boolean(value: &str) -> Result<bool, String> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err("Setting value must be true or false".into()),
    }
}

pub(super) fn write(
    path: &Path,
    key: &str,
    value: &str,
    expected_revision: Option<&str>,
) -> Result<CodexSettings, String> {
    let _guard = crate::json_config::write_lock()?;
    let original = storage::read(path)?;
    storage::check_revision(path, original.as_deref(), expected_revision)?;
    let source = storage::text(original.as_deref(), "")?;
    let mut doc = parse(source)?;
    // TOML serialization can normalize CRLF even when no field changed.
    let before = doc.to_string();
    match key {
        "approval_mode" => {
            let policy = match value {
                "read-only" | "workspace-write" => "on-request",
                "danger-full-access" => "never",
                _ => return Err("Unsupported Codex permission mode".into()),
            };
            if doc.contains_key("profile") {
                return Err("A Codex configuration profile is selected; edit that profile's permissions instead".into());
            }
            // An explicit preset selection replaces the active permission profile,
            // retaining profile definitions and other workspace preferences.
            doc.remove("default_permissions");
            set_scalar(&mut doc, "approval_policy", toml_edit::value(policy));
            set_scalar(&mut doc, "sandbox_mode", toml_edit::value(value));
            // Only remove invalid values generated by the old permissions control.
            if matches!(
                doc.get("personality").and_then(Item::as_str),
                Some("suggest" | "auto-edit" | "full-auto")
            ) {
                doc.remove("personality");
            }
        }
        "reasoning_effort" => {
            if !matches!(
                value,
                "" | "none" | "minimal" | "low" | "medium" | "high" | "xhigh" | "max" | "ultra"
            ) {
                return Err("Unsupported Codex reasoning effort".into());
            }
            crate::commands::extra::config_profiles::set_codex_reasoning_effort(&mut doc, value);
        }
        "disable_response_storage" => {
            set_scalar(
                &mut doc,
                "disable_response_storage",
                toml_edit::value(boolean(value)?),
            );
        }
        "context_window_1m" => {
            if boolean(value)? {
                set_scalar(
                    &mut doc,
                    "model_context_window",
                    toml_edit::value(1_000_000),
                );
            } else if doc.get("model_context_window").and_then(Item::as_integer) == Some(1_000_000)
            {
                doc.remove("model_context_window");
            }
        }
        _ => return Err("Unsupported Codex setting".into()),
    }
    let mut desired = doc.to_string();
    if desired == before {
        return settings(path, original.as_deref());
    }
    if source.contains("\r\n") && !source.replace("\r\n", "").contains('\n') {
        desired = desired.replace("\r\n", "\n").replace('\n', "\r\n");
    }
    // Validate the resulting snapshot before changing any file.
    let result = settings(path, Some(desired.as_bytes()))?;
    storage::save(path, original, desired)?;
    Ok(result)
}
