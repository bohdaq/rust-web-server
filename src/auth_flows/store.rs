//! Where accounts live: [`UserStore`], in memory or in a database table.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// An account as the auth flows see it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthUser {
    /// Your user id, as text (`"42"`, a UUID).
    pub id: String,
    /// Lower case, trimmed.
    pub email: String,
    /// Argon2id PHC string ([`crypto::hash_password`](crate::crypto::hash_password)).
    pub password_hash: String,
    /// Whether the email address was confirmed.
    pub confirmed: bool,
}

/// Accounts: implement it over your users table, or use [`MemoryUserStore`]
/// / [`DbUserStore`]. Errors are messages; the flows answer them `500`.
/// The methods are synchronous: the flows run as a middleware, on the
/// blocking pool.
pub trait UserStore: Send + Sync {
    /// The account with this email (already lower-cased), if any.
    fn find_by_email(&self, email: &str) -> Result<Option<AuthUser>, String>;
    /// The account with this id, if any.
    fn find_by_id(&self, id: &str) -> Result<Option<AuthUser>, String>;
    /// Create an account (not confirmed). `Ok(None)` when the email is taken.
    fn create(&self, email: &str, password_hash: &str) -> Result<Option<AuthUser>, String>;
    /// Replace the password hash.
    fn set_password_hash(&self, id: &str, password_hash: &str) -> Result<(), String>;
    /// Mark the email address confirmed.
    fn confirm(&self, id: &str) -> Result<(), String>;
}

/// Accounts in memory: tests and trying things out. Clones share them.
#[derive(Clone, Default)]
pub struct MemoryUserStore {
    users: Arc<Mutex<HashMap<String, AuthUser>>>,
}

impl MemoryUserStore {
    pub fn new() -> Self {
        MemoryUserStore::default()
    }

    /// Every account, by id.
    pub fn users(&self) -> Vec<AuthUser> {
        let mut users: Vec<AuthUser> = self.users.lock().unwrap_or_else(|e| e.into_inner()).values().cloned().collect();
        users.sort_by(|a, b| a.id.cmp(&b.id));
        users
    }
}

impl UserStore for MemoryUserStore {
    fn find_by_email(&self, email: &str) -> Result<Option<AuthUser>, String> {
        Ok(self.users.lock().unwrap_or_else(|e| e.into_inner()).values().find(|u| u.email == email).cloned())
    }

    fn find_by_id(&self, id: &str) -> Result<Option<AuthUser>, String> {
        Ok(self.users.lock().unwrap_or_else(|e| e.into_inner()).get(id).cloned())
    }

    fn create(&self, email: &str, password_hash: &str) -> Result<Option<AuthUser>, String> {
        let mut users = self.users.lock().unwrap_or_else(|e| e.into_inner());
        if users.values().any(|u| u.email == email) {
            return Ok(None);
        }
        let id = (users.len() + 1).to_string();
        let user = AuthUser { id: id.clone(), email: email.to_string(), password_hash: password_hash.to_string(), confirmed: false };
        users.insert(id, user.clone());
        Ok(Some(user))
    }

    fn set_password_hash(&self, id: &str, password_hash: &str) -> Result<(), String> {
        match self.users.lock().unwrap_or_else(|e| e.into_inner()).get_mut(id) {
            Some(user) => {
                user.password_hash = password_hash.to_string();
                Ok(())
            }
            None => Err(format!("no user {id}")),
        }
    }

    fn confirm(&self, id: &str) -> Result<(), String> {
        match self.users.lock().unwrap_or_else(|e| e.into_inner()).get_mut(id) {
            Some(user) => {
                user.confirmed = true;
                Ok(())
            }
            None => Err(format!("no user {id}")),
        }
    }
}

/// Accounts in the `rws_users` table of a [`DbPool`](crate::model::DbPool)
/// (`model-*` features), created if absent:
///
/// ```sql
/// CREATE TABLE rws_users (
///     id            VARCHAR(64)  PRIMARY KEY,   -- 32 hex digits
///     email         VARCHAR(191) NOT NULL UNIQUE,
///     password_hash TEXT         NOT NULL,
///     confirmed     INTEGER      NOT NULL DEFAULT 0,
///     created_at    BIGINT       NOT NULL
/// )
/// ```
///
/// The store's calls wait for the database on the calling thread (the
/// flows run on the blocking pool).
#[cfg(any(feature = "model-sqlite", feature = "model-postgres", feature = "model-mysql"))]
#[derive(Clone)]
pub struct DbUserStore {
    pool: crate::model::DbPool,
}

