use super::*;

fn binding(at: Instant, cached: u64) -> Binding {
    Binding {
        profile: "p".into(),
        snapshot: "hash".into(),
        principal: None,
        turn: Turn {
            marker: Some("turn".into()),
            within: false,
        },
        at,
        cached,
    }
}

#[test]
fn modes_expiry_cache_threshold_and_turn_boundaries_are_explicit() {
    let now = Instant::now();
    let within = Turn {
        marker: Some("turn".into()),
        within: true,
    };
    let next = Turn {
        marker: Some("next".into()),
        within: true,
    };
    let plain = Turn::default();
    assert!(!keep(AffinityMode::Off, &binding(now, 1024), &within, now));
    assert!(keep(AffinityMode::Session, &binding(now, 0), &plain, now));
    assert!(keep(AffinityMode::Turn, &binding(now, 0), &within, now));
    assert!(!keep(AffinityMode::Turn, &binding(now, 0), &next, now));
    assert!(!keep(AffinityMode::Turn, &binding(now, 0), &plain, now));
    assert!(keep(AffinityMode::Auto, &binding(now, 0), &within, now));
    assert!(keep(AffinityMode::Auto, &binding(now, 1024), &plain, now));
    assert!(!keep(AffinityMode::Auto, &binding(now, 1023), &plain, now));
    assert!(!keep(
        AffinityMode::Auto,
        &binding(now - CACHE_KEEP, 1024),
        &plain,
        now
    ));
    assert!(!keep(
        AffinityMode::Session,
        &binding(now - KEEP, 0),
        &within,
        now
    ));
}

#[test]
fn newer_requests_own_the_binding_and_eviction_or_expiry_cannot_be_resurrected() {
    let now = Instant::now();
    let mut store = Store::default();
    let (old, _) = store.begin("session".into(), now);
    let (new, _) = store.begin("session".into(), now);
    store.commit("session", old, binding(now, 0));
    assert!(store.0[0].binding.is_none());
    store.commit("session", new, binding(now, 7));
    assert_eq!(store.0[0].binding.as_ref().unwrap().cached, 7);
    for index in 0..MAX_SESSIONS {
        store.begin(index.to_string(), now);
    }
    assert_eq!(store.0.len(), MAX_SESSIONS);
    store.commit("session", new, binding(now, 0));
    assert!(!store.0.iter().any(|entry| entry.key == "session"));
    store.begin("expired".into(), now + KEEP);
    assert_eq!(store.0.len(), 1);
    let (ticket, _) = store.begin("expired".into(), now + KEEP);
    store.commit("expired", ticket, binding(now, 1024));
    assert!(store.begin("expired".into(), now + KEEP).1.is_none());
}
