//! The routes, one module per resource. A module has its types, its
//! handlers (`#[get]`, `#[post]`, … — the attribute checks the route's
//! `{parameters}` against the `Path` argument at compile time), a
//! `register` that adds them to the app, and a `describe` for OpenAPI.

pub mod todos;
