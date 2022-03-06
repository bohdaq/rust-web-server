pub struct Header {
    pub(crate) header_name: String,
    pub(crate) header_value: String,
}

impl Header {
    pub(crate) const CONTENT_RANGE_REGEX: &'static str = "bytes\\s(?P<start>\\d{1,})-(?P<end>\\d{1,})/(?P<size>\\d{1,})";

    pub(crate) const CONTENT_TYPE: &'static str = "Content-Type";
    pub(crate) const CONTENT_LENGTH: &'static str = "Content-Length";
    pub(crate) const X_CONTENT_TYPE_OPTIONS: &'static str = "X-Content-Type-Options";
    pub(crate) const RANGE: &'static str = "Range";
    pub(crate) const ACCEPT_RANGES: &'static str = "Accept-Ranges";
    pub(crate) const CONTENT_RANGE: &'static str = "Content-Range";
    pub(crate) const HOST: &'static str = "Host";
    pub(crate) const ORIGIN: &'static str = "Origin";
    pub(crate) const ACCESS_CONTROL_REQUEST_METHOD: &'static str = "Access-Control-Request-Method";
    pub(crate) const ACCESS_CONTROL_REQUEST_HEADERS: &'static str = "Access-Control-Request-Headers";
}

