#![deny(missing_docs, clippy::missing_docs_in_private_items)]
//! Application configuration loading and strong domain types.
//!
//! Provides [`AppConfig`] and [`ExecutionMode`] for configuring the gateway via
//! TOML/JSON configuration files and `CDD__*` environment variables.

use cdd_engine::daemon::ProcessConfig;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Execution mode for running CDD target language compilers in JSON-RPC.
#[derive(Debug, Default, Deserialize, Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ExecutionMode {
    /// Execute native target binaries via OS subprocesses.
    #[default]
    Native,
    /// Execute WebAssembly target binaries inside the sandboxed WASM runtime.
    Wasm,
}

/// Application configuration loaded from a file and/or environment variables.
///
/// All fields can be overridden via environment variables prefixed with `CDD__`
/// (double underscore as separator), e.g. `CDD__JWT_SECRET=mysecret`.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AppConfig {
    /// `PostgreSQL` connection URL (env: `CDD__DATABASE_URL`).
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
    /// GitHub OAuth application client ID (env: `CDD__GITHUB_CLIENT_ID`).
    #[serde(default = "default_github_client_id")]
    pub github_client_id: String,
    /// GitHub OAuth application client secret (env: `CDD__GITHUB_CLIENT_SECRET`).
    #[serde(default = "default_github_client_secret")]
    pub github_client_secret: String,
    /// Base URL for the GitHub REST API (env: `CDD__GITHUB_API_BASE_URL`).
    #[serde(default = "default_github_api_base_url")]
    pub github_api_base_url: String,
    /// Base URL for GitHub HTML / OAuth endpoints (env: `CDD__GITHUB_HTML_BASE_URL`).
    #[serde(default = "default_github_html_base_url")]
    pub github_html_base_url: String,
    /// Google OAuth 2.0 client ID (env: `CDD__GOOGLE_CLIENT_ID`).
    #[serde(default = "default_google_client_id")]
    pub google_client_id: String,
    /// Google OAuth 2.0 client secret (env: `CDD__GOOGLE_CLIENT_SECRET`).
    #[serde(default = "default_google_client_secret")]
    pub google_client_secret: String,
    /// Google OAuth 2.0 token exchange URL (env: `CDD__GOOGLE_TOKEN_URL`).
    #[serde(default = "default_google_token_url")]
    pub google_token_url: String,
    /// Google OAuth 2.0 `UserInfo` endpoint URL (env: `CDD__GOOGLE_USERINFO_URL`).
    #[serde(default = "default_google_userinfo_url")]
    pub google_userinfo_url: String,
    /// Maximum number of requests allowed per client IP within the rate-limit window
    /// (env: `CDD__RATE_LIMIT_MAX_REQUESTS`).
    #[serde(default = "default_rate_limit_max_requests")]
    pub rate_limit_max_requests: usize,
    /// Duration of the rate-limit sliding window in seconds (env: `CDD__RATE_LIMIT_WINDOW_SECS`).
    #[serde(default = "default_rate_limit_window_secs")]
    pub rate_limit_window_secs: u64,
    /// Execution mode (`native` or `wasm`) for JSON-RPC target execution
    /// (env: `CDD__EXECUTION_MODE`).
    #[serde(default)]
    pub execution_mode: ExecutionMode,
    /// When `true` the server starts without a `PostgreSQL` connection and uses
    /// an in-memory repository instead (env: `CDD__OFFLINE_MODE`).
    #[serde(default)]
    pub offline_mode: bool,
    /// URL for the control-plane backend (env: `CDD__CONTROL_PLANE_URL`).
    pub control_plane_url: String,
    /// URL for the docs-ui frontend (env: `CDD__DOCS_UI_URL`).
    pub docs_ui_url: String,
    /// URL for the web-ui frontend (env: `CDD__WEB_UI_URL`).
    pub web_ui_url: String,
    /// Child-process configuration keyed by tool name.
    #[serde(default)]
    pub servers: HashMap<String, ProcessConfig>,
}

/// Returns the default GitHub OAuth client ID.
fn default_github_client_id() -> String {
    "github_client_id".to_string()
}

/// Returns the default GitHub OAuth client secret.
fn default_github_client_secret() -> String {
    "github_client_secret".to_string()
}

/// Returns the default GitHub REST API base URL.
fn default_github_api_base_url() -> String {
    "https://api.github.com".to_string()
}

/// Returns the default GitHub HTML base URL.
fn default_github_html_base_url() -> String {
    "https://github.com".to_string()
}

/// Returns the default Google OAuth 2.0 client ID.
fn default_google_client_id() -> String {
    "google_client_id".to_string()
}

/// Returns the default Google OAuth 2.0 client secret.
fn default_google_client_secret() -> String {
    "google_client_secret".to_string()
}

/// Returns the default Google OAuth 2.0 token endpoint URL.
fn default_google_token_url() -> String {
    "https://oauth2.googleapis.com/token".to_string()
}

/// Returns the default Google OAuth 2.0 `UserInfo` endpoint URL.
fn default_google_userinfo_url() -> String {
    "https://www.googleapis.com/oauth2/v3/userinfo".to_string()
}

