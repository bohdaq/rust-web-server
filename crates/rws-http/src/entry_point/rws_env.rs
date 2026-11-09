//! `RWS_ENV`: which kind of environment the process runs in — `dev`,
//! `test` or `prod` (the default). Since 18.28.0.
//!
//! Read from the process environment only (no config-file key, no flag):
//! it says how the rest of the configuration should be treated, so it is
//! set where the process is started. It decides whether `dev_mcp()`
//! mounts its tools without a token and, since 18.35.0 (#95), the
//! defaults that differ between a developer's machine and production:
//! [`shows_error_details`](RwsEnv::shows_error_details) (the error pages of
//! `ErrorPages`) and [`reloads_templates`](RwsEnv::reloads_templates)
//! (templates re-read before each render).

/// The environment kind named by `RWS_ENV`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RwsEnv {
    /// `RWS_ENV=dev` (or `development`): a developer's machine.
    Dev,
    /// `RWS_ENV=test`: an automated test run.
    Test,
    /// `RWS_ENV=prod` (or `production`), unset, or anything else.
    #[default]
    Prod,
}

impl RwsEnv {
    /// The environment variable read by [`RwsEnv::current`].
    pub const VAR: &'static str = "RWS_ENV";

    /// `RWS_ENV` as it is now; unset or unrecognised is [`RwsEnv::Prod`],
    /// so nothing is opened up by a typo.
    pub fn current() -> RwsEnv {
        std::env::var(Self::VAR).ok().and_then(|v| Self::parse(&v)).unwrap_or_default()
    }

    /// `dev`/`development`, `test`, `prod`/`production` (case-insensitive,
    /// surrounding spaces ignored); `None` for anything else.
    pub fn parse(value: &str) -> Option<RwsEnv> {
        match value.trim().to_ascii_lowercase().as_str() {
            "dev" | "development" => Some(RwsEnv::Dev),
            "test" => Some(RwsEnv::Test),
            "prod" | "production" => Some(RwsEnv::Prod),
            _ => None,
        }
    }

    /// `"dev"`, `"test"` or `"prod"`.
    pub fn as_str(self) -> &'static str {
        match self {
            RwsEnv::Dev => "dev",
            RwsEnv::Test => "test",
            RwsEnv::Prod => "prod",
        }
    }

    /// `true` for [`RwsEnv::Dev`].
    pub fn is_dev(self) -> bool {
        self == RwsEnv::Dev
    }

    /// Whether errors show their details (the panic message, where it
    /// happened, the backtrace) to the client: in `dev` and `test`, never in
    /// `prod` (18.35.0).
    pub fn shows_error_details(self) -> bool {
        self != RwsEnv::Prod
    }

    /// Whether templates are re-read from disk before each render, so an
    /// edit shows on the next reload of the page: in `dev` (18.35.0).
    pub fn reloads_templates(self) -> bool {
        self == RwsEnv::Dev
    }
}

impl std::fmt::Display for RwsEnv {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::RwsEnv;

    #[test]
    fn profile_defaults_per_environment() {
        assert!(RwsEnv::Dev.shows_error_details() && RwsEnv::Test.shows_error_details() && !RwsEnv::Prod.shows_error_details());
        assert!(RwsEnv::Dev.reloads_templates() && !RwsEnv::Test.reloads_templates() && !RwsEnv::Prod.reloads_templates());
    }

    #[test]
    fn parses_the_three_names_and_their_long_forms() {
        assert_eq!(RwsEnv::parse("dev"), Some(RwsEnv::Dev));
        assert_eq!(RwsEnv::parse(" Development "), Some(RwsEnv::Dev));
        assert_eq!(RwsEnv::parse("TEST"), Some(RwsEnv::Test));
        assert_eq!(RwsEnv::parse("prod"), Some(RwsEnv::Prod));
        assert_eq!(RwsEnv::parse("production"), Some(RwsEnv::Prod));
    }

    #[test]
    fn anything_else_is_not_an_environment() {
        assert_eq!(RwsEnv::parse(""), None);
        assert_eq!(RwsEnv::parse("staging"), None);
        assert_eq!(RwsEnv::parse("devel"), None);
    }

    #[test]
    fn default_is_prod_and_names_round_trip() {
        assert_eq!(RwsEnv::default(), RwsEnv::Prod);
        for env in [RwsEnv::Dev, RwsEnv::Test, RwsEnv::Prod] {
            assert_eq!(RwsEnv::parse(env.as_str()), Some(env));
            assert_eq!(env.to_string(), env.as_str());
        }
        assert!(RwsEnv::Dev.is_dev());
        assert!(!RwsEnv::Test.is_dev());
    }
}
