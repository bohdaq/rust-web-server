use std::fs::{File, metadata, read_dir};
use std::time::UNIX_EPOCH;
use file_ext::FileExt;
use crate::controller::Controller;
use crate::header::Header;
use crate::mime_type::MimeType;
use crate::range::{ContentRange, Range};
use crate::request::{METHOD, Request};
use crate::response::{Error, Response, STATUS_CODE_REASON_PHRASE};
use crate::server::ConnectionInfo;
use crate::symbol::SYMBOL;
use crate::url::URL;

#[cfg(test)]
mod tests;
pub(crate) mod open_file_cache;

pub struct StaticResourceController;

impl Controller for StaticResourceController {
    fn is_matching(request: &Request, _connection: &ConnectionInfo) -> bool {
        // Only origin-form targets name a file ("*" and anything a caller
        // built by hand don't); URL::parse below expects "http://localhost/...".
        if !request.request_uri.starts_with('/') {
            return false;
        }
        let url = request.request_uri.to_string();

        let boxed_url_components = URL::parse_request_target(&url);
        if boxed_url_components.is_err() {
            let message = boxed_url_components.as_ref().err().unwrap().to_string();
            // unfallable
            println!("unexpected error, {}", message);
        }

        let components = boxed_url_components.unwrap();

        let os_specific_separator : String = FileExt::get_path_separator();
        let os_specific_path = &components.path.replace(SYMBOL.slash, os_specific_separator.as_str());

        let boxed_static_filepath = static_fs_path(&os_specific_path);
        if boxed_static_filepath.is_err() {
            return false
        }

        let static_filepath = boxed_static_filepath.unwrap();

        // Any existing directory matches now — with an `index.html` it is served as
        // before, otherwise `process_static_resources` renders a directory listing.
        let mut is_directory = false;

        // A file the open-file cache holds needs no stat; otherwise one stat
        // answers both "exists?" and "directory?" — no open().
        let mut is_file = open_file_cache::is_cached_file(&static_filepath);
        if !is_file {
            if let Ok(md) = metadata(&static_filepath) {
                // Cached files were checked when opened; the rest here.
                if !inside_root(&static_filepath) {
                    return false;
                }
                is_directory = md.is_dir();
                is_file = md.is_file();
            }
        }

        let is_get = request.method == METHOD.get;
        let is_head = request.method == METHOD.head;
        let is_options = request.method == METHOD.options;

        let is_matching_method = (is_get || is_head || is_options) && (request.request_uri != SYMBOL.slash);

        if is_file || is_directory {
            is_matching_method
        } else {
            // check if file with same name and .html extension exists
            if !static_filepath.ends_with(".html") {
                let html_suffix = ".html";
                let html_file = [&components.path.replace(SYMBOL.slash, &FileExt::get_path_separator()), html_suffix].join(SYMBOL.empty_string);
                if let Ok(html_static_filepath) = static_fs_path(&html_file) {
                    if File::open(&html_static_filepath).is_ok() {
                        return is_matching_method;
                    }
                }
            }

            // Last resort: the configured SPA fallback file (RWS_CONFIG_SPA_FALLBACK),
            // for a GET/HEAD request that doesn't look like a missed static asset and
            // isn't under an excluded prefix. Returns false (unchanged prior behavior)
            // whenever the fallback isn't configured or doesn't apply.
            is_matching_method && spa_fallback_url_path(&components.path).is_some()
        }

    }

