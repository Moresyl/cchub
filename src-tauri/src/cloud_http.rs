//! Account-scoped throttling shared by manual and automatic cloud operations.
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime};

use reqwest::header::{HeaderMap, DATE, RETRY_AFTER};
use reqwest::{RequestBuilder, Response, StatusCode};

const FIRST_RETRY: Duration = Duration::from_secs(30 * 60);
const MAX_BACKOFF: Duration = Duration::from_secs(2 * 60 * 60);
const MAX_RETRY_AFTER: Duration = Duration::from_secs(6 * 60 * 60);
const MAX_ACCOUNTS: usize = 128;

#[derive(Clone, Copy)]
struct Cooldown {
    until: Instant,
    fallback: Duration,
}

#[derive(Default)]
struct Registry {
    accounts: HashMap<String, Cooldown>,
}

impl Registry {
    fn remaining(&self, scope: &str, now: Instant) -> Option<Duration> {
        self.accounts
            .get(scope)
            .and_then(|state| state.until.checked_duration_since(now))
            .filter(|wait| !wait.is_zero())
    }

    fn record(&mut self, scope: &str, headers: &HeaderMap, now: Instant) -> Duration {
        // Discard old failure history, and bound memory when connection tests
        // are made against many accounts. Keys are credential-scope hashes.
        self.accounts
            .retain(|_, state| now.saturating_duration_since(state.until) < MAX_RETRY_AFTER);
        let fallback = self
            .accounts
            .get(scope)
            .map(|state| state.fallback.saturating_mul(2).min(MAX_BACKOFF))
            .unwrap_or(FIRST_RETRY);
        let delay = retry_after(headers, SystemTime::now()).unwrap_or(fallback);
        if self.accounts.len() >= MAX_ACCOUNTS && !self.accounts.contains_key(scope) {
            if let Some(oldest) = self
                .accounts
                .iter()
                .min_by_key(|(_, state)| state.until)
                .map(|(key, _)| key.clone())
            {
                self.accounts.remove(&oldest);
            }
        }
        let until = now + delay;
        // An overlapping request must never shorten an existing server wait.
        let until = self
            .accounts
            .get(scope)
            .map(|state| state.until.max(until))
            .unwrap_or(until);
        self.accounts
            .insert(scope.to_string(), Cooldown { until, fallback });
        until.saturating_duration_since(now)
    }

    fn complete(&mut self, scope: &str, now: Instant) {
        // Success from an older in-flight operation cannot erase a newer limit.
        if self.remaining(scope, now).is_none() {
            self.accounts.remove(scope);
        }
    }
}

fn registry() -> &'static Mutex<Registry> {
    static STATE: OnceLock<Mutex<Registry>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(Registry::default()))
}

fn retry_after(headers: &HeaderMap, now: SystemTime) -> Option<Duration> {
    let value = headers.get(RETRY_AFTER)?.to_str().ok()?.trim();
    let delay = if value.bytes().all(|byte| byte.is_ascii_digit()) {
        Duration::from_secs(value.parse::<u64>().ok()?)
    } else {
        // Server Date avoids extending/reducing a wait due to local clock skew.
        let server_now = headers
            .get(DATE)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| httpdate::parse_http_date(value).ok())
            .unwrap_or(now);
        httpdate::parse_http_date(value)
            .ok()?
            .duration_since(server_now)
            .unwrap_or_default()
    };
    Some(delay.clamp(Duration::from_secs(1), MAX_RETRY_AFTER))
}

pub(crate) fn remaining(scope: &str) -> Option<Duration> {
    registry()
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .remaining(scope, Instant::now())
}

pub(crate) fn complete(scope: &str) {
    registry()
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .complete(scope, Instant::now());
}

fn wait_message(delay: Duration) -> String {
    let seconds = delay.as_secs() + u64::from(delay.subsec_nanos() != 0);
    let wait = if seconds >= 60 {
        format!("{} 分钟", seconds.div_ceil(60))
    } else {
        format!("{seconds} 秒")
    };
    format!("云存储繁忙或请求过于频繁，请在 {wait}后重试")
}

pub(crate) async fn send(request: RequestBuilder, scope: &str) -> Result<Response, String> {
    if let Some(delay) = remaining(scope) {
        return Err(wait_message(delay));
    }
    // Do not replay requests: conditional writes must be freshly authorized
    // by the next workflow, and uploads may have already reached the server.
    let response = request
        .send()
        .await
        .map_err(|_| "云存储请求失败，请检查连接后重试".to_string())?;
    if matches!(
        response.status(),
        StatusCode::TOO_MANY_REQUESTS | StatusCode::SERVICE_UNAVAILABLE
    ) {
        let delay = registry()
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .record(scope, response.headers(), Instant::now());
        return Err(format!(
            "{}（HTTP {}）",
            wait_message(delay),
            response.status().as_u16()
        ));
    }
    Ok(response)
}

#[cfg(test)]
mod tests;
