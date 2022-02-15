pub struct Constants {
    pub(crate) NEW_LINE_SEPARATOR: &'static str,
    pub(crate) EMPTY_STRING: &'static str,
    pub(crate) WHITESPACE: &'static str,
    pub(crate) HEADER_NAME_VALUE_SEPARATOR: &'static str,
    pub(crate) SLASH: &'static str,
    pub(crate) CHARSET: &'static str,
    pub(crate) UTF_8: &'static str,
    pub(crate) NOSNIFF: &'static str,
    pub(crate) BYTES: &'static str,
    pub(crate) NONE: &'static str,
}

pub const CONSTANTS: Constants = Constants {
    NEW_LINE_SEPARATOR: "\r\n",
    EMPTY_STRING: "",
    WHITESPACE: " ",
    HEADER_NAME_VALUE_SEPARATOR: ": ",
    SLASH: "/",
    CHARSET: "charset",
    UTF_8: "UTF-8",
    NOSNIFF: "nosniff",
    BYTES: "bytes",
    NONE: "none",
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

pub struct StatusCodeReasonPhrase {
    pub(crate) STATUS_CODE: &'static str,
    pub(crate) REASON_PHRASE: &'static str,
}

pub struct ResponseStatusCodeReasonPhrase {
    pub(crate) N200_OK: &'static StatusCodeReasonPhrase,
    pub(crate) N206_PARTIAL_CONTENT: &'static StatusCodeReasonPhrase,
    pub(crate) N404_NOT_FOUND: &'static StatusCodeReasonPhrase,
    pub(crate) N416_RANGE_NOT_SATISFIABLE: &'static StatusCodeReasonPhrase,
}

pub const RESPONSE_STATUS_CODE_REASON_PHRASES: ResponseStatusCodeReasonPhrase = ResponseStatusCodeReasonPhrase {
    N200_OK: &StatusCodeReasonPhrase {
        STATUS_CODE: "200",
        REASON_PHRASE: "OK"
    },

    N206_PARTIAL_CONTENT: &StatusCodeReasonPhrase {
        STATUS_CODE: "206",
        REASON_PHRASE: "Partial Content"
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

pub struct HTTPHeader {
    pub(crate) CONTENT_TYPE: &'static str,
    pub(crate) CONTENT_LENGTH: &'static str,
    pub(crate) X_CONTENT_TYPE_OPTIONS: &'static str,
    pub(crate) RANGE: &'static str,
    pub(crate) ACCEPT_RANGES: &'static str,
    pub(crate) CONTENT_RANGE: &'static str,
}

pub const HTTP_HEADERS: HTTPHeader = HTTPHeader {
    CONTENT_TYPE: "Content-Type",
    X_CONTENT_TYPE_OPTIONS: "X-Content-Type-Options",
    CONTENT_LENGTH: "Content-Length",
    RANGE: "Range",
    ACCEPT_RANGES: "Accept-Ranges",
    CONTENT_RANGE: "Content-Range",
};