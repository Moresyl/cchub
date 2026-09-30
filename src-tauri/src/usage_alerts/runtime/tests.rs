use super::*;
use crate::usage_alerts::engine::tests::fixture;
use serde_json::json;

#[test]
fn old_query_cannot_overwrite_revised_rule_or_changed_account() {
    let (mut state, mut profile) = fixture();
    let expected = state.rules[&profile.id].clone();
    let payload = Ok(json!({"success":true,"data":[{"utilization":95}]}));
    state.rules.get_mut(&profile.id).unwrap().revision = "new".into();
    apply_result(&mut state, &profile, &expected, &payload, 1000);
    assert!(state.events.is_empty());
    state.rules.get_mut(&profile.id).unwrap().revision = expected.revision.clone();
    profile.config_snapshot = profile
        .config_snapshot
        .replace("private-key", "another-key");
    apply_result(&mut state, &profile, &expected, &payload, 1100);
    assert!(state.events.is_empty());
    assert_eq!(state.rules[&profile.id].checked_at, None);
}

#[test]
fn delivery_failures_retry_six_times_and_accepted_events_never_resubmit() {
    let (mut state, profile) = fixture();
    engine::observe(
        &mut state,
        &profile,
        &json!({"success":true,"data":[{"utilization":95}]}),
        1000,
    );
    for attempt in 1..=6 {
        assert!(submit_pending(&mut state, &[profile.clone()], |_| false));
        assert_eq!(state.events[0].attempts, attempt);
    }
    assert_eq!(state.events[0].event.system_status, "failed");
    assert!(!submit_pending(&mut state, &[profile.clone()], |_| panic!(
        "failed events need explicit retry"
    )));
    state.events[0].event.system_status = "pending".into();
    assert!(submit_pending(&mut state, &[profile.clone()], |_| true));
    assert_eq!(state.events[0].event.system_status, "accepted");
    assert!(!submit_pending(&mut state, &[profile], |_| panic!(
        "already accepted"
    )));
}

#[test]
fn removed_changed_or_disabled_account_cancels_pending_delivery() {
    for scenario in ["removed", "changed", "disabled"] {
        let (mut state, mut profile) = fixture();
        engine::observe(
            &mut state,
            &profile,
            &json!({"success":true,"data":[{"utilization":95}]}),
            1000,
        );
        if scenario == "changed" {
            profile.config_snapshot = profile.config_snapshot.replace("private-key", "new-key");
        }
        if scenario == "disabled" {
            state.rules.get_mut(&profile.id).unwrap().settings.enabled = false;
        }
        let profiles = if scenario == "removed" {
            vec![]
        } else {
            vec![profile]
        };
        assert!(submit_pending(&mut state, &profiles, |_| panic!(
            "must not submit"
        )));
        assert_eq!(state.events[0].event.system_status, "cancelled");
    }
}
