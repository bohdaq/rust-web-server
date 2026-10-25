//! HPACK's Huffman string coding (RFC 7541 §5.2, Appendix B).
//!
//! The encoder isn't on the response path (rws sends header strings as raw
//! octets, which every decoder accepts); it exists for tests. The decoder
//! walks a binary trie built once from the code table, one bit at a time,
//! and rejects what §5.2 forbids: the EOS symbol inside a string, and
//! padding longer than 7 bits or not made of ones.

use std::sync::OnceLock;

use super::huffman_table::{CODES, EOS_CODE, EOS_LENGTH, LENGTHS};

/// A trie node: children for bit 0 and 1. A child below `LEAF` is another
/// node; at or above it, the symbol `child - LEAF` (256 = EOS).
#[derive(Clone, Copy)]
struct Node([u16; 2]);

const LEAF: u16 = 0x8000;
const EMPTY: u16 = u16::MAX;
const EOS: u16 = 256;

fn trie() -> &'static [Node] {
    static TRIE: OnceLock<Vec<Node>> = OnceLock::new();
    TRIE.get_or_init(|| {
        let mut nodes = vec![Node([EMPTY, EMPTY])];
        let symbols = (0..256u16).map(|s| (s, CODES[s as usize], LENGTHS[s as usize])).chain([(EOS, EOS_CODE, EOS_LENGTH)]);
        for (symbol, code, length) in symbols {
            let mut node = 0usize;
            for i in (0..length).rev() {
                let bit = ((code >> i) & 1) as usize;
                if i == 0 {
                    nodes[node].0[bit] = LEAF + symbol;
                } else {
                    let child = nodes[node].0[bit];
                    node = if child == EMPTY {
                        nodes.push(Node([EMPTY, EMPTY]));
                        let new = (nodes.len() - 1) as u16;
                        nodes[node].0[bit] = new;
                        new as usize
                    } else {
                        child as usize
                    };
                }
            }
        }
        nodes
    })
}

/// Decode `input`, appending to `out`. `Err` for an invalid string.
pub(crate) fn decode(input: &[u8], out: &mut Vec<u8>) -> Result<(), ()> {
    let nodes = trie();
    let mut node = 0usize;
    // Bits read since the last symbol, and whether they were all ones (a
    // valid padding prefix of EOS).
    let mut pending_bits = 0u32;
    let mut pending_ones = true;
    for &byte in input {
        for i in (0..8).rev() {
            let bit = ((byte >> i) & 1) as usize;
            let next = nodes[node].0[bit];
            pending_bits += 1;
            pending_ones &= bit == 1;
            if next == EMPTY {
                return Err(());
            }
            if next >= LEAF {
                let symbol = next - LEAF;
                if symbol == EOS {
                    return Err(());
                }
                out.push(symbol as u8);
                node = 0;
                pending_bits = 0;
                pending_ones = true;
            } else {
                node = next as usize;
            }
        }
    }
    if pending_bits > 7 || !pending_ones {
        return Err(());
    }
    Ok(())
}

/// Huffman-encode `input` (padding with ones).
#[cfg(test)]
pub(crate) fn encode(input: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut acc: u64 = 0;
    let mut bits = 0u32;
    for &b in input {
        acc = (acc << LENGTHS[b as usize]) | CODES[b as usize] as u64;
        bits += LENGTHS[b as usize] as u32;
        while bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    if bits > 0 {
        out.push(((acc << (8 - bits)) as u8) | (0xff >> bits));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc_7541_examples_decode() {
        // C.4.1 "www.example.com", C.4.2 "no-cache", C.4.3 "custom-key"/"custom-value".
        let cases: [(&str, &[u8]); 4] = [
            ("www.example.com", &[0xf1, 0xe3, 0xc2, 0xe5, 0xf2, 0x3a, 0x6b, 0xa0, 0xab, 0x90, 0xf4, 0xff]),
            ("no-cache", &[0xa8, 0xeb, 0x10, 0x64, 0x9c, 0xbf]),
            ("custom-key", &[0x25, 0xa8, 0x49, 0xe9, 0x5b, 0xa9, 0x7d, 0x7f]),
            ("custom-value", &[0x25, 0xa8, 0x49, 0xe9, 0x5b, 0xb8, 0xe8, 0xb4, 0xbf]),
        ];
        for (text, coded) in cases {
            let mut out = Vec::new();
            decode(coded, &mut out).unwrap();
            assert_eq!(text.as_bytes(), &out[..]);
            assert_eq!(coded, &encode(text.as_bytes())[..]);
        }
    }

    #[test]
    fn every_byte_round_trips() {
        let all: Vec<u8> = (0..=255u8).chain((0..=255u8).rev()).collect();
        let mut out = Vec::new();
        decode(&encode(&all), &mut out).unwrap();
        assert_eq!(all, out);
        let mut empty = Vec::new();
        decode(&[], &mut empty).unwrap();
        assert!(empty.is_empty());
    }

    #[test]
    fn invalid_padding_and_eos_are_rejected() {
        let mut out = Vec::new();
        // "a" is 00011 (5 bits): padded with 111 is valid, with 000 is not.
        assert!(decode(&[0b0001_1111], &mut out).is_ok());
        assert!(decode(&[0b0001_1000], &mut out).is_err());
        // A whole byte of padding (8 ones) is too long.
        assert!(decode(&[0b0001_1111, 0xff], &mut out).is_err());
        // EOS (30 ones) inside the string.
        assert!(decode(&[0xff, 0xff, 0xff, 0xfc], &mut out).is_err());
    }
}
