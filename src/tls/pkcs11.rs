//! TLS private keys in an HSM or a cloud KMS, through PKCS#11 (18.23.0, #79).
//!
//! `tls_key_file` (and a virtual host's `key_file`) may be an RFC 7512 URI
//! instead of a path:
//!
//! ```text
//! pkcs11:token=rws;object=web-key?module-path=/usr/lib/softhsm/libsofthsm2.so&pin-source=env:HSM_PIN
//! ```
//!
//! The private key stays on the token: rws asks the token to sign each TLS
//! handshake (ECDSA P-256 / P-384, RSA-PSS or RSA PKCS#1 v1.5). The module
//! is loaded at runtime; one session per key, logged in once, reopened when
//! the token drops it. Path attributes: `token` (the token label), `object`
//! (the key's label), `id` (its `CKA_ID`, percent-encoded); query
//! attributes: `module-path`, and the PIN as `pin-source=env:VAR`,
//! `pin-source=file:/path` (RFC 7512's form) or `pin-value=…` (discouraged:
//! it ends up in the config).

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use cryptoki::context::{CInitializeArgs, CInitializeFlags, Pkcs11};
use cryptoki::mechanism::rsa::{PkcsMgfType, PkcsPssParams};
use cryptoki::mechanism::{Mechanism, MechanismType};
use cryptoki::object::{Attribute, AttributeType, KeyType, ObjectClass, ObjectHandle};
use cryptoki::session::{Session, UserType};
use cryptoki::slot::Slot;
use cryptoki::types::AuthPin;
use rustls::sign::{Signer, SigningKey};
use rustls::{SignatureAlgorithm, SignatureScheme};

/// A parsed `pkcs11:` URI.
#[derive(Debug, Clone, PartialEq)]
pub struct Pkcs11Uri {
    pub token: Option<String>,
    pub object: Option<String>,
    pub id: Option<Vec<u8>>,
    pub module_path: String,
    /// Where the PIN comes from: `env:VAR`, `file:/path`, or a literal.
    pub pin: Option<PinSource>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PinSource {
    Env(String),
    File(String),
    Value(String),
}

impl PinSource {
    fn read(&self) -> Result<String, String> {
        match self {
            PinSource::Env(var) => std::env::var(var).map(|p| p.trim().to_string()).map_err(|_| format!("PKCS#11 PIN variable {var} is unset")),
            PinSource::File(path) => std::fs::read_to_string(path).map(|p| p.trim().to_string()).map_err(|e| format!("PKCS#11 PIN file {path}: {e}")),
            PinSource::Value(v) => Ok(v.clone()),
        }
    }
}

fn pct(s: &str) -> Vec<u8> {
    crate::waf::decode::percent_decode(s).into_bytes()
}

impl Pkcs11Uri {
    /// Whether `value` is a `pkcs11:` URI rather than a file path.
    pub fn is_uri(value: &str) -> bool {
        value.trim_start().get(..7).is_some_and(|p| p.eq_ignore_ascii_case("pkcs11:"))
    }

