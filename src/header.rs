pub struct Header {
    pub(crate) header_name: String,
    pub(crate) header_value: String,
}

impl Header {
    pub(crate) const CONTENT_TYPE: &'static str = "Content-Type";
    pub(crate) const CONTENT_LENGTH: &'static str = "Content-Length";
    pub(crate) const X_CONTENT_TYPE_OPTIONS: &'static str = "X-Content-Type-Options";
    pub(crate) const RANGE: &'static str = "Range";
    pub(crate) const ACCEPT_RANGES: &'static str = "Accept-Ranges";
    pub(crate) const CONTENT_RANGE: &'static str = "Content-Range";
    pub(crate) const HOST: &'static str = "Host";
    pub(crate) const VARY: &'static str = "Vary";
    pub(crate) const ORIGIN: &'static str = "Origin";

    pub(crate) const ACCESS_CONTROL_REQUEST_METHOD: &'static str = "Access-Control-Request-Method";
    pub(crate) const ACCESS_CONTROL_REQUEST_HEADERS: &'static str = "Access-Control-Request-Headers";
    pub(crate) const ACCESS_CONTROL_ALLOW_ORIGIN: &'static str = "Access-Control-Allow-Origin";
    pub(crate) const ACCESS_CONTROL_ALLOW_METHODS: &'static str = "Access-Control-Allow-Methods";
    pub(crate) const ACCESS_CONTROL_ALLOW_HEADERS: &'static str = "Access-Control-Allow-Headers";
    pub(crate) const ACCESS_CONTROL_ALLOW_CREDENTIALS: &'static str = "Access-Control-Allow-Credentials";
    pub(crate) const ACCESS_CONTROL_MAX_AGE: &'static str = "Access-Control-Max-Age";
    pub(crate) const ACCESS_CONTROL_EXPOSE_HEADERS: &'static str = "Access-Control-Expose-Headers";

}

