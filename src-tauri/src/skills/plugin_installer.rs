use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};
use std::time::Duration;
use tar::Archive;
use uuid::Uuid;
use zip::read::ZipArchive;

const MAX_ARCHIVE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_UNPACKED_BYTES: u64 = 256 * 1024 * 1024;
const MAX_ARCHIVE_FILES: usize = 4096;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledPlugin {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub version: Option<String>,
    pub source_url: String,
    pub path: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ArchiveKind {
    Zip,
    Tar,
    GzipTar,
}

struct SourceArchive {
    bytes: Vec<u8>,
    label: String,
}

pub async fn install_plugin(source: &str) -> Result<InstalledPlugin, String> {
    let source = source.trim();
    if source.is_empty() {
        return Err("Plugin source is required".to_string());
    }

    let archive = load_source(source).await?;
    let plugins_dir = crate::skills::scanner::get_plugins_dir()
        .ok_or_else(|| "Cannot find Claude plugins directory".to_string())?;
    fs::create_dir_all(&plugins_dir)
        .map_err(|e| format!("Failed to create plugins directory: {e}"))?;

    let staging = tempfile::Builder::new()
        .prefix(".cchub-plugin-")
        .tempdir_in(&plugins_dir)
        .map_err(|e| format!("Failed to create plugin staging directory: {e}"))?;
    let extracted_root = staging.path().join("root");
    fs::create_dir_all(&extracted_root)
        .map_err(|e| format!("Failed to create plugin extraction directory: {e}"))?;
    extract_archive(&archive.bytes, &archive.label, &extracted_root)?;

    let plugin_root = locate_plugin_root(&extracted_root)?;
    let metadata = read_plugin_metadata(&plugin_root);
    if !has_plugin_payload(&plugin_root) {
        return Err(
            "Plugin archive did not contain package metadata or Markdown skills".to_string(),
        );
    }

    let fallback = Path::new(&archive.label)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("plugin");
    let id = sanitize_plugin_id(metadata.0.as_deref().unwrap_or(fallback))?;
    let destination = plugins_dir.join(&id);
    let backup_path = backup_existing_plugin(&destination)?;

    if let Err(error) = fs::rename(&plugin_root, &destination) {
        if let Some(backup) = backup_path.as_ref() {
            let _ = fs::rename(backup, &destination);
        }
        return Err(format!("Failed to activate plugin {id}: {error}"));
    }

    Ok(InstalledPlugin {
        id,
        name: metadata.0.unwrap_or_else(|| fallback.to_string()),
        description: metadata.1,
        version: metadata.2,
        source_url: source.to_string(),
        path: destination.to_string_lossy().to_string(),
    })
}

async fn load_source(source: &str) -> Result<SourceArchive, String> {
    let local_path = Path::new(source);
    if local_path.is_file() {
        let metadata = fs::metadata(local_path)
            .map_err(|e| format!("Plugin source is not a readable file: {e}"))?;
        if metadata.len() > MAX_ARCHIVE_BYTES {
            return Err(format!(
                "Plugin archive exceeds {} MiB",
                MAX_ARCHIVE_BYTES / 1024 / 1024
            ));
        }
        return Ok(SourceArchive {
            bytes: fs::read(local_path)
                .map_err(|e| format!("Failed to read plugin archive: {e}"))?,
            label: local_path.to_string_lossy().to_string(),
        });
    }

    if let Ok(url) = url::Url::parse(source) {
        if !matches!(url.scheme(), "http" | "https") {
            return Err("Plugin URL must use http or https".to_string());
        }
        let client = crate::shared::http_client::build_http_client(
            None,
            Some("CCHub"),
            Duration::from_secs(60),
        )?;
        let response = client
            .get(url.clone())
            .send()
            .await
            .map_err(|e| format!("Failed to download plugin: {e}"))?
            .error_for_status()
            .map_err(|e| format!("Plugin download failed: {e}"))?;
        if response
            .content_length()
            .is_some_and(|size| size > MAX_ARCHIVE_BYTES)
        {
            return Err(format!(
                "Plugin archive exceeds {} MiB",
                MAX_ARCHIVE_BYTES / 1024 / 1024
            ));
        }

        let mut bytes = Vec::new();
        if let Some(size) = response.content_length() {
            bytes.reserve(size as usize);
        }
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| format!("Failed to read plugin archive: {e}"))?;
            if bytes.len() as u64 + chunk.len() as u64 > MAX_ARCHIVE_BYTES {
                return Err(format!(
                    "Plugin archive exceeds {} MiB",
                    MAX_ARCHIVE_BYTES / 1024 / 1024
                ));
            }
            bytes.extend_from_slice(&chunk);
        }
        return Ok(SourceArchive {
            bytes,
            label: url.path().to_string(),
        });
    }

    Err("Plugin source must be an HTTP(S) archive URL or an archive file".to_string())
}

