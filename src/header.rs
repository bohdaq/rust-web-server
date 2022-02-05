pub struct Header {
    pub(crate) header_name: String,
    pub(crate) header_value: String,
}

impl std::fmt::Display for Header {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(fmt, "Header name {} and value {}", self.header_name, self.header_value)
    }
}