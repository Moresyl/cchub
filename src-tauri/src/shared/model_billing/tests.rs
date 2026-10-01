use super::*;
use serde_json::json;

#[test]
fn interprets_free_premium_fractional_and_promotional_request_units() {
    for (raw, kind, multiplier) in [
        (
            json!({"is_premium":false,"multiplier":0}),
            BillingKind::Free,
            0.0,
        ),
        (
            json!({"is_premium":true,"multiplier":0}),
            BillingKind::Free,
            0.0,
        ),
        (
            json!({"is_premium":true,"multiplier":"0.33"}),
            BillingKind::Premium,
            0.33,
        ),
    ] {
        let billing = ModelBilling::from_value(&raw);
        assert_eq!(billing.kind, kind);
        assert_eq!(billing.multiplier, Some(multiplier));
        assert_eq!(billing.premium(), kind == BillingKind::Premium);
    }
}

#[test]
fn malformed_missing_negative_and_conflicting_metadata_are_unknown() {
    for value in [
        json!(null),
        json!({}),
        json!({"is_premium":true}),
        json!({"is_premium":false,"multiplier":1}),
        json!({"multiplier":0}),
        json!({"is_premium":true,"multiplier":-1}),
        json!({"is_premium":true,"multiplier":"NaN"}),
        json!({"is_premium":true,"multiplier":"Infinity"}),
        json!({"is_premium":"true","multiplier":1}),
    ] {
        let billing = ModelBilling::from_value(&value);
        assert_eq!(billing, ModelBilling::default(), "{value}");
        assert!(!billing.premium());
    }
}

#[test]
fn catalog_agreement_keeps_known_premium_kind_without_inventing_a_multiplier() {
    let models = catalog(&json!({"data":[
        {"id":"same","billing":{"is_premium":true,"multiplier":0.33}},
        {"id":"same","billing":{"is_premium":true,"multiplier":0.33}},
        {"id":"varying","billing":{"is_premium":true,"multiplier":1}},
        {"id":"varying","billing":{"is_premium":true,"multiplier":2}},
        {"id":"uncertain","billing":{"is_premium":true,"multiplier":1}}, {"id":"uncertain"},
        {"id":"mixed","billing":{"is_premium":true,"multiplier":1}},
        {"id":"mixed","billing":{"is_premium":false,"multiplier":0}}
    ]}));
    assert_eq!(models["same"].multiplier, Some(0.33));
    assert_eq!(
        models["varying"],
        ModelBilling {
            kind: BillingKind::Premium,
            multiplier: None
        }
    );
    assert!(models["varying"].premium());
    assert_eq!(models["uncertain"], ModelBilling::default());
    assert_eq!(models["mixed"], ModelBilling::default());
}

#[test]
fn catalog_bounds_and_exact_identity_do_not_promote_partial_metadata() {
    let entry = json!({"id":"paid","billing":{"is_premium":true,"multiplier":1}});
    assert!(catalog(&json!({"data":vec![entry.clone();513]})).is_empty());
    assert!(catalog(&json!({"data":null})).is_empty());
    assert!(catalog(
        &json!({"data":[{"id":" ","billing":{"is_premium":true,"multiplier":1}},
        {"id":"x".repeat(1025),"billing":{"is_premium":true,"multiplier":1}}]})
    )
    .is_empty());
    let models = catalog(&json!({"data":[entry]}));
    assert!(!models.contains_key("PAID"));
}

#[test]
fn normalized_billing_serializes_without_turning_it_into_monetary_prices() {
    let billing = ModelBilling::from_value(&json!({"is_premium":true,"multiplier":0.33}));
    let value = serde_json::to_value(billing).unwrap();
    assert_eq!(value, json!({"kind":"premium","multiplier":0.33}));
    assert_eq!(
        serde_json::from_value::<ModelBilling>(value).unwrap(),
        billing
    );
}
