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
mod client_hint;

extern crate core;

use crate::entry_point::start;


fn main() {
 const VERSION: &str = env!("CARGO_PKG_VERSION");
 const AUTHORS: &str = env!("CARGO_PKG_AUTHORS");
 const REPOSITORY: &str = env!("CARGO_PKG_REPOSITORY");
 const DESCRIPTION: &str = env!("CARGO_PKG_DESCRIPTION");

 println!("Rust Web Server");
 println!("Version:    {}", VERSION);
 println!("Authors:    {}", AUTHORS);
 println!("Repository: {}", REPOSITORY);
 println!("Desciption: {}\n\n", DESCRIPTION);
 println!("RWS Configuration Start: \n");

 start();
}
