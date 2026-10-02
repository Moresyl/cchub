use super::{S3SyncSettings, DB_COMPAT_VERSION, MANIFEST_NAME, MAX_SYNC_BYTES, PROTOCOL_VERSION};
use crate::cloud_credentials;
use chrono::Utc;
use sha2::{Digest, Sha256};
use std::time::Duration;

pub(super) fn normalize_segment(value: &str, fallback: &str) -> String {
    let normalized = value.trim().trim_matches('/');
    if normalized.is_empty() {
        fallback.to_string()
    } else {
        normalized.to_string()
    }
}

pub(super) fn endpoint(settings: &S3SyncSettings) -> String {
    if settings.endpoint.is_empty() {
        format!("https://s3.{}.amazonaws.com", settings.region)
    } else if settings.endpoint.starts_with("http://") || settings.endpoint.starts_with("https://")
    {
        settings.endpoint.clone()
    } else {
        format!("https://{}", settings.endpoint)
    }
}

pub(super) fn profile_path(settings: &S3SyncSettings) -> String {
    format!(
        "{}/v{}/db-v{}/{}",
        settings.remote_root, PROTOCOL_VERSION, DB_COMPAT_VERSION, settings.profile
    )
}

pub(super) fn object_key(settings: &S3SyncSettings, name: &str) -> String {
    format!(
        "{}/{}",
        profile_path(settings),
        name.trim_start_matches('/')
    )
}

pub(super) fn object_url(settings: &S3SyncSettings, key: &str) -> Result<url::Url, String> {
    let mut url = cloud_credentials::validate_url(&endpoint(settings))?;
    let mut path = format!(
        "{}/{}/",
        url.path().trim_end_matches('/'),
        settings.bucket.trim_matches('/')
    );
    path.push_str(key.trim_matches('/'));
    url.set_path(&path);
    Ok(url)
}

pub(super) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub(super) fn sha256_hex(bytes: &[u8]) -> String {
    crate::cloud_transfer::sha256(bytes)
}

pub(super) fn hmac_sha256(key: &[u8], message: &[u8]) -> Vec<u8> {
    let mut block = [0u8; 64];
    if key.len() > block.len() {
        block[..32].copy_from_slice(&Sha256::digest(key));
    } else {
        block[..key.len()].copy_from_slice(key);
    }
    let mut inner = [0u8; 64];
    let mut outer = [0u8; 64];
    for index in 0..64 {
        inner[index] = block[index] ^ 0x36;
        outer[index] = block[index] ^ 0x5c;
    }
    let mut inner_hash = Sha256::new();
    inner_hash.update(inner);
    inner_hash.update(message);
    let mut outer_hash = Sha256::new();
    outer_hash.update(outer);
    outer_hash.update(inner_hash.finalize());
    outer_hash.finalize().to_vec()
}

pub(super) fn signed_request(
    client: &reqwest::Client,
    settings: &S3SyncSettings,
    method: reqwest::Method,
    key: &str,
    body: Vec<u8>,
    condition: Option<&crate::cloud_revision::WriteCondition>,
) -> Result<reqwest::RequestBuilder, String> {
    let url = object_url(settings, key)?;
    let host = host_header(&url)?;
    let now = Utc::now();
    let amz_date = now.format("%Y%m%dT%H%M%SZ").to_string();
    let short_date = now.format("%Y%m%d").to_string();
    let payload_hash = sha256_hex(&body);
    let canonical_uri = if url.path().is_empty() {
        "/"
    } else {
        url.path()
    };
    let (conditional_header, conditional_value) = match condition {
        Some(crate::cloud_revision::WriteCondition::Absent) => ("if-none-match", "*"),
        Some(crate::cloud_revision::WriteCondition::Matches(value)) => (
            "if-match",
            value.to_str().map_err(|_| "无效的远端版本标识")?,
        ),
        None => ("", ""),
    };
    let conditional_line = if condition.is_some() {
        format!("{conditional_header}:{conditional_value}\n")
    } else {
        String::new()
    };
    let canonical_headers = format!("host:{host}\n{conditional_line}x-amz-content-sha256:{payload_hash}\nx-amz-date:{amz_date}\n");
    let signed_headers = if condition.is_some() {
        format!("host;{conditional_header};x-amz-content-sha256;x-amz-date")
    } else {
        "host;x-amz-content-sha256;x-amz-date".into()
    };
    let canonical_request = format!(
        "{}\n{}\n\n{}\n{}\n{}",
        method.as_str(),
        canonical_uri,
        canonical_headers,
        signed_headers,
        payload_hash
    );
    let scope = format!("{short_date}/{}/{}/aws4_request", settings.region, "s3");
    let credential_scope = scope.clone();
    let string_to_sign = format!(
        "AWS4-HMAC-SHA256\n{amz_date}\n{credential_scope}\n{}",
        sha256_hex(canonical_request.as_bytes())
    );
    let k_date = hmac_sha256(
        format!("AWS4{}", settings.secret_access_key).as_bytes(),
        short_date.as_bytes(),
    );
    let k_region = hmac_sha256(&k_date, settings.region.as_bytes());
    let k_service = hmac_sha256(&k_region, b"s3");
    let k_signing = hmac_sha256(&k_service, b"aws4_request");
    let signature = hex(&hmac_sha256(&k_signing, string_to_sign.as_bytes()));
    let authorization = format!(
        "AWS4-HMAC-SHA256 Credential={}/{}, SignedHeaders={}, Signature={}",
        settings.access_key_id, scope, signed_headers, signature
    );
    let request = client
        .request(method, url)
        .header("host", host)
        .header("x-amz-date", amz_date)
        .header("x-amz-content-sha256", payload_hash)
        .header("Authorization", authorization)
        .body(body);
    // Conditional fields participate in the signature; send() adds their
    // actual headers once. Appending them here too would duplicate values.
    Ok(request)
}

