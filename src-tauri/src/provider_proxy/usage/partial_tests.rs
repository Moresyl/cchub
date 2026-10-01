use super::*;
use serde_json::json;

#[test]
fn scanner_retains_independent_gemini_counters_across_byte_boundaries() {
    let mut buffer = String::new();
    let mut usage = ProxyUsageMetrics::default();
    let mut gemini = GeminiUsage::default();
    let input = concat!(
        "data: {\"usageMetadata\":{\"totalTokenCount\":20}}\n\n",
        "data: {\"usageMetadata\":{\"promptTokenCount\":7}}\n\n",
        "data: {\"usageMetadata\":{\"candidatesTokenCount\":15}}\n\n",
        "data: {\"usageMetadata\":{\"thoughtsTokenCount\":3}}\n\n",
        "data: {\"usageMetadata\":{\"cachedContentTokenCount\":2}}\n\n"
    );
    for _ in 0..2 {
        for byte in input.as_bytes() {
            scan_stream_usage_buffer(
                &mut buffer,
                std::str::from_utf8(std::slice::from_ref(byte)).unwrap(),
                &mut usage,
                &mut gemini,
            );
        }
    }
    assert_eq!(
        (
            usage.input_tokens,
            usage.output_tokens,
            usage.cache_read_tokens
        ),
        (7, 18, 2)
    );
}

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
