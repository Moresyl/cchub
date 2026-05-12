use std::process::Command;
use std::time::Duration;

use reqwest::{Method, StatusCode, Url};

use crate::utils::configure_background_command;

use super::{
    WebDavManifest, WebDavRemoteLayout, WebDavSyncSettings, WEBDAV_DB_COMPAT_VERSION,
    WEBDAV_FORMAT, WEBDAV_MANIFEST_FILE, WEBDAV_PROTOCOL_VERSION,
};

pub(super) fn normalize_segment(value: &str, fallback: &str) -> String {
    let trimmed = value.trim().trim_matches('/');
    if trimmed.is_empty() {
        fallback.to_string()
    } else {
        trimmed.to_string()
    }
}

pub(super) fn normalize_base_url(value: &str) -> String {
    format!("{}/", value.trim().trim_end_matches('/'))
}

pub(super) fn validate_base_url(value: &str) -> Result<(), String> {
    let url = Url::parse(&normalize_base_url(value))
        .map_err(|_| "WebDAV base URL is invalid".to_string())?;
    match url.scheme() {
        "http" | "https" => Ok(()),
        _ => Err("WebDAV base URL must use http or https".to_string()),
    }
}

pub(super) fn method_propfind() -> Result<Method, String> {
    Method::from_bytes(b"PROPFIND")
        .map_err(|error| format!("Failed to construct PROPFIND method: {error}"))
}

pub(super) fn method_mkcol() -> Result<Method, String> {
    Method::from_bytes(b"MKCOL")
        .map_err(|error| format!("Failed to construct MKCOL method: {error}"))
}

pub(super) fn remote_prefix_segments(
    settings: &WebDavSyncSettings,
    layout: WebDavRemoteLayout,
) -> Vec<String> {
    match layout {
        WebDavRemoteLayout::Current => vec![
            settings.remote_root.clone(),
            format!("v{WEBDAV_PROTOCOL_VERSION}"),
            format!("db-v{WEBDAV_DB_COMPAT_VERSION}"),
            settings.profile.clone(),
        ],
        WebDavRemoteLayout::Legacy => {
            vec![settings.remote_root.clone(), settings.profile.clone()]
        }
    }
}

pub(super) fn remote_profile_path(
    settings: &WebDavSyncSettings,
    layout: WebDavRemoteLayout,
) -> String {
    remote_prefix_segments(settings, layout).join("/")
}

pub(super) fn split_relative_path(relative_path: &str) -> Vec<String> {
    relative_path
        .trim_matches('/')
        .split('/')
        .filter(|segment| !segment.is_empty())
        .map(ToString::to_string)
        .collect()
}

pub(super) fn build_remote_url(
    base_url: &str,
    segments: &[String],
    trailing_slash: bool,
) -> Result<String, String> {
    validate_base_url(base_url)?;
    let mut url = Url::parse(&normalize_base_url(base_url))
        .map_err(|error| format!("WebDAV base URL is invalid: {error}"))?;
    {
        let mut path = url
            .path_segments_mut()
            .map_err(|_| "WebDAV base URL cannot be used to append path segments".to_string())?;
        path.pop_if_empty();
        for segment in segments {
            path.push(segment);
        }
    }

    let mut output = url.to_string();
    if trailing_slash && !output.ends_with('/') {
        output.push('/');
    }
    Ok(output)
}

pub(super) fn remote_file_url(
    settings: &WebDavSyncSettings,
    layout: WebDavRemoteLayout,
    relative_path: &str,
) -> Result<String, String> {
    let mut segments = remote_prefix_segments(settings, layout);
    segments.extend(split_relative_path(relative_path));
    build_remote_url(&settings.base_url, &segments, false)
}

pub(super) fn manifest_url_for_layout(
    settings: &WebDavSyncSettings,
    layout: WebDavRemoteLayout,
) -> Result<String, String> {
    remote_file_url(settings, layout, WEBDAV_MANIFEST_FILE)
}

pub(super) fn build_client() -> Result<reqwest::Client, String> {
    crate::shared::http_client::build_http_client(
        None,
        Some(&format!("CCHub/{} WebDAV", env!("CARGO_PKG_VERSION"))),
        Duration::from_secs(30),
    )
    .map_err(|error| format!("Failed to build WebDAV HTTP client: {error}"))
}

pub(super) fn auth_request(
    builder: reqwest::RequestBuilder,
    settings: &WebDavSyncSettings,
) -> reqwest::RequestBuilder {
    builder.basic_auth(&settings.username, Some(&settings.password))
}

