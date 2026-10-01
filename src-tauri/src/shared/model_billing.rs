use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BillingKind {
    Free,
    Premium,
    #[default]
    Unknown,
}

/// Vendor-reported premium-request units, never a monetary token price.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ModelBilling {
    pub kind: BillingKind,
    pub multiplier: Option<f64>,
}

impl ModelBilling {
    pub(crate) fn from_value(value: &Value) -> Self {
        let premium = value.get("is_premium").and_then(Value::as_bool);
        let multiplier = value
            .get("multiplier")
            .and_then(|value| value.as_f64().or_else(|| value.as_str()?.parse().ok()))
            .filter(|number| number.is_finite() && *number >= 0.0);
        let kind = match (premium, multiplier) {
            (Some(_), Some(0.0)) => BillingKind::Free,
            (Some(true), Some(value)) if value > 0.0 => BillingKind::Premium,
            _ => return Self::default(),
        };
        Self { kind, multiplier }
    }

    pub(crate) fn premium(&self) -> bool {
        self.kind == BillingKind::Premium
    }

    pub(crate) fn agree(self, other: Self) -> Self {
        if self == other {
            self
        } else if self.kind == BillingKind::Premium && other.kind == BillingKind::Premium {
            Self {
                kind: BillingKind::Premium,
                multiplier: None,
            }
        } else {
            Self::default()
        }
    }
}

/// Billing disagreements, missing duplicate metadata and excessive catalogs
/// are unknown. This same interpretation is used by account UI and routing.
pub(crate) fn catalog(value: &Value) -> HashMap<String, ModelBilling> {
    let mut models = HashMap::<String, ModelBilling>::new();
    if let Some(entries) = value
        .get("data")
        .and_then(Value::as_array)
        .filter(|entries| entries.len() <= 512)
    {
        for entry in entries {
            let Some(id) = entry
                .get("id")
                .and_then(Value::as_str)
                .filter(|id| !id.trim().is_empty() && id.len() <= 1024)
            else {
                continue;
            };
            let billing = ModelBilling::from_value(entry.get("billing").unwrap_or(&Value::Null));
            models
                .entry(id.into())
                .and_modify(|previous| *previous = previous.agree(billing))
                .or_insert(billing);
        }
    }
    models
}

#[cfg(test)]
mod tests;
