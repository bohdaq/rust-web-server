//! Keep rws from filling a disk (18.15.0): before writing to a file it
//! keeps growing — the access log, the disk cache — rws asks
//! [`has_room`]. Below `RWS_CONFIG_MIN_FREE_DISK` free on that file's
//! filesystem the write is skipped (and counted) until space comes back:
//! a full disk costs log lines and cache entries, never the server or
//! whatever shares the disk with it.
//!
//! `RWS_CONFIG_MIN_FREE_DISK`: a size (`2G`, `500M`) or a share of the
//! filesystem (`10%`); unset or empty means the larger of 1% and 1 GiB (the
//! space counted is what an unprivileged process may use, so the root
//! reserve most filesystems keep is already set aside); `0` turns the guard
//! off. Free space is read (`statvfs`) at most every
//! [`RECHECK`] per directory. One line is logged when a directory runs low
//! and one when it recovers.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

/// How long a free-space reading is reused.
pub const RECHECK: Duration = Duration::from_secs(5);

/// `RWS_CONFIG_MIN_FREE_DISK`.
pub const RWS_CONFIG_MIN_FREE_DISK: &str = "RWS_CONFIG_MIN_FREE_DISK";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Floor {
    Off,
    Bytes(u64),
    Percent(u64),
    /// The default: the larger of 1% and 1 GiB.
    Default,
}

fn floor() -> Floor {
    static FLOOR: OnceLock<Floor> = OnceLock::new();
    *FLOOR.get_or_init(|| {
        let value = std::env::var(RWS_CONFIG_MIN_FREE_DISK).unwrap_or_default();
        // Tests don't depend on how full the machine running them is.
        if cfg!(test) && value.trim().is_empty() {
            return Floor::Off;
        }
        parse_floor(&value).unwrap_or_else(|| {
            eprintln!("{RWS_CONFIG_MIN_FREE_DISK}: not a size or a percentage; using the default (1%, at least 1G)");
            Floor::Default
        })
    })
}

fn parse_floor(value: &str) -> Option<Floor> {
    let value = value.trim();
    if value.is_empty() {
        return Some(Floor::Default);
    }
    if let Some(percent) = value.strip_suffix('%') {
        let percent: u64 = percent.trim().parse().ok().filter(|p| *p <= 100)?;
        return Some(if percent == 0 { Floor::Off } else { Floor::Percent(percent) });
    }
    let bytes = crate::log::access_sink::parse_size(value)?;
    Some(if bytes == 0 { Floor::Off } else { Floor::Bytes(bytes) })
}

/// Bytes that must stay free on a filesystem of `total` bytes.
fn required(floor: Floor, total: u64) -> u64 {
    match floor {
        Floor::Off => 0,
        Floor::Bytes(bytes) => bytes,
        Floor::Percent(percent) => total / 100 * percent,
        Floor::Default => (total / 100).max(1 << 30).min(total / 2),
    }
}

/// `(available to unprivileged writers, total)` bytes of the filesystem
/// holding `dir`.
#[cfg(all(unix, any(feature = "http1", feature = "http2")))]
fn space(dir: &Path) -> Option<(u64, u64)> {
    use std::os::unix::ffi::OsStrExt;
    let path = std::ffi::CString::new(dir.as_os_str().as_bytes()).ok()?;
    // SAFETY: `stat` is plain old data written by statvfs; `path` is a valid
    // NUL-terminated string that outlives the call.
    let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statvfs(path.as_ptr(), &mut stat) } != 0 {
        return None;
    }
    let block = stat.f_frsize as u64;
    Some((stat.f_bavail as u64 * block, stat.f_blocks as u64 * block))
}

#[cfg(not(all(unix, any(feature = "http1", feature = "http2"))))]
fn space(_dir: &Path) -> Option<(u64, u64)> {
    None
}

struct Reading {
    at: Instant,
    free: u64,
    total: u64,
    low: bool,
}

fn readings() -> &'static Mutex<HashMap<PathBuf, Reading>> {
    static READINGS: OnceLock<Mutex<HashMap<PathBuf, Reading>>> = OnceLock::new();
    READINGS.get_or_init(Mutex::default)
}

