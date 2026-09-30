//! Validate remote backup metadata before allocating or touching local state.
use sha2::{Digest, Sha256};

pub(crate) const MANIFEST_LIMIT: usize = 64 * 1024;
pub(crate) const SNAPSHOT_LIMIT: usize = 15 * 1024 * 1024;

pub(crate) async fn read_bounded(
    mut response: reqwest::Response,
    limit: usize,
) -> Result<Vec<u8>, String> {
    if response
        .content_length()
        .is_some_and(|size| size > limit as u64)
    {
        return Err("远端备份超过允许的大小，已停止下载".into());
    }
    let mut bytes = Vec::with_capacity(
        response
            .content_length()
            .unwrap_or(0)
            .min(limit as u64)
            .min(64 * 1024) as usize,
    );
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| "读取远端备份失败，未修改本地数据".to_string())?
    {
        if chunk.len() > limit.saturating_sub(bytes.len()) {
            return Err("远端备份超过允许的大小，已停止下载".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

pub(crate) fn validate_snapshot_path(path: &str) -> Result<(), String> {
    let file = path.strip_prefix("snapshots/").ok_or("远端备份路径无效")?;
    if file.is_empty()
        || matches!(file, "." | "..")
        || path.len() > 512
        || file
            .chars()
            .any(|ch| ch.is_control() || matches!(ch, '/' | '\\' | '%' | '?' | '#' | ':'))
    {
        return Err("远端备份路径无效".into());
    }
    Ok(())
}

pub(crate) fn validate_size_and_digest(size: u64, digest: Option<&str>) -> Result<(), String> {
    if size == 0 || size > SNAPSHOT_LIMIT as u64 {
        return Err("远端备份大小无效或超过允许的大小".into());
    }
    if let Some(digest) = digest {
        if digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err("远端备份校验信息无效".into());
        }
    }
    Ok(())
}

pub(crate) fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub(crate) fn verify_snapshot(bytes: &[u8], size: u64, digest: Option<&str>) -> Result<(), String> {
    validate_size_and_digest(size, digest)?;
    if bytes.len() as u64 != size
        || digest.is_some_and(|expected| !sha256(bytes).eq_ignore_ascii_case(expected))
    {
        return Err("远端备份完整性校验失败，未修改本地数据".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests;
