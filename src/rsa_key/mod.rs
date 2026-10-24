//! RSA public keys and RSASSA-PKCS1-v1_5 / SHA-256 signature verification —
//! everything `rws` does with RSA (RS256 JWTs, SAML `rsa-sha256` signatures),
//! implemented in-crate.
//!
//! This replaces the `rsa` crate, whose RUSTSEC-2023-0071 ("Marvin": a timing
//! side channel in private-key operations) has no fixed release. `rws` never
//! holds an RSA private key: verification uses only public data — the key,
//! the message and the signature — so nothing secret can leak through
//! timing, and the arithmetic here needn't be constant-time. It is pure Rust
//! (works on `wasm32` too).
//!
//! Verification follows RFC 8017 §8.2.2: the signature must be exactly the
//! modulus length and smaller than the modulus; `s^e mod n` is computed with
//! Montgomery multiplication; and the result must equal, byte for byte, the
//! expected encoding `00 01 FF…FF 00 || DigestInfo(SHA-256) || hash`.
//! Comparing the whole encoded block (rather than parsing it) rules out the
//! classic padding-forgery tricks. Keys must be 2048–8192 bits with an odd
//! public exponent of at least 3.

#[cfg(test)]
mod tests;

#[cfg(all(test, not(target_arch = "wasm32")))]
pub(crate) mod test_support;

use sha2::{Digest, Sha256};

/// DER `DigestInfo` prefix for SHA-256 (RFC 8017 §9.2, note 1).
const SHA256_DIGEST_INFO: [u8; 19] = [
    0x30, 0x31, 0x30, 0x0d, 0x06, 0x09, 0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x02, 0x01, 0x05, 0x00, 0x04, 0x20,
];

/// `rsaEncryption` OID 1.2.840.113549.1.1.1, DER-encoded content.
const RSA_ENCRYPTION_OID: [u8; 9] = [0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x01, 0x01];

const MIN_BITS: usize = 2048;
const MAX_BITS: usize = 8192;

/// An RSA public key, ready to verify signatures.
#[derive(Clone)]
pub struct RsaPublicKey {
    /// Modulus, little-endian 32-bit limbs.
    n: Vec<u32>,
    /// Modulus length in bytes (the signature length).
    k: usize,
    e: u64,
    /// `-n⁻¹ mod 2³²`, for Montgomery reduction.
    n0_inv: u32,
    /// `R² mod n`, `R = 2^(32·limbs)`.
    r2: Vec<u32>,
    /// Big-endian modulus as given (for `Debug` and comparisons).
    n_bytes: Vec<u8>,
}

impl std::fmt::Debug for RsaPublicKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RsaPublicKey({} bits, e = {})", self.size_bits(), self.e)
    }
}

impl PartialEq for RsaPublicKey {
    fn eq(&self, other: &Self) -> bool {
        self.n_bytes == other.n_bytes && self.e == other.e
    }
}

impl Eq for RsaPublicKey {}

impl RsaPublicKey {
    /// From the big-endian modulus `n` and public exponent `e` — e.g. a JWK's
    /// base64url-decoded `n` and `e`.
    pub fn from_components(n: &[u8], e: &[u8]) -> Result<RsaPublicKey, String> {
        let n = strip_leading_zeros(n);
        let e_bytes = strip_leading_zeros(e);
        if n.is_empty() {
            return Err("RSA modulus is zero".to_string());
        }
        let bits = (n.len() - 1) * 8 + (8 - n[0].leading_zeros() as usize);
        if !(MIN_BITS..=MAX_BITS).contains(&bits) {
            return Err(format!("RSA key is {bits} bits; {MIN_BITS}–{MAX_BITS} are accepted"));
        }
        if n[n.len() - 1] & 1 == 0 {
            return Err("RSA modulus is even".to_string());
        }
        if e_bytes.is_empty() || e_bytes.len() > 8 {
            return Err("RSA public exponent out of range".to_string());
        }
        let e = e_bytes.iter().fold(0u64, |acc, b| (acc << 8) | *b as u64);
        if e < 3 || e & 1 == 0 {
            return Err(format!("RSA public exponent {e} is not an odd number of at least 3"));
        }

        let limbs = n.len().div_ceil(4);
        let n_limbs = from_be_bytes(n, limbs);
        let n0_inv = montgomery_n0_inv(n_limbs[0]);
        let r2 = r_squared_mod(&n_limbs);
        Ok(RsaPublicKey { n: n_limbs, k: n.len(), e, n0_inv, r2, n_bytes: n.to_vec() })
    }

