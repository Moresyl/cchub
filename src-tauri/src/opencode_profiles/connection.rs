use serde_json::{Map, Value};

pub(crate) struct Connection {
    pub package: String,
    pub settings: Map<String, Value>,
    pub model: String,
    pub headers: Vec<(String, String)>,
    pub body: Value,
}

impl Connection {
    pub(crate) fn body_override(&self, extra: Option<Value>) -> Option<Value> {
        if self.body.as_object().is_none_or(Map::is_empty) {
            return extra;
        }
        let mut body = self.body.clone();
        if let Some(extra) = extra {
            super::merge_fields(&mut body, &extra);
        }
        Some(body)
    }

    pub(crate) fn apply_probe_body(&self, body: &mut Value) {
        let original = body.clone();
        super::merge_fields(body, &self.body);
        for key in [
            "model",
            "messages",
            "input",
            "contents",
            "stream",
            "max_tokens",
            "max_output_tokens",
            "max_completion_tokens",
        ] {
            if let Some(value) = original.get(key) {
                body[key] = value.clone();
            } else {
                body.as_object_mut().unwrap().remove(key);
            }
        }
        if let Some(limit) = original.pointer("/generationConfig/maxOutputTokens") {
            if !body.get("generationConfig").is_some_and(Value::is_object) {
                body["generationConfig"] = serde_json::json!({});
            }
            body["generationConfig"]["maxOutputTokens"] = limit.clone();
        }
    }

    pub(crate) fn base_url(&self) -> Option<String> {
        self.text("baseURL").or_else(|| self.text("baseUrl"))
    }

    pub(crate) fn text(&self, key: &str) -> Option<String> {
        let value = self.settings.get(key)?.as_str()?.trim();
        if value.is_empty() {
            return None;
        }
        if let Some(variable) = value
            .strip_prefix("{env:")
            .and_then(|value| value.strip_suffix('}'))
        {
            return std::env::var(variable)
                .ok()
                .filter(|value| !value.trim().is_empty());
        }
        // Do not transmit unresolved file references as literal credentials.
        if value.starts_with("{file:") {
            return None;
        }
        Some(value.to_string())
    }

    pub(crate) fn responses(&self) -> bool {
        self.package == "@ai-sdk/openai"
            || self.package.ends_with("/openai")
            || self.package.ends_with("/responses")
    }

    pub(crate) fn default_base_url(&self) -> String {
        if self.package.contains("anthropic") {
            "https://api.anthropic.com"
        } else if self.package.contains("google") {
            "https://generativelanguage.googleapis.com/v1beta"
        } else {
            "https://api.openai.com/v1"
        }
        .into()
    }

    pub(crate) fn auth_headers(&self) -> Result<Vec<(String, String)>, String> {
        let mut headers: Vec<(String, String)> = if let Some(token) = self.text("authToken") {
            vec![("authorization".into(), format!("Bearer {token}"))]
        } else if let Some(token) = self.text("apiKey") {
            if self.package.contains("anthropic") {
                vec![("x-api-key".into(), token)]
            } else if self.package.contains("google") {
                vec![("x-goog-api-key".into(), token)]
            } else {
                vec![("authorization".into(), format!("Bearer {token}"))]
            }
        } else {
            Vec::new()
        };
        if self.package.contains("anthropic") {
            headers.push(("anthropic-version".into(), "2023-06-01".into()));
        }
        for (name, value) in &self.headers {
            let parsed_name = reqwest::header::HeaderName::from_bytes(name.as_bytes())
                .map_err(|_| "Invalid OpenCode request header name")?;
            reqwest::header::HeaderValue::from_str(value)
                .map_err(|_| "Invalid OpenCode request header value")?;
            if matches!(
                parsed_name.as_str(),
                "host" | "connection" | "content-length" | "transfer-encoding" | "upgrade"
            ) {
                return Err("OpenCode request headers cannot override transport headers".into());
            }
            headers.retain(|(existing, _)| !name.eq_ignore_ascii_case(existing));
            headers.push((name.clone(), value.clone()));
        }
        if !headers.iter().any(|(name, value)| {
            !value.trim().is_empty()
                && matches!(
                    name.to_ascii_lowercase().as_str(),
                    "authorization" | "x-api-key" | "x-goog-api-key"
                )
        }) {
            return Err("No OpenCode API credential configured".into());
        }
        Ok(headers)
    }
}

