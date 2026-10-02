use axum::http::StatusCode;
use bytes::Bytes;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::{LocalProviderProxyRuntimeInner, UpstreamTarget};

mod messages;
#[cfg(test)]
mod tests;

const TTL: Duration = Duration::from_secs(600);
const MAX_POLICIES: usize = 128;
const FIELDS: [&str; 3] = ["reasoning_content", "reasoning", "reasoning_details"];

pub(super) fn is_chat_reply(value: &serde_json::Value) -> bool {
    value
        .get("choices")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|choices| {
            !choices.is_empty()
                && choices.iter().all(|choice| {
                    choice.get("message").is_some_and(|message| {
                        message.get("role").and_then(serde_json::Value::as_str) == Some("assistant")
                    }) && choice
                        .get("finish_reason")
                        .and_then(serde_json::Value::as_str)
                        .is_some_and(|reason| !reason.is_empty())
                })
        })
}

struct Policy {
    fields: u8,
    expires: Instant,
}

#[derive(Default)]
pub(super) struct Store(HashMap<String, Policy>);

impl Store {
    fn get(&mut self, key: &str, now: Instant) -> u8 {
        self.0.retain(|_, policy| policy.expires > now);
        self.0.get(key).map_or(0, |policy| policy.fields)
    }

    fn learn(&mut self, key: String, fields: u8, now: Instant) {
        if fields == 0 {
            return;
        }
        let fields = fields | self.get(&key, now);
        if !self.0.contains_key(&key) && self.0.len() >= MAX_POLICIES {
            if let Some(oldest) = self
                .0
                .iter()
                .min_by_key(|(_, policy)| policy.expires)
                .map(|(key, _)| key.clone())
            {
                self.0.remove(&oldest);
            }
        }
        self.0.insert(
            key,
            Policy {
                fields,
                expires: now + TTL,
            },
        );
    }
}

#[derive(Clone)]
pub(super) struct Recovery {
    runtime: Arc<Mutex<LocalProviderProxyRuntimeInner>>,
    key: Option<String>,
    mistral: bool,
    cached: u8,
    learned: u8,
    retries: usize,
}

impl Recovery {
    pub(super) fn new(
        runtime: Arc<Mutex<LocalProviderProxyRuntimeInner>>,
        tool: &str,
        upstream: &UpstreamTarget,
        snapshot: &str,
        path: &str,
        url: &str,
        model: Option<&str>,
        post: bool,
    ) -> Self {
        let chat = post
            && matches!(
                path.trim_matches('/'),
                "chat/completions" | "v1/chat/completions"
            );
        let key = chat.then(|| {
            let mut digest = Sha256::new();
            let principal = upstream.managed_principal.as_ref();
            for part in [
                tool,
                &upstream.profile_id,
                snapshot,
                path,
                url,
                model.unwrap_or(""),
                principal.map_or("", |p| p.account_id.as_str()),
                principal.map_or("", |p| p.revision.as_str()),
            ] {
                digest.update((part.len() as u64).to_le_bytes());
                digest.update(part.as_bytes());
            }
            format!("{:x}", digest.finalize())
        });
        let cached = key.as_ref().map_or(0, |key| {
            runtime
                .lock()
                .map_or(0, |mut state| state.chat_history.get(key, Instant::now()))
        });
        let mistral = chat
            && reqwest::Url::parse(url)
                .ok()
                .is_some_and(|url| url.host_str() == Some("api.mistral.ai"));
        Self {
            runtime,
            key,
            mistral,
            cached,
            learned: 0,
            retries: 0,
        }
    }

    pub(super) fn prepare(&self, body: Bytes) -> Bytes {
        if self.key.is_none() {
            return body;
        }
        messages::edit(&body, self.mistral, self.cached).unwrap_or(body)
    }

    pub(super) fn retry(
        &mut self,
        status: StatusCode,
        error: &Bytes,
        body: &Bytes,
    ) -> Option<Bytes> {
        if !self.can_retry(status) {
            return None;
        }
        let fields = messages::refused(error, body) & !self.learned;
        let next = (fields != 0)
            .then(|| messages::edit(body, false, fields))
            .flatten()?;
        self.learned |= fields;
        self.retries += 1;
        Some(next)
    }

    pub(super) fn can_retry(&self, status: StatusCode) -> bool {
        self.key.is_some()
            && self.retries < FIELDS.len()
            && matches!(
                status,
                StatusCode::BAD_REQUEST | StatusCode::UNPROCESSABLE_ENTITY
            )
    }

    /// Commit only after a usable JSON reply or a completed stream. Failed
    /// repairs must not change how the next independent request is forwarded.
    pub(super) fn success(&self) {
        if let Some(key) = self.key.as_ref().filter(|_| self.learned != 0) {
            if let Ok(mut state) = self.runtime.lock() {
                state
                    .chat_history
                    .learn(key.clone(), self.learned, Instant::now());
            }
        }
    }
}
