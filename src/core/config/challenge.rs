//! Bounded challenge-solving configuration.

use serde::{Deserialize, Serialize};

use crate::core::error::{CoreError, CoreResult};

const MAX_DIFFICULTY_LIMIT: u8 = 10;
const MAX_ATTEMPTS_LIMIT: u64 = 100_000_000;
const TIMEOUT_MS_MIN: u64 = 100;
const TIMEOUT_MS_MAX: u64 = 30_000;
const COOKIE_TTL_MAX_SECONDS: u64 = 3600;

/// Bounded, opt-in challenge-solving configuration.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChallengeSection {
    /// Whether computational challenges may be solved. Default: false.
    #[serde(default)]
    pub enabled: bool,
    /// Difficulty ceiling for schemes that publish one. Default: 5, bounded 1..=10.
    #[serde(default = "default_challenge_max_difficulty")]
    pub max_difficulty: u8,
    /// Candidate/derivation ceiling per solve. Default: 2_000_000, bounded 1..=100_000_000.
    #[serde(default = "default_challenge_max_attempts")]
    pub max_attempts: u64,
    /// Wall-clock ceiling per solve. Default: 2000 ms, bounded 100..=30000.
    #[serde(default = "default_challenge_timeout_ms")]
    pub timeout_ms: u64,
    /// Pass-cookie reuse window per provider. Default: 240 s, bounded 0..=3600; `0` disables reuse.
    #[serde(default = "default_challenge_cookie_ttl_seconds")]
    pub cookie_ttl_seconds: u64,
}

impl Default for ChallengeSection {
    fn default() -> Self {
        Self {
            enabled: false,
            max_difficulty: default_challenge_max_difficulty(),
            max_attempts: default_challenge_max_attempts(),
            timeout_ms: default_challenge_timeout_ms(),
            cookie_ttl_seconds: default_challenge_cookie_ttl_seconds(),
        }
    }
}

impl ChallengeSection {
    pub(super) fn validate(&self) -> CoreResult<()> {
        for (name, value, min, max) in [
            (
                "max_difficulty",
                u64::from(self.max_difficulty),
                1,
                u64::from(MAX_DIFFICULTY_LIMIT),
            ),
            ("max_attempts", self.max_attempts, 1, MAX_ATTEMPTS_LIMIT),
            (
                "timeout_ms",
                self.timeout_ms,
                TIMEOUT_MS_MIN,
                TIMEOUT_MS_MAX,
            ),
        ] {
            if !(min..=max).contains(&value) {
                return Err(CoreError::Config(format!(
                    "[search.challenge].{name} ({value}) must be between {min} and {max}"
                )));
            }
        }
        if self.cookie_ttl_seconds > COOKIE_TTL_MAX_SECONDS {
            return Err(CoreError::Config(format!(
                "[search.challenge].cookie_ttl_seconds ({}) must be <= {COOKIE_TTL_MAX_SECONDS}; 0 disables reuse",
                self.cookie_ttl_seconds
            )));
        }
        Ok(())
    }
}

fn default_challenge_max_difficulty() -> u8 {
    5
}
fn default_challenge_max_attempts() -> u64 {
    2_000_000
}
fn default_challenge_timeout_ms() -> u64 {
    2000
}
fn default_challenge_cookie_ttl_seconds() -> u64 {
    240
}
