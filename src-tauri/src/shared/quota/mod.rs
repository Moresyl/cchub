mod parse;

use serde_json::Value;
use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;
use std::time::{Duration, Instant};

const MAX_ENTRIES: usize = 256;
const FRESHNESS: Duration = Duration::from_secs(300);

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Resource {
    CodexUsage,
    CopilotUsage,
    CopilotModels,
}

pub(crate) enum Snapshot {
    Codex(Option<i64>),
    Copilot {
        chat: Option<i64>,
        premium: Option<i64>,
    },
    Models(HashMap<String, bool>),
}

pub(crate) struct Query {
    ticket: uuid::Uuid,
}

struct Entry {
    account: String,
    revision: String,
    resource: Resource,
    ticket: uuid::Uuid,
    started: Instant,
    snapshot: Option<Snapshot>,
}

/// In-memory observations belong to one manager and one exact account login.
/// A newer query immediately makes older observations unknown. Slow responses
/// cannot overwrite newer results, and freshness includes time spent querying.
#[derive(Default)]
pub(crate) struct QuotaCache(Mutex<VecDeque<Entry>>);

impl QuotaCache {
    pub(crate) fn begin(&self, account: &str, revision: &str, resource: Resource) -> Query {
        let ticket = uuid::Uuid::new_v4();
        if let Ok(mut entries) = self.0.lock() {
            entries.retain(|entry| {
                entry.account != account || entry.revision != revision || entry.resource != resource
            });
            while entries.len() >= MAX_ENTRIES {
                entries.pop_front();
            }
            entries.push_back(Entry {
                account: account.into(),
                revision: revision.into(),
                resource,
                ticket,
                started: Instant::now(),
                snapshot: None,
            });
        }
        Query { ticket }
    }

    pub(crate) fn complete(&self, query: Query, value: Option<&Value>) {
        if let Ok(mut entries) = self.0.lock() {
            if let Some(entry) = entries
                .iter_mut()
                .find(|entry| entry.ticket == query.ticket)
            {
                entry.snapshot = value.map(|value| parse::snapshot(entry.resource, value));
            }
        }
    }

    pub(crate) fn blocked(
        &self,
        account: &str,
        revision: &str,
        model: Option<&str>,
    ) -> Option<u64> {
        self.blocked_at(
            account,
            revision,
            model,
            Instant::now(),
            chrono::Utc::now().timestamp(),
        )
    }

    fn blocked_at(
        &self,
        account: &str,
        revision: &str,
        model: Option<&str>,
        now: Instant,
        wall: i64,
    ) -> Option<u64> {
        let entries = self.0.lock().ok()?;
        let remaining = |entry: &Entry| {
            FRESHNESS
                .checked_sub(now.saturating_duration_since(entry.started))
                .filter(|duration| !duration.is_zero())
        };
        let model_entry = entries.iter().find(|entry| {
            entry.account == account
                && entry.revision == revision
                && entry.resource == Resource::CopilotModels
                && remaining(entry).is_some()
        });
        let premium_model = model_entry
            .and_then(|entry| match entry.snapshot.as_ref()? {
                Snapshot::Models(models) => models.get(model?).copied(),
                _ => None,
            })
            .unwrap_or(false);
        entries
            .iter()
            .filter(|entry| entry.account == account && entry.revision == revision)
            .filter_map(|entry| {
                let freshness = remaining(entry)?;
                let retry = |until: Option<i64>, freshness: Duration| {
                    until.filter(|until| *until > wall).map(|until| {
                        freshness
                            .as_secs()
                            .max(1)
                            .min(until.saturating_sub(wall) as u64)
                    })
                };
                match entry.snapshot.as_ref()? {
                    Snapshot::Codex(until) => retry(*until, freshness),
                    Snapshot::Copilot { chat, premium } => {
                        let premium_retry = if premium_model {
                            retry(*premium, freshness.min(remaining(model_entry?)?))
                        } else {
                            None
                        };
                        retry(*chat, freshness)
                            .into_iter()
                            .chain(premium_retry)
                            .max()
                    }
                    Snapshot::Models(_) => None,
                }
            })
            .max()
    }
}

#[cfg(test)]
mod tests;
