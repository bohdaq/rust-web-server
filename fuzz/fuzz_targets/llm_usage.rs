//! A provider's response as the LLM proxy reads it (#41): token usage from
//! a JSON body or an event stream, and chunked framing. Must never panic.
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    rust_web_server::fuzz_api::llm_usage(data);
});
