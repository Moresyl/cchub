use super::*;
use serde_json::json;

fn read(value: &Value) -> Value {
    extract_profile(value).unwrap()
}

#[test]
fn native_configuration_round_trip_keeps_comments_extensions_and_variant_selection() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("opencode.jsonc");
    let source = "{\n // retain\n \"theme\": \"dark\",\n \"mcp\": {\"keep\": {}},\n \"providers\": {\"local\": {\"settings\": {\"apiKey\": \"old\", \"timeout\": 5000}, \"extension\": {\"keep\": true}, \"models\": {\"m\": {\"variants\": [{\"id\": \"fast\", \"settings\": {\"effort\": \"low\"}}], \"limit\": {\"context\": 10000}}}}, \"other\": {}},\n \"model\": {\"providerID\": \"local\", \"model\": \"m\", \"variant\": \"fast\"}\n}";
    std::fs::write(&path, source).unwrap();
    let mut profile: Value = serde_json::from_str(&read_profile(&path).unwrap()).unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), source);
    assert_eq!(profile["metadata"]["nativeFormat"], "providers");
    assert_eq!(profile["metadata"]["nativeModelId"], "m#fast");
    assert!(profile["models"]["m"].get("contextLimit").is_none());
    profile["settings"]["apiKey"] = json!("new");
    apply_profile(&path, &profile.to_string()).unwrap();
    let output = std::fs::read_to_string(&path).unwrap();
    assert!(output.contains("// retain"));
    assert!(output.contains("\"mcp\": {\"keep\": {}}"));
    let output = crate::json_config::parse_json_object(&output).unwrap();
    assert_eq!(output["providers"]["local"]["settings"]["apiKey"], "new");
    assert_eq!(output["providers"]["local"]["settings"]["timeout"], 5000);
    assert_eq!(
        output["providers"]["local"]["extension"],
        json!({"keep":true})
    );
    assert_eq!(
        output["model"],
        json!({"providerID":"local","model":"m","variant":"fast"})
    );
    assert!(output.get("provider").is_none());
    assert!(output["providers"]["local"].get("metadata").is_none());
    assert_eq!(read(&output)["models"], profile["models"]);
}

#[test]
fn empty_and_models_only_native_overrides_keep_their_source_format() {
    for provider in [json!({}), json!({"models":{"m":{}}})] {
        let document = json!({"providers":{"anthropic":provider},"model":"anthropic/m"});
        let profile = read(&document);
        assert_eq!(profile["metadata"]["nativeFormat"], "providers");
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("config.json");
        apply_profile(&path, &profile.to_string()).unwrap();
        let output: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        assert_eq!(output["providers"]["anthropic"], provider);
        assert!(output.get("provider").is_none());
        assert_eq!(output["model"], "anthropic/m");
    }
}

#[test]
fn native_priority_is_independent_of_key_order_and_invalid_entries_do_not_shadow_legacy() {
    for source in [
        r#"{"provider":{"same":{"options":{"apiKey":"old"}}},"providers":{"same":{"settings":{"apiKey":"new"}}},"model":"same/m"}"#,
        r#"{"providers":{"same":{"settings":{"apiKey":"new"}}},"model":"same/m","provider":{"same":{"options":{"apiKey":"old"}}}}"#,
    ] {
        let profile = read(&serde_json::from_str(source).unwrap());
        assert_eq!(profile["settings"]["apiKey"], "new");
        assert!(profile.get("options").is_none());
    }
    for invalid in [
        json!(null),
        json!({"settings":[]}),
        json!({"models":{"m":{"variants":{}}}}),
    ] {
        let document = json!({"provider":{"same":{"options":{"apiKey":"old"}}},"providers":{"same":invalid},"model":"same/m"});
        let profile = read(&document);
        assert_eq!(profile["options"]["apiKey"], "old");
        assert!(profile["metadata"].get("nativeFormat").is_none());
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("config.json");
        std::fs::write(&path, document.to_string()).unwrap();
        apply_profile(&path, &profile.to_string()).unwrap();
        let output: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        assert_eq!(output["providers"]["same"], invalid);
    }
}

