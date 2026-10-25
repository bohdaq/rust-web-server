//! HPACK (RFC 7541): the header compression of HTTP/2.
//!
//! [`Decoder`] reads request header blocks: static and dynamic table,
//! Huffman strings, dynamic-table size updates (only at the start of a block,
//! never above our `SETTINGS_HEADER_TABLE_SIZE`). Every length and index is
//! checked before use — the input comes from the client — and decoding is a
//! loop over representations, never recursive.
//!
//! [`Encoder`] writes response header blocks: `:status` from the static
//! table, a header seen before as a one-byte index into the dynamic table,
//! values that change on every response (dates, ETags) as literals that are
//! never indexed (indexing them would evict the headers that do repeat).
//! Strings are sent as raw octets, not Huffman-coded: valid for every
//! decoder, and no per-response encoding work.

use std::collections::{HashMap, VecDeque};

use super::huffman;

/// RFC 7541 Appendix A.
pub(crate) const STATIC_TABLE: [(&str, &str); 61] = [
    (":authority", ""),
    (":method", "GET"),
    (":method", "POST"),
    (":path", "/"),
    (":path", "/index.html"),
    (":scheme", "http"),
    (":scheme", "https"),
    (":status", "200"),
    (":status", "204"),
    (":status", "206"),
    (":status", "304"),
    (":status", "400"),
    (":status", "404"),
    (":status", "500"),
    ("accept-charset", ""),
    ("accept-encoding", "gzip, deflate"),
    ("accept-language", ""),
    ("accept-ranges", ""),
    ("accept", ""),
    ("access-control-allow-origin", ""),
    ("age", ""),
    ("allow", ""),
    ("authorization", ""),
    ("cache-control", ""),
    ("content-disposition", ""),
    ("content-encoding", ""),
    ("content-language", ""),
    ("content-length", ""),
    ("content-location", ""),
    ("content-range", ""),
    ("content-type", ""),
    ("cookie", ""),
    ("date", ""),
    ("etag", ""),
    ("expect", ""),
    ("expires", ""),
    ("from", ""),
    ("host", ""),
    ("if-match", ""),
    ("if-modified-since", ""),
    ("if-none-match", ""),
    ("if-range", ""),
    ("if-unmodified-since", ""),
    ("last-modified", ""),
    ("link", ""),
    ("location", ""),
    ("max-forwards", ""),
    ("proxy-authenticate", ""),
    ("proxy-authorization", ""),
    ("range", ""),
    ("referer", ""),
    ("refresh", ""),
    ("retry-after", ""),
    ("server", ""),
    ("set-cookie", ""),
    ("strict-transport-security", ""),
    ("transfer-encoding", ""),
    ("user-agent", ""),
    ("vary", ""),
    ("via", ""),
    ("www-authenticate", ""),
];

/// Per-entry overhead counted against the table size (§4.1).
const ENTRY_OVERHEAD: usize = 32;

/// The table size both sides start with (the protocol default).
pub(crate) const DEFAULT_TABLE_SIZE: usize = 4096;

/// Why a header block was rejected.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum DecodeError {
    /// Malformed HPACK: a connection error (`COMPRESSION_ERROR`).
    Compression,
    /// Decoded fine but larger than the header-list limit: the stream is
    /// refused, the connection lives on (the table was still updated).
    ListTooLarge,
}

/// A dynamic table: newest entry first.
struct Table {
    entries: VecDeque<(Vec<u8>, Vec<u8>)>,
    size: usize,
    max_size: usize,
}

impl Table {
    fn new(max_size: usize) -> Self {
        Table { entries: VecDeque::new(), size: 0, max_size }
    }

    fn set_max_size(&mut self, max_size: usize) {
        self.max_size = max_size;
        self.evict(0);
    }

    /// Evict the oldest entries until `incoming` more bytes fit.
    fn evict(&mut self, incoming: usize) {
        while self.size + incoming > self.max_size {
            match self.entries.pop_back() {
                Some((n, v)) => self.size -= n.len() + v.len() + ENTRY_OVERHEAD,
                None => break,
            }
        }
    }