    fn process(request: &Request, mut response: Response, _connection: &ConnectionInfo) -> Response {
        // A Range with an If-Range that no longer matches is answered as if
        // there were no Range: the whole, current file (#56).
        let without_stale_range;
        let request = if range_is_stale(request) {
            let mut copy = request.clone();
            copy.headers.retain(|h| !h.name.eq_ignore_ascii_case(Header::_RANGE));
            without_stale_range = copy;
            &without_stale_range
        } else {
            request
        };
        if let Some(served) = StaticResourceController::serve_regular_file(request, &mut response) {
            return served;
        }
        let boxed_content_range_list = StaticResourceController::process_static_resources(&request);
        if boxed_content_range_list.is_ok() {
            let content_range_list = boxed_content_range_list.unwrap();

            if content_range_list.len() != 0 {

                let mut status_code_reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok;

                let does_request_include_range_header = request.get_header(Header::_RANGE.to_string()).is_some();
                if does_request_include_range_header {
                    status_code_reason_phrase = STATUS_CODE_REASON_PHRASE.n206_partial_content;
                }

                let is_options_request = request.method == METHOD.options;
                if is_options_request {
                    status_code_reason_phrase = STATUS_CODE_REASON_PHRASE.n204_no_content;
                }



                let url = request.request_uri.to_string();

                let boxed_url_components = URL::parse_request_target(&url);
                if boxed_url_components.is_err() {
                    let message = boxed_url_components.as_ref().err().unwrap().to_string();
                    // unfallable
                    println!("unexpected error, {}", message);
                }

                let components = boxed_url_components.unwrap();

                // Decoded and checked like the lookup itself (#55): the raw
                // path names no file when it has escapes.
                let static_filepath = static_fs_path(&components.path).unwrap_or_default();
                let boxed_modified_date_time = file_modified_utc(&static_filepath);

                if boxed_modified_date_time.is_ok() {
                    let modified_date_time = boxed_modified_date_time.unwrap();
                    let last_modified_unix_nanos = Header{ name: Header::_LAST_MODIFIED_UNIX_EPOCH_NANOS.to_string(), value: modified_date_time.to_string() };
                    response.headers.push(last_modified_unix_nanos);

                    let file_size = metadata(&static_filepath).map(|m| m.len()).unwrap_or(0);
                    let etag_value = format!("\"{}-{}\"", modified_date_time, file_size);

                    let if_none_match = request.get_header(Header::_IF_NONE_MATCH.to_string());
                    if let Some(inm) = if_none_match {
                        if if_none_match_matches(&inm.value, &etag_value) {
                            response.status_code = *STATUS_CODE_REASON_PHRASE.n304_not_modified.status_code;
                            response.reason_phrase = STATUS_CODE_REASON_PHRASE.n304_not_modified.reason_phrase.to_string();
                            response.headers.push(Header { name: Header::_ETAG.to_string(), value: etag_value });
                            return response;
                        }
                    }

                    response.headers.push(Header { name: Header::_ETAG.to_string(), value: etag_value });

                    // Stream large files (> 8 MB) without loading into memory, unless it's
                    // a range request (which needs precise byte slicing from the loaded body).
                    let is_range_request = request.get_header(Header::_RANGE.to_string()).is_some();
                    if file_size > StaticResourceController::STREAM_THRESHOLD && !is_range_request {
                        let mime = MimeType::detect_mime_type(&static_filepath);
                        response.headers.push(Header {
                            name: Header::_CONTENT_TYPE.to_string(),
                            value: mime,
                        });
                        response.headers.push(Header {
                            name: Header::_CONTENT_LENGTH.to_string(),
                            value: file_size.to_string(),
                        });
                        response.status_code = *status_code_reason_phrase.status_code;
                        response.reason_phrase = status_code_reason_phrase.reason_phrase.to_string();
                        response.stream_file = Some(static_filepath);
                        return response;
                    }
                }

                response.status_code = *status_code_reason_phrase.status_code;
                response.reason_phrase = status_code_reason_phrase.reason_phrase.to_string();
                response.content_range_list = content_range_list;

            }
        } else {
            let error : Error = boxed_content_range_list.err().unwrap();
            let body = error.message;

            let content_range = Range::get_content_range(
                Vec::from(body.as_bytes()),
                MimeType::TEXT_HTML.to_string()
            );

            let content_range_list = vec![content_range];

            response.status_code = *error.status_code_reason_phrase.status_code;
            response.reason_phrase = error.status_code_reason_phrase.reason_phrase.to_string();
            response.content_range_list = content_range_list;

            // A 416 says how long the file is (RFC 9110 §15.5.17, #56).
            if response.status_code == 416 {
                let length = URL::parse_request_target(&request.request_uri).ok()
                    .and_then(|c| static_fs_path(&c.path).ok())
                    .and_then(|p| metadata(p).ok())
                    .map(|md| md.len());
                if let Some(length) = length {
                    response.headers.push(Header { name: Header::_CONTENT_RANGE.to_string(), value: format!("bytes */{}", length) });
                }
            }
        }


        response
    }
}

//backward compatability
impl StaticResourceController {

    pub fn is_matching_request(request: &Request) -> bool {
        let boxed_static_filepath = static_fs_path(&request.request_uri);
        if boxed_static_filepath.is_err() {
            return false
        }

        let static_filepath = boxed_static_filepath.unwrap();

        let boxed_md = metadata(&static_filepath);
        if boxed_md.is_err() || !inside_root(&static_filepath) {
            return false
        }

        let md = boxed_md.unwrap();
        if md.is_dir() {
            return false
        }

        let boxed_file = File::open(&static_filepath);

        let is_get = request.method == METHOD.get;
        let is_head = request.method == METHOD.head;
        let is_options = request.method == METHOD.options;
        boxed_file.is_ok() && (is_get || is_head || is_options && request.request_uri != SYMBOL.slash)
    }