    pub fn parse(uri: &str) -> Result<Pkcs11Uri, String> {
        let uri = uri.trim();
        if !Self::is_uri(uri) {
            return Err(format!("{uri:?} is not a pkcs11: URI"));
        }
        let rest = &uri[7..];
        let (path, query) = rest.split_once('?').unwrap_or((rest, ""));
        let mut parsed = Pkcs11Uri { token: None, object: None, id: None, module_path: String::new(), pin: None };
        for attr in path.split(';').filter(|a| !a.is_empty()) {
            let (k, v) = attr.split_once('=').ok_or_else(|| format!("PKCS#11 URI attribute {attr:?} has no value"))?;
            let text = || String::from_utf8_lossy(&pct(v)).into_owned();
            match k {
                "token" => parsed.token = Some(text()),
                "object" => parsed.object = Some(text()),
                "id" => parsed.id = Some(pct(v)),
                // Accepted and ignored: rws looks for a private key anyway.
                "type" | "manufacturer" | "model" | "serial" | "library-description" | "library-manufacturer" | "library-version" | "slot-id" | "slot-description" | "slot-manufacturer" => {}
                other => return Err(format!("PKCS#11 URI: unknown attribute {other:?}")),
            }
        }
        for attr in query.split('&').filter(|a| !a.is_empty()) {
            let (k, v) = attr.split_once('=').ok_or_else(|| format!("PKCS#11 URI query attribute {attr:?} has no value"))?;
            let text = String::from_utf8_lossy(&pct(v)).into_owned();
            match k {
                "module-path" => parsed.module_path = text,
                "pin-value" => parsed.pin = Some(PinSource::Value(text)),
                "pin-source" => {
                    parsed.pin = Some(match text.strip_prefix("env:") {
                        Some(var) => PinSource::Env(var.to_string()),
                        None => PinSource::File(text.strip_prefix("file:").unwrap_or(&text).trim_start_matches("//").to_string()),
                    })
                }
                "module-name" => {}
                other => return Err(format!("PKCS#11 URI: unknown query attribute {other:?}")),
            }
        }
        if parsed.module_path.is_empty() {
            return Err("PKCS#11 URI needs module-path=<the token's .so/.dll>".to_string());
        }
        if parsed.object.is_none() && parsed.id.is_none() {
            return Err("PKCS#11 URI needs object=<key label> or id=<key id>".to_string());
        }
        Ok(parsed)
    }
}

/// One initialized library per module path for the whole process.
fn module(path: &str) -> Result<Pkcs11, String> {
    static MODULES: OnceLock<Mutex<HashMap<String, Pkcs11>>> = OnceLock::new();
    let mut modules = MODULES.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
    if let Some(m) = modules.get(path) {
        return Ok(m.clone());
    }
    let ctx = Pkcs11::new(path).map_err(|e| format!("PKCS#11 module {path}: {e}"))?;
    match ctx.initialize(CInitializeArgs::new(CInitializeFlags::OS_LOCKING_OK)) {
        Ok(()) | Err(cryptoki::error::Error::Pkcs11(cryptoki::error::RvError::CryptokiAlreadyInitialized, _)) => {}
        Err(e) => return Err(format!("PKCS#11 module {path}: C_Initialize: {e}")),
    }
    modules.insert(path.to_string(), ctx.clone());
    Ok(ctx)
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Kind {
    P256,
    P384,
    Rsa,
}

/// A private key on a token, as a rustls [`SigningKey`].
pub struct Pkcs11Key {
    ctx: Pkcs11,
    slot: Slot,
    uri: Pkcs11Uri,
    kind: Kind,
    session: Mutex<Option<(Session, ObjectHandle)>>,
}

impl std::fmt::Debug for Pkcs11Key {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Pkcs11Key").field("token", &self.uri.token).field("object", &self.uri.object).field("kind", &self.kind).finish()
    }
}

const P256_OID: &[u8] = &[0x06, 0x08, 0x2a, 0x86, 0x48, 0xce, 0x3d, 0x03, 0x01, 0x07];
const P384_OID: &[u8] = &[0x06, 0x05, 0x2b, 0x81, 0x04, 0x00, 0x22];

impl Pkcs11Key {
    /// Find the key `uri` names, logging in to its token.
    pub fn open(uri: &str) -> Result<Pkcs11Key, String> {
        let uri = Pkcs11Uri::parse(uri)?;
        let ctx = module(&uri.module_path)?;
        let slots = ctx.get_slots_with_token().map_err(|e| format!("PKCS#11: listing tokens: {e}"))?;
        let slot = slots
            .into_iter()
            .find(|s| match &uri.token {
                Some(label) => ctx.get_token_info(*s).is_ok_and(|t| t.label().trim() == label),
                None => true,
            })
            .ok_or_else(|| format!("PKCS#11: no token {:?} in {}", uri.token.as_deref().unwrap_or("(any)"), uri.module_path))?;
        let mut key = Pkcs11Key { ctx, slot, uri, kind: Kind::Rsa, session: Mutex::new(None) };
        key.kind = key.with_session(|session, handle| {
            let attrs = session.get_attributes(handle, &[AttributeType::KeyType, AttributeType::EcParams]).map_err(|e| e.to_string())?;
            let mut kind = None;
            for a in attrs {
                match a {
                    Attribute::KeyType(t) if t == KeyType::RSA => kind = Some(Kind::Rsa),
                    Attribute::EcParams(p) if p == P256_OID => kind = Some(Kind::P256),
                    Attribute::EcParams(p) if p == P384_OID => kind = Some(Kind::P384),
                    _ => {}
                }
            }
            kind.ok_or_else(|| "the key is neither RSA nor EC P-256/P-384".to_string())
        })?;
        Ok(key)
    }

