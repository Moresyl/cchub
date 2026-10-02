use crate::proxy_optimizer::admission::AdmissionConfig;
use serde::Serialize;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::oneshot;
use tokio::time::Instant;

mod identity;
pub(super) use identity::{scope_key, uses_managed_identity};

const MAX_LANES: usize = 1024;
const MAX_ACTIVE: usize = 8192;
const MAX_WAITERS: usize = 4096;
const MAX_QUEUED_BYTES: usize = 256 * 1024 * 1024;
const MAX_RECENT: usize = 256;
const RECENT_TTL: Duration = Duration::from_secs(3600);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Rejection {
    QueueFull,
    TimedOut,
    Capacity,
    Unavailable,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub key: String,
    pub profiles: Vec<String>,
    pub active: usize,
    pub queued: usize,
    pub limit: u32,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    pub entries: Vec<Entry>,
    pub active: usize,
    pub queued: usize,
    pub queued_bytes: usize,
}

struct Waiter {
    bytes: usize,
    ticket: u64,
    deadline: Instant,
    sender: oneshot::Sender<Result<(), Rejection>>,
}

#[derive(Default)]
struct Lane {
    active: HashSet<u64>,
    waiting: VecDeque<Waiter>,
    profiles: Vec<String>,
}

#[derive(Default)]
struct Inner {
    recent: VecDeque<Recent>,
    config: AdmissionConfig,
    lanes: HashMap<String, Lane>,
    next_ticket: u64,
    active: usize,
    queued: usize,
    queued_bytes: usize,
}

struct Recent {
    key: String,
    profiles: Vec<String>,
    at: Instant,
}

#[derive(Clone, Default)]
pub(super) struct Store(Arc<Mutex<Inner>>);

impl Inner {
    fn remember(&mut self, key: &str, profile: &str) {
        let now = Instant::now();
        self.recent
            .retain(|entry| now.saturating_duration_since(entry.at) < RECENT_TTL);
        let mut entry = self
            .recent
            .iter()
            .position(|entry| entry.key == key)
            .and_then(|index| self.recent.remove(index))
            .unwrap_or_else(|| Recent {
                key: key.into(),
                profiles: Vec::new(),
                at: now,
            });
        if entry.profiles.len() < 16 && !entry.profiles.iter().any(|value| value == profile) {
            entry.profiles.push(profile.into());
        }
        entry.at = now;
        self.recent.push_back(entry);
        while self.recent.len() > MAX_RECENT {
            self.recent.pop_front();
        }
    }

    fn dispatch(&mut self) {
        let now = Instant::now();
        for (key, lane) in &mut self.lanes {
            let limit = self.config.limit(key) as usize;
            while let Some(front) = lane.waiting.front() {
                if front.sender.is_closed() || now >= front.deadline {
                    let waiter = lane.waiting.pop_front().unwrap();
                    self.queued -= 1;
                    self.queued_bytes -= waiter.bytes;
                    let _ = waiter.sender.send(Err(Rejection::TimedOut));
                    continue;
                }
                if self.active >= MAX_ACTIVE || (limit != 0 && lane.active.len() >= limit) {
                    break;
                }
                let waiter = lane.waiting.pop_front().unwrap();
                self.queued -= 1;
                self.queued_bytes -= waiter.bytes;
                lane.active.insert(waiter.ticket);
                self.active += 1;
                if waiter.sender.send(Ok(())).is_err() {
                    lane.active.remove(&waiter.ticket);
                    self.active -= 1;
                }
            }
        }
        self.lanes
            .retain(|_, lane| !lane.active.is_empty() || !lane.waiting.is_empty());
    }
}

impl Store {
    pub(super) fn configure(&self, config: AdmissionConfig) -> Result<(), Rejection> {
        config.validate().map_err(|_| Rejection::Unavailable)?;
        let mut inner = self.0.lock().map_err(|_| Rejection::Unavailable)?;
        inner.config = config;
        // Lowering a limit never cancels admitted work. Raising it wakes FIFO waiters.
        // Existing waiters retain their original deadline and capacity reservation.
        inner.dispatch();
        Ok(())
    }