    pub fn process_request(request: &Request, mut response: Response) -> Response {
        if let Some(streamed) = StaticResourceController::serve_regular_file(request, &mut response) {
            return streamed;
        }
        let boxed_content_range_list = StaticResourceController::process_static_resources(&request);
        if boxed_content_range_list.is_ok() {
            let content_range_list = boxed_content_range_list.unwrap();

            if content_range_list.len() != 0 {

                let mut status_code_reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok;

                let does_request_include_range_header = request.get_header(Header::_RANGE.to_string()).is_some();
                if does_request_include_range_header {
                    status_code_reason_phrase = STATUS_CODE_REASON_PHRASE.n206_partial_content;
                }

                let is_options_request = request.method == METHOD.options;
                if is_options_request {
                    status_code_reason_phrase = STATUS_CODE_REASON_PHRASE.n204_no_content;
                }


                let static_filepath = static_fs_path(&request.request_uri).unwrap_or_default();
                let boxed_modified_date_time = file_modified_utc(&static_filepath);

                if boxed_modified_date_time.is_ok() {
                    let modified_date_time = boxed_modified_date_time.unwrap();
                    let last_modified_unix_nanos = Header{ name: Header::_LAST_MODIFIED_UNIX_EPOCH_NANOS.to_string(), value: modified_date_time.to_string() };
                    response.headers.push(last_modified_unix_nanos);

                    let file_size = metadata(&static_filepath).map(|m| m.len()).unwrap_or(0);
                    let etag_value = format!("\"{}-{}\"", modified_date_time, file_size);

                    let if_none_match = request.get_header(Header::_IF_NONE_MATCH.to_string());
                    if let Some(inm) = if_none_match {
                        if if_none_match_matches(&inm.value, &etag_value) {
                            response.status_code = *STATUS_CODE_REASON_PHRASE.n304_not_modified.status_code;
                            response.reason_phrase = STATUS_CODE_REASON_PHRASE.n304_not_modified.reason_phrase.to_string();
                            response.headers.push(Header { name: Header::_ETAG.to_string(), value: etag_value });
                            return response;
                        }
                    }

                    response.headers.push(Header { name: Header::_ETAG.to_string(), value: etag_value });

                    let is_range_request = request.get_header(Header::_RANGE.to_string()).is_some();
                    if file_size > StaticResourceController::STREAM_THRESHOLD && !is_range_request {
                        let mime = MimeType::detect_mime_type(&static_filepath);
                        response.headers.push(Header {
                            name: Header::_CONTENT_TYPE.to_string(),
                            value: mime,
                        });
                        response.headers.push(Header {
                            name: Header::_CONTENT_LENGTH.to_string(),
                            value: file_size.to_string(),
                        });
                        response.status_code = *status_code_reason_phrase.status_code;
                        response.reason_phrase = status_code_reason_phrase.reason_phrase.to_string();
                        response.stream_file = Some(static_filepath);
                        return response;
                    }
                }

                response.status_code = *status_code_reason_phrase.status_code;
                response.reason_phrase = status_code_reason_phrase.reason_phrase.to_string();
                response.content_range_list = content_range_list;

            }
        } else {
            let error : Error = boxed_content_range_list.err().unwrap();
            let body = error.message;

            let content_range = Range::get_content_range(
                Vec::from(body.as_bytes()),
                MimeType::TEXT_HTML.to_string()
            );

            let content_range_list = vec![content_range];

            response.status_code = *error.status_code_reason_phrase.status_code;
            response.reason_phrase = error.status_code_reason_phrase.reason_phrase.to_string();
            response.content_range_list = content_range_list;

        }


        response
    }

    /// Files larger than this are streamed from disk (`Response::stream_file`)
    /// rather than read into memory — 64 concurrent downloads of a 1 MiB file
    /// otherwise hold ~64 MiB plus copies.
    pub const STREAM_THRESHOLD: u64 = 256 * 1024;

