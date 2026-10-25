---
title: Relations
description: Load related records with async HasMany, HasOne, and BelongsTo helpers that make queries explicit and prevent hidden N+1 problems.
---

The ORM provides three relationship helpers: `HasMany<T>`, `HasOne<O>`, and `BelongsTo<O>`. Each holds the information needed to load related records on demand. There is no lazy loading — you call `.load(&pool).await` explicitly when you want the data.

Relation helpers are not fields of a model: `#[derive(Model)]` maps every field to a column, and a `HasMany` field has no column. Build the helper from a loaded record instead — a small method per relation keeps the call site readable.

## HasMany

`HasMany<T>` represents a one-to-many relationship from an owner to a collection of child records. The child table has a foreign key column that holds the owner's primary key. Construct it with the owner's primary key value and the foreign key column name on the child table:

```rust
use rust_web_server::model::relation::HasMany;
use rust_web_server::model::Value;
use rust_web_server::Model;

#[derive(Model, Debug, Clone, Default)]
#[table(name = "users")]
pub struct User {
    #[primary_key(auto_increment)]
    pub id: i64,
    pub name: String,
}

impl User {
    pub fn posts(&self) -> HasMany<Post> {
        HasMany::new(Value::Int(self.id), "user_id")
    }
}
```

Load the related records by calling `.load(&pool).await`:

```rust
let posts: Vec<Post> = user.posts().load(&pool).await?;
```

Under the hood this issues: `SELECT * FROM posts WHERE user_id = ?`.

## HasOne

`HasOne<O>` represents a one-to-one relationship where the child table holds the foreign key (the inverse of `BelongsTo`).

```rust
use rust_web_server::model::relation::HasOne;

impl User {
    pub fn profile(&self) -> HasOne<Profile> {
        HasOne::new(Value::Int(self.id), "user_id")
    }
}

// Loading
let profile: Option<Profile> = user.profile().load(&pool).await?;
```

Under the hood: `SELECT * FROM profiles WHERE user_id = ? LIMIT 1`.

## BelongsTo

`BelongsTo<O>` is the inverse side — the child record holds a foreign key that points to the owner's primary key.

```rust
use rust_web_server::model::relation::BelongsTo;

#[derive(Model, Debug, Clone, Default)]
#[table(name = "posts")]
pub struct Post {
    #[primary_key(auto_increment)]
    pub id: i64,
    pub title: String,
    pub user_id: i64,
}

impl Post {
    pub fn user(&self) -> BelongsTo<User> {
        BelongsTo::new(Value::Int(self.user_id))
    }
}

// Loading
let author: Option<User> = post.user().load(&pool).await?;
```

Under the hood: `SELECT * FROM users WHERE id = ? LIMIT 1`.

## Complete User + Post example

```rust
use rust_web_server::model::{DbPool, Value};

let pool = DbPool::from_env().await?;

// Load a user
let user: User = pool
    .query::<User>("SELECT * FROM users WHERE id = ? LIMIT 1", &[Value::Int(1)])
    .await?
    .into_iter()
    .next()
    .expect("user 1");

// Load that user's posts — one extra query
let posts: Vec<Post> = user.posts().load(&pool).await?;

// Load the author of a post — one extra query
if let Some(post) = posts.first() {
    let author: Option<User> = post.user().load(&pool).await?;
}
```

## Avoiding N+1 queries

The explicit `.load().await` design makes N+1 queries visible — if you call `.load().await` inside a loop you will issue one query per iteration. The recommended pattern is to batch-load related records using `IN (…)`.

```rust
use rust_web_server::model::{DbPool, Value};

// Step 1 — load users (1 query)
let users: Vec<User> = pool.query::<User>("SELECT * FROM users WHERE active = ?", &[Value::Bool(true)]).await?;

// Step 2 — collect IDs
let user_ids: Vec<Value> = users.iter().map(|u| Value::Int(u.id)).collect();

// Step 3 — build IN clause and load all posts (1 query)
if !user_ids.is_empty() {
    let placeholders = user_ids.iter().map(|_| "?").collect::<Vec<_>>().join(", ");
    let sql = format!("SELECT * FROM posts WHERE user_id IN ({})", placeholders);
    let posts: Vec<Post> = pool.query::<Post>(&sql, &user_ids).await?;

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
Lazy loading hides database access behind field access, making it easy to accidentally trigger hundreds of queries. Explicit `.load().await` calls keep all I/O visible at the call site, making performance characteristics clear during code review.
:::
