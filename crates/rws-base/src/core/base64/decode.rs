//! The one base64 decoder: a lookup table per alphabet and a single pass
//! over the input, so decoding is linear in its length.
//!
//! Padding is optional. A `=` ends a quantum of two or three characters
//! (at most the padding that quantum needs); data after it starts a new
//! segment, so concatenated padded base64 decodes as a whole. A lone
//! trailing character (six bits, not a byte) is an error.

use std::fmt;

/// Which characters encode 62 and 63.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Alphabet {
    /// `+` and `/` (RFC 4648 §4).
    Standard,
    /// `-` and `_` (RFC 4648 §5).
    UrlSafe,
    /// Either pair, for input whose producer isn't known.
    Either,
}

/// A base64 decoder configuration.
///
/// ```
/// # extern crate rws_base as rust_web_server;
/// use rust_web_server::core::base64::Decoder;
///
/// assert_eq!(Decoder::STANDARD.decode("TWFu").unwrap(), b"Man");
/// assert_eq!(Decoder::URL_SAFE.decode("-_8").unwrap(), [0xfb, 0xff]);
/// assert_eq!(Decoder::STANDARD.skip_whitespace().decode("TW\r\nFu").unwrap(), b"Man");
/// assert!(Decoder::STANDARD.decode("TWF").is_ok());   // padding is optional
/// assert!(Decoder::STANDARD.decode("T").is_err());    // six bits aren't a byte
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Decoder {
    alphabet: Alphabet,
    whitespace: bool,
    skip_invalid: bool,
}

/// Why input isn't base64.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecodeError {
    /// Byte offset of the offending character, or the input length when it
    /// ends with a lone character.
    pub position: usize,
    reason: &'static str,
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} at byte {}", self.reason, self.position)
    }
}

impl std::error::Error for DecodeError {}

impl From<DecodeError> for String {
    fn from(e: DecodeError) -> String {
        e.to_string()
    }
}

const INVALID: u8 = 0xff;

const fn table(alphabet: Alphabet) -> [u8; 256] {
    let mut t = [INVALID; 256];
    let mut i = 0;
    while i < 26 {
        t[b'A' as usize + i] = i as u8;
        t[b'a' as usize + i] = 26 + i as u8;
        i += 1;
    }
    let mut d = 0;
    while d < 10 {
        t[b'0' as usize + d] = 52 + d as u8;
        d += 1;
    }
    if !matches!(alphabet, Alphabet::UrlSafe) {
        t[b'+' as usize] = 62;
        t[b'/' as usize] = 63;
    }
    if !matches!(alphabet, Alphabet::Standard) {
        t[b'-' as usize] = 62;
        t[b'_' as usize] = 63;
    }
    t
}

static STANDARD: [u8; 256] = table(Alphabet::Standard);
static URL_SAFE: [u8; 256] = table(Alphabet::UrlSafe);
static EITHER: [u8; 256] = table(Alphabet::Either);

impl Decoder {
    /// `+` and `/`, no whitespace.
    pub const STANDARD: Decoder = Decoder::new(Alphabet::Standard);
    /// `-` and `_`, no whitespace.
    pub const URL_SAFE: Decoder = Decoder::new(Alphabet::UrlSafe);
    /// Both alphabets, no whitespace.
    pub const EITHER: Decoder = Decoder::new(Alphabet::Either);

    /// A decoder for `alphabet` that refuses whitespace.
    pub const fn new(alphabet: Alphabet) -> Decoder {
        Decoder { alphabet, whitespace: false, skip_invalid: false }
    }

    /// Ignore ASCII whitespace anywhere (PEM bodies, wrapped lines).
    pub const fn skip_whitespace(self) -> Decoder {
        Decoder { whitespace: true, ..self }
    }

    /// Ignore every character outside the alphabet, padding included —
    /// only for callers that always did (`x509::base64_decode_std`).
    #[doc(hidden)]
    pub const fn skip_invalid(self) -> Decoder {
        Decoder { whitespace: true, skip_invalid: true, ..self }
    }

    /// Decode `input`.
    pub fn decode(&self, input: impl AsRef<[u8]>) -> Result<Vec<u8>, DecodeError> {
        let input = input.as_ref();
        let table = match self.alphabet {
            Alphabet::Standard => &STANDARD,
            Alphabet::UrlSafe => &URL_SAFE,
            Alphabet::Either => &EITHER,
        };
        let mut out = Vec::with_capacity(input.len() / 4 * 3 + 3);
        // Sextets of the current quantum, and how many.
        let mut quantum = 0u32;
        let mut n = 0u32;
        // Padding still allowed after a `=` (0 outside padding).
        let mut pads_left = 0u32;
        let mut padding = false;
        for (i, &c) in input.iter().enumerate() {
            let v = table[c as usize];
            if v != INVALID {
                padding = false;
                quantum = quantum << 6 | v as u32;
                n += 1;
                if n == 4 {
                    out.extend_from_slice(&[(quantum >> 16) as u8, (quantum >> 8) as u8, quantum as u8]);
                    quantum = 0;
                    n = 0;
                }
                continue;
            }
            if c == b'=' {
                if self.skip_invalid {
                    continue;
                }
                if padding {
                    if pads_left == 0 {
                        return Err(DecodeError { position: i, reason: "too much padding" });
                    }
                    pads_left -= 1;
                    continue;
                }
                if n < 2 {
                    return Err(DecodeError { position: i, reason: "padding where none belongs" });
                }
                flush(&mut out, quantum, n);
                pads_left = 3 - n;
                quantum = 0;
                n = 0;
                padding = true;
                continue;
            }
            if (self.whitespace && c.is_ascii_whitespace()) || self.skip_invalid {
                continue;
            }
            return Err(DecodeError { position: i, reason: "not a base64 character" });
        }
        if n == 1 {
            return Err(DecodeError { position: input.len(), reason: "a lone trailing character" });
        }
        flush(&mut out, quantum, n);
        Ok(out)
    }
}

/// The bytes of an unfinished quantum of `n` (2 or 3) sextets.
fn flush(out: &mut Vec<u8>, quantum: u32, n: u32) {
    match n {
        2 => out.push((quantum >> 4) as u8),
        3 => out.extend_from_slice(&[(quantum >> 10) as u8, (quantum >> 2) as u8]),
        _ => {}
    }
}
