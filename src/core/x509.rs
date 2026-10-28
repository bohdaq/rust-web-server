//! X.509 certificate validity (notBefore/notAfter) and the RFC 9773 ARI
//! certificate id, read with a minimal DER walker — shared by ACME renewal
//! and the TLS certificate-expiry metric, so it isn't behind a feature.
//! Certificate bytes may come from a CA or a file: every length is checked,
//! nothing panics on malformed input.

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

// ── Certificate expiry ────────────────────────────────────────────────────────

/// What renewal needs from the leaf certificate: its validity window (Unix
/// seconds) and its ACME Renewal Information certificate identifier
/// (RFC 9773 §4.1), when it has an Authority Key Identifier.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CertInfo {
    pub not_before: i64,
    pub not_after: i64,
    /// `base64url(AKI keyIdentifier) "." base64url(serial)`; `None` without
    /// an Authority Key Identifier extension.
    pub cert_id: Option<String>,
}

/// [`CertInfo`] of the first certificate in the PEM file at `cert_path`;
/// `None` when it can't be read or parsed.
/// [`CertInfo`] of the first certificate in PEM text.
pub fn cert_info_from_pem(pem: &[u8]) -> Option<CertInfo> {
    cert_info_from_der(&pem_to_first_cert_der(std::str::from_utf8(pem).ok()?)?)
}

pub fn read_cert_info(cert_path: &str) -> Option<CertInfo> {
    let pem = std::fs::read_to_string(cert_path).ok()?;
    cert_info_from_der(&pem_to_first_cert_der(&pem)?)
}

/// [`CertInfo`] from a DER certificate. CA-supplied bytes: every length is
/// checked by `read_tlv`, nothing panics on malformed input.
pub fn cert_info_from_der(cert_der: &[u8]) -> Option<CertInfo> {
    let (_, cert_body, _) = read_tlv(cert_der, 0)?;
    let (_, tbs, _) = read_tlv(cert_body, 0)?;
    let mut pos = 0;
    if tbs.get(pos) == Some(&0xa0) {
        pos = read_tlv(tbs, pos)?.2;
    }
    let (serial_tag, serial, next) = read_tlv(tbs, pos)?;
    if serial_tag != 0x02 || serial.is_empty() {
        return None;
    }
    pos = skip_tlvs(tbs, next, 2)?; // signature, issuer
    let (_, validity, next) = read_tlv(tbs, pos)?;
    let (nb_tag, nb_val, nb_end) = read_tlv(validity, 0)?;
    let (na_tag, na_val, _) = read_tlv(validity, nb_end)?;
    let is_time = |tag: u8| tag == 0x17 || tag == 0x18;
    if !is_time(nb_tag) || !is_time(na_tag) {
        return None;
    }
    let not_before = parse_asn1_time(nb_val)?;
    let not_after = parse_asn1_time(na_val)?;
    pos = skip_tlvs(tbs, next, 2)?; // subject, subjectPublicKeyInfo

    // Optional issuerUniqueID [1], subjectUniqueID [2], then extensions [3].
    let mut aki: Option<&[u8]> = None;
    while pos < tbs.len() {
        let (tag, value, next) = read_tlv(tbs, pos)?;
        if tag == 0xa3 {
            aki = authority_key_identifier(value);
        }
        pos = next;
    }
    let cert_id = aki.map(|key_id| format!("{}.{}", base64url(key_id), base64url(serial)));
    Some(CertInfo { not_before, not_after, cert_id })
}

/// The keyIdentifier of the Authority Key Identifier extension
/// (OID 2.5.29.35) in an `[3] EXPLICIT Extensions` value.
fn authority_key_identifier(explicit: &[u8]) -> Option<&[u8]> {
    const AKI_OID: &[u8] = &[0x55, 0x1d, 0x23];
    let (_, extensions, _) = read_tlv(explicit, 0)?;
    let mut pos = 0;
    while pos < extensions.len() {
        let (_, extension, next) = read_tlv(extensions, pos)?;
        pos = next;
        let (oid_tag, oid, mut inner) = read_tlv(extension, 0)?;
        if oid_tag != 0x06 || oid != AKI_OID {
            continue;
        }
        let (mut tag, mut value, mut after) = read_tlv(extension, inner)?;
        if tag == 0x01 {
            // critical BOOLEAN
            inner = after;
            (tag, value, after) = read_tlv(extension, inner)?;
        }
        let _ = after;
        if tag != 0x04 {
            return None;
        }
        // OCTET STRING → AuthorityKeyIdentifier SEQUENCE → [0] IMPLICIT keyIdentifier
        let (_, aki, _) = read_tlv(value, 0)?;
        let (key_tag, key_id, _) = read_tlv(aki, 0)?;
        return (key_tag == 0x80 && !key_id.is_empty()).then_some(key_id);
    }
    None
}

