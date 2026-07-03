---
title: Transactions
description: Group multiple async database operations into an atomic unit that rolls back automatically on error.
---

`DbPool` provides two ways to work with transactions: a closure-based helper that handles commit and rollback automatically, and a manual `begin` / `commit` / `rollback` API for more complex flows.

## Closure-based transactions

`pool.transaction(|mut tx| async move { … }).await` is the recommended approach. Pass an `async` closure that receives a `DbTransaction`. If the closure returns `Ok`, the transaction is committed. If it returns `Err`, the transaction is rolled back automatically — you never need to call `rollback()` yourself unless you want to roll back inside the closure explicitly.

```rust
use rust_web_server::model::{DbPool, Value};

let pool = DbPool::from_env().await?;

pool.transaction(|mut tx| async move {
    tx.execute(
        "INSERT INTO users (name, email) VALUES (?, ?)",
        &[Value::Text("Alice".into()), Value::Text("alice@example.com".into())],
    ).await?;
    tx.execute(
        "INSERT INTO profiles (user_id, bio) VALUES (?, ?)",
        &[Value::Int(1), Value::Text("Hello!".into())],
    ).await?;
    tx.commit().await
}).await?;
```

The closure can return any `Ok(T)` value; `transaction` passes it back to the caller.

```rust
let new_id: i64 = pool.transaction(|mut tx| async move {
    tx.execute(
        "INSERT INTO orders (user_id, total) VALUES (?, ?)",
        &[Value::Int(1), Value::Float(49.99)],
    ).await?;
    // Retrieve the generated PK via a follow-up query
    let rows = tx.query_rows("SELECT last_insert_rowid() AS id", &[]).await?;
    let id: i64 = rows[0].get("id")?;
    tx.commit().await?;
    Ok(id)
}).await?;
```

## Manual transactions

Use `pool.begin().await` when you need explicit control — for example, when the commit/rollback decision is made outside a single closure scope.

```rust
let mut tx = pool.begin().await?;

let result = do_first_step(&mut tx).await;
if result.is_err() {
    let _ = tx.rollback().await;
    return Err(result.unwrap_err());
}

let result2 = do_second_step(&mut tx).await;
if result2.is_err() {
    let _ = tx.rollback().await;
    return Err(result2.unwrap_err());
}

tx.commit().await?;
```

## Inserting a User and Profile atomically

A common pattern is creating a parent record and a related child record together so that neither exists without the other.

```rust
use rust_web_server::model::{DbPool, Value};

let pool = DbPool::from_env().await?;

pool.transaction(|mut tx| async move {
    // Insert the user
    tx.execute(
        "INSERT INTO users (name, email, active) VALUES (?, ?, ?)",
        &[
            Value::Text("Bob".into()),
            Value::Text("bob@example.com".into()),
            Value::Bool(true),
        ],
    ).await?;

    // Retrieve the generated PK
    let rows = tx.query_rows("SELECT last_insert_rowid() AS id", &[]).await?;
    let user_id: i64 = rows[0].get("id")?;

    // Insert the profile linked to that user
    tx.execute(
        "INSERT INTO profiles (user_id, bio, avatar_url) VALUES (?, ?, ?)",
        &[
            Value::Int(user_id),
            Value::Text("Software engineer".into()),
            Value::Null, // optional field
        ],
    ).await?;

    tx.commit().await?;
    Ok(user_id)
}).await?;
```

If the profile insert fails (e.g., a constraint violation), the closure returns `Err` and `transaction` rolls back the user insert as well, leaving the database unchanged.

## Error handling pattern

Return `Err` from the closure to trigger an automatic rollback:

```rust
use rust_web_server::model::{DbError, Value};

pool.transaction(|mut tx| async move {
    let affected = tx.execute(
        "UPDATE accounts SET balance = balance - ? WHERE id = ? AND balance >= ?",
        &[Value::Float(100.0), Value::Int(sender_id), Value::Float(100.0)],
    ).await?;

    if affected == 0 {
        // Returning Err here causes an automatic rollback
        return Err(DbError::new("insufficient funds or account not found"));
    }

    tx.execute(
        "UPDATE accounts SET balance = balance + ? WHERE id = ?",
        &[Value::Float(100.0), Value::Int(receiver_id)],
    ).await?;

    tx.commit().await
}).await?;
```

:::note[Savepoints]
The current API does not expose SQL savepoints. For nested transaction semantics, use the closure form and handle all steps within a single `transaction` call.
:::
