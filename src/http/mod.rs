#[cfg(test)]
mod tests;

pub struct Version {
    pub(crate) http_0_9: &'static str,
    pub(crate) http_1_0: &'static str,
    pub(crate) http_1_1: &'static str,
    pub(crate) http_2_0: &'static str,
}

pub const VERSION: Version = Version {
    http_0_9: "HTTP/0.9",
    http_1_0: "HTTP/1.0",
    http_1_1: "HTTP/1.1",
    http_2_0: "HTTP/2.0",
};