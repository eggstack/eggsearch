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

    #[error("toml parse error: {0}")]
    TomlDe(#[from] toml::de::Error),

    #[error("toml serialization error: {0}")]
    TomlSer(String),

    #[error("{0}")]
    Other(String),
}

pub type CoreResult<T> = Result<T, CoreError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_url_message() {
        let err = CoreError::InvalidUrl("not a url".to_string());
        assert!(err.to_string().contains("invalid URL"));
        assert!(err.to_string().contains("not a url"));
    }

    #[test]
    fn invalid_query_message() {
        let err = CoreError::InvalidQuery("query too long".to_string());
        assert!(err.to_string().contains("invalid query"));
        assert!(err.to_string().contains("query too long"));
    }

    #[test]
    fn config_error_message() {
        let err = CoreError::Config("missing field".to_string());
        assert!(err.to_string().contains("config error"));
        assert!(err.to_string().contains("missing field"));
    }

    #[test]
    fn provider_error_message() {
        let err = CoreError::Provider {
            provider: "duckduckgo".to_string(),
            message: "timeout".to_string(),
        };
        let s = err.to_string();
        assert!(s.contains("provider 'duckduckgo'"));
        assert!(s.contains("timeout"));
    }

    #[test]
    fn io_error_from_std() {
        use std::io;
        let err = CoreError::Io(io::Error::new(io::ErrorKind::NotFound, "file not found"));
        assert!(err.to_string().contains("io error"));
    }

    #[test]
    fn serde_error_from_json() {
        let json_err = serde_json::from_str::<()>("invalid json").unwrap_err();
        let err = CoreError::Serde(json_err);
        assert!(err.to_string().contains("serialization error"));
    }

    #[test]
    fn toml_de_error_from_invalid_toml() {
        let toml_err = toml::from_str::<()>("invalid = toml").unwrap_err();
        let err = CoreError::TomlDe(toml_err);
        assert!(err.to_string().contains("toml parse error"));
    }

    #[test]
    fn toml_ser_error_message() {
        let err = CoreError::TomlSer("serialization failed".to_string());
        assert!(err.to_string().contains("toml serialization error"));
    }

    #[test]
    fn other_error_message() {
        let err = CoreError::Other("something went wrong".to_string());
        assert!(err.to_string().contains("something went wrong"));
    }
}
