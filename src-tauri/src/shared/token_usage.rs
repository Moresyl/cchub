use serde_json::{json, Value};

// Messages counts only fresh input; Chat, Responses and Gemini include cache.
// Select this from the actual wire protocol, never from the client/app name.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum InputTokenBasis {
    #[default]
    IncludesCache,
    ExcludesCache,
}

impl InputTokenBasis {
    pub(crate) fn ordinary(self, input: u64, read: u64, write: u64) -> u64 {
        match self {
            Self::IncludesCache => input.saturating_sub(read).saturating_sub(write),
            Self::ExcludesCache => input,
        }
    }

    pub(crate) fn total(self, input: u64, read: u64, write: u64) -> u64 {
        match self {
            Self::IncludesCache => input,
            Self::ExcludesCache => input.saturating_add(read).saturating_add(write),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct TokenUsage {
    pub(crate) input: Option<u64>,
    pub(crate) output: Option<u64>,
    pub(crate) cache_read: Option<u64>,
    pub(crate) cache_write: Option<u64>,
}

fn first_counter(value: &Value, paths: &[&str]) -> Option<u64> {
    // A valid zero is authoritative. Invalid aliases don't hide valid fallbacks.
    paths.iter().find_map(|path| value.pointer(path)?.as_u64())
}

impl TokenUsage {
    pub(crate) fn parse(value: &Value) -> Option<Self> {
        let usage = Self {
            input: first_counter(value, &["/input_tokens", "/prompt_tokens"]),
            output: first_counter(value, &["/output_tokens", "/completion_tokens"]),
            cache_read: first_counter(
                value,
                &[
                    "/cache_read_input_tokens",
                    "/input_tokens_details/cached_tokens",
                    "/prompt_tokens_details/cached_tokens",
                    "/prompt_cache_hit_tokens",
                    "/cached_tokens",
                ],
            ),
            cache_write: first_counter(
                value,
                &[
                    "/cache_creation_input_tokens",
                    "/input_tokens_details/cache_write_tokens",
                    "/prompt_tokens_details/cache_write_tokens",
                    "/cache_write_tokens",
                ],
            ),
        };
        [
            usage.input,
            usage.output,
            usage.cache_read,
            usage.cache_write,
        ]
        .iter()
        .any(Option::is_some)
        .then_some(usage)
    }

    pub(crate) fn merge(&mut self, next: &Self) {
        for (current, next) in [
            (&mut self.input, next.input),
            (&mut self.output, next.output),
            (&mut self.cache_read, next.cache_read),
            (&mut self.cache_write, next.cache_write),
        ] {
            if let Some(next) = next {
                *current = Some(current.unwrap_or(0).max(next));
            }
        }
    }

    pub(crate) fn anthropic(&self, basis: InputTokenBasis) -> Value {
        let mut value = json!({
            "input_tokens": basis.ordinary(self.input.unwrap_or(0), self.cache_read.unwrap_or(0), self.cache_write.unwrap_or(0)),
            "output_tokens": self.output.unwrap_or(0),
        });
        if let Some(read) = self.cache_read {
            value["cache_read_input_tokens"] = json!(read);
        }
        if let Some(write) = self.cache_write {
            value["cache_creation_input_tokens"] = json!(write);
        }
        value
    }
}

#[cfg(test)]
mod tests;
