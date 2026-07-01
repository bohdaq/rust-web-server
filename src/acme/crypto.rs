use aws_lc_rs::digest as lc_digest;
use aws_lc_rs::signature::{EcdsaKeyPair, KeyPair as SignKeyPair, ECDSA_P256_SHA256_FIXED_SIGNING};

// ── AccountKey ────────────────────────────────────────────────────────────────

/// ACME account key backed by ECDSA P-256.  Signing produces the 64-byte
/// fixed r||s format required by JOSE (JWS compact form).
pub struct AccountKey {
    inner: EcdsaKeyPair,
}

impl AccountKey {
    /// Generate a fresh P-256 key pair. Returns `(key, pkcs8_der)`.
    pub fn generate() -> Result<(Self, Vec<u8>), String> {
        let key = EcdsaKeyPair::generate(&ECDSA_P256_SHA256_FIXED_SIGNING)
            .map_err(|_| "account key generation failed".to_string())?;
        let doc = key.to_pkcs8v1()
            .map_err(|_| "account key serialization failed".to_string())?;
        let der = doc.as_ref().to_vec();
        Ok((AccountKey { inner: key }, der))
    }

    /// Load a key from raw PKCS#8 DER bytes.
    pub fn from_pkcs8(der: &[u8]) -> Result<Self, String> {
        let key = EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, der)
            .map_err(|e| format!("account key parse failed: {e}"))?;
        Ok(AccountKey { inner: key })
    }

    /// Sign `data` with ECDSA P-256 SHA-256.  Returns raw r||s (64 bytes).
    pub fn sign(&self, data: &[u8]) -> Result<[u8; 64], String> {
        let rng = aws_lc_rs::rand::SystemRandom::new();
        let sig = self.inner.sign(&rng, data)
            .map_err(|_| "ECDSA signing failed".to_string())?;
        let bytes = sig.as_ref();
        if bytes.len() != 64 {
            return Err(format!("unexpected ECDSA fixed signature length: {}", bytes.len()));
        }
        let mut out = [0u8; 64];
        out.copy_from_slice(bytes);
        Ok(out)
    }

    /// Returns the uncompressed EC public key: `04 || x(32) || y(32)` (65 bytes).
    pub fn public_key_raw(&self) -> &[u8] {
        self.inner.public_key().as_ref()
    }
}

// ── base64url ─────────────────────────────────────────────────────────────────

pub fn base64url(data: &[u8]) -> String {
    const ALPHA: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = Vec::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as usize;
        let b1 = if chunk.len() > 1 { chunk[1] as usize } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as usize } else { 0 };
        out.push(ALPHA[b0 >> 2]);
        out.push(ALPHA[((b0 & 3) << 4) | (b1 >> 4)]);
        if chunk.len() > 1 { out.push(ALPHA[((b1 & 0xf) << 2) | (b2 >> 6)]); }
        if chunk.len() > 2 { out.push(ALPHA[b2 & 0x3f]); }
    }
    String::from_utf8(out).unwrap()
}

pub fn sha256(data: &[u8]) -> [u8; 32] {
    let d = lc_digest::digest(&lc_digest::SHA256, data);
    let mut out = [0u8; 32];
    out.copy_from_slice(d.as_ref());
    out
}

// ── EC key helpers ────────────────────────────────────────────────────────────

/// Extract x, y from an uncompressed EC P-256 point `04 || x(32) || y(32)`.
pub fn ec_point_xy(raw: &[u8]) -> Result<([u8; 32], [u8; 32]), String> {
    if raw.len() != 65 || raw[0] != 0x04 {
        return Err(format!(
            "unexpected EC public key: {} bytes, first byte 0x{:02x}",
            raw.len(),
            raw.first().copied().unwrap_or(0)
        ));
    }
    let mut x = [0u8; 32];
    let mut y = [0u8; 32];
    x.copy_from_slice(&raw[1..33]);
    y.copy_from_slice(&raw[33..65]);
    Ok((x, y))
}

