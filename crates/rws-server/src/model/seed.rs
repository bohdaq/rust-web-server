//! Seeds written in Rust (18.30.0), run after the `seeds/*.sql` files by
//! [`DbPool::seed_with`] — and by an app's own `db seed` command, built on
//! `rust_web_server::db_cli::run_with_seeders`.
//!
//! A seeder is a name and an async function of the pool; factories
//! (`#[derive(Factory)]`) make the rows:
//!
//! ```rust,ignore
//! use rust_web_server::model::{DbPool, Seeders};
//!
//! pub fn seeders() -> Seeders {
//!     Seeders::new()
//!         .add("users", |pool: DbPool| async move {
//!             User::factory().create_many(10, &pool).await?;
//!             Ok(())
//!         })
//!         .add("admin", |pool: DbPool| async move {
//!             User::factory().with(|u| u.admin = true).create(&pool).await?;
//!             Ok(())
//!         })
//! }
//!
//! pool.seed_with("seeds", &seeders()).await?; // seeds/*.sql, then users, admin
//! ```
//!
//! Like SQL seeds, a seeder runs every time it is asked to — nothing
//! records that it ran — so write it to tolerate the rows being there
//! (look them up first, or use keys that a second insert can't repeat).
//! It isn't wrapped in a transaction: use [`DbPool::transaction`] in it
//! when its rows must go in together.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use super::pool::DbPool;
use super::DbError;

/// The future a [`Seeder`] answers.
pub type SeedFuture<'a> = Pin<Box<dyn Future<Output = Result<(), DbError>> + Send + 'a>>;

/// Rows put in a database by Rust code; see the [module docs](self).
/// [`Seeders::add`] makes one from a closure.
pub trait Seeder: Send + Sync {
    /// What `seed_with` and `db seed` report it as.
    fn name(&self) -> &str;

    /// Put the rows in `pool` (the primary of a replicated pool).
    fn run<'a>(&'a self, pool: &'a DbPool) -> SeedFuture<'a>;
}

struct FnSeeder<F> {
    name: String,
    run: F,
}

impl<F, Fut> Seeder for FnSeeder<F>
where
    F: Fn(DbPool) -> Fut + Send + Sync,
    Fut: Future<Output = Result<(), DbError>> + Send + 'static,
{
    fn name(&self) -> &str {
        &self.name
    }

    fn run<'a>(&'a self, pool: &'a DbPool) -> SeedFuture<'a> {
        Box::pin((self.run)(pool.clone()))
    }
}

/// Seeders, run in the order added. Cheap to clone.
#[derive(Clone, Default)]
pub struct Seeders {
    list: Vec<Arc<dyn Seeder>>,
}

impl std::fmt::Debug for Seeders {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.names()).finish()
    }
}

impl Seeders {
    /// No seeders.
    pub fn new() -> Self {
        Seeders::default()
    }

    /// Add a seeder named `name` that runs `run` with the pool.
    pub fn add<F, Fut>(self, name: impl Into<String>, run: F) -> Self
    where
        F: Fn(DbPool) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), DbError>> + Send + 'static,
    {
        self.seeder(FnSeeder { name: name.into(), run })
    }

    /// Add a [`Seeder`] implemented by hand.
    pub fn seeder(mut self, seeder: impl Seeder + 'static) -> Self {
        self.list.push(Arc::new(seeder));
        self
    }

    /// The seeders' names, in order.
    pub fn names(&self) -> Vec<&str> {
        self.list.iter().map(|s| s.name()).collect()
    }

    /// How many there are.
    pub fn len(&self) -> usize {
        self.list.len()
    }

    /// Whether there are none.
    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }

    /// Run every seeder on `pool`'s primary, in order; answers their names.
    /// Stops at the first that fails, with its name in the error (what it
    /// wrote before failing stays).
    pub async fn run(&self, pool: &DbPool) -> Result<Vec<String>, DbError> {
        let primary = pool.primary();
        let mut ran = Vec::with_capacity(self.list.len());
        for seeder in &self.list {
            seeder.run(primary).await.map_err(|e| DbError::new(format!("seeder '{}' failed: {}", seeder.name(), e.0)))?;
            ran.push(seeder.name().to_string());
        }
        Ok(ran)
    }
}

impl DbPool {
    /// [`seed`](DbPool::seed) the `*.sql` files in `dir`, then run
    /// `seeders` (18.30.0); answers the files and the seeders' names, in the
    /// order they ran. With seeders, a missing `dir` means no SQL seeds
    /// rather than an error.
    pub async fn seed_with(&self, dir: &str, seeders: &Seeders) -> Result<Vec<String>, DbError> {
        let mut ran = if seeders.is_empty() || std::path::Path::new(dir).is_dir() { self.seed(dir).await? } else { Vec::new() };
        ran.extend(seeders.run(self).await?);
        Ok(ran)
    }
}
