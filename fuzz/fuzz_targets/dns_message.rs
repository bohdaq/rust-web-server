//! A DNS message through the shared parser (`dns::parse`): what a DNS
//! server sends to SRV discovery and to ACME's DNS-01 (TXT lookups, SOA
//! lookups, UPDATE replies with TSIG). Must never panic.
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    rust_web_server::fuzz_api::dns_message(data);
});
