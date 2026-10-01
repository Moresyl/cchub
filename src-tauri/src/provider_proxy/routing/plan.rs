use super::facts::Facts;
use super::*;
use crate::db::DbState;
use crate::provider_proxy::{LocalProviderProxyRuntime, ProfileCandidate};
use std::collections::{HashMap, HashSet, VecDeque};
use tauri::{AppHandle, Manager};

pub(super) fn resolve(
    document: &RoutingDocument,
    body: &[u8],
    path: &str,
    available: &[String],
    request_bytes: Option<u64>,
    offset: impl FnMut(&str, usize) -> usize,
) -> Result<RoutingPreview, String> {
    let facts = request_facts(document, body, path, request_bytes);
    resolve_facts(document, facts.as_ref(), available, offset)
}

fn request_facts(
    document: &RoutingDocument,
    body: &[u8],
    path: &str,
    request_bytes: Option<u64>,
) -> Option<Facts> {
    (document.policy.enabled && !document.policy.rules.is_empty())
        .then(|| Facts::from_body(body, path).with_size(request_bytes))
}

fn resolve_facts(
    document: &RoutingDocument,
    facts: Option<&Facts>,
    available: &[String],
    mut offset: impl FnMut(&str, usize) -> usize,
) -> Result<RoutingPreview, String> {
    let policy = &document.policy;
    let rule = facts.and_then(|facts| policy.rules.iter().find(|rule| facts.matches(rule)));
    let group = if policy.enabled {
        rule.map(|rule| rule.group_id.as_str())
            .or(policy.default_group_id.as_deref())
    } else {
        None
    };
    let mut ids = Vec::new();
    if let Some(id) = group {
        validation::validate(policy)?;
        let mut offsets = HashMap::new();
        expand(
            policy,
            id,
            &mut |group, size| {
                *offsets
                    .entry(group.to_string())
                    .or_insert_with(|| offset(group, size))
            },
            &mut ids,
        )?;
        let mut seen = HashSet::new();
        ids.retain(|id| seen.insert(id.clone()));
        if ids.len() > 256 {
            return Err("A routing plan exceeds 256 profiles".into());
        }
        if ids.iter().any(|id| !available.contains(id)) {
            return Err(
                "A selected routing group references a missing profile; update its members".into(),
            );
        }
    } else {
        ids.extend_from_slice(available);
    }
    Ok(RoutingPreview {
        group_id: group.map(str::to_string),
        rule_id: rule.map(|rule| rule.id.clone()),
        profile_ids: ids,
        reason: if rule.is_some() {
            "rule"
        } else if group.is_some() {
            "defaultGroup"
        } else {
            "activeProfile"
        }
        .into(),
    })
}

fn expand(
    policy: &RoutingPolicy,
    id: &str,
    offset: &mut impl FnMut(&str, usize) -> usize,
    out: &mut Vec<String>,
) -> Result<(), String> {
    let group = policy
        .groups
        .iter()
        .find(|group| group.id == id)
        .ok_or("A routing group no longer exists")?;
    if group.mode == RoutingMode::Manual {
        out.push(
            group
                .picked_profile_id
                .clone()
                .ok_or("Manual routing needs a selected profile")?,
        );
        return Ok(());
    }
    let mut members = group.members.clone();
    if group.mode == RoutingMode::RoundRobin && !members.is_empty() {
        let shift = offset(&group.id, members.len()) % members.len();
        members.rotate_left(shift);
    }
    for member in members {
        match member {
            RoutingMember::Profile { profile_id } => out.push(profile_id),
            RoutingMember::Group { group_id } => expand(policy, &group_id, offset, out)?,
        }
        if out.len() > 4096 {
            return Err("Routing expansion exceeds supported limits".into());
        }
    }
    Ok(())
}

// Bound counters across revisions and tools; previews do not consume a turn.
pub(super) fn rotation(
    counters: &mut VecDeque<(String, u64)>,
    key: String,
    size: usize,
    advance: bool,
) -> usize {
    let index = counters.iter().position(|(id, _)| *id == key);
    let current = index.map(|index| counters[index].1).unwrap_or(0);
    if advance {
        if let Some(index) = index {
            counters.remove(index);
        }
        if counters.len() >= 512 {
            counters.pop_front();
        }
        counters.push_back((key, current.wrapping_add(1)));
    }
    (current % size.max(1) as u64) as usize
}

pub(in crate::provider_proxy) fn apply<R: tauri::Runtime>(
    app: &AppHandle<R>,
    tool: &str,
    path: &str,
    body: &[u8],
    candidates: Vec<ProfileCandidate>,
) -> Result<
    (
        Vec<ProfileCandidate>,
        bool,
        Option<(RoutingDocument, Option<String>)>,
    ),
    String,
> {
    let document = {
        let db = app.state::<DbState>();
        let conn = db.0.lock().map_err(|_| "Database lock failed")?;
        storage::load(&conn, tool)?
    };
    if !document.policy.enabled {
        return Ok((candidates, false, None));
    }
    let available = candidates
        .iter()
        .map(|profile| profile.profile_id.clone())
        .collect::<Vec<_>>();
    // Incoming bodies can reach 64 MiB; parsing must not hold the runtime lock
    // used by circuit recovery and other concurrent requests.
    let facts = request_facts(&document, body, path, None);
    let state = app.state::<LocalProviderProxyRuntime>();
    let mut runtime = state.0.lock().map_err(|_| "Routing runtime lock failed")?;
    let plan = resolve_facts(&document, facts.as_ref(), &available, |group, size| {
        rotation(
            &mut runtime.routing_rotations,
            format!(
                "{tool}:{}:{group}",
                document.revision.as_deref().unwrap_or("")
            ),
            size,
            true,
        )
    })?;
    drop(runtime);
    let routed = plan.group_id.is_some();
    let mut candidates = candidates;
    let ordered = plan
        .profile_ids
        .into_iter()
        .filter_map(|id| {
            candidates
                .iter()
                .position(|profile| profile.profile_id == id)
                .map(|index| candidates.remove(index))
        })
        .collect();
    Ok((ordered, routed, Some((document, plan.group_id))))
}
