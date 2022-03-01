mod range_test;
mod response_test;
mod request_test;
mod mime_type_test;
mod server_test;

use std::{env, fs};
use regex::Regex;


#[cfg(test)]
mod tests {
    use std::borrow::Borrow;
    use std::fs::{File, metadata};
    use std::io::{BufReader, Read, Seek, SeekFrom};
    use crate::CONSTANTS;
    use crate::constant::{HTTP_HEADERS, HTTP_VERSIONS, REQUEST_METHODS, RESPONSE_STATUS_CODE_REASON_PHRASES};
    use crate::header::Header;
    use crate::mime_type::MimeType;
    use crate::range::{ContentRange, Range};
    use crate::request::Request;
    use crate::response::Response;
    use crate::server::Server;
    use super::*;
}
