//! The few protobuf messages of Envoy's `ext_authz` API rws needs
//! (`envoy.service.auth.v3`, `envoy.config.core.v3`, `google.rpc.Status`),
//! encoded and decoded by hand: a `CheckRequest` with the HTTP request's
//! attributes, and the parts of a `CheckResponse` that decide.
//!
//! Field numbers from envoy/service/auth/v3/{external_auth,attribute_context}.proto
//! and envoy/config/core/v3/{address,base}.proto.

/// What the `CheckRequest` says about the client's request.
pub struct HttpAttributes<'a> {
    pub method: &'a str,
    /// The request target as sent: path and query.
    pub path: &'a str,
    pub host: &'a str,
    pub scheme: &'a str,
    /// Lower-case names.
    pub headers: &'a [(String, String)],
    pub body: &'a [u8],
    pub source_ip: &'a str,
}

fn varint(out: &mut Vec<u8>, mut value: u64) {
    while value >= 0x80 {
        out.push((value as u8) | 0x80);
        value >>= 7;
    }
    out.push(value as u8);
}

fn bytes_field(out: &mut Vec<u8>, field: u32, value: &[u8]) {
    varint(out, ((field as u64) << 3) | 2);
    varint(out, value.len() as u64);
    out.extend_from_slice(value);
}

fn string_field(out: &mut Vec<u8>, field: u32, value: &str) {
    if !value.is_empty() {
        bytes_field(out, field, value.as_bytes());
    }
}

fn varint_field(out: &mut Vec<u8>, field: u32, value: u64) {
    varint(out, (field as u64) << 3);
    varint(out, value);
}

/// `CheckRequest { attributes: AttributeContext { source, request: { http } } }`.
pub fn check_request(a: &HttpAttributes) -> Vec<u8> {
    // HttpRequest
    let mut http = Vec::new();
    string_field(&mut http, 2, a.method);
    for (name, value) in a.headers {
        let mut entry = Vec::new();
        string_field(&mut entry, 1, name);
        string_field(&mut entry, 2, value);
        bytes_field(&mut http, 3, &entry);
    }
    string_field(&mut http, 4, a.path);
    string_field(&mut http, 5, a.host);
    string_field(&mut http, 6, a.scheme);
    if let Some((_, query)) = a.path.split_once('?') {
        string_field(&mut http, 7, query);
    }
    varint_field(&mut http, 9, a.body.len() as u64);
    string_field(&mut http, 10, "HTTP/1.1");
    if !a.body.is_empty() {
        bytes_field(&mut http, 12, a.body);
    }
    // AttributeContext.Request { http = 2 }
    let mut request = Vec::new();
    bytes_field(&mut request, 2, &http);
    // AttributeContext.Peer { address = 1: Address { socket_address = 1: { address = 2 } } }
    let mut socket_address = Vec::new();
    string_field(&mut socket_address, 2, a.source_ip);
    let mut address = Vec::new();
    bytes_field(&mut address, 1, &socket_address);
    let mut source = Vec::new();
    bytes_field(&mut source, 1, &address);
    // AttributeContext { source = 1, request = 4 }
    let mut context = Vec::new();
    bytes_field(&mut context, 1, &source);
    bytes_field(&mut context, 4, &request);
    let mut out = Vec::new();
    bytes_field(&mut out, 1, &context);
    out
}

/// One field: its number and a varint or length-delimited value.
enum Value<'a> {
    Varint(u64),
    Bytes(&'a [u8]),
}

fn read_varint(input: &[u8], at: &mut usize) -> Result<u64, String> {
    let mut value = 0u64;
    for shift in (0..64).step_by(7) {
        let byte = *input.get(*at).ok_or("truncated varint")?;
        *at += 1;
        value |= u64::from(byte & 0x7f) << shift;
        if byte < 0x80 {
            return Ok(value);
        }
    }
    Err("varint too long".to_string())
}

