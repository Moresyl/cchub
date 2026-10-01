use bytes::Bytes;
use futures_util::{Stream, StreamExt};

#[path = "streaming_chat/json.rs"]
mod json;

// A compatibility repair must never impose a new response limit on raw relays.
// Oversized events pass through in bounded pieces; subsequent events are repaired.
const MAX_REPAIR_BYTES: usize = 8 * 1024 * 1024;
const PASS_CHUNK_BYTES: usize = 32 * 1024;

pub(super) fn repair_whole(bytes: Bytes) -> Bytes {
    if bytes.len() > MAX_REPAIR_BYTES {
        return bytes;
    }
    std::str::from_utf8(&bytes)
        .ok()
        .and_then(json::repair_whole)
        .map(Bytes::from)
        .unwrap_or(bytes)
}

pub(super) fn normalize<S>(stream: S) -> impl Stream<Item = Result<Bytes, std::io::Error>> + Send
where
    S: Stream<Item = Result<Bytes, std::io::Error>> + Send + 'static,
{
    normalize_limited(stream, MAX_REPAIR_BYTES)
}

// The input has already passed the shared UTF-8/CRLF decoder. Keep partial
// frames on EOF/error so the existing health and protocol termination layers
// remain responsible for reporting incomplete streams, without inventing DONE.
fn normalize_limited<S>(
    stream: S,
    limit: usize,
) -> impl Stream<Item = Result<Bytes, std::io::Error>> + Send
where
    S: Stream<Item = Result<Bytes, std::io::Error>> + Send + 'static,
{
    async_stream::stream! {
        let mut pending = Vec::new();
        let mut passing = false;
        let mut has_data = false;
        let mut previous_newline = false;
        tokio::pin!(stream);
        while let Some(chunk) = stream.next().await {
            let chunk = match chunk {
                Ok(chunk) => chunk,
                Err(error) => {
                    if !pending.is_empty() { yield Ok(Bytes::from(std::mem::take(&mut pending))); }
                    yield Err(error);
                    return;
                }
            };
            for byte in chunk {
                if !passing && pending.len() == limit {
                    yield Ok(Bytes::from(std::mem::take(&mut pending)));
                    passing = true;
                }
                pending.push(byte);
                let complete = previous_newline && byte == b'\n';
                previous_newline = byte == b'\n';
                if complete {
                    let frame = Bytes::from(std::mem::take(&mut pending));
                    yield Ok(if passing { frame } else { repair_frame(frame) });
                    passing = false;
                    has_data = false;
                    previous_newline = false;
                } else if passing && pending.len() == PASS_CHUNK_BYTES {
                    yield Ok(Bytes::from(std::mem::take(&mut pending)));
                } else if !passing && !has_data && byte == b'\n' {
                    let line = std::str::from_utf8(&pending).unwrap_or("")
                        .trim_start_matches('\u{feff}').trim_end_matches('\n');
                    has_data = line == "data" || line.starts_with("data:");
                    // Comment heartbeats and metadata do not require a blank
                    // line. Forward them immediately instead of accumulating
                    // them while a healthy upstream waits to emit actual data.
                    if !has_data { yield Ok(Bytes::from(std::mem::take(&mut pending))); }
                }
            }
            // Do not delay a large passthrough frame until the next network read.
            if passing && !pending.is_empty() {
                yield Ok(Bytes::from(std::mem::take(&mut pending)));
            }
        }
        if !pending.is_empty() { yield Ok(Bytes::from(pending)); }
    }
}

fn repair_frame(frame: Bytes) -> Bytes {
    let Some(repaired) = repaired_frame(&frame) else {
        return frame;
    };
    Bytes::from(repaired)
}

fn repaired_frame(frame: &Bytes) -> Option<String> {
    let text = std::str::from_utf8(frame).ok()?;
    let payload = crate::provider_proxy_transform::stream_frames::event_data(text)?;
    let repaired = json::repair(&payload)?;
    let mut output = String::with_capacity(text.len());
    let mut emitted = false;
    for (index, line) in text.split_inclusive('\n').enumerate() {
        let bare = line.trim_end_matches('\n');
        let bare = if index == 0 {
            bare.trim_start_matches('\u{feff}')
        } else {
            bare
        };
        if bare == "data" || bare.starts_with("data:") {
            if !emitted {
                if index == 0 && line.starts_with('\u{feff}') {
                    output.push('\u{feff}');
                }
                for part in repaired.split('\n') {
                    output.push_str("data: ");
                    output.push_str(part);
                    output.push('\n');
                }
                emitted = true;
            }
        } else {
            output.push_str(line);
        }
    }
    Some(output)
}

#[cfg(test)]
#[path = "streaming_chat/tests.rs"]
mod tests;
