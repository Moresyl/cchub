use serde_json::Value;

// Gemini reports these as independent cumulative counters. Keep each counter
// across events; missing fields must not erase a previously reported reading.
#[derive(Default)]
pub(crate) struct GeminiUsage {
    pub(crate) input: Option<u64>,
    pub(crate) cached: Option<u64>,
    candidates: Option<u64>,
    thoughts: Option<u64>,
    total: Option<u64>,
}

impl GeminiUsage {
    pub(crate) fn observe(&mut self, metadata: &Value) -> bool {
        let mut observed = false;
        for (field, reading) in [
            ("promptTokenCount", &mut self.input),
            ("cachedContentTokenCount", &mut self.cached),
            ("candidatesTokenCount", &mut self.candidates),
            ("thoughtsTokenCount", &mut self.thoughts),
            ("totalTokenCount", &mut self.total),
        ] {
            if let Some(next) = metadata.get(field).and_then(Value::as_u64) {
                *reading = Some(reading.unwrap_or(0).max(next));
                observed = true;
            }
        }
        observed
    }

    pub(crate) fn output(&self) -> u64 {
        let components = self
            .candidates
            .unwrap_or(0)
            .saturating_add(self.thoughts.unwrap_or(0));
        // A total by itself has no known input/output split. Once input is
        // available, never let an older total erase newer component readings.
        let from_total = self
            .total
            .zip(self.input)
            .map(|(total, input)| total.saturating_sub(input))
            .unwrap_or(0);
        components.max(from_total)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn independently_arriving_fields_and_duplicates_preserve_all_readings() {
        for reversed in [true, false] {
            let mut usage = GeminiUsage::default();
            let mut events = vec![
                json!({"promptTokenCount":7}),
                json!({"candidatesTokenCount":5}),
                json!({"thoughtsTokenCount":3}),
                json!({"cachedContentTokenCount":2}),
            ];
            if reversed {
                events.reverse();
            }
            for event in events.iter().chain(events.iter()) {
                assert!(usage.observe(event));
            }
            assert_eq!(
                (usage.input, usage.output(), usage.cached),
                (Some(7), 8, Some(2))
            );
            usage.observe(&json!({"candidatesTokenCount":2,"thoughtsTokenCount":1}));
            assert_eq!(usage.output(), 8);
        }
    }

    #[test]
    fn total_requires_input_and_cannot_erase_newer_components() {
        let mut usage = GeminiUsage::default();
        usage.observe(&json!({"totalTokenCount":20}));
        assert_eq!(usage.output(), 0);
        usage.observe(&json!({"promptTokenCount":7}));
        assert_eq!(usage.output(), 13);
        usage.observe(&json!({"candidatesTokenCount":15,"thoughtsTokenCount":3}));
        assert_eq!(usage.output(), 18);
    }

    #[test]
    fn invalid_counters_do_not_change_state_and_arithmetic_saturates() {
        let mut usage = GeminiUsage::default();
        for metadata in [
            Value::Null,
            json!({}),
            json!({"promptTokenCount":-1}),
            json!({"thoughtsTokenCount":"3"}),
        ] {
            assert!(!usage.observe(&metadata));
        }
        assert!(usage.observe(&json!({"promptTokenCount":0})));
        assert_eq!(usage.input, Some(0));
        usage.observe(&json!({"candidatesTokenCount":u64::MAX,"thoughtsTokenCount":1}));
        assert_eq!(usage.output(), u64::MAX);
    }
}
