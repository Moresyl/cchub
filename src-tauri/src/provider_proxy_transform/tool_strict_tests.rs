use super::*;

fn schema() -> Value {
    json!({
        "type":"object",
        "properties":{
            "path":{"type":"string","pattern":"^(?!.*\\.\\.).+$"},
            "options":{"type":"object","properties":{"limit":{"type":"integer"}}}
        },
        "required":["path"]
    })
}

fn request(strict: Option<Value>) -> Value {
    let mut tool = json!({"name":"read_file","description":"Read a file","input_schema":schema()});
    if let Some(strict) = strict {
        tool["strict"] = strict;
    }
    json!({"model":"fixture-model","messages":[],"tools":[tool]})
}

#[test]
fn translated_responses_explicitly_keeps_loose_schemas_and_client_strictness() {
    for oauth in [false, true] {
        for (strict, expected) in [
            (None, false),
            (Some(Value::Null), false),
            (Some(json!(false)), false),
            (Some(json!(true)), true),
        ] {
            let converted = anthropic_to_responses(request(strict), oauth).unwrap();
            let tool = &converted["tools"][0];
            assert_eq!(tool["strict"], expected);
            assert_eq!(tool["parameters"], schema());
            assert_eq!(tool["name"], "read_file");
        }
    }
}

#[test]
fn chat_preserves_explicit_strictness_without_changing_optional_parameters() {
    for strict in [
        None,
        Some(Value::Null),
        Some(json!(false)),
        Some(json!(true)),
    ] {
        let expected = strict.as_ref().and_then(Value::as_bool);
        let converted = anthropic_to_openai(request(strict)).unwrap();
        let function = &converted["tools"][0]["function"];
        assert_eq!(function.get("strict").and_then(Value::as_bool), expected);
        assert_eq!(function["parameters"], schema());
    }
}

#[test]
fn responses_does_not_add_function_strictness_to_hosted_or_batch_tools() {
    let converted = anthropic_to_responses(
        json!({"model":"fixture-model","messages":[],"tools":[
            {"type":"web_search_20250305","name":"web_search"},
            {"type":"BatchTool","name":"batch"},
            {"name":"read_file","input_schema":schema()}
        ]}),
        false,
    )
    .unwrap();
    assert_eq!(converted["tools"].as_array().unwrap().len(), 2);
    assert_eq!(converted["tools"][0]["type"], "web_search");
    assert!(converted["tools"][0].get("strict").is_none());
    assert_eq!(converted["tools"][1]["strict"], false);
}
