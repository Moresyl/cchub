pub(super) const MAX_BLOCKS: usize = 4096;
pub(super) const MAX_ID_BYTES: usize = 1024;
pub(super) const MAX_PENDING_ARGS: usize = 8 * 1024 * 1024;
pub(super) const BLOCK_LIMIT: &str = "Upstream stream exceeded the content block limit";

pub(super) fn allocate(next: &mut u32) -> Option<u32> {
    if *next as usize >= MAX_BLOCKS {
        return None;
    }
    let index = *next;
    *next += 1;
    Some(index)
}

pub(super) fn valid_response_ids(value: &serde_json::Value) -> bool {
    [
        "/item_id",
        "/call_id",
        "/name",
        "/item/id",
        "/item/call_id",
        "/item/name",
    ]
    .iter()
    .all(|path| {
        value
            .pointer(path)
            .and_then(serde_json::Value::as_str)
            .is_none_or(|id| id.len() <= MAX_ID_BYTES)
    })
}

#[cfg(test)]
mod tests;
