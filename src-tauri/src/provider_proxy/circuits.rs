use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use uuid::Uuid;

use super::LocalProviderProxyRuntimeInner;
use crate::proxy_optimizer::OptimizerConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum CircuitState {
    #[default]
    Closed,
    Open,
    HalfOpen,
}

#[derive(Debug, Clone)]
pub(crate) struct EndpointCircuitState {
    pub(super) state: CircuitState,
    pub(super) consecutive_failures: u32,
    pub(super) consecutive_successes: u32,
    pub(super) open_until: Option<Instant>,
    half_open_permit_taken: bool,
    incarnation: Uuid,
    generation: u64,
}

impl Default for EndpointCircuitState {
    fn default() -> Self {
        Self {
            state: CircuitState::Closed,
            consecutive_failures: 0,
            consecutive_successes: 0,
            open_until: None,
            half_open_permit_taken: false,
            incarnation: Uuid::new_v4(),
            generation: 0,
        }
    }
}

#[derive(Clone, Copy)]
struct Permit {
    incarnation: Uuid,
    generation: u64,
    probe: bool,
}

impl EndpointCircuitState {
    // Candidate discovery never reserves the single recovery probe.
    pub(super) fn is_available(&self) -> bool {
        match self.state {
            CircuitState::Closed => true,
            CircuitState::Open => self.open_until.is_some_and(|until| Instant::now() >= until),
            CircuitState::HalfOpen => !self.half_open_permit_taken,
        }
    }

    fn acquire(&mut self) -> Option<Permit> {
        if !self.is_available() {
            return None;
        }
        if self.state == CircuitState::Open {
            self.state = CircuitState::HalfOpen;
            self.generation = self.generation.wrapping_add(1);
            self.consecutive_successes = 0;
        }
        let probe = self.state == CircuitState::HalfOpen;
        if probe {
            self.half_open_permit_taken = true;
        }
        Some(Permit {
            incarnation: self.incarnation,
            generation: self.generation,
            probe,
        })
    }

    fn finish(&mut self, permit: Permit, outcome: Option<bool>, config: &OptimizerConfig) -> bool {
        // A reset or a newer circuit transition invalidates older in-flight completions.
        if self.incarnation != permit.incarnation || self.generation != permit.generation {
            return false;
        }
        if permit.probe {
            self.half_open_permit_taken = false;
        }
        match outcome {
            Some(true) => {
                self.consecutive_failures = 0;
                if permit.probe {
                    self.consecutive_successes = self.consecutive_successes.saturating_add(1);
                    if self.consecutive_successes >= config.circuit_success_threshold.max(1) {
                        self.state = CircuitState::Closed;
                        self.open_until = None;
                        self.generation = self.generation.wrapping_add(1);
                    }
                }
            }
            Some(false) => {
                self.consecutive_successes = 0;
                self.consecutive_failures = self.consecutive_failures.saturating_add(1);
                if permit.probe
                    || self.consecutive_failures >= config.circuit_failure_threshold.max(1)
                {
                    self.state = CircuitState::Open;
                    self.open_until =
                        Some(Instant::now() + Duration::from_secs(config.circuit_timeout_secs));
                    self.consecutive_failures = 0;
                    self.generation = self.generation.wrapping_add(1);
                }
            }
            None => {}
        }
        true
    }
}

#[derive(Clone, Copy)]
pub(super) enum CircuitScope {
    Profile,
    Endpoint,
}

pub(super) fn profile_available(
    runtime: &Arc<Mutex<LocalProviderProxyRuntimeInner>>,
    key: &str,
) -> bool {
    runtime.lock().ok().is_some_and(|inner| {
        inner
            .profile_circuits
            .get(key)
            .is_none_or(EndpointCircuitState::is_available)
    })
}