    /// From a PKCS#1 `RSAPublicKey` (`SEQUENCE { n INTEGER, e INTEGER }`), DER.
    pub fn from_pkcs1_der(der: &[u8]) -> Result<RsaPublicKey, String> {
        let (seq, rest) = expect_tlv(der, 0x30, "RSAPublicKey")?;
        if !rest.is_empty() {
            return Err("trailing bytes after RSAPublicKey".to_string());
        }
        let (n, after_n) = expect_tlv(seq, 0x02, "modulus")?;
        let (e, after_e) = expect_tlv(after_n, 0x02, "public exponent")?;
        if !after_e.is_empty() {
            return Err("unexpected fields in RSAPublicKey".to_string());
        }
        RsaPublicKey::from_components(positive_integer(n)?, positive_integer(e)?)
    }

    /// From an X.509 `SubjectPublicKeyInfo` (the `BEGIN PUBLIC KEY` form, and
    /// what a certificate carries), DER. Must be an `rsaEncryption` key.
    pub fn from_public_key_der(spki: &[u8]) -> Result<RsaPublicKey, String> {
        let (seq, rest) = expect_tlv(spki, 0x30, "SubjectPublicKeyInfo")?;
        if !rest.is_empty() {
            return Err("trailing bytes after SubjectPublicKeyInfo".to_string());
        }
        let (algorithm, after_alg) = expect_tlv(seq, 0x30, "AlgorithmIdentifier")?;
        let (oid, _params) = expect_tlv(algorithm, 0x06, "algorithm OID")?;
        if oid != RSA_ENCRYPTION_OID {
            return Err("not an RSA public key".to_string());
        }
        let (bits, after_bits) = expect_tlv(after_alg, 0x03, "subjectPublicKey")?;
        if !after_bits.is_empty() {
            return Err("unexpected fields in SubjectPublicKeyInfo".to_string());
        }
        match bits.split_first() {
            Some((0, key)) => RsaPublicKey::from_pkcs1_der(key),
            _ => Err("subjectPublicKey has unused bits".to_string()),
        }
    }

    /// From PEM: `-----BEGIN PUBLIC KEY-----` (SubjectPublicKeyInfo) or
    /// `-----BEGIN RSA PUBLIC KEY-----` (PKCS#1).
    pub fn from_public_key_pem(pem: &str) -> Result<RsaPublicKey, String> {
        for (label, spki) in [("PUBLIC KEY", true), ("RSA PUBLIC KEY", false)] {
            let begin = format!("-----BEGIN {label}-----");
            let end = format!("-----END {label}-----");
            if let Some(start) = pem.find(&begin) {
                let body_start = start + begin.len();
                let body_end = pem[body_start..].find(&end).map(|i| body_start + i).ok_or("PEM end line missing")?;
                let der = base64_decode(&pem[body_start..body_end]).ok_or("PEM body is not base64")?;
                return if spki { RsaPublicKey::from_public_key_der(&der) } else { RsaPublicKey::from_pkcs1_der(&der) };
            }
        }
        Err("no `BEGIN PUBLIC KEY` or `BEGIN RSA PUBLIC KEY` block".to_string())
    }

    /// Modulus size in bits.
    pub fn size_bits(&self) -> usize {
        (self.k - 1) * 8 + (8 - self.n_bytes[0].leading_zeros() as usize)
    }

    /// The big-endian modulus.
    pub fn modulus(&self) -> &[u8] {
        &self.n_bytes
    }

    /// The public exponent.
    pub fn exponent(&self) -> u64 {
        self.e
    }

