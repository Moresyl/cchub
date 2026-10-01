use super::*;
use std::collections::HashSet;

fn label(value: &str, max: usize) -> bool {
    !value.trim().is_empty() && value.chars().count() <= max && !value.chars().any(char::is_control)
}

pub(super) fn validate(policy: &RoutingPolicy) -> Result<(), String> {
    if policy.groups.len() > 64 || policy.rules.len() > 128 {
        return Err("Routing supports at most 64 groups and 128 rules".into());
    }
    let mut ids = HashSet::new();
    for group in &policy.groups {
        if !label(&group.id, 128) || !label(&group.name, 128) || !ids.insert(&group.id) {
            return Err("Routing groups need unique identifiers and nonempty names".into());
        }
        if group.members.is_empty() || group.members.len() > 128 {
            return Err("Each routing group needs between 1 and 128 members".into());
        }
        let mut members = HashSet::new();
        for member in &group.members {
            let (kind, id) = match member {
                RoutingMember::Profile { profile_id } => ("profile", profile_id),
                RoutingMember::Group { group_id } => ("group", group_id),
            };
            if !label(id, 128) || !members.insert((kind, id)) {
                return Err("Routing group members need unique nonempty identifiers".into());
            }
        }
        if group.mode == RoutingMode::Manual && !group.picked_profile_id.as_ref().is_some_and(|picked|
            group.members.iter().any(|member| matches!(member, RoutingMember::Profile {profile_id} if profile_id == picked))) {
            return Err("Manual routing requires a directly selected profile member".into());
        }
    }
    if policy
        .default_group_id
        .as_ref()
        .is_some_and(|id| !ids.contains(id))
    {
        return Err("The default routing group no longer exists".into());
    }
    let mut rule_ids = HashSet::new();
    for rule in &policy.rules {
        if !label(&rule.id, 128)
            || !label(&rule.name, 128)
            || !rule_ids.insert(&rule.id)
            || !ids.contains(&rule.group_id)
        {
            return Err(
                "Routing rules need unique identifiers, names and an existing target group".into(),
            );
        }
        if rule.model.chars().count() > 256
            || rule.model.chars().any(char::is_control)
            || rule.min_request_bytes > super::super::MAX_PROXY_BODY_BYTES as u64
        {
            return Err("Routing rule conditions exceed supported limits".into());
        }
        if rule.model.is_empty() && !rule.images && !rule.thinking && rule.min_request_bytes == 0 {
            return Err("Each routing rule needs at least one condition".into());
        }
    }
    let mut visits = 0;
    for group in &policy.groups {
        walk(policy, &group.id, &mut Vec::new(), &mut visits)?;
    }
    if policy.enabled
        && !policy.quota_aware
        && policy.default_group_id.is_none()
        && policy.rules.is_empty()
    {
        return Err(
            "Enabled routing needs a default group, a rule or quota-aware selection".into(),
        );
    }
    Ok(())
}

fn walk(
    policy: &RoutingPolicy,
    id: &str,
    path: &mut Vec<String>,
    visits: &mut usize,
) -> Result<(), String> {
    *visits += 1;
    if *visits > 4096 {
        return Err("Routing group expansion exceeds supported limits".into());
    }
    if path.iter().any(|item| item == id) || path.len() >= 8 {
        return Err("Routing groups contain a cycle or exceed eight nesting levels".into());
    }
    let group = policy
        .groups
        .iter()
        .find(|group| group.id == id)
        .ok_or("A nested routing group no longer exists")?;
    path.push(id.into());
    for member in &group.members {
        if let RoutingMember::Group { group_id } = member {
            walk(policy, group_id, path, visits)?;
        }
    }
    path.pop();
    Ok(())
}
