//! Factories for models: `#[derive(Factory)]` (next to `#[derive(Model)]`)
//! gives a model `T::factory()`, a [`ModelFactory`] that builds instances
//! with a default value for every field, overrides from
//! [`with`](ModelFactory::with), and saves them with
//! [`create`](ModelFactory::create).
//!
//! ```rust,ignore
//! use rust_web_server::{Factory, Model};
//!
//! #[derive(Model, Factory, Debug, Clone)]
//! #[table(name = "users")]
//! pub struct User {
//!     #[primary_key(auto_increment)]
//!     pub id: i64,
//!     pub name: String,   // "name-1", "name-2", …
//!     pub email: String,  // "email-1@example.com", …
//!     pub age: Option<i32>, // None
//!     pub admin: bool,    // false
//! }
//!
//! let ann = User::factory().with(|u| u.name = "Ann".into()).create(&pool).await?;
//! assert!(ann.id > 0);
//! let unsaved = User::factory().build();
//! let five = User::factory().create_many(5, &pool).await?;
//! ```
//!
//! Default values ([`FactoryValue`]): numbers `0`, `bool` `false`, `Option`
//! `None`, a `String` the field's name and a sequence number (`name-3`; for
//! a field whose name contains `email`, `email-3@example.com`), so unique
//! columns stay unique. The primary key is `0` when it's
//! `auto_increment` (the database picks it) and the sequence number
//! otherwise; `#[ignore]` fields and field types without a `FactoryValue`
//! get `Default::default()`.
//!
//! The sequence is per model type and process-wide — counting across the
//! tests that run in the process, in whatever order they run — so assert on
//! the values a factory returns, not on `name-1`.

use std::any::TypeId;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::{Mutex, OnceLock};

use crate::model::repository::insert_entity;
use crate::model::{DbError, DbPool, Model};

/// A model a factory can build; implemented by `#[derive(Factory)]`.
pub trait Factory: Model + Sized {
    /// The instance with default values for sequence number `seq` (≥ 1),
    /// before overrides.
    fn factory_default(seq: u64) -> Self;
}

/// A type with a factory default; see the module docs for the values.
pub trait FactoryValue: Sized {
    /// The value of field `field` in instance `seq`.
    fn factory_value(field: &str, seq: u64) -> Self;

    /// The value as a primary key that isn't auto-incremented: unique per
    /// instance where the type allows it.
    fn factory_key(field: &str, seq: u64) -> Self {
        Self::factory_value(field, seq)
    }
}

macro_rules! numbers {
    ($($t:ty),*) => {$(
        impl FactoryValue for $t {
            fn factory_value(_field: &str, _seq: u64) -> Self {
                0 as $t
            }
            fn factory_key(_field: &str, seq: u64) -> Self {
                seq as $t
            }
        }
    )*};
}
numbers!(i8, i16, i32, i64, u8, u16, u32, u64, f32, f64);

impl FactoryValue for bool {
    fn factory_value(_field: &str, _seq: u64) -> Self {
        false
    }
}

impl FactoryValue for String {
    fn factory_value(field: &str, seq: u64) -> Self {
        if field.to_ascii_lowercase().contains("email") {
            format!("{field}-{seq}@example.com")
        } else {
            format!("{field}-{seq}")
        }
    }
}

impl<T> FactoryValue for Option<T> {
    fn factory_value(_field: &str, _seq: u64) -> Self {
        None
    }
}

impl FactoryValue for Vec<u8> {
    fn factory_value(_field: &str, _seq: u64) -> Self {
        Vec::new()
    }
}

/// The next sequence number for `T` (1, 2, …), process-wide.
fn next_seq<T: 'static>() -> u64 {
    static SEQUENCES: OnceLock<Mutex<HashMap<TypeId, u64>>> = OnceLock::new();
    let mut sequences = SEQUENCES.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
    let seq = sequences.entry(TypeId::of::<T>()).or_insert(0);
    *seq += 1;
    *seq
}

type Override<T> = Box<dyn Fn(&mut T) + Send + Sync>;

/// Builds instances of `T`; from `T::factory()`.
pub struct ModelFactory<T> {
    overrides: Vec<Override<T>>,
    _model: PhantomData<fn() -> T>,
}

impl<T: Factory + 'static> Default for ModelFactory<T> {
    fn default() -> Self {
        ModelFactory::new()
    }
}

impl<T: Factory + 'static> ModelFactory<T> {
    /// A factory with no overrides.
    pub fn new() -> Self {
        ModelFactory { overrides: Vec::new(), _model: PhantomData }
    }

    /// Change the built instance: `f` runs on each one after the defaults
    /// (and after earlier `with`s), e.g. `.with(|u| u.admin = true)`.
    pub fn with(mut self, f: impl Fn(&mut T) + Send + Sync + 'static) -> Self {
        self.overrides.push(Box::new(f));
        self
    }

    /// The next instance, not saved.
    pub fn build(&self) -> T {
        let mut model = T::factory_default(next_seq::<T>());
        for f in &self.overrides {
            f(&mut model);
        }
        model
    }

    /// `n` instances, not saved.
    pub fn build_many(&self, n: usize) -> Vec<T> {
        (0..n).map(|_| self.build()).collect()
    }

    /// The next instance, inserted into `T`'s table and read back (with its
    /// database-assigned id). Always an `INSERT`, whatever the primary key.
    pub async fn create(&self, pool: &DbPool) -> Result<T, DbError> {
        insert_entity(pool, &self.build()).await
    }

    /// `n` instances, inserted one by one.
    pub async fn create_many(&self, n: usize, pool: &DbPool) -> Result<Vec<T>, DbError> {
        let mut created = Vec::with_capacity(n);
        for _ in 0..n {
            created.push(self.create(pool).await?);
        }
        Ok(created)
    }
}

/// What `#[derive(Factory)]` expands to uses; not API.
#[doc(hidden)]
pub mod __private {
    use super::FactoryValue;
    use std::marker::PhantomData;

    /// Picks a field's default by autoref: [`ViaValue`] when the type has a
    /// `FactoryValue`, else [`ViaDefault`].
    pub struct Probe<T>(pub PhantomData<T>);

    pub trait ViaValue<T> {
        fn make(&self, field: &str, seq: u64) -> T;
        fn make_key(&self, field: &str, seq: u64) -> T;
    }

    impl<T: FactoryValue> ViaValue<T> for &Probe<T> {
        fn make(&self, field: &str, seq: u64) -> T {
            T::factory_value(field, seq)
        }
        fn make_key(&self, field: &str, seq: u64) -> T {
            T::factory_key(field, seq)
        }
    }

    pub trait ViaDefault<T> {
        fn make(&self, field: &str, seq: u64) -> T;
        fn make_key(&self, field: &str, seq: u64) -> T;
    }

    impl<T: Default> ViaDefault<T> for Probe<T> {
        fn make(&self, _field: &str, _seq: u64) -> T {
            T::default()
        }
        fn make_key(&self, _field: &str, _seq: u64) -> T {
            T::default()
        }
    }
}
