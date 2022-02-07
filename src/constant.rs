pub struct Constants {

}

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
    pub(crate) N404_NOT_FOUND: &'static StatusCodeReasonPhrase,
}

pub const RESPONSE_STATUS_CODE_REASON_PHRASES: ResponseStatusCodeReasonPhrase = ResponseStatusCodeReasonPhrase {
    N200_OK: &StatusCodeReasonPhrase {
        STATUS_CODE: "200",
        REASON_PHRASE: "OK"
    },

    N404_NOT_FOUND: &StatusCodeReasonPhrase {
        STATUS_CODE: "404",
        REASON_PHRASE: "NOT FOUND"
    },
};