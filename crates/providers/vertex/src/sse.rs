//! A *data-only* SSE reader for the Vertex raw-prediction stream
//! (`docs/14` §9 item 28: the family's streaming answers carry `data:` lines
//! whose payload is one Anthropic stream-event JSON each; no `event:` name
//! lines; an optional `data: [DONE]` sentinel closes the transport).
//!
//! This is the vertex crate's own module — like the other adapters' wire
//! plumbing, it shares nothing with them beyond the `myharness-provider-sdk`
//! contract (the adapters are separate native packages; the anti-shim rule
//! keeps them from growing a shared gateway-shaped client).
//!
//! Shape tolerance: payloads are JSON objects, so one `data:` line is one
//! event (no multi-line joining is invented); a line that is not valid JSON
//! surfaces as an error rather than being skipped; SSE comments (leading
//! `:`) and blank lines are transport keep-alives that never count as
//! stream progress (`TEST-AI-016`).

use myharness_provider_sdk::{ErrorClass, ProviderError};

/// One parsed line of a data-only stream.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum SseItem {
    /// One `data:` line whose payload parsed as JSON.
    Data(serde_json::Value),
    /// A `data: [DONE]` sentinel (transport terminator).
    Sentinel,
    /// A comment or blank line — transport, not progress.
    Keepalive,
}

/// The incremental reader. Feed bytes; pull out whatever complete lines
/// have arrived.
#[derive(Debug, Clone, Default)]
pub(crate) struct DataOnlyParser {
    buffer: Vec<u8>,
}

impl DataOnlyParser {
    pub(crate) fn new() -> Self {
        DataOnlyParser {
            buffer: Vec::new(),
        }
    }

    /// Absorb one byte chunk and return every parsed line.
    pub(crate) fn feed(
        &mut self,
        chunk: &[u8],
    ) -> Vec<Result<SseItem, ProviderError>> {
        self.buffer.extend_from_slice(chunk);
        let mut output = Vec::new();
        while let Some(end) = self.buffer.iter().position(|byte| *byte == b'\n') {
            let mut line: Vec<u8> = self.buffer.drain(..=end).collect();
            // Trailing \n (drained above) and optional \r.
            if line.last() == Some(&b'\n') {
                line.pop();
            }
            if line.last() == Some(&b'\r') {
                line.pop();
            }
            let line = String::from_utf8_lossy(&line).into_owned();
            let line = line.trim_end_matches('\r');
            if line.is_empty() {
                output.push(Ok(SseItem::Keepalive));
                continue;
            }
            if let Some(comment) = line.strip_prefix(':') {
                // An SSE comment (e.g. a keep-alive ping): transport.
                let _ = comment;
                output.push(Ok(SseItem::Keepalive));
                continue;
            }
            let Some(data) = line.strip_prefix("data:") else {
                output.push(Err(ProviderError {
                    class: ErrorClass::UnsupportedModality,
                    status: None,
                    message: format!(
                        "the stream carried a {line:?} line where this family's data-only stream expects a data line"
                    ),
                }));
                continue;
            };
            // One optional leading space.
            let payload = data.strip_prefix(' ').unwrap_or(data);
            if payload.trim() == "[DONE]" {
                output.push(Ok(SseItem::Sentinel));
                continue;
            }
            match serde_json::from_str(payload) {
                Ok(value) => output.push(Ok(SseItem::Data(value))),
                Err(error) => output.push(Err(ProviderError {
                    class: ErrorClass::InvalidRequest,
                    status: None,
                    message: format!(
                        "the stream carried a data line that is not valid JSON: {error}"
                    ),
                })),
            }
        }
        output
    }
}