/// Returns the default maximum requests per rate-limit window.
const fn default_rate_limit_max_requests() -> usize {
    100
}

/// Returns the default rate-limit window duration in seconds.
const fn default_rate_limit_window_secs() -> u64 {
    60
}

impl Default for AppConfig {
    /// Creates an [`AppConfig`] populated with local development defaults.
    ///
    /// # Returns
    /// * [`AppConfig`] - Default configuration instance.
    fn default() -> Self {
        Self {
            database_url: "postgres://postgres:password@localhost/cdd".to_string(),
            server_bind: "0.0.0.0:8080".to_string(),
            jwt_secret: "super-secret-key".to_string(),
            webhook_secret: "my_webhook_secret".to_string(),
            github_token: Some("system_token".to_string()),
            github_client_id: default_github_client_id(),
            github_client_secret: default_github_client_secret(),
            github_api_base_url: default_github_api_base_url(),
            github_html_base_url: default_github_html_base_url(),
            google_client_id: default_google_client_id(),
            google_client_secret: default_google_client_secret(),
            google_token_url: default_google_token_url(),
            google_userinfo_url: default_google_userinfo_url(),
            rate_limit_max_requests: default_rate_limit_max_requests(),
            rate_limit_window_secs: default_rate_limit_window_secs(),
            execution_mode: ExecutionMode::Native,
            offline_mode: false,
            control_plane_url: "http://localhost:8081".to_string(),
            docs_ui_url: "http://localhost:8082".to_string(),
            web_ui_url: "http://localhost:8083".to_string(),
            servers: HashMap::new(),
        }
    }
}