/// Build the canonical JWK JSON for ES256 (sorted keys — required for thumbprint).
pub fn ec_jwk_json(x: &[u8; 32], y: &[u8; 32]) -> String {
    // RFC 7638 §3.3: keys must be sorted alphabetically ("crv", "kty", "x", "y")
    format!(
        r#"{{"crv":"P-256","kty":"EC","x":"{}","y":"{}"}}"#,
        base64url(x),
        base64url(y),
    )
}

/// Compute the JOSE key thumbprint (RFC 7638) for a P-256 key pair.
pub fn key_thumbprint(key: &AccountKey) -> Result<String, String> {
    let (x, y) = ec_point_xy(key.public_key_raw())?;
    Ok(base64url(&sha256(ec_jwk_json(&x, &y).as_bytes())))
}

// ── JWS construction ──────────────────────────────────────────────────────────

/// Build a JWS-signed POST body for an ACME request.
///
/// - `account_url = None` → use `"jwk"` (for `newAccount`)
/// - `account_url = Some(_)` → use `"kid"` (for all subsequent requests)
/// - `payload = None` → POST-as-GET (empty payload)
pub fn build_jws(
    key: &AccountKey,
    nonce: &str,
    url: &str,
    account_url: Option<&str>,
    payload: Option<&str>,
) -> Result<String, String> {
    let protected = match account_url {
        Some(kid) => format!(
            r#"{{"alg":"ES256","kid":"{kid}","nonce":"{nonce}","url":"{url}"}}"#,
            kid = kid,
            nonce = nonce,
            url = url,
        ),
        None => {
            let (x, y) = ec_point_xy(key.public_key_raw())?;
            let jwk = ec_jwk_json(&x, &y);
            format!(
                r#"{{"alg":"ES256","jwk":{jwk},"nonce":"{nonce}","url":"{url}"}}"#,
                jwk = jwk,
                nonce = nonce,
                url = url,
            )
        }
    };

    let protected_b64 = base64url(protected.as_bytes());
    let payload_b64 = payload.map(|p| base64url(p.as_bytes())).unwrap_or_default();

    let signing_input = format!("{}.{}", protected_b64, payload_b64);
    let sig_bytes = key.sign(signing_input.as_bytes())?;
    let sig_b64 = base64url(&sig_bytes);

    Ok(format!(
        r#"{{"protected":"{protected_b64}","payload":"{payload_b64}","signature":"{sig_b64}"}}"#,
        protected_b64 = protected_b64,
        payload_b64 = payload_b64,
        sig_b64 = sig_b64,
    ))
}

// ── Certificate expiry ────────────────────────────────────────────────────────

/// Return the number of days until the first certificate in `cert_pem_path`
/// expires. Returns `None` when the file can't be read or parsed.
pub fn cert_days_until_expiry(cert_path: &str) -> Option<i64> {
    let pem = std::fs::read_to_string(cert_path).ok()?;
    let der = pem_to_first_cert_der(&pem)?;
    let not_after_secs = parse_cert_not_after(&der)?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs() as i64;
    Some((not_after_secs - now) / 86400)
}

fn pem_to_first_cert_der(pem: &str) -> Option<Vec<u8>> {
    let start = pem.find("-----BEGIN CERTIFICATE-----")?;
    let after = &pem[start + 27..];
    let end = after.find("-----END CERTIFICATE-----")?;
    let b64: String = after[..end].chars().filter(|c| !c.is_whitespace()).collect();
    base64_decode_std(&b64)
}

// ── minimal DER TLV parser ────────────────────────────────────────────────────

fn read_tlv(data: &[u8], pos: usize) -> Option<(u8, &[u8], usize)> {
    if pos >= data.len() { return None; }
    let tag = data[pos];
    let mut p = pos + 1;
    let len = {
        let b = *data.get(p)?; p += 1;
        if b < 0x80 { b as usize } else {
            let n = (b & 0x7f) as usize;
            let mut l = 0usize;
            for _ in 0..n { l = (l << 8) | *data.get(p)? as usize; p += 1; }
            l
        }
    };
    let end = p + len;
    if end > data.len() { return None; }
    Some((tag, &data[p..end], end))
}

