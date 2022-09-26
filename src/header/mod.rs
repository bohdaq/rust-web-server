#[cfg(test)]
mod tests;

#[derive(PartialEq, Eq, Clone, Debug)]
pub struct Header {
    pub name: String,
    pub value: String,
}

impl Header {
    pub const CONTENT_TYPE: &'static str = "Content-Type";
    pub const CONTENT_LENGTH: &'static str = "Content-Length";
    pub const X_CONTENT_TYPE_OPTIONS: &'static str = "X-Content-Type-Options";
    pub const X_CONTENT_TYPE_OPTIONS_VALUE_NOSNIFF: &'static str = "nosniff";
    pub const RANGE: &'static str = "Range";
    pub const ACCEPT_RANGES: &'static str = "Accept-Ranges";
    pub const CONTENT_RANGE: &'static str = "Content-Range";
    pub const _HOST: &'static str = "Host";
    pub const VARY: &'static str = "Vary";
    pub const ORIGIN: &'static str = "Origin";

    pub const ACCESS_CONTROL_REQUEST_METHOD: &'static str = "Access-Control-Request-Method";
    pub const ACCESS_CONTROL_REQUEST_HEADERS: &'static str = "Access-Control-Request-Headers";
    pub const ACCESS_CONTROL_ALLOW_ORIGIN: &'static str = "Access-Control-Allow-Origin";
    pub const ACCESS_CONTROL_ALLOW_METHODS: &'static str = "Access-Control-Allow-Methods";
    pub const ACCESS_CONTROL_ALLOW_HEADERS: &'static str = "Access-Control-Allow-Headers";
    pub const ACCESS_CONTROL_ALLOW_CREDENTIALS: &'static str = "Access-Control-Allow-Credentials";
    pub const ACCESS_CONTROL_MAX_AGE: &'static str = "Access-Control-Max-Age";
    pub const ACCESS_CONTROL_EXPOSE_HEADERS: &'static str = "Access-Control-Expose-Headers";


    pub const NAME_VALUE_SEPARATOR: &'static str = ": ";

}