    /// Fast path for a GET/HEAD of a regular file without a `Range` header:
    /// one `open` and one `fstat` — size and modified time come from the open
    /// handle, and a small file is read from that same handle — like nginx.
    /// (The general path below stats the file five or six times and opens it
    /// twice.) Files over [`Self::STREAM_THRESHOLD`] are answered with
    /// `stream_file` before anything is read. The `Content-Type` comes from
    /// the requested name. `None` = not applicable; the general path handles
    /// it (directories, `.html` resolution, ranges, OPTIONS, symlinks whose
    /// name has no known type).
    fn serve_regular_file(request: &Request, response: &mut Response) -> Option<Response> {
        if request.method != METHOD.get && request.method != METHOD.head {
            return None;
        }
        if request.get_header(Header::_RANGE.to_string()).is_some() {
            return None;
        }
        let path = URL::parse_request_target(&request.request_uri).ok()?.path;
        let fs_path = static_fs_path(&path.replace(SYMBOL.slash, &FileExt::get_path_separator())).ok()?;
        // A symlink whose own name has no recognizable type goes to the
        // general path, which takes the type from the link target: such names
        // are opened with O_NOFOLLOW, so a symlink simply fails to open.
        let mime = MimeType::detect_mime_type(&fs_path);
        let follow_symlinks = mime != MimeType::APPLICATION_OCTET_STREAM;
        let file = open_file_cache::open(&fs_path, follow_symlinks)?;
        let state = file.state()?;

        let mut response = std::mem::replace(response, Response::get_response(STATUS_CODE_REASON_PHRASE.n200_ok, None, None));
        let modified = state.modified;
        let etag_value = format!("\"{}-{}\"", modified, state.len);
        response.headers.push(Header { name: Header::_LAST_MODIFIED_UNIX_EPOCH_NANOS.to_string(), value: modified.to_string() });
        let not_modified = request
            .get_header(Header::_IF_NONE_MATCH.to_string())
            .map_or(false, |inm| if_none_match_matches(&inm.value, &etag_value));
        response.headers.push(Header { name: Header::_ETAG.to_string(), value: etag_value });
        if not_modified {
            response.status_code = *STATUS_CODE_REASON_PHRASE.n304_not_modified.status_code;
            response.reason_phrase = STATUS_CODE_REASON_PHRASE.n304_not_modified.reason_phrase.to_string();
            return Some(response);
        }
        response.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
        response.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();

        if state.len > StaticResourceController::STREAM_THRESHOLD {
            response.headers.push(Header { name: Header::_CONTENT_TYPE.to_string(), value: mime });
            response.headers.push(Header { name: Header::_CONTENT_LENGTH.to_string(), value: state.len.to_string() });
            response.stream_file = Some(fs_path);
            return Some(response);
        }

        let body = file.read_all(state.len).ok()?;
        response.content_range_list = vec![Range::get_content_range(body, mime)];
        Some(response)
    }

