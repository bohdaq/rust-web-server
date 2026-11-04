//! The test client's Server-Sent Events parser (`test_client::SseParser`),
//! fed whole and in pieces. Must never panic, and both feeds must yield
//! the same events.
#![no_main]
use libfuzzer_sys::fuzz_target;
use rust_web_server::test_client::SseParser;

fuzz_target!(|data: &[u8]| {
    let whole = SseParser::new().feed(data);
    let mut parser = SseParser::new();
    let mut pieces = Ok(Vec::new());
    for piece in data.chunks(5) {
        match parser.feed(piece) {
            Ok(events) => pieces.as_mut().unwrap().extend(events),
            Err(e) => {
                pieces = Err(e);
                break;
            }
        }
    }
    if let (Ok(whole), Ok(pieces)) = (whole, pieces) {
        assert_eq!(whole, pieces);
    }
});
