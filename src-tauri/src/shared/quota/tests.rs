use super::*;
use serde_json::json;

fn record(cache: &QuotaCache, resource: Resource, value: Value) {
    let query = cache.begin("one", "login", resource);
    cache.complete(query, Some(&value));
}
fn exhausted() -> Value {
    json!({"rate_limit":{"primary_window":{"used_percent":100,"reset_at":chrono::Utc::now().timestamp()+3600}}})
}
fn premium() -> Value {
    json!({"quota_snapshots":{"premium_interactions":{"entitlement":300,"quota_remaining":0}}})
}
fn models() -> Value {
    json!({"data":[
        {"id":"paid","billing":{"is_premium":true,"multiplier":0.33}},
        {"id":"free","billing":{"is_premium":false,"multiplier":0}},
        {"id":"unknown"}, {"id":"zero","billing":{"is_premium":true,"multiplier":0}}
    ]})
}

#[test]
fn quota_is_scoped_to_the_exact_login_and_fresh_query() {
    let cache = QuotaCache::default();
    record(&cache, Resource::CodexUsage, exhausted());
    assert!(cache.blocked("one", "login", None).is_some());
    for (account, revision) in [("two", "login"), ("one", "new-login")] {
        assert_eq!(cache.blocked(account, revision, None), None);
    }
    let now = Instant::now() + FRESHNESS;
    assert_eq!(
        cache.blocked_at("one", "login", None, now, chrono::Utc::now().timestamp()),
        None
    );
}

#[test]
fn newest_query_owns_success_failure_and_cancellation() {
    let cache = QuotaCache::default();
    let slow = cache.begin("one", "login", Resource::CodexUsage);
    let recent = cache.begin("one", "login", Resource::CodexUsage);
    cache.complete(recent, Some(&json!({"rate_limit":{"allowed":true}})));
    cache.complete(slow, Some(&exhausted()));
    assert_eq!(cache.blocked("one", "login", None), None);
    record(&cache, Resource::CodexUsage, exhausted());
    let _pending = cache.begin("one", "login", Resource::CodexUsage);
    assert_eq!(cache.blocked("one", "login", None), None);
    record(&cache, Resource::CodexUsage, exhausted());
    let failed = cache.begin("one", "login", Resource::CodexUsage);
    cache.complete(failed, None);
    assert_eq!(cache.blocked("one", "login", None), None);
}

#[test]
fn premium_exhaustion_uses_actual_model_billing_and_independent_freshness() {
    let cache = QuotaCache::default();
    record(&cache, Resource::CopilotUsage, premium());
    assert_eq!(cache.blocked("one", "login", Some("paid")), None);
    record(&cache, Resource::CopilotModels, models());
    assert!(cache.blocked("one", "login", Some("paid")).is_some());
    for model in [
        None,
        Some("free"),
        Some("unknown"),
        Some("zero"),
        Some("PAID"),
    ] {
        assert_eq!(cache.blocked("one", "login", model), None);
    }
    cache
        .0
        .lock()
        .unwrap()
        .iter_mut()
        .find(|entry| entry.resource == Resource::CopilotModels)
        .unwrap()
        .started -= FRESHNESS;
    assert_eq!(cache.blocked("one", "login", Some("paid")), None);
}

#[test]
fn chat_quota_does_not_depend_on_model_metadata_or_completion_quota() {
    let cache = QuotaCache::default();
    record(
        &cache,
        Resource::CopilotUsage,
        json!({"quota_snapshots":{"completions":{"entitlement":2000,"remaining":0}}}),
    );
    assert_eq!(cache.blocked("one", "login", None), None);
    record(
        &cache,
        Resource::CopilotUsage,
        json!({"quota_snapshots":{"chat":{"entitlement":50,"remaining":0}}}),
    );
    assert!(cache.blocked("one", "login", None).is_some());
}

#[test]
fn unlimited_overage_and_inconsistent_snapshots_remain_eligible() {
    let cache = QuotaCache::default();
    record(&cache, Resource::CopilotModels, models());
    for patch in [
        json!({"unlimited":true}),
        json!({"overage_permitted":true}),
        json!({"has_quota":false}),
        json!({"percent_remaining":50}),
        json!({"entitlement":0}),
        json!({"quota_remaining":0.01}),
        json!({"quota_remaining":"NaN"}),
    ] {
        let mut value = premium();
        let window = value["quota_snapshots"]["premium_interactions"]
            .as_object_mut()
            .unwrap();
        window.extend(patch.as_object().unwrap().clone());
        record(&cache, Resource::CopilotUsage, value);
        assert_eq!(cache.blocked("one", "login", Some("paid")), None, "{patch}");
    }
}

#[test]
fn reset_ends_a_block_before_the_cache_expires_and_bounds_retry_after() {
    let cache = QuotaCache::default();
    let wall = chrono::Utc::now().timestamp();
    record(
        &cache,
        Resource::CodexUsage,
        json!({"rate_limit":{"primary_window":{"used_percent":"100","reset_at":wall+10}}}),
    );
    assert_eq!(
        cache.blocked_at("one", "login", None, Instant::now(), wall + 3),
        Some(7)
    );
    assert_eq!(
        cache.blocked_at("one", "login", None, Instant::now(), wall + 10),
        None
    );
    record(
        &cache,
        Resource::CodexUsage,
        json!({"rate_limit":{"allowed":false,"limit_reached":true,
        "primary_window":{"used_percent":100,"reset_at":wall-1}}}),
    );
    assert_eq!(cache.blocked("one", "login", None), None);
}

