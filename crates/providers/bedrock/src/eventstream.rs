//! The AWS event-stream binary transport this family streams over —
//! `invoke-with-response-stream` answers with `application/vnd.amazon.eventstream`
//! frames, not SSE (`docs/12` §4.1: the streamed surfaces are "SSE/event
//! stream"; this family's streaming member is the event-stream one).
//!
//! The frame grammar implemented here is the event-stream wire format:
//! a prelude (`message_length`, `headers_length`, both big-endian `u32`,
//! followed by a CRC-32C over the first eight bytes), a header block of
//! typed name/value pairs, a payload, and a trailing CRC-32C over the whole
//! message. The CRC polynomial is the event-stream one (Castagnoli,
//! reflected, init/final-xor `0xFFFFFFFF`), pinned here against the
//! published CRC-32C check value (`"123456789"` → `0xE3069283`).
//!
//! The framing spelling is part of the `docs/14` §9 approved decision for
//! this family (the shipped implementation's transport was never captured);
//! the tests pin it byte-for-byte, including a frame assembled by hand.

use myharness_provider_sdk::{ErrorClass, ProviderError};

/// A sane upper bound for one frame: the event-stream protocol caps messages
/// at well under a megabyte; a larger `message_length` is corruption, not a
/// big model event.
pub const MAX_FRAME_LEN: u32 = 1024 * 1024;

// ---------------------------------------------------------------------------
// CRC-32C (Castagnoli, reflected) — the event-stream CRC
// ---------------------------------------------------------------------------

/// The CRC-32C table (reflected polynomial `0x82F63B78`), built once.
fn crc32c_table() -> &'static [u32; 256] {
    use std::sync::OnceLock;
    static TABLE: OnceLock<[u32; 256]> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut table = [0u32; 256];
        for (index, slot) in table.iter_mut().enumerate() {
            let mut crc = index as u32;
            for _ in 0..8 {
                crc = if crc & 1 != 0 {
                    (crc >> 1) ^ 0x82F63B78
                } else {
                    crc >> 1
                };
            }
            *slot = crc;
        }
        table
    })
}

/// The CRC-32C of `data` — the exact event-stream CRC
/// (reflected, init and final XOR `0xFFFFFFFF`).
pub fn crc32c(data: &[u8]) -> u32 {
    let table = crc32c_table();
    let mut crc = 0xFFFF_FFFFu32;
    for byte in data {
        crc = (crc >> 8) ^ table[((crc ^ u32::from(*byte)) & 0xFF) as usize];
    }
    crc ^ 0xFFFF_FFFF
}

// ---------------------------------------------------------------------------
// Headers
// ---------------------------------------------------------------------------

/// One typed header value inside a frame's header block.
#[derive(Debug, Clone, PartialEq)]
pub enum HeaderValue {
    True,
    False,
    I8(i8),
    I16(i16),
    I32(i32),
    I64(i64),
    Bytes(Vec<u8>),
    Str(String),
    /// Milliseconds since the epoch.
    Timestamp(i64),
}

impl HeaderValue {
    /// The type byte on the wire.
    fn type_code(&self) -> u8 {
        match self {
            HeaderValue::True => 0,
            HeaderValue::False => 1,
            HeaderValue::I8(_) => 2,
            HeaderValue::I16(_) => 3,
            HeaderValue::I32(_) => 4,
            HeaderValue::I64(_) => 5,
            HeaderValue::Bytes(_) => 6,
            HeaderValue::Str(_) => 7,
            HeaderValue::Timestamp(_) => 8,
        }
    }

