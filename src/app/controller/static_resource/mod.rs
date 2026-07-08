use std::env;
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

pub struct StaticResourceController;

impl Controller for StaticResourceController {
    fn is_matching(request: &Request, _connection: &ConnectionInfo) -> bool {
        let url_array = ["http://", "localhost", &request.request_uri];
        let url = url_array.join(SYMBOL.empty_string);

        let boxed_url_components = URL::parse(&url);
        if boxed_url_components.is_err() {
            let message = boxed_url_components.as_ref().err().unwrap().to_string();
            // unfallable
            println!("unexpected error, {}", message);
        }

        let components = boxed_url_components.unwrap();

        let os_specific_separator : String = FileExt::get_path_separator();
        let os_specific_path = &components.path.replace(SYMBOL.slash, os_specific_separator.as_str());

        let boxed_static_filepath = FileExt::get_static_filepath(&os_specific_path);
        if boxed_static_filepath.is_err() {
            return false
        }

        let static_filepath = boxed_static_filepath.unwrap();

        // Any existing directory matches now — with an `index.html` it is served as
        // before, otherwise `process_static_resources` renders a directory listing.
        let mut is_directory = false;

        let boxed_md = metadata(&static_filepath);
        if boxed_md.is_ok() {
            let md = boxed_md.unwrap();
            if md.is_dir() {
                is_directory = true;
            }
        }



        let boxed_file = File::open(&static_filepath);

        let is_get = request.method == METHOD.get;
        let is_head = request.method == METHOD.head;
        let is_options = request.method == METHOD.options;

        let is_matching_method = (is_get || is_head || is_options) && (request.request_uri != SYMBOL.slash);

        if boxed_file.is_ok() || is_directory {
            is_matching_method
        } else {
            // check if file with same name and .html extension exists
            if static_filepath.ends_with(".html") {
                return false
            }

            let html_suffix = ".html";
            let html_file = [&components.path.replace(SYMBOL.slash, &FileExt::get_path_separator()), html_suffix].join(SYMBOL.empty_string);
            let boxed_static_filepath = FileExt::get_static_filepath(&html_file);
            if boxed_static_filepath.is_err() {
                return false
            }

            let static_filepath = boxed_static_filepath.unwrap();
            let boxed_file = File::open(&static_filepath);

            boxed_file.is_ok() && is_matching_method
        }

    }

