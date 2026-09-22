use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McodeState {
    config_path: String,
    installed: bool,
    providers: BTreeMap<String, Value>,
}

fn explicit_data_dir(minimax: Option<&str>, mavis: Option<&str>) -> Option<PathBuf> {
    [minimax, mavis]
        .into_iter()
        .flatten()
        .map(str::trim)
        .find(|value| !value.is_empty())
        .map(PathBuf::from)
}

pub(crate) fn config_path() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;
    let dir = explicit_data_dir(
        std::env::var("MINIMAX_DATA_DIR").ok().as_deref(),
        std::env::var("MAVIS_DATA_DIR").ok().as_deref(),
    )
    .unwrap_or_else(|| home.join(".minimax"));
    Ok(dir.join("config.yaml"))
}

fn read_document(path: &Path) -> Result<serde_yaml::Value, String> {
    if !path.exists() {
        return Ok(serde_yaml::Value::Mapping(Default::default()));
    }
    let content = fs::read_to_string(path)
        .map_err(|error| format!("Cannot read MiniMax Code config: {error}"))?;
    let document: serde_yaml::Value = serde_yaml::from_str(&content)
        .map_err(|_| "Invalid MiniMax Code YAML configuration".to_string())?;
    if document.is_null() {
        return Ok(serde_yaml::Value::Mapping(Default::default()));
    }
    if !document.is_mapping() {
        return Err("MiniMax Code configuration must be a mapping".to_string());
    }
    Ok(document)
}

fn providers(document: &serde_yaml::Value) -> Result<BTreeMap<String, Value>, String> {
    match document.get("custom_provider") {
        None | Some(serde_yaml::Value::Null) => Ok(BTreeMap::new()),
        Some(value) => {
            let entries: BTreeMap<String, Value> = serde_yaml::from_value(value.clone())
                .map_err(|_| "Invalid MiniMax Code custom_provider mapping".to_string())?;
            Ok(entries
                .into_iter()
                .filter(|(_, provider)| {
                    provider
                        .get("kind")
                        .and_then(Value::as_str)
                        .is_none_or(|kind| kind == "custom")
                })
                .collect())
        }
    }
}

fn validate_provider(id: &str, provider: &Value) -> Result<(), String> {
    if id.is_empty()
        || id
            .chars()
            .any(|c| !c.is_ascii_alphanumeric() && !matches!(c, '-' | '_'))
    {
        return Err("Provider ID must contain letters, digits, '-' or '_'".to_string());
    }
    if !provider.is_object()
        || provider
            .get("kind")
            .and_then(Value::as_str)
            .is_some_and(|kind| kind != "custom")
    {
        return Err("MiniMax Code provider must have kind 'custom'".to_string());
    }
    if !matches!(
        provider
            .get("api")
            .and_then(Value::as_str)
            .unwrap_or("anthropic-messages"),
        "anthropic-messages" | "openai-completions" | "openai-responses"
    ) {
        return Err("Unsupported MiniMax Code API format".to_string());
    }
    let url = provider
        .pointer("/options/baseURL")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if !url::Url::parse(url)
        .is_ok_and(|url| matches!(url.scheme(), "http" | "https") && url.host_str().is_some())
    {
        return Err("Enter a valid MiniMax Code endpoint URL".to_string());
    }
    if provider
        .pointer("/options/apiKey")
        .and_then(Value::as_str)
        .is_none_or(|key| key.trim().is_empty())
    {
        return Err("Enter a MiniMax Code API key".to_string());
    }
    if provider
        .get("models")
        .and_then(Value::as_object)
        .is_none_or(|models| models.is_empty() || models.keys().any(|id| id.trim().is_empty()))
    {
        return Err("Add at least one MiniMax Code model".to_string());
    }
    Ok(())
}