    pub fn process_static_resources(request: &Request) -> Result<Vec<ContentRange>, Error> {

        let url = request.request_uri.to_string();

        let boxed_url_components = URL::parse_request_target(&url);
        if boxed_url_components.is_err() {
            let message = boxed_url_components.as_ref().err().unwrap().to_string();
            // unfallable
            println!("unexpected error, {}", message);
        }

        let components = boxed_url_components.unwrap();

        let os_specific_separator : String = FileExt::get_path_separator();
        let os_specific_path = &components.path.replace(SYMBOL.slash, os_specific_separator.as_str());

        let static_filepath = match static_fs_path(&os_specific_path) {
            Ok(path) => path,
            Err(message) => {
                return Err(Error { status_code_reason_phrase: STATUS_CODE_REASON_PHRASE.n404_not_found, message });
            }
        };

        let mut content_range_list = Vec::new();


        let mut boxed_md = metadata(&static_filepath);
        if boxed_md.is_err() {
            let dot_html = format!("{}{}", &static_filepath, ".html");
            boxed_md = metadata(&dot_html);

            if boxed_md.is_err() {
                let slash_index_html = format!("{}{}{}", &static_filepath, os_specific_separator,  "index.html");
                boxed_md = metadata(&slash_index_html);
            }
        }
        if boxed_md.is_ok() {
            let md = boxed_md.unwrap();

            if md.is_dir() {
                let mut directory_index : String = "index.html".to_string();

                let last_char = components.path.chars().last().unwrap();
                if last_char != '/' {
                    let index : String = "index.html".to_string();
                    directory_index = format!("{}{}", os_specific_separator, index);
                }
                let index_html_in_directory = format!("{}{}", os_specific_path, directory_index);
                let index_html_fs_path = format!("{}{}", static_filepath, directory_index);

                if File::open(&index_html_fs_path).is_ok() {
                    let mut range_header = &Header {
                        name: Header::_RANGE.to_string(),
                        value: "bytes=0-".to_string()
                    };

                    let boxed_header = request.get_header(Header::_RANGE.to_string());
                    if boxed_header.is_some() {
                        range_header = boxed_header.unwrap();
                    }

                    let boxed_content_range_list = Range::get_content_range_list(&index_html_in_directory, range_header);
                    if boxed_content_range_list.is_ok() {
                        content_range_list = boxed_content_range_list.unwrap();
                    } else {
                        let error = boxed_content_range_list.err().unwrap();
                        return Err(error)
                    }
                } else {
                    let listing_html = StaticResourceController::render_directory_listing(&static_filepath, &components.path);
                    let content_range = Range::get_content_range(listing_html.into_bytes(), MimeType::TEXT_HTML.to_string());
                    content_range_list = vec![content_range];
                }

                return Ok(content_range_list);
            }

            let boxed_file = File::open(&static_filepath);
            if boxed_file.is_ok()  {
                let md = metadata(&static_filepath).unwrap();
                if md.is_dir() {
                    let mut range_header = &Header {
                        name: Header::_RANGE.to_string(),
                        value: "bytes=0-".to_string()
                    };

                    let boxed_header = request.get_header(Header::_RANGE.to_string());
                    if boxed_header.is_some() {
                        range_header = boxed_header.unwrap();
                    }

                    let mut directory_index : String = "index.html".to_string();

                    let last_char = components.path.chars().last().unwrap();
                    if last_char != '/' {
                        let index : String = "index.html".to_string();
                        directory_index = format!("{}{}", os_specific_separator, index);
                    }
                    let index_html_in_directory = format!("{}{}", os_specific_path, directory_index);


                    let boxed_content_range_list = Range::get_content_range_list(&index_html_in_directory, range_header);
                    if boxed_content_range_list.is_ok() {
                        content_range_list = boxed_content_range_list.unwrap();
                    } else {
                        let error = boxed_content_range_list.err().unwrap();
                        return Err(error)
                    }
                }

                if md.is_file() {
                    let mut range_header = &Header {
                        name: Header::_RANGE.to_string(),
                        value: "bytes=0-".to_string()
                    };

                    let boxed_header = request.get_header(Header::_RANGE.to_string());
                    if boxed_header.is_some() {
                        range_header = boxed_header.unwrap();
                    }

                    let boxed_content_range_list = Range::get_content_range_list(&request.request_uri, range_header);
                    if boxed_content_range_list.is_ok() {
                        content_range_list = boxed_content_range_list.unwrap();
                    } else {
                        let error = boxed_content_range_list.err().unwrap();
                        return Err(error)
                    }
                }
            }


            if boxed_file.is_err() {
                //check if .html file exists
                // Decoded like every lookup (#55): `/my%20page` is `my page.html`.
                let static_filepath = static_fs_path(&format!("{}.html", components.path)).unwrap_or_default();

                let boxed_file = File::open(&static_filepath);
                if boxed_file.is_ok()  {
                    let md = metadata(&static_filepath).unwrap();
                    if md.is_file() {
                        let mut range_header = &Header {
                            name: Header::_RANGE.to_string(),
                            value: "bytes=0-".to_string()
                        };

                        let boxed_header = request.get_header(Header::_RANGE.to_string());
                        if boxed_header.is_some() {
                            range_header = boxed_header.unwrap();
                        }

                        let url = request.request_uri.to_string();

                        let boxed_url_components = URL::parse_request_target(&url);
                        if boxed_url_components.is_err() {
                            let message = boxed_url_components.as_ref().err().unwrap().to_string();
                            // unfallable
                            println!("unexpected error, {}", message);
                        }

                        let components = boxed_url_components.unwrap();

                        // let html_file = [SYMBOL.slash, ].join(SYMBOL.empty_string);


                        let html_file = [components.path.as_str(), ".html"].join(SYMBOL.empty_string);
                        let boxed_content_range_list = Range::get_content_range_list(html_file.as_str(), range_header);
                        if boxed_content_range_list.is_ok() {
                            content_range_list = boxed_content_range_list.unwrap();
                        } else {
                            let error = boxed_content_range_list.err().unwrap();
                            return Err(error)
                        }
                    }
                }
            }
        }

        // Nothing on disk matched — try the configured SPA fallback (a no-op,
        // returning the untouched empty list, when it's unconfigured or doesn't
        // apply to this request).
        if content_range_list.is_empty() {
            if let Some(fallback_url_path) = spa_fallback_url_path(&components.path) {
                let mut range_header = &Header {
                    name: Header::_RANGE.to_string(),
                    value: "bytes=0-".to_string()
                };

                let boxed_header = request.get_header(Header::_RANGE.to_string());
                if boxed_header.is_some() {
                    range_header = boxed_header.unwrap();
                }

                if let Ok(list) = Range::get_content_range_list(&fallback_url_path, range_header) {
                    content_range_list = list;
                }
            }
        }

        Ok(content_range_list)
    }
}

