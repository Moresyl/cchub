use super::*;
use serde_json::{json, Value};

#[test]
fn repair_changes_only_search_item_ids_and_preserves_opaque_history_bytes() {
    let source = r#"{ "input": [
      { "type": "tool_search_call", "id": "fc_legacy", "call_id": "original", "arguments": {"n": 184467440737095516160, "decimal": 1.2300e+15}},
      {"type":"tool_search_output","call_id":"original","tools":[{"type":"function","name":"日历"}]},
      {"type":"reasoning","encrypted_content":"ciphertext\u003d\u003d","signature":"opaque\\signature"},
      {"type":"function_call","id":"fc_shell","call_id":"unchanged","arguments":"{\"exact\":true}"}
    ], "arbitrary": 999999999999999999999999999999.00100 }"#;
    let output = repair("v1/responses", Bytes::copy_from_slice(source.as_bytes()));
    let expected = source.replacen("\"fc_legacy\"", "\"tsc_legacy\"", 1);
    assert_eq!(output.as_ref(), expected.as_bytes());
    assert_eq!(repair("v1/responses", output.clone()), output);
}

#[test]
fn encoded_type_and_id_strings_are_decoded_without_rewriting_other_fields() {
    let source = r#"{"input":[{"type":"tool_\u0073earch_call","id":"fc\u005fescaped","call_id":"fc_escaped","arguments":{"text":"fc_escaped"}}]}"#;
    let output = repair(
        "responses/compact",
        Bytes::copy_from_slice(source.as_bytes()),
    );
    assert_eq!(
        output.as_ref(),
        source
            .replacen(r#""fc\u005fescaped""#, r#""tsc_escaped""#, 1)
            .as_bytes()
    );
}

#[test]
fn already_valid_absent_malformed_or_unrelated_histories_retain_bytes_and_allocation() {
    for source in [
        r#"{ "input": [{"type":"tool_search_call","id":"tsc_keep"},{"type":"tool_search_call","call_id":"missing"}] }"#,
        r#"{"input":[{"type":"tool_search_call","id":null},{"type":"tool_search_call","id":2},{"type":"tool_search_call","id":""}]}"#,
        r#"{"input":[{"type":"function_call","id":"fc_keep"}]}"#,
        r#"{"input":"plain text"}"#,
        r#"{"input":{}}"#,
        r#"{"input":[{"type":"tool_search_call","id":"fc_bad"}]} trailing"#,
        r#"{"input":[],"input":[{"type":"tool_search_call","id":"fc_duplicate_field"}]}"#,
    ] {
        let input = Bytes::copy_from_slice(source.as_bytes());
        let output = repair("v1/responses", input.clone());
        assert_eq!(output, input);
        assert_eq!(output.as_ptr(), input.as_ptr());
    }
    let input = Bytes::from_static(br#"{"input":[{"type":"tool_search_call","id":"fc_other"}]}"#);
    for path in [
        "v1/messages",
        "v1/chat/completions",
        "v1/responses/arbitrary",
        "other/responses",
    ] {
        assert_eq!(repair(path, input.clone()).as_ptr(), input.as_ptr());
    }
}

#[test]
fn collision_repairs_are_unique_stable_and_keep_original_duplicates_consistent() {
    let collision = format!("tsc_{:x}", Sha256::digest(b"fc_one"));
    let input = Bytes::from(
        json!({"input":[
            {"type":"tool_search_call","id":"fc_one","call_id":"a"},
            {"type":"tool_search_call","id":"tsc_one","call_id":"b"},
            {"type":"tool_search_call","id":"other_one","call_id":"c"},
            {"type":"tool_search_call","id":"fc_one","call_id":"a"},
            {"type":"message","id":collision}
        ]})
        .to_string(),
    );
    let output = repair("responses", input.clone());
    assert_eq!(repair("responses", input), output);
    let value: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(value["input"][0]["id"], format!("{collision}_1"));
    assert_eq!(value["input"][1]["id"], "tsc_one");
    assert_ne!(value["input"][0]["id"], value["input"][2]["id"]);
    assert_eq!(value["input"][0]["id"], value["input"][3]["id"]);
    assert_eq!(value["input"][4]["id"], collision);
    assert_eq!(value["input"][0]["call_id"], "a");
}

#[test]
fn item_id_size_bounds_do_not_damage_unknown_or_oversized_items() {
    let exact = "x".repeat(1024);
    let oversized = "x".repeat(1025);
    let input = Bytes::from(
        json!({"input":[
            {"type":"tool_search_call","id":exact},
            {"type":"tool_search_call","id":oversized},
            {"type":"tool_search_call","id":"plain"}
        ]})
        .to_string(),
    );
    let output: Value = serde_json::from_slice(&repair("/v1/responses/compact/", input)).unwrap();
    assert_eq!(output["input"][0]["id"], format!("tsc_{exact}"));
    assert_eq!(output["input"][1]["id"], oversized);
    assert_eq!(output["input"][2]["id"], "tsc_plain");
}
