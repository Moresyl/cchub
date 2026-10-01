use bytes::Bytes;
use futures_util::{Stream, StreamExt};

// Network chunks need not end on UTF-8 or CRLF boundaries.
pub(crate) fn normalize_sse_stream<S, E>(
    stream: S,
) -> impl Stream<Item = Result<Bytes, std::io::Error>> + Send
where
    S: Stream<Item = Result<Bytes, E>> + Send + 'static,
    E: std::error::Error + Send + Sync + 'static,
{
    async_stream::stream! {
        let mut pending = Vec::new();
        let mut previous_cr = false;
        tokio::pin!(stream);
        while let Some(chunk) = stream.next().await {
            let chunk = match chunk {
                Ok(chunk) => chunk,
                Err(error) => { yield Err(std::io::Error::other(error)); return; }
            };
            pending.extend_from_slice(&chunk);
            let valid = match std::str::from_utf8(&pending) {
                Ok(_) => pending.len(),
                Err(error) if error.error_len().is_none() => error.valid_up_to(),
                Err(_) => {
                    yield Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Upstream SSE contains invalid UTF-8"));
                    return;
                }
            };
            if valid > 0 {
                let text = std::str::from_utf8(&pending[..valid]).expect("validated UTF-8 prefix");
                let mut normalized = String::with_capacity(text.len());
                for character in text.chars() {
                    if previous_cr {
                        previous_cr = false;
                        if character == '\n' { continue; }
                    }
                    if character == '\r' {
                        normalized.push('\n');
                        previous_cr = true;
                    } else { normalized.push(character); }
                }
                pending.drain(..valid);
                if !normalized.is_empty() { yield Ok(Bytes::from(normalized)); }
            }
        }
        if !pending.is_empty() {
            yield Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "Upstream SSE ended inside a UTF-8 character"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn every_byte_split_preserves_unicode_and_normalizes_line_endings() {
        let input = "data: 你好🦀\r\n\r\ndata: next\r\rdata: last\n\n";
        let chunks = input
            .as_bytes()
            .iter()
            .map(|byte| Ok::<_, std::io::Error>(Bytes::copy_from_slice(&[*byte])))
            .collect::<Vec<_>>();
        let output = normalize_sse_stream(futures_util::stream::iter(chunks))
            .collect::<Vec<_>>()
            .await;
        let text: Vec<u8> = output
            .into_iter()
            .flat_map(|bytes| bytes.unwrap().to_vec())
            .collect();
        assert_eq!(
            String::from_utf8(text).unwrap(),
            "data: 你好🦀\n\ndata: next\n\ndata: last\n\n"
        );
    }

    #[tokio::test]
    async fn invalid_or_truncated_utf8_never_becomes_replacement_characters() {
        for input in [vec![0xff], vec![0xe4, 0xbd]] {
            let stream = futures_util::stream::iter([Ok::<_, std::io::Error>(Bytes::from(input))]);
            let output = normalize_sse_stream(stream).collect::<Vec<_>>().await;
            assert_eq!(output.len(), 1);
            assert!(output[0].is_err());
        }
    }
}