    /// The value bytes after the type byte (length included for types 6/7).
    fn value_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        match self {
            HeaderValue::True | HeaderValue::False => {}
            HeaderValue::I8(value) => out.extend_from_slice(&value.to_be_bytes()),
            HeaderValue::I16(value) => out.extend_from_slice(&value.to_be_bytes()),
            HeaderValue::I32(value) => out.extend_from_slice(&value.to_be_bytes()),
            HeaderValue::I64(value) => out.extend_from_slice(&value.to_be_bytes()),
            HeaderValue::Timestamp(value) => out.extend_from_slice(&value.to_be_bytes()),
            HeaderValue::Bytes(bytes) => {
                out.extend_from_slice(&(bytes.len() as u16).to_be_bytes());
                out.extend_from_slice(bytes);
            }
            HeaderValue::Str(value) => {
                out.extend_from_slice(&(value.len() as u16).to_be_bytes());
                out.extend_from_slice(value.as_bytes());
            }
        }
        out
    }

    /// Parse one value off a buffer; `None` when the buffer is too short or
    /// the type is outside the grammar.
    fn parse(type_code: u8, body: &[u8]) -> Option<(HeaderValue, usize)> {
        let read_u16 = |at: usize| -> Option<u16> {
            Some(u16::from_be_bytes([*body.get(at)?, *body.get(at + 1)?]))
        };
        match type_code {
            0 => Some((HeaderValue::True, 0)),
            1 => Some((HeaderValue::False, 0)),
            2 => Some((HeaderValue::I8(*body.first()? as i8), 1)),
            3 => Some((
                HeaderValue::I16(i16::from_be_bytes([*body.first()?, *body.get(1)?])),
                2,
            )),
            4 => Some((
                HeaderValue::I32(i32::from_be_bytes([
                    *body.first()?,
                    *body.get(1)?,
                    *body.get(2)?,
                    *body.get(3)?,
                ])),
                4,
            )),
            5 | 8 => {
                let mut bytes = [0u8; 8];
                for (index, slot) in bytes.iter_mut().enumerate() {
                    *slot = *body.get(index)?;
                }
                let value = i64::from_be_bytes(bytes);
                Some((
                    if type_code == 8 {
                        HeaderValue::Timestamp(value)
                    } else {
                        HeaderValue::I64(value)
                    },
                    8,
                ))
            }
            6 | 7 => {
                let length = read_u16(0)? as usize;
                let bytes = body.get(2..2 + length)?.to_vec();
                Some((
                    if type_code == 7 {
                        HeaderValue::Str(String::from_utf8(bytes).ok()?)
                    } else {
                        HeaderValue::Bytes(bytes)
                    },
                    2 + length,
                ))
            }
            _ => None,
        }
    }
}

/// A decoded or described frame header: one name and its typed value.
#[derive(Debug, Clone, PartialEq)]
pub struct Header {
    pub name: String,
    pub value: HeaderValue,
}

impl Header {
    pub fn new(name: &str, value: HeaderValue) -> Self {
        Header {
            name: name.to_string(),
            value,
        }
    }

    /// The header block bytes (`name_length`, name, `type`, value).
    fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.push(self.name.len() as u8);
        out.extend_from_slice(self.name.as_bytes());
        out.push(self.value.type_code());
        out.extend_from_slice(&self.value.value_bytes());
        out
    }
}

// ---------------------------------------------------------------------------
// Frames
// ---------------------------------------------------------------------------

/// One event-stream message. The headers carry the transport metadata
/// (`:message-type` `event`/`exception`, `:event-type`, `:exception-type`);
/// the payload carries the protocol event (this family's: the Anthropic
/// stream event JSON).
#[derive(Debug, Clone, PartialEq)]
pub struct Frame {
    pub headers: Vec<Header>,
    pub payload: Vec<u8>,
}

impl Frame {
    /// The `:message-type` header's string value, when present.
    pub fn message_type(&self) -> Option<&str> {
        self.header(":message-type")
    }

    /// The `:event-type` header's string value, when present.
    pub fn event_type(&self) -> Option<&str> {
        self.header(":event-type")
    }

    /// The `:exception-type` header's string value, when present.
    pub fn exception_type(&self) -> Option<&str> {
        self.header(":exception-type")
    }

    /// A string header by name.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find_map(|header| {
            if header.name == name {
                match &header.value {
                    HeaderValue::Str(value) => Some(value.as_str()),
                    _ => None,
                }
            } else {
                None
            }
        })
    }
}

/// Encode one frame. Public so the wire fixtures can build exact frames —
/// the CRC-32C vector above and the hand-assembled frame in the unit tests
/// pin the encoder and decoder against external ground truth.
pub fn encode_frame(headers: &[Header], payload: &[u8]) -> Vec<u8> {
    let mut header_block = Vec::new();
    for header in headers {
        header_block.extend_from_slice(&header.encode());
    }
    let headers_length = header_block.len() as u32;
    let total = (12 + header_block.len() + payload.len() + 4) as u32;
    let mut out = Vec::with_capacity(total as usize);
    out.extend_from_slice(&total.to_be_bytes());
    out.extend_from_slice(&headers_length.to_be_bytes());
    out.extend_from_slice(&crc32c(&out[..8]).to_be_bytes());
    out.extend_from_slice(&header_block);
    out.extend_from_slice(payload);
    let crc = crc32c(&out);
    out.extend_from_slice(&crc.to_be_bytes());
    out
}