#[test]
fn credits_and_explicit_permission_prevent_false_codex_exhaustion() {
    let cache = QuotaCache::default();
    for credits in [
        json!({"unlimited":true}),
        json!({"has_credits":true}),
        json!({}),
        json!({"unlimited":false,"has_credits":false,"balance":"1.5"}),
    ] {
        let mut value = exhausted();
        value["credits"] = credits.clone();
        record(&cache, Resource::CodexUsage, value);
        assert_eq!(cache.blocked("one", "login", None), None, "{credits}");
    }
    let mut value = exhausted();
    value["credits"] = json!({"unlimited":false,"has_credits":false,"balance":"0"});
    record(&cache, Resource::CodexUsage, value.clone());
    assert!(cache.blocked("one", "login", None).is_some());
    value["rate_limit"]["allowed"] = json!(true);
    record(&cache, Resource::CodexUsage, value);
    assert_eq!(cache.blocked("one", "login", None), None);
}

#[test]
fn malformed_unknown_and_model_specific_limits_are_not_global_exhaustion() {
    let cache = QuotaCache::default();
    for value in [
        json!(null),
        json!({}),
        json!({"additional_rate_limits":[{"model":"paid","limit_reached":true}]}),
        json!({"rate_limit":{"primary_window":{"used_percent":"Infinity","reset_at":i64::MAX}}}),
        json!({"rate_limit":{"primary_window":{"used_percent":100}}}),
    ] {
        record(&cache, Resource::CodexUsage, value);
        assert_eq!(cache.blocked("one", "login", None), None);
    }
}

#[test]
fn duplicate_billing_conflicts_and_failed_models_cannot_block() {
    let cache = QuotaCache::default();
    record(&cache, Resource::CopilotUsage, premium());
    record(
        &cache,
        Resource::CopilotModels,
        json!({"data":[
        {"id":"paid","billing":{"is_premium":true,"multiplier":1}}, {"id":"paid"}]}),
    );
    assert_eq!(cache.blocked("one", "login", Some("paid")), None);
    record(&cache, Resource::CopilotModels, models());
    let failed = cache.begin("one", "login", Resource::CopilotModels);
    cache.complete(failed, None);
    assert_eq!(cache.blocked("one", "login", Some("paid")), None);
}

#[test]
fn cache_is_bounded_and_eviction_never_restores_an_old_query() {
    let cache = QuotaCache::default();
    let slow = cache.begin("old", "login", Resource::CodexUsage);
    for index in 0..MAX_ENTRIES {
        cache.begin(&index.to_string(), "login", Resource::CodexUsage);
    }
    cache.complete(slow, Some(&exhausted()));
    assert_eq!(cache.0.lock().unwrap().len(), MAX_ENTRIES);
    assert_eq!(cache.blocked("old", "login", None), None);
}

#[test]
fn oversized_model_metadata_is_unknown_instead_of_trusting_a_partial_catalog() {
    let cache = QuotaCache::default();
    record(&cache, Resource::CopilotUsage, premium());
    let mut entries = vec![json!({"id":"paid","billing":{"is_premium":true,"multiplier":1}}); 512];
    entries.push(json!({"id":"paid","billing":{"is_premium":false,"multiplier":0}}));
    record(&cache, Resource::CopilotModels, json!({"data":entries}));
    assert_eq!(cache.blocked("one", "login", Some("paid")), None);
}

#[test]
fn multiple_quotas_keep_independent_reset_and_metadata_lifetimes() {
    let cache = QuotaCache::default();
    let wall = chrono::Utc::now().timestamp();
    record(&cache, Resource::CopilotModels, models());
    record(
        &cache,
        Resource::CopilotUsage,
        json!({"quota_reset_date_utc":chrono::DateTime::from_timestamp(wall+10,0).unwrap().to_rfc3339(),
        "quota_snapshots":{"chat":{"entitlement":50,"remaining":0},"premium_interactions":{"entitlement":300,"remaining":0}}}),
    );
    assert_eq!(
        cache.blocked_at("one", "login", Some("paid"), Instant::now(), wall + 10),
        None
    );
    record(
        &cache,
        Resource::CopilotUsage,
        json!({"quota_snapshots":{"chat":{"entitlement":50,"remaining":0}}}),
    );
    cache
        .0
        .lock()
        .unwrap()
        .iter_mut()
        .find(|entry| entry.resource == Resource::CopilotModels)
        .unwrap()
        .started -= FRESHNESS;
    assert!(cache.blocked("one", "login", Some("paid")).is_some());
    record(&cache, Resource::CopilotUsage, json!({}));
    record(
        &cache,
        Resource::CodexUsage,
        json!({"rate_limit":{"primary_window":{"used_percent":100,"reset_at":(wall+10).to_string()},
        "secondary_window":{"used_percent":100,"reset_at":(wall+20)*1000}}}),
    );
    assert_eq!(
        cache.blocked_at("one", "login", None, Instant::now(), wall + 10),
        Some(10)
    );
    assert_eq!(
        cache.blocked_at("one", "login", None, Instant::now(), wall + 20),
        None
    );
}

#[test]
fn malformed_reset_or_explicit_non_exhaustion_is_unknown_or_eligible() {
    let cache = QuotaCache::default();
    record(
        &cache,
        Resource::CopilotUsage,
        json!({"quota_reset_date":"not-a-date","quota_snapshots":{"chat":{"entitlement":50,"remaining":0}}}),
    );
    assert_eq!(cache.blocked("one", "login", None), None);
    let mut value = exhausted();
    value["rate_limit"]["limit_reached"] = json!(false);
    record(&cache, Resource::CodexUsage, value);
    assert_eq!(cache.blocked("one", "login", None), None);
}