/// Directory listing generation — used by [`StaticResourceController`] whenever a
/// requested directory has no `index.html`. Self-contained HTML (inline CSS/JS, no
/// external requests), dark/light adaptive via `prefers-color-scheme`.
impl StaticResourceController {
    /// Renders a directory listing page for `fs_dir_path` (absolute filesystem path)
    /// requested at `request_path` (the URL path, used to build links and breadcrumbs).
    /// Hidden entries (dotfiles) are omitted. Directories sort before files;
    /// each group is sorted case-insensitively by name.
    pub fn render_directory_listing(fs_dir_path: &str, request_path: &str) -> String {
        let normalized_request_path = if request_path.ends_with('/') {
            request_path.to_string()
        } else {
            format!("{}/", request_path)
        };

        struct Entry {
            name: String,
            is_dir: bool,
            size: u64,
            modified_epoch_secs: u64,
        }

        let mut entries: Vec<Entry> = Vec::new();
        if let Ok(dir_entries) = read_dir(fs_dir_path) {
            for dir_entry in dir_entries.flatten() {
                let name = dir_entry.file_name().to_string_lossy().to_string();
                if name.starts_with('.') {
                    continue;
                }

                if let Ok(md) = dir_entry.metadata() {
                    let modified_epoch_secs = md.modified()
                        .ok()
                        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                        .map(|d| d.as_secs())
                        .unwrap_or(0);

                    entries.push(Entry {
                        name,
                        is_dir: md.is_dir(),
                        size: md.len(),
                        modified_epoch_secs,
                    });
                }
            }
        }

        entries.sort_by(|a, b| {
            match (a.is_dir, b.is_dir) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            }
        });

        let segments: Vec<&str> = normalized_request_path.split('/').filter(|s| !s.is_empty()).collect();

        let mut breadcrumb_html = String::from("<a href=\"/\">~</a>");
        let mut accumulated_path = String::new();
        for segment in &segments {
            accumulated_path.push('/');
            accumulated_path.push_str(segment);
            breadcrumb_html.push_str(&format!(
                " <span class=\"sep\">/</span> <a href=\"{}/\">{}</a>",
                accumulated_path, html_escape(segment)
            ));
        }

        let parent_row = if !segments.is_empty() {
            let parent_path = if segments.len() > 1 {
                format!("/{}/", segments[..segments.len() - 1].join("/"))
            } else {
                "/".to_string()
            };
            format!(
                "<tr class=\"entry parent\"><td class=\"name\"><span class=\"icon\">\u{21A9}\u{FE0F}</span><a href=\"{}\">.. (parent directory)</a></td><td class=\"size\">\u{2014}</td><td class=\"modified\">\u{2014}</td></tr>\n",
                parent_path
            )
        } else {
            String::new()
        };

        let entry_count = entries.len();

        let mut rows = String::new();
        for entry in &entries {
            let escaped_name = html_escape(&entry.name);
            let encoded_name = URL::percent_encode(&entry.name);
            let href = if entry.is_dir {
                format!("{}{}/", normalized_request_path, encoded_name)
            } else {
                format!("{}{}", normalized_request_path, encoded_name)
            };
            let size_display = if entry.is_dir { "\u{2014}".to_string() } else { human_size(entry.size) };
            let modified_display = format_modified(entry.modified_epoch_secs);
            let icon = icon_for(&entry.name, entry.is_dir);
            let filter_key = html_escape(&entry.name.to_lowercase());

            rows.push_str(&format!(
                "<tr class=\"entry\" data-name=\"{}\"><td class=\"name\"><span class=\"icon\">{}</span><a href=\"{}\">{}{}</a></td><td class=\"size\">{}</td><td class=\"modified\">{}</td></tr>\n",
                filter_key, icon, href, escaped_name, if entry.is_dir { "/" } else { "" }, size_display, modified_display
            ));
        }

        let mut html = String::new();
        html.push_str(DIRECTORY_LISTING_HEAD);
        html.push_str(&format!("<div class=\"breadcrumb\">{}</div>\n", breadcrumb_html));
        html.push_str(&format!("<h1>Index of {}</h1>\n", html_escape(&normalized_request_path)));
        html.push_str(DIRECTORY_LISTING_TOOLBAR);
        html.push_str("<div class=\"card\"><table><thead><tr><th>Name</th><th>Size</th><th>Modified</th></tr></thead><tbody id=\"rows\">\n");
        html.push_str(&parent_row);
        html.push_str(&rows);
        html.push_str("</tbody></table></div>\n");
        html.push_str(&format!(
            "<footer>{} item{} &middot; served by rws</footer>\n",
            entry_count, if entry_count == 1 { "" } else { "s" }
        ));
        html.push_str(DIRECTORY_LISTING_TAIL);

        html
    }
}

/// `true` if the last `/`-separated segment of `path` has a file extension
/// (contains a `.` after the final `/`) — the standard heuristic (also used
/// by webpack-dev-server's `historyApiFallback`, `sirv`, `vite preview`)
/// distinguishing a client-side route (`/dashboard/settings`, no extension)
/// from a missed static asset (`/logo.png`, has one), so the SPA fallback
/// doesn't silently swallow the latter's real 404.
fn path_has_extension(path: &str) -> bool {
    path.rsplit('/').next().unwrap_or("").contains('.')
}

