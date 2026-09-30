use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BalanceThreshold {
    pub unit: String,
    pub amount: f64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AlertSettings {
    pub enabled: bool,
    pub quota_percent: Option<f64>,
    pub balances: Vec<BalanceThreshold>,
    pub system_notifications: bool,
}

impl AlertSettings {
    pub fn validate(&mut self) -> Result<(), String> {
        if self
            .quota_percent
            .is_some_and(|value| !value.is_finite() || !(1.0..=100.0).contains(&value))
        {
            return Err("Quota threshold must be between 1 and 100 percent".into());
        }
        if self.balances.len() > 10 {
            return Err("At most ten balance thresholds are allowed".into());
        }
        let mut units = std::collections::HashSet::new();
        for balance in &mut self.balances {
            balance.unit = balance.unit.trim().to_string();
            if balance.unit.is_empty()
                || balance.unit.len() > 32
                || balance.unit.chars().any(char::is_control)
                || !balance.amount.is_finite()
                || balance.amount < 0.0
                || !units.insert(balance.unit.to_ascii_lowercase())
            {
                return Err(
                    "Balance thresholds need unique units and finite nonnegative amounts".into(),
                );
            }
        }
        if self.enabled && self.quota_percent.is_none() && self.balances.is_empty() {
            return Err("Enable at least one quota or balance threshold".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Rule {
    pub settings: AlertSettings,
    pub identity: String,
    pub revision: String,
    pub checked_at: Option<i64>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Mark {
    pub reset_at: Option<i64>,
    pub notified: bool,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AlertEvent {
    pub id: String,
    pub profile_id: String,
    pub profile_name: String,
    pub tool_id: String,
    pub kind: String,
    pub label: String,
    pub value: f64,
    pub threshold: f64,
    pub unit: Option<String>,
    pub reset_at: Option<i64>,
    pub created_at: i64,
    pub read: bool,
    pub system_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StoredEvent {
    pub event: AlertEvent,
    pub identity: String,
    pub attempts: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StoredState {
    pub version: u32,
    pub rules: BTreeMap<String, Rule>,
    pub marks: BTreeMap<String, BTreeMap<String, Mark>>,
    pub events: Vec<StoredEvent>,
}

impl Default for StoredState {
    fn default() -> Self {
        Self {
            version: 1,
            rules: BTreeMap::new(),
            marks: BTreeMap::new(),
            events: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleView {
    pub profile_id: String,
    pub query_identity: Option<String>,
    pub settings: AlertSettings,
    pub paused: bool,
    pub status: String,
    pub checked_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AlertOverview {
    pub rules: Vec<RuleView>,
    pub events: Vec<AlertEvent>,
    pub polling: bool,
}
