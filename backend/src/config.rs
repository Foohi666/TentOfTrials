use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::env;
use std::path::Path;
use thiserror::Error;

const DEFAULT_BACKEND_HOST: &str = "0.0.0.0";
const DEFAULT_BACKEND_PORT: u16 = 8080;
const DEFAULT_LOG_LEVEL: &str = "info";
const DEFAULT_ENABLE_EXPERIMENTAL: bool = false;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub log_level: String,
    pub enable_experimental: bool,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ConfigError {
    #[error("{name} must not be empty")]
    Empty { name: &'static str },
    #[error("{name} must be a valid TCP port between 1 and 65535, got {value:?}")]
    InvalidPort { name: &'static str, value: String },
    #[error("{name} must be one of true, false, 1, 0, yes, no, on, or off, got {value:?}")]
    InvalidBool { name: &'static str, value: String },
    #[error("{name} must be one of trace, debug, info, warn, or error, got {value:?}")]
    InvalidLogLevel { name: &'static str, value: String },
}

impl Default for Config {
    fn default() -> Self {
        Self {
            host: DEFAULT_BACKEND_HOST.into(),
            port: DEFAULT_BACKEND_PORT,
            log_level: DEFAULT_LOG_LEVEL.into(),
            enable_experimental: DEFAULT_ENABLE_EXPERIMENTAL,
        }
    }
}

impl Config {
    pub fn from_env() -> std::result::Result<Self, ConfigError> {
        Self::from_env_reader(|name| env::var(name).ok())
    }

    fn from_env_reader<F>(read: F) -> std::result::Result<Self, ConfigError>
    where
        F: Fn(&'static str) -> Option<String>,
    {
        let defaults = Self::default();
        let host = match read("TOT_BACKEND_HOST") {
            Some(value) => parse_non_empty("TOT_BACKEND_HOST", value)?,
            None => defaults.host,
        };
        let port = match read("TOT_BACKEND_PORT") {
            Some(value) => parse_port("TOT_BACKEND_PORT", value)?,
            None => defaults.port,
        };
        let log_level = match read("TOT_LOG_LEVEL") {
            Some(value) => parse_log_level("TOT_LOG_LEVEL", value)?,
            None => defaults.log_level,
        };
        let enable_experimental = match read("TOT_ENABLE_EXPERIMENTAL") {
            Some(value) => parse_bool("TOT_ENABLE_EXPERIMENTAL", value)?,
            None => defaults.enable_experimental,
        };

        Ok(Self {
            host,
            port,
            log_level,
            enable_experimental,
        })
    }
}

fn parse_non_empty(name: &'static str, value: String) -> std::result::Result<String, ConfigError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(ConfigError::Empty { name });
    }
    Ok(trimmed.to_owned())
}

fn parse_port(name: &'static str, value: String) -> std::result::Result<u16, ConfigError> {
    let parsed = value
        .trim()
        .parse::<u16>()
        .map_err(|_| ConfigError::InvalidPort {
            name,
            value: value.clone(),
        })?;
    if parsed == 0 {
        return Err(ConfigError::InvalidPort { name, value });
    }
    Ok(parsed)
}

fn parse_bool(name: &'static str, value: String) -> std::result::Result<bool, ConfigError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Ok(true),
        "false" | "0" | "no" | "off" => Ok(false),
        _ => Err(ConfigError::InvalidBool { name, value }),
    }
}

fn parse_log_level(name: &'static str, value: String) -> std::result::Result<String, ConfigError> {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "trace" | "debug" | "info" | "warn" | "error" => Ok(normalized),
        _ => Err(ConfigError::InvalidLogLevel { name, value }),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceConfig {
    pub name: String,
    pub version: String,
    pub host: String,
    pub port: u16,
    pub tls_enabled: bool,
    pub tls_cert_path: Option<String>,
    pub tls_key_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryConfig {
    pub backend: String,
    pub endpoints: Vec<String>,
    pub heartbeat_interval_ms: u64,
    pub ttl_seconds: u64,
    pub replication_factor: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryConfig {
    pub provider: String,
    pub namespace: String,
    pub tags: Vec<String>,
    pub health_check_path: String,
    pub health_check_interval_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessagingConfig {
    pub broker_type: String,
    pub uris: Vec<String>,
    pub consumer_group: String,
    pub max_retries: u32,
    pub retry_backoff_ms: u64,
    pub batch_size: u32,
    pub compression: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RootConfig {
    pub service: ServiceConfig,
    pub registry: RegistryConfig,
    pub discovery: DiscoveryConfig,
    pub messaging: MessagingConfig,
}

impl Default for RootConfig {
    fn default() -> Self {
        Self {
            service: ServiceConfig {
                name: "tent-backend".into(),
                version: "0.1.0".into(),
                host: DEFAULT_BACKEND_HOST.into(),
                port: DEFAULT_BACKEND_PORT,
                tls_enabled: false,
                tls_cert_path: None,
                tls_key_path: None,
            },
            registry: RegistryConfig {
                backend: "etcd".into(),
                endpoints: vec!["localhost:2379".into()],
                heartbeat_interval_ms: 5000,
                ttl_seconds: 30,
                replication_factor: 3,
            },
            discovery: DiscoveryConfig {
                provider: "consul".into(),
                namespace: "tent".into(),
                tags: vec!["microservice".into(), "orchestration".into()],
                health_check_path: "/health".into(),
                health_check_interval_ms: 10000,
            },
            messaging: MessagingConfig {
                broker_type: "kafka".into(),
                uris: vec!["localhost:9092".into()],
                consumer_group: "tent-consumers".into(),
                max_retries: 3,
                retry_backoff_ms: 1000,
                batch_size: 500,
                compression: "snappy".into(),
            },
        }
    }
}

pub async fn load_config(path: &str) -> Result<RootConfig> {
    let path = Path::new(path);
    if path.exists() {
        let contents = tokio::fs::read_to_string(path).await?;
        let config: RootConfig = toml::from_str(&contents)?;
        tracing::info!("configuration loaded from {}", path.display());
        Ok(config)
    } else {
        tracing::warn!("config file {} not found, using defaults", path.display());
        Ok(RootConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn read_env(
        vars: HashMap<&'static str, &'static str>,
    ) -> impl Fn(&'static str) -> Option<String> {
        move |name| vars.get(name).map(|value| (*value).to_owned())
    }

    #[test]
    fn from_env_uses_safe_defaults() {
        let config = Config::from_env_reader(read_env(HashMap::new())).unwrap();

        assert_eq!(config.host, "0.0.0.0");
        assert_eq!(config.port, 8080);
        assert_eq!(config.log_level, "info");
        assert!(!config.enable_experimental);
    }

    #[test]
    fn from_env_accepts_valid_overrides() {
        let config = Config::from_env_reader(read_env(HashMap::from([
            ("TOT_BACKEND_HOST", "127.0.0.1"),
            ("TOT_BACKEND_PORT", "9090"),
            ("TOT_LOG_LEVEL", "debug"),
            ("TOT_ENABLE_EXPERIMENTAL", "yes"),
        ])))
        .unwrap();

        assert_eq!(config.host, "127.0.0.1");
        assert_eq!(config.port, 9090);
        assert_eq!(config.log_level, "debug");
        assert!(config.enable_experimental);
    }

    #[test]
    fn from_env_rejects_invalid_ports() {
        let error =
            Config::from_env_reader(read_env(HashMap::from([("TOT_BACKEND_PORT", "70000")])))
                .unwrap_err();

        assert_eq!(
            error,
            ConfigError::InvalidPort {
                name: "TOT_BACKEND_PORT",
                value: "70000".into()
            }
        );
    }

    #[test]
    fn from_env_rejects_invalid_boolean_values() {
        let error = Config::from_env_reader(read_env(HashMap::from([(
            "TOT_ENABLE_EXPERIMENTAL",
            "maybe",
        )])))
        .unwrap_err();

        assert_eq!(
            error,
            ConfigError::InvalidBool {
                name: "TOT_ENABLE_EXPERIMENTAL",
                value: "maybe".into()
            }
        );
    }
}
