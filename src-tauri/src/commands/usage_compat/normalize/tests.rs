use super::*;

#[test]
fn keeps_rows_units_zero_balances_and_finite_strings() {
    let result = normalize_usage(
        "relay",
        &json!({"data": [
            {"name": "Wallet", "balance": " 0 ", "currency": "USD"},
            {"name": "Requests", "used": 2, "limit": 10, "unit": "requests"},
            {"balance": "NaN"}, [1, 2]
        ]}),
    );
    assert_eq!(result["data"].as_array().unwrap().len(), 2);
    assert_eq!(result["data"][0]["remaining"], 0.0);
    assert_eq!(result["data"][0]["unit"], "USD");
    assert_eq!(result["data"][1]["total"], 10.0);
}

#[test]
fn failures_and_unrecognized_payloads_never_have_successful_rows() {
    for payload in [
        json!({"success": false, "data": {"balance": 100}}),
        json!({"status": "error", "tiers": [{"utilization": 50}]}),
        json!({"data": {"unit": "USD"}}),
    ] {
        let result = normalize_usage("relay", &payload);
        assert_eq!(result["success"], false);
        assert_eq!(result["data"], json!([]));
    }
}

#[test]
fn quota_requires_a_real_percentage_or_a_complete_known_ratio() {
    let result = normalize_usage(
        "relay",
        &json!({"data": [
            {"name": "Five hours", "remaining": 25, "total": 100, "resetAt": "2027-01-15T08:00:00Z"},
            {"name": "Unknown", "total": 100}, {"name": "Empty", "used": 0, "total": 0},
            {"name": "Overflow", "total": 1e-300, "used": 1e300}
        ]}),
    );
    let quota = quota_from_usage("relay", &result);
    assert_eq!(quota["tiers"].as_array().unwrap().len(), 1);
    assert_eq!(quota["tiers"][0]["utilization"], 75.0);
    assert!(quota["tiers"][0].get("resetsAt").is_some());
    let balance_only = normalize_usage("relay", &json!({"balance": 3}));
    assert_eq!(
        quota_from_usage("relay", &balance_only)["status"],
        "not_found"
    );
}

#[test]
fn normalizes_native_quota_without_losing_window_identity() {
    let result = normalize_usage(
        "kimi",
        &json!({"status": "ok", "tiers": [{"name": "weekly_limit", "utilization": 42, "resetsAt": "2027-01-15T08:00:00Z"}]}),
    );
    assert_eq!(result["success"], true);
    assert_eq!(result["data"][0]["planName"], "weekly_limit");
    assert_eq!(result["data"][0]["utilization"], 42.0);
}

#[test]
fn distinct_metrics_in_the_same_window_survive_normalization() {
    let result = normalize_usage(
        "zhipu",
        &json!({"status": "ok", "tiers": [
            {"name": "five_hour", "metric": "tokens_limit", "utilization": 10},
            {"name": "five_hour", "metric": "credit_limit", "utilization": 20}
        ]}),
    );
    assert_eq!(result["data"][0]["metric"], "tokens_limit");
    let quota = quota_from_usage("zhipu", &result);
    assert_eq!(quota["tiers"][1]["metric"], "credit_limit");
}
#[test]
fn preserves_freshness_and_account_identity_and_drops_explicit_failed_rows() {
    let payload = serde_json::json!({"stale":true,"asOf":"previous","data":[
        {"remaining":1,"unit":"USD","accountId":"account","asOf":"old","stale":true},
        {"utilization":95,"success":false}
    ]});
    let result = super::normalize_usage("provider", &payload);
    assert_eq!(result["stale"], true);
    assert_eq!(result["asOf"], "previous");
    assert_eq!(result["data"].as_array().unwrap().len(), 1);
    assert_eq!(result["data"][0]["accountId"], "account");
    assert_eq!(result["data"][0]["asOf"], "old");
}
