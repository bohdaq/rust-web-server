//! An rws WebAssembly plugin (ABI 1, see rws's `wasm_plugin` module docs).
//!
//! - every request gets `x-plugin: rws-example`;
//! - a request with `x-block: yes` is answered `403` by the plugin itself;
//! - every response gets `x-plugin-config: <the config string>`.
//!
//! ```sh
//! cargo build --release --target wasm32-unknown-unknown
//! # target/wasm32-unknown-unknown/release/rws_wasm_plugin_example.wasm
//! ```
//!
//! No dependencies and no imports: rws refuses a module that imports
//! anything.

use std::cell::RefCell;

thread_local! {
    /// Buffers handed out by `rws_alloc` and returned as output; they live
    /// until the instance ends (one request).
    static BUFFERS: RefCell<Vec<Vec<u8>>> = const { RefCell::new(Vec::new()) };
    static CONFIG: RefCell<String> = const { RefCell::new(String::new()) };
}

fn keep(bytes: Vec<u8>) -> (i32, i32) {
    let ptr = bytes.as_ptr() as i32;
    let len = bytes.len() as i32;
    BUFFERS.with(|b| b.borrow_mut().push(bytes));
    (ptr, len)
}

fn input<'a>(ptr: i32, len: i32) -> &'a str {
    // SAFETY: rws wrote `len` bytes at `ptr`, a buffer rws_alloc returned.
    let bytes = unsafe { std::slice::from_raw_parts(ptr as *const u8, len as usize) };
    std::str::from_utf8(bytes).unwrap_or("")
}

fn output(text: String) -> i64 {
    let (ptr, len) = keep(text.into_bytes());
    ((ptr as i64) << 32) | len as i64
}

#[no_mangle]
pub extern "C" fn rws_alloc(len: i32) -> i32 {
    keep(vec![0; len as usize]).0
}

#[no_mangle]
pub extern "C" fn rws_init(ptr: i32, len: i32) {
    let config = input(ptr, len).to_string();
    CONFIG.with(|c| *c.borrow_mut() = config);
}

#[no_mangle]
pub extern "C" fn rws_on_request(ptr: i32, len: i32) -> i64 {
    let request = input(ptr, len);
    // Line 1: method, target, version; line 2: client address; then headers.
    let blocked = request.lines().skip(2).take_while(|l| !l.is_empty()).any(|l| l.eq_ignore_ascii_case("x-block: yes"));
    if blocked {
        return output("respond 403\ncontent-type: text/plain\n\nblocked by the example plugin\n".to_string());
    }
    output("set x-plugin: rws-example\n".to_string())
}

#[no_mangle]
pub extern "C" fn rws_on_response(_ptr: i32, _len: i32) -> i64 {
    let config = CONFIG.with(|c| c.borrow().clone());
    if config.is_empty() {
        return 0;
    }
    output(format!("set x-plugin-config: {config}\n"))
}
