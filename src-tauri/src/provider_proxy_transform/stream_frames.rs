use bytes::Bytes;
use futures_util::{Stream, StreamExt};

pub(super) const MAX_FRAME_BYTES: usize = 8 * 1024 * 1024;

pub(super) fn event_data(frame: &str) -> Option<String> {
    let fields = frame
        .trim_start_matches('\u{feff}')
        .lines()
        .filter_map(|line| super::strip_sse_field(line, "data"))
        .collect::<Vec<_>>();
    (!fields.is_empty()).then(|| fields.join("\n"))
}

#[derive(Debug)]
pub(super) enum FrameError {
    Limit,
    Utf8,
    Incomplete,
}

impl std::fmt::Display for FrameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Limit => "Upstream SSE event exceeded the 8 MiB limit",
            Self::Utf8 => "Upstream SSE event contains invalid UTF-8",
            Self::Incomplete => "Upstream SSE ended inside an incomplete event",
        })
    }
}

impl std::error::Error for FrameError {}

// Retain one bounded event, rather than a whole network chunk or an unfinished
// stream. A coalesced chunk can contain any number of individually valid events.
pub(super) fn frames<S, E>(stream: S) -> impl Stream<Item = Result<Bytes, std::io::Error>> + Send
where
    S: Stream<Item = Result<Bytes, E>> + Send + 'static,
    E: std::error::Error + Send + 'static,
{
    async_stream::stream! {
        let mut pending = Vec::new();
        let mut previous_cr = false;
        tokio::pin!(stream);
        while let Some(chunk) = stream.next().await {
            let chunk = match chunk {
                Ok(chunk) => chunk,
                Err(error) => { yield Err(std::io::Error::other(error.to_string())); return; }
            };
            for byte in chunk {
                if previous_cr {
                    previous_cr = false;
                    if byte == b'\n' { continue; }
                }
                let byte = if byte == b'\r' { previous_cr = true; b'\n' } else { byte };
                if pending.len() == MAX_FRAME_BYTES {
                    yield Err(std::io::Error::new(std::io::ErrorKind::InvalidData, FrameError::Limit));
                    return;
                }
                pending.push(byte);
                if pending.ends_with(b"\n\n") {
                    match String::from_utf8(std::mem::take(&mut pending)) {
                        Ok(event) => { yield Ok(Bytes::from(event)); }
                        Err(_) => {
                            yield Err(std::io::Error::new(std::io::ErrorKind::InvalidData, FrameError::Utf8));
                            return;
                        }
                    }
                }
            }
        }
        if pending.iter().any(|byte| !byte.is_ascii_whitespace()) {
            yield Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, FrameError::Incomplete));
        }
    }
}

#[cfg(test)]
mod tests;
