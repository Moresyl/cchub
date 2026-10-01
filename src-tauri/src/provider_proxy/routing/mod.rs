mod facts;
mod plan;
mod storage;
mod validation;

use serde::{Deserialize, Serialize};

pub(super) use plan::apply;
pub(crate) use storage::{load, preview, save};

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct RoutingPolicy {
    pub enabled: bool,
    pub quota_aware: bool,
    pub affinity: AffinityMode,
    pub default_group_id: Option<String>,
    pub groups: Vec<RoutingGroup>,
    pub rules: Vec<RoutingRule>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum AffinityMode {
    #[default]
    Off,
    Auto,
    Session,
    Turn,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RoutingGroup {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub mode: RoutingMode,
    pub members: Vec<RoutingMember>,
    #[serde(default)]
    pub picked_profile_id: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum RoutingMode {
    #[default]
    Ordered,
    RoundRobin,
    Manual,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum RoutingMember {
    Profile {
        #[serde(rename = "profileId")]
        profile_id: String,
    },
    Group {
        #[serde(rename = "groupId")]
        group_id: String,
    },
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct RoutingRule {
    pub id: String,
    pub name: String,
    pub group_id: String,
    pub model: String,
    pub match_mode: ModelMatch,
    pub images: bool,
    pub thinking: bool,
    pub min_request_bytes: u64,
}

impl Default for RoutingRule {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            group_id: String::new(),
            model: String::new(),
            match_mode: ModelMatch::Exact,
            images: false,
            thinking: false,
            min_request_bytes: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ModelMatch {
    #[default]
    Exact,
    Prefix,
    Contains,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RoutingDocument {
    pub revision: Option<String>,
    pub policy: RoutingPolicy,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoutingPreview {
    pub group_id: Option<String>,
    pub rule_id: Option<String>,
    pub profile_ids: Vec<String>,
    pub reason: String,
}

#[cfg(test)]
mod tests;
