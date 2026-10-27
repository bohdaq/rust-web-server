//! The PROXY protocol v1/v2 header parser (`server::proxy_protocol::parse`).
//! Must never panic, never claim more bytes than it was given, and never ask
//! for fewer than it has or more than the protocol's limits.
#![no_main]
use libfuzzer_sys::fuzz_target;
use rust_web_server::server::proxy_protocol::{parse, Parsed, V1_MAX, V2_MAX};

fuzz_target!(|data: &[u8]| {
    match parse(data) {
        Parsed::Header { len, .. } => assert!(len <= data.len() && len >= 15.min(data.len())),
        Parsed::Need(n) => assert!(n > data.len() && n <= V1_MAX.max(V2_MAX)),
        Parsed::Invalid(_) => {}
    }
});
