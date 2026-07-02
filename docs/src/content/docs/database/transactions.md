---
title: Transactions
description: Group multiple database operations into an atomic unit that rolls back automatically on error.
---

`DbConnection` provides two ways to work with transactions: a closure-based helper that handles commit and rollback automatically, and a manual `begin` / `commit` / `rollback` API for more complex flows.

## Closure-based transactions

`conn.transaction(|c| { … })` is the recommended approach. Pass a closure that receives a mutable borrow of the connection. If the closure returns `Ok`, the transaction is committed. If it returns `Err`, the transaction is rolled back automatically — you never need to call `rollback()` yourself.

```rust
use rust_web_server::model::{DbConfig, DbConnection, Value};

let config = DbConfig::from_env()?;
let mut conn = DbConnection::open(&config)?;

conn.transaction(|c| {
    c.execute(
        "INSERT INTO users (name, email) VALUES (?, ?)",
        &[Value::Text("Alice".into()), Value::Text("alice@example.com".into())],
    )?;
    c.execute(
        "INSERT INTO profiles (user_id, bio) VALUES (?, ?)",
        &[Value::Int(c.last_insert_rowid()), Value::Text("Hello!".into())],
    )?;
    Ok(())
})?;
```

The closure can return any `Ok(T)` value; `transaction` threads it back to the caller.

```rust
let new_id: i64 = conn.transaction(|c| {
    c.execute(
        "INSERT INTO orders (user_id, total) VALUES (?, ?)",
        &[Value::Int(1), Value::Float(49.99)],
    )?;
    Ok(c.last_insert_rowid())
})?;
```

## Manual transactions

Use `begin()`, `commit()`, and `rollback()` when you need explicit control — for example, when the commit/rollback decision is made outside a single closure scope.

```rust
conn.begin()?;

let result = do_first_step(&mut conn);
if result.is_err() {
    conn.rollback()?;
    return Err(result.unwrap_err());
}

let result2 = do_second_step(&mut conn);
if result2.is_err() {
    conn.rollback()?;
    return Err(result2.unwrap_err());
}

conn.commit()?;
```

Each of these methods calls `conn.execute("BEGIN" | "COMMIT" | "ROLLBACK", &[])` internally, so they work with all supported backends.

## Inserting a User and Profile atomically

A common pattern is creating a parent record and a related child record together so that neither exists without the other.

```rust
use rust_web_server::model::{DbConfig, DbConnection, Value};

let config = DbConfig::from_env()?;
let mut conn = DbConnection::open(&config)?;

conn.transaction(|c| {
    // Insert the user
    c.execute(
        "INSERT INTO users (name, email, active) VALUES (?, ?, ?)",
        &[
            Value::Text("Bob".into()),
            Value::Text("bob@example.com".into()),
            Value::Bool(true),
        ],
    )?;

    // Retrieve the generated PK (SQLite)
    let user_id = c.last_insert_rowid();

    // Insert the profile linked to that user
    c.execute(
        "INSERT INTO profiles (user_id, bio, avatar_url) VALUES (?, ?, ?)",
        &[
            Value::Int(user_id),
            Value::Text("Software engineer".into()),
            Value::Null, // optional field
        ],
    )?;

    Ok(user_id)
})?;
```

If the profile insert fails (e.g., a constraint violation), the closure returns `Err` and `transaction` rolls back the user insert as well, leaving the database unchanged.

## Error handling pattern

Return `Err` from the closure to trigger an automatic rollback:

```rust
use rust_web_server::model::{DbError, Value};

conn.transaction(|c| {
    let affected = c.execute(
        "UPDATE accounts SET balance = balance - ? WHERE id = ? AND balance >= ?",
        &[Value::Float(100.0), Value::Int(sender_id), Value::Float(100.0)],
    )?;

    if affected == 0 {
        // Returning Err here causes an automatic rollback
        return Err(DbError::new("insufficient funds or account not found"));
    }

    c.execute(
        "UPDATE accounts SET balance = balance + ? WHERE id = ?",
        &[Value::Float(100.0), Value::Int(receiver_id)],
    )?;

    Ok(())
})?;
```

:::note[Savepoints]
The current API does not expose SQL savepoints. For nested transaction semantics, use the closure form and handle all steps within a single `transaction` call.
:::