    /// Run `f` with a logged-in session and the key's handle; a session the
    /// token dropped is reopened once.
    fn with_session<T>(&self, f: impl Fn(&Session, ObjectHandle) -> Result<T, String>) -> Result<T, String> {
        let mut guard = self.session.lock().unwrap_or_else(|e| e.into_inner());
        for attempt in 0..2 {
            if guard.is_none() {
                *guard = Some(self.login()?);
            }
            let (session, handle) = guard.as_ref().expect("just opened");
            match f(session, *handle) {
                Ok(v) => return Ok(v),
                Err(e) if attempt == 0 => {
                    // Session closed, logged out, token reinserted: start over.
                    eprintln!("[pkcs11] {e}; reopening the session");
                    *guard = None;
                }
                Err(e) => return Err(e),
            }
        }
        unreachable!("the loop returns")
    }

    fn login(&self) -> Result<(Session, ObjectHandle), String> {
        let session = self.ctx.open_ro_session(self.slot).map_err(|e| format!("PKCS#11: opening a session: {e}"))?;
        if let Some(source) = &self.uri.pin {
            let pin = AuthPin::new(source.read()?.into());
            match session.login(UserType::User, Some(&pin)) {
                Ok(()) | Err(cryptoki::error::Error::Pkcs11(cryptoki::error::RvError::UserAlreadyLoggedIn, _)) => {}
                Err(e) => return Err(format!("PKCS#11: login: {e}")),
            }
        }
        let mut template = vec![Attribute::Class(ObjectClass::PRIVATE_KEY)];
        if let Some(label) = &self.uri.object {
            template.push(Attribute::Label(label.as_bytes().to_vec()));
        }
        if let Some(id) = &self.uri.id {
            template.push(Attribute::Id(id.clone()));
        }
        let handle = session
            .find_objects(&template)
            .map_err(|e| format!("PKCS#11: finding the key: {e}"))?
            .into_iter()
            .next()
            .ok_or_else(|| format!("PKCS#11: no private key {:?} on token {:?}", self.uri.object.as_deref().unwrap_or("(by id)"), self.uri.token.as_deref().unwrap_or("(any)")))?;
        Ok((session, handle))
    }
}

/// `r || s` (each `n` bytes) as the DER `ECDSA-Sig-Value` TLS carries.
pub fn ecdsa_der(raw: &[u8]) -> Vec<u8> {
    let half = raw.len() / 2;
    let int = |bytes: &[u8]| {
        let trimmed: &[u8] = {
            let start = bytes.iter().position(|b| *b != 0).unwrap_or(bytes.len().saturating_sub(1));
            &bytes[start..]
        };
        let mut v = vec![0x02];
        let pad = trimmed.first().is_some_and(|b| b & 0x80 != 0);
        v.push((trimmed.len() + pad as usize) as u8);
        if pad {
            v.push(0);
        }
        v.extend_from_slice(trimmed);
        v
    };
    let body = [int(&raw[..half]), int(&raw[half..])].concat();
    let mut out = vec![0x30];
    if body.len() < 0x80 {
        out.push(body.len() as u8);
    } else {
        out.extend_from_slice(&[0x81, body.len() as u8]);
    }
    out.extend(body);
    out
}

struct Pkcs11Signer {
    key: Arc<Pkcs11Key>,
    scheme: SignatureScheme,
}

impl std::fmt::Debug for Pkcs11Signer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Pkcs11Signer({:?})", self.scheme)
    }
}

