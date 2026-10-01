mod identity;
mod state;
pub(super) use state::Store;

use super::managed_auth::{AuthProvider, ManagedPrincipal};
use super::routing::{AffinityMode, RoutingDocument};
use super::{LocalProviderProxyRuntime, ProfileCandidate, ProxyUsageMetrics, UpstreamTarget};
use axum::http::{HeaderName, HeaderValue};
use identity::{hash, Turn};
use rusqlite::Connection;
use serde_json::{json, Value};
use std::time::Instant;
use tauri::{AppHandle, Manager};
use uuid::Uuid;

#[derive(Clone)]
struct Binding {
    profile: String,
    snapshot: String,
    principal: Option<ManagedPrincipal>,
    turn: Turn,
    at: Instant,
    cached: u64,
}

#[derive(Clone, Debug)]
pub(super) struct Attempt {
    key: String,
    ticket: Uuid,
    policy: String,
    snapshot: String,
    turn: Turn,
}

pub(super) struct Request {
    attempt: Attempt,
    pin: Option<Binding>,
}

fn policy_hash(document: &RoutingDocument) -> Option<String> {
    serde_json::to_vec(document).ok().map(|bytes| hash(&bytes))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn prepare<R: tauri::Runtime>(
    app: &AppHandle<R>,
    tool: &str,
    path: &str,
    headers: &[(HeaderName, HeaderValue)],
    body: &[u8],
    routing: Option<&(RoutingDocument, Option<String>)>,
    candidates: &mut [ProfileCandidate],
    optimizer: &crate::proxy_optimizer::OptimizerConfig,
) -> Option<Request> {
    let (document, group) = routing?;
    if !(path.ends_with("messages")
        || path.ends_with("responses")
        || path.ends_with("chat/completions")
        || path.contains(":generateContent")
        || path.contains(":streamGenerateContent"))
    {
        return None;
    }
    let mode = document.policy.affinity;
    if mode == AffinityMode::Off {
        return None;
    }
    let body: Value = serde_json::from_slice(body).ok()?;
    let policy = policy_hash(document)?;
    let optimizer = serde_json::to_vec(optimizer).ok()?;
    let scope = format!("{policy}:{}", hash(&optimizer));
    let key = identity::request_key(tool, path, &scope, group.as_deref(), headers, &body)?;
    let turn = identity::turn(&body);
    let state = app.state::<LocalProviderProxyRuntime>();
    let (ticket, old) = state
        .0
        .lock()
        .ok()?
        .affinity
        .begin(key.clone(), Instant::now());
    let mut pin = None;
    if let Some(binding) = old.filter(|binding| state::keep(mode, binding, &turn, Instant::now())) {
        let index = candidates.iter().position(|candidate| {
            candidate.profile_id == binding.profile
                && hash(candidate.snapshot.as_bytes()) == binding.snapshot
        });
        let current = binding
            .principal
            .as_ref()
            .is_none_or(|principal| principal.if_current(app, || {}));
        if let Some(index) = index.filter(|_| current) {
            candidates[..=index].rotate_right(1);
            pin = Some(binding);
        } else if let Ok(mut runtime) = state.0.lock() {
            runtime.affinity.forget(&key, ticket);
        }
    }
    Some(Request {
        attempt: Attempt {
            key,
            ticket,
            policy,
            snapshot: String::new(),
            turn,
        },
        pin,
    })
}

impl Request {
    pub(super) fn snapshot(&self, candidate: &ProfileCandidate) -> Result<String, String> {
        let Some(principal) = self
            .pin
            .as_ref()
            .filter(|pin| pin.profile == candidate.profile_id)
            .and_then(|pin| pin.principal.as_ref())
        else {
            return Ok(candidate.snapshot.clone());
        };
        let mut snapshot: Value = serde_json::from_str(&candidate.snapshot)
            .map_err(|_| "Invalid provider configuration")?;
        let provider = match principal.provider {
            AuthProvider::Codex => "codex_oauth",
            AuthProvider::Xai => "xai_oauth",
            AuthProvider::Copilot => "github_copilot",
        };
        if snapshot.get("metadata").is_none_or(|value| value.is_null()) {
            snapshot["metadata"] = json!({});
        }
        if !snapshot["metadata"].is_object() {
            return Err("Invalid provider metadata".into());
        }
        snapshot["metadata"]["authBinding"] =
            json!({"authProvider":provider,"accountId":principal.account_id});
        serde_json::to_string(&snapshot)
            .map_err(|_| "Could not prepare provider configuration".into())
    }

    pub(super) fn attach(
        &self,
        candidate: &ProfileCandidate,
        upstream: &mut UpstreamTarget,
    ) -> Result<(), String> {
        if let Some(principal) = self
            .pin
            .as_ref()
            .filter(|pin| pin.profile == candidate.profile_id)
            .and_then(|pin| pin.principal.as_ref())
        {
            if upstream.managed_principal.as_ref() != Some(principal) {
                return Err("The conversation's account changed while preparing the request; retry the request".into());
            }
        }
        let mut attempt = self.attempt.clone();
        attempt.snapshot = hash(candidate.snapshot.as_bytes());
        upstream.affinity = Some(attempt);
        Ok(())
    }
}

// Called only for a verified whole reply or a delivered stream completion. Hold
// the existing DB/account guards through the generation-checked runtime write.
pub(super) fn commit<R: tauri::Runtime>(
    app: &AppHandle<R>,
    conn: &Connection,
    tool: &str,
    upstream: &UpstreamTarget,
    usage: &ProxyUsageMetrics,
) {
    let Some(attempt) = &upstream.affinity else {
        return;
    };
    let current = super::routing::load(conn, tool)
        .ok()
        .and_then(|doc| policy_hash(&doc));
    if current.as_deref() != Some(&attempt.policy) {
        return;
    }
    let snapshot: Option<String> = conn
        .query_row(
            "SELECT config_snapshot FROM config_profiles WHERE id=?1 AND tool_id=?2",
            rusqlite::params![upstream.profile_id, tool],
            |row| row.get(0),
        )
        .ok();
    if snapshot
        .as_ref()
        .is_none_or(|snapshot| hash(snapshot.as_bytes()) != attempt.snapshot)
        || upstream.profile_id.len() > 1024
    {
        return;
    }
    let promote = || {
        let state = app.state::<LocalProviderProxyRuntime>();
        if let Ok(mut runtime) = state.0.lock() {
            runtime.affinity.commit(
                &attempt.key,
                attempt.ticket,
                Binding {
                    profile: upstream.profile_id.clone(),
                    snapshot: attempt.snapshot.clone(),
                    principal: upstream.managed_principal.clone(),
                    turn: attempt.turn.clone(),
                    at: Instant::now(),
                    cached: usage.cache_read_tokens,
                },
            );
        };
    };
    if let Some(principal) = &upstream.managed_principal {
        if principal.account_id.len() <= 4096 && principal.revision.len() <= 512 {
            principal.if_current(app, promote);
        }
    } else {
        promote();
    }
}