    /// Insert an entry; one larger than the whole table empties it (§4.4).
    fn insert(&mut self, name: Vec<u8>, value: Vec<u8>) {
        let size = name.len() + value.len() + ENTRY_OVERHEAD;
        self.evict(size);
        if size <= self.max_size {
            self.size += size;
            self.entries.push_front((name, value));
        }
    }

    /// 1-based HPACK index (static first, then dynamic).
    fn get(&self, index: usize) -> Option<(&[u8], &[u8])> {
        match index {
            0 => None,
            1..=61 => {
                let (n, v) = STATIC_TABLE[index - 1];
                Some((n.as_bytes(), v.as_bytes()))
            }
            _ => self.entries.get(index - 62).map(|(n, v)| (&n[..], &v[..])),
        }
    }
}

/// Decodes request header blocks; one per connection.
pub(crate) struct Decoder {
    table: Table,
    /// Our `SETTINGS_HEADER_TABLE_SIZE`: the most a size update may ask for.
    limit: usize,
}

impl Decoder {
    pub(crate) fn new(limit: usize) -> Self {
        Decoder { table: Table::new(limit), limit }
    }

    /// Decode a complete header block, calling `emit(name, value)` for each
    /// field in order. `max_list` bounds the decoded size (`name + value +
    /// 32` per field, as `SETTINGS_MAX_HEADER_LIST_SIZE` counts it); past it
    /// the block is still decoded — the table must stay in step with the
    /// client's — but nothing more is emitted and `ListTooLarge` returned.
    pub(crate) fn decode(
        &mut self,
        block: &[u8],
        max_list: usize,
        mut emit: impl FnMut(&[u8], &[u8]),
    ) -> Result<(), DecodeError> {
        let mut pos = 0;
        let mut list_size = 0usize;
        let mut too_large = false;
        let mut fields_seen = false;
        let mut name_buf = Vec::new();
        let mut value_buf = Vec::new();
        while pos < block.len() {
            let byte = block[pos];
            if byte & 0x80 != 0 {
                // Indexed field.
                let index = integer(block, &mut pos, 7)?;
                let (n, v) = self.table.get(index).ok_or(DecodeError::Compression)?;
                fields_seen = true;
                list_size += n.len() + v.len() + ENTRY_OVERHEAD;
                too_large |= list_size > max_list;
                if !too_large {
                    emit(n, v);
                }
            } else if byte & 0xe0 == 0x20 {
                // Dynamic table size update: only before the first field.
                if fields_seen {
                    return Err(DecodeError::Compression);
                }
                let size = integer(block, &mut pos, 5)?;
                if size > self.limit {
                    return Err(DecodeError::Compression);
                }
                self.table.set_max_size(size);
            } else {
                // Literal: with incremental indexing (01), without (0000) or
                // never indexed (0001).
                let (prefix, index_it) = if byte & 0xc0 == 0x40 { (6, true) } else { (4, false) };
                let name_index = integer(block, &mut pos, prefix)?;
                name_buf.clear();
                if name_index == 0 {
                    string(block, &mut pos, &mut name_buf)?;
                } else {
                    let (n, _) = self.table.get(name_index).ok_or(DecodeError::Compression)?;
                    name_buf.extend_from_slice(n);
                }
                value_buf.clear();
                string(block, &mut pos, &mut value_buf)?;
                fields_seen = true;
                list_size += name_buf.len() + value_buf.len() + ENTRY_OVERHEAD;
                too_large |= list_size > max_list;
                if !too_large {
                    emit(&name_buf, &value_buf);
                }
                if index_it {
                    self.table.insert(name_buf.clone(), value_buf.clone());
                }
            }
        }
        if too_large {
            Err(DecodeError::ListTooLarge)
        } else {
            Ok(())
        }
    }
}