impl Signer for Pkcs11Signer {
    fn sign(&self, message: &[u8]) -> Result<Vec<u8>, rustls::Error> {
        let fail = |e: String| rustls::Error::General(format!("PKCS#11 signing: {e}"));
        // CK_ULONG is 32 bits on Windows: convert from usize, which fits either way.
        let pss = |hash: MechanismType, mgf: PkcsMgfType, len: usize| PkcsPssParams { hash_alg: hash, mgf, s_len: cryptoki::types::Ulong::try_from(len).expect("a salt length fits CK_ULONG") };
        match self.scheme {
            SignatureScheme::ECDSA_NISTP256_SHA256 | SignatureScheme::ECDSA_NISTP384_SHA384 => {
                let algorithm = if self.scheme == SignatureScheme::ECDSA_NISTP256_SHA256 { &aws_lc_rs::digest::SHA256 } else { &aws_lc_rs::digest::SHA384 };
                let digest = aws_lc_rs::digest::digest(algorithm, message);
                let raw = self.key.with_session(|s, k| s.sign(&Mechanism::Ecdsa, k, digest.as_ref()).map_err(|e| e.to_string())).map_err(fail)?;
                Ok(ecdsa_der(&raw))
            }
            SignatureScheme::RSA_PSS_SHA256 => self.key.with_session(|s, k| s.sign(&Mechanism::Sha256RsaPkcsPss(pss(MechanismType::SHA256, PkcsMgfType::MGF1_SHA256, 32)), k, message).map_err(|e| e.to_string())).map_err(fail),
            SignatureScheme::RSA_PSS_SHA384 => self.key.with_session(|s, k| s.sign(&Mechanism::Sha384RsaPkcsPss(pss(MechanismType::SHA384, PkcsMgfType::MGF1_SHA384, 48)), k, message).map_err(|e| e.to_string())).map_err(fail),
            SignatureScheme::RSA_PKCS1_SHA256 => self.key.with_session(|s, k| s.sign(&Mechanism::Sha256RsaPkcs, k, message).map_err(|e| e.to_string())).map_err(fail),
            SignatureScheme::RSA_PKCS1_SHA384 => self.key.with_session(|s, k| s.sign(&Mechanism::Sha384RsaPkcs, k, message).map_err(|e| e.to_string())).map_err(fail),
            other => Err(fail(format!("unsupported scheme {other:?}"))),
        }
    }

    fn scheme(&self) -> SignatureScheme {
        self.scheme
    }
}

/// A [`Pkcs11Key`] behind an `Arc`, so signers can share it.
#[derive(Debug)]
pub struct Pkcs11SigningKey(Arc<Pkcs11Key>);

impl SigningKey for Pkcs11SigningKey {
    fn choose_scheme(&self, offered: &[SignatureScheme]) -> Option<Box<dyn Signer>> {
        let preference: &[SignatureScheme] = match self.0.kind {
            Kind::P256 => &[SignatureScheme::ECDSA_NISTP256_SHA256],
            Kind::P384 => &[SignatureScheme::ECDSA_NISTP384_SHA384],
            Kind::Rsa => &[SignatureScheme::RSA_PSS_SHA256, SignatureScheme::RSA_PSS_SHA384, SignatureScheme::RSA_PKCS1_SHA256, SignatureScheme::RSA_PKCS1_SHA384],
        };
        let scheme = preference.iter().find(|s| offered.contains(s))?;
        Some(Box::new(Pkcs11Signer { key: Arc::clone(&self.0), scheme: *scheme }))
    }

    fn algorithm(&self) -> SignatureAlgorithm {
        match self.0.kind {
            Kind::P256 | Kind::P384 => SignatureAlgorithm::ECDSA,
            Kind::Rsa => SignatureAlgorithm::RSA,
        }
    }
}

/// The signing key a `pkcs11:` URI names.
pub fn signing_key(uri: &str) -> Result<Arc<dyn SigningKey>, String> {
    Ok(Arc::new(Pkcs11SigningKey(Arc::new(Pkcs11Key::open(uri)?))))
}

#[cfg(test)]
#[path = "pkcs11_tests.rs"]
mod tests;