/// Resolves the SPA-fallback "URL path" to pass to [`Range::get_content_range_list`]
/// for a request at `url_path` — e.g. `"/index.html"` for the common
/// `RWS_CONFIG_SPA_FALLBACK=index.html` case — or `None` if the fallback doesn't
/// apply: unconfigured, `url_path` is under an excluded prefix
/// (`RWS_CONFIG_SPA_FALLBACK_EXCLUDE_PREFIXES`), `url_path` looks like a missed
/// static asset (see [`path_has_extension`]), or the configured file doesn't
/// actually exist on disk. Method filtering (GET/HEAD only) is the caller's
/// responsibility — both call sites already have it via `is_matching_method`.
fn spa_fallback_url_path(url_path: &str) -> Option<String> {
    if path_has_extension(url_path) {
        return None;
    }

    let fallback_name = crate::entry_point::get_spa_fallback()?;

    let exclude_prefixes = crate::entry_point::get_spa_fallback_exclude_prefixes();
    if exclude_prefixes.iter().any(|prefix| url_path.starts_with(prefix.as_str())) {
        return None;
    }

    let separator = FileExt::get_path_separator();
    let normalized_name = fallback_name.trim_start_matches('/').replace('/', &separator);
    let fallback_url_path = format!("{}{}", separator, normalized_name);

    let boxed_fs_path = static_fs_path(&fallback_url_path);
    let fs_path = boxed_fs_path.ok()?;
    if File::open(&fs_path).is_ok() {
        Some(fallback_url_path)
    } else {
        None
    }
}

fn html_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    if bytes < 1024 {
        return format!("{} {}", bytes, UNITS[0]);
    }

    let mut size = bytes as f64;
    let mut unit_index = 0;
    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }

    format!("{:.1} {}", size, UNITS[unit_index])
}

fn format_modified(epoch_secs: u64) -> String {
    let (sec, min, hour, day, month, _dow) = crate::scheduler::cron::epoch_to_datetime(epoch_secs);
    let (year, _, _) = crate::scheduler::cron::days_to_ymd(epoch_secs / 86400);
    format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}", year, month, day, hour, min, sec)
}

fn icon_for(name: &str, is_dir: bool) -> &'static str {
    if is_dir {
        return "\u{1F4C1}";
    }

    let ext = name.rsplit('.').next().unwrap_or("").to_lowercase();
    match ext.as_str() {
        "png" | "jpg" | "jpeg" | "gif" | "svg" | "webp" | "ico" | "bmp" => "\u{1F5BC}\u{FE0F}",
        "mp4" | "mov" | "avi" | "mkv" | "webm" => "\u{1F3A5}",
        "mp3" | "wav" | "flac" | "m4a" | "ogg" => "\u{1F3A7}",
        "zip" | "tar" | "gz" | "rar" | "7z" | "bz2" => "\u{1F4E6}",
        "pdf" => "\u{1F4D5}",
        "html" | "htm" | "css" | "js" | "ts" | "rs" | "py" | "json" | "toml" | "yaml" | "yml" | "sh" => "\u{1F9E9}",
        _ => "\u{1F4C4}",
    }
}

// CSS/JS are served as same-origin `<link>`/`<script src>` assets (see
// `crate::app::controller::directory_listing::DirectoryListingAssetsController`)
// rather than inlined here — inline `<style>`/`<script>` would be silently
// blocked under the framework's default `Content-Security-Policy: default-src 'self'`.
const DIRECTORY_LISTING_HEAD: &str = concat!(
    "<!DOCTYPE html>\n",
    "<html lang=\"en\">\n",
    "<head>\n",
    "<meta charset=\"UTF-8\">\n",
    "<meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">\n",
    "<title>Directory listing</title>\n",
    "<link rel=\"stylesheet\" href=\"/rws-directory-listing.css\">\n",
    "</head>\n",
    "<body>\n",
    "<div class=\"wrap\">\n",
);

const DIRECTORY_LISTING_TOOLBAR: &str = "<div class=\"toolbar\"><input type=\"search\" id=\"filter\" placeholder=\"Filter entries...\" autocomplete=\"off\"></div>\n";

const DIRECTORY_LISTING_TAIL: &str = concat!(
    "</div>\n",
    "<script src=\"/rws-directory-listing.js\" defer></script>\n",
    "</body>\n",
    "</html>\n",
);

