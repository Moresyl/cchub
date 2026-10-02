use super::*;
use crate::cloud_revision::tests::{fixture, reply};
use reqwest::header::HeaderValue;

fn headers(after: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(RETRY_AFTER, HeaderValue::from_str(after).unwrap());
    headers
}

#[test]
fn numeric_retry_after_is_bounded_and_invalid_values_fall_back() {
    let now = SystemTime::now();
    for (value, seconds) in [("0", 1), ("10", 10), ("900", 900), ("86400", 21600)] {
        assert_eq!(
            retry_after(&headers(value), now),
            Some(Duration::from_secs(seconds))
        );
    }
    for value in ["", "-1", "1.5", "+900", "soon", "18446744073709551616"] {
        assert_eq!(retry_after(&headers(value), now), None, "{value}");
    }
    assert_eq!(retry_after(&HeaderMap::new(), now), None);
}

#[test]
fn http_dates_use_server_clock_and_support_old_http_formats() {
    let server = httpdate::parse_http_date("Sun, 06 Nov 1994 08:49:37 GMT").unwrap();
    let mut map = headers("Sun, 06 Nov 1994 09:49:37 GMT");
    map.insert(
        DATE,
        HeaderValue::from_static("Sun, 06 Nov 1994 08:49:37 GMT"),
    );
    // The current local clock is decades ahead; server-relative wait is still one hour.
    assert_eq!(
        retry_after(&map, SystemTime::now()),
        Some(Duration::from_secs(3600))
    );
    for value in ["Sunday, 06-Nov-94 09:49:37 GMT", "Sun Nov  6 09:49:37 1994"] {
        assert_eq!(
            retry_after(&headers(value), server),
            Some(Duration::from_secs(3600))
        );
    }
    assert_eq!(
        retry_after(&headers("Sun, 06 Nov 1994 08:49:36 GMT"), server),
        Some(Duration::from_secs(1))
    );
    map.insert(DATE, HeaderValue::from_static("invalid"));
    assert_eq!(retry_after(&map, server), Some(Duration::from_secs(3600)));
}

#[test]
fn backoff_increases_only_on_limits_and_success_resets_it() {
    let mut registry = Registry::default();
    let mut now = Instant::now();
    for seconds in [1800, 3600, 7200, 7200] {
        let delay = registry.record("account", &HeaderMap::new(), now);
        assert_eq!(delay, Duration::from_secs(seconds));
        assert_eq!(registry.remaining("account", now), Some(delay));
        assert!(registry.remaining("another-account", now).is_none());
        registry.complete("account", now);
        assert_eq!(registry.remaining("account", now), Some(delay));
        now += delay;
        assert!(registry.remaining("account", now).is_none());
    }
    registry.complete("account", now);
    assert_eq!(
        registry.record("account", &HeaderMap::new(), now),
        FIRST_RETRY
    );
}

#[test]
fn overlapping_limits_and_success_cannot_shorten_an_active_wait() {
    let mut registry = Registry::default();
    let now = Instant::now();
    registry.record("account", &headers("3600"), now);
    assert_eq!(
        registry.record("account", &headers("10"), now + Duration::from_secs(5)),
        Duration::from_secs(3595)
    );
    registry.complete("account", now + Duration::from_secs(6));
    assert_eq!(
        registry.remaining("account", now + Duration::from_secs(6)),
        Some(Duration::from_secs(3594))
    );
    registry.complete("other", now);
    assert_eq!(registry.accounts.len(), 1);
}

#[test]
fn failure_history_expires_and_connection_tests_cannot_grow_memory_without_bound() {
    let mut registry = Registry::default();
    let now = Instant::now();
    registry.record("account", &HeaderMap::new(), now);
    assert_eq!(
        registry.record(
            "account",
            &HeaderMap::new(),
            now + FIRST_RETRY + MAX_RETRY_AFTER
        ),
        FIRST_RETRY
    );
    for index in 0..MAX_ACCOUNTS + 20 {
        registry.record(&format!("scope-{index}"), &headers("20"), now);
    }
    assert_eq!(registry.accounts.len(), MAX_ACCOUNTS);
}

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap()
}

#[tokio::test]
async fn limit_blocks_repeated_requests_and_errors_do_not_expose_url_or_credentials() {
    for status in [429, 503] {
        let response = format!("HTTP/1.1 {status} Limited\r\nRetry-After: 120\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").into_bytes();
        let (url, worker) = fixture(vec![response]);
        let scope = format!("limit-{url}");
        let request = || {
            client()
                .get(format!("{url}/private?secret=hidden"))
                .basic_auth("private-user", Some("private-password"))
        };
        let error = send(request(), &scope).await.unwrap_err();
        assert!(error.contains(&format!("HTTP {status}")));
        assert!(error.contains("2 分钟"));
        assert!(!error.contains("private") && !error.contains("hidden") && !error.contains(&url));
        assert!(remaining(&scope).is_some());
        let error = send(request(), &scope).await.unwrap_err();
        assert!(error.contains("后重试"));
        assert!(!error.contains("请求失败"));
        assert_eq!(worker.join().unwrap().len(), 1);
    }
}

#[tokio::test]
async fn ordinary_errors_and_successes_are_not_treated_as_throttling() {
    for status in [200, 204, 404, 409, 412, 428, 400, 405, 501, 403] {
        let (url, worker) = fixture(vec![reply(status, None, b"")]);
        let response = send(client().get(&url), &url).await.unwrap();
        assert_eq!(response.status().as_u16(), status);
        assert!(remaining(&url).is_none());
        assert_eq!(worker.join().unwrap().len(), 1);
    }
}

#[tokio::test]
async fn network_failure_is_sanitized_and_does_not_create_a_server_cooldown() {
    let request = client().get("http://127.0.0.1:1/private?secret=hidden");
    let error = send(request, "network-failure-test").await.unwrap_err();
    assert_eq!(error, "云存储请求失败，请检查连接后重试");
    assert!(remaining("network-failure-test").is_none());
}

#[test]
fn wait_text_rounds_up_without_showing_zero_or_large_second_counts() {
    assert!(wait_message(Duration::from_millis(1)).contains("1 秒"));
    assert!(wait_message(Duration::from_secs(59)).contains("59 秒"));
    assert!(wait_message(Duration::from_millis(60001)).contains("2 分钟"));
    assert!(wait_message(MAX_RETRY_AFTER).contains("360 分钟"));
}

#[tokio::test]
async fn completed_workflow_resets_an_expired_limit_without_resetting_an_active_one() {
    let (url, worker) = fixture(vec![reply(503, None, b""), reply(200, None, b"")]);
    let scope = format!("recovery-{url}");
    assert!(send(client().get(&url), &scope).await.is_err());
    complete(&scope);
    assert!(remaining(&scope).is_some());
    // Advance just this test account's deadline instead of sleeping for 30 minutes.
    registry()
        .lock()
        .unwrap()
        .accounts
        .get_mut(&scope)
        .unwrap()
        .until = Instant::now() - Duration::from_secs(1);
    assert!(send(client().get(&url), &scope)
        .await
        .unwrap()
        .status()
        .is_success());
    complete(&scope);
    assert!(!registry().lock().unwrap().accounts.contains_key(&scope));
    assert_eq!(worker.join().unwrap().len(), 2);
}