pub(super) async fn ensure_remote_directories(
    client: &reqwest::Client,
    settings: &WebDavSyncSettings,
    layout: WebDavRemoteLayout,
) -> Result<(), String> {
    let mkcol = method_mkcol()?;
    let mut path_segments = remote_prefix_segments(settings, layout);
    path_segments.push("snapshots".to_string());

    for depth in 1..=path_segments.len() {
        let target = build_remote_url(&settings.base_url, &path_segments[..depth], true)?;
        let response = auth_request(client.request(mkcol.clone(), target), settings)
            .send()
            .await
            .map_err(|error| format!("Failed to create remote WebDAV directory: {error}"))?;
        let status = response.status();
        if !(status.is_success()
            || status == StatusCode::METHOD_NOT_ALLOWED
            || status == StatusCode::CONFLICT
            || status.is_redirection())
        {
            return Err(format!(
                "WebDAV directory creation failed with status {status}"
            ));
        }
    }
    Ok(())
}

pub(super) async fn upload_bytes(
    client: &reqwest::Client,
    settings: &WebDavSyncSettings,
    url: &str,
    content_type: &str,
    bytes: Vec<u8>,
) -> Result<(), String> {
    auth_request(
        client
            .put(url)
            .header("Content-Type", content_type)
            .body(bytes),
        settings,
    )
    .send()
    .await
    .map_err(|error| format!("WebDAV upload failed: {error}"))?
    .error_for_status()
    .map_err(|error| format!("WebDAV upload returned error: {error}"))?;
    Ok(())
}

pub(super) async fn fetch_manifest_with_fallback(
    client: &reqwest::Client,
    settings: &WebDavSyncSettings,
) -> Result<Option<(WebDavManifest, WebDavRemoteLayout)>, String> {
    if let Some(manifest) =
        fetch_manifest_for_layout(client, settings, WebDavRemoteLayout::Current).await?
    {
        return Ok(Some((manifest, WebDavRemoteLayout::Current)));
    }
    if let Some(manifest) =
        fetch_manifest_for_layout(client, settings, WebDavRemoteLayout::Legacy).await?
    {
        return Ok(Some((manifest, WebDavRemoteLayout::Legacy)));
    }
    Ok(None)
}

pub(super) async fn fetch_manifest_for_layout(
    client: &reqwest::Client,
    settings: &WebDavSyncSettings,
    layout: WebDavRemoteLayout,
) -> Result<Option<WebDavManifest>, String> {
    let response = auth_request(
        client.get(manifest_url_for_layout(settings, layout)?),
        settings,
    )
    .send()
    .await
    .map_err(|error| format!("Failed to fetch WebDAV manifest: {error}"))?;

    if response.status() == StatusCode::NOT_FOUND {
        return Ok(None);
    }

    let response = response
        .error_for_status()
        .map_err(|error| format!("WebDAV manifest request failed: {error}"))?;
    let bytes = response
        .bytes()
        .await
        .map_err(|error| format!("Failed to read WebDAV manifest body: {error}"))?;
    let manifest = serde_json::from_slice::<WebDavManifest>(&bytes)
        .map_err(|error| format!("Invalid WebDAV manifest: {error}"))?;
    Ok(Some(manifest))
}

pub(super) fn validate_manifest_compatibility(
    manifest: &WebDavManifest,
    layout: WebDavRemoteLayout,
) -> Result<(), String> {
    if manifest.snapshot_path.trim().is_empty() {
        return Err("WebDAV manifest is missing snapshot path".to_string());
    }

    if layout == WebDavRemoteLayout::Current {
        if manifest.format != WEBDAV_FORMAT {
            return Err(format!(
                "WebDAV manifest format mismatch: expected {WEBDAV_FORMAT}, got {}",
                manifest.format
            ));
        }
        if manifest.protocol_version != Some(WEBDAV_PROTOCOL_VERSION) {
            return Err(format!(
                "WebDAV protocol version mismatch: expected v{WEBDAV_PROTOCOL_VERSION}, got {:?}",
                manifest.protocol_version
            ));
        }
        if manifest.db_compat_version != Some(WEBDAV_DB_COMPAT_VERSION) {
            return Err(format!(
                "WebDAV DB compatibility mismatch: expected db-v{WEBDAV_DB_COMPAT_VERSION}, got {:?}",
                manifest.db_compat_version
            ));
        }
    }

    Ok(())
}

pub(super) fn device_name() -> String {
    for key in ["CC_SWITCH_DEVICE_NAME", "COMPUTERNAME", "HOSTNAME"] {
        if let Ok(value) = std::env::var(key) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return trimmed.chars().take(64).collect();
            }
        }
    }

    let mut command = Command::new("hostname");
    configure_background_command(&mut command);
    if let Ok(output) = command.output() {
        if output.status.success() {
            if let Ok(hostname) = String::from_utf8(output.stdout) {
                let trimmed = hostname.trim();
                if !trimmed.is_empty() {
                    return trimmed.chars().take(64).collect();
                }
            }
        }
    }

    "cchub".to_string()
}