pub(super) fn retry_after_seconds(
    runtime: &Arc<Mutex<LocalProviderProxyRuntimeInner>>,
    tool_id: &str,
    profile_ids: &[String],
) -> u64 {
    let Ok(inner) = runtime.lock() else {
        return 1;
    };
    let now = Instant::now();
    inner
        .profile_circuits
        .iter()
        .filter(|(key, _)| {
            profile_ids
                .iter()
                .any(|id| **key == format!("{tool_id}::{id}"))
        })
        .chain(inner.endpoint_circuits.iter().filter(|(key, _)| {
            profile_ids
                .iter()
                .any(|id| key.starts_with(&format!("{id}::")))
        }))
        .filter(|(_, state)| !state.is_available())
        .map(|(_, state)| {
            state
                .open_until
                .map(|until| {
                    until
                        .saturating_duration_since(now)
                        .as_secs()
                        .saturating_add(1)
                })
                .unwrap_or(1)
                .max(1)
        })
        .min()
        .unwrap_or(1)
}

pub(super) struct CircuitLease {
    runtime: Arc<Mutex<LocalProviderProxyRuntimeInner>>,
    scope: CircuitScope,
    key: String,
    permit: Option<Permit>,
    config: OptimizerConfig,
}

impl CircuitLease {
    pub(super) fn acquire(
        runtime: Arc<Mutex<LocalProviderProxyRuntimeInner>>,
        scope: CircuitScope,
        key: String,
        config: &OptimizerConfig,
    ) -> Option<Self> {
        let permit = {
            let mut inner = runtime.lock().ok()?;
            let states = match scope {
                CircuitScope::Profile => &mut inner.profile_circuits,
                CircuitScope::Endpoint => &mut inner.endpoint_circuits,
            };
            states.entry(key.clone()).or_default().acquire()?
        };
        Some(Self {
            runtime,
            scope,
            key,
            permit: Some(permit),
            config: config.clone(),
        })
    }

    pub(super) fn success(&mut self) -> bool {
        self.finish(Some(true))
    }
    pub(super) fn failure(&mut self) {
        self.finish(Some(false));
    }

    fn finish(&mut self, outcome: Option<bool>) -> bool {
        let Some(permit) = self.permit.take() else {
            return false;
        };
        let Ok(mut inner) = self.runtime.lock() else {
            return false;
        };
        let states = match self.scope {
            CircuitScope::Profile => &mut inner.profile_circuits,
            CircuitScope::Endpoint => &mut inner.endpoint_circuits,
        };
        if let Some(state) = states.get_mut(&self.key) {
            let previous = state.state;
            let accepted = state.finish(permit, outcome, &self.config);
            if previous != CircuitState::Open && state.state == CircuitState::Open {
                crate::utils::append_runtime_log(
                    "warn",
                    "provider_proxy",
                    &format!(
                        "Proxy circuit opened {} for {}s",
                        self.key, self.config.circuit_timeout_secs
                    ),
                );
            }
            return accepted;
        }
        false
    }
}

impl Drop for CircuitLease {
    fn drop(&mut self) {
        self.finish(None);
    }
}

// Ownership follows the response body, including bodies dropped before their first poll.
pub(super) fn track_body(
    body: axum::body::Body,
    mut profile: CircuitLease,
    mut endpoint: CircuitLease,
    successful_status: bool,
    health: super::forward::streaming_health::StreamHealth,
    on_success: impl FnOnce() + Send + 'static,
) -> axum::body::Body {
    use futures_util::StreamExt;
    axum::body::Body::from_stream(async_stream::stream! {
        let stream = body.into_data_stream();
        tokio::pin!(stream);
        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(bytes) => {
                    if health.failed() {
                        endpoint.failure();
                        profile.failure();
                    }
                    yield Ok(bytes);
                }
                Err(error) => {
                    endpoint.failure();
                    profile.failure();
                    yield Err(error);
                    return;
                }
            }
        }
        if health.failed() {
            endpoint.failure();
            profile.failure();
        } else if successful_status && health.verified() {
            let endpoint_accepted = endpoint.success();
            let profile_accepted = profile.success();
            if endpoint_accepted && profile_accepted { on_success(); }
        }
    })
}

#[cfg(test)]
#[path = "circuits_tests.rs"]
mod tests;
