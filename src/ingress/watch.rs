//! Kubernetes "watch" stream reader.
//!
//! Turns a `?watch=true` streaming response into a low-latency "something
//! changed" signal that triggers a full re-list, rather than
//! incrementally applying `ADDED`/`MODIFIED`/`DELETED` events to an
//! in-memory per-object cache.
//!
//! A fully correct watch client tracks a `resourceVersion` bookmark,
//! applies each event as a delta, and re-lists from scratch on a
//! `410 Gone` (the bookmark expired). That's meaningfully more surface
//! area to get subtly wrong (missed deletes, duplicate or stale entries,
//! resourceVersion bookkeeping across reconnects) than this crate's
//! existing polling loop ever had to handle. Reading every event line as
//! a plain trigger for [`super::KubernetesIngressWatcher::poll`] still
//! delivers both benefits the gap this closes named: a quiet cluster
//! leaves this thread blocked on a read with no polling work at all, and
//! a real change unblocks that read — and triggers a re-list — as soon as
//! the API server sends it, instead of waiting up to `poll_interval_secs`.
//! The existing interval-based resync loop keeps running unchanged
//! alongside this as a safety net (and as the sole mechanism if the watch
//! connection can't be established at all, e.g. the API server doesn't
//! support it for some reason).

use std::io::Read;

/// Read a `Transfer-Encoding: chunked` HTTP response from `read` — first
/// consuming the status line + headers (returning an error for a non-2xx
/// status), then decoding the chunked body into newline-delimited lines,
/// calling `on_line` for each non-empty one.
///
/// Returns `Ok(())` once the stream ends cleanly (last chunk or EOF) —
/// the caller is expected to reconnect after a backoff. Blocks on `read`
/// according to whatever timeout the caller configured on the underlying
/// stream; a timeout surfaces as an `Err` like any other I/O error.
pub(crate) fn read_chunked_lines(
    mut read: impl Read,
    mut on_line: impl FnMut(&str),
) -> Result<(), String> {
    let mut buf: Vec<u8> = Vec::new();
    let mut tmp = [0u8; 4096];

    let header_end = loop {
        if let Some(pos) = find(&buf, b"\r\n\r\n") {
            break pos + 4;
        }
        let n = read
            .read(&mut tmp)
            .map_err(|e| format!("ingress watcher: watch stream read failed: {e}"))?;
        if n == 0 {
            return Err("ingress watcher: connection closed before watch response headers completed".to_string());
        }
        buf.extend_from_slice(&tmp[..n]);
    };

    let header_str = std::str::from_utf8(&buf[..header_end]).unwrap_or("");
    let status_line = header_str.lines().next().unwrap_or("");
    let status: u16 = status_line
        .splitn(3, ' ')
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    if !(200..300).contains(&status) {
        return Err(format!("ingress watcher: watch request returned status {status}"));
    }

    let mut leftover = buf[header_end..].to_vec();
    let mut line_buf: Vec<u8> = Vec::new();

    loop {
        loop {
            let Some(line_end) = find(&leftover, b"\r\n") else { break };
            let size_line = std::str::from_utf8(&leftover[..line_end])
                .map_err(|_| "ingress watcher: non-ASCII chunk size".to_string())?
                .trim();
            let size_str = size_line.split(';').next().unwrap_or("").trim();
            if size_str.is_empty() {
                break;
            }
            let chunk_size = match usize::from_str_radix(size_str, 16) {
                Ok(n) => n,
                Err(_) => return Err(format!("ingress watcher: invalid chunk size '{size_str}'")),
            };
            let chunk_start = line_end + 2;
            let chunk_end = chunk_start + chunk_size;
            if leftover.len() < chunk_end + 2 {
                break; // wait for the rest of this chunk to arrive
            }
            if chunk_size == 0 {
                return Ok(()); // last-chunk marker — stream is done
            }
            line_buf.extend_from_slice(&leftover[chunk_start..chunk_end]);
            leftover.drain(..chunk_end + 2);

            while let Some(nl) = line_buf.iter().position(|&b| b == b'\n') {
                let line_bytes: Vec<u8> = line_buf.drain(..=nl).collect();
                let line = std::str::from_utf8(&line_bytes).unwrap_or("").trim();
                if !line.is_empty() {
                    on_line(line);
                }
            }
        }

        let n = match read.read(&mut tmp) {
            Ok(0) => return Ok(()),
            Ok(n) => n,
            // See the identical check in `super::tls::https_get` — a TLS
            // stream reports an abrupt close (no `close_notify`) as an
            // error, not EOF; treat it as the connection simply ending, the
            // same outcome as `Ok(0)`, since the caller reconnects either way.
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(()),
            Err(e) => return Err(format!("ingress watcher: watch stream read failed: {e}")),
        };
        leftover.extend_from_slice(&tmp[..n]);
    }
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if haystack.len() < needle.len() {
        return None;
    }
    haystack.windows(needle.len()).position(|w| w == needle)
}