    /// `true` if `signature` is a valid RSASSA-PKCS1-v1_5 signature with
    /// SHA-256 (JWS `RS256`, XML-DSig `rsa-sha256`) of `message`.
    pub fn verify_pkcs1v15_sha256(&self, message: &[u8], signature: &[u8]) -> bool {
        let hash = Sha256::digest(message);
        self.verify_pkcs1v15_sha256_prehashed(&hash, signature)
    }

    /// As [`verify_pkcs1v15_sha256`](Self::verify_pkcs1v15_sha256), given the
    /// SHA-256 hash of the message.
    pub fn verify_pkcs1v15_sha256_prehashed(&self, hash: &[u8], signature: &[u8]) -> bool {
        if hash.len() != 32 || signature.len() != self.k {
            return false;
        }
        let s = from_be_bytes(signature, self.n.len());
        if !less_than(&s, &self.n) {
            return false;
        }
        let m = self.mod_pow(&s);
        let em = to_be_bytes(&m, self.k);

        // 00 01 FF…FF 00 || DigestInfo || H  (at least 8 bytes of FF)
        let t_len = SHA256_DIGEST_INFO.len() + 32;
        if self.k < t_len + 11 {
            return false;
        }
        let mut expected = Vec::with_capacity(self.k);
        expected.extend_from_slice(&[0x00, 0x01]);
        expected.resize(self.k - t_len - 1, 0xff);
        expected.push(0x00);
        expected.extend_from_slice(&SHA256_DIGEST_INFO);
        expected.extend_from_slice(hash);
        em == expected
    }

    /// `base^e mod n` (`base < n`), square-and-multiply in Montgomery form.
    fn mod_pow(&self, base: &[u32]) -> Vec<u32> {
        let one = {
            let mut v = vec![0u32; self.n.len()];
            v[0] = 1;
            v
        };
        let base_m = self.mont_mul(base, &self.r2);
        let mut acc = self.mont_mul(&one, &self.r2); // R mod n: Montgomery 1
        for bit in (0..64 - self.e.leading_zeros()).rev() {
            acc = self.mont_mul(&acc, &acc);
            if (self.e >> bit) & 1 == 1 {
                acc = self.mont_mul(&acc, &base_m);
            }
        }
        self.mont_mul(&acc, &one)
    }

    /// Montgomery product `a·b·R⁻¹ mod n` (CIOS).
    fn mont_mul(&self, a: &[u32], b: &[u32]) -> Vec<u32> {
        let s = self.n.len();
        let n = &self.n;
        let mut t = vec![0u32; s + 2];
        for &bi in b.iter().take(s) {
            let mut carry: u64 = 0;
            for j in 0..s {
                let x = t[j] as u64 + a[j] as u64 * bi as u64 + carry;
                t[j] = x as u32;
                carry = x >> 32;
            }
            let x = t[s] as u64 + carry;
            t[s] = x as u32;
            t[s + 1] = (x >> 32) as u32;

            let m = t[0].wrapping_mul(self.n0_inv);
            let x = t[0] as u64 + m as u64 * n[0] as u64;
            let mut carry = x >> 32;
            for j in 1..s {
                let x = t[j] as u64 + m as u64 * n[j] as u64 + carry;
                t[j - 1] = x as u32;
                carry = x >> 32;
            }
            let x = t[s] as u64 + carry;
            t[s - 1] = x as u32;
            t[s] = t[s + 1] + (x >> 32) as u32;
            t[s + 1] = 0;
        }
        let mut result = t[..s].to_vec();
        if t[s] != 0 || !less_than(&result, n) {
            subtract_in_place(&mut result, n);
        }
        result
    }
}

fn strip_leading_zeros(bytes: &[u8]) -> &[u8] {
    let first = bytes.iter().position(|&b| b != 0).unwrap_or(bytes.len());
    &bytes[first..]
}

/// Big-endian bytes → `limbs` little-endian u32 limbs (zero-extended).
fn from_be_bytes(bytes: &[u8], limbs: usize) -> Vec<u32> {
    let mut out = vec![0u32; limbs.max(bytes.len().div_ceil(4))];
    for (i, &b) in bytes.iter().rev().enumerate() {
        out[i / 4] |= (b as u32) << (8 * (i % 4));
    }
    out
}

