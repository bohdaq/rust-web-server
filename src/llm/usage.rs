//! Token usage out of provider responses, as they stream past.
//!
//! [`UsageParser`] takes a response body in pieces of any size, as a JSON
//! document or as `text/event-stream`, and keeps the latest usage it saw.
//! It reads the shapes of OpenAI's Chat Completions (`usage.prompt_tokens`
//! / `completion_tokens`, in the final chunk of a stream), OpenAI's
//! Responses API (`usage.input_tokens` / `output_tokens`, nested under
//! `response` in its `response.completed` event) and Anthropic's Messages
//! API (`message_start` → `message.usage`, `message_delta` → `usage`, whose
//! `output_tokens` is cumulative). Anthropic's
//! `cache_creation_input_tokens` and `cache_read_input_tokens` count as
//! input: they are billed, so they count toward budgets.
//!
//! Loops only, every buffer bounded: provider responses are untrusted input.

use std::io::Read;
use std::time::{Duration, Instant};

use crate::mcp::json_rpc;

/// Tokens a response reported.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Usage {
    /// Prompt tokens, cache reads and writes included.
    pub input_tokens: u64,
    pub output_tokens: u64,
}

impl Usage {
    pub fn total(&self) -> u64 {
        self.input_tokens.saturating_add(self.output_tokens)
    }
}

/// The longest event-stream line kept; a longer one is skipped.
const MAX_LINE: usize = 1024 * 1024;
/// The largest JSON body parsed; past it the body is not read for usage.
const MAX_JSON: usize = 8 * 1024 * 1024;

/// Usage from a response body fed in pieces. See the [module docs](self).
#[derive(Debug)]
pub struct UsageParser {
    sse: bool,
    buffer: Vec<u8>,
    /// The current line (SSE) or body (JSON) grew past its cap.
    overflow: bool,
    usage: Option<Usage>,
}

impl UsageParser {
    /// `content_type` is the response's: `text/event-stream` parses events,
    /// anything else one JSON document.
    pub fn new(content_type: &str) -> UsageParser {
        UsageParser {
            sse: content_type.to_ascii_lowercase().contains("text/event-stream"),
            buffer: Vec::new(),
            overflow: false,
            usage: None,
        }
    }

    pub fn feed(&mut self, mut bytes: &[u8]) {
        if !self.sse {
            if !self.overflow && self.buffer.len() + bytes.len() <= MAX_JSON {
                self.buffer.extend_from_slice(bytes);
            } else {
                self.overflow = true;
                self.buffer = Vec::new();
            }
            return;
        }
        while let Some(end) = bytes.iter().position(|&b| b == b'\n') {
            if !self.overflow && self.buffer.len() + end <= MAX_LINE {
                self.buffer.extend_from_slice(&bytes[..end]);
                let line = std::mem::take(&mut self.buffer);
                self.line(&line);
            }
            self.buffer.clear();
            self.overflow = false;
            bytes = &bytes[end + 1..];
        }
        if self.overflow || self.buffer.len() + bytes.len() > MAX_LINE {
            self.overflow = true;
            self.buffer.clear();
        } else {
            self.buffer.extend_from_slice(bytes);
        }
    }

    /// The last usage the body reported, `None` if it reported none.
    pub fn finish(mut self) -> Option<Usage> {
        let rest = std::mem::take(&mut self.buffer);
        if !self.overflow {
            if self.sse {
                self.line(&rest);
            } else if let Ok(text) = std::str::from_utf8(&rest) {
                self.object(text);
            }
        }
        self.usage
    }

    fn line(&mut self, line: &[u8]) {
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        let Some(data) = line.strip_prefix(b"data:") else { return };
        let Ok(data) = std::str::from_utf8(data) else { return };
        let data = data.trim();
        if data.starts_with('{') {
            self.object(data);
        }
    }

    /// One event or body: its own `usage`, or one under `message` /
    /// `response`.
    fn object(&mut self, json: &str) {
        let Some(members) = json_rpc::object_members(json) else { return };
        for (key, value) in members {
            match key.as_str() {
                "usage" => self.apply(&value),
                "message" | "response" => {
                    if let Some(usage) = member(&value, "usage") {
                        self.apply(&usage);
                    }
                }
                _ => {}
            }
        }
    }

    /// Overwrite the counts a usage object names; keep the others.
    fn apply(&mut self, usage: &str) {
        let Some(members) = json_rpc::object_members(usage) else { return };
        let mut input = None::<u64>;
        let mut output = None::<u64>;
        for (key, value) in members {
            let Ok(n) = value.parse::<u64>() else { continue };
            match key.as_str() {
                "prompt_tokens" | "input_tokens" | "cache_creation_input_tokens" | "cache_read_input_tokens" => {
                    input = Some(input.unwrap_or(0).saturating_add(n));
                }
                "completion_tokens" | "output_tokens" => output = Some(n),
                _ => {}
            }
        }
        if input.is_none() && output.is_none() {
            return;
        }
        let usage = self.usage.get_or_insert_with(Usage::default);
        if let Some(n) = input {
            usage.input_tokens = n;
        }
        if let Some(n) = output {
            usage.output_tokens = n;
        }
    }
}

/// The raw value of top-level member `key` of object `json`.
pub(crate) fn member(json: &str, key: &str) -> Option<String> {
    json_rpc::object_members(json)?.into_iter().find(|(k, _)| k == key).map(|(_, v)| v)
}

/// HTTP/1.1 chunked framing removed from bytes fed in pieces.
#[derive(Debug, Default)]
pub(crate) struct ChunkDecoder {
    state: ChunkState,
    line: Vec<u8>,
}

