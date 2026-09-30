//! A viewed remote backup is not permission for automatic replacement.
use reqwest::{
    header::{HeaderMap, HeaderValue, ETAG, IF_MATCH, IF_NONE_MATCH},
    RequestBuilder, StatusCode,
};
use serde::{Deserialize, Serialize};

use crate::{
    cloud_credentials::{self, CredentialStore},
    cloud_transfer::sha256,
};

pub(crate) const CONFLICT: &str =
    "远端备份已变化，上传已停止。请刷新远端，选择恢复备份或确认用本地配置替换";
const UNSUPPORTED: &str =
    "服务器没有提供可用于安全替换的版本标识，请更换支持条件写入的存储；仍可下载备份";

#[derive(Debug, Clone)]
pub(crate) struct ObservedRevision {
    token: String,
    etag: Option<String>,
    manifest_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadReview {
    pub revision: String,
    pub requires_confirmation: bool,
    pub conditional_supported: bool,
}

#[derive(Debug, Clone)]
pub(crate) enum WriteCondition {
    Absent,
    Matches(HeaderValue),
}

fn valid_etag(value: &str) -> bool {
    value.len() <= 1024
        && value.starts_with('"')
        && value.ends_with('"')
        && value.len() >= 2
        && value.as_bytes()[1..value.len() - 1]
            .iter()
            .all(|byte| *byte == 0x21 || (0x23..=0x7e).contains(byte))
}

pub(crate) fn strong_etag(headers: &HeaderMap) -> Option<String> {
    let mut values = headers.get_all(ETAG).iter();
    let value = values.next()?.to_str().ok()?;
    if values.next().is_some() || !valid_etag(value) {
        return None;
    }
    Some(value.to_string())
}

fn token(scope: &str, hash: &str, etag: &str) -> String {
    let mut bytes = Vec::new();
    for value in [scope, hash, etag] {
        bytes.extend_from_slice(&(value.len() as u64).to_be_bytes());
        bytes.extend_from_slice(value.as_bytes());
    }
    sha256(&bytes)
}

pub(crate) fn observe(scope: &str, bytes: &[u8], headers: &HeaderMap) -> ObservedRevision {
    let etag = strong_etag(headers);
    let manifest_hash = sha256(bytes);
    ObservedRevision {
        token: token(scope, &manifest_hash, etag.as_deref().unwrap_or("")),
        etag,
        manifest_hash,
    }
}

pub(crate) fn verify_written(
    bytes: &[u8],
    observed: Option<ObservedRevision>,
) -> Result<ObservedRevision, String> {
    let observed = observed.ok_or("备份已上传，但无法确认远端版本；请刷新远端后重试")?;
    if observed.manifest_hash != sha256(bytes) || observed.etag.is_none() {
        return Err("备份已上传，但远端版本无法确认或已经变化；请刷新远端后重试".into());
    }
    Ok(observed)
}

fn missing_token(scope: &str) -> String {
    token(scope, "missing", "")
}
fn account(scope: &str) -> String {
    cloud_credentials::scope("cloud_accepted_revision", scope, "")
}

pub(crate) fn review(
    store: &impl CredentialStore,
    scope: &str,
    observed: Option<&ObservedRevision>,
) -> Result<UploadReview, String> {
    let accepted = store.get(&account(scope))?;
    let revision = observed
        .map(|value| value.token.clone())
        .unwrap_or_else(|| missing_token(scope));
    Ok(UploadReview {
        requires_confirmation: match accepted {
            Some(accepted) => accepted != revision,
            None => observed.is_some(),
        },
        conditional_supported: observed.is_none_or(|value| value.etag.is_some()),
        revision,
    })
}

pub(crate) fn authorize(
    store: &impl CredentialStore,
    scope: &str,
    observed: Option<&ObservedRevision>,
    reviewed: Option<&str>,
) -> Result<WriteCondition, String> {
    let review = review(store, scope, observed)?;
    if !review.conditional_supported {
        return Err(UNSUPPORTED.into());
    }
    if let Some(expected) = reviewed {
        if expected != review.revision {
            return Err(CONFLICT.into());
        }
    } else if review.requires_confirmation {
        return Err(CONFLICT.into());
    }
    Ok(match observed {
        None => WriteCondition::Absent,
        Some(value) => WriteCondition::Matches(
            HeaderValue::from_str(value.etag.as_deref().ok_or(UNSUPPORTED)?)
                .map_err(|_| UNSUPPORTED)?,
        ),
    })
}

pub(crate) fn accept(
    store: &impl CredentialStore,
    scope: &str,
    observed: &ObservedRevision,
) -> Result<(), String> {
    store.set(&account(scope), &observed.token).map_err(|_| {
        "备份操作已完成，但未能记录本机接受的远端版本；下次上传前请重新检查远端".to_string()
    })
}

pub(crate) fn apply(request: RequestBuilder, condition: &WriteCondition) -> RequestBuilder {
    match condition {
        WriteCondition::Absent => request.header(IF_NONE_MATCH, "*"),
        WriteCondition::Matches(etag) => request.header(IF_MATCH, etag),
    }
}

pub(crate) async fn send(
    request: RequestBuilder,
    condition: &WriteCondition,
) -> Result<HeaderMap, String> {
    let response = apply(request, condition)
        .send()
        .await
        .map_err(|_| "条件上传请求失败，请检查连接并刷新远端状态".to_string())?;
    match response.status() {
        StatusCode::CONFLICT
        | StatusCode::PRECONDITION_FAILED
        | StatusCode::PRECONDITION_REQUIRED => Err(CONFLICT.into()),
        StatusCode::BAD_REQUEST | StatusCode::NOT_IMPLEMENTED | StatusCode::METHOD_NOT_ALLOWED => {
            Err(format!(
                "服务器拒绝条件写入（HTTP {}），上传已停止，请使用支持条件写入的存储",
                response.status().as_u16()
            ))
        }
        status if status.is_success() => Ok(response.headers().clone()),
        status => Err(format!(
            "条件上传失败（HTTP {}），请检查存储权限与连接",
            status.as_u16()
        )),
    }
}

#[cfg(test)]
pub(crate) mod tests;