fn protect_selected_models(
    document: &serde_yaml::Value,
    id: &str,
    next: Option<&Value>,
) -> Result<(), String> {
    let prefix = format!("custom_provider:{id}/");
    for key in ["defaultModel", "defaultLightModel"] {
        let Some(model) = document
            .get(key)
            .and_then(serde_yaml::Value::as_str)
            .and_then(|value| value.strip_prefix(&prefix))
        else {
            continue;
        };
        let remains = next.is_some_and(|provider| {
            provider.get("enabled") != Some(&Value::Bool(false))
                && provider
                    .get("models")
                    .and_then(|models| models.get(model))
                    .is_some_and(|value| {
                        value.is_object() && value.get("enabled") != Some(&Value::Bool(false))
                    })
        });
        if !remains {
            return Err("Select another default model in MiniMax Code before removing this provider or model".to_string());
        }
    }
    Ok(())
}

struct ConfigLock(PathBuf);
impl Drop for ConfigLock {
    fn drop(&mut self) {
        let _ = fs::remove_dir(&self.0);
    }
}

#[cfg(windows)]
fn replace_existing(path: &Path, replacement: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    #[link(name = "Kernel32")]
    extern "system" {
        fn ReplaceFileW(
            replaced: *const u16,
            replacement: *const u16,
            backup: *const u16,
            flags: u32,
            exclude: *const std::ffi::c_void,
            reserved: *const std::ffi::c_void,
        ) -> i32;
    }
    let original = path
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let next = replacement
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let result = unsafe {
        ReplaceFileW(
            original.as_ptr(),
            next.as_ptr(),
            std::ptr::null(),
            0,
            std::ptr::null(),
            std::ptr::null(),
        )
    };
    if result == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

pub(crate) fn write_config(path: &Path, content: &str) -> std::io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| std::io::Error::other("Missing config directory"))?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    temporary.write_all(content.as_bytes())?;
    temporary.as_file().sync_all()?;
    if path.exists() {
        let temporary_path = temporary.into_temp_path();
        #[cfg(windows)]
        replace_existing(path, temporary_path.as_ref())?;
        #[cfg(not(windows))]
        fs::rename(temporary_path.as_ref(), path)?;
    } else {
        temporary.persist(path).map_err(|error| error.error)?;
    }
    Ok(())
}

fn update_at(path: &Path, id: &str, provider: Option<Value>) -> Result<(), String> {
    if let Some(ref provider) = provider {
        validate_provider(id, provider)?;
    } else if id.is_empty()
        || id
            .chars()
            .any(|c| !c.is_ascii_alphanumeric() && !matches!(c, '-' | '_'))
    {
        return Err("Invalid provider ID".to_string());
    }
    if provider.is_none() && !path.exists() {
        return Ok(());
    }
    let parent = path.parent().ok_or("Invalid MiniMax Code config path")?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("Cannot create MiniMax Code directory: {error}"))?;
    let lock_path = PathBuf::from(format!("{}.lock", path.display()));
    fs::create_dir(&lock_path).map_err(|_| {
        "MiniMax Code configuration is busy; retry after it finishes saving".to_string()
    })?;
    let _lock = ConfigLock(lock_path);
    let mut document = read_document(path)?;
    protect_selected_models(&document, id, provider.as_ref())?;
    let root = document
        .as_mapping_mut()
        .ok_or("Invalid MiniMax Code config mapping")?;
    let entry = root
        .entry("custom_provider".into())
        .or_insert_with(|| serde_yaml::Value::Mapping(Default::default()));
    if entry.is_null() {
        *entry = serde_yaml::Value::Mapping(Default::default());
    }
    let mapping = entry
        .as_mapping_mut()
        .ok_or("Invalid MiniMax Code custom_provider mapping")?;
    let key = serde_yaml::Value::from(id);
    if mapping
        .get(&key)
        .and_then(|value| value.get("kind"))
        .and_then(serde_yaml::Value::as_str)
        .is_some_and(|kind| kind != "custom")
    {
        return Err("MiniMax Code owns this account provider".to_string());
    }
    match provider {
        Some(provider) => {
            mapping.insert(
                key,
                serde_yaml::to_value(provider)
                    .map_err(|_| "Invalid MiniMax Code provider".to_string())?,
            );
        }
        None => {
            mapping.remove(&key);
        }
    }
    let content = serde_yaml::to_string(&document)
        .map_err(|_| "Cannot serialize MiniMax Code config".to_string())?;
    write_config(path, &content)
        .map_err(|error| format!("Cannot save MiniMax Code config: {error}"))
}

