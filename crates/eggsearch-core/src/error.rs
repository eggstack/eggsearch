//! Error types for eggsearch-core.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("invalid URL: {0}")]
    InvalidUrl(String),

    #[error("invalid query: {0}")]
    InvalidQuery(String),

    #[error("config error: {0}")]
    Config(String),

    #[error("provider '{provider}' failed: {message}")]
    Provider { provider: String, message: String },

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("toml error: {0}")]
    Toml(#[from] toml::de::Error),

    #[error("config crate error: {0}")]
    ConfigCrate(#[from] config::ConfigError),

    #[error("{0}")]
    Other(String),
}

pub type CoreResult<T> = Result<T, CoreError>;
