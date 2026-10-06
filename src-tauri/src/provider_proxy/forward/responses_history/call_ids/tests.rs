use super::*;
use serde_json::{json, Value};

#[test]
fn bounds_links_for_all_supported_call_pairs_and_is_stable_across_requests() {
    let id = "foreign-call-".repeat(8);
    for (call, result) in [
        ("function_call", "function_call_output"),
        ("tool_search_call", "tool_search_output"),
        ("custom_tool_call", "custom_tool_call_output"),
    ] {
        let body =
            json!({"input":[{"type":call,"call_id":id},{"type":result,"call_id":id}]}).to_string();
        let output = repair(Bytes::from(body.clone()));
        let value: Value = serde_json::from_slice(&output).unwrap();
        let link = value["input"][0]["call_id"].as_str().unwrap();
        assert_eq!(link.len(), 64);
        assert_eq!(value["input"][1]["call_id"], link);
        assert_eq!(repair(Bytes::from(body)), output);
        assert_eq!(repair(output.clone()), output);
        let followup: Value = serde_json::from_slice(&repair(Bytes::from(
            json!({"input":[{"type":result,"call_id":id}]}).to_string(),
        )))
        .unwrap();
        assert_eq!(followup["input"][0]["call_id"], link);
    }
}

#[test]
fn retains_exact_opaque_payload_bytes_and_escaped_pairing() {
    let id = "a".repeat(65);
    let escaped = format!("{}\\u0061", "a".repeat(64));
    let body = format!(
        r#" {{ "input": [
        {{"type":"function_call","call_id":"{id}","arguments":"{{\"raw\":1.2300}}"}},
        {{"type":"function_call_output","call_id":"{escaped}","output":184467440737095516160}},
        {{"type":"reasoning","encrypted_content":"{id}\\u003d\\u003d"}}
      ],"extra":1.230000e+45 }} "#
    );
    let digest = format!("{:x}", Sha256::digest(id.as_bytes()));
    let expected = body
        .replacen(
            &format!(r#""call_id":"{id}""#),
            &format!(r#""call_id":"{digest}""#),
            1,
        )
        .replacen(
            &format!(r#""call_id":"{escaped}""#),
            &format!(r#""call_id":"{digest}""#),
            1,
        );
    assert_eq!(repair(Bytes::from(body)).as_ref(), expected.as_bytes());
}

#[test]
fn valid_missing_wrongly_typed_and_unrelated_links_keep_their_bytes_and_allocation() {
    for body in [
        json!({"input":[{"type":"function_call","call_id":"x".repeat(64)}]}).to_string(),
        json!({"input":[{"type":"function_call","call_id":"字".repeat(64)}]}).to_string(),
        json!({"input":[{"type":"message","call_id":"x".repeat(80)}]}).to_string(),
        r#"{"input":[{"type":"function_call","call_id":null},{"type":"function_call_output"}]}"#
            .into(),
        r#"{"input":[],"input":[]}"#.into(),
        r#"{"input":"text"}"#.into(),
        r#"{"input":[] } invalid"#.into(),
    ] {
        let bytes = Bytes::from(body);
        let output = repair(bytes.clone());
        assert_eq!(output, bytes);
        assert_eq!(output.as_ptr(), bytes.as_ptr());
    }
}

#[test]
fn different_long_links_remain_distinct_and_do_not_collide_with_existing_short_links() {
    let first = "x".repeat(65);
    let collision = format!("{:x}", Sha256::digest(first.as_bytes()));
    let body = json!({"input":[
        {"type":"function_call","call_id":first},
        {"type":"function_call_output","call_id":first},
        {"type":"function_call","call_id":"字".repeat(65)},
        {"type":"function_call","call_id":collision}
    ]})
    .to_string();
    let output: Value = serde_json::from_slice(&repair(Bytes::from(body))).unwrap();
    assert_ne!(output["input"][0]["call_id"], collision);
    assert_eq!(output["input"][0]["call_id"], output["input"][1]["call_id"]);
    assert_ne!(output["input"][0]["call_id"], output["input"][2]["call_id"]);
    assert_eq!(output["input"][2]["call_id"].as_str().unwrap().len(), 64);
    assert_eq!(output["input"][3]["call_id"], collision);
}
