pub struct Constants {
    pub(crate) NEW_LINE_SEPARATOR: &'static str,
    pub(crate) NEW_LINE: &'static str,
    pub(crate) EMPTY_STRING: &'static str,
    pub(crate) WHITESPACE: &'static str,
    pub(crate) EQUALS: &'static str,
    pub(crate) COMMA: &'static str,
    pub(crate) HYPHEN: &'static str,
    pub(crate) HEADER_NAME_VALUE_SEPARATOR: &'static str,
    pub(crate) SLASH: &'static str,
    pub(crate) CHARSET: &'static str,
    pub(crate) UTF_8: &'static str,
    pub(crate) NOSNIFF: &'static str,
    pub(crate) BYTES: &'static str,
    pub(crate) NONE: &'static str,
    pub(crate) MULTIPART: &'static str,
    pub(crate) BYTERANGES: &'static str,
    pub(crate) SEMICOLON: &'static str,
    pub(crate) BOUNDARY: &'static str,
    pub(crate) STRING_SEPARATOR: &'static str,
    pub(crate) SEPARATOR: &'static str,
}

pub const CONSTANTS: Constants = Constants {
    NEW_LINE: "\n",
    NEW_LINE_SEPARATOR: "\r\n",
    EMPTY_STRING: "",
    WHITESPACE: " ",
    EQUALS: "=",
    COMMA: ",",
    HYPHEN: "-",
    HEADER_NAME_VALUE_SEPARATOR: ": ",
    SLASH: "/",
    CHARSET: "charset",
    UTF_8: "UTF-8",
    NOSNIFF: "nosniff",
    BYTES: "bytes",
    NONE: "none",
    MULTIPART: "multipart",
    BYTERANGES: "byteranges",
    SEMICOLON: ";",
    BOUNDARY: "boundary",
    STRING_SEPARATOR: "String_separator",
    SEPARATOR: "--String_separator"
};


pub struct RequestMethod {
    pub(crate) GET: &'static str,
    pub(crate) HEAD: &'static str,
    pub(crate) POST: &'static str,
    pub(crate) PUT: &'static str,
    pub(crate) DELETE: &'static str,
    pub(crate) CONNECT: &'static str,
    pub(crate) OPTIONS: &'static str,
    pub(crate) TRACE: &'static str,
}

pub const REQUEST_METHODS: RequestMethod = RequestMethod {
    GET: "GET",
    HEAD : "HEAD",
    POST : "POST",
    PUT : "PUT",
    DELETE : "DELETE",
    CONNECT : "CONNECT",
    OPTIONS : "OPTIONS",
    TRACE : "TRACE",
};

pub struct HTTPVersion {
    pub(crate) HTTP_VERSION_0_9: &'static str,
    pub(crate) HTTP_VERSION_1_0: &'static str,
    pub(crate) HTTP_VERSION_1_1: &'static str,
    pub(crate) HTTP_VERSION_2_0: &'static str,
}

pub const HTTP_VERSIONS: HTTPVersion = HTTPVersion {
    HTTP_VERSION_0_9: "HTTP/0.9",
    HTTP_VERSION_1_0 : "HTTP/1.0",
    HTTP_VERSION_1_1 : "HTTP/1.1",
    HTTP_VERSION_2_0 : "HTTP/2.0",
};

#[derive(Debug)]
pub struct StatusCodeReasonPhrase {
    pub(crate) STATUS_CODE: &'static str,
    pub(crate) REASON_PHRASE: &'static str,
}

pub struct ResponseStatusCodeReasonPhrase {
    pub(crate) N200_OK: &'static StatusCodeReasonPhrase,
    pub(crate) N204_NO_CONTENT: &'static StatusCodeReasonPhrase,
    pub(crate) N206_PARTIAL_CONTENT: &'static StatusCodeReasonPhrase,
    pub(crate) N400_BAD_REQUEST: &'static StatusCodeReasonPhrase,
    pub(crate) N404_NOT_FOUND: &'static StatusCodeReasonPhrase,
    pub(crate) N416_RANGE_NOT_SATISFIABLE: &'static StatusCodeReasonPhrase,
}

pub const RESPONSE_STATUS_CODE_REASON_PHRASES: ResponseStatusCodeReasonPhrase = ResponseStatusCodeReasonPhrase {
    N200_OK: &StatusCodeReasonPhrase {
        STATUS_CODE: "200",
        REASON_PHRASE: "OK"
    },

    N204_NO_CONTENT: &StatusCodeReasonPhrase {
        STATUS_CODE: "204",
        REASON_PHRASE: "No Content"
    },

    N206_PARTIAL_CONTENT: &StatusCodeReasonPhrase {
        STATUS_CODE: "206",
        REASON_PHRASE: "Partial Content"
    },

    N400_BAD_REQUEST: &StatusCodeReasonPhrase {
        STATUS_CODE: "400",
        REASON_PHRASE: "Bad Request"
    },

    N404_NOT_FOUND: &StatusCodeReasonPhrase {
        STATUS_CODE: "404",
        REASON_PHRASE: "Not Found"
    },

    N416_RANGE_NOT_SATISFIABLE: &StatusCodeReasonPhrase {
        STATUS_CODE: "416",
        REASON_PHRASE: "Range Not Satisfiable"
    },

};

#[derive(Debug)]
pub struct HTTPError {
    pub(crate) STATUS_CODE_REASON_PHRASE: &'static StatusCodeReasonPhrase,
    pub(crate) MESSAGE: String,
}