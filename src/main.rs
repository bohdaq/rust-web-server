mod entry_point;
mod symbol;
mod header;
mod response;
mod server;
mod app;
mod thread_pool;
mod mime_type;
mod range;
mod cors;
mod request;
mod http;
mod ext;

extern crate core;

use crate::entry_point::start;


fn main() {
 start();
}