fn archive_kind(bytes: &[u8], label: &str) -> Result<ArchiveKind, String> {
    if bytes.starts_with(b"PK\x03\x04") || bytes.starts_with(b"PK\x05\x06") {
        return Ok(ArchiveKind::Zip);
    }
    if bytes.starts_with(&[0x1f, 0x8b]) {
        return Ok(ArchiveKind::GzipTar);
    }
    let lower = label.to_ascii_lowercase();
    if lower.ends_with(".tar") {
        Ok(ArchiveKind::Tar)
    } else {
        Err("Plugin source must be a ZIP, TAR, or TAR.GZ archive".to_string())
    }
}

fn extract_archive(bytes: &[u8], label: &str, destination: &Path) -> Result<(), String> {
    match archive_kind(bytes, label)? {
        ArchiveKind::Zip => extract_zip(bytes, destination),
        ArchiveKind::Tar => extract_tar(bytes, destination),
        ArchiveKind::GzipTar => {
            let decoder = flate2::read::GzDecoder::new(bytes);
            extract_tar_reader(decoder, destination)
        }
    }
}

fn extract_zip(bytes: &[u8], destination: &Path) -> Result<(), String> {
    let cursor = io::Cursor::new(bytes);
    let mut archive = ZipArchive::new(cursor).map_err(|e| format!("Invalid ZIP archive: {e}"))?;
    let mut file_count = 0usize;
    let mut unpacked = 0u64;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|e| e.to_string())?;
        let relative = safe_archive_path(entry.name())?;
        let target = destination.join(&relative);
        if entry
            .unix_mode()
            .is_some_and(|mode| mode & 0o170000 == 0o120000)
        {
            return Err(format!(
                "Plugin archive contains a symlink: {}",
                entry.name()
            ));
        }
        if entry.is_dir() {
            fs::create_dir_all(&target).map_err(|e| e.to_string())?;
            continue;
        }
        file_count += 1;
        if file_count > MAX_ARCHIVE_FILES {
            return Err("Plugin archive contains too many files".to_string());
        }
        let declared = entry.size();
        if unpacked.saturating_add(declared) > MAX_UNPACKED_BYTES {
            return Err("Plugin archive expands beyond the allowed size".to_string());
        }
        unpacked += declared;
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut output = File::create(&target).map_err(|e| e.to_string())?;
        io::copy(&mut entry, &mut output).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn extract_tar(bytes: &[u8], destination: &Path) -> Result<(), String> {
    extract_tar_reader(io::Cursor::new(bytes), destination)
}

