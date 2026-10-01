use super::*;
use serde_json::json;

#[test]
fn partial_gemini_usage_preserves_input_cache_and_reasoning_without_double_counting() {
    let mut current = ProxyUsageMetrics::default();
    for (value, expected) in [
        (
            json!({"usageMetadata":{"promptTokenCount":7,"cachedContentTokenCount":2}}),
            (7, 0, 2),
        ),
        (
            json!({"usageMetadata":{"candidatesTokenCount":3}}),
            (7, 3, 2),
        ),
        (
            json!({"usageMetadata":{"candidatesTokenCount":5,"thoughtsTokenCount":3}}),
            (7, 8, 2),
        ),
        (
            json!({"usageMetadata":{"promptTokenCount":7,"totalTokenCount":20,"candidatesTokenCount":5,"thoughtsTokenCount":3}}),
            (7, 13, 2),
        ),
    ] {
        let next = extract_stream_usage_metrics_from_event(&value).unwrap();
        merge_proxy_usage_metrics(&mut current, &next);
        merge_proxy_usage_metrics(&mut current, &next);
        assert_eq!(
            (
                current.input_tokens,
                current.output_tokens,
                current.cache_read_tokens
            ),
            expected
        );
    }
}

#[test]
fn partial_gemini_total_does_not_invent_output_and_invalid_counters_are_ignored() {
    let next =
        extract_stream_usage_metrics_from_event(&json!({"usageMetadata":{"totalTokenCount":20}}))
            .unwrap();
    assert_eq!(next.input_tokens, 0);
    assert_eq!(next.output_tokens, 0);
    for value in [
        json!({"usageMetadata":{}}),
        json!({"usageMetadata":{"promptTokenCount":-1}}),
        json!({"usageMetadata":{"totalTokenCount":"unknown"}}),
    ] {
        assert!(extract_stream_usage_metrics_from_event(&value).is_none());
    }
    let zero =
        extract_stream_usage_metrics_from_event(&json!({"usageMetadata":{"promptTokenCount":0}}))
            .unwrap();
    assert_eq!(zero.input_tokens, 0);
}
