#[cfg(test)]
mod tests;

use std::io::prelude::*;
use std::fs::{metadata};
use std::io::{Cursor};
use file_ext::FileExt;

use crate::response::{Error, Response, STATUS_CODE_REASON_PHRASE};
use crate::header::Header;
use crate::mime_type::MimeType;
use crate::symbol::SYMBOL;

#[derive(PartialEq, Eq, Clone, Debug)]
pub struct Range {
    pub start: u64,
    pub end: u64,
}

#[derive(PartialEq, Eq, Clone, Debug)]
pub struct ContentRange {
    pub unit: String,
    pub range: Range,
    pub size: String,
    pub body: Vec<u8>,
    pub content_type: String,
}


impl Range {
    pub const STRING_SEPARATOR: &'static str = "String_separator";
    pub const BOUNDARY: &'static str = "boundary";
    pub const BYTERANGES: &'static str = "byteranges";
    pub const MULTIPART: &'static str = "multipart";
    pub const BYTES: &'static str = "bytes";


    pub const _ERROR_NO_EMPTY_LINE_BETWEEN_CONTENT_RANGE_HEADER_AND_BODY: &'static str = "no empty line between content range headers and body";
    pub const _ERROR_UNABLE_TO_PARSE_CONTENT_RANGE: &'static str = "unable to parse content-range";

    pub const ERROR_START_IS_AFTER_END_CONTENT_RANGE: &'static str = "start is after end in content range";
    pub const ERROR_START_IS_BIGGER_THAN_FILESIZE_CONTENT_RANGE: &'static str = "start is bigger than filesize in content range";
    pub const ERROR_END_IS_BIGGER_THAN_FILESIZE_CONTENT_RANGE: &'static str = "end is bigger than filesize in content range";
    pub const ERROR_MALFORMED_RANGE_HEADER_WRONG_UNIT: &'static str = "range header malformed, most likely you have an error in unit statement";

    pub const ERROR_UNABLE_TO_PARSE_RANGE_START: &'static str = "unable to parse range start";
    pub const ERROR_UNABLE_TO_PARSE_RANGE_END: &'static str = "unable to parse range end";


    pub fn parse_range_in_content_range(filelength: u64, range_str: &str) -> Result<Range, Error> {
        const START_INDEX: usize = 0;
        const END_INDEX: usize = 1;

        let mut range = Range { start: 0, end: filelength };
        let parts: Vec<&str> = range_str.split(SYMBOL.hyphen).collect();

        let mut start_range_not_provided = true;
        for (i, part) in parts.iter().enumerate() {

            let num = part.trim();
            let length = num.len();

            if i == START_INDEX && length != 0 {
                start_range_not_provided = false;
            }
            if i == START_INDEX && length != 0 {
                let boxed_start  = num.parse();
                if boxed_start.is_ok() {
                    range.start = boxed_start.unwrap()
                } else {
                    let message = Range::ERROR_UNABLE_TO_PARSE_RANGE_START.to_string();
                    let error = Error {
                        status_code_reason_phrase: STATUS_CODE_REASON_PHRASE.n416_range_not_satisfiable,
                        message: message.to_string()
                    };
                    return Err(error)
                }
            }
            if i == END_INDEX && length != 0 {
                let boxed_end  = num.parse();
                if boxed_end.is_ok() {
                    range.end = boxed_end.unwrap()
                } else {
                    let message = Range::ERROR_UNABLE_TO_PARSE_RANGE_END.to_string();
                    let error = Error {
                        status_code_reason_phrase: STATUS_CODE_REASON_PHRASE.n416_range_not_satisfiable,
                        message: message.to_string()
                    };
                    return Err(error)
                }
            }
            if i == END_INDEX && length != 0 && start_range_not_provided {
                let boxed_parse = num.parse();
                if boxed_parse.is_err() {
                    let error = Error {
                        status_code_reason_phrase: STATUS_CODE_REASON_PHRASE.n416_range_not_satisfiable,
                        message: Range::ERROR_UNABLE_TO_PARSE_RANGE_END.to_string()
                    };
                    return Err(error)
                }
                let num_usize : u64 = boxed_parse.unwrap();
                range.start = filelength - num_usize;
                range.end = filelength;
            }

            if range.end > filelength {
                let message = Range::ERROR_END_IS_BIGGER_THAN_FILESIZE_CONTENT_RANGE.to_string();
                let error = Error {
                    status_code_reason_phrase: STATUS_CODE_REASON_PHRASE.n416_range_not_satisfiable,
                    message,
                };
                return Err(error);
            }

            if range.start > filelength {
                let message = Range::ERROR_START_IS_BIGGER_THAN_FILESIZE_CONTENT_RANGE.to_string();
                let error = Error {
                    status_code_reason_phrase: STATUS_CODE_REASON_PHRASE.n416_range_not_satisfiable,
                    message,
                };
                return Err(error);
            }

            if range.start > range.end {
                let message = Range::ERROR_START_IS_AFTER_END_CONTENT_RANGE.to_string();
                let error = Error {
                    status_code_reason_phrase: STATUS_CODE_REASON_PHRASE.n416_range_not_satisfiable,
                    message,
                };
                return Err(error);
            }



        }
        Ok(range)
    }

