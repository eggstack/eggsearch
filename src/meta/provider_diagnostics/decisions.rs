//! Provider routing decisions: explicit, profile, default, and intent-aware.

use serde::{Deserialize, Serialize};

use crate::core::provider::{ProviderSkipCode, KNOWN_PROVIDER_IDS};
use crate::core::repo_search::SearchProfile;

use super::ProviderHealthRegistry;

/// A skip reason for a single provider in the routing decision.
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ProviderSkipReason {
    /// The provider id that was skipped.
    pub provider_id: String,
    /// Machine-actionable reason code for programmatic handling.
    /// Stable across versions — agents can match on these.
    ///
    /// Known codes: `"cooldown"`, `"not_built"`, `"unknown"`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub reason_code: String,
    /// Human-readable reason for skipping.
    pub reason: String,
    /// The failure class if skipped due to a recent failure.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure_class: Option<String>,
    /// When cooldown expires, if skipped due to cooldown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cooldown_until: Option<String>,
    /// Machine-actionable skip code for programmatic handling.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skip_code: Option<ProviderSkipCode>,
}

/// One provider considered by intent-aware routing and why it was or was not
/// added to the routed set.
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct IntentRoutingAttempt {
    /// The provider id under consideration.
    pub provider_id: String,
    /// Stable disposition code: `added`, `already_selected`, `not_enabled`,
    /// `not_built`, `cooling_down`, `keyword_unsupported`,
    /// `capability_missing`, or `cap_exceeded`.
    pub disposition: String,
}

/// The intent-routing report attached to a routing decision.
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ProviderIntentRoutingReport {
    /// The effective retrieval intent that drove selection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intent: Option<String>,
    /// The configured `[search.routing].intent_mode`.
    pub intent_mode: String,
    /// The configured `[search.routing].max_extra_providers`.
    pub max_extra_providers: usize,
    /// Providers actually appended to the routed set, in table order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub additions: Vec<String>,
    /// Capability-valid providers withheld only by `max_extra_providers`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub withheld: Vec<String>,
    /// Every provider the preference table considered, in table order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub considered: Vec<IntentRoutingAttempt>,
}

/// Result of provider routing resolution.
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ProviderRoutingDecision {
    /// The profile requested by the caller, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested_profile: Option<SearchProfile>,
    /// The explicit provider IDs requested by the caller.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requested_providers: Vec<String>,
    /// The providers actually selected for this request.
    pub selected_providers: Vec<String>,
    /// Providers that were skipped and why.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skipped_providers: Vec<ProviderSkipReason>,
    /// Whether the profile fell back to default providers.
    pub degraded: bool,
    /// Whether some providers were skipped but others remain active.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub partial: bool,
    /// Human-readable explanation of the routing decision.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// The intent-routing decision, present only for the default path when an
    /// intent was supplied.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intent_routing: Option<ProviderIntentRoutingReport>,
}

/// Error from provider routing resolution.
#[derive(Debug)]
pub enum ProviderRoutingError {
    /// An explicitly requested provider is not known.
    UnknownProvider(String),
    /// An explicitly requested provider is disabled.
    DisabledProvider(String),
    /// An explicitly requested provider is enabled and configured, but the
    /// environment variable holding its required credential is unset or empty.
    MissingCredential {
        /// The requested provider id.
        id: String,
        /// The credential environment variable the operator must populate.
        env: String,
    },
    /// An explicitly requested provider is enabled and configured, but this
    /// build has no search engine for it, so it can never be queried.
    NotBuilt(String),
    /// No default providers are available.
    NoDefaultProviders(String),
}

impl std::fmt::Display for ProviderRoutingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownProvider(id) => write!(f, "unknown provider id: {id}"),
            Self::DisabledProvider(id) => write!(f, "provider is disabled: {id}"),
            Self::MissingCredential { id, env } => write!(
                f,
                "provider {id} is enabled but its required credential is not set; populate the {env} environment variable"
            ),
            Self::NotBuilt(id) => write!(
                f,
                "provider {id} is enabled and configured but has no engine in this build, so it cannot be queried"
            ),
            Self::NoDefaultProviders(msg) => write!(f, "no default providers: {msg}"),
        }
    }
}

impl ProviderRoutingError {
    /// Stable skip code describing why an explicitly requested provider could
    /// not be routed, for typed downstream handling.
    pub fn skip_code(&self) -> ProviderSkipCode {
        match self {
            Self::UnknownProvider(_) => ProviderSkipCode::UnknownProvider,
            Self::DisabledProvider(_) => ProviderSkipCode::DisabledByUser,
            Self::MissingCredential { .. } => ProviderSkipCode::MissingApiKey,
            Self::NotBuilt(_) => ProviderSkipCode::NotBuilt,
            Self::NoDefaultProviders(_) => ProviderSkipCode::Unknown,
        }
    }
}

impl std::error::Error for ProviderRoutingError {}

