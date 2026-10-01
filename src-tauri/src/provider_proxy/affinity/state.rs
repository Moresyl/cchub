use super::identity::Turn;
use super::Binding;
use crate::provider_proxy::routing::AffinityMode;
use std::collections::VecDeque;
use std::time::{Duration, Instant};
use uuid::Uuid;

const MAX_SESSIONS: usize = 4096;
const KEEP: Duration = Duration::from_secs(24 * 60 * 60);
const CACHE_KEEP: Duration = Duration::from_secs(5 * 60);

struct Entry {
    key: String,
    ticket: Uuid,
    touched: Instant,
    binding: Option<Binding>,
}

#[derive(Default)]
pub(in crate::provider_proxy) struct Store(VecDeque<Entry>);

impl Store {
    pub(super) fn begin(&mut self, key: String, now: Instant) -> (Uuid, Option<Binding>) {
        self.0
            .retain(|entry| now.saturating_duration_since(entry.touched) < KEEP);
        let old = self
            .0
            .iter()
            .position(|entry| entry.key == key)
            .and_then(|index| self.0.remove(index));
        let ticket = Uuid::new_v4();
        let binding = old
            .and_then(|entry| entry.binding)
            .filter(|binding| now.saturating_duration_since(binding.at) < KEEP);
        if self.0.len() >= MAX_SESSIONS {
            self.0.pop_front();
        }
        self.0.push_back(Entry {
            key,
            ticket,
            touched: now,
            binding: binding.clone(),
        });
        (ticket, binding)
    }

    pub(super) fn commit(&mut self, key: &str, ticket: Uuid, binding: Binding) {
        if let Some(entry) = self
            .0
            .iter_mut()
            .find(|entry| entry.key == key && entry.ticket == ticket)
        {
            entry.binding = Some(binding);
        }
    }

    pub(super) fn forget(&mut self, key: &str, ticket: Uuid) {
        if let Some(entry) = self
            .0
            .iter_mut()
            .find(|entry| entry.key == key && entry.ticket == ticket)
        {
            entry.binding = None;
        }
    }
}

pub(super) fn keep(mode: AffinityMode, binding: &Binding, turn: &Turn, now: Instant) -> bool {
    let age = now.saturating_duration_since(binding.at);
    if age >= KEEP {
        return false;
    }
    let within = turn.within && (turn.marker.is_none() || turn.marker == binding.turn.marker);
    match mode {
        AffinityMode::Off => false,
        AffinityMode::Session => true,
        AffinityMode::Turn => within,
        AffinityMode::Auto => within || (binding.cached >= 1024 && age < CACHE_KEEP),
    }
}

#[cfg(test)]
mod tests;