impl AppConfig {
    /// Load configuration from an optional file path and environment variables.
    ///
    /// Precedence (highest → lowest):
    /// 1. Environment variables (`CDD__*`)
    /// 2. Config file (if `config_path` is `Some`)
    /// 3. Built-in defaults
    ///
    /// # Arguments
    /// * `config_path` - Optional filesystem path to a configuration file (e.g., TOML/JSON).
    ///
    /// # Returns
    /// * `Ok(AppConfig)` - The resolved and deserialized configuration.
    ///
    /// # Errors
    /// * [`crate::error::CddGatewayError::Config`] - If building or deserializing the configuration fails.
    pub fn load(config_path: Option<&str>) -> Result<Self, crate::error::CddGatewayError> {
        let mut builder = config::Config::builder()
            .set_default("database_url", "postgres://postgres:password@localhost/cdd")?
            .set_default("server_bind", "0.0.0.0:8080")?
            .set_default("jwt_secret", "super-secret-key")?
            .set_default("webhook_secret", "my_webhook_secret")?
            .set_default("github_client_id", "github_client_id")?
            .set_default("github_client_secret", "github_client_secret")?
            .set_default("github_api_base_url", "https://api.github.com")?
            .set_default("github_html_base_url", "https://github.com")?
            .set_default("google_client_id", "google_client_id")?
            .set_default("google_client_secret", "google_client_secret")?
            .set_default("google_token_url", "https://oauth2.googleapis.com/token")?
            .set_default("google_userinfo_url", default_google_userinfo_url())?
            .set_default("rate_limit_max_requests", 100_i64)?
            .set_default("rate_limit_window_secs", 60_i64)?
            .set_default("execution_mode", "native")?
            .set_default("offline_mode", false)?
            .set_default("control_plane_url", "http://localhost:8081")?
            .set_default("docs_ui_url", "http://localhost:8082")?
            .set_default("web_ui_url", "http://localhost:8083")?;

        if let Some(path) = config_path {
            builder = builder.add_source(config::File::with_name(path).required(false));
        }

        Ok(builder
            .add_source(config::Environment::with_prefix("CDD").separator("__"))
            .build()?
            .try_deserialize()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::CddGatewayError;
    use std::sync::Mutex;

    static ENV_MUTEX: std::sync::LazyLock<Mutex<()>> = std::sync::LazyLock::new(|| Mutex::new(()));

    #[test]
    fn test_config_env_overrides() -> Result<(), CddGatewayError> {
        let _lock = ENV_MUTEX
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        std::env::remove_var("CDD__JWT_SECRET");
        std::env::remove_var("CDD__WEBHOOK_SECRET");
        std::env::remove_var("CDD__GITHUB_TOKEN");
        std::env::remove_var("CDD__OFFLINE_MODE");
        std::env::remove_var("CDD__EXECUTION_MODE");

        let cfg = AppConfig::load(None)?;
        assert_eq!(cfg.server_bind, "0.0.0.0:8080");
        assert_eq!(
            cfg.database_url,
            "postgres://postgres:password@localhost/cdd"
        );
        assert_eq!(cfg.jwt_secret, "super-secret-key");
        assert_eq!(cfg.webhook_secret, "my_webhook_secret");
        assert!(cfg.github_token.is_none());
        assert_eq!(cfg.execution_mode, ExecutionMode::Native);
        assert!(!cfg.offline_mode);

        std::env::set_var("CDD__JWT_SECRET", "test-jwt-secret");
        let cfg = AppConfig::load(None)?;
        assert_eq!(cfg.jwt_secret, "test-jwt-secret");
        std::env::remove_var("CDD__JWT_SECRET");

        std::env::set_var("CDD__WEBHOOK_SECRET", "test-webhook-secret");
        let cfg = AppConfig::load(None)?;
        assert_eq!(cfg.webhook_secret, "test-webhook-secret");
        std::env::remove_var("CDD__WEBHOOK_SECRET");

        std::env::set_var("CDD__GITHUB_TOKEN", "ghp_test123");
        let cfg = AppConfig::load(None)?;
        assert_eq!(cfg.github_token.as_deref(), Some("ghp_test123"));
        std::env::remove_var("CDD__GITHUB_TOKEN");

        std::env::set_var("CDD__OFFLINE_MODE", "true");
        let cfg = AppConfig::load(None)?;
        assert!(cfg.offline_mode);
        std::env::remove_var("CDD__OFFLINE_MODE");

        std::env::set_var("CDD__EXECUTION_MODE", "wasm");
        let cfg = AppConfig::load(None)?;
        assert_eq!(cfg.execution_mode, ExecutionMode::Wasm);
        std::env::remove_var("CDD__EXECUTION_MODE");

        Ok(())
    }

    #[test]
    fn test_config_load_with_file_path() -> Result<(), CddGatewayError> {
        use std::io::Write;
        let _lock = ENV_MUTEX
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let file_path = std::env::temp_dir().join(format!("cdd_cfg_{}.toml", uuid::Uuid::new_v4()));
        let mut file = std::fs::File::create(&file_path)?;
        writeln!(file, r#"server_bind = "127.0.0.1:9090""#)?;

        let path_str = file_path.to_string_lossy();
        let config = AppConfig::load(Some(&path_str))?;
        assert_eq!(config.server_bind, "127.0.0.1:9090");

        let _ = std::fs::remove_file(&file_path);

        Ok(())
    }

    #[test]
    fn test_config_load_deserialize_error() {
        let _lock = ENV_MUTEX
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        std::env::set_var("CDD__OFFLINE_MODE", "not_a_bool");
        let result = AppConfig::load(None);
        assert!(result.is_err());
        std::env::remove_var("CDD__OFFLINE_MODE");
    }

    #[test]
    fn test_config_derives() -> Result<(), CddGatewayError> {
        let _lock = ENV_MUTEX
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let cfg = AppConfig::load(None)?;

        let cloned = cfg.clone();
        assert_eq!(cfg.server_bind, cloned.server_bind);

        let dbg = format!("{cfg:?}");
        assert!(dbg.contains("AppConfig"));

        let ser = serde_json::to_string(&cfg)?;
        assert!(ser.contains("0.0.0.0:8080"));

        let de: AppConfig = serde_json::from_str(&ser)?;
        assert_eq!(de.server_bind, "0.0.0.0:8080");

        let minimal_json = r#"{"database_url":"db","server_bind":"sb","jwt_secret":"js","webhook_secret":"ws","control_plane_url":"cp","docs_ui_url":"du","web_ui_url":"wu"}"#;
        let de_min: AppConfig = serde_json::from_str(minimal_json)?;
        assert_eq!(de_min.google_client_id, "google_client_id");
        assert_eq!(de_min.google_client_secret, "google_client_secret");
        assert_eq!(
            de_min.google_token_url,
            "https://oauth2.googleapis.com/token"
        );
        assert_eq!(
            de_min.google_userinfo_url,
            "https://www.googleapis.com/oauth2/v3/userinfo"
        );

        Ok(())
    }

    #[test]
    fn test_default() {
        let cfg = AppConfig::default();
        assert_eq!(cfg.server_bind, "0.0.0.0:8080");
        assert_eq!(
            cfg.database_url,
            "postgres://postgres:password@localhost/cdd"
        );
        assert_eq!(cfg.jwt_secret, "super-secret-key");
        assert_eq!(cfg.webhook_secret, "my_webhook_secret");
        assert_eq!(cfg.github_client_id, "github_client_id");
        assert_eq!(cfg.github_client_secret, "github_client_secret");
        assert_eq!(cfg.github_api_base_url, "https://api.github.com");
        assert_eq!(cfg.github_html_base_url, "https://github.com");
        assert_eq!(cfg.google_client_id, "google_client_id");
        assert_eq!(cfg.google_client_secret, "google_client_secret");
        assert_eq!(cfg.google_token_url, "https://oauth2.googleapis.com/token");
        assert_eq!(
            cfg.google_userinfo_url,
            "https://www.googleapis.com/oauth2/v3/userinfo"
        );
        assert_eq!(cfg.rate_limit_max_requests, 100);
        assert_eq!(cfg.rate_limit_window_secs, 60);
        assert_eq!(cfg.execution_mode, ExecutionMode::Native);
        assert!(!cfg.offline_mode);
    }
}
