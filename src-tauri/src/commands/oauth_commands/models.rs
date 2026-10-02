use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::{Map, Value};

use crate::commands::provider_models::{merge_catalog, parse_model_info, ModelInfo};
use crate::shared::oauth_request::{read_json, ResourceError};

const MODELS_URL: &str = "https://chatgpt.com/backend-api/codex/models";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexCliModel {
    #[serde(flatten)]
    pub model: ModelInfo,
    pub owned_by: Option<String>,
}

pub(super) fn request(
    client: &reqwest::Client,
    token: &str,
    account_id: Option<&str>,
) -> reqwest::RequestBuilder {
    let mut request = client
        .get(MODELS_URL)
        .query(&[("client_version", env!("CARGO_PKG_VERSION"))])
        .bearer_auth(token)
        .header("originator", "cchub")
        .header("Accept", "application/json");
    if let Some(account_id) = account_id {
        request = request.header("ChatGPT-Account-Id", account_id);
    }
    request
}

fn resource_error(error: ResourceError) -> String {
    match error {
        ResourceError::Http(status) => format!("Codex model API returned HTTP {status}"),
        ResourceError::TooLarge => "Codex model response exceeds the size limit".into(),
        ResourceError::InvalidPayload => "Invalid Codex model response".into(),
        ResourceError::Timeout => "Codex model request timed out".into(),
        ResourceError::Transport => "Codex model request failed".into(),
    }
}

pub(super) async fn fetch_cli_models(
    request: reqwest::RequestBuilder,
) -> Result<Vec<CodexCliModel>, String> {
    // The deadline also covers streamed bodies, even if the supplied client has
    // no timeout. Neither transport errors nor upstream bodies enter UI messages.
    tokio::time::timeout(super::REQUEST_TIMEOUT, async {
        let response = request.send().await.map_err(|error| {
            resource_error(if error.is_timeout() {
                ResourceError::Timeout
            } else {
                ResourceError::Transport
            })
        })?;
        let value = read_json(response).await.map_err(resource_error)?;
        parse(&value)
    })
    .await
    .map_err(|_| resource_error(ResourceError::Timeout))?
}

fn text(object: &Map<String, Value>, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        object
            .get(*key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
    })
}

fn entry(value: &Value, fallback_id: Option<&str>) -> Option<CodexCliModel> {
    if let Some(id) = value.as_str().map(str::trim).filter(|id| !id.is_empty()) {
        return Some(CodexCliModel {
            model: ModelInfo {
                id: id.into(),
                ..Default::default()
            },
            owned_by: Some("Codex".into()),
        });
    }
    let object = value.as_object()?;
    if object.get("hidden").and_then(Value::as_bool) == Some(true)
        || object.get("visibility").and_then(Value::as_str) == Some("hide")
    {
        return None;
    }
    let id = text(object, &["slug", "id", "model", "name"]).or_else(|| {
        fallback_id
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .map(str::to_owned)
    })?;
    let mut normalized = object.clone();
    normalized.insert("id".into(), Value::String(id));
    let mut model = parse_model_info(&Value::Object(normalized), false)?;
    model.display_name = text(object, &["display_name", "displayName", "label"]);
    Some(CodexCliModel {
        model,
        owned_by: text(object, &["owned_by", "ownedBy", "provider", "vendor"])
            .or_else(|| Some("Codex".into())),
    })
}

pub(super) fn parse(value: &Value) -> Result<Vec<CodexCliModel>, String> {
    let entries = value
        .get("data")
        .and_then(Value::as_array)
        .or_else(|| value.get("models").and_then(Value::as_array))
        .or_else(|| value.get("items").and_then(Value::as_array))
        .or_else(|| value.as_array());
    let map = value.get("models").and_then(Value::as_object);
    let rows: Vec<_> = if let Some(entries) = entries {
        entries
            .iter()
            .filter_map(|value| entry(value, None))
            .collect()
    } else if let Some(map) = map {
        map.iter()
            .filter_map(|(id, value)| entry(value, Some(id)))
            .collect()
    } else {
        return Err("Invalid Codex model catalog container".into());
    };
    let owners: BTreeMap<_, _> = rows
        .iter()
        .map(|row| (row.model.id.clone(), row.owned_by.clone()))
        .collect();
    Ok(
        merge_catalog(rows.into_iter().map(|row| row.model).collect())
            .into_iter()
            .map(|model| CodexCliModel {
                owned_by: owners.get(&model.id).cloned().flatten(),
                model,
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests;
