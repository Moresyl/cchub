use bytes::Bytes;
use serde_json::Value;

use super::ProxyRequestInsights;

const MAX_ROWS: usize = 128;
const MAX_NAME_BYTES: usize = 1024;

#[derive(Debug, Default)]
pub(crate) struct ModelAliases(Vec<(String, String)>);

fn valid_name(value: &str, template: bool) -> bool {
    !value.is_empty()
        && value.len() <= MAX_NAME_BYTES
        && value
            .chars()
            .all(|c| c.is_alphanumeric() || "._-/:@+".contains(c) || (template && c == '*'))
}

impl ModelAliases {
    pub(crate) fn from_snapshot(snapshot: &str) -> Result<Self, String> {
        // Non-JSON native snapshots are still accepted by the profile store.
        let Ok(snapshot) = serde_json::from_str::<Value>(snapshot) else {
            return Ok(Self::default());
        };
        let Some(raw) = snapshot
            .get("metadata")
            .and_then(|m| m.get("localProxyModelAliases"))
        else {
            return Ok(Self::default());
        };
        let rows = raw
            .as_array()
            .ok_or("Local proxy model aliases must be an array")?;
        if rows.len() > MAX_ROWS {
            return Err("Local proxy model aliases support at most 128 rows".into());
        }
        let mut aliases = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for (index, row) in rows.iter().enumerate() {
            let invalid = || format!("Invalid local proxy model alias at row {}", index + 1);
            let model = row
                .get("model")
                .and_then(Value::as_str)
                .ok_or_else(invalid)?
                .trim();
            let upstream = row
                .get("upstream")
                .and_then(Value::as_str)
                .ok_or_else(invalid)?
                .trim();
            if model.is_empty() && upstream.is_empty() {
                continue;
            }
            if !(model == "*" || valid_name(model, false)) || !valid_name(upstream, true) {
                return Err(invalid());
            }
            if !seen.insert(model.to_owned()) {
                return Err(format!(
                    "Duplicate local proxy model alias at row {}",
                    index + 1
                ));
            }
            aliases.push((model.to_owned(), upstream.to_owned()));
        }
        Ok(Self(aliases))
    }

    fn resolve(&self, model: &str) -> Result<Option<String>, String> {
        let template = self
            .0
            .iter()
            .find(|(key, _)| key == model)
            .or_else(|| self.0.iter().find(|(key, _)| key == "*"));
        let Some((_, template)) = template else {
            return Ok(None);
        };
        let stars = template.bytes().filter(|c| *c == b'*').count();
        if template.len() - stars + stars * model.len() > MAX_NAME_BYTES {
            return Err("Expanded local proxy model alias exceeds 1024 bytes".into());
        }
        let name = template.replace('*', model);
        if !valid_name(&name, false) {
            return Err("Expanded local proxy model alias is invalid".into());
        }
        Ok((name != model).then_some(name))
    }

    pub(super) fn apply(
        &self,
        path: String,
        body: Bytes,
        mut insights: ProxyRequestInsights,
    ) -> Result<(String, Bytes, ProxyRequestInsights), String> {
        let Some(model) = insights.request_model.as_deref() else {
            return Ok((path, body, insights));
        };
        let Some(sent) = self.resolve(model)? else {
            return Ok((path, body, insights));
        };
        let mut parsed: Value = serde_json::from_slice(&body)
            .map_err(|_| "Model aliases require a JSON request body")?;
        let path = rewrite_gemini_path(&path, &sent).unwrap_or(path);
        if parsed.get("model").is_some() {
            parsed["model"] = Value::String(sent.clone());
        }
        insights.upstream_model = Some(sent);
        let body = serde_json::to_vec(&parsed).map_err(|_| "Cannot encode model alias request")?;
        Ok((path, Bytes::from(body), insights))
    }
}

pub(super) fn alias_base_url(
    base: &str,
    full: bool,
    path: &str,
    insights: &ProxyRequestInsights,
) -> Result<String, String> {
    if let Some(model) = insights.upstream_model.as_deref().filter(|_| full) {
        if rewrite_gemini_path(path, model).is_some() {
            return rewrite_gemini_path(base, model).ok_or_else(|| {
                "Full URL must include a Gemini model endpoint to apply model aliases".into()
            });
        }
    }
    Ok(base.to_owned())
}

fn rewrite_gemini_path(path: &str, model: &str) -> Option<String> {
    let (prefix, suffix) = path.split_once("models/")?;
    let (endpoint, query) = suffix
        .split_once('?')
        .map_or((suffix, None), |(path, query)| (path, Some(query)));
    let (_, action_name) = endpoint.rsplit_once(':')?;
    if action_name != "generateContent" && action_name != "streamGenerateContent" {
        return None;
    }
    let path = format!("{prefix}models/{}:{action_name}", encode_component(model));
    Some(query.map_or_else(|| path.clone(), |query| format!("{path}?{query}")))
}

fn encode_component(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) {
            encoded.push(char::from(byte));
        } else {
            use std::fmt::Write;
            write!(&mut encoded, "%{byte:02X}").expect("writing a string cannot fail");
        }
    }
    encoded
}

#[cfg(test)]
#[path = "model_aliases_tests.rs"]
mod tests;