fn extract_tar_reader<R: Read>(reader: R, destination: &Path) -> Result<(), String> {
    let mut archive = Archive::new(reader);
    let entries = archive
        .entries()
        .map_err(|e| format!("Invalid TAR archive: {e}"))?;
    let mut file_count = 0usize;
    let mut unpacked = 0u64;
    for entry in entries {
        let mut entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path().map_err(|e| e.to_string())?.to_path_buf();
        let relative = safe_archive_path(&path.to_string_lossy())?;
        let target = destination.join(&relative);
        let entry_type = entry.header().entry_type();
        if entry_type.is_dir() {
            fs::create_dir_all(&target).map_err(|e| e.to_string())?;
            continue;
        }
        if !entry_type.is_file() {
            return Err(format!(
                "Plugin archive contains an unsupported entry: {}",
                path.display()
            ));
        }
        file_count += 1;
        if file_count > MAX_ARCHIVE_FILES {
            return Err("Plugin archive contains too many files".to_string());
        }
        let declared = entry.size();
        if unpacked.saturating_add(declared) > MAX_UNPACKED_BYTES {
            return Err("Plugin archive expands beyond the allowed size".to_string());
        }
        unpacked += declared;
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut output = File::create(&target).map_err(|e| e.to_string())?;
        io::copy(&mut entry, &mut output).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn safe_archive_path(raw: &str) -> Result<PathBuf, String> {
    if raw.is_empty() || raw.contains('\0') || raw.contains(':') || raw.contains('\\') {
        return Err(format!("Plugin archive contains an unsafe path: {raw}"));
    }
    let path = Path::new(raw);
    if path.is_absolute() {
        return Err(format!("Plugin archive contains an absolute path: {raw}"));
    }
    for component in path.components() {
        if matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        ) {
            return Err(format!("Plugin archive contains a traversal path: {raw}"));
        }
    }
    Ok(path.to_path_buf())
}

fn locate_plugin_root(extracted: &Path) -> Result<PathBuf, String> {
    if has_metadata_file(extracted) {
        return Ok(extracted.to_path_buf());
    }
    let mut directories = Vec::new();
    for entry in fs::read_dir(extracted).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        if entry.file_type().map_err(|e| e.to_string())?.is_dir() {
            directories.push(entry.path());
        }
    }
    if directories.len() == 1 && has_plugin_payload(&directories[0]) {
        return Ok(directories.remove(0));
    }
    Ok(extracted.to_path_buf())
}

fn has_metadata_file(path: &Path) -> bool {
    ["plugin.json", "package.json", ".mcp.json"]
        .iter()
        .any(|name| path.join(name).is_file())
}

fn has_plugin_payload(path: &Path) -> bool {
    if has_metadata_file(path) {
        return true;
    }
    let mut stack = vec![path.to_path_buf()];
    while let Some(current) = stack.pop() {
        let Ok(entries) = fs::read_dir(&current) else {
            continue;
        };
        for entry in entries.flatten() {
            let entry_path = entry.path();
            if entry_path.is_dir() {
                stack.push(entry_path);
            } else if entry_path.extension().and_then(|value| value.to_str()) == Some("md") {
                return true;
            }
        }
    }
    false
}

fn read_plugin_metadata(path: &Path) -> (Option<String>, Option<String>, Option<String>) {
    for file_name in ["plugin.json", "package.json"] {
        let metadata_path = path.join(file_name);
        let Ok(content) = fs::read_to_string(metadata_path) else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&content) else {
            continue;
        };
        let name = value
            .get("name")
            .or_else(|| value.get("id"))
            .and_then(|value| value.as_str())
            .map(ToString::to_string);
        let description = value
            .get("description")
            .and_then(|value| value.as_str())
            .map(ToString::to_string);
        let version = value
            .get("version")
            .and_then(|value| value.as_str())
            .map(ToString::to_string);
        return (name, description, version);
    }
    (None, None, None)
}

