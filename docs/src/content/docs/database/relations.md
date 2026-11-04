---
title: Relations
description: Load related records with HasMany, HasOne, BelongsTo and BelongsToMany, eager-load them for a whole result set with with(), and manage many-to-many links.
---

The ORM provides relationship helpers for one record — `HasMany<T>`, `HasOne<O>`, `BelongsTo<O>` and (18.30.0) `BelongsToMany<T>` — and `Relation<O, R>` descriptors that [load a relation for a whole result set](#eager-loading-with-with). There is no lazy loading — you call `.load(&pool).await` or `.with(…)` explicitly when you want the data.

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


## Eager loading with `with()`

Calling `.load()` for each row of a list is one query per row (N+1). `with()` loads a relation for the whole result set in **one more query per relation**, `WHERE fk IN (…)` (18.30.0). Describe each relation once, as a `Relation<Owner, Related>`:

```rust
use rust_web_server::model::{Order, Relation};

impl Post {
    pub fn comments() -> Relation<Post, Comment> { Relation::has_many("post_id") } // comments.post_id = posts.id
    pub fn author() -> Relation<Post, User> { Relation::belongs_to("user_id") }    // posts.user_id = users.id
    pub fn tags() -> Relation<Post, Tag> { Relation::belongs_to_many("post_tags", "post_id", "tag_id") }
}

impl User {
    pub fn profile() -> Relation<User, Profile> { Relation::has_one("user_id") }
}

// Three queries, however many posts: the posts, their comments, their authors.
let posts = Post::query(&pool)
    .where_eq("published", true)
    .order_by("id", Order::Desc)
    .with(Post::comments())
    .with(Post::author())
    .fetch_all()
    .await?;

for post in &posts {
    let comments: &[Comment] = post.related(&Post::comments());
    let author: Option<&User> = post.one(&Post::author());
    println!("{} by {:?}: {} comments", post.title, author.map(|a| &a.name), comments.len());
}
```

`with()` turns the query into an `EagerQuery` whose `fetch_all`, `fetch_one` and `paginate` answer `Loaded<Post>` values. A `Loaded<T>` derefs to the model (`post.title`); `related(&rel)` is the relation's rows (empty when there are none), `one(&rel)` the first — the row of a `belongs_to` or `has_one` — `take(&rel)` moves them out, `into_inner()` drops the relations, and `is_loaded(&rel)` says whether the query loaded it.

| Constructor | Meaning | Query |
|---|---|---|
| `Relation::has_many(fk)` | `R.fk` holds the owner's key | `SELECT * FROM r WHERE fk IN (…)` |
| `Relation::has_one(fk)` | as `has_many`, at most one row (the lowest key) | the same |
| `Relation::belongs_to(fk)` | the owner's `fk` holds `R`'s key (`NULL`: none) | `SELECT * FROM r WHERE id IN (…)` |
| `Relation::belongs_to_many(join, owner_key, related_key)` | through a join table | `SELECT r.*, j.owner_key … JOIN join j ON j.related_key = r.id WHERE j.owner_key IN (…)` |

Related rows come in primary-key order, soft-deleted ones (`#[soft_delete]`) left out. Keys go in batches of 500 (`eager::IN_BATCH`), so a larger result set costs one query per batch. A row related to several owners (a shared author, a tag) is cloned into each, so the related type must be `Clone`. The same descriptor loads without a query builder: `Post::comments().load(&pool, &post)` for one owner, `Post::comments().load_for(&pool, &posts)` for a slice (one `Vec` per owner, in order).

Nested loading (comments' authors) is a second step: collect the comments and call `Comment::author().load_for(&pool, &comments)`.

## Many-to-many: `BelongsToMany`

`BelongsToMany<R>` is one owner's links through a join table (18.30.0). It loads the linked rows and adds and removes the links:

```rust
use rust_web_server::model::{BelongsToMany, Value};

// CREATE TABLE post_tags (post_id BIGINT NOT NULL, tag_id BIGINT NOT NULL, PRIMARY KEY (post_id, tag_id));
impl Post {
    pub fn tag_links(&self) -> BelongsToMany<Tag> {
        BelongsToMany::new(Value::Int(self.id), "post_tags", "post_id", "tag_id")
    }
}

let links = post.tag_links();
links.attach(&pool, [rust.id, web.id]).await?;          // links added: 2 (an existing link is skipped)
links.detach(&pool, [web.id]).await?;                   // links removed: 1
let changes = links.sync(&pool, [rust.id, async_.id]).await?; // exactly these: adds and removes as needed
println!("attached {:?}, detached {:?}", changes.attached, changes.detached);
let tags: Vec<Tag> = links.load(&pool).await?;          // the linked rows, soft-deleted ones left out
let ids = links.ids(&pool).await?;                      // the join table's keys
links.detach_all(&pool).await?;
```

`attach` and `sync` run in a transaction (read the links, then write). They insert the two key columns only, so other join-table columns need defaults. Give the join table a primary key or unique index over both columns: the methods never link twice, but two processes attaching at once could. For a whole result set use `Relation::belongs_to_many(…)` with `with()`, above.

:::note[No lazy loading by design]
Lazy loading hides database access behind field access, making it easy to accidentally trigger hundreds of queries. Explicit `.load().await` calls and `with()` keep all I/O visible at the call site, making performance characteristics clear during code review.
:::