/// The directory whose filesystem `file` lives on.
fn directory_of(file: &Path) -> PathBuf {
    if file.is_dir() {
        return file.to_path_buf();
    }
    match file.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.to_path_buf(),
        _ => PathBuf::from("."),
    }
}

/// Whether there's room to keep writing to `file` (or into the directory
/// `file`). `true` when the guard is off or the space can't be read.
pub fn has_room(file: &Path) -> bool {
    has_room_with(file, floor())
}

fn has_room_with(file: &Path, floor: Floor) -> bool {
    if floor == Floor::Off {
        return true;
    }
    let dir = directory_of(file);
    let mut readings = readings().lock().unwrap_or_else(|e| e.into_inner());
    if let Some(reading) = readings.get(&dir) {
        if reading.at.elapsed() < RECHECK && !cfg!(test) {
            return !reading.low;
        }
    }
    let Some((free, total)) = space(&dir) else { return true };
    let low = free < required(floor, total);
    let was_low = readings.get(&dir).is_some_and(|r| r.low);
    if low && !was_low {
        eprintln!(
            "disk: {} has {} MiB free, under the {} MiB {RWS_CONFIG_MIN_FREE_DISK} floor; rws stops writing there until space is freed",
            dir.display(),
            free >> 20,
            required(floor, total) >> 20
        );
    } else if !low && was_low {
        eprintln!("disk: {} has {} MiB free again; rws writes there again", dir.display(), free >> 20);
    }
    readings.insert(dir, Reading { at: Instant::now(), free, total, low });
    !low
}

static DROPPED_LOG_LINES: AtomicU64 = AtomicU64::new(0);
static SKIPPED_CACHE_WRITES: AtomicU64 = AtomicU64::new(0);

/// Access-log lines not written for want of space.
pub(crate) fn dropped_log_lines(lines: u64) {
    DROPPED_LOG_LINES.fetch_add(lines, Ordering::Relaxed);
}

/// Disk-cache entries not stored for want of space.
pub(crate) fn skipped_cache_write() {
    SKIPPED_CACHE_WRITES.fetch_add(1, Ordering::Relaxed);
}

/// Every directory checked so far: `(directory, free bytes, total bytes,
/// low)`, sorted.
pub fn status() -> Vec<(String, u64, u64, bool)> {
    let readings = readings().lock().unwrap_or_else(|e| e.into_inner());
    let mut out: Vec<_> = readings.iter().map(|(dir, r)| (dir.display().to_string(), r.free, r.total, r.low)).collect();
    out.sort();
    out
}

/// `rws_disk_free_bytes{dir}`, `rws_disk_low{dir}` and the drop counters;
/// empty until a directory has been checked.
pub(crate) fn prometheus_text() -> String {
    let status = status();
    if status.is_empty() {
        return String::new();
    }
    let label = |dir: &str| dir.replace('\\', "\\\\").replace('"', "\\\"");
    let mut out = String::from("# HELP rws_disk_free_bytes Free space where rws writes (access log, disk cache)\n# TYPE rws_disk_free_bytes gauge\n");
    for (dir, free, _, _) in &status {
        out.push_str(&format!("rws_disk_free_bytes{{dir=\"{}\"}} {free}\n", label(dir)));
    }
    out.push_str(&format!("# HELP rws_disk_low 1 while a directory is under {RWS_CONFIG_MIN_FREE_DISK} and rws isn't writing there\n# TYPE rws_disk_low gauge\n"));
    for (dir, _, _, low) in &status {
        out.push_str(&format!("rws_disk_low{{dir=\"{}\"}} {}\n", label(dir), *low as u8));
    }
    out.push_str("# HELP rws_access_log_dropped_lines_total Access-log lines not written because the disk was low\n# TYPE rws_access_log_dropped_lines_total counter\n");
    out.push_str(&format!("rws_access_log_dropped_lines_total {}\n", DROPPED_LOG_LINES.load(Ordering::Relaxed)));
    out.push_str("# HELP rws_cache_skipped_writes_total Disk-cache entries not stored because the disk was low\n# TYPE rws_cache_skipped_writes_total counter\n");
    out.push_str(&format!("rws_cache_skipped_writes_total {}\n", SKIPPED_CACHE_WRITES.load(Ordering::Relaxed)));
    out
}

#[cfg(test)]
mod tests;
