//! The MCP tools and resources, one module per subject. Each module has its
//! types (serde + `JsonSchema`, which become the tools' schemas) and a
//! `register` that adds its tools to the server.

pub mod notes;
