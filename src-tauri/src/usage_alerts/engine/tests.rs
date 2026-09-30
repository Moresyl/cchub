use super::*;
use serde_json::json;

pub(crate) fn fixture() -> (StoredState, ConfigProfile) {
    let profile = ConfigProfile { id: "profile".into(), name: "Account".into(), tool_id: "claude".into(),
        config_snapshot: json!({"env":{"ANTHROPIC_BASE_URL":"https://example.com","ANTHROPIC_AUTH_TOKEN":"private-key"}}).to_string(),
        sort_order: 0, source_type: None, source_key: None, created_at: None, updated_at: None };
    let mut state = StoredState::default();
    super::super::storage::set_rule(
        &mut state,
        &profile,
        AlertSettings {
            enabled: true,
            quota_percent: Some(80.0),
            balances: vec![BalanceThreshold {
                unit: "USD".into(),
                amount: 5.0,
            }],
            system_notifications: true,
        },
    )
    .unwrap();
    (state, profile)
}

fn quota(percent: f64, reset: Option<i64>) -> Value {
    let mut row = json!({"planName":"five_hour", "utilization":percent});
    if let Some(reset) = reset {
        row["resetAt"] = json!(chrono::DateTime::from_timestamp(reset, 0)
            .unwrap()
            .to_rfc3339());
    }
    json!({"success":true,"data":[row]})
}

#[test]
fn known_window_does_not_realert_when_usage_fluctuates_or_reset_drifts() {
    let (mut state, profile) = fixture();
    assert_eq!(
        observe(&mut state, &profile, &quota(80.0, Some(2000)), 1000),
        1
    );
    for percent in [30.0, 81.0, 0.0, 100.0] {
        assert_eq!(
            observe(&mut state, &profile, &quota(percent, Some(2030)), 1100),
            0
        );
    }
    assert_eq!(observe(&mut state, &profile, &quota(90.0, None), 1200), 0);
    assert_eq!(
        observe(&mut state, &profile, &quota(90.0, Some(4000)), 2100),
        1
    );
    assert_eq!(state.events.len(), 2);
}

#[test]
fn unknown_window_uses_hysteresis_and_failed_or_stale_data_never_rearms() {
    let (mut state, profile) = fixture();
    assert_eq!(observe(&mut state, &profile, &quota(90.0, None), 1000), 1);
    assert_eq!(observe(&mut state, &profile, &quota(79.0, None), 1100), 0);
    let mut failed = quota(0.0, None);
    failed["success"] = json!(false);
    observe(&mut state, &profile, &failed, 1200);
    let mut stale = quota(0.0, None);
    stale["asOf"] = json!("previous");
    observe(&mut state, &profile, &stale, 1300);
    assert_eq!(observe(&mut state, &profile, &quota(90.0, None), 1400), 0);
    observe(&mut state, &profile, &quota(75.0, None), 1500);
    assert_eq!(observe(&mut state, &profile, &quota(80.0, None), 1600), 1);
}

#[test]
fn balance_units_do_not_mix_and_recharge_requires_hysteresis() {
    let (mut state, profile) = fixture();
    let balance = |amount, unit| json!({"success":true,"data":{"remaining":amount,"unit":unit}});
    assert_eq!(observe(&mut state, &profile, &balance(1.0, "CNY"), 1000), 0);
    assert_eq!(
        observe(&mut state, &profile, &balance(-1.0, "usd"), 1000),
        1
    );
    observe(&mut state, &profile, &balance(5.1, "USD"), 1100);
    assert_eq!(observe(&mut state, &profile, &balance(0.0, "USD"), 1200), 0);
    observe(&mut state, &profile, &balance(5.6, "USD"), 1300);
    assert_eq!(observe(&mut state, &profile, &balance(5.0, "USD"), 1400), 1);
}

#[test]
fn zero_balance_threshold_alerts_zero_and_rearms_only_after_positive_balance() {
    let (mut state, profile) = fixture();
    state.rules.get_mut(&profile.id).unwrap().settings.balances[0].amount = 0.0;
    let balance = |amount| json!({"success":true,"data":[{"remaining":amount,"unit":"USD"}]});
    assert_eq!(observe(&mut state, &profile, &balance(0.0), 1000), 1);
    assert_eq!(observe(&mut state, &profile, &balance(-1.0), 1100), 0);
    observe(&mut state, &profile, &balance(0.01), 1200);
    assert_eq!(observe(&mut state, &profile, &balance(0.0), 1300), 1);
}

#[test]
fn bad_reset_suppresses_quota_but_does_not_hide_valid_balance() {
    let (mut state, profile) = fixture();
    assert_eq!(
        observe(&mut state, &profile, &quota(100.0, Some(900)), 1000),
        0
    );
    assert_eq!(
        observe(
            &mut state,
            &profile,
            &json!({"success":true,"data":[{"resetAt":"invalid","utilization":95,"remaining":1,"unit":"USD"}]}),
            1000
        ),
        1
    );
    assert_eq!(state.events[0].event.kind, "balance");
}

#[test]
fn missing_nonfinite_and_ambiguous_rows_are_not_guessed() {
    let (mut state, profile) = fixture();
    for row in [
        json!({"used":100}),
        json!({"remaining":"NaN","unit":"USD"}),
        json!({"utilization":"Infinity"}),
        json!({"utilization":95,"isValid":false}),
    ] {
        assert_eq!(
            observe(
                &mut state,
                &profile,
                &json!({"success":true,"data":[row]}),
                1000
            ),
            0
        );
    }
    assert_eq!(
        observe(
            &mut state,
            &profile,
            &json!({"success":true,"data":[{"name":"quota","utilization":95},{"name":"quota","utilization":50}]}),
            1000
        ),
        0
    );
    assert_eq!(
        observe(
            &mut state,
            &profile,
            &json!({"success":true,"data":[{"name":"quota","used":"80","total":"100"}]}),
            1000
        ),
        1
    );
}

#[test]
fn full_pending_history_does_not_lose_events_or_suppress_future_alert() {
    let (mut state, profile) = fixture();
    observe(&mut state, &profile, &quota(95.0, None), 1000);
    let event = state.events[0].clone();
    state.events = vec![event; 200];
    state.marks.clear();
    assert_eq!(observe(&mut state, &profile, &quota(95.0, None), 1100), 0);
    state.events[0].event.system_status = "accepted".into();
    assert_eq!(observe(&mut state, &profile, &quota(95.0, None), 1200), 1);
    assert_eq!(state.events.len(), 200);
}
