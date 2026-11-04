//! [`Uuid`]: a 128-bit RFC 9562 UUID for primary keys (18.30.0).
//!
//! rws has its own rather than the `uuid` crate: it isn't in the dependency
//! tree, and a key needs only parsing, formatting and generation — v7
//! (time-ordered, so new rows land at the end of a B-tree index) and v4
//! (random), from the operating system's generator through aws-lc-rs,
//! which every `model-*` build already links.
//!
//! Stored as its 36-character lower-case text on every backend: a `TEXT` /
//! `CHAR(36)` / `VARCHAR(36)` column. PostgreSQL `UUID` columns are read
//! too, but rws binds text, which PostgreSQL won't compare with or assign to
//! `UUID` without a cast — use a text column there.

use super::{DbError, FromColumn, ToColumn, Value};

/// A UUID, kept as its 16 bytes.
///
/// ```rust
/// # extern crate rws_server as rust_web_server;
/// use rust_web_server::model::Uuid;
///
/// let id = Uuid::now_v7();
/// assert_eq!(id.version(), 7);
/// assert_eq!(Uuid::parse(&id.to_string()).unwrap(), id);
/// assert!(Uuid::default().is_nil());
/// ```
#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Uuid([u8; 16]);

impl Uuid {
    /// `00000000-0000-0000-0000-000000000000`, also `Uuid::default()`: a key
    /// not chosen yet — `save` generates a v7 in its place.
    pub const NIL: Uuid = Uuid([0; 16]);

    /// The UUID with these bytes.
    pub const fn from_bytes(bytes: [u8; 16]) -> Uuid {
        Uuid(bytes)
    }

    /// Its 16 bytes.
    pub const fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }

    /// Whether it is [`Uuid::NIL`].
    pub fn is_nil(&self) -> bool {
        self.0 == [0; 16]
    }

    /// The version field (4 for [`Uuid::new_v4`], 7 for [`Uuid::now_v7`]).
    pub fn version(&self) -> u8 {
        self.0[6] >> 4
    }

    /// A random (version 4) UUID.
    pub fn new_v4() -> Uuid {
        let mut bytes = random_bytes();
        bytes[6] = (bytes[6] & 0x0f) | 0x40;
        bytes[8] = (bytes[8] & 0x3f) | 0x80;
        Uuid(bytes)
    }

    /// A time-ordered (version 7) UUID: the Unix time in milliseconds, then
    /// 74 random bits. Two made in the same millisecond are in random order.
    pub fn now_v7() -> Uuid {
        use std::time::{SystemTime, UNIX_EPOCH};
        let millis = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0);
        let mut bytes = random_bytes();
        bytes[..6].copy_from_slice(&millis.to_be_bytes()[2..]);
        bytes[6] = (bytes[6] & 0x0f) | 0x70;
        bytes[8] = (bytes[8] & 0x3f) | 0x80;
        Uuid(bytes)
    }

    /// Parse the hyphenated form (`67e55044-10b1-426f-9247-bb680e5fe0c8`),
    /// either case, or the 32 hex digits without hyphens.
    pub fn parse(text: &str) -> Result<Uuid, DbError> {
        parse(text.trim()).ok_or_else(|| {
            DbError::new(format!("cannot read {text:?} as a UUID: expected 8-4-4-4-12 hex digits, e.g. 67e55044-10b1-426f-9247-bb680e5fe0c8"))
        })
    }
}

fn random_bytes() -> [u8; 16] {
    use aws_lc_rs::rand::SecureRandom;
    let mut bytes = [0u8; 16];
    // The system generator failing is not something a caller can handle;
    // aws-lc aborts on it itself in FIPS mode.
    aws_lc_rs::rand::SystemRandom::new().fill(&mut bytes).expect("the operating system's random number generator failed");
    bytes
}

fn parse(text: &str) -> Option<Uuid> {
    let hex: Vec<u8> = match text.len() {
        36 => {
            let b = text.as_bytes();
            if [8, 13, 18, 23].iter().any(|&i| b[i] != b'-') {
                return None;
            }
            b.iter().copied().filter(|&c| c != b'-').collect()
        }
        32 => text.as_bytes().to_vec(),
        _ => return None,
    };
    if hex.len() != 32 {
        return None;
    }
    let mut bytes = [0u8; 16];
    for (i, pair) in hex.chunks(2).enumerate() {
        let hi = (pair[0] as char).to_digit(16)?;
        let lo = (pair[1] as char).to_digit(16)?;
        bytes[i] = (hi * 16 + lo) as u8;
    }
    Some(Uuid(bytes))
}

impl std::fmt::Display for Uuid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (i, b) in self.0.iter().enumerate() {
            if matches!(i, 4 | 6 | 8 | 10) {
                f.write_str("-")?;
            }
            write!(f, "{b:02x}")?;
        }
        Ok(())
    }
}

impl std::fmt::Debug for Uuid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Uuid({self})")
    }
}

impl std::str::FromStr for Uuid {
    type Err = DbError;
    fn from_str(s: &str) -> Result<Self, DbError> {
        Uuid::parse(s)
    }
}

impl FromColumn for Uuid {
    fn from_column(v: Value) -> Result<Self, DbError> {
        match v {
            Value::Text(s) => Uuid::parse(&s),
            Value::Bytes(b) if b.len() == 16 => {
                let mut bytes = [0u8; 16];
                bytes.copy_from_slice(&b);
                Ok(Uuid(bytes))
            }
            Value::Null => Err(DbError::new("unexpected NULL for Uuid (use Option<Uuid>)")),
            other => Err(DbError::new(format!("cannot convert {other:?} to Uuid"))),
        }
    }
}

impl ToColumn for Uuid {
    fn to_column(&self) -> Value {
        Value::Text(self.to_string())
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for Uuid {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for Uuid {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Uuid::parse(&text).map_err(|e| serde::de::Error::custom(e.0))
    }
}
