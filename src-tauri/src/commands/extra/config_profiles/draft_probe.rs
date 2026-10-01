use futures_util::{stream, StreamExt};
use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, State};

use super::{
    build_provider_probe_client, extract_stream_check_request, ConfigProfile,
    StreamCheckRequestSpec,
};
use crate::db::DbState;
use crate::provider_proxy::{extract_local_proxy_overrides, extract_transport_headers};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DraftModelCheckResult {
    pub model: String,
    pub status: String,
    pub http_status: Option<u16>,
    pub latency_ms: Option<u64>,
    pub message: String,
}

#[tauri::command(rename_all = "camelCase")]
pub async fn test_profile_draft<R: tauri::Runtime>(
    tool_id: String,
    config_snapshot: String,
    models: Vec<String>,
    app_handle: AppHandle<R>,
    db: State<'_, DbState>,
) -> Result<Vec<DraftModelCheckResult>, String> {
    let client = {
        let conn =
            db.0.lock()
                .map_err(|_| "Configuration store is unavailable")?;
        build_provider_probe_client(&conn)?
    };
    check_draft(&app_handle, tool_id, config_snapshot, models, client).await
}

pub(super) async fn check_draft<R: tauri::Runtime>(
    app: &AppHandle<R>,
    tool: String,
    snapshot: String,
    models: Vec<String>,
    client: reqwest::Client,
) -> Result<Vec<DraftModelCheckResult>, String> {
    if snapshot.len() > 1024 * 1024 {
        return Err("Draft configuration is too large to test".into());
    }
    let parsed: Value = serde_json::from_str(&snapshot)
        .map_err(|_| "Correct the draft JSON before testing models")?;
    if !parsed.is_object() {
        return Err("Draft configuration must be a JSON object".into());
    }
    let models = validate_models(models)?;
    let profile = ConfigProfile {
        id: "draft".into(),
        name: "Draft".into(),
        tool_id: tool,
        config_snapshot: snapshot,
        sort_order: 0,
        source_type: None,
        source_key: None,
        created_at: None,
        updated_at: None,
    };
    let request = extract_stream_check_request(app, &profile).await?;
    let requests = models
        .into_iter()
        .map(|model| {
            prepare_model_request(&parsed, &request, &model).map(|request| (model, request))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(stream::iter(requests)
        .map(|(model, request)| {
            let client = client.clone();
            async move {
                let outcome = super::stream_probe::execute(client, request).await;
                DraftModelCheckResult {
                    model,
                    status: outcome.status.into(),
                    http_status: outcome.http_status,
                    latency_ms: outcome.latency_ms,
                    message: outcome.message,
                }
            }
        })
        .buffered(4)
        .collect()
        .await)
}

fn validate_models(models: Vec<String>) -> Result<Vec<String>, String> {
    if models.is_empty() || models.len() > 32 {
        return Err("Select between 1 and 32 models to test".into());
    }
    let mut result = Vec::new();
    for model in models {
        let model = model.trim();
        if model.is_empty() || model.len() > 256 || model.chars().any(char::is_control) {
            return Err("Model IDs must be non-empty and contain no control characters".into());
        }
        if !result.iter().any(|item| item == model) {
            result.push(model.to_string());
        }
    }
    Ok(result)
}

fn prepare_model_request(
    parsed: &Value,
    original: &StreamCheckRequestSpec,
    model: &str,
) -> Result<StreamCheckRequestSpec, String> {
    let mut request = StreamCheckRequestSpec {
        endpoint: original.endpoint.clone(),
        headers: original.headers.clone(),
        body: original.body.clone(),
    };
    let (overrides, body) = extract_local_proxy_overrides(parsed);
    for (name, value) in extract_transport_headers(parsed)
        .into_iter()
        .chain(overrides)
    {
        request
            .headers
            .retain(|(existing, _)| !existing.eq_ignore_ascii_case(&name));
        request.headers.push((name, value));
    }
    if let Some(body) = body {
        merge(&mut request.body, &body);
    }
    if let Some(object) = request.body.as_object_mut() {
        for field in [
            "messages",
            "input",
            "contents",
            "stream",
            "max_tokens",
            "max_output_tokens",
            "max_completion_tokens",
        ] {
            object.remove(field);
        }
    }
    // The transport options are the draft's; prompt, model and budget are the explicit probe's.
    for field in [
        "messages",
        "input",
        "contents",
        "stream",
        "max_tokens",
        "max_output_tokens",
    ] {
        if let Some(value) = original.body.get(field) {
            request.body[field] = value.clone();
        }
    }
    if let Some(limit) = original.body.pointer("/generationConfig/maxOutputTokens") {
        if !request
            .body
            .get("generationConfig")
            .is_some_and(Value::is_object)
        {
            request.body["generationConfig"] = serde_json::json!({});
        }
        request.body["generationConfig"]["maxOutputTokens"] = limit.clone();
    }
    if original.body.get("model").is_some() {
        request.body["model"] = Value::String(model.into());
    } else {
        let mut url =
            reqwest::Url::parse(&request.endpoint).map_err(|_| "Draft endpoint URL is invalid")?;
        let mut segments = url
            .path_segments()
            .ok_or("Draft endpoint URL has no model path")?
            .map(str::to_string)
            .collect::<Vec<_>>();
        let index = segments
            .iter()
            .rposition(|part| part.ends_with(":streamGenerateContent"))
            .ok_or("The full endpoint cannot select a different model")?;
        segments[index] = format!("{model}:streamGenerateContent");
        url.path_segments_mut()
            .map_err(|_| "Draft endpoint URL has no model path")?
            .clear()
            .extend(&segments);
        request.endpoint = url.into();
    }
    Ok(request)
}

fn merge(target: &mut Value, overrides: &Value) {
    if let (Some(target), Some(overrides)) = (target.as_object_mut(), overrides.as_object()) {
        for (key, value) in overrides {
            if let Some(current) = target
                .get_mut(key)
                .filter(|current| current.is_object() && value.is_object())
            {
                merge(current, value);
            } else {
                target.insert(key.clone(), value.clone());
            }
        }
    }
}

#[cfg(test)]
mod tests;
