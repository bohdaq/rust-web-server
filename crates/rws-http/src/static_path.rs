//! Request paths to filesystem paths under the working directory: percent
//! decoding, refusing `..` and hidden segments, the OS separator. Used by the
//! static file controller and by `Range` (which reads the file it ranges);
//! it sits low so both layers can.

#[doc(hidden)]
pub fn working_directory() -> &'static str {
    static DIR: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    DIR.get_or_init(|| {
        std::env::current_dir()
            .map(|d| d.to_string_lossy().into_owned())
            .unwrap_or_default()
    })
}

/// `FileExt::get_static_filepath` without the per-call `current_dir()`:
/// the working directory followed by `path` — refused (`Err`) when `path`
/// could leave the web root or names a hidden file (#43):
///
/// - a `..` segment (`/../etc/passwd`): served before 17.113.1;
/// - a segment starting with `.` (`.git/`, `.env`), except `.well-known`,
///   unless `RWS_CONFIG_SERVE_DOTFILES=true`.
///
/// A symlink inside the root may still point outside it; `inside_root` (static_resource)
/// checks the resolved path where files are opened.
///
/// `path` is the request path as sent: it's percent-decoded here, once,
/// before those checks (#55) — `/my%20file.txt` is `my file.txt`, and
/// `/%2e%2e/` is refused like `/../`.
#[doc(hidden)]
pub fn static_fs_path(path: &str) -> Result<String, String> {
    let path = percent_decode_path(path)?;
    check_request_path(&path)?;
    let working_directory = working_directory();
    let mut fs_path = String::with_capacity(working_directory.len() + path.len());
    fs_path.push_str(working_directory);
    fs_path.push_str(&path);
    Ok(fs_path)
}

/// A URL path with `/` turned into the OS path separator — borrowed (no
/// copy) where the separator is `/`.
#[doc(hidden)]
pub fn os_specific_path(path: &str) -> std::borrow::Cow<'_, str> {
    if std::path::MAIN_SEPARATOR == '/' {
        std::borrow::Cow::Borrowed(path)
    } else {
        std::borrow::Cow::Owned(path.replace('/', std::path::MAIN_SEPARATOR_STR))
    }
}

/// Percent-decode a request path (RFC 3986 §2.1). `+` stays `+` (it means
/// a space only in form-encoded queries); a `%` not followed by two hex
/// digits is kept as is; a result that isn't UTF-8 is an error. Borrows
/// when there is nothing to decode.
#[doc(hidden)]
pub fn percent_decode_path(path: &str) -> Result<std::borrow::Cow<'_, str>, String> {
    if !path.contains('%') {
        return Ok(std::borrow::Cow::Borrowed(path));
    }
    let hex = |b: u8| (b as char).to_digit(16).map(|d| d as u8);
    let bytes = path.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(high), Some(low)) = (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                decoded.push(high << 4 | low);
                i += 3;
                continue;
            }
        }
        decoded.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(decoded)
        .map(std::borrow::Cow::Owned)
        .map_err(|_| "path is not UTF-8 after percent-decoding".to_string())
}

/// The segment rules of [`static_fs_path`], on a path using `/` or the
/// platform separator.
#[doc(hidden)]
pub fn check_request_path(path: &str) -> Result<(), String> {
    for segment in path.split(['/', '\\']) {
        if segment == ".." {
            return Err("path leaves the web root".to_string());
        }
        if segment.starts_with('.') && segment != "." && segment != ".well-known" && !serve_dotfiles() {
            return Err("hidden files are not served".to_string());
        }
        if segment.contains('\0') {
            return Err("invalid path".to_string());
        }
    }
    // Keys and the config file, wherever they sit in the root (#45).
    let name = path.rsplit(['/', '\\']).next().unwrap_or("").to_ascii_lowercase();
    if name.ends_with(".pem") || name.ends_with(".key") || name == "rws.config.toml" {
        return Err("key and config files are not served".to_string());
    }
    Ok(())
}

/// `RWS_CONFIG_SERVE_DOTFILES` (default `false`), read once.
fn serve_dotfiles() -> bool {
    static SERVE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *SERVE.get_or_init(|| {
        std::env::var("RWS_CONFIG_SERVE_DOTFILES").is_ok_and(|v| v.trim().eq_ignore_ascii_case("true"))
    })
}
