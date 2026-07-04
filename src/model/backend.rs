//! [`Backend`] — which compiled-in database driver a [`super::DbPool`] uses.

use super::DbError;

/// Selects which compiled-in database backend a [`super::DbPool`] talks to.
///
/// Unlike most of this crate's feature flags, `model-sqlite`, `model-postgres`,
/// and `model-mysql` are not mutually exclusive: enabling more than one in the
/// same binary compiles a `DbPool` variant for each, so e.g. a hot-path SQLite
/// pool and an analytics-tier Postgres pool can coexist in one process. Each
/// `DbPool`/`DbConfig` picks one backend to talk to via this enum.
///
/// `Backend` only has variants for the `model-*` features actually compiled
/// in — matching on it is exhaustive without a wildcard arm, so enabling a
/// backend and forgetting a match arm for it is a compile error, not a silent
/// gap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    #[cfg(feature = "model-sqlite")]
    Sqlite,
    #[cfg(feature = "model-postgres")]
    Postgres,
    #[cfg(feature = "model-mysql")]
    MySql,
}

impl Backend {
    /// Parse a backend name as read from `RWS_DB_BACKEND`: `"sqlite"`,
    /// `"postgres"`/`"postgresql"`, or `"mysql"` (case-insensitive). Errors
    /// (naming the backends actually compiled in) if `s` names a backend that
    /// either doesn't exist or wasn't compiled in.
    pub fn parse(s: &str) -> Result<Backend, DbError> {
        match s.to_ascii_lowercase().as_str() {
            #[cfg(feature = "model-sqlite")]
            "sqlite" => Ok(Backend::Sqlite),
            #[cfg(feature = "model-postgres")]
            "postgres" | "postgresql" => Ok(Backend::Postgres),
            #[cfg(feature = "model-mysql")]
            "mysql" => Ok(Backend::MySql),
            other => Err(DbError::new(format!(
                "unknown or not-compiled-in database backend '{}' (compiled in: {})",
                other,
                Backend::compiled_in_names(),
            ))),
        }
    }

    /// Lowercase name, e.g. for error messages and logging.
    pub fn name(&self) -> &'static str {
        match self {
            #[cfg(feature = "model-sqlite")]
            Backend::Sqlite => "sqlite",
            #[cfg(feature = "model-postgres")]
            Backend::Postgres => "postgres",
            #[cfg(feature = "model-mysql")]
            Backend::MySql => "mysql",
        }
    }

    fn compiled_in_names() -> String {
        let mut names: Vec<&str> = Vec::new();
        #[cfg(feature = "model-sqlite")]
        names.push("sqlite");
        #[cfg(feature = "model-postgres")]
        names.push("postgres");
        #[cfg(feature = "model-mysql")]
        names.push("mysql");
        names.join(", ")
    }

    /// `Some(backend)` if exactly one `model-*` feature is compiled in — the
    /// unambiguous default used by [`super::DbConfig::from_env`] when
    /// `RWS_DB_BACKEND` isn't set. `None` when more than one is compiled in,
    /// which requires the caller to say explicitly which one they mean.
    pub(crate) fn unambiguous_default() -> Option<Backend> {
        let candidates: &[Backend] = &[
            #[cfg(feature = "model-sqlite")]
            Backend::Sqlite,
            #[cfg(feature = "model-postgres")]
            Backend::Postgres,
            #[cfg(feature = "model-mysql")]
            Backend::MySql,
        ];
        if candidates.len() == 1 { Some(candidates[0]) } else { None }
    }
}
