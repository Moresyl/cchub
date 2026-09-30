use super::*;
use serde_json::json;

#[test]
fn vendor_capabilities_and_duplicate_enrichment_survive() {
    let models = merge_catalog(parse_catalog(&json!({"data":[
        {"id":" alpha ","name":"Alpha","context_length":200000,"native_endpoints":["/messages"],
         "supported_endpoints":["/responses"],"supported_reasoning_levels":["low",{"effort":"high"},"low"],
         "architecture":{"input_modalities":["text","image"],"output_modalities":["text"]}},
        {"id":"alpha","context_window":"bad","max_output_tokens":"16000","pricing":{"prompt":0,"completion":"0.002"}},
        {"id":"beta"}
    ]}), false).unwrap());
    assert_eq!(models.len(), 2);
    let alpha = &models[0];
    assert_eq!(alpha.display_name.as_deref(), Some("Alpha"));
    assert_eq!(alpha.context_window, Some(200000));
    assert_eq!(alpha.max_output_tokens, Some(16000));
    assert_eq!(alpha.input_price.as_deref(), Some("0"));
    assert_eq!(alpha.native_endpoints, Some(vec!["/messages".into()]));
    assert_eq!(
        alpha.supported_reasoning_levels,
        Some(vec!["low".into(), "high".into()])
    );
    assert_eq!(
        alpha.input_modalities,
        Some(vec!["text".into(), "image".into()])
    );
}

#[test]
fn malformed_optional_fields_do_not_lose_the_catalog() {
    let models = parse_catalog(&json!({"data":[
        {"id":"usable","context_window":{},"max_output_tokens":-1,"display_name":[],
         "pricing":{"prompt":"NaN","completion":true},"native_endpoints":42,"supported_reasoning_levels":[{}]},
        {"id":12},null,{"id":" "},"plain-id"
    ]}), false).unwrap();
    assert_eq!(models.len(), 2);
    assert_eq!(
        models[0],
        ModelInfo {
            id: "usable".into(),
            ..Default::default()
        }
    );
    assert_eq!(models[1].id, "plain-id");
}

#[test]
fn gemini_and_anthropic_limits_are_not_lost() {
    let gemini = parse_catalog(
        &json!({"models":[{"name":"models/gemini-custom","displayName":"Custom",
        "inputTokenLimit":1048576,"outputTokenLimit":65536}]}),
        true,
    )
    .unwrap();
    assert_eq!(gemini[0].id, "gemini-custom");
    assert_eq!(gemini[0].display_name.as_deref(), Some("Custom"));
    assert_eq!(gemini[0].context_window, Some(1048576));
    assert_eq!(gemini[0].max_output_tokens, Some(65536));
    let claude = parse_catalog(
        &json!({"data":[{"id":"claude","max_input_tokens":200000,"max_tokens":8192}]}),
        false,
    )
    .unwrap();
    assert_eq!(claude[0].context_window, Some(200000));
    assert_eq!(claude[0].max_output_tokens, Some(8192));
}

#[test]
fn unsafe_or_fractional_limits_are_ignored_and_valid_aliases_are_used() {
    for invalid in [
        json!(0),
        json!(-1),
        json!(1.5),
        json!("1.5"),
        json!(9007199254740992u64),
    ] {
        let models = parse_catalog(
            &json!({"data":[{"id":"a","context_window":invalid,"limit":{"context":128000}}]}),
            false,
        )
        .unwrap();
        assert_eq!(models[0].context_window, Some(128000));
    }
}

#[test]
fn explicit_empty_capabilities_are_authoritative() {
    let models = parse_catalog(
        &json!({"data":[{"id":"a","native_endpoints":[],
        "supported_endpoints":["/responses"],"modalities":{"input":[],"output":[]}}]}),
        false,
    )
    .unwrap();
    assert_eq!(models[0].native_endpoints, Some(vec![]));
    assert_eq!(models[0].input_modalities, Some(vec![]));
    assert_eq!(models[0].supported_reasoning_levels, None);
}

#[test]
fn legacy_model_info_still_deserializes() {
    let model: ModelInfo = serde_json::from_value(json!({"id":"legacy"})).unwrap();
    assert_eq!(
        model,
        ModelInfo {
            id: "legacy".into(),
            ..Default::default()
        }
    );
}

#[test]
fn wrong_response_container_is_not_an_empty_success() {
    for payload in [
        json!({"error":"unauthorized"}),
        json!({"data":{}}),
        json!({"models":null}),
    ] {
        assert!(parse_catalog(&payload, false).is_err());
    }
    assert!(parse_catalog(&json!({"data":[]}), false)
        .unwrap()
        .is_empty());
}