/// An HPACK integer with an `n`-bit prefix (§5.1), advancing `pos`.
fn integer(input: &[u8], pos: &mut usize, n: u32) -> Result<usize, DecodeError> {
    let mask = (1u32 << n) - 1;
    let first = *input.get(*pos).ok_or(DecodeError::Compression)? as u32 & mask;
    *pos += 1;
    if first < mask {
        return Ok(first as usize);
    }
    let mut value = mask as u64;
    let mut shift = 0u32;
    loop {
        let byte = *input.get(*pos).ok_or(DecodeError::Compression)?;
        *pos += 1;
        value += ((byte & 0x7f) as u64) << shift;
        // Nothing legitimate needs more than 2^28; stop before overflowing.
        if value > (1 << 28) {
            return Err(DecodeError::Compression);
        }
        if byte & 0x80 == 0 {
            return Ok(value as usize);
        }
        shift += 7;
        // Zero-valued continuation bytes (0x80) don't grow `value` but do
        // grow the shift: bound it too.
        if shift > 28 {
            return Err(DecodeError::Compression);
        }
    }
}

/// An HPACK string literal (§5.2) appended to `out`, advancing `pos`.
fn string(input: &[u8], pos: &mut usize, out: &mut Vec<u8>) -> Result<(), DecodeError> {
    let huffman_coded = *input.get(*pos).ok_or(DecodeError::Compression)? & 0x80 != 0;
    let len = integer(input, pos, 7)?;
    let end = pos.checked_add(len).filter(|end| *end <= input.len()).ok_or(DecodeError::Compression)?;
    let raw = &input[*pos..end];
    *pos = end;
    if huffman_coded {
        huffman::decode(raw, out).map_err(|_| DecodeError::Compression)
    } else {
        out.extend_from_slice(raw);
        Ok(())
    }
}

/// Write an HPACK integer with an `n`-bit prefix; `first` holds the
/// representation's flag bits.
pub(crate) fn write_integer(out: &mut Vec<u8>, first: u8, n: u32, value: usize) {
    let mask = (1usize << n) - 1;
    if value < mask {
        out.push(first | value as u8);
        return;
    }
    out.push(first | mask as u8);
    let mut rest = value - mask;
    while rest >= 0x80 {
        out.push((rest as u8 & 0x7f) | 0x80);
        rest >>= 7;
    }
    out.push(rest as u8);
}

/// A string literal as raw octets (no Huffman).
fn write_string(out: &mut Vec<u8>, s: &[u8]) {
    write_integer(out, 0, 7, s.len());
    out.extend_from_slice(s);
}

/// The static-table index of a (lower-case) name, if it has one.
fn static_name_index(name: &[u8]) -> Option<usize> {
    STATIC_TABLE.iter().position(|(n, _)| n.as_bytes() == name).map(|i| i + 1)
}

/// Encodes response header blocks; one per connection.
pub(crate) struct Encoder {
    table: Table,
    /// Insertion number of each dynamic entry, keyed by name + NUL + value:
    /// its index is `62 + (inserted - 1 - number)`.
    positions: HashMap<Vec<u8>, u64>,
    inserted: u64,
    /// A size update to send at the start of the next block (§4.2).
    pending_size_update: Option<usize>,
    key: Vec<u8>,
}

impl Encoder {
    pub(crate) fn new() -> Self {
        Encoder {
            table: Table::new(DEFAULT_TABLE_SIZE),
            positions: HashMap::new(),
            inserted: 0,
            pending_size_update: None,
            key: Vec::new(),
        }
    }

    /// The client's `SETTINGS_HEADER_TABLE_SIZE` changed: use at most that
    /// (and never more than the default) and tell the client's decoder.
    pub(crate) fn set_peer_table_size(&mut self, size: usize) {
        let size = size.min(DEFAULT_TABLE_SIZE);
        if size != self.table.max_size || self.pending_size_update.is_some() {
            self.table.set_max_size(size);
            self.forget_evicted();
            self.pending_size_update = Some(size);
        }
    }

    /// Start a header block.
    pub(crate) fn begin(&mut self, out: &mut Vec<u8>) {
        if let Some(size) = self.pending_size_update.take() {
            write_integer(out, 0x20, 5, size);
        }
    }