#[test]
fn shadowed_legacy_write_and_invalid_native_write_leave_original_bytes_unchanged() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("config.jsonc");
    let source = "{ // preserve\n\"providers\":{\"same\":{}},\"model\":\"same/m\"}";
    std::fs::write(&path, source).unwrap();
    let legacy = json!({"metadata":{"nativeProviderId":"same"},"options":{"apiKey":"new"}});
    assert!(apply_profile(&path, &legacy.to_string())
        .unwrap_err()
        .contains("shadows"));
    for provider in [
        json!({"settings":[]}),
        json!({"headers":{"authorization":123}}),
        json!({"env":[false]}),
        json!({"package":false}),
        json!({"models":{"m":{"variants":{"fast":{}}}}}),
        json!({"models":{"m":{"limit":{"context":1.5}}}}),
        json!({"models":{"m":{"capabilities":{"tools":true}}}}),
        json!({"models":{"m":{"cost":{"input":1}}}}),
        json!({"settings":{"transport":"invalid"}}),
    ] {
        let snapshot = json!({"providers":{"same":provider},"model":"same/m"});
        assert!(normalize_profile(&snapshot.to_string()).is_err());
        assert!(apply_profile(&path, &snapshot.to_string()).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), source);
    }
}

#[test]
fn native_selected_provider_never_imports_an_unrelated_secret() {
    let document = json!({"providers":{"other":{"settings":{"apiKey":"unrelated"}}},"model":{"providerID":"anthropic","model":"m"}});
    let profile = read(&document);
    assert_eq!(profile["metadata"]["nativeProviderId"], "anthropic");
    assert_eq!(profile["metadata"]["nativeFormat"], "providers");
    assert!(profile.get("settings").is_none());
    assert!(
        extract_profile(&json!({"providers":{"bad":{"settings":[]}},"model":"bad/m"})).is_err()
    );
}

#[test]
fn clearing_native_object_selection_only_clears_this_provider() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("config.json");
    let mut profile = read(
        &json!({"providers":{"local":{}},"model":{"providerID":"local","model":"m","variant":"fast"}}),
    );
    profile["metadata"]["nativeModelId"] = json!("");
    for id in ["local", "other"] {
        std::fs::write(
            &path,
            json!({"providers":{"local":{}},"model":{"providerID":id,"model":"m"}}).to_string(),
        )
        .unwrap();
        apply_profile(&path, &profile.to_string()).unwrap();
        let output: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(output.get("model").is_none(), id == "local");
    }
}

#[test]
fn native_provider_ids_are_preserved_without_creating_a_trimmed_duplicate() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("config.json");
    let source =
        json!({"providers":{" local ":{"settings":{"apiKey":"before"}}},"model":" local /m"});
    std::fs::write(&path, source.to_string()).unwrap();
    let mut profile = read(&source);
    profile["settings"]["apiKey"] = json!("after");
    apply_profile(&path, &profile.to_string()).unwrap();
    let output: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    assert_eq!(output["providers"].as_object().unwrap().len(), 1);
    assert_eq!(
        output["providers"][" local "]["settings"]["apiKey"],
        "after"
    );
    assert_eq!(output["model"], " local /m");
    for id in ["", "  ", "a/b", "a#b", "a\nb"] {
        assert!(normalize_profile(
            &json!({"settings":{},"metadata":{"nativeProviderId":id}}).to_string()
        )
        .is_err());
    }
}

#[test]
fn complete_native_schema_accepts_model_and_provider_overlays_without_rewriting_extensions() {
    let profile = json!({"canonical":"openai","name":"Local","env":["EXAMPLE_KEY"],"package":"@opencode/ai/openai-compatible","settings":{"timeout":false,"chunkTimeout":0,"transport":"http","compaction":{"type":"native"},"extension":[1]},"headers":{"x-custom":"yes"},"body":{"temperature":0.2},"models":{"m":{"modelID":"vendor/m","family":"family","package":"custom","disabled":false,"compatibility":{"reasoningField":"reasoning_content","maxTokensField":"max_tokens","requireReasoning":true},"capabilities":{"tools":true,"input":["text","image"],"output":["text"]},"variants":[{"id":"fast","settings":{"effort":"low"},"headers":{},"body":{}}],"cost":[{"input":1,"output":2,"cache":{"read":0.1},"tier":{"type":"context","size":1000}}],"limit":{"context":100000,"output":0},"unknown":{"keep":true}}}});
    let normalized: Value =
        serde_json::from_str(&normalize_profile(&profile.to_string()).unwrap()).unwrap();
    for (name, value) in profile.as_object().unwrap() {
        assert_eq!(normalized[name], *value);
    }
    assert_eq!(normalized["metadata"]["nativeFormat"], "providers");
}
