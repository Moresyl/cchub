use serde_json::{json, Value};
use std::collections::HashMap;

const MAX_PARTS: usize = 4096;
const MAX_ITEM_ID_BYTES: usize = 1024;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum Owner {
    Item(String),
    Output(u64),
    Missing,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Kind {
    Summary,
    Content,
    Legacy,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct PartKey {
    owner: Owner,
    kind: Kind,
    index: u64,
}

struct Block {
    index: u32,
    open: bool,
    has_text: bool,
}

#[derive(Default)]
pub(super) struct ReasoningBlocks {
    blocks: HashMap<PartKey, Block>,
}

impl ReasoningBlocks {
    pub(super) fn handles(event: &str) -> bool {
        matches!(
            event,
            "response.reasoning_summary_part.added"
                | "response.reasoning_summary_part.done"
                | "response.reasoning_summary_text.delta"
                | "response.reasoning_summary_text.done"
                | "response.reasoning_text.delta"
                | "response.reasoning_text.done"
                | "response.reasoning.delta"
                | "response.reasoning.done"
        )
    }

    // Keep only identity and lifecycle state, never accumulated reasoning text.
    pub(super) fn handle(
        &mut self,
        event: &str,
        data: &Value,
        next_index: &mut u32,
    ) -> Result<Vec<Value>, &'static str> {
        let added = event.ends_with(".added");
        let done = event.ends_with(".done");
        let text = if event.contains("_part.") {
            if data.pointer("/part/type").and_then(Value::as_str) != Some("summary_text") {
                return Ok(Vec::new());
            }
            data.pointer("/part/text").and_then(Value::as_str)
        } else if done {
            data.get("text").and_then(Value::as_str)
        } else {
            data.get("delta")
                .and_then(Value::as_str)
                .or_else(|| data.get("text").and_then(Value::as_str))
        };
        if !added && !done && text.is_none_or(str::is_empty) {
            return Ok(Vec::new());
        }
        let key = part_key(event, data)?;
        if !self.blocks.contains_key(&key) {
            if done && text.is_none_or(str::is_empty) {
                return Ok(Vec::new());
            }
            if self.blocks.len() >= MAX_PARTS {
                return Err("Upstream reasoning stream exceeded the part limit");
            }
            let index = super::stream_limits::allocate(next_index)
                .ok_or(super::stream_limits::BLOCK_LIMIT)?;
            self.blocks.insert(
                key.clone(),
                Block {
                    index,
                    open: false,
                    has_text: false,
                },
            );
            let block = self.blocks.get_mut(&key).expect("inserted reasoning block");
            block.open = true;
            let mut events = vec![json!({"type":"content_block_start", "index":block.index,
                "content_block":{"type":"thinking", "thinking":""}})];
            append_text(block, text, &mut events);
            if done {
                close(block, &mut events);
            }
            return Ok(events);
        }
        let block = self.blocks.get_mut(&key).expect("existing reasoning block");
        if !block.open {
            return Ok(Vec::new());
        }
        let mut events = Vec::new();
        // Added/done snapshots repeat the text already emitted by deltas.
        if !added && (!done || !block.has_text) {
            append_text(block, text, &mut events);
        }
        if done {
            close(block, &mut events);
        }
        Ok(events)
    }

    pub(super) fn close_all(&mut self) -> Vec<Value> {
        let mut blocks = self.blocks.values_mut().collect::<Vec<_>>();
        blocks.sort_unstable_by_key(|block| block.index);
        let mut events = Vec::new();
        for block in blocks {
            close(block, &mut events);
        }
        events
    }

    pub(super) fn finish_item(
        &mut self,
        data: &Value,
        next_index: &mut u32,
    ) -> Result<Vec<Value>, &'static str> {
        let Some(item) = data.get("item") else {
            return Ok(Vec::new());
        };
        let summary = item.get("summary").and_then(Value::as_array);
        let (parts, kind, event, field) = if summary.is_some_and(|parts| {
            parts.iter().any(|part| {
                part.get("type").and_then(Value::as_str) == Some("summary_text")
                    && part
                        .get("text")
                        .and_then(Value::as_str)
                        .is_some_and(|text| !text.is_empty())
            })
        }) {
            (
                summary,
                "summary_text",
                "response.reasoning_summary_text.done",
                "summary_index",
            )
        } else {
            (
                item.get("content").and_then(Value::as_array),
                "reasoning_text",
                "response.reasoning_text.done",
                "content_index",
            )
        };
        let mut events = Vec::new();
        for (index, part) in parts.into_iter().flatten().enumerate() {
            if part.get("type").and_then(Value::as_str) != Some(kind) {
                continue;
            }
            let done = json!({
                "item_id":item.get("id").or_else(|| data.get("item_id")),
                "output_index":data.get("output_index"), field:index,
                "text":part.get("text")
            });
            events.extend(self.handle(event, &done, next_index)?);
        }
        let identity = json!({"item_id":item.get("id").or_else(|| data.get("item_id")), "output_index":data.get("output_index")});
        let owner = part_key("response.reasoning.done", &identity)?.owner;
        let mut blocks = self
            .blocks
            .iter_mut()
            .filter_map(|(key, block)| (key.owner == owner).then_some(block))
            .collect::<Vec<_>>();
        blocks.sort_unstable_by_key(|block| block.index);
        for block in blocks {
            close(block, &mut events);
        }
        Ok(events)
    }
}

fn part_key(event: &str, data: &Value) -> Result<PartKey, &'static str> {
    let owner = if let Some(id) = data
        .get("item_id")
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty())
    {
        if id.len() > MAX_ITEM_ID_BYTES {
            return Err("Upstream reasoning stream exceeded the identity limit");
        }
        Owner::Item(id.to_owned())
    } else if let Some(index) = data.get("output_index").and_then(Value::as_u64) {
        Owner::Output(index)
    } else {
        Owner::Missing
    };
    let (kind, index_field) = if event.starts_with("response.reasoning_summary_") {
        (Kind::Summary, "summary_index")
    } else if event.starts_with("response.reasoning_text.") {
        (Kind::Content, "content_index")
    } else {
        (Kind::Legacy, "content_index")
    };
    Ok(PartKey {
        owner,
        kind,
        index: data.get(index_field).and_then(Value::as_u64).unwrap_or(0),
    })
}

fn append_text(block: &mut Block, text: Option<&str>, events: &mut Vec<Value>) {
    if let Some(text) = text.filter(|text| !text.is_empty()) {
        events.push(json!({"type":"content_block_delta", "index":block.index,
            "delta":{"type":"thinking_delta", "thinking":text}}));
        block.has_text = true;
    }
}

fn close(block: &mut Block, events: &mut Vec<Value>) {
    if block.open {
        events.push(json!({"type":"content_block_stop", "index":block.index}));
        block.open = false;
    }
}

pub(super) fn whole_reasoning_text(item: &Value) -> String {
    let texts = |field: &str, kind: &str| {
        item.get(field)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter(|part| part.get("type").and_then(Value::as_str) == Some(kind))
            .filter_map(|part| part.get("text").and_then(Value::as_str))
            .collect::<String>()
    };
    let summary = texts("summary", "summary_text");
    if summary.is_empty() {
        texts("content", "reasoning_text")
    } else {
        summary
    }
}

#[cfg(test)]
mod tests;
