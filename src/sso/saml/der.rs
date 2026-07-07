//! Just enough DER/ASN.1 to pull an RSA `SubjectPublicKeyInfo` out of an
//! X.509 certificate — not a general ASN.1 parser.
//!
//! SAML IdP metadata embeds the signing certificate as raw base64 DER
//! (`<ds:X509Certificate>`). To get from that to an [`rsa::RsaPublicKey`]
//! we only need to locate the `subjectPublicKeyInfo` field inside
//! `Certificate.tbsCertificate` — `rsa::pkcs8::DecodePublicKey` (already a
//! transitive capability of the `rsa` crate this crate already depends on
//! for JWKS verification) parses that slice directly. Full X.509 parsing
//! (extensions, name constraints, validity, etc.) is not needed and not
//! implemented — this module's only job is walking past the DER fields
//! that come *before* the public key.

/// Extract the raw `SubjectPublicKeyInfo` DER bytes from a DER-encoded
/// X.509 certificate, per RFC 5280:
///
/// ```text
/// Certificate ::= SEQUENCE {
///     tbsCertificate       TBSCertificate,
///     ...
/// }
/// TBSCertificate ::= SEQUENCE {
///     version         [0]  EXPLICIT Version DEFAULT v1,
///     serialNumber         CertificateSerialNumber,
///     signature            AlgorithmIdentifier,
///     issuer               Name,
///     validity             Validity,
///     subject              Name,
///     subjectPublicKeyInfo SubjectPublicKeyInfo,
///     ...
/// }
/// ```
pub(crate) fn extract_subject_public_key_info(cert_der: &[u8]) -> Result<Vec<u8>, String> {
    let (cert_body, _) = read_tlv(cert_der, 0)?; // Certificate ::= SEQUENCE { ... }
    let tbs_outer = cert_body.value;
    let (tbs, _) = read_tlv(tbs_outer, 0)?; // tbsCertificate ::= SEQUENCE { ... }

    let mut pos = 0usize;
    // Optional [0] EXPLICIT version — context-constructed tag 0xA0.
    if tbs.value.first() == Some(&0xA0) {
        let (_, next) = read_tlv(tbs.value, pos)?;
        pos = next;
    }
    // serialNumber, signature, issuer, validity, subject — five fields to skip.
    for _ in 0..5 {
        let (_, next) = read_tlv(tbs.value, pos)?;
        pos = next;
    }
    // subjectPublicKeyInfo — the field we want, whole TLV included.
    let (spki, _) = read_tlv(tbs.value, pos)?;
    Ok(spki.whole.to_vec())
}

struct Tlv<'a> {
    /// The complete tag+length+value bytes.
    whole: &'a [u8],
    /// Just the value bytes.
    value: &'a [u8],
}

/// Read one DER TLV starting at `pos`, returning it and the position right
/// after it. Supports single-byte tags (sufficient for every tag this
/// module encounters) and both short- and long-form DER lengths.
fn read_tlv(data: &[u8], pos: usize) -> Result<(Tlv<'_>, usize), String> {
    if pos >= data.len() {
        return Err("unexpected end of DER data".to_string());
    }
    let tag_start = pos;
    let mut p = pos + 1; // single-byte tag
    let len_byte = *data.get(p).ok_or("unexpected end of DER data reading length")?;
    p += 1;
    let (value_len, header_len) = if len_byte & 0x80 == 0 {
        (len_byte as usize, p - tag_start)
    } else {
        let n_bytes = (len_byte & 0x7f) as usize;
        if n_bytes == 0 || n_bytes > 4 {
            return Err("unsupported DER length encoding".to_string());
        }
        let len_bytes = data.get(p..p + n_bytes).ok_or("truncated DER length")?;
        let mut len = 0usize;
        for b in len_bytes {
            len = (len << 8) | (*b as usize);
        }
        p += n_bytes;
        (len, p - tag_start)
    };
    let value_start = tag_start + header_len;
    let value_end = value_start.checked_add(value_len).ok_or("DER length overflow")?;
    let value = data.get(value_start..value_end).ok_or("DER value runs past end of input")?;
    let whole = &data[tag_start..value_end];
    Ok((Tlv { whole, value }, value_end))
}
