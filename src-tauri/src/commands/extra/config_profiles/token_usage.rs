use std::io::{Read, Seek, SeekFrom};

const CODEX_TOKEN_TAIL_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SessionTokenTotals {
    input_tokens: u64,
    output_tokens: u64,
    total_tokens: u64,
    has_usage: bool,
}

impl SessionTokenTotals {
    fn record(
        &mut self,
        input_tokens: Option<u64>,
        output_tokens: Option<u64>,
        total_tokens: Option<u64>,
    ) {
        let resolved_input = input_tokens.unwrap_or(0);
        let resolved_output = output_tokens.unwrap_or(0);
        let resolved_total =
            total_tokens.unwrap_or_else(|| resolved_input.saturating_add(resolved_output));

        if resolved_input == 0 && resolved_output == 0 && resolved_total == 0 {
            return;
        }

        self.input_tokens = self.input_tokens.saturating_add(resolved_input);
        self.output_tokens = self.output_tokens.saturating_add(resolved_output);
        self.total_tokens = self.total_tokens.saturating_add(resolved_total);
        self.has_usage = true;
    }

    fn from_snapshot(
        input_tokens: Option<u64>,
        output_tokens: Option<u64>,
        total_tokens: Option<u64>,
    ) -> Self {
        let mut totals = Self::default();
        totals.record(input_tokens, output_tokens, total_tokens);
        totals
    }

    pub fn input_option(self) -> Option<u64> {
        self.has_usage.then_some(self.input_tokens)
    }

    pub fn output_option(self) -> Option<u64> {
        self.has_usage.then_some(self.output_tokens)
    }

    pub fn total_option(self) -> Option<u64> {
        self.has_usage.then_some(self.total_tokens)
    }
}

fn read_token_u64(value: &serde_json::Value) -> Option<u64> {
    match value {
        serde_json::Value::Number(number) => number.as_u64(),
        serde_json::Value::String(text) => text.trim().parse::<u64>().ok(),
        _ => None,
    }
}

fn object_usage_totals(
    map: &serde_json::Map<String, serde_json::Value>,
) -> Option<(Option<u64>, Option<u64>, Option<u64>)> {
    let input_tokens = [
        "input_tokens",
        "prompt_tokens",
        "inputTokenCount",
        "inputTokens",
    ]
    .iter()
    .find_map(|key| map.get(*key).and_then(read_token_u64));
    let output_tokens = [
        "output_tokens",
        "completion_tokens",
        "candidatesTokenCount",
        "outputTokenCount",
        "outputTokens",
    ]
    .iter()
    .find_map(|key| map.get(*key).and_then(read_token_u64));
    let total_tokens = ["total_tokens", "totalTokenCount", "totalTokens"]
        .iter()
        .find_map(|key| map.get(*key).and_then(read_token_u64));

    (input_tokens.is_some() || output_tokens.is_some() || total_tokens.is_some()).then_some((
        input_tokens,
        output_tokens,
        total_tokens,
    ))
}

pub fn accumulate_token_usage_from_value(
    value: &serde_json::Value,
    totals: &mut SessionTokenTotals,
    depth: usize,
) {
    if depth > 8 {
        return;
    }

    match value {
        serde_json::Value::Object(map) => {
            if let Some((input_tokens, output_tokens, total_tokens)) = object_usage_totals(map) {
                totals.record(input_tokens, output_tokens, total_tokens);
                return;
            }

            for child in map.values() {
                accumulate_token_usage_from_value(child, totals, depth + 1);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                accumulate_token_usage_from_value(item, totals, depth + 1);
            }
        }
        _ => {}
    }
}

fn codex_cumulative_snapshot(value: &serde_json::Value) -> Option<SessionTokenTotals> {
    let snapshot = match value.get("type").and_then(serde_json::Value::as_str) {
        Some("event_msg")
            if value
                .pointer("/payload/type")
                .and_then(serde_json::Value::as_str)
                == Some("token_count") =>
        {
            value.pointer("/payload/info/total_token_usage")
        }
        Some("token_usage_record") => value.pointer("/payload/thread_token_usage"),
        _ => None,
    }?;
    let (input, output, total) = object_usage_totals(snapshot.as_object()?)?;
    Some(SessionTokenTotals::from_snapshot(input, output, total))
}

pub fn read_codex_session_token_totals(path: &std::path::Path) -> SessionTokenTotals {
    let mut file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(_) => return SessionTokenTotals::default(),
    };
    let file_len = match file.metadata() {
        Ok(metadata) => metadata.len(),
        Err(_) => return SessionTokenTotals::default(),
    };
    let start = file_len.saturating_sub(CODEX_TOKEN_TAIL_BYTES);
    if file.seek(SeekFrom::Start(start)).is_err() {
        return SessionTokenTotals::default();
    }

    let mut tail = Vec::with_capacity((file_len - start) as usize);
    if file.read_to_end(&mut tail).is_err() {
        return SessionTokenTotals::default();
    }
    let complete_tail = if start == 0 {
        tail.as_slice()
    } else {
        tail.iter()
            .position(|byte| *byte == b'\n')
            .map(|index| &tail[index + 1..])
            .unwrap_or_default()
    };

    String::from_utf8_lossy(complete_tail)
        .lines()
        .rev()
        .filter(|line| line.contains("token_count") || line.contains("token_usage_record"))
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .find_map(|value| codex_cumulative_snapshot(&value))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn codex_reader_uses_latest_cumulative_snapshot_once() {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        let records = [
            serde_json::json!({
                "type": "token_usage_record",
                "payload": {
                    "usage": { "input_tokens": 25, "output_tokens": 5, "total_tokens": 30 },
                    "turn_token_usage": { "input_tokens": 25, "output_tokens": 5, "total_tokens": 30 },
                    "thread_token_usage": { "input_tokens": 25, "output_tokens": 5, "total_tokens": 30 }
                }
            }),
            serde_json::json!({
                "type": "event_msg",
                "payload": { "type": "token_count", "info": {
                    "total_token_usage": { "input_tokens": 80, "output_tokens": 20, "total_tokens": 100 },
                    "last_token_usage": { "input_tokens": 55, "output_tokens": 15, "total_tokens": 70 }
                }}
            }),
            serde_json::json!({
                "type": "event_msg",
                "payload": { "type": "token_count", "info": {
                    "total_token_usage": { "input_tokens": 180, "output_tokens": 40, "total_tokens": 220 },
                    "last_token_usage": { "input_tokens": 100, "output_tokens": 20, "total_tokens": 120 }
                }}
            }),
        ];
        for record in records {
            writeln!(file, "{record}").unwrap();
        }

        let totals = read_codex_session_token_totals(file.path());
        assert_eq!(totals.input_option(), Some(180));
        assert_eq!(totals.output_option(), Some(40));
        assert_eq!(totals.total_option(), Some(220));
    }

    #[test]
    fn codex_reader_finds_snapshot_without_parsing_the_whole_file() {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(&vec![b'x'; (CODEX_TOKEN_TAIL_BYTES + 1024) as usize])
            .unwrap();
        writeln!(file).unwrap();
        writeln!(
            file,
            "{}",
            serde_json::json!({
                "type": "token_usage_record",
                "payload": { "thread_token_usage": {
                    "input_tokens": "300", "output_tokens": 60, "total_tokens": 360
                }}
            })
        )
        .unwrap();

        let totals = read_codex_session_token_totals(file.path());
        assert_eq!(totals.input_option(), Some(300));
        assert_eq!(totals.output_option(), Some(60));
        assert_eq!(totals.total_option(), Some(360));
    }
}