#[cfg(any(feature = "model-sqlite", feature = "model-postgres", feature = "model-mysql"))]
impl DbUserStore {
    /// Use `pool`, creating `rws_users` if it isn't there.
    pub async fn new(pool: crate::model::DbPool) -> Result<Self, crate::model::DbError> {
        pool.execute(
            "CREATE TABLE IF NOT EXISTS rws_users (id VARCHAR(64) PRIMARY KEY, email VARCHAR(191) NOT NULL UNIQUE, \
             password_hash TEXT NOT NULL, confirmed INTEGER NOT NULL DEFAULT 0, created_at BIGINT NOT NULL)",
            &[],
        )
        .await?;
        Ok(DbUserStore { pool })
    }

    pub fn pool(&self) -> &crate::model::DbPool {
        &self.pool
    }

    fn sql(&self, sql: &str) -> String {
        #[cfg(feature = "model-postgres")]
        if self.pool.backend() == crate::model::Backend::Postgres {
            let mut out = String::new();
            let mut n = 0;
            for c in sql.chars() {
                if c == '?' {
                    n += 1;
                    out.push_str(&format!("${n}"));
                } else {
                    out.push(c);
                }
            }
            return out;
        }
        sql.to_string()
    }

    fn run<T: Send + 'static>(&self, future: impl std::future::Future<Output = Result<T, crate::model::DbError>> + Send + 'static) -> Result<T, String> {
        rws_server::async_bridge::block_on_shared(future).map_err(|e| e.to_string())
    }

    fn one(&self, sql: &str, params: Vec<crate::model::Value>) -> Result<Option<AuthUser>, String> {
        let (pool, sql) = (self.pool.clone(), self.sql(sql));
        let rows = self.run(async move { pool.query_rows(&sql, &params).await })?;
        let Some(row) = rows.first() else { return Ok(None) };
        let user = (|| -> Result<AuthUser, crate::model::DbError> {
            Ok(AuthUser { id: row.get("id")?, email: row.get("email")?, password_hash: row.get("password_hash")?, confirmed: row.get::<i64>("confirmed")? != 0 })
        })()
        .map_err(|e| e.to_string())?;
        Ok(Some(user))
    }

    fn exec(&self, sql: &str, params: Vec<crate::model::Value>) -> Result<u64, String> {
        let (pool, sql) = (self.pool.clone(), self.sql(sql));
        self.run(async move { pool.execute(&sql, &params).await })
    }
}

#[cfg(any(feature = "model-sqlite", feature = "model-postgres", feature = "model-mysql"))]
impl UserStore for DbUserStore {
    fn find_by_email(&self, email: &str) -> Result<Option<AuthUser>, String> {
        self.one("SELECT id, email, password_hash, confirmed FROM rws_users WHERE email = ?", vec![crate::model::Value::Text(email.to_string())])
    }

    fn find_by_id(&self, id: &str) -> Result<Option<AuthUser>, String> {
        self.one("SELECT id, email, password_hash, confirmed FROM rws_users WHERE id = ?", vec![crate::model::Value::Text(id.to_string())])
    }

    fn create(&self, email: &str, password_hash: &str) -> Result<Option<AuthUser>, String> {
        use crate::model::Value;
        if self.find_by_email(email)?.is_some() {
            return Ok(None);
        }
        let id = crate::crypto::generate_token(16);
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
        let inserted = self.exec(
            "INSERT INTO rws_users (id, email, password_hash, confirmed, created_at) VALUES (?, ?, ?, 0, ?)",
            vec![Value::Text(id.clone()), Value::Text(email.to_string()), Value::Text(password_hash.to_string()), Value::Int(now)],
        );
        match inserted {
            Ok(_) => Ok(Some(AuthUser { id, email: email.to_string(), password_hash: password_hash.to_string(), confirmed: false })),
            // The UNIQUE index: another sign-up took the email in between.
            Err(_) if self.find_by_email(email)?.is_some() => Ok(None),
            Err(e) => Err(e),
        }
    }

    fn set_password_hash(&self, id: &str, password_hash: &str) -> Result<(), String> {
        use crate::model::Value;
        self.exec("UPDATE rws_users SET password_hash = ? WHERE id = ?", vec![Value::Text(password_hash.to_string()), Value::Text(id.to_string())]).map(|_| ())
    }

    fn confirm(&self, id: &str) -> Result<(), String> {
        use crate::model::Value;
        self.exec("UPDATE rws_users SET confirmed = 1 WHERE id = ?", vec![Value::Text(id.to_string())]).map(|_| ())
    }
}
