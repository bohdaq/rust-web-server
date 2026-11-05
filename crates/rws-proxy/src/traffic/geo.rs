//! Country of an IP address, from a table compiled into the binary (the
//! `geo` feature): DB-IP's "IP to Country Lite" (CC BY 4.0), converted by
//! `ci/gen_geo.py` into `geo.bin` — range starts and a country each,
//! deflated (~700 KB). It's decoded on first use into sorted arrays
//! (~5 MB) and looked up by binary search. `[traffic] geo_db` loads a newer
//! file of the same format instead.
//!
//! Only the country is used; the address isn't kept.

use std::net::IpAddr;
use std::sync::OnceLock;

/// A decoded table.
#[doc(hidden)]
pub struct Table {
    /// The source, e.g. `dbip-country-lite-2026-11`.
    pub source: String,
    countries: Vec<[u8; 2]>,
    v4: Vec<u32>,
    v4_country: Vec<u8>,
    v6: Vec<u64>,
    v6_country: Vec<u8>,
}

const MAGIC: &[u8] = b"RWSGEO1 ";
/// More ranges than any real table has: a corrupt count can't make us
/// allocate gigabytes.
const MAX_RANGES: usize = 4_000_000;

#[cfg(feature = "geo")]
static BUNDLED: &[u8] = include_bytes!("geo.bin");

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn varint(&mut self) -> Result<u64, String> {
        let mut n = 0u64;
        for shift in (0..64).step_by(7) {
            let b = *self.data.get(self.pos).ok_or("truncated")?;
            self.pos += 1;
            n |= u64::from(b & 0x7f) << shift;
            if b < 0x80 {
                return Ok(n);
            }
        }
        Err("varint too long".to_string())
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        let end = self.pos.checked_add(n).filter(|e| *e <= self.data.len()).ok_or("truncated")?;
        let s = &self.data[self.pos..end];
        self.pos = end;
        Ok(s)
    }

    fn starts(&mut self, n: usize, max: u64) -> Result<Vec<u64>, String> {
        let mut out = Vec::with_capacity(n);
        let mut value = 0u64;
        for i in 0..n {
            let d = self.varint()?;
            if i > 0 && d == 0 {
                return Err("range starts not ascending".to_string());
            }
            value = value.checked_add(d).filter(|v| *v <= max).ok_or("range start out of range")?;
            if i == 0 && value != 0 {
                return Err("first range doesn't start at 0".to_string());
            }
            out.push(value);
        }
        Ok(out)
    }
}

impl Table {
    /// Decode a table: `compressed` is the deflated (zlib) file.
    #[doc(hidden)]
    pub fn decode(compressed: &[u8]) -> Result<Table, String> {
        let data = inflate(compressed)?;
        Self::parse(&data)
    }

    /// Parse the inflated bytes.
    #[doc(hidden)]
    pub fn parse(data: &[u8]) -> Result<Table, String> {
        let rest = data.strip_prefix(MAGIC).ok_or("not a geo table")?;
        let nl = rest.iter().position(|b| *b == b'\n').filter(|n| *n <= 200).ok_or("bad header")?;
        let source = String::from_utf8_lossy(&rest[..nl]).into_owned();
        let mut r = Reader { data, pos: MAGIC.len() + nl + 1 };
        let nc = r.varint()? as usize;
        if nc == 0 || nc > 256 {
            return Err("bad country count".to_string());
        }
        let countries = r.take(nc * 2)?.chunks_exact(2).map(|c| [c[0], c[1]]).collect();
        let n4 = r.varint()? as usize;
        let n6 = r.varint()? as usize;
        // Each range takes at least two bytes (a start and a country): a
        // count the rest of the file can't hold is a damaged file, not a
        // reason to reserve memory.
        if n4 == 0 || n6 == 0 || n4 > MAX_RANGES || n6 > MAX_RANGES || (n4 + n6) * 2 > data.len() - r.pos {
            return Err("bad range count".to_string());
        }
        let v4 = r.starts(n4, u64::from(u32::MAX))?.into_iter().map(|v| v as u32).collect();
        let v6 = r.starts(n6, u64::MAX)?;
        let idx = r.take(n4 + n6)?;
        if r.pos != data.len() {
            return Err("trailing bytes".to_string());
        }
        if idx.iter().any(|i| *i as usize >= nc) {
            return Err("bad country index".to_string());
        }
        Ok(Table { source, countries, v4, v4_country: idx[..n4].to_vec(), v6, v6_country: idx[n4..].to_vec() })
    }

    /// The two-letter country code (upper case) of `ip`, `None` when the
    /// table has none (private, reserved and unassigned addresses).
    pub fn country(&self, ip: IpAddr) -> Option<[u8; 2]> {
        let i = match ip {
            IpAddr::V4(v4) => {
                let key = u32::from(v4);
                self.v4_country[self.v4.partition_point(|s| *s <= key) - 1]
            }
            IpAddr::V6(v6) => {
                if let Some(v4) = v6.to_ipv4_mapped() {
                    return self.country(IpAddr::V4(v4));
                }
                let key = (u128::from(v6) >> 64) as u64;
                self.v6_country[self.v6.partition_point(|s| *s <= key) - 1]
            }
        };
        let code = self.countries[i as usize];
        (code != *b"--").then_some(code)
    }

    /// IPv4 and IPv6 ranges.
    pub fn len(&self) -> (usize, usize) {
        (self.v4.len(), self.v6.len())
    }
}

/// Inflate at most 64 MiB (a real table is ~6 MB).
fn inflate(compressed: &[u8]) -> Result<Vec<u8>, String> {
    #[cfg(feature = "geo")]
    {
        use std::io::Read;
        let mut out = Vec::new();
        flate2::read::ZlibDecoder::new(compressed).take(64 << 20).read_to_end(&mut out).map_err(|e| format!("can't inflate: {e}"))?;
        Ok(out)
    }
    #[cfg(not(feature = "geo"))]
    {
        let _ = compressed;
        Err("built without the `geo` feature".to_string())
    }
}

/// The table in use: `[traffic] geo_db` if set and readable, else the one
/// built in; `None` without either. Decoded once.
pub fn table() -> Option<&'static Table> {
    static TABLE: OnceLock<Option<Table>> = OnceLock::new();
    TABLE
        .get_or_init(|| {
            if let Some(path) = super::config::geo_db() {
                match std::fs::read(&path).map_err(|e| e.to_string()).and_then(|bytes| Table::decode(&bytes)) {
                    Ok(table) => return Some(table),
                    Err(e) => eprintln!("traffic: can't load geo_db {}: {e}; using the built-in table", path.display()),
                }
            }
            bundled()
        })
        .as_ref()
}

#[cfg(feature = "geo")]
fn bundled() -> Option<Table> {
    match Table::decode(BUNDLED) {
        Ok(table) => Some(table),
        Err(e) => {
            eprintln!("traffic: built-in geo table: {e}");
            None
        }
    }
}

#[cfg(not(feature = "geo"))]
fn bundled() -> Option<Table> {
    None
}

/// The country of `ip`, as `"DE"`; `None` when unknown or without a table.
pub fn country(ip: IpAddr) -> Option<[u8; 2]> {
    table()?.country(ip)
}