    pub fn parse_content_range(filepath: &str, filelength: u64, raw_range_value: &str) -> Result<Vec<ContentRange>, Error> {
        const INDEX_AFTER_UNIT_DECLARATION : usize = 1;
        let mut content_range_list: Vec<ContentRange> = vec![];

        let prefix = [Range::BYTES, SYMBOL.equals].join("");
        if !raw_range_value.starts_with(prefix.as_str()) {
            let message = Range::ERROR_MALFORMED_RANGE_HEADER_WRONG_UNIT.to_string();
            let error = Error {
                status_code_reason_phrase: STATUS_CODE_REASON_PHRASE.n416_range_not_satisfiable,
                message,
            };
            return Err(error);
        }

        let split_raw_range_value: Vec<&str> = raw_range_value.split(SYMBOL.equals).collect();
        let boxed_raw_bytes = split_raw_range_value.get(INDEX_AFTER_UNIT_DECLARATION);
        if boxed_raw_bytes.is_none() {
            let message = Range::ERROR_UNABLE_TO_PARSE_RANGE_START.to_string();
            let error = Error {
                status_code_reason_phrase: STATUS_CODE_REASON_PHRASE.n416_range_not_satisfiable,
                message: message.to_string()
            };
            return Err(error)
        }

        let raw_bytes = boxed_raw_bytes.unwrap();

        let bytes: Vec<&str> = raw_bytes.split(SYMBOL.comma).collect();
        for byte in bytes {
            let boxed_range = Range::parse_range_in_content_range(filelength, byte);
            if boxed_range.is_ok() {
                let range = boxed_range.unwrap();
                let boxed_read = FileExt::read_file_partially(filepath, range.start, range.end);
                if boxed_read.is_ok() {

                    let content_type = MimeType::detect_mime_type(filepath);
                    let body = boxed_read.unwrap();
                    let content_range = ContentRange {
                        unit: Range::BYTES.to_string(),
                        range,
                        size: filelength.to_string(),
                        body,
                        content_type,
                    };
                    content_range_list.push(content_range);
                } else {
                    let error : Error = Error {
                        status_code_reason_phrase:  STATUS_CODE_REASON_PHRASE.n416_range_not_satisfiable,
                        message: boxed_read.err().unwrap().to_string()
                    };
                    return Err(error)
                }
            } else {
                let error : Error = boxed_range.err().unwrap();
                return Err(error);
            }
        }
        Ok(content_range_list)
    }

    pub fn get_content_range_list(request_uri: &str, range: &Header) -> Result<Vec<ContentRange>, Error> {
        let mut content_range_list : Vec<ContentRange> = vec![];
        let file_path_part = request_uri.replace(SYMBOL.slash, &FileExt::get_path_separator());

        let boxed_static_filepath = FileExt::get_static_filepath(&file_path_part);
        if boxed_static_filepath.is_err() {
            let error = Error {
                status_code_reason_phrase: STATUS_CODE_REASON_PHRASE.n500_internal_server_error,
                message: boxed_static_filepath.err().unwrap()
            };
            eprintln!("{}", &error.message);
            return Err(error);
        }
        let static_filepath = boxed_static_filepath.unwrap();

        let boxed_metadata = metadata(&static_filepath);
        if boxed_metadata.is_err() {
            let error = Error {
                status_code_reason_phrase: STATUS_CODE_REASON_PHRASE.n500_internal_server_error,
                message: boxed_metadata.err().unwrap().to_string()
            };
            eprintln!("{}", &error.message);
            return Err(error);
        }

        let md = boxed_metadata.unwrap();
        if md.is_file() {
            let mut path = static_filepath.as_str().to_string();

            let boxed_is_link = FileExt::is_symlink(&static_filepath);
            if boxed_is_link.is_err() {
                let error = Error {
                    status_code_reason_phrase: STATUS_CODE_REASON_PHRASE.n500_internal_server_error,
                    message: boxed_is_link.err().unwrap()
                };
                eprintln!("{}", &error.message);
                return Err(error);
            }


            let is_link = boxed_is_link.unwrap();
            if is_link {
                let boxed_points_to = FileExt::symlink_points_to(&static_filepath);
                if boxed_points_to.is_err() {
                    let error = Error {
                        status_code_reason_phrase: STATUS_CODE_REASON_PHRASE.n500_internal_server_error,
                        message: boxed_points_to.err().unwrap()
                    };
                    eprintln!("{}", &error.message);
                    return Err(error);
                }

                let points_to = boxed_points_to.unwrap();
                let reversed_link = &static_filepath.chars().rev().collect::<String>();

                let mut symlink_directory = SYMBOL.empty_string.to_string();
                let boxed_split = reversed_link.split_once(&FileExt::get_path_separator());
                if boxed_split.is_some() {
                    let (_filename, path) = boxed_split.unwrap();
                    symlink_directory = path.chars().rev().collect::<String>();
                }

                let resolved_link = FileExt::resolve_symlink_path(&symlink_directory, &points_to).unwrap();
                path = resolved_link;
            }

            let boxed_content_range_list = Range::parse_content_range(&path, md.len(), &range.value);
            if boxed_content_range_list.is_ok() {
                content_range_list = boxed_content_range_list.unwrap();
            } else {
                let error = boxed_content_range_list.err().unwrap();
                return Err(error)
            }
        }

        Ok(content_range_list)
    }

