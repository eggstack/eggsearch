//! Result-level types: trust labels and per-provider warnings.

use serde::{Deserialize, Serialize};

/// Trust label attached to every `SourceCard`. For the MVP, all live web
/// results are `ExternalUntrusted`. Local-index results (feature-gated
/// behind `local-index`) may be `LocalTrusted`.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TrustLevel {
    /// Live content fetched from the open web; treat as untrusted.
    #[default]
    ExternalUntrusted,
    /// Content originated from a configured local source (trusted by the operator).
    LocalTrusted,
    /// Trust level is unknown; default to caution.
    Unknown,
}

impl TrustLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ExternalUntrusted => "external_untrusted",
            Self::LocalTrusted => "local_trusted",
            Self::Unknown => "unknown",
        }
    }
}

/// A warning emitted by a single provider during a search. Provider
/// failures are non-fatal: they are surfaced as warnings, not raised as
/// errors.
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SearchWarning {
    pub provider_id: String,
    pub message: String,
}

impl SearchWarning {
    pub fn new(provider_id: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            provider_id: provider_id.into(),
            message: message.into(),
        }
    }
}