fn sanitize_plugin_id(raw: &str) -> Result<String, String> {
    let normalized = raw
        .trim()
        .replace(['/', '\\'], "-")
        .chars()
        .map(|value| {
            if value.is_ascii_alphanumeric() || matches!(value, '-' | '_' | '.') {
                value
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches(['.', '-', '_'])
        .to_string();
    if normalized.is_empty() || normalized == "." || normalized.len() > 80 {
        return Err("Plugin name is invalid or too long".to_string());
    }
    Ok(normalized)
}

pub fn validate_plugin_id(raw: &str) -> Result<String, String> {
    let value = raw.trim();
    if value.is_empty()
        || value.len() > 120
        || value == "."
        || value == ".."
        || value.contains('/')
        || value.contains('\\')
        || value.contains(':')
        || value.chars().any(|character| character.is_control())
    {
        return Err("Plugin id contains unsupported path characters".to_string());
    }
    Ok(value.to_string())
}

fn backup_existing_plugin(destination: &Path) -> Result<Option<PathBuf>, String> {
    if !destination.exists() {
        return Ok(None);
    }
    let home = dirs::home_dir().ok_or_else(|| "Cannot find home directory".to_string())?;
    let backup_dir = home.join(".cchub").join("plugin-backups");
    fs::create_dir_all(&backup_dir)
        .map_err(|e| format!("Failed to create plugin backup directory: {e}"))?;
    let backup = backup_dir.join(format!(
        "{}-{}",
        destination
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("plugin"),
        Uuid::new_v4()
    ));
    fs::rename(destination, &backup)
        .map_err(|e| format!("Failed to back up existing plugin: {e}"))?;
    Ok(Some(backup))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_archive_traversal_and_windows_paths() {
        assert!(safe_archive_path("../plugin.json").is_err());
        assert!(safe_archive_path("C:/plugin.json").is_err());
        assert!(safe_archive_path("folder\\plugin.json").is_err());
        assert!(safe_archive_path("plugin/plugin.json").is_ok());
    }

    #[test]
    fn sanitizes_plugin_names_without_path_escape() {
        assert_eq!(sanitize_plugin_id("scope/plugin").unwrap(), "scope-plugin");
        assert!(validate_plugin_id("scope/plugin").is_err());
        assert_eq!(validate_plugin_id("safe_plugin").unwrap(), "safe_plugin");
        assert_eq!(
            validate_plugin_id("@scope plugin").unwrap(),
            "@scope plugin"
        );
        assert!(sanitize_plugin_id("..").is_err());
        assert!(sanitize_plugin_id(&"a".repeat(81)).is_err());
    }

    #[test]
    fn detects_supported_archive_formats() {
        assert_eq!(
            archive_kind(b"PK\x03\x04payload", "plugin.bin").unwrap(),
            ArchiveKind::Zip
        );
        assert_eq!(
            archive_kind(&[0x1f, 0x8b, 0x08], "plugin.bin").unwrap(),
            ArchiveKind::GzipTar
        );
        assert_eq!(
            archive_kind(b"ustar", "plugin.tar").unwrap(),
            ArchiveKind::Tar
        );
        assert!(archive_kind(b"text", "plugin.txt").is_err());
    }

    #[test]
    fn extracts_zip_payload_into_a_confined_directory() {
        use std::io::Write;
        use tempfile::tempdir;
        use zip::{write::SimpleFileOptions, ZipWriter};

        let mut archive = ZipWriter::new(std::io::Cursor::new(Vec::<u8>::new()));
        archive
            .start_file("demo/package.json", SimpleFileOptions::default())
            .unwrap();
        archive
            .write_all(br#"{"name":"demo","version":"1.0.0"}"#)
            .unwrap();
        archive
            .start_file("demo/skills/example.md", SimpleFileOptions::default())
            .unwrap();
        archive.write_all(b"# Example").unwrap();
        let bytes = archive.finish().unwrap().into_inner();

        let root = tempdir().unwrap();
        let destination = root.path().join("out");
        std::fs::create_dir_all(&destination).unwrap();
        extract_zip(&bytes, &destination).unwrap();
        assert!(destination.join("demo/package.json").is_file());
        assert!(destination.join("demo/skills/example.md").is_file());
        let plugin_root = locate_plugin_root(&destination).unwrap();
        assert_eq!(
            plugin_root.file_name().and_then(|value| value.to_str()),
            Some("demo")
        );
    }
}