fn overlay(target: &mut Map<String, Value>, source: Option<&Value>) {
    if let Some(source) = source.and_then(Value::as_object) {
        for (name, value) in source {
            target.insert(name.clone(), value.clone());
        }
    }
}

pub(crate) fn has_credentials(document: &Value) -> bool {
    if document.get("model").is_some()
        || (document.get("provider").is_none() && document.get("providers").is_none())
    {
        return from_profile(document).is_ok_and(|connection| connection.auth_headers().is_ok());
    }
    ["provider", "providers"]
        .into_iter()
        .filter_map(|key| document.get(key).and_then(Value::as_object))
        .flat_map(|providers| providers.keys())
        .any(|id| {
            let mut document = document.clone();
            document["model"] = Value::String(format!("{id}/_"));
            from_profile(&document).is_ok_and(|connection| connection.auth_headers().is_ok())
        })
}

pub(crate) fn from_profile(value: &Value) -> Result<Connection, String> {
    let normalized = super::normalize_profile(&value.to_string())?;
    let profile: Value = serde_json::from_str(&normalized).map_err(|error| error.to_string())?;
    let native = super::native::is_native(&profile);
    let models = profile.get("models").and_then(Value::as_object);
    let selection = profile
        .pointer("/metadata/nativeModelId")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| models.and_then(|models| models.keys().next().cloned()))
        .unwrap_or_default();
    let (model_id, variant) = selection
        .split_once('#')
        .map(|(model, variant)| (model, Some(variant)))
        .unwrap_or((&selection, None));
    let model = models.and_then(|models| models.get(model_id));
    let variant = if native {
        variant.and_then(|id| {
            model?
                .get("variants")?
                .as_array()?
                .iter()
                .find(|item| item.get("id").and_then(Value::as_str) == Some(id))
        })
    } else {
        None
    };
    let mut settings = Map::new();
    let mut headers = Map::new();
    let mut body = Map::new();
    overlay(
        &mut settings,
        profile.get(if native { "settings" } else { "options" }),
    );
    if native {
        for item in [Some(&profile), model, variant].into_iter().flatten() {
            overlay(&mut settings, item.get("settings"));
            overlay(&mut headers, item.get("headers"));
            overlay(&mut body, item.get("body"));
        }
        if !settings.contains_key("apiKey") {
            if let Some(key) = profile
                .get("env")
                .and_then(Value::as_array)
                .and_then(|names| {
                    names.iter().filter_map(Value::as_str).find_map(|name| {
                        std::env::var(name)
                            .ok()
                            .filter(|value| !value.trim().is_empty())
                    })
                })
            {
                settings.insert("apiKey".into(), Value::String(key));
            }
        }
    }
    let provider_id = profile
        .get("canonical")
        .and_then(Value::as_str)
        .or_else(|| {
            profile
                .pointer("/metadata/nativeProviderId")
                .and_then(Value::as_str)
        });
    let default_package = match provider_id {
        Some("anthropic") => "@opencode/ai/anthropic",
        Some("google") => "@opencode/ai/google",
        Some("openai") => "@opencode/ai/openai",
        _ => "@opencode/ai/openai-compatible",
    };
    let package = if native {
        model
            .and_then(|model| model.get("package"))
            .or_else(|| profile.get("package"))
    } else {
        profile.get("npm")
    }
    .and_then(Value::as_str)
    .unwrap_or(default_package)
    .to_string();
    Ok(Connection {
        package,
        settings,
        model: model
            .and_then(|value| value.get("modelID"))
            .and_then(Value::as_str)
            .unwrap_or(model_id)
            .to_string(),
        headers: headers
            .into_iter()
            .filter_map(|(name, value)| value.as_str().map(|value| (name, value.to_string())))
            .collect(),
        body: Value::Object(body),
    })
}