    pub(super) fn stats(&self) -> Result<Stats, Rejection> {
        let mut inner = self.0.lock().map_err(|_| Rejection::Unavailable)?;
        let now = Instant::now();
        inner
            .recent
            .retain(|entry| now.saturating_duration_since(entry.at) < RECENT_TTL);
        let mut entries: Vec<_> = inner
            .lanes
            .iter()
            .map(|(key, lane)| Entry {
                key: key.clone(),
                profiles: lane.profiles.clone(),
                active: lane.active.len(),
                queued: lane.waiting.len(),
                limit: inner.config.limit(key),
            })
            .collect();
        let now = Instant::now();
        for recent in &inner.recent {
            if now.saturating_duration_since(recent.at) < RECENT_TTL
                && !inner.lanes.contains_key(&recent.key)
            {
                entries.push(Entry {
                    key: recent.key.clone(),
                    profiles: recent.profiles.clone(),
                    active: 0,
                    queued: 0,
                    limit: inner.config.limit(&recent.key),
                });
            }
        }
        entries.sort_by(|a, b| a.key.cmp(&b.key));
        Ok(Stats {
            entries,
            active: inner.active,
            queued: inner.queued,
            queued_bytes: inner.queued_bytes,
        })
    }

    #[cfg(test)]
    pub(super) async fn acquire(&self, key: String, profile: &str) -> Result<Permit, Rejection> {
        self.acquire_with_bytes(key, profile, 0).await
    }

    pub(super) async fn acquire_with_bytes(
        &self,
        key: String,
        profile: &str,
        bytes: usize,
    ) -> Result<Permit, Rejection> {
        let (owner, receiver, deadline) = {
            let mut inner = self.0.lock().map_err(|_| Rejection::Unavailable)?;
            inner.dispatch();
            if !inner.lanes.contains_key(&key) && inner.lanes.len() >= MAX_LANES {
                return Err(Rejection::Capacity);
            }
            let limit = inner.config.limit(&key) as usize;
            let available = inner.active < MAX_ACTIVE
                && inner.lanes.get(&key).is_none_or(|lane| {
                    lane.waiting.is_empty() && (limit == 0 || lane.active.len() < limit)
                });
            if !available
                && (inner.queued >= MAX_WAITERS
                    || bytes > MAX_QUEUED_BYTES.saturating_sub(inner.queued_bytes)
                    || inner.lanes.get(&key).map_or(0, |lane| lane.waiting.len())
                        >= inner.config.max_queued as usize)
            {
                return Err(Rejection::QueueFull);
            }
            // No wrapped ticket may accidentally release an older owner's slot.
            inner.next_ticket = inner
                .next_ticket
                .checked_add(1)
                .ok_or(Rejection::Capacity)?;
            let ticket = inner.next_ticket;
            let deadline = Instant::now() + Duration::from_secs(inner.config.queue_timeout_secs);
            let profile: String = profile
                .chars()
                .filter(|value| !value.is_control())
                .take(128)
                .collect();
            inner.remember(&key, &profile);
            let lane = inner.lanes.entry(key.clone()).or_default();
            if lane.profiles.len() < 16 && !lane.profiles.contains(&profile) {
                lane.profiles.push(profile);
            }
            let owner = Permit {
                store: self.clone(),
                key,
                ticket,
            };
            if available {
                lane.active.insert(ticket);
                inner.active += 1;
                return Ok(owner);
            }
            let (sender, receiver) = oneshot::channel();
            lane.waiting.push_back(Waiter {
                bytes,
                ticket,
                deadline,
                sender,
            });
            inner.queued += 1;
            inner.queued_bytes += bytes;
            (owner, receiver, deadline)
        };
        // The owner exists before the first await. Cancellation, a raced grant,
        // receiver errors and deadlines all use the same idempotent release path.
        tokio::select! {
            biased;
            _ = tokio::time::sleep_until(deadline) => Err(Rejection::TimedOut),
            result = receiver => {
                result.map_err(|_| Rejection::Unavailable)??;
                Ok(owner)
            }
        }
    }
}

pub(super) struct Permit {
    store: Store,
    key: String,
    ticket: u64,
}

impl Drop for Permit {
    fn drop(&mut self) {
        let Ok(mut inner) = self.store.0.lock() else {
            return;
        };
        if let Some(lane) = inner.lanes.get_mut(&self.key) {
            if lane.active.remove(&self.ticket) {
                inner.active -= 1;
            } else if let Some(index) = lane
                .waiting
                .iter()
                .position(|waiter| waiter.ticket == self.ticket)
            {
                let waiter = lane.waiting.remove(index).unwrap();
                inner.queued -= 1;
                inner.queued_bytes -= waiter.bytes;
            }
        }
        // Reconsider every lane because the global active cap may have blocked another account.
        inner.dispatch();
    }
}

pub(super) fn track_body(body: axum::body::Body, permit: Permit) -> axum::body::Body {
    use futures_util::StreamExt;
    // Ownership is captured before polling, including for bodies never read by a client.
    let owner = (body, permit);
    axum::body::Body::from_stream(async_stream::stream! {
        let (body, _permit) = owner;
        let stream = body.into_data_stream();
        tokio::pin!(stream);
        while let Some(chunk) = stream.next().await { yield chunk; }
    })
}

#[cfg(test)]
mod tests;