// ---------------------------------------------------------------------------
// The incremental decoder
// ---------------------------------------------------------------------------

/// Byte-stream in, frames out — incremental, so a transport that delivers a
/// frame across many chunks still yields the frames exactly once each.
#[derive(Debug, Default)]
pub struct FrameDecoder {
    buffer: Vec<u8>,
}

/// A frame split for inspection: prelude, header block, payload.
struct Split {
    headers: Vec<Header>,
    payload: Vec<u8>,
}

impl FrameDecoder {
    pub fn new() -> Self {
        FrameDecoder { buffer: Vec::new() }
    }

    /// Feed one transport chunk; return every frame that completed.
    ///
    /// An error is malformed or corrupt framing — the caller surfaces it and
    /// stops; nothing is silently dropped (`AI-MM-01`).
    pub fn feed(&mut self, chunk: &[u8]) -> Vec<Result<Frame, ProviderError>> {
        self.buffer.extend_from_slice(chunk);
        let mut frames = Vec::new();
        loop {
            if self.buffer.len() < 12 {
                break;
            }
            let total = u32::from_be_bytes([
                self.buffer[0],
                self.buffer[1],
                self.buffer[2],
                self.buffer[3],
            ]);
            if !(16..=MAX_FRAME_LEN).contains(&total) {
                frames.push(Err(ProviderError {
                    class: ErrorClass::InvalidRequest,
                    status: None,
                    message: format!(
                        "the stream delivered an event-stream frame of an impossible length: {total}"
                    ),
                }));
                self.buffer.clear();
                break;
            }
            let total = total as usize;
            if self.buffer.len() < total {
                break;
            }
            let message = self.buffer.drain(..total).collect::<Vec<u8>>();
            match split_frame(&message) {
                Ok(split) => frames.push(Ok(Frame {
                    headers: split.headers,
                    payload: split.payload,
                })),
                Err(error) => {
                    frames.push(Err(error));
                    self.buffer.clear();
                    break;
                }
            }
        }
        frames
    }
}

/// Verify the CRCs and split one whole message into its pieces.
fn split_frame(message: &[u8]) -> Result<Split, ProviderError> {
    let malformed = |message: &str| ProviderError {
        class: ErrorClass::InvalidRequest,
        status: None,
        message: message.to_string(),
    };
    let total = usize::try_from(u32::from_be_bytes([
        message[0], message[1], message[2], message[3],
    ]))
    .unwrap_or_default();
    let headers_length = usize::try_from(u32::from_be_bytes([
        message[4], message[5], message[6], message[7],
    ]))
    .unwrap_or_default();
    if message.len() != total {
        return Err(malformed("the event-stream message length lied"));
    }
    let expected_prelude_crc =
        u32::from_be_bytes([message[8], message[9], message[10], message[11]]);
    if crc32c(&message[..8]) != expected_prelude_crc {
        return Err(malformed("the event-stream prelude failed its CRC check"));
    }
    let body_end = total - 4;
    let expected_crc = u32::from_be_bytes([
        message[body_end],
        message[body_end + 1],
        message[body_end + 2],
        message[body_end + 3],
    ]);
    if crc32c(&message[..body_end]) != expected_crc {
        return Err(malformed("the event-stream message failed its CRC check"));
    }
    let headers_end = 12 + headers_length;
    if headers_end > body_end {
        return Err(malformed(
            "the event-stream header block overruns the message",
        ));
    }
    let mut headers = Vec::new();
    let mut at = 12;
    while at < headers_end {
        let name_length = *message
            .get(at)
            .ok_or_else(|| malformed("an event-stream header block ended mid-header-name"))?
            as usize;
        at += 1;
        let name = String::from_utf8(
            message
                .get(at..at + name_length)
                .ok_or_else(|| malformed("an event-stream header name ran off the message"))?
                .to_vec(),
        )
        .map_err(|_| malformed("an event-stream header name was not UTF-8"))?;
        at += name_length;
        let type_code = *message
            .get(at)
            .ok_or_else(|| malformed("an event-stream header block ended mid-header-type"))?;
        at += 1;
        let (value, consumed) =
            HeaderValue::parse(type_code, message.get(at..headers_end).unwrap_or(&[]))
                .ok_or_else(|| malformed("an event-stream header value was malformed"))?;
        at += consumed;
        headers.push(Header { name, value });
    }
    let payload = message[headers_end..body_end].to_vec();
    Ok(Split { headers, payload })
}
