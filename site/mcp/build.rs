// RWS_VERSION: the rust-web-server version this server is built against
// (the checkout's Cargo.toml), named in check_config's answers.
fn main() {
    let manifest = std::fs::read_to_string("../../Cargo.toml").expect("../../Cargo.toml");
    let version = manifest
        .lines()
        .find_map(|l| l.strip_prefix("version = \"").and_then(|v| v.strip_suffix('"')))
        .expect("version in ../../Cargo.toml");
    println!("cargo:rustc-env=RWS_VERSION={version}");
    println!("cargo:rerun-if-changed=../../Cargo.toml");
}
