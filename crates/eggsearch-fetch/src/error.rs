//! Fetch error types.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum FetchError {
    #[error("invalid URL: {0}")]
    InvalidUrl(String),

    #[error("network error: {0}")]
    Network(String),

    #[error("upstream returned status {0}")]
    BadStatus(u16),

    #[error("response too large (limit {limit} bytes)")]
    TooLarge { limit: usize },

    #[error("timed out after {0} ms")]
    Timeout(u64),

    #[error("robots policy disallows fetching {0}")]
    RobotsDenied(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("extraction error: {0}")]
    Extract(String),

    #[error("io::json error: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("{0}")]
    Other(String),
}

pub type FetchResult<T> = Result<T, FetchError>;
