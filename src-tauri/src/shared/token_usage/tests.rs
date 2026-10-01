use super::*;

#[test]
fn each_cache_write_shape_is_a_separate_input_category() {
    for usage in [
        json!({"input_tokens":1000,"output_tokens":5,"input_tokens_details":{"cached_tokens":800,"cache_write_tokens":100}}),
        json!({"prompt_tokens":1000,"completion_tokens":5,"prompt_tokens_details":{"cached_tokens":800,"cache_write_tokens":100}}),
        json!({"input_tokens":1000,"output_tokens":5,"cache_read_input_tokens":800,"cache_creation_input_tokens":100}),
        json!({"input_tokens":1000,"output_tokens":5,"cached_tokens":800,"cache_write_tokens":100}),
    ] {
        let usage = TokenUsage::parse(&usage).unwrap();
        assert_eq!(
            usage.anthropic(InputTokenBasis::IncludesCache),
            json!({
                "input_tokens":100,"output_tokens":5,"cache_read_input_tokens":800,"cache_creation_input_tokens":100,
            })
        );
    }
}

#[test]
fn ordinary_and_total_counts_follow_wire_protocol_not_relative_counter_sizes() {
    for (basis, raw) in [
        (InputTokenBasis::IncludesCache, 1000),
        (InputTokenBasis::ExcludesCache, 100),
    ] {
        assert_eq!(basis.ordinary(raw, 800, 100), 100);
        assert_eq!(basis.total(raw, 800, 100), 1000);
    }
    assert_eq!(
        InputTokenBasis::ExcludesCache.ordinary(1000, 800, 100),
        1000
    );
    assert_eq!(InputTokenBasis::IncludesCache.ordinary(100, 800, 100), 0);
}

#[test]
fn zero_is_authoritative_aliases_do_not_add_and_invalid_values_allow_fallback() {
    let usage = TokenUsage::parse(&json!({
        "input_tokens":"invalid","prompt_tokens":1000,
        "cache_read_input_tokens":0,"input_tokens_details":{"cached_tokens":800,"cache_write_tokens":100},
        "cache_creation_input_tokens":-1,"prompt_tokens_details":{"cached_tokens":900,"cache_write_tokens":200},
    })).unwrap();
    assert_eq!(usage.input, Some(1000));
    assert_eq!(usage.cache_read, Some(0));
    assert_eq!(usage.cache_write, Some(100));
    assert_eq!(
        usage.anthropic(InputTokenBasis::IncludesCache)["input_tokens"],
        900
    );
}

#[test]
fn invalid_only_usage_is_unknown_and_valid_zero_is_known() {
    for value in [
        Value::Null,
        json!({}),
        json!({"input_tokens":-1}),
        json!({"input_tokens":1.5}),
        json!({"input_tokens":"100"}),
        json!({"input_tokens_details":{"cache_write_tokens":-1}}),
    ] {
        assert!(TokenUsage::parse(&value).is_none());
    }
    assert_eq!(
        TokenUsage::parse(&json!({"input_tokens":0})).unwrap().input,
        Some(0)
    );
}

#[test]
fn partial_counters_merge_independently_in_any_order_without_double_counting() {
    let events = [
        json!({"prompt_tokens":1000}),
        json!({"completion_tokens":5}),
        json!({"prompt_tokens_details":{"cached_tokens":800}}),
        json!({"prompt_tokens_details":{"cache_write_tokens":100}}),
    ];
    for reversed in [false, true] {
        let mut events = events.to_vec();
        if reversed {
            events.reverse();
        }
        let mut usage = TokenUsage::default();
        for event in events.iter().chain(events.iter()) {
            usage.merge(&TokenUsage::parse(event).unwrap());
        }
        usage.merge(&TokenUsage::parse(&json!({"completion_tokens":2,"prompt_tokens_details":{"cached_tokens":1,"cache_write_tokens":0}})).unwrap());
        assert_eq!(
            usage.anthropic(InputTokenBasis::IncludesCache),
            json!({"input_tokens":100,"output_tokens":5,"cache_read_input_tokens":800,"cache_creation_input_tokens":100})
        );
    }
}

#[test]
fn large_token_counts_keep_u64_precision_and_saturate_arithmetic() {
    let large = u32::MAX as u64 + 123;
    let usage =
        TokenUsage::parse(&json!({"prompt_tokens":large,"completion_tokens":large})).unwrap();
    assert_eq!(
        usage.anthropic(InputTokenBasis::IncludesCache)["input_tokens"],
        large
    );
    assert_eq!(
        usage.anthropic(InputTokenBasis::IncludesCache)["output_tokens"],
        large
    );
    assert_eq!(
        InputTokenBasis::ExcludesCache.total(u64::MAX, 1, 1),
        u64::MAX
    );
    assert_eq!(InputTokenBasis::IncludesCache.ordinary(1, u64::MAX, 1), 0);
}