pub(super) fn host_header(url: &url::Url) -> Result<String, String> {
    let host = url
        .host_str()
        .ok_or_else(|| "S3 endpoint has no host".to_string())?;
    Ok(match url.port() {
        Some(port) => format!("{host}:{port}"),
        None => host.to_string(),
    })
}

pub(super) fn client(settings: &S3SyncSettings) -> Result<reqwest::Client, String> {
    crate::shared::http_client::build_http_client(
        settings.proxy_url.as_deref(),
        Some(&format!("CCHub/{} S3", env!("CARGO_PKG_VERSION"))),
        Duration::from_secs(30),
    )
    .map_err(|error| format!("Failed to build S3 HTTP client: {error}"))
}

pub(super) async fn request_object(
    settings: &S3SyncSettings,
    method: reqwest::Method,
    key: &str,
    body: Vec<u8>,
) -> Result<reqwest::Response, String> {
    request_object_with_client(&client(settings)?, settings, method, key, body).await
}

async fn request_object_with_client(
    client: &reqwest::Client,
    settings: &S3SyncSettings,
    method: reqwest::Method,
    key: &str,
    body: Vec<u8>,
) -> Result<reqwest::Response, String> {
    let request = signed_request(client, settings, method, key, body, None)?;
    crate::cloud_http::send(request, &super::credential_scope(settings)).await
}

pub(super) async fn get_object(
    settings: &S3SyncSettings,
    key: &str,
) -> Result<Option<Vec<u8>>, String> {
    Ok(get_object_with_headers(settings, key)
        .await?
        .map(|(bytes, _)| bytes))
}

pub(super) async fn get_object_with_headers(
    settings: &S3SyncSettings,
    key: &str,
) -> Result<Option<(Vec<u8>, reqwest::header::HeaderMap)>, String> {
    get_object_with_headers_using(&client(settings)?, settings, key).await
}

pub(super) async fn get_object_with_headers_using(
    client: &reqwest::Client,
    settings: &S3SyncSettings,
    key: &str,
) -> Result<Option<(Vec<u8>, reqwest::header::HeaderMap)>, String> {
    let response =
        request_object_with_client(client, settings, reqwest::Method::GET, key, Vec::new()).await?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    let response = response
        .error_for_status()
        .map_err(|error| format!("S3 download failed: {error}"))?;
    let limit = if key == object_key(settings, MANIFEST_NAME) {
        crate::cloud_transfer::MANIFEST_LIMIT
    } else {
        MAX_SYNC_BYTES
    };
    let headers = response.headers().clone();
    Ok(Some((
        crate::cloud_transfer::read_bounded(response, limit).await?,
        headers,
    )))
}

pub(super) async fn put_object(
    settings: &S3SyncSettings,
    key: &str,
    body: Vec<u8>,
    condition: &crate::cloud_revision::WriteCondition,
) -> Result<reqwest::header::HeaderMap, String> {
    put_object_using(&client(settings)?, settings, key, body, condition).await
}

pub(super) async fn put_object_using(
    client: &reqwest::Client,
    settings: &S3SyncSettings,
    key: &str,
    body: Vec<u8>,
    condition: &crate::cloud_revision::WriteCondition,
) -> Result<reqwest::header::HeaderMap, String> {
    let request = signed_request(
        client,
        settings,
        reqwest::Method::PUT,
        key,
        body,
        Some(condition),
    )?;
    crate::cloud_revision::send(request, condition, &super::credential_scope(settings)).await
}
