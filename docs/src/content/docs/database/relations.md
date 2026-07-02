---
title: Relations
description: Load related records with HasMany, HasOne, and BelongsTo helpers that make queries explicit and prevent hidden N+1 problems.
---

The ORM provides three relationship helpers: `HasMany<T>`, `HasOne<O>`, and `BelongsTo<O>`. Each holds the information needed to load related records on demand. There is no lazy loading — you call `.load(&mut conn)` explicitly when you want the data.

## HasMany

`HasMany<T>` represents a one-to-many relationship from an owner to a collection of child records. The child table has a foreign key column that holds the owner's primary key.

```rust
use rust_web_server::model::relation::HasMany;
use rust_web_server::model::Value;

pub struct User {
    pub id: i64,
    pub name: String,
    // Declares the relationship — stores owner PK + FK column name
    pub posts: HasMany<Post>,
}
```

Construct the helper with the owner's primary key value and the foreign key column name on the child table:

```rust
impl User {
    pub fn new(id: i64, name: String) -> Self {
        User {
            posts: HasMany::new(Value::Int(id), "user_id"),
            id,
            name,
        }
    }
}
```

Load the related records by calling `.load`:

```rust
let mut conn = DbConnection::open(&config)?;
let user = /* fetch user */;

let posts: Vec<Post> = user.posts.load(&mut conn)?;
```

Under the hood this issues: `SELECT * FROM posts WHERE user_id = ?`.

## HasOne

`HasOne<O>` represents a one-to-one relationship where the child table holds the foreign key (the inverse of `BelongsTo`).

```rust
use rust_web_server::model::relation::HasOne;
use rust_web_server::model::Value;

pub struct User {
    pub id: i64,
    pub name: String,
    pub profile: HasOne<Profile>,
}

impl User {
    pub fn new(id: i64, name: String) -> Self {
        User {
            profile: HasOne::new(Value::Int(id), "user_id"),
            id,
            name,
        }
    }
}

// Loading
let profile: Option<Profile> = user.profile.load(&mut conn)?;
```

Under the hood: `SELECT * FROM profiles WHERE user_id = ? LIMIT 1`.

## BelongsTo

`BelongsTo<O>` is the inverse side — the child record holds a foreign key that points to the owner's primary key.

```rust
use rust_web_server::model::relation::BelongsTo;
use rust_web_server::model::Value;

pub struct Post {
    pub id: i64,
    pub title: String,
    pub user_id: i64,
    pub user: BelongsTo<User>,
}

impl Post {
    pub fn new(id: i64, title: String, user_id: i64) -> Self {
        Post {
            user: BelongsTo::new(Value::Int(user_id)),
            id,
            title,
            user_id,
        }
    }
}

// Loading
let author: Option<User> = post.user.load(&mut conn)?;
```

Under the hood: `SELECT * FROM users WHERE id = ? LIMIT 1`.

## Complete User + Post example

```rust
use rust_web_server::model::{DbConfig, DbConnection, Value};
use rust_web_server::model::relation::{HasMany, BelongsTo};

// --- Models ---

pub struct User {
    pub id: i64,
    pub name: String,
    pub posts: HasMany<Post>,
}

pub struct Post {
    pub id: i64,
    pub user_id: i64,
    pub title: String,
    pub user: BelongsTo<User>,
}

// --- Usage ---

let config = DbConfig::from_env()?;
let mut conn = DbConnection::open(&config)?;

// Load a user
let rows = conn.query_raw("SELECT * FROM users WHERE id = ? LIMIT 1", &[Value::Int(1)])?;
let row = rows.into_iter().next().unwrap();
let user = User {
    id: row.get::<i64>("id")?,
    name: row.get::<String>("name")?,
    posts: HasMany::new(row.get::<Value>("id").unwrap_or(Value::Null), "user_id"),
};

// Load that user's posts — one extra query
let posts: Vec<Post> = user.posts.load(&mut conn)?;

// Load the author of a post — one extra query
if let Some(post) = posts.first() {
    let author: Option<User> = post.user.load(&mut conn)?;
}
```

## Avoiding N+1 queries

The explicit `.load()` design makes N+1 queries visible — if you call `.load()` inside a loop you will issue one query per iteration. The recommended pattern is to batch-load related records using `IN (…)`.

```rust
use rust_web_server::model::{DbConnection, ModelRow, Value};

// Step 1 — load users (1 query)
let users: Vec<User> = conn.query::<User>("SELECT * FROM users WHERE active = ?", &[Value::Bool(true)])?;

// Step 2 — collect IDs
let user_ids: Vec<Value> = users.iter().map(|u| Value::Int(u.id)).collect();

// Step 3 — build IN clause and load all posts (1 query)
if !user_ids.is_empty() {
    let placeholders = user_ids.iter().enumerate()
        .map(|(i, _)| format!("?"))          // use $N for PostgreSQL
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!("SELECT * FROM posts WHERE user_id IN ({})", placeholders);
    let posts: Vec<Post> = conn.query::<Post>(&sql, &user_ids)?;

    // Step 4 — group in memory
    use std::collections::HashMap;
    let mut posts_by_user: HashMap<i64, Vec<Post>> = HashMap::new();
    for post in posts {
        posts_by_user.entry(post.user_id).or_default().push(post);
    }
}
```

This loads N users and all their posts in exactly 2 queries regardless of how many users there are.

:::note[No lazy loading by design]
Lazy loading hides database access behind field access, making it easy to accidentally trigger hundreds of queries. Explicit `.load()` calls keep all I/O visible at the call site, making performance characteristics clear during code review.
:::
