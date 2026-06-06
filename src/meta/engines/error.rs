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

    #[error("engine '{engine}' network error: {reason}")]
    NetworkError {
        engine: &'static str,
        reason: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timeout_message() {
        let err = EngineError::Timeout {
            engine: "duckduckgo",
        };
        assert_eq!(err.to_string(), "engine 'duckduckgo' timed out");
    }

    #[test]
    fn bad_status_message() {
        let err = EngineError::BadStatus {
            engine: "brave",
            status: 429,
        };
        assert_eq!(err.to_string(), "engine 'brave' returned status 429");
    }

    #[test]
    fn parse_failed_message() {
        let err = EngineError::ParseFailed {
            engine: "startpage",
            reason: "missing selector".to_string(),
        };
        let s = err.to_string();
        assert!(s.contains("engine 'startpage'"));
        assert!(s.contains("parse failed"));
        assert!(s.contains("missing selector"));
    }

    #[test]
    fn network_error_message() {
        let err = EngineError::NetworkError {
            engine: "yahoo",
            reason: "connection refused".to_string(),
        };
        let s = err.to_string();
        assert!(s.contains("engine 'yahoo'"));
        assert!(s.contains("network error"));
        assert!(s.contains("connection refused"));
    }
}