    pub(crate) fn status(&mut self, out: &mut Vec<u8>, status: u16) {
        let index = match status {
            200 => 8,
            204 => 9,
            206 => 10,
            304 => 11,
            400 => 12,
            404 => 13,
            500 => 14,
            _ => 0,
        };
        if index != 0 {
            write_integer(out, 0x80, 7, index);
        } else {
            let mut digits = [b'0'; 3];
            let s = status.min(999);
            digits[0] += (s / 100) as u8;
            digits[1] += (s / 10 % 10) as u8;
            digits[2] += (s % 10) as u8;
            write_integer(out, 0x00, 4, 8);
            write_string(out, &digits);
        }
    }

    /// One field; `name` lower-case. `volatile` values (changing on every
    /// response) are never added to the table.
    pub(crate) fn field(&mut self, out: &mut Vec<u8>, name: &[u8], value: &[u8], volatile: bool) {
        self.key.clear();
        self.key.extend_from_slice(name);
        self.key.push(0);
        self.key.extend_from_slice(value);
        if let Some(&number) = self.positions.get(&self.key) {
            write_integer(out, 0x80, 7, 62 + (self.inserted - 1 - number) as usize);
            return;
        }
        let name_index = static_name_index(name);
        let size = name.len() + value.len() + ENTRY_OVERHEAD;
        let index_it = !volatile && size <= self.table.max_size / 2;
        let (first, prefix) = if index_it { (0x40, 6) } else { (0x00, 4) };
        match name_index {
            Some(i) => write_integer(out, first, prefix, i),
            None => {
                write_integer(out, first, prefix, 0);
                write_string(out, name);
            }
        }
        write_string(out, value);
        if index_it {
            self.table.insert(name.to_vec(), value.to_vec());
            self.positions.insert(self.key.clone(), self.inserted);
            self.inserted += 1;
            self.forget_evicted();
        }
    }

