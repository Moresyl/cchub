use super::*;

#[test]
fn availability_is_read_only_and_recovery_requires_fresh_probes() {
    let config = OptimizerConfig::default();
    let mut state = EndpointCircuitState {
        state: CircuitState::Open,
        open_until: Some(Instant::now() - Duration::from_secs(1)),
        ..Default::default()
    };
    assert!(state.is_available());
    assert!(state.is_available());
    assert_eq!(state.state, CircuitState::Open);
    let first = state.acquire().unwrap();
    assert!(!state.is_available());
    assert!(state.acquire().is_none());
    assert!(state.finish(first, Some(true), &config));
    assert_eq!(state.state, CircuitState::HalfOpen);
    assert_eq!(state.consecutive_successes, 1);
    let second = state.acquire().unwrap();
    assert!(state.finish(second, Some(true), &config));
    assert_eq!(state.state, CircuitState::Closed);
    assert!(state.open_until.is_none());
}

#[test]
fn late_success_does_not_close_a_newer_open_circuit() {
    let config = OptimizerConfig {
        circuit_failure_threshold: 1,
        ..Default::default()
    };
    let mut state = EndpointCircuitState::default();
    let late = state.acquire().unwrap();
    let failing = state.acquire().unwrap();
    assert!(state.finish(failing, Some(false), &config));
    let until = state.open_until;
    assert!(!state.finish(late, Some(true), &config));
    assert_eq!(state.state, CircuitState::Open);
    assert_eq!(state.open_until, until);
}

#[test]
fn stale_completion_after_manual_reset_cannot_change_new_incarnation() {
    let config = OptimizerConfig {
        circuit_failure_threshold: 1,
        ..Default::default()
    };
    let runtime = Arc::new(Mutex::new(LocalProviderProxyRuntimeInner::default()));
    let key = "claude::profile".to_string();
    let mut old =
        CircuitLease::acquire(runtime.clone(), CircuitScope::Profile, key.clone(), &config)
            .unwrap();
    runtime.lock().unwrap().profile_circuits.clear();
    let mut new =
        CircuitLease::acquire(runtime.clone(), CircuitScope::Profile, key.clone(), &config)
            .unwrap();
    new.failure();
    assert!(!old.success());
    assert_eq!(
        runtime.lock().unwrap().profile_circuits[&key].state,
        CircuitState::Open
    );
}

#[test]
fn failure_threshold_and_neutral_drop_preserve_health_counters() {
    let config = OptimizerConfig {
        circuit_failure_threshold: 2,
        ..Default::default()
    };
    let runtime = Arc::new(Mutex::new(LocalProviderProxyRuntimeInner::default()));
    let key = "profile::endpoint".to_string();
    let mut first = CircuitLease::acquire(
        runtime.clone(),
        CircuitScope::Endpoint,
        key.clone(),
        &config,
    )
    .unwrap();
    first.failure();
    assert_eq!(
        runtime.lock().unwrap().endpoint_circuits[&key].consecutive_failures,
        1
    );
    drop(
        CircuitLease::acquire(
            runtime.clone(),
            CircuitScope::Endpoint,
            key.clone(),
            &config,
        )
        .unwrap(),
    );
    assert_eq!(
        runtime.lock().unwrap().endpoint_circuits[&key].consecutive_failures,
        1
    );
    let mut second = CircuitLease::acquire(
        runtime.clone(),
        CircuitScope::Endpoint,
        key.clone(),
        &config,
    )
    .unwrap();
    second.failure();
    assert_eq!(
        runtime.lock().unwrap().endpoint_circuits[&key].state,
        CircuitState::Open
    );
    assert!(CircuitLease::acquire(runtime, CircuitScope::Endpoint, key, &config).is_none());
}