    fn process(request: &Request, mut response: Response, _connection: &ConnectionInfo) -> Response {
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


                let dir = env::current_dir().unwrap();
                let working_directory = dir.as_path().to_str().unwrap();

                let url_array = ["http://", "localhost", &request.request_uri];
                let url = url_array.join(SYMBOL.empty_string);

                let boxed_url_components = URL::parse(&url);
                if boxed_url_components.is_err() {
                    let message = boxed_url_components.as_ref().err().unwrap().to_string();
                    // unfallable
                    println!("unexpected error, {}", message);
                }

                let components = boxed_url_components.unwrap();

                let static_filepath = [working_directory, components.path.as_str()].join(SYMBOL.empty_string);
                let boxed_modified_date_time = FileExt::file_modified_utc(&static_filepath);

                if boxed_modified_date_time.is_ok() {
                    let modified_date_time = boxed_modified_date_time.unwrap();
                    let last_modified_unix_nanos = Header{ name: Header::_LAST_MODIFIED_UNIX_EPOCH_NANOS.to_string(), value: modified_date_time.to_string() };
                    response.headers.push(last_modified_unix_nanos);

                    let file_size = metadata(&static_filepath).map(|m| m.len()).unwrap_or(0);
                    let etag_value = format!("\"{}-{}\"", modified_date_time, file_size);

                    let if_none_match = request.get_header(Header::_IF_NONE_MATCH.to_string());
                    if let Some(inm) = if_none_match {
                        if inm.value == etag_value || inm.value == "*" {
                            response.status_code = *STATUS_CODE_REASON_PHRASE.n304_not_modified.status_code;
                            response.reason_phrase = STATUS_CODE_REASON_PHRASE.n304_not_modified.reason_phrase.to_string();
                            response.headers.push(Header { name: Header::_ETAG.to_string(), value: etag_value });
                            return response;
                        }
                    }

                    response.headers.push(Header { name: Header::_ETAG.to_string(), value: etag_value });

                    // Stream large files (> 8 MB) without loading into memory, unless it's
                    // a range request (which needs precise byte slicing from the loaded body).
                    const STREAM_THRESHOLD: u64 = 8 * 1024 * 1024;
                    let is_range_request = request.get_header(Header::_RANGE.to_string()).is_some();
                    if file_size > STREAM_THRESHOLD && !is_range_request {
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
}

//backward compatability
impl StaticResourceController {

    pub fn is_matching_request(request: &Request) -> bool {
        let boxed_static_filepath = FileExt::get_static_filepath(&request.request_uri);
        if boxed_static_filepath.is_err() {
            return false
        }

        let static_filepath = boxed_static_filepath.unwrap();

        let boxed_md = metadata(&static_filepath);
        if boxed_md.is_err() {
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


                let dir = env::current_dir().unwrap();
                let working_directory = dir.as_path().to_str().unwrap();
                let static_filepath = [working_directory, request.request_uri.as_str()].join(SYMBOL.empty_string);
                let boxed_modified_date_time = FileExt::file_modified_utc(&static_filepath);

                if boxed_modified_date_time.is_ok() {
                    let modified_date_time = boxed_modified_date_time.unwrap();
                    let last_modified_unix_nanos = Header{ name: Header::_LAST_MODIFIED_UNIX_EPOCH_NANOS.to_string(), value: modified_date_time.to_string() };
                    response.headers.push(last_modified_unix_nanos);

                    let file_size = metadata(&static_filepath).map(|m| m.len()).unwrap_or(0);
                    let etag_value = format!("\"{}-{}\"", modified_date_time, file_size);

                    let if_none_match = request.get_header(Header::_IF_NONE_MATCH.to_string());
                    if let Some(inm) = if_none_match {
                        if inm.value == etag_value || inm.value == "*" {
                            response.status_code = *STATUS_CODE_REASON_PHRASE.n304_not_modified.status_code;
                            response.reason_phrase = STATUS_CODE_REASON_PHRASE.n304_not_modified.reason_phrase.to_string();
                            response.headers.push(Header { name: Header::_ETAG.to_string(), value: etag_value });
                            return response;
                        }
                    }

                    response.headers.push(Header { name: Header::_ETAG.to_string(), value: etag_value });

                    const STREAM_THRESHOLD: u64 = 8 * 1024 * 1024;
                    let is_range_request = request.get_header(Header::_RANGE.to_string()).is_some();
                    if file_size > STREAM_THRESHOLD && !is_range_request {
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

    pub fn process_static_resources(request: &Request) -> Result<Vec<ContentRange>, Error> {
        let dir = env::current_dir().unwrap();
        let working_directory = dir.as_path().to_str().unwrap();

        let url_array = ["http://", "localhost", &request.request_uri];
        let url = url_array.join(SYMBOL.empty_string);

        let boxed_url_components = URL::parse(&url);
        if boxed_url_components.is_err() {
            let message = boxed_url_components.as_ref().err().unwrap().to_string();
            // unfallable
            println!("unexpected error, {}", message);
        }

        let components = boxed_url_components.unwrap();

        let os_specific_separator : String = FileExt::get_path_separator();
        let os_specific_path = &components.path.replace(SYMBOL.slash, os_specific_separator.as_str());

        let boxed_static_filepath = FileExt::get_static_filepath(&os_specific_path);

        let static_filepath = boxed_static_filepath.unwrap();

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
                let static_filepath = [working_directory, components.path.as_str(), ".html"].join(SYMBOL.empty_string);

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

                        let url_array = ["http://", "localhost", &request.request_uri];
                        let url = url_array.join(SYMBOL.empty_string);

                        let boxed_url_components = URL::parse(&url);
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