    /// Drop position entries for evicted table entries.
    fn forget_evicted(&mut self) {
        let oldest = self.inserted - self.table.entries.len() as u64;
        if self.positions.len() > self.table.entries.len() {
            self.positions.retain(|_, number| *number >= oldest);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode_all(decoder: &mut Decoder, block: &[u8]) -> Result<Vec<(String, String)>, DecodeError> {
        let mut fields = Vec::new();
        decoder.decode(block, 1 << 20, |n, v| {
            fields.push((String::from_utf8_lossy(n).to_string(), String::from_utf8_lossy(v).to_string()))
        })?;
        Ok(fields)
    }

    fn hex(s: &str) -> Vec<u8> {
        let s: String = s.chars().filter(|c| !c.is_whitespace()).collect();
        (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
    }

    /// RFC 7541 C.3 (requests without Huffman) and C.4 (with), three
    /// requests each sharing one decoder.
    #[test]
    fn rfc_7541_request_examples() {
        for blocks in [
            ["828684410f7777772e6578616d706c652e636f6d", "828684be58086e6f2d6361636865", "828785bf400a637573746f6d2d6b65790c637573746f6d2d76616c7565"],
            ["828684418cf1e3c2e5f23a6ba0ab90f4ff", "828684be5886a8eb10649cbf", "828785bf408825a849e95ba97d7f8925a849e95bb8e8b4bf"],
        ] {
            let mut d = Decoder::new(DEFAULT_TABLE_SIZE);
            let first = decode_all(&mut d, &hex(blocks[0])).unwrap();
            assert_eq!(first[3], (":authority".to_string(), "www.example.com".to_string()));
            let second = decode_all(&mut d, &hex(blocks[1])).unwrap();
            assert_eq!(second[3], (":authority".to_string(), "www.example.com".to_string()));
            assert_eq!(second[4], ("cache-control".to_string(), "no-cache".to_string()));
            let third = decode_all(&mut d, &hex(blocks[2])).unwrap();
            assert_eq!(third[1], (":scheme".to_string(), "https".to_string()));
            assert_eq!(third[3], (":authority".to_string(), "www.example.com".to_string()));
            assert_eq!(third[4], ("custom-key".to_string(), "custom-value".to_string()));
            assert_eq!(d.table.size, 164);
        }
    }

    #[test]
    fn malformed_blocks_are_compression_errors() {
        for bad in [
            &[0x80][..],                // index 0
            &[0xbe][..],                // index 62 with an empty dynamic table
            &[0x41, 0x05, b'a'][..],    // string longer than the block
            &[0xff, 0xff, 0xff, 0xff, 0xff, 0x0f][..], // integer overflow
            &[0xff, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x01][..], // endless zero continuations (fuzz)
            &[0x82, 0x3f, 0xe1, 0x1f][..], // size update after a field
            &[0x3f, 0xe2, 0x1f][..],    // size update above the limit (4097)
            &[0x40, 0x81, 0x00, 0x00][..], // Huffman name with bad padding
        ] {
            let mut d = Decoder::new(DEFAULT_TABLE_SIZE);
            assert_eq!(Err(DecodeError::Compression), decode_all(&mut d, bad).map(|_| ()), "{bad:02x?}");
        }
    }

    #[test]
    fn oversized_list_is_refused_but_still_indexed() {
        let mut d = Decoder::new(DEFAULT_TABLE_SIZE);
        let mut block = Vec::new();
        block.push(0x40);
        write_string(&mut block, b"x-big");
        write_string(&mut block, &[b'a'; 200]);
        let mut seen = 0;
        assert_eq!(Err(DecodeError::ListTooLarge), d.decode(&block, 100, |_, _| seen += 1));
        assert_eq!(0, seen);
        // The entry made it into the table: the next block can index it.
        let fields = decode_all(&mut d, &[0xbe]).unwrap();
        assert_eq!("x-big", fields[0].0);
    }

    /// Our encoder's blocks decode to what went in, repeated fields become
    /// one-byte indexes, volatile ones stay out of the table, and a smaller
    /// peer table size is announced and respected.
    #[test]
    fn encoder_round_trips_and_indexes_repeats() {
        let mut e = Encoder::new();
        let mut d = Decoder::new(DEFAULT_TABLE_SIZE);
        let fields: [(&str, &str, bool); 4] = [("vary", "Origin", false), ("x-custom", "1", false), ("date", "Sun, 25 Oct 2026 10:00:00 GMT", true), ("content-type", "text/html", false)];
        let mut sizes = Vec::new();
        for round in 0..3 {
            let mut block = Vec::new();
            e.begin(&mut block);
            e.status(&mut block, if round == 2 { 418 } else { 200 });
            for (n, v, volatile) in fields {
                e.field(&mut block, n.as_bytes(), v.as_bytes(), volatile);
            }
            sizes.push(block.len());
            let decoded = decode_all(&mut d, &block).unwrap();
            assert_eq!(decoded[0].1, if round == 2 { "418" } else { "200" });
            for (i, (n, v, _)) in fields.iter().enumerate() {
                assert_eq!((n.to_string(), v.to_string()), decoded[i + 1]);
            }
        }
        // Round two: status and three indexed fields (a byte each), then the
        // 29-byte date as a literal: 1 + 3 + 32.
        assert_eq!(36, sizes[1], "{sizes:?}");

        e.set_peer_table_size(64);
        let mut block = Vec::new();
        e.begin(&mut block);
        e.field(&mut block, b"vary", b"Origin", false);
        assert_eq!(0x3f, block[0], "size update first");
        let decoded = decode_all(&mut d, &block).unwrap();
        assert_eq!(("vary".to_string(), "Origin".to_string()), decoded[0]);
        assert!(d.table.max_size == 64 && d.table.size <= 64);
    }

    #[test]
    fn integers_round_trip_at_prefix_boundaries() {
        for n in [4u32, 5, 6, 7] {
            for value in [0usize, 1, (1 << n) - 2, (1 << n) - 1, 1 << n, 127, 128, 16_383, 1 << 20] {
                let mut out = Vec::new();
                write_integer(&mut out, 0, n, value);
                let mut pos = 0;
                assert_eq!(Ok(value), integer(&out, &mut pos, n));
                assert_eq!(out.len(), pos);
            }
        }
    }
}