/// Resolve provider routing with optional health-aware cooldown.
///
/// This consolidates the provider selection logic used by `web_search`,
/// `repo_search`, `security_search`, and `research_search`.
///
/// # Arguments
///
/// * `requested_providers` - Explicit provider IDs from the caller.
/// * `profile` - Optional search profile for provider selection.
/// * `adapter_provider_ids` - IDs of providers actually built by the adapter.
/// * `config` - Application configuration for provider resolution.
/// * `health` - Process-local health registry for cooldown decisions.
/// * `strict_explicit` - When true, explicit provider errors are hard failures.
/// * `intent` - Effective retrieval intent for the default path, if known.
pub fn resolve_provider_routing(
    requested_providers: &[String],
    profile: Option<SearchProfile>,
    adapter_provider_ids: &[String],
    config: &crate::core::config::AppConfig,
    health: &ProviderHealthRegistry,
    strict_explicit: bool,
    intent: Option<crate::core::query::SearchIntent>,
) -> Result<ProviderRoutingDecision, ProviderRoutingError> {
    let adapter_set: std::collections::BTreeSet<&str> =
        adapter_provider_ids.iter().map(|s| s.as_str()).collect();

    // Explicit providers are strict
    if !requested_providers.is_empty() {
        return resolve_explicit_providers(
            requested_providers,
            &adapter_set,
            config,
            strict_explicit,
        );
    }

    // Profile-based or default routing
    let (mut effective_providers, degraded, _warnings) =
        config.resolve_profile_providers(profile, &[]);

    let mut skipped = Vec::new();
    let mut partial = false;

    // Filter through actual adapter availability
    if let Some(profile) = profile {
        let mut filtered = Vec::new();
        let mut any_skipped = false;

        for id in &effective_providers {
            if adapter_set.contains(id.as_str()) {
                // Check cooldown — skip cooled-down providers only for profile/default routing
                if health.is_in_cooldown(id) {
                    let snapshot = health.snapshot(id, true, true);
                    skipped.push(ProviderSkipReason {
                        provider_id: id.clone(),
                        reason_code: "cooldown".to_string(),
                        reason: format!(
                            "in cooldown after {}",
                            snapshot.cooldown_reason.unwrap_or_default()
                        ),
                        failure_class: snapshot.recent_failure_class,
                        cooldown_until: snapshot.cooldown_until,
                        skip_code: Some(ProviderSkipCode::CooldownActive),
                    });
                    any_skipped = true;
                } else {
                    filtered.push(id.clone());
                }
            } else {
                skipped.push(ProviderSkipReason {
                    provider_id: id.clone(),
                    reason_code: "not_built".to_string(),
                    reason: format!(
                        "[{}] {}",
                        ProviderSkipCode::NotBuilt.as_str(),
                        ProviderSkipCode::NotBuilt.display_name()
                    ),
                    failure_class: None,
                    cooldown_until: None,
                    skip_code: Some(ProviderSkipCode::NotBuilt),
                });
                any_skipped = true;
            }
        }

        if filtered.is_empty() && !effective_providers.is_empty() {
            // All profile providers unavailable — degrade to defaults
            effective_providers = config
                .resolve_providers(&[])
                .map_err(|e| ProviderRoutingError::NoDefaultProviders(e.to_string()))?;
            return Ok(ProviderRoutingDecision {
                requested_profile: Some(profile),
                requested_providers: vec![],
                selected_providers: effective_providers,
                skipped_providers: skipped,
                degraded: true,
                partial: false,
                reason: Some(format!("{profile} profile fell back to default providers")),
                intent_routing: None,
            });
        }

        if any_skipped && !filtered.is_empty() {
            partial = true;
        }

        effective_providers = filtered;
    } else {
        let mut intent_routing: Option<ProviderIntentRoutingReport> = None;
        if let Some(intent) = intent {
            let settings = &config.search.routing;
            let enabled = config.effective_provider_ids();
            let cooling: Vec<String> = enabled
                .iter()
                .filter(|id| health.is_in_cooldown(id))
                .cloned()
                .collect();
            let attempts = if settings.intent_mode == crate::core::config::IntentRoutingMode::Apply
            {
                crate::meta::routing::select_intent_additions(
                    intent,
                    &effective_providers,
                    &enabled,
                    adapter_provider_ids,
                    &cooling,
                    settings.max_extra_providers,
                )
            } else {
                Vec::new()
            };
            for id in crate::meta::routing::added_ids(&attempts) {
                if !effective_providers.contains(&id) {
                    effective_providers.push(id);
                }
            }
            intent_routing = Some(ProviderIntentRoutingReport {
                intent: Some(intent.as_str().to_string()),
                intent_mode: settings.intent_mode.as_str().to_string(),
                max_extra_providers: settings.max_extra_providers,
                additions: crate::meta::routing::added_ids(&attempts),
                withheld: crate::meta::routing::withheld_ids(&attempts),
                considered: attempts
                    .iter()
                    .map(|attempt| IntentRoutingAttempt {
                        provider_id: attempt.provider_id.to_string(),
                        disposition: attempt.disposition.as_str().to_string(),
                    })
                    .collect(),
            });
        }

        // Default routing — also filter by adapter availability
        let mut filtered = Vec::new();
        for id in &effective_providers {
            if adapter_set.contains(id.as_str()) && !health.is_in_cooldown(id) {
                filtered.push(id.clone());
            } else if !adapter_set.contains(id.as_str()) {
                // Provider not built — skip silently for default routing
            } else {
                // In cooldown — skip for default routing
                let snapshot = health.snapshot(id, true, true);
                skipped.push(ProviderSkipReason {
                    provider_id: id.clone(),
                    reason_code: "cooldown".to_string(),
                    reason: format!(
                        "in cooldown after {}",
                        snapshot.cooldown_reason.unwrap_or_default()
                    ),
                    failure_class: snapshot.recent_failure_class,
                    cooldown_until: snapshot.cooldown_until,
                    skip_code: Some(ProviderSkipCode::CooldownActive),
                });
            }
        }
        if !filtered.is_empty() {
            effective_providers = filtered;
        }

        let reason = if degraded {
            Some("profile fell back to default providers".to_string())
        } else {
            profile.map(|p| format!("using {} profile providers", p.as_str()))
        };

        return Ok(ProviderRoutingDecision {
            requested_profile: profile,
            requested_providers: vec![],
            selected_providers: effective_providers,
            skipped_providers: skipped,
            degraded,
            partial,
            reason,
            intent_routing,
        });
    }

    let reason = if degraded {
        Some("profile fell back to default providers".to_string())
    } else if partial {
        Some(format!(
            "{:?} profile skipped unavailable providers",
            profile.unwrap_or_default()
        ))
    } else {
        profile.map(|p| format!("using {} profile providers", p.as_str()))
    };

    Ok(ProviderRoutingDecision {
        requested_profile: profile,
        requested_providers: vec![],
        selected_providers: effective_providers,
        skipped_providers: skipped,
        degraded,
        partial,
        reason,
        intent_routing: None,
    })
}

/// Handle explicit provider resolution.
fn resolve_explicit_providers(
    requested_providers: &[String],
    adapter_set: &std::collections::BTreeSet<&str>,
    config: &crate::core::config::AppConfig,
    strict: bool,
) -> Result<ProviderRoutingDecision, ProviderRoutingError> {
    // Deduplicate while preserving order
    let mut seen = std::collections::HashSet::new();
    let mut deduped = Vec::new();
    for id in requested_providers {
        if seen.insert(id.clone()) {
            deduped.push(id.clone());
        }
    }

    // Validate every requested id against three distinct failure classes, so an
    // explicit request can never be silently narrowed to a different provider
    // set than the caller asked for:
    //
    //   1. not a known provider id at all;
    //   2. known, but the operator has not enabled it (or it needs a
    //      credential that is configured-but-unset);
    //   3. known and enabled, but this build has no engine for it.
    //
    // In strict mode each class is a hard, typed error. In non-strict mode each
    // class is recorded as a typed skip so the decision still explains itself.
    let mut skipped: Vec<ProviderSkipReason> = Vec::new();
    let mut selected: Vec<String> = Vec::new();
    for id in &deduped {
        let rejection = if adapter_set.contains(id.as_str()) {
            None
        } else if !KNOWN_PROVIDER_IDS.contains(&id.as_str())
            && !config.search.providers.contains_key(id)
            && !config.search.api.contains_key(id)
        {
            Some(ProviderRoutingError::UnknownProvider(id.clone()))
        } else if let Some(env) = config.api_provider_credential_missing(id) {
            Some(ProviderRoutingError::MissingCredential {
                id: id.clone(),
                env: env.to_string(),
            })
        } else if !config.provider_is_available(id) {
            Some(ProviderRoutingError::DisabledProvider(id.clone()))
        } else {
            // Enabled and configured, yet no engine exists for this id in this
            // build. Selecting it anyway would report success with nothing
            // queried, so it must be surfaced rather than dropped.
            Some(ProviderRoutingError::NotBuilt(id.clone()))
        };

        match rejection {
            None => selected.push(id.clone()),
            Some(err) => {
                if strict {
                    return Err(err);
                }
                skipped.push(ProviderSkipReason {
                    provider_id: id.clone(),
                    reason_code: err.skip_code().as_str().to_string(),
                    reason: err.to_string(),
                    failure_class: None,
                    cooldown_until: None,
                    skip_code: Some(err.skip_code()),
                });
            }
        }
    }

    let partial = !skipped.is_empty() && !selected.is_empty();
    Ok(ProviderRoutingDecision {
        requested_profile: None,
        requested_providers: deduped,
        selected_providers: selected,
        skipped_providers: skipped,
        degraded: false,
        partial,
        reason: Some(if partial {
            "using explicitly requested providers; some were skipped as unroutable".to_string()
        } else {
            "using explicitly requested providers".to_string()
        }),
        intent_routing: None,
    })
}
