#[cfg(test)]
mod tests;

pub struct Version {
    pub http_0_9: &'static str,
    pub http_1_0: &'static str,
    pub http_1_1: &'static str,
    pub http_2_0: &'static str,
    pub http_3_0: &'static str,
}

pub const VERSION: Version = Version {
    http_0_9: "HTTP/0.9",
    http_1_0: "HTTP/1.0",
    http_1_1: "HTTP/1.1",
    http_2_0: "HTTP/2.0",
    http_3_0: "HTTP/3.0",
};

pub struct HTTP;

impl HTTP {
    /// `true` if `version` (any case) is one of [`HTTP::version_list`],
    /// without building that list.
    pub fn is_supported_version(version: &str) -> bool {
        [VERSION.http_0_9, VERSION.http_1_0, VERSION.http_1_1, VERSION.http_2_0, VERSION.http_3_0]
            .iter()
            .any(|v| v.eq_ignore_ascii_case(version))
    }

    pub fn version_list() -> Vec<String> {
        let version_0_9 = VERSION.http_0_9.to_string();
        let version_1_0 = VERSION.http_1_0.to_string();
        let version_1_1 = VERSION.http_1_1.to_string();
        let version_2_0 = VERSION.http_2_0.to_string();
        let version_3_0 = VERSION.http_3_0.to_string();

        let list : Vec<String> = vec![
            version_0_9,
            version_1_0,
            version_1_1,
            version_2_0,
            version_3_0,
        ];
        list
    }
}