/// The fields of a message (fixed-width fields are skipped).
fn fields(input: &[u8]) -> Result<Vec<(u64, Value<'_>)>, String> {
    let mut at = 0;
    let mut out = Vec::new();
    while at < input.len() {
        let key = read_varint(input, &mut at)?;
        let (field, wire) = (key >> 3, key & 7);
        match wire {
            0 => out.push((field, Value::Varint(read_varint(input, &mut at)?))),
            2 => {
                let len = read_varint(input, &mut at)? as usize;
                let end = at.checked_add(len).filter(|e| *e <= input.len()).ok_or("truncated field")?;
                out.push((field, Value::Bytes(&input[at..end])));
                at = end;
            }
            1 => at = at.checked_add(8).filter(|e| *e <= input.len()).ok_or("truncated field")?,
            5 => at = at.checked_add(4).filter(|e| *e <= input.len()).ok_or("truncated field")?,
            other => return Err(format!("unsupported wire type {other}")),
        }
    }
    Ok(out)
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// The deciding parts of a `CheckResponse`.
#[derive(Debug, Default, PartialEq)]
pub struct Verdict {
    /// `status.code`: `0` (OK) allows.
    pub code: i32,
    /// `ok_response.headers`, or `denied_response.headers`.
    pub headers: Vec<(String, String)>,
    /// `ok_response.headers_to_remove`.
    pub headers_to_remove: Vec<String>,
    /// `denied_response.status.code` (0 when unset).
    pub denied_status: i32,
    /// `denied_response.body`.
    pub denied_body: String,
}

/// `HeaderValueOption { header = 1: HeaderValue { key = 1, value = 2, raw_value = 3 } }`.
fn header_option(bytes: &[u8]) -> Result<Option<(String, String)>, String> {
    for (field, value) in fields(bytes)? {
        if let (1, Value::Bytes(header)) = (field, value) {
            let (mut key, mut text_value, mut raw) = (String::new(), String::new(), None);
            for (f, v) in fields(header)? {
                match (f, v) {
                    (1, Value::Bytes(b)) => key = text(b),
                    (2, Value::Bytes(b)) => text_value = text(b),
                    (3, Value::Bytes(b)) => raw = Some(text(b)),
                    _ => {}
                }
            }
            return Ok(Some((key, raw.unwrap_or(text_value))));
        }
    }
    Ok(None)
}

/// Read a `CheckResponse`.
pub fn check_response(message: &[u8]) -> Result<Verdict, String> {
    let mut verdict = Verdict::default();
    for (field, value) in fields(message)? {
        match (field, value) {
            // google.rpc.Status { code = 1 }
            (1, Value::Bytes(status)) => {
                for (f, v) in fields(status)? {
                    if let (1, Value::Varint(code)) = (f, v) {
                        verdict.code = code as i32;
                    }
                }
            }
            // DeniedHttpResponse { status = 1: HttpStatus { code = 1 }, headers = 2, body = 3 }
            (2, Value::Bytes(denied)) => {
                for (f, v) in fields(denied)? {
                    match (f, v) {
                        (1, Value::Bytes(status)) => {
                            for (sf, sv) in fields(status)? {
                                if let (1, Value::Varint(code)) = (sf, sv) {
                                    verdict.denied_status = code as i32;
                                }
                            }
                        }
                        (2, Value::Bytes(option)) => verdict.headers.extend(header_option(option)?),
                        (3, Value::Bytes(body)) => verdict.denied_body = text(body),
                        _ => {}
                    }
                }
            }
            // OkHttpResponse { headers = 2, headers_to_remove = 5 }
            (3, Value::Bytes(ok)) => {
                for (f, v) in fields(ok)? {
                    match (f, v) {
                        (2, Value::Bytes(option)) => verdict.headers.extend(header_option(option)?),
                        (5, Value::Bytes(name)) => verdict.headers_to_remove.push(text(name)),
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    Ok(verdict)
}

/// Test and fuzz helpers: a `CheckResponse` encoded.
#[doc(hidden)]
pub fn encode_check_response(verdict: &Verdict) -> Vec<u8> {
    let header = |name: &str, value: &str| {
        let mut h = Vec::new();
        string_field(&mut h, 1, name);
        string_field(&mut h, 2, value);
        let mut option = Vec::new();
        bytes_field(&mut option, 1, &h);
        option
    };
    let mut status = Vec::new();
    if verdict.code != 0 {
        varint_field(&mut status, 1, verdict.code as u64);
    }
    let mut out = Vec::new();
    bytes_field(&mut out, 1, &status);
    if verdict.code == 0 {
        let mut ok = Vec::new();
        for (name, value) in &verdict.headers {
            bytes_field(&mut ok, 2, &header(name, value));
        }
        for name in &verdict.headers_to_remove {
            string_field(&mut ok, 5, name);
        }
        bytes_field(&mut out, 3, &ok);
    } else {
        let mut denied = Vec::new();
        let mut http_status = Vec::new();
        if verdict.denied_status != 0 {
            varint_field(&mut http_status, 1, verdict.denied_status as u64);
        }
        bytes_field(&mut denied, 1, &http_status);
        for (name, value) in &verdict.headers {
            bytes_field(&mut denied, 2, &header(name, value));
        }
        string_field(&mut denied, 3, &verdict.denied_body);
        bytes_field(&mut out, 2, &denied);
    }
    out
}

/// Test helper: the attributes a `CheckRequest` carries, read back.
#[doc(hidden)]
pub fn decode_check_request(message: &[u8]) -> Result<(String, String, String, Vec<(String, String)>), String> {
    let field = |bytes: &[u8], wanted: u64| -> Result<Vec<u8>, String> {
        fields(bytes)?.into_iter().find_map(|(f, v)| match v {
            Value::Bytes(b) if f == wanted => Some(b.to_vec()),
            _ => None,
        }).ok_or_else(|| format!("no field {wanted}"))
    };
    let context = field(message, 1)?;
    let request = field(&context, 4)?;
    let http = field(&request, 2)?;
    let (mut method, mut path, mut host, mut headers) = (String::new(), String::new(), String::new(), Vec::new());
    for (f, v) in fields(&http)? {
        match (f, v) {
            (2, Value::Bytes(b)) => method = text(b),
            (3, Value::Bytes(entry)) => {
                let (mut k, mut val) = (String::new(), String::new());
                for (ef, ev) in fields(entry)? {
                    match (ef, ev) {
                        (1, Value::Bytes(b)) => k = text(b),
                        (2, Value::Bytes(b)) => val = text(b),
                        _ => {}
                    }
                }
                headers.push((k, val));
            }
            (4, Value::Bytes(b)) => path = text(b),
            (5, Value::Bytes(b)) => host = text(b),
            _ => {}
        }
    }
    Ok((method, path, host, headers))
}
