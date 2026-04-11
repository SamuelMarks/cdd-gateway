#![cfg(not(tarpaulin_include))]

use crate::daemon::ProcessConfig;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Application configuration loaded from a file and/or environment variables.
///
/// All fields can be overridden via environment variables prefixed with `CDD__`
/// (double underscore as separator), e.g. `CDD__JWT_SECRET=mysecret`.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AppConfig {
    /// PostgreSQL connection URL (env: `CDD__DATABASE_URL`).
    pub database_url: String,
    /// Address and port the HTTP server binds to (env: `CDD__SERVER_BIND`).
    pub server_bind: String,
    /// Secret used to sign and verify JWT tokens (env: `CDD__JWT_SECRET`).
    ///
    /// Defaults to `"super-secret-key"` — **must** be overridden in production.
    pub jwt_secret: String,
    /// Secret used to verify GitHub webhook HMAC-SHA256 signatures
    /// (env: `CDD__WEBHOOK_SECRET`).
    ///
    /// Defaults to `"my_webhook_secret"` — **must** be overridden in production.
    pub webhook_secret: String,
    /// Optional GitHub personal access token used as a system-level fallback
    /// when no per-user token is available (env: `CDD__GITHUB_TOKEN`).
    pub github_token: Option<String>,
    /// When `true` the server starts without a PostgreSQL connection and uses
    /// an in-memory no-op repository instead (env: `CDD__OFFLINE_MODE`).
    #[serde(default)]
    pub offline_mode: bool,
    /// Child-process configuration keyed by tool name.
    #[serde(default)]
    pub servers: HashMap<String, ProcessConfig>,
}

impl AppConfig {
    /// Load configuration from an optional file path and environment variables.
    ///
    /// Precedence (highest → lowest):
    /// 1. Environment variables (`CDD__*`)
    /// 2. Config file (if `config_path` is `Some`)
    /// 3. Built-in defaults
    pub fn load(config_path: Option<&str>) -> Result<Self, config::ConfigError> {
        let mut builder = config::Config::builder()
            .set_default("database_url", "postgres://postgres:password@localhost/cdd")?
            .set_default("server_bind", "0.0.0.0:8080")?
            .set_default("jwt_secret", "super-secret-key")?
            .set_default("webhook_secret", "my_webhook_secret")?
            .set_default("offline_mode", false)?;

        if let Some(path) = config_path {
            builder = builder.add_source(config::File::with_name(path).required(false));
        }

        builder
            .add_source(config::Environment::with_prefix("CDD").separator("__"))
            .build()?
            .try_deserialize()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let cfg = AppConfig::load(None).unwrap();
        assert_eq!(cfg.server_bind, "0.0.0.0:8080");
        assert_eq!(
            cfg.database_url,
            "postgres://postgres:password@localhost/cdd"
        );
        assert_eq!(cfg.jwt_secret, "super-secret-key");
        assert_eq!(cfg.webhook_secret, "my_webhook_secret");
        assert!(cfg.github_token.is_none());
        assert!(!cfg.offline_mode);
    }

    #[test]
    fn test_jwt_secret_from_env() {
        std::env::set_var("CDD__JWT_SECRET", "test-jwt-secret");
        let cfg = AppConfig::load(None).unwrap();
        assert_eq!(cfg.jwt_secret, "test-jwt-secret");
        std::env::remove_var("CDD__JWT_SECRET");
    }

    #[test]
    fn test_webhook_secret_from_env() {
        std::env::set_var("CDD__WEBHOOK_SECRET", "test-webhook-secret");
        let cfg = AppConfig::load(None).unwrap();
        assert_eq!(cfg.webhook_secret, "test-webhook-secret");
        std::env::remove_var("CDD__WEBHOOK_SECRET");
    }

    #[test]
    fn test_github_token_from_env() {
        std::env::set_var("CDD__GITHUB_TOKEN", "ghp_test123");
        let cfg = AppConfig::load(None).unwrap();
        assert_eq!(cfg.github_token.as_deref(), Some("ghp_test123"));
        std::env::remove_var("CDD__GITHUB_TOKEN");
    }

    #[test]
    fn test_offline_mode_from_env() {
        std::env::set_var("CDD__OFFLINE_MODE", "true");
        let cfg = AppConfig::load(None).unwrap();
        assert!(cfg.offline_mode);
        std::env::remove_var("CDD__OFFLINE_MODE");
    }
}