#[tauri::command]
pub fn get_mcode_state() -> Result<McodeState, String> {
    let path = config_path()?;
    let installed = path.exists();
    Ok(McodeState {
        config_path: path.to_string_lossy().into_owned(),
        installed,
        providers: providers(&read_document(&path)?)?,
    })
}

#[tauri::command]
pub fn save_mcode_provider(id: String, provider: Value) -> Result<(), String> {
    update_at(&config_path()?, &id, Some(provider))
}

#[tauri::command]
pub fn delete_mcode_provider(id: String) -> Result<(), String> {
    update_at(&config_path()?, &id, None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample() -> Value {
        json!({"kind":"custom","enabled":true,"api":"anthropic-messages","options":{"baseURL":"https://example.com/v1","apiKey":"secret"},"models":{"example-model":{"name":"Example"}}})
    }

    #[test]
    fn config_directory_override_precedence() {
        assert_eq!(
            explicit_data_dir(Some(" /primary "), Some("/fallback")),
            Some("/primary".into())
        );
        assert_eq!(
            explicit_data_dir(Some(" "), Some(" /fallback ")),
            Some("/fallback".into())
        );
    }

    #[test]
    fn validation_rejects_unsafe_or_incomplete_providers() {
        assert!(validate_provider("unsafe/key", &sample()).is_err());
        let mut invalid = sample();
        invalid["options"]["baseURL"] = json!("file:///tmp/secret");
        assert!(validate_provider("safe", &invalid).is_err());
        invalid["options"]["baseURL"] = json!("https://example.com");
        invalid["models"] = json!({});
        assert!(validate_provider("safe", &invalid).is_err());
    }

    #[test]
    fn updates_one_provider_without_losing_other_fields() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.yaml");
        fs::write(&path, "defaultModel: custom_provider:other/model\ncustom_provider:\n  other:\n    kind: custom\n    models:\n      model: {}\nextra: preserved\n").unwrap();
        update_at(&path, "new", Some(sample())).unwrap();
        let saved = read_document(&path).unwrap();
        assert_eq!(saved["extra"].as_str(), Some("preserved"));
        assert_eq!(
            saved["custom_provider"]["other"]["models"]["model"].is_mapping(),
            true
        );
        assert!(saved["custom_provider"]["new"].is_mapping());
        assert!(!path.with_extension("yaml.lock").exists());
    }

    #[test]
    fn rejects_removing_selected_model_and_account_provider() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.yaml");
        fs::write(&path, "defaultModel: custom_provider:active/example-model\ncustom_provider:\n  active:\n    kind: custom\n    models:\n      example-model: {}\n  account:\n    kind: account\n").unwrap();
        assert!(update_at(&path, "active", None).is_err());
        assert!(update_at(&path, "active", Some(json!({"kind":"custom","options":{"baseURL":"https://example.com","apiKey":"secret"},"models":{"other":{}}}))).is_err());
        assert!(update_at(&path, "account", Some(sample())).is_err());
        assert!(update_at(&path, "account", None).is_err());
        assert!(!path.with_extension("yaml.lock").exists());
    }

    #[test]
    fn busy_lock_does_not_mutate_configuration() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.yaml");
        let lock = path.with_extension("yaml.lock");
        fs::create_dir(&lock).unwrap();
        assert!(update_at(&path, "new", Some(sample())).is_err());
        assert!(!path.exists());
    }
}
