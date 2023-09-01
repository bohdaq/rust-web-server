use std::env;
use std::fs::{File, metadata};
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
        if request.method != METHOD.get {
            return false;
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

        let os_specific_separator : String = FileExt::get_path_separator();
        let os_specific_path = &components.path.replace(SYMBOL.slash, os_specific_separator.as_str());

        let boxed_static_filepath = FileExt::get_static_filepath(&os_specific_path);
        if boxed_static_filepath.is_err() {
            return false
        }

        let static_filepath = boxed_static_filepath.unwrap();

        let mut is_directory_with_index_html = false;

        let boxed_md = metadata(&static_filepath);
        if boxed_md.is_ok() {
            let md = boxed_md.unwrap();
            if md.is_dir() {
                let mut directory_index : String = "index.html".to_string();

                let last_char = components.path.chars().last().unwrap();
                if last_char != '/' {
                    let index : String = "index.html".to_string();
                    directory_index = format!("{}{}", os_specific_separator, index);

                }
                let index_html_in_directory = format!("{}{}", static_filepath, directory_index);


                let boxed_file = File::open(&index_html_in_directory);
                if boxed_file.is_err() {
                    return false
                }

                is_directory_with_index_html = true;
            }
        }



        let boxed_file = File::open(&static_filepath);

        let is_get = request.method == METHOD.get;
        let is_head = request.method == METHOD.head;
        let is_options = request.method == METHOD.options;

        let is_matching_method = (is_get || is_head || is_options) && (request.request_uri != SYMBOL.slash);

        if boxed_file.is_ok() || is_directory_with_index_html {
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