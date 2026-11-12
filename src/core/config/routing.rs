//! Intent-aware provider routing configuration.

use serde::{Deserialize, Serialize};

use crate::core::error::{CoreError, CoreResult};

/// Maximum `max_extra_providers` value the validator accepts.
pub const INTENT_ROUTING_MAX_EXTRA_PROVIDERS: usize = 4;

/// Whether the default routing path appends intent-appropriate specialist
/// providers to the candidates it already resolved.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum IntentRoutingMode {
    /// Report the intent but never change the routed provider set. Default.
    #[default]
    Off,
    /// Append capability-valid providers for the effective intent.
    Apply,
}

impl<'de> Deserialize<'de> for IntentRoutingMode {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Self::from_alias(&raw).ok_or_else(|| {
            serde::de::Error::custom(format!(
                "invalid intent_mode `{raw}`; valid values: off, apply"
            ))
        })
    }
}

impl IntentRoutingMode {
    /// Stable lowercase string form.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Apply => "apply",
        }
    }

    fn from_alias(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "off" | "observe_only" | "disabled" => Some(Self::Off),
            "apply" | "on" | "enabled" => Some(Self::Apply),
            _ => None,
        }
    }
}

/// The `[search.routing]` section of the eggsearch configuration file.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RoutingSection {
    /// Intent routing mode: `off` or `apply`. Default: `off`.
    #[serde(default = "default_intent_routing_mode")]
    pub intent_mode: IntentRoutingMode,
    /// Maximum extra providers intent routing may append. Default: 2,
    /// bounded 0..=4; `0` reports the intent and the would-be additions
    /// but adds nothing.
    #[serde(default = "default_max_extra_providers")]
    pub max_extra_providers: usize,
}

impl Default for RoutingSection {
    fn default() -> Self {
        Self {
            intent_mode: default_intent_routing_mode(),
            max_extra_providers: default_max_extra_providers(),
        }
    }
}

impl RoutingSection {
    pub(super) fn validate(&self) -> CoreResult<()> {
        if self.max_extra_providers > INTENT_ROUTING_MAX_EXTRA_PROVIDERS {
            return Err(CoreError::Config(format!(
                "[search.routing].max_extra_providers ({}) must be between 0 and {INTENT_ROUTING_MAX_EXTRA_PROVIDERS}",
                self.max_extra_providers
            )));
        }
        Ok(())
    }
}

pub(super) fn default_intent_routing_mode() -> IntentRoutingMode {
    IntentRoutingMode::Off
}

pub(super) fn default_max_extra_providers() -> usize {
    2
}