/// The process working directory — the static-file root — read once.
/// `rws` never changes it after startup, and `std::env::current_dir()` is
/// several syscalls (on macOS it opens "."): the static-file path used to ask
/// for it four to six times per request. A library user who changes the
/// working directory after the first request keeps the original root.
pub(crate) fn working_directory() -> &'static str {
    static DIR: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    DIR.get_or_init(|| {
        std::env::current_dir()
            .map(|d| d.to_string_lossy().into_owned())
            .unwrap_or_default()
    })
}

/// `If-None-Match` against the current entity tag (RFC 9110 §13.1.2): a
/// comma-separated list, or `*`, compared weakly — `W/"x"` matches `"x"`
/// (#56). Before 17.113.2 only a header equal to the tag matched, so a
/// client sending several tags, or a weak one, never got a `304`.
pub(crate) fn if_none_match_matches(header_value: &str, etag: &str) -> bool {
    let opaque = |tag: &str| tag.trim().strip_prefix("W/").unwrap_or(tag.trim()).to_string();
    let current = opaque(etag);
    header_value.split(',').any(|candidate| {
        let candidate = candidate.trim();
        candidate == "*" || (!candidate.is_empty() && opaque(candidate) == current)
    })
}

/// `true` when the request has a `Range` and an `If-Range` that doesn't
/// match the file as it is now (RFC 9110 §13.1.5): an entity tag must match
/// the current `ETag` strongly (a weak tag never does); a date never matches,
/// since no `Last-Modified` is sent to compare it with. Then the `Range` is
/// ignored and the whole file is sent — the client's piece is out of date.
pub(crate) fn range_is_stale(request: &Request) -> bool {
    let Some(if_range) = request.get_header(Header::_IF_RANGE.to_string()) else { return false };
    if request.get_header(Header::_RANGE.to_string()).is_none() {
        return false;
    }
    let validator = if_range.value.trim();
    if !validator.starts_with('"') {
        return true;
    }
    let Ok(components) = URL::parse_request_target(&request.request_uri) else { return true };
    let Ok(fs_path) = static_fs_path(&components.path) else { return true };
    match (file_modified_utc(&fs_path), metadata(&fs_path)) {
        (Ok(modified), Ok(md)) => validator != format!("\"{}-{}\"", modified, md.len()),
        _ => true,
    }
}

/// `FileExt::get_static_filepath` without the per-call `current_dir()`:
/// the working directory followed by `path` — refused (`Err`) when `path`
/// could leave the web root or names a hidden file (#43):
///
/// - a `..` segment (`/../etc/passwd`): served before 17.113.1;
/// - a segment starting with `.` (`.git/`, `.env`), except `.well-known`,
///   unless `RWS_CONFIG_SERVE_DOTFILES=true`.
///
/// A symlink inside the root may still point outside it; [`inside_root`]
/// checks the resolved path where files are opened.
///
/// `path` is the request path as sent: it's percent-decoded here, once,
/// before those checks (#55) — `/my%20file.txt` is `my file.txt`, and
/// `/%2e%2e/` is refused like `/../`.
pub(crate) fn static_fs_path(path: &str) -> Result<String, String> {
    let path = percent_decode_path(path)?;
    check_request_path(&path)?;
    Ok(format!("{}{}", working_directory(), path))
}

/// Percent-decode a request path (RFC 3986 §2.1). `+` stays `+` (it means
/// a space only in form-encoded queries); a `%` not followed by two hex
/// digits is kept as is; a result that isn't UTF-8 is an error. Borrows
/// when there is nothing to decode.
pub(crate) fn percent_decode_path(path: &str) -> Result<std::borrow::Cow<'_, str>, String> {
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
pub(crate) fn check_request_path(path: &str) -> Result<(), String> {
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

/// `true` when `fs_path` exists and, symlinks resolved, lies inside the web
/// root (the working directory). A symlink in the root pointing outside it
/// (`assets -> /etc`) is not followed out (#43). One `realpath`: done where
/// a file is opened, not per request on the cached fast path.
pub(crate) fn inside_root(fs_path: &str) -> bool {
    static ROOT: std::sync::OnceLock<Option<std::path::PathBuf>> = std::sync::OnceLock::new();
    let Some(root) = ROOT.get_or_init(|| std::fs::canonicalize(working_directory()).ok()) else {
        return false;
    };
    std::fs::canonicalize(fs_path).is_ok_and(|resolved| resolved.starts_with(root))
}

/// `FileExt::file_modified_utc` (nanoseconds since the Unix epoch) from a
/// `stat`, without opening the file.
pub(crate) fn file_modified_utc(path: &str) -> Result<u128, String> {
    let modified = metadata(path)
        .and_then(|md| md.modified())
        .map_err(|e| format!("<p>Unable to open file: {}</p> <p>error: {}</p>", path, e))?;
    Ok(modified
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0))
}

