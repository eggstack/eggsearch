//! Serializable provider health view types shared by diagnostics and
//! provider status output.

use serde::{Deserialize, Serialize};

/// Health status for a provider, derived from the health entry.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderHealthStatus {
    /// No recent failures; provider is operating normally.
    Healthy,
    /// Some recent failures but not in cooldown.
    Degraded,
    /// Provider is in cooldown after repeated failures.
    Cooldown,
    /// No health data recorded yet.
    Unknown,
}

/// A compact per-provider health view embedded in provider descriptors.
///
/// Distinct from [`crate::meta::ProviderHealthSnapshot`] which mixes config
/// state (enabled, configured) with health. This view is purely health-focused
/// and designed to be embedded in provider status output.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderHealthView {
    /// Derived health status.
    pub status: ProviderHealthStatus,
    /// Number of consecutive failures (0 if last call succeeded).
    pub consecutive_failures: u32,
    /// Error class of the most recent failure, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error_class: Option<String>,
    /// Bounded human-readable message of the most recent failure.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error_message: Option<String>,
    /// When the provider will exit cooldown, if in cooldown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cooldown_until: Option<String>,
    /// Reason for the current cooldown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cooldown_reason: Option<String>,
    /// Latency of the most recent call in milliseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_latency_ms: Option<u64>,
    /// When the last successful call occurred (ISO 8601 / RFC 3339).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_success_at: Option<String>,
    /// When the last failed call occurred (ISO 8601 / RFC 3339).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_failure_at: Option<String>,
    /// Most recent shape-bearing parse outcome, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parse_shape: Option<ParseShape>,
}

/// Most recent shape-bearing provider outcome.
///
/// Diagnostic only: `ok` means the provider returned rows, `empty` means it
/// returned success with zero results (including a documented empty-state
/// marker), and `shifted` means it reported a structural parse failure.
/// Providers with no shape-bearing observation omit the value.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParseShape {
    /// The provider returned at least one row.
    Ok,
    /// The provider returned success with zero results.
    Empty,
    /// The provider reported a structural parse failure.
    Shifted,
}

impl ParseShape {
    /// Stable snake-case string form.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Empty => "empty",
            Self::Shifted => "shifted",
        }
    }
}
