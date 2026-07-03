---
title: "#[derive(Model)]"
description: Map a Rust struct to a database table with the Model proc-macro and its field attributes.
---

`#[derive(Model)]` generates the full `Model` trait implementation for a struct, including typed serialisation, deserialisation, and helper constructors. It is the entry point for every other database feature.

## Minimal example

```rust
use rust_web_server::Model;

#[derive(Model, Debug, Clone)]
pub struct User {
    #[primary_key(auto_increment)]
    pub id: i64,
    pub email: String,
    pub active: bool,
}
```

The table name defaults to the lowercased struct name (`user`). The column names default to the field names.

## Struct attribute: `#[table(name = "...")]`

Override the table name:

```rust
#[derive(Model, Debug, Clone)]
#[table(name = "users")]
pub struct User {
    #[primary_key(auto_increment)]
    pub id: i64,
    pub email: String,
    pub active: bool,
}
```

## Field attributes

### `#[primary_key]`

Marks the field as the primary key. Exactly one field must carry this attribute; the derive macro returns a compile error otherwise.

```rust
#[primary_key]
pub id: i64,   // caller sets id before INSERT
```

Use this variant when you manage the primary key yourself (e.g. a UUID stored as text).

### `#[primary_key(auto_increment)]`

Marks the field as an auto-increment primary key. On INSERT the field is excluded from the column list; the database assigns the value. The generated row is re-fetched after the insert to populate the field.

```rust
#[primary_key(auto_increment)]
pub id: i64,
```

Backend behaviour:
- **SQLite** — uses `last_insert_rowid()`
- **PostgreSQL** — uses `RETURNING id`
- **MySQL** — uses `last_insert_id()`

### `#[column(name = "...")]`

Overrides the column name used in SQL. The field name in Rust can differ from the database column name.

```rust
#[column(name = "first_name")]
pub name: String,
```

### `#[column(unique)]`

Marks the column as unique. This is informational only — the derive macro does not create or alter the index. Use a migration to add the `UNIQUE` constraint in the schema.

```rust
#[column(unique)]
pub email: String,
```

### `#[ignore]`

Excludes the field from all database operations. On `from_row` the field receives `Default::default()`. On `to_values` the field is omitted entirely, so it never appears in INSERT or UPDATE statements.

```rust
#[ignore]
pub display_label: String,   // computed at runtime, not stored
```

## Supported field types

| Rust type | `Value` variant |
|---|---|
| `i16`, `i32`, `i64` | `Value::Int(i64)` |
| `u32`, `u64` | `Value::Int(i64)` (cast) |
| `f32`, `f64` | `Value::Float(f64)` |
| `bool` | `Value::Bool(bool)` |
| `String` | `Value::Text(String)` |
| `Option<T>` | `Value::Null` or inner type |

## Complete example

The following struct uses every attribute type:

```rust
use rust_web_server::Model;

#[derive(Model, Debug, Clone)]
#[table(name = "users")]
pub struct User {
    /// Auto-assigned by the database.
    #[primary_key(auto_increment)]
    pub id: i64,

    /// Maps to the `first_name` column.
    #[column(name = "first_name")]
    pub name: String,

    /// Unique constraint enforced in the schema.
    #[column(unique)]
    pub email: String,

    pub role: String,
    pub active: bool,
    pub score: f64,

    /// Optional — stored as NULL when None.
    pub bio: Option<String>,

    /// Not persisted; computed at runtime.
    #[ignore]
    pub display_label: String,
}
```

## What the macro generates

`#[derive(Model)]` expands to an `impl Model for User` block with these methods:

```rust
impl Model for User {
    fn table_name() -> &'static str { "users" }

    fn column_names() -> &'static [&'static str] {
        &["id", "first_name", "email", "role", "active", "score", "bio"]
        // "display_label" is absent because it is #[ignore]
    }

    fn primary_key_name() -> &'static str { "id" }

    fn primary_key_value(&self) -> Value { self.id.to_column() }

    fn primary_key_auto_increment() -> bool { true }

    fn from_row(row: &ModelRow) -> Result<Self, DbError> {
        Ok(User {
            id:            row.get("id")?,
            name:          row.get("first_name")?,
            email:         row.get("email")?,
            role:          row.get("role")?,
            active:        row.get("active")?,
            score:         row.get("score")?,
            bio:           row.get("bio")?,
            display_label: Default::default(),  // ignored field
        })
    }

    fn to_values(&self) -> Vec<(&'static str, Value)> {
        vec![
            ("id",         self.id.to_column()),
            ("first_name", self.name.to_column()),
            ("email",      self.email.to_column()),
            ("role",       self.role.to_column()),
            ("active",     self.active.to_column()),
            ("score",      self.score.to_column()),
            ("bio",        self.bio.to_column()),
            // "display_label" is absent
        ]
    }
}
```

In addition, two inherent methods are generated on the struct:

```rust
impl User {
    /// Returns a ModelRepository bound to this pool.
    pub fn repository(pool: &DbPool) -> ModelRepository<User, i64>;

    /// Returns a QueryBuilder bound to this pool.
    pub fn query(pool: &DbPool) -> QueryBuilder<User>;
}
```

These are the primary entry points for [repository operations](/database/repository/) and [query building](/database/query-builder/).

Usage example:

```rust
use rust_web_server::model::DbPool;

let pool = DbPool::from_env().await?;

// Save a new user
let mut user = User { id: 0, email: "alice@example.com".into(), active: true };
let mut repo = User::repository(&pool);
repo.save(&mut user).await?;      // id is populated after insert

// Query with the builder
let admins: Vec<User> = User::query(&pool)
    .where_eq("active", true)
    .fetch_all().await?;
```