fn skip_tlvs(data: &[u8], mut pos: usize, count: usize) -> Option<usize> {
    for _ in 0..count {
        let (_, _, next) = read_tlv(data, pos)?;
        pos = next;
    }
    Some(pos)
}

/// Parse the `notAfter` date from an X.509 DER certificate → Unix timestamp.
fn parse_cert_not_after(cert_der: &[u8]) -> Option<i64> {
    // Certificate → TBSCertificate
    let (_, cert_body, _) = read_tlv(cert_der, 0)?; // Certificate SEQUENCE
    let (_, tbs, _) = read_tlv(cert_body, 0)?;       // TBSCertificate SEQUENCE

    let mut pos = 0;
    if tbs.get(pos) == Some(&0xa0) { // optional version [0]
        let (_, _, next) = read_tlv(tbs, pos)?;
        pos = next;
    }
    pos = skip_tlvs(tbs, pos, 3)?; // serialNumber, signature, issuer

    // Validity SEQUENCE
    let (_, validity, _) = read_tlv(tbs, pos)?;
    let (_, _, nb_end) = read_tlv(validity, 0)?;     // skip notBefore
    let (na_tag, na_val, _) = read_tlv(validity, nb_end)?; // notAfter
    if na_tag != 0x17 && na_tag != 0x18 { return None; }
    parse_asn1_time(na_val)
}

fn parse_asn1_time(t: &[u8]) -> Option<i64> {
    let s = std::str::from_utf8(t).ok()?;
    if !s.ends_with('Z') { return None; }
    let s = &s[..s.len() - 1]; // strip 'Z'
    let (year, rest) = if s.len() == 12 {
        let yy: i64 = s[..2].parse().ok()?;
        let year = if yy < 50 { 2000 + yy } else { 1900 + yy };
        (year, &s[2..])
    } else if s.len() == 14 {
        (s[..4].parse().ok()?, &s[4..])
    } else {
        return None;
    };
    let mo: i64 = rest[0..2].parse().ok()?;
    let da: i64 = rest[2..4].parse().ok()?;
    let hr: i64 = rest[4..6].parse().ok()?;
    let mi: i64 = rest[6..8].parse().ok()?;
    let se: i64 = rest[8..10].parse().ok()?;
    let days = days_since_epoch(year, mo, da)?;
    Some(days * 86400 + hr * 3600 + mi * 60 + se)
}

fn days_since_epoch(year: i64, month: i64, day: i64) -> Option<i64> {
    // https://howardhinnant.github.io/date_algorithms.html "days_from_civil"
    let y = if month <= 2 { year - 1 } else { year };
    let m = if month <= 2 { month + 9 } else { month - 3 };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * m + 2) / 5 + (day - 1);
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some(era * 146097 + doe - 719468)
}

// ── standard base64 decoder (no padding required) ────────────────────────────

pub fn base64_decode_std(s: &str) -> Option<Vec<u8>> {
    let digits: Vec<u8> = s.bytes().filter_map(|b| match b {
        b'A'..=b'Z' => Some(b - b'A'),
        b'a'..=b'z' => Some(b - b'a' + 26),
        b'0'..=b'9' => Some(b - b'0' + 52),
        b'+' | b'-' => Some(62),
        b'/' | b'_' => Some(63),
        b'=' => None,
        _ => None,
    }).collect();
    let mut out = Vec::with_capacity(digits.len() * 3 / 4);
    for chunk in digits.chunks(4) {
        let v = chunk.iter().enumerate()
            .fold(0u32, |acc, (i, &d)| acc | ((d as u32) << (18 - i * 6)));
        if chunk.len() >= 2 { out.push((v >> 16) as u8); }
        if chunk.len() >= 3 { out.push((v >> 8) as u8); }
        if chunk.len() >= 4 { out.push(v as u8); }
    }
    Some(out)
}
