pub mod entry_point;
pub mod symbol;
pub mod header;
pub mod response;
pub mod server;
pub mod app;
pub mod thread_pool;
pub mod mime_type;
pub mod range;
pub mod cors;
pub mod request;
pub mod http;
pub mod ext;
pub mod client_hint;
pub mod language;
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
