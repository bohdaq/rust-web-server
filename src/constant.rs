use crate::response::StatusCodeReasonPhrase;

pub struct Constants {
    pub(crate) new_line_separator: &'static str,
    pub(crate) new_line: &'static str,
    pub(crate) empty_string: &'static str,
    pub(crate) whitespace: &'static str,
    pub(crate) equals: &'static str,
    pub(crate) comma: &'static str,
    pub(crate) hyphen: &'static str,
    pub(crate) header_name_value_separator: &'static str,
    pub(crate) slash: &'static str,
    pub(crate) charset: &'static str,
    pub(crate) utf_8: &'static str,
    pub(crate) nosniff: &'static str,
    pub(crate) bytes: &'static str,
    pub(crate) none: &'static str,
    pub(crate) multipart: &'static str,
    pub(crate) byteranges: &'static str,
    pub(crate) semicolon: &'static str,
    pub(crate) boundary: &'static str,
    pub(crate) string_separator: &'static str,
    pub(crate) separator: &'static str,
    pub(crate) http_version_and_status_code_and_reason_phrase_regex: &'static str,
    pub(crate) content_range_regex: &'static str
}

pub const CONSTANTS: Constants = Constants {
    new_line: "\n",
    new_line_separator: "\r\n",
    empty_string: "",
    whitespace: " ",
    equals: "=",
    comma: ",",
    hyphen: "-",
    header_name_value_separator: ": ",
    slash: "/",
    charset: "charset",
    utf_8: "UTF-8",
    nosniff: "nosniff",
    bytes: "bytes",
    none: "none",
    multipart: "multipart",
    byteranges: "byteranges",
    semicolon: ";",
    boundary: "boundary",
    string_separator: "String_separator",
    separator: "--String_separator",
    http_version_and_status_code_and_reason_phrase_regex: "(?P<http_version>\\w+/\\w+.\\w)\\s(?P<status_code>\\w+)\\s(?P<reason_phrase>.+)",
    content_range_regex: "bytes\\s(?P<start>\\d{1,})-(?P<end>\\d{1,})/(?P<size>\\d{1,})"
};


pub struct HTTPVersion {
    pub(crate) http_version_0_9: &'static str,
    pub(crate) http_version_1_0: &'static str,
    pub(crate) http_version_1_1: &'static str,
    pub(crate) http_version_2_0: &'static str,
}

pub const HTTP_VERSIONS: HTTPVersion = HTTPVersion {
    http_version_0_9: "HTTP/0.9",
    http_version_1_0: "HTTP/1.0",
    http_version_1_1: "HTTP/1.1",
    http_version_2_0: "HTTP/2.0",
};


#[derive(Debug)]
pub struct HTTPError {
    pub(crate) status_code_reason_phrase: &'static StatusCodeReasonPhrase,
    pub(crate) message: String,
}