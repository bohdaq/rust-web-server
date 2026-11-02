/// Configuration for a single virtual host (one domain → one cert/key pair).
#[derive(Clone, Debug)]
pub struct VirtualHostConfig {
    pub domain: String,
    pub cert_file: String,
    pub key_file: String,
}