    pub fn _parse_multipart_body(cursor: &mut Cursor<&[u8]>, mut content_range_list: Vec<ContentRange>) -> Result<Vec<ContentRange>, String> {

        let mut buffer = Range::_parse_line_as_bytes(cursor);
        let new_line_char_found = buffer.len() != 0;
        let mut string = Range::_convert_bytes_array_to_string(buffer);

        if !new_line_char_found {
            return Ok(content_range_list)
        };

        let mut content_range: ContentRange = ContentRange {
            unit: Range::BYTES.to_string(),
            range: Range { start: 0, end: 0 },
            size: "".to_string(),
            body: vec![],
            content_type: "".to_string()
        };

        let content_range_is_not_parsed = content_range.body.len() == 0;
        let separator = [SYMBOL.hyphen, SYMBOL.hyphen, Range::STRING_SEPARATOR].join("");
        if string.starts_with(separator.as_str()) && content_range_is_not_parsed {
            //read next line - Content-Type
            buffer = Range::_parse_line_as_bytes(cursor);
            string = Range::_convert_bytes_array_to_string(buffer);
        }

        let content_type_is_not_parsed = content_range.content_type.len() == 0;
        if string.starts_with(Header::_CONTENT_TYPE) && content_type_is_not_parsed {
            let content_type = Response::_parse_http_response_header_string(string.as_str());
            content_range.content_type = content_type.value.trim().to_string();

            //read next line - Content-Range
            buffer = Range::_parse_line_as_bytes(cursor);
            string = Range::_convert_bytes_array_to_string(buffer);
        }

        let content_range_is_not_parsed = content_range.size.len() == 0;
        if string.starts_with(Header::_CONTENT_RANGE) && content_range_is_not_parsed {
            let content_range_header = Response::_parse_http_response_header_string(string.as_str());

            let boxed_result = Range::_parse_content_range_header_value(content_range_header.value);
            if boxed_result.is_ok() {
                let (start, end, size) = boxed_result.unwrap();

                content_range.size = size.to_string();
                content_range.range.start = start as u64;
                content_range.range.end = end as u64;
            } else {
                return Err(boxed_result.err().unwrap())
            }



            // read next line - empty line
            buffer = Range::_parse_line_as_bytes(cursor);
            string = Range::_convert_bytes_array_to_string(buffer);

            if string.trim().len() > 0 {
                return Err(Range::_ERROR_NO_EMPTY_LINE_BETWEEN_CONTENT_RANGE_HEADER_AND_BODY.to_string());
            }

            // read next line - separator between content ranges
            buffer = Range::_parse_line_as_bytes(cursor);
            string = Range::_convert_bytes_array_to_string(buffer);
        }

        let content_range_is_parsed = content_range.size.len() != 0;
        let content_type_is_parsed = content_range.content_type.len() != 0;
        if content_range_is_parsed && content_type_is_parsed {
            let mut body : Vec<u8> = vec![];
            body = [body, string.as_bytes().to_vec()].concat();

            let mut buf = Vec::from(string.as_bytes());
            let separator = [SYMBOL.hyphen, SYMBOL.hyphen, Range::STRING_SEPARATOR].join("");
            while !buf.starts_with(separator.as_bytes()) {
                buf = vec![];
                cursor.read_until(b'\n', &mut buf).unwrap();
                let separator = [SYMBOL.hyphen, SYMBOL.hyphen, Range::STRING_SEPARATOR].join("");
                if !buf.starts_with(separator.as_bytes()) {
                    body = [body, buf.to_vec()].concat();
                }
            }

            let mut mutable_body : Vec<u8>  = body;
            mutable_body.pop(); // remove /r
            mutable_body.pop(); // remove /n


            content_range.body = mutable_body;

            content_range_list.push(content_range);
        }

        let boxed_result = Range::_parse_multipart_body(cursor, content_range_list);
        return if boxed_result.is_ok() {
            Ok(boxed_result.unwrap())
        } else {
            let error = boxed_result.err().unwrap();
            Err(error)
        }

    }

