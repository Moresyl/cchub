use std::sync::{Arc, Mutex};

use bytes::Bytes;
use futures_util::{Stream, StreamExt};

use super::scan_stream_usage_buffer;
use crate::provider_proxy::ProxyUsageMetrics;
use crate::shared::gemini_usage::GeminiUsage;
use crate::shared::token_usage::InputTokenBasis;

#[derive(Clone, Default)]
pub(in crate::provider_proxy) struct UsageCapture(Arc<Mutex<ProxyUsageMetrics>>);

impl UsageCapture {
    pub(in crate::provider_proxy) fn snapshot(&self) -> ProxyUsageMetrics {
        self.0
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }

    fn update(&self, usage: &ProxyUsageMetrics) {
        *self.0.lock().unwrap_or_else(|error| error.into_inner()) = usage.clone();
    }
}

// Observe native readings before adapters can omit metadata-only events. The
// capture belongs to this request and contains counters, never response bodies.
pub(in crate::provider_proxy) fn capture_stream_usage<S, E>(
    stream: S,
    capture: UsageCapture,
    basis: InputTokenBasis,
) -> impl Stream<Item = Result<Bytes, E>> + Send
where
    S: Stream<Item = Result<Bytes, E>> + Send + 'static,
    E: Send + 'static,
{
    async_stream::stream! {
        let mut buffer = String::new();
        let mut usage = ProxyUsageMetrics::default();
        let mut gemini = GeminiUsage::default();
        tokio::pin!(stream);
        while let Some(chunk) = stream.next().await {
            if let Ok(bytes) = &chunk {
                let normalized = String::from_utf8_lossy(bytes).replace("\r\n", "\n");
                if scan_stream_usage_buffer(&mut buffer, &normalized, &mut usage, &mut gemini, basis) {
                    capture.update(&usage);
                }
                if buffer.len() > 1024 * 1024 { buffer.clear(); }
            }
            yield chunk;
        }
        if !buffer.trim().is_empty()
            && scan_stream_usage_buffer(&mut buffer, "\n\n", &mut usage, &mut gemini, basis) {
            capture.update(&usage);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn source_readings_survive_transport_errors_without_changing_wire_bytes() {
        let frame =
            "data: {\"usageMetadata\":{\"promptTokenCount\":7,\"candidatesTokenCount\":5}}\n\n";
        let source = futures_util::stream::iter([
            Ok(Bytes::from_static(frame.as_bytes())),
            Err(std::io::Error::new(
                std::io::ErrorKind::ConnectionReset,
                "fixture",
            )),
        ]);
        let capture = UsageCapture::default();
        let output = capture_stream_usage(source, capture.clone(), InputTokenBasis::IncludesCache)
            .collect::<Vec<_>>()
            .await;
        assert_eq!(output[0].as_ref().unwrap().as_ref(), frame.as_bytes());
        assert_eq!(
            output[1].as_ref().unwrap_err().kind(),
            std::io::ErrorKind::ConnectionReset
        );
        assert_eq!(
            (
                capture.snapshot().input_tokens,
                capture.snapshot().output_tokens
            ),
            (7, 5)
        );
    }

    #[tokio::test]
    async fn captures_are_request_local_and_preserve_unicode_after_normalization() {
        let capture = UsageCapture::default();
        let another_request = UsageCapture::default();
        let frame = "data: {\"modelVersion\":\"模型🦀\",\"usageMetadata\":{\"promptTokenCount\":7}}\r\n\r\n";
        let chunks = frame
            .as_bytes()
            .iter()
            .map(|byte| Ok::<_, std::io::Error>(Bytes::copy_from_slice(&[*byte])))
            .collect::<Vec<_>>();
        let normalized = crate::provider_proxy_transform::normalize_sse_stream(
            futures_util::stream::iter(chunks),
        );
        let output =
            capture_stream_usage(normalized, capture.clone(), InputTokenBasis::IncludesCache)
                .collect::<Vec<_>>()
                .await;
        assert!(output.iter().all(Result::is_ok));
        assert_eq!(capture.snapshot().response_model.as_deref(), Some("模型🦀"));
        assert_eq!(capture.snapshot().input_tokens, 7);
        assert_eq!(another_request.snapshot().input_tokens, 0);
        assert!(another_request.snapshot().response_model.is_none());
    }
}