/// A subjectAltName entry rws can match.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SubjectAltName {
    Dns(String),
    Uri(String),
}

/// The DNS and URI names of a DER certificate's subjectAltName extension
/// (OID 2.5.29.17); `None` when the certificate can't be read. Backend
/// certificates are untrusted input: loops only, every read bounded.
pub fn subject_alt_names(cert_der: &[u8]) -> Option<Vec<SubjectAltName>> {
    const SAN_OID: &[u8] = &[0x55, 0x1d, 0x11];
    let (_, cert_body, _) = read_tlv(cert_der, 0)?;
    let (_, tbs, _) = read_tlv(cert_body, 0)?;
    let mut pos = 0;
    let mut names = Vec::new();
    while pos < tbs.len() {
        let (tag, value, next) = read_tlv(tbs, pos)?;
        pos = next;
        if tag != 0xa3 {
            continue;
        }
        let (_, extensions, _) = read_tlv(value, 0)?;
        let mut at = 0;
        while at < extensions.len() {
            let (_, extension, next) = read_tlv(extensions, at)?;
            at = next;
            let (oid_tag, oid, mut inner) = read_tlv(extension, 0)?;
            if oid_tag != 0x06 || oid != SAN_OID {
                continue;
            }
            let (mut tag, mut value, after) = read_tlv(extension, inner)?;
            if tag == 0x01 {
                inner = after; // critical BOOLEAN
                (tag, value, _) = read_tlv(extension, inner)?;
            }
            if tag != 0x04 {
                return None;
            }
            // OCTET STRING → GeneralNames SEQUENCE → [2] dNSName / [6] URI (IMPLICIT IA5String)
            let (_, general_names, _) = read_tlv(value, 0)?;
            let mut n = 0;
            while n < general_names.len() {
                let (name_tag, name, next) = read_tlv(general_names, n)?;
                n = next;
                let text = || std::str::from_utf8(name).ok().map(str::to_string);
                match name_tag {
                    0x82 => names.extend(text().map(SubjectAltName::Dns)),
                    0x86 => names.extend(text().map(SubjectAltName::Uri)),
                    _ => {}
                }
            }
        }
    }
    Some(names)
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
            // At most 4 length bytes: more would overflow the shift (a
            // panic in debug builds) and no certificate is that large.
            let n = (b & 0x7f) as usize;
            if n == 0 || n > 4 { return None; }
            let mut l = 0usize;
            for _ in 0..n { l = (l << 8) | *data.get(p)? as usize; p += 1; }
            l
        }
    };
    let end = p.checked_add(len)?;
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

fn parse_asn1_time(t: &[u8]) -> Option<i64> {
    // Digits then `Z`: anything else (and any non-ASCII byte, which the
    // slicing below would split) is not a time rws can read.
    if t.len() < 2 || !t[..t.len() - 1].iter().all(u8::is_ascii_digit) {
        return None;
    }
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

pub fn days_since_epoch(year: i64, month: i64, day: i64) -> Option<i64> {
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

#[cfg(test)]
mod subject_alt_name_tests {
    use super::*;

    #[test]
    fn dns_and_uri_names_are_read_others_skipped() {
        let mut params = rcgen::CertificateParams::new(vec!["abc.example.com".to_string()]).unwrap();
        params.subject_alt_names.push(rcgen::SanType::URI("spiffe://cluster.local/ns/x/sa/y".try_into().unwrap()));
        params.subject_alt_names.push(rcgen::SanType::IpAddress("10.0.0.1".parse().unwrap()));
        let cert = params.self_signed(&rcgen::KeyPair::generate().unwrap()).unwrap();
        let der: &[u8] = cert.der();
        assert_eq!(
            Some(vec![SubjectAltName::Dns("abc.example.com".to_string()), SubjectAltName::Uri("spiffe://cluster.local/ns/x/sa/y".to_string())]),
            subject_alt_names(der),
            "the IP name is skipped"
        );
        for cut in [0, 1, 10, der.len() / 2, der.len() - 1] {
            assert!(subject_alt_names(&der[..cut]).is_none_or(|n| n.len() <= 2), "a truncated certificate never panics");
        }
        assert_eq!(None, subject_alt_names(b"not DER"));
    }
}