    pub fn _parse_raw_content_range_header_value(unparsed_header_value: &str)-> Result<(i64, i64, i64), String> {
        let lowercase_unparsed_header_value = unparsed_header_value.trim().to_lowercase();

        let start : i64;
        let end : i64;
        let size : i64;


        let boxed_split_without_bytes = lowercase_unparsed_header_value.split_once(SYMBOL.whitespace);
        if boxed_split_without_bytes.is_none() {
            return Err(Range::_ERROR_UNABLE_TO_PARSE_CONTENT_RANGE.to_string())
        }

        let (bytes, without_bytes) = boxed_split_without_bytes.unwrap();
        if !bytes.eq("bytes") {
            return Err(Range::_ERROR_UNABLE_TO_PARSE_CONTENT_RANGE.to_string())
        }

        let boxed_without_bytes = without_bytes.split_once(SYMBOL.hyphen);
        if boxed_without_bytes.is_none() {
            return Err(Range::_ERROR_UNABLE_TO_PARSE_CONTENT_RANGE.to_string())
        }

        let (_start, _without_start) = boxed_without_bytes.unwrap();

        let boxed_start = _start.parse::<i64>();
        if boxed_start.is_err() {
            return Err(Range::_ERROR_UNABLE_TO_PARSE_CONTENT_RANGE.to_string())
        }

        start = boxed_start.unwrap();



        let boxed_without_start = _without_start.split_once(SYMBOL.slash);
        if boxed_without_start.is_none() {
            return Err(Range::_ERROR_UNABLE_TO_PARSE_CONTENT_RANGE.to_string())
        }
        let (_end, _size) = boxed_without_start.unwrap();

        let boxed_end = _end.parse::<i64>();
        if boxed_end.is_err() {
            return Err(Range::_ERROR_UNABLE_TO_PARSE_CONTENT_RANGE.to_string())
        }

        end = boxed_end.unwrap();

        let boxed_size = _size.parse::<i64>();
        if boxed_size.is_err() {
            return Err(Range::_ERROR_UNABLE_TO_PARSE_CONTENT_RANGE.to_string())
        }

        size = boxed_size.unwrap();

        Ok((start, end, size))
    }

    pub fn _parse_content_range_header_value(header_value: String) -> Result<(i64, i64, i64), String> {
        let boxed_parse_result = Range::_parse_raw_content_range_header_value(&header_value);
        if boxed_parse_result.is_err() {
            return Err(boxed_parse_result.err().unwrap())
        }
        let (start, end, size) = boxed_parse_result.unwrap();

        if start > end {
            return Err(Range::ERROR_START_IS_AFTER_END_CONTENT_RANGE.to_string())
        }

        if start > size {
            return Err(Range::ERROR_START_IS_BIGGER_THAN_FILESIZE_CONTENT_RANGE.to_string());
        }
        if end > size {
            return  Err(Range::ERROR_END_IS_BIGGER_THAN_FILESIZE_CONTENT_RANGE.to_string());
        }

        Ok((start, end, size))
    }

    pub fn _parse_line_as_bytes(cursor: &mut Cursor<&[u8]>) -> Vec<u8> {
        let mut buffer = vec![];
        cursor.read_until(b'\n', &mut buffer).unwrap();
        buffer
    }

    pub fn _convert_bytes_array_to_string(buffer: Vec<u8>) -> String {
        let buffer_as_u8_array: &[u8] = &buffer;
        String::from_utf8(Vec::from(buffer_as_u8_array)).unwrap()
    }

    pub fn get_content_range(body: Vec<u8>, mime_type: String) -> ContentRange {
        let length = body.len() as u64;
        let content_range = ContentRange {
            unit: Range::BYTES.to_string(),
            range: Range { start: 0, end: length },
            size: length.to_string(),
            body,
            content_type: mime_type
        };

        content_range
    }

    pub fn get_content_range_of_a_file(filepath: &str) -> Result<ContentRange, String> {
        let body: Vec<u8>;
        let boxed_file = FileExt::read_file(filepath);
        if boxed_file.is_err() {
            let error = boxed_file.err().unwrap();
            return Err(error);
        }

        body = boxed_file.unwrap();
        let mime_type = MimeType::detect_mime_type(filepath);
        let content_range = Range::get_content_range(body, mime_type);
        Ok(content_range)
    }
}


