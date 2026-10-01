use super::{repair, repair_whole};
use serde_json::{json, Value};

#[test]
fn empty_shapes_are_repaired_in_every_choice_and_call() {
    let source = r#"{"choices":[{"index":0,"delta":{"reasoning_content":"","tool_calls":[{"index":0,"function":{"name":"","arguments":"a"}},{"index":1,"function":{"name":"grep","arguments":"b"}}]},"finish_reason":""},{"index":1,"delta":{"content":"你好","tool_calls": [ ]},"finish_reason":""}]}"#;
    let output = repair(source).unwrap();
    let value: Value = serde_json::from_str(&output).unwrap();
    assert_eq!(
        value,
        json!({"choices":[
            {"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"a"}},{"index":1,"function":{"name":"grep","arguments":"b"}}]},"finish_reason":null},
            {"index":1,"delta":{"content":"你好"},"finish_reason":null}
        ]})
    );
}

#[test]
fn opaque_fields_numbers_escapes_and_layout_keep_their_original_bytes() {
    let source = r#"{ "choices" : [{ "delta" : { "content":"\u4f60\u597d", "reasoning_content":"", "signature":"YWJj==\/\u003d", "unknown": {"n":99999999999999999999999999999999999,"e":1.2300e+999,"duplicate":1,"duplicate":2} }, "finish_reason" : "" }], "encrypted_content":"abc\/def\u003d", "usage": { "prompt_tokens": 7 } }"#;
    let output = repair(source).unwrap();
    assert_eq!(
        output,
        source
            .replace(" \"reasoning_content\":\"\",", "")
            .replace("\"finish_reason\" : \"\"", "\"finish_reason\" : null")
    );
}

#[test]
fn escaped_field_names_are_recognized_without_rewriting_other_keys() {
    let source = r#"{"choices":[{"delta":{"reasoning_\u0063ontent":"","tool_\u0063alls":[]},"finish_\u0072eason":""}],"\u006deta":"\u0061"}"#;
    assert_eq!(
        repair(source).unwrap(),
        r#"{"choices":[{"delta":{},"finish_\u0072eason":null}],"\u006deta":"\u0061"}"#
    );
}

#[test]
fn empty_array_hints_accept_all_json_whitespace_without_other_empty_values() {
    for array in ["[]", "[ ]", "[\t\r\n ]"] {
        let source =
            format!(r#"{{"choices":[{{"delta":{{"tool_\u0063alls":{array},"content":"a"}}}}]}}"#);
        assert_eq!(
            repair(&source).unwrap(),
            r#"{"choices":[{"delta":{"content":"a"}}]}"#
        );
    }
}

#[test]
fn deleting_first_middle_last_and_adjacent_fields_keeps_json_valid() {
    for (fields, expected) in [
        (
            r#""reasoning_content":"","tool_calls":[],"content":"a""#,
            r#""content":"a""#,
        ),
        (
            r#""content":"a","reasoning_content":"","tool_calls":[]"#,
            r#""content":"a""#,
        ),
        (
            r#""content":"a","reasoning_content":"","tool_calls":[],"extra":1"#,
            r#""content":"a","extra":1"#,
        ),
        (r#""reasoning_content":"","tool_calls":[]"#, ""),
        (
            r#""reasoning_content":"","content":"a","tool_calls":[]"#,
            r#""content":"a""#,
        ),
        (
            r#""tool_calls":[],"content":"a","reasoning_content":"","extra":1"#,
            r#""content":"a","extra":1"#,
        ),
    ] {
        let source = format!(r#"{{"choices":[{{"delta":{{{fields}}}}}]}}"#);
        let output = repair(&source).unwrap();
        assert_eq!(
            output,
            format!(r#"{{"choices":[{{"delta":{{{expected}}}}}]}}"#)
        );
        assert!(serde_json::from_str::<Value>(&output).is_ok());
    }
    let source = "{\"choices\":[{\"delta\":{\n \"content\":\"a\" , \n \"reasoning_content\": \"\" , \"tool_calls\": [ ] \n}}]}";
    assert_eq!(
        repair(source).unwrap(),
        "{\"choices\":[{\"delta\":{\n \"content\":\"a\"  \n}}]}"
    );
}

#[test]
fn only_empty_tool_names_are_removed_at_each_position() {
    for (function, expected) in [
        (
            r#"{"name":"","arguments":"{}","x":1}"#,
            r#"{"arguments":"{}","x":1}"#,
        ),
        (
            r#"{"arguments":"{}","name":"","x":1}"#,
            r#"{"arguments":"{}","x":1}"#,
        ),
        (
            r#"{"arguments":"{}","x":1,"name":""}"#,
            r#"{"arguments":"{}","x":1}"#,
        ),
        (r#"{"name":""}"#, "{}"),
    ] {
        let source =
            format!(r#"{{"choices":[{{"delta":{{"tool_calls":[{{"function":{function}}}]}}}}]}}"#);
        assert_eq!(repair(&source).unwrap(), source.replace(function, expected));
    }
}

#[test]
fn malformed_ambiguous_and_non_target_data_is_left_untouched() {
    for source in [
        "[DONE]",
        "{",
        "null",
        "[]",
        r#"{"choices":null}"#,
        r#"{"choices":[{"delta":null,"finish_reason":""}]}"#,
        r#"{"choices":[{"delta":{"tool_calls":null},"finish_reason":""}]}"#,
        r#"{"choices":[{"delta":{"tool_calls":[null]},"finish_reason":""}]}"#,
        r#"{"choices":[{"delta":{"tool_calls":[{"function":null}]},"finish_reason":""}]}"#,
        r#"{"choices":[],"choices":[{"finish_reason":""}]}"#,
        r#"{"choices":[{"delta":{"reasoning_content":"","reasoning_\u0063ontent":"real"}}]}"#,
        r#"{"choices":[{"delta":{},"finish_reason":"","finish_reason":"stop"}]}"#,
        r#"{"choices":[{"delta":{"tool_calls":[{"function":{"name":"","name":"real"}}]}}]}"#,
        r#"{"choices":[{"delta":{"reasoning_content":"think","tool_calls":[{"function":{"name":"grep","arguments":""}}]},"finish_reason":"stop"}]}"#,
        r#"{"choices":[{"delta":{"content":"","reasoning_content":null},"finish_reason":null}],"name":"","tool_calls":[],"finish_reason":""}"#,
    ] {
        assert!(repair(source).is_none(), "unexpected change: {source}");
    }
}

#[test]
fn last_wins_tool_name_clients_keep_the_identity_and_all_argument_fragments() {
    let mut name = String::new();
    let mut arguments = String::new();
    for function in [
        r#"{"name":"grep","arguments":"{\"path\":\""}"#,
        r#"{"name":"","arguments":"文件"}"#,
        r#"{"name":"","arguments":"\"}"}"#,
    ] {
        let source = format!(
            r#"{{"choices":[{{"delta":{{"tool_calls":[{{"index":0,"function":{function}}}]}}}}]}}"#
        );
        let output = repair(&source).unwrap_or(source);
        let value: Value = serde_json::from_str(&output).unwrap();
        let function = &value["choices"][0]["delta"]["tool_calls"][0]["function"];
        if let Some(next) = function["name"].as_str() {
            name = next.to_owned();
        }
        arguments.push_str(function["arguments"].as_str().unwrap());
    }
    assert_eq!(name, "grep");
    assert_eq!(arguments, r#"{"path":"文件"}"#);
}

#[test]
fn typed_content_edits_preserve_opaque_numbers_signatures_and_escaped_keys() {
    let source = r#"{ "choices" : [{ "delta" : { "con\u0074ent":[{"type":"thinking","thinking":[{"type":"text","text":"想🦀"}],"closed":true},{"type":"text","text":"答"}], "reasoning_\u0063ontent":"先", "signature":"YWJj==\/\u003d", "unknown":{"n":99999999999999999999999999,"e":1.2300e+999,"duplicate":1,"duplicate":2} }, "finish_reason":null }],"usage":{"prompt_tokens":7},"encrypted_content":"abc\/def\u003d" }"#;
    let output = repair(source).unwrap();
    assert_eq!(output, source.replace(r#"[{"type":"thinking","thinking":[{"type":"text","text":"想🦀"}],"closed":true},{"type":"text","text":"答"}]"#, r#""答""#)
        .replace(r#""reasoning_\u0063ontent":"先""#, r#""reasoning_\u0063ontent":"先想🦀""#));
}

#[test]
fn inserts_reasoning_and_combines_aliases_without_overlapping_empty_shape_edits() {
    for fields in [
        r#""content":[{"type":"thinking","thinking":"想"}],"tool_calls":[],"reasoning":"先""#,
        r#""reasoning":"先","tool_calls":[],"content":[{"type":"thinking","thinking":"想"}]"#,
        r#""reasoning_content":"","content":[{"type":"thinking","thinking":"想"}],"tool_calls":[]"#,
        r#""content":[{"type":"thinking","thinking":"想"}],"reasoning_content":null"#,
        r#""reasoning_content":"先","reasoning":"先","content":[{"type":"thinking","thinking":"想"}]"#,
    ] {
        let source = format!(r#"{{"choices":[{{"delta":{{{fields}}},"finish_reason":""}}]}}"#);
        let output = repair(&source).unwrap();
        let value: Value = serde_json::from_str(&output).unwrap();
        let delta = &value["choices"][0]["delta"];
        assert_eq!(delta["content"], "");
        assert_eq!(
            delta["reasoning_content"],
            if fields.contains("先") {
                "先想"
            } else {
                "想"
            }
        );
        assert!(delta.get("reasoning").is_none() && delta.get("tool_calls").is_none());
        assert!(value["choices"][0]["finish_reason"].is_null());
    }
}

#[test]
fn unknown_duplicate_and_signed_parts_remain_opaque_in_native_relays() {
    for part in [
        r#"{"type":"image_url","image_url":{"url":"secret://image"}}"#,
        r#"{"type":"text","text":"one","text":"two"}"#,
        r#"{"type":"thinking","thinking":"one","signature":"opaque"}"#,
        r#"{"type":"thinking","thinking":[{"type":"image","text":"one"}]}"#,
        r#"{"type":"text","text":"one","annotations":[{"x":1.200e+99}]}"#,
    ] {
        let source =
            format!(r#"{{"choices":[{{"delta":{{"content":[{part}]}},"finish_reason":null}}]}}"#);
        assert!(repair(&source).is_none(), "{source}");
        assert!(repair_whole(&source.replace("delta", "message")).is_none());
    }
    let source = r#"{"choices":[{"delta":{"content":[{"type":"thinking","thinking":"new"}],"reasoning":"one","reasoning_content":"two"}}]}"#;
    assert!(repair(source).is_none());
}

#[test]
fn whole_chat_only_flattens_parts_and_retains_tool_identity_and_empty_metadata() {
    let source = r#"{"choices":[{"message":{"content":[{"type":"text","text":"答"}],"reasoning_content":"","tool_calls":[{"function":{"name":"","arguments":"{}"}}]},"finish_reason":""}],"opaque":1.2300e+99}"#;
    assert_eq!(
        repair_whole(source).unwrap(),
        source.replace(r#"[{"type":"text","text":"答"}]"#, r#""答""#)
    );
    let ordinary = source.replace(r#"[{"type":"text","text":"答"}]"#, r#""答""#);
    assert!(repair_whole(&ordinary).is_none());
}
