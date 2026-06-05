use thiserror::Error;

#[derive(Debug, Error)]
pub enum EngineError {
    #[error("engine '{engine}' timed out")]
    Timeout { engine: &'static str },

    #[error("engine '{engine}' HTTP error: {source}")]
    Http {
        engine: &'static str,
        #[source]
        source: reqwest::Error,
    },

    #[error("engine '{engine}' returned status {status}")]
    BadStatus { engine: &'static str, status: u16 },

    #[error("engine '{engine}' parse failed: {reason}")]
    ParseFailed {
        engine: &'static str,
        reason: String,
    },
}
