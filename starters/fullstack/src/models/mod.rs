//! The `#[derive(Model)]` types, one module per table. Register each with
//! the dev MCP endpoint in `dev_tools()` (src/lib.rs) so its `models` tool
//! lists it.

pub mod note;

pub use note::Note;