/// Little-endian limbs → exactly `len` big-endian bytes.
fn to_be_bytes(limbs: &[u32], len: usize) -> Vec<u8> {
    let mut out = vec![0u8; len];
    for i in 0..len {
        let limb = limbs.get(i / 4).copied().unwrap_or(0);
        out[len - 1 - i] = (limb >> (8 * (i % 4))) as u8;
    }
    out
}

fn less_than(a: &[u32], b: &[u32]) -> bool {
    let len = a.len().max(b.len());
    for i in (0..len).rev() {
        let (x, y) = (a.get(i).copied().unwrap_or(0), b.get(i).copied().unwrap_or(0));
        if x != y {
            return x < y;
        }
    }
    false
}

/// `a -= b`, assuming the true result is non-negative modulo 2^(32·len).
fn subtract_in_place(a: &mut [u32], b: &[u32]) {
    let mut borrow: i64 = 0;
    for i in 0..a.len() {
        let x = a[i] as i64 - b.get(i).copied().unwrap_or(0) as i64 - borrow;
        if x < 0 {
            a[i] = (x + (1i64 << 32)) as u32;
            borrow = 1;
        } else {
            a[i] = x as u32;
            borrow = 0;
        }
    }
}

/// `-n0⁻¹ mod 2³²` for odd `n0` (Newton iteration: each step doubles the
/// number of correct bits).
fn montgomery_n0_inv(n0: u32) -> u32 {
    let mut inv: u32 = 1;
    for _ in 0..5 {
        inv = inv.wrapping_mul(2u32.wrapping_sub(n0.wrapping_mul(inv)));
    }
    inv.wrapping_neg()
}

/// `R² mod n`, `R = 2^(32·limbs)`: start from 1 and double `2·32·limbs`
/// times, subtracting `n` whenever the value reaches it.
fn r_squared_mod(n: &[u32]) -> Vec<u32> {
    let s = n.len();
    let mut x = vec![0u32; s];
    x[0] = 1;
    for _ in 0..2 * 32 * s {
        let mut carry = 0u32;
        for limb in x.iter_mut() {
            let next = *limb >> 31;
            *limb = (*limb << 1) | carry;
            carry = next;
        }
        if carry != 0 || !less_than(&x, n) {
            subtract_in_place(&mut x, n);
        }
    }
    x
}

/// One DER element with tag `tag`: `(contents, rest)`.
fn expect_tlv<'a>(input: &'a [u8], tag: u8, what: &str) -> Result<(&'a [u8], &'a [u8]), String> {
    let (&t, rest) = input.split_first().ok_or_else(|| format!("{what}: unexpected end of DER"))?;
    if t != tag {
        return Err(format!("{what}: expected DER tag {tag:#04x}, found {t:#04x}"));
    }
    let (&first, rest) = rest.split_first().ok_or_else(|| format!("{what}: missing length"))?;
    let (len, rest) = if first < 0x80 {
        (first as usize, rest)
    } else {
        let count = (first & 0x7f) as usize;
        if count == 0 || count > 4 || rest.len() < count {
            return Err(format!("{what}: unsupported DER length"));
        }
        let len = rest[..count].iter().fold(0usize, |acc, b| (acc << 8) | *b as usize);
        (len, &rest[count..])
    };
    if rest.len() < len {
        return Err(format!("{what}: DER length past the end of the input"));
    }
    Ok((&rest[..len], &rest[len..]))
}

/// A DER INTEGER's magnitude; negative values are rejected.
fn positive_integer(bytes: &[u8]) -> Result<&[u8], String> {
    match bytes.first() {
        None => Err("empty DER INTEGER".to_string()),
        Some(b) if b & 0x80 != 0 => Err("negative DER INTEGER".to_string()),
        Some(_) => Ok(strip_leading_zeros(bytes)),
    }
}

/// Standard base64 (RFC 4648 §4), ignoring whitespace; padding optional.
fn base64_decode(input: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(input.len() * 3 / 4);
    let mut buf = 0u32;
    let mut bits = 0u32;
    for c in input.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' => break,
            b' ' | b'\n' | b'\r' | b'\t' => continue,
            _ => return None,
        };
        buf = (buf << 6) | v as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
        }
    }
    Some(out)
}
