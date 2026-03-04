use crate::daemon::ProcessConfig;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Deserialize, Serialize)]
pub struct AppConfig {
    pub database_url: String,
    pub server_bind: String,
    #[serde(default)]
    pub servers: HashMap<String, ProcessConfig>,
}

impl AppConfig {
    pub fn load(config_path: Option<&str>) -> Result<Self, config::ConfigError> {
        let mut builder = config::Config::builder()
            .set_default("database_url", "postgres://postgres:password@localhost/cdd")?
            .set_default("server_bind", "0.0.0.0:8080")?;

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
    }
}
