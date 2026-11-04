//! The routes, one module per resource. A module has its handlers
//! (`#[get]`, `#[post]`, … on `async fn`s; the attribute checks the
//! route's `{parameters}` against the `Path` argument at compile time) and
//! a `register` that adds them to the app.

pub mod health;
pub mod notes;
