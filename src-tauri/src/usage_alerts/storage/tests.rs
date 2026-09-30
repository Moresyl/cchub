use super::*;
use crate::usage_alerts::engine::{observe, tests::fixture};
use serde_json::json;

fn database() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch("CREATE TABLE app_settings (key TEXT PRIMARY KEY, value TEXT NOT NULL)")
        .unwrap();
    conn
}

#[test]
fn durable_event_and_mark_survive_restart_without_secrets() {
    let conn = database();
    let (mut state, profile) = fixture();
    let payload = json!({"success":true,"data":[{"utilization":95}]});
    observe(&mut state, &profile, &payload, 1000);
    save(&conn, &state).unwrap();
    let mut loaded = load(&conn).unwrap();
    assert_eq!(loaded.events.len(), 1);
    assert_eq!(observe(&mut loaded, &profile, &payload, 1100), 0);
    let raw: String = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key=?1",
            [KEY],
            |row| row.get(0),
        )
        .unwrap();
    assert!(!raw.contains("private-key"));
    assert!(!raw.contains("example.com"));
}

#[test]
fn corrupt_or_future_data_is_preserved_instead_of_reset() {
    let conn = database();
    for raw in [
        "invalid",
        "{\"version\":2,\"rules\":{},\"marks\":{},\"events\":[]}",
    ] {
        conn.execute(
            "INSERT OR REPLACE INTO app_settings VALUES (?1,?2)",
            rusqlite::params![KEY, raw],
        )
        .unwrap();
        assert!(load(&conn).is_err());
        let actual: String = conn
            .query_row(
                "SELECT value FROM app_settings WHERE key=?1",
                [KEY],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(actual, raw);
    }
}

#[test]
fn account_changes_pause_and_explicit_save_rebinds_while_model_changes_do_not() {
    let (mut state, mut profile) = fixture();
    let original = state.rules[&profile.id].identity.clone();
    let mut snapshot: serde_json::Value = serde_json::from_str(&profile.config_snapshot).unwrap();
    snapshot["env"]["ANTHROPIC_MODEL"] = json!("another-model");
    profile.config_snapshot = snapshot.to_string();
    profile.name = "Renamed".into();
    assert_eq!(usage_identity(&profile).unwrap(), original);
    snapshot["env"]["ANTHROPIC_AUTH_TOKEN"] = json!("replacement");
    profile.config_snapshot = snapshot.to_string();
    assert!(overview(&state, &[profile.clone()], false).rules[0].paused);
    let settings = state.rules[&profile.id].settings.clone();
    set_rule(&mut state, &profile, settings).unwrap();
    assert!(!overview(&state, &[profile], false).rules[0].paused);
}

#[test]
fn effective_script_identity_tracks_script_and_overrides_only() {
    let (_, mut profile) = fixture();
    let mut value = json!({"env":{"ANTHROPIC_AUTH_TOKEN":"unused"},"metadata":{"usageScript":{"enabled":true,"code":"console.log('{}')","apiKey":"override","baseUrl":"https://script.example"}}});
    profile.config_snapshot = value.to_string();
    let identity = usage_identity(&profile).unwrap();
    value["env"]["ANTHROPIC_AUTH_TOKEN"] = json!("still-unused");
    profile.config_snapshot = value.to_string();
    assert_eq!(usage_identity(&profile).unwrap(), identity);
    value["metadata"]["usageScript"]["code"] = json!("console.log('[]')");
    profile.config_snapshot = value.to_string();
    assert_ne!(usage_identity(&profile).unwrap(), identity);
}

#[test]
fn validates_threshold_edges_and_duplicate_units() {
    let mut settings = AlertSettings {
        enabled: true,
        quota_percent: None,
        balances: vec![],
        system_notifications: false,
    };
    assert!(settings.validate().is_err());
    settings.quota_percent = Some(f64::NAN);
    assert!(settings.validate().is_err());
    settings.quota_percent = Some(100.0);
    assert!(settings.validate().is_ok());
    settings.balances = vec![BalanceThreshold {
        unit: " USD ".into(),
        amount: 0.0,
    }];
    settings.validate().unwrap();
    assert_eq!(settings.balances[0].unit, "USD");
    settings.balances.push(BalanceThreshold {
        unit: "usd".into(),
        amount: 5.0,
    });
    assert!(settings.validate().is_err());
}

#[test]
fn removing_profiles_reclaims_rule_capacity_without_deleting_history() {
    let (mut state, profile) = fixture();
    observe(
        &mut state,
        &profile,
        &json!({"success":true,"data":[{"utilization":95}]}),
        1000,
    );
    assert!(retain_profiles(&mut state, &[]));
    assert!(state.rules.is_empty());
    assert!(state.marks.is_empty());
    assert_eq!(state.events.len(), 1);
    assert_eq!(state.events[0].event.system_status, "cancelled");
    assert!(!retain_profiles(&mut state, &[]));
}

#[test]
fn partial_profile_reads_are_errors_instead_of_evidence_for_rule_deletion() {
    let conn = Connection::open_in_memory().unwrap();
    crate::db::schema::run_migrations(&conn).unwrap();
    conn.execute_batch("INSERT INTO config_profiles(id,name,tool_id,config_snapshot) VALUES ('valid','Valid','claude','{}'), ('invalid','Invalid','claude',X'01')").unwrap();
    assert!(profiles(&conn).unwrap_err().contains("could not be read"));
    conn.execute("DELETE FROM config_profiles WHERE id='invalid'", [])
        .unwrap();
    assert_eq!(profiles(&conn).unwrap().len(), 1);
}
#[test]
fn an_old_form_cannot_bind_monitoring_to_replaced_credentials() {
    let (mut state, mut profile) = fixture();
    let original = serde_json::to_value(&state).unwrap();
    let expected = usage_identity(&profile).unwrap();
    profile.config_snapshot = profile
        .config_snapshot
        .replace("private-key", "replacement");
    let settings = state.rules[&profile.id].settings.clone();
    assert!(
        set_rule_if_current(&mut state, &profile, settings, &expected)
            .unwrap_err()
            .contains("Configuration changed")
    );
    assert_eq!(serde_json::to_value(&state).unwrap(), original);
}