#[derive(Debug, Default, PartialEq)]
enum ChunkState {
    #[default]
    Size,
    Data(u64),
    DataEnd,
    Trailers,
    Done,
}

/// The longest chunk-size or trailer line accepted.
const MAX_CHUNK_LINE: usize = 8 * 1024;

impl ChunkDecoder {
    pub(crate) fn done(&self) -> bool {
        self.state == ChunkState::Done
    }

    /// Append the data in `input` to `out`; an error for broken framing.
    pub(crate) fn decode(&mut self, mut input: &[u8], out: &mut Vec<u8>) -> Result<(), String> {
        while !input.is_empty() {
            match self.state {
                ChunkState::Data(left) => {
                    let n = (left.min(input.len() as u64)) as usize;
                    out.extend_from_slice(&input[..n]);
                    input = &input[n..];
                    self.state = if left == n as u64 { ChunkState::DataEnd } else { ChunkState::Data(left - n as u64) };
                }
                ChunkState::Done => return Ok(()),
                ChunkState::Size | ChunkState::DataEnd | ChunkState::Trailers => {
                    let Some(end) = input.iter().position(|&b| b == b'\n') else {
                        if self.line.len() + input.len() > MAX_CHUNK_LINE {
                            return Err("chunk line too long".to_string());
                        }
                        self.line.extend_from_slice(input);
                        return Ok(());
                    };
                    if self.line.len() + end > MAX_CHUNK_LINE {
                        return Err("chunk line too long".to_string());
                    }
                    self.line.extend_from_slice(&input[..end]);
                    input = &input[end + 1..];
                    let line = std::mem::take(&mut self.line);
                    let line = line.strip_suffix(b"\r").unwrap_or(&line);
                    self.state = match self.state {
                        ChunkState::DataEnd if line.is_empty() => ChunkState::Size,
                        ChunkState::DataEnd => return Err("chunk data longer than its size".to_string()),
                        ChunkState::Trailers if line.is_empty() => ChunkState::Done,
                        ChunkState::Trailers => ChunkState::Trailers,
                        _ => {
                            let size = line.split(|&b| b == b';').next().unwrap_or_default();
                            let size = std::str::from_utf8(size).ok().map(str::trim).filter(|s| !s.is_empty() && s.len() <= 16);
                            match size.and_then(|s| u64::from_str_radix(s, 16).ok()) {
                                Some(0) => ChunkState::Trailers,
                                Some(n) => ChunkState::Data(n),
                                None => return Err("bad chunk size".to_string()),
                            }
                        }
                    };
                }
            }
        }
        Ok(())
    }
}

/// What a finished response reports: its usage (if any) and the time from
/// the request to its first body byte.
pub(crate) type OnFinish = Box<dyn FnOnce(Option<Usage>, Option<Duration>) + Send>;

/// A streamed response body read through a [`UsageParser`]: the bytes pass
/// through (chunked framing removed when `chunked`) and `on_finish` runs
/// once, at the end of the body or when the reader is dropped early (a
/// client that went away still spent the tokens it got).
pub(crate) struct UsageReader {
    inner: Box<dyn Read + Send>,
    chunked: Option<ChunkDecoder>,
    parser: Option<UsageParser>,
    started: Instant,
    first_byte: Option<Duration>,
    pending: Vec<u8>,
    offset: usize,
    raw: Vec<u8>,
    on_finish: Option<OnFinish>,
}

impl UsageReader {
    pub(crate) fn new(inner: Box<dyn Read + Send>, content_type: &str, chunked: bool, started: Instant, on_finish: OnFinish) -> UsageReader {
        UsageReader {
            inner,
            chunked: chunked.then(ChunkDecoder::default),
            parser: Some(UsageParser::new(content_type)),
            started,
            first_byte: None,
            pending: Vec::new(),
            offset: 0,
            raw: vec![0; 16 * 1024],
            on_finish: Some(on_finish),
        }
    }

    fn finish(&mut self) {
        if let Some(on_finish) = self.on_finish.take() {
            let usage = self.parser.take().and_then(UsageParser::finish);
            on_finish(usage, self.first_byte);
        }
    }

    fn saw(first_byte: &mut Option<Duration>, started: Instant, parser: &mut Option<UsageParser>, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        first_byte.get_or_insert_with(|| started.elapsed());
        if let Some(parser) = parser {
            parser.feed(bytes);
        }
    }
}

impl Read for UsageReader {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        if out.is_empty() {
            return Ok(0);
        }
        loop {
            if self.offset < self.pending.len() {
                let n = out.len().min(self.pending.len() - self.offset);
                out[..n].copy_from_slice(&self.pending[self.offset..self.offset + n]);
                self.offset += n;
                return Ok(n);
            }
            if self.chunked.as_ref().is_some_and(ChunkDecoder::done) {
                self.finish();
                return Ok(0);
            }
            let Some(decoder) = &mut self.chunked else {
                let n = self.inner.read(out)?;
                if n == 0 {
                    self.finish();
                } else {
                    Self::saw(&mut self.first_byte, self.started, &mut self.parser, &out[..n]);
                }
                return Ok(n);
            };
            let n = self.inner.read(&mut self.raw)?;
            if n == 0 {
                self.finish();
                return Ok(0);
            }
            self.pending.clear();
            self.offset = 0;
            decoder
                .decode(&self.raw[..n], &mut self.pending)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
            Self::saw(&mut self.first_byte, self.started, &mut self.parser, &self.pending);
        }
    }
}

impl Drop for UsageReader {
    fn drop(&mut self) {
        self.finish();
    }
}
