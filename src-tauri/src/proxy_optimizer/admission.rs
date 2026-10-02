use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AdmissionConfig {
    pub max_concurrent: u32,
    pub max_queued: u32,
    pub queue_timeout_secs: u64,
    pub account_limits: BTreeMap<String, u32>,
    pub account_labels: BTreeMap<String, String>,
}

impl Default for AdmissionConfig {
    fn default() -> Self {
        Self {
            max_concurrent: 0,
            max_queued: 32,
            queue_timeout_secs: 30,
            account_limits: BTreeMap::new(),
            account_labels: BTreeMap::new(),
        }
    }
}

impl AdmissionConfig {
    pub fn validate(&self) -> Result<(), String> {
        if self.max_concurrent > 1000 || self.max_queued > 256 {
            return Err(
                "Concurrency must not exceed 1000; queue capacity must not exceed 256".into(),
            );
        }
        if !(1..=600).contains(&self.queue_timeout_secs) {
            return Err("Queue timeout must be between 1 and 600 seconds".into());
        }
        if self.account_limits.len() > 1024
            || self.account_limits.iter().any(|(key, limit)| {
                key.len() != 64
                    || !key
                        .bytes()
                        .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
                    || *limit > 1000
            })
        {
            return Err("Invalid account concurrency overrides".into());
        }
        if self.account_labels.len() > 1024
            || self.account_labels.iter().any(|(key, label)| {
                !self.account_limits.contains_key(key)
                    || label.chars().count() > 128
                    || label.chars().any(char::is_control)
            })
        {
            return Err("Invalid account concurrency labels".into());
        }
        Ok(())
    }

    pub(crate) fn limit(&self, key: &str) -> u32 {
        self.account_limits
            .get(key)
            .copied()
            .unwrap_or(self.max_concurrent)
    }
}
