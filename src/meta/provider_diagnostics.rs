//! Provider health tracking, routing decisions, and capability enforcement telemetry.
//!
//! This module provides process-local provider health snapshots and
//! capability-aware selection telemetry. Health state is non-authoritative
//! and advisory — it influences profile/default routing but does not
//! override explicit provider requests.

use std::collections::BTreeMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::core::provider::{
    built_in_provider_descriptor, provider_configured_state, ProviderSkipCode, KNOWN_PROVIDER_IDS,
};
use crate::core::repo_search::SearchProfile;

/// Coarse error class for health tracking. Matches `ErrorClass` from adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FailureClass {
    /// The engine did not respond within the per-engine timeout.
    Timeout,
    /// The engine responded with a non-2xx HTTP status.
    HttpStatus,
    /// The engine responded but the HTML could not be parsed.
    ParseError,
    /// A network-level error (DNS, TLS, connection reset, etc.).
    NetworkError,
    /// The engine returned HTTP 429 (rate-limited).
    RateLimited,
    /// The provider task panicked during dispatch.
    Panic,
    /// Unclassified failure.
    Unknown,
}

/// Maximum length for error messages exposed through health snapshots.
const MAX_ERROR_MESSAGE_LEN: usize = 512;

/// Bound an error message string for safe exposure in MCP/CLI output.
///
/// Strips control characters, truncates to 512 chars,
/// and returns `None` for empty strings.
pub fn bound_error_message(msg: &str) -> Option<String> {
    let cleaned: String = msg
        .chars()
        .filter(|c| !c.is_control() || *c == '\n')
        .collect();
    let trimmed = cleaned.trim();
    if trimmed.is_empty() {
        None
    } else if trimmed.chars().count() > MAX_ERROR_MESSAGE_LEN {
        let truncated: String = trimmed.chars().take(MAX_ERROR_MESSAGE_LEN).collect();
        Some(format!("{truncated}…"))
    } else {
        Some(trimmed.to_string())
    }
}

impl FailureClass {
    /// Stable snake-case string form.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Timeout => "timeout",
            Self::HttpStatus => "http_status",
            Self::ParseError => "parse_error",
            Self::NetworkError => "network_error",
            Self::RateLimited => "rate_limited",
            Self::Panic => "panic",
            Self::Unknown => "unknown",
        }
    }

    /// Derive from the string representation in `ProviderFailure.error_class`.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        match s {
            "timeout" => Self::Timeout,
            "http_status" => Self::HttpStatus,
            "parse_error" => Self::ParseError,
            "network_error" => Self::NetworkError,
            "rate_limited" => Self::RateLimited,
            "panic" => Self::Panic,
            _ => Self::Unknown,
        }
    }
}

impl From<crate::meta::adapter::ErrorClass> for FailureClass {
    fn from(ec: crate::meta::adapter::ErrorClass) -> Self {
        match ec {
            crate::meta::adapter::ErrorClass::Timeout => Self::Timeout,
            crate::meta::adapter::ErrorClass::HttpStatus => Self::HttpStatus,
            crate::meta::adapter::ErrorClass::ParseError => Self::ParseError,
            crate::meta::adapter::ErrorClass::NetworkError => Self::NetworkError,
            crate::meta::adapter::ErrorClass::RateLimited => Self::RateLimited,
            crate::meta::adapter::ErrorClass::Panic => Self::Panic,
            crate::meta::adapter::ErrorClass::Unknown => Self::Unknown,
        }
    }
}

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
/// Distinct from [`ProviderHealthSnapshot`] which mixes config state
/// (enabled, configured) with health. This view is purely health-focused
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
}

/// A serializable snapshot of a single provider's health state.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProviderHealthSnapshot {
    /// The provider id.
    pub provider_id: String,
    /// Whether the provider is enabled in config.
    pub enabled: bool,
    /// Whether the provider is configured (API key resolves, etc.).
    pub configured: bool,
    /// Derived health status.
    pub status: ProviderHealthStatus,
    /// Number of consecutive failures (0 if last call succeeded).
    pub consecutive_failures: u32,
    /// Error class of the most recent failure, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recent_failure_class: Option<String>,
    /// Human-readable message of the most recent failure.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recent_failure_message: Option<String>,
    /// Latency of the most recent call in milliseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recent_latency_ms: Option<u64>,
    /// When the provider will exit cooldown, if in cooldown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cooldown_until: Option<String>,
    /// Reason for the current cooldown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cooldown_reason: Option<String>,
}

/// Per-provider health entry (internal, not serialized directly).
#[derive(Clone, Debug)]
struct ProviderHealthEntry {
    last_success_at: Option<Instant>,
    last_failure_at: Option<Instant>,
    last_failure_class: Option<FailureClass>,
    last_failure_message: Option<String>,
    last_latency_ms: Option<u64>,
    consecutive_failures: u32,
    cooldown_until: Option<Instant>,
    cooldown_reason: Option<String>,
}

impl ProviderHealthEntry {
    fn new() -> Self {
        Self {
            last_success_at: None,
            last_failure_at: None,
            last_failure_class: None,
            last_failure_message: None,
            last_latency_ms: None,
            consecutive_failures: 0,
            cooldown_until: None,
            cooldown_reason: None,
        }
    }

    fn status(&self, now: Instant) -> ProviderHealthStatus {
        if let Some(until) = self.cooldown_until {
            if now < until {
                return ProviderHealthStatus::Cooldown;
            }
        }
        if self.consecutive_failures > 0 {
            return ProviderHealthStatus::Degraded;
        }
        if self.last_success_at.is_some() || self.last_failure_at.is_some() {
            return ProviderHealthStatus::Healthy;
        }
        ProviderHealthStatus::Unknown
    }

    fn view(&self, now: Instant) -> ProviderHealthView {
        ProviderHealthView {
            status: self.status(now),
            consecutive_failures: self.consecutive_failures,
            last_error_class: self.last_failure_class.map(|c| c.as_str().to_string()),
            last_error_message: self
                .last_failure_message
                .as_deref()
                .and_then(bound_error_message),
            cooldown_until: self.cooldown_until.and_then(|until| {
                if now < until {
                    let remaining = until.duration_since(now).as_secs();
                    Some(format!("{remaining}s"))
                } else {
                    None
                }
            }),
            cooldown_reason: self.cooldown_reason.clone(),
            last_latency_ms: self.last_latency_ms,
            last_success_at: self.last_success_at.map(|t| {
                let elapsed = t.elapsed().as_secs();
                format!("{elapsed}s ago")
            }),
            last_failure_at: self.last_failure_at.map(|t| {
                let elapsed = t.elapsed().as_secs();
                format!("{elapsed}s ago")
            }),
        }
    }
}

/// Default cooldown durations.
const RATE_LIMIT_COOLDOWN: Duration = Duration::from_secs(60);
const TIMEOUT_COOLDOWN: Duration = Duration::from_secs(15);
const TRANSPORT_COOLDOWN: Duration = Duration::from_secs(30);
/// Number of consecutive failures before entering cooldown.
const COOLDOWN_THRESHOLD: u32 = 3;

/// Process-local provider health registry.
///
/// Wrapped in `Arc<ProviderHealthRegistry>` and shared across requests.
/// All mutations are protected by an internal mutex. Critical sections
/// are kept small — no provider calls happen while the lock is held.
pub struct ProviderHealthRegistry {
    entries: Mutex<BTreeMap<String, ProviderHealthEntry>>,
}

impl ProviderHealthRegistry {
    /// Create an empty health registry.
    pub fn new() -> Self {
        Self {
            entries: Mutex::new(BTreeMap::new()),
        }
    }

    /// Record a successful provider call.
    pub fn record_success(&self, provider_id: &str, latency_ms: u64) {
        let mut entries = self
            .entries
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let entry = entries
            .entry(provider_id.to_string())
            .or_insert_with(ProviderHealthEntry::new);
        entry.last_success_at = Some(Instant::now());
        entry.consecutive_failures = 0;
        entry.last_latency_ms = Some(latency_ms);
        entry.cooldown_until = None;
        entry.cooldown_reason = None;
    }

    /// Record a failed provider call.
    pub fn record_failure(
        &self,
        provider_id: &str,
        failure_class: FailureClass,
        message: &str,
        latency_ms: u64,
    ) {
        let now = Instant::now();
        let bounded = bound_error_message(message);
        let mut entries = self
            .entries
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let entry = entries
            .entry(provider_id.to_string())
            .or_insert_with(ProviderHealthEntry::new);
        entry.last_failure_at = Some(now);
        entry.last_failure_class = Some(failure_class);
        entry.last_failure_message = bounded;
        entry.last_latency_ms = Some(latency_ms);
        entry.consecutive_failures += 1;

        // Enter cooldown after threshold consecutive failures
        if entry.consecutive_failures >= COOLDOWN_THRESHOLD && entry.cooldown_until.is_none() {
            let (duration, reason) = match failure_class {
                FailureClass::RateLimited => (RATE_LIMIT_COOLDOWN, "rate limited"),
                FailureClass::Timeout => (TIMEOUT_COOLDOWN, "repeated timeouts"),
                FailureClass::NetworkError | FailureClass::HttpStatus => {
                    (TRANSPORT_COOLDOWN, "transport failures")
                }
                _ => (TRANSPORT_COOLDOWN, "repeated failures"),
            };
            entry.cooldown_until = Some(now + duration);
            entry.cooldown_reason = Some(reason.to_string());
        }
    }

    /// Check if a provider is currently in cooldown.
    pub fn is_in_cooldown(&self, provider_id: &str) -> bool {
        let entries = self
            .entries
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(entry) = entries.get(provider_id) {
            if let Some(until) = entry.cooldown_until {
                return Instant::now() < until;
            }
        }
        false
    }

    /// Get a compact health view for a single provider.
    pub fn health_view(&self, provider_id: &str) -> ProviderHealthView {
        let entries = self
            .entries
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let now = Instant::now();
        entries
            .get(provider_id)
            .map(|e| e.view(now))
            .unwrap_or(ProviderHealthView {
                status: ProviderHealthStatus::Unknown,
                consecutive_failures: 0,
                last_error_class: None,
                last_error_message: None,
                cooldown_until: None,
                cooldown_reason: None,
                last_latency_ms: None,
                last_success_at: None,
                last_failure_at: None,
            })
    }

    /// Get a serializable health snapshot for a single provider.
    pub fn snapshot(
        &self,
        provider_id: &str,
        enabled: bool,
        configured: bool,
    ) -> ProviderHealthSnapshot {
        let entries = self
            .entries
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let now = Instant::now();
        let entry = entries.get(provider_id);
        ProviderHealthSnapshot {
            provider_id: provider_id.to_string(),
            enabled,
            configured,
            status: entry
                .map(|e| e.status(now))
                .unwrap_or(ProviderHealthStatus::Unknown),
            consecutive_failures: entry.map(|e| e.consecutive_failures).unwrap_or(0),
            recent_failure_class: entry
                .and_then(|e| e.last_failure_class)
                .map(|c| c.as_str().to_string()),
            recent_failure_message: entry.and_then(|e| e.last_failure_message.clone()),
            recent_latency_ms: entry.and_then(|e| e.last_latency_ms),
            cooldown_until: entry.and_then(|e| {
                e.cooldown_until.and_then(|until| {
                    let now = Instant::now();
                    if now < until {
                        let remaining = until.duration_since(now).as_secs();
                        Some(format!("{remaining}s"))
                    } else {
                        None
                    }
                })
            }),
            cooldown_reason: entry.and_then(|e| e.cooldown_reason.clone()),
        }
    }

    /// Get health snapshots for all providers.
    pub fn all_snapshots(
        &self,
        enabled_ids: &[String],
        searxng_configured: bool,
        api_configured: &BTreeMap<String, bool>,
        local_backend_available: bool,
    ) -> Vec<ProviderHealthSnapshot> {
        let mut snapshots = Vec::new();
        for id in KNOWN_PROVIDER_IDS {
            let enabled = if *id == "local_workspace" {
                local_backend_available
            } else {
                enabled_ids.iter().any(|s| s.as_str() == *id)
            };
            let configured = provider_configured_state(
                id,
                searxng_configured,
                api_configured.get(*id).copied().unwrap_or(false),
                local_backend_available,
            );
            snapshots.push(self.snapshot(id, enabled, configured));
        }
        // Add API-configured providers not in KNOWN_PROVIDER_IDS
        for (id, &configured) in api_configured {
            if !KNOWN_PROVIDER_IDS.contains(&id.as_str()) {
                let enabled = enabled_ids.iter().any(|s| s == id);
                snapshots.push(self.snapshot(id, enabled, configured));
            }
        }
        snapshots
    }
}

impl Default for ProviderHealthRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for ProviderHealthRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let entries = self
            .entries
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        f.debug_struct("ProviderHealthRegistry")
            .field("entries_count", &entries.len())
            .finish()
    }
}

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
}

/// Error from provider routing resolution.
#[derive(Debug)]
pub enum ProviderRoutingError {
    /// An explicitly requested provider is not known.
    UnknownProvider(String),
    /// An explicitly requested provider is disabled.
    DisabledProvider(String),
    /// No default providers are available.
    NoDefaultProviders(String),
}

impl std::fmt::Display for ProviderRoutingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownProvider(id) => write!(f, "unknown provider id: {id}"),
            Self::DisabledProvider(id) => write!(f, "provider is disabled: {id}"),
            Self::NoDefaultProviders(msg) => write!(f, "no default providers: {msg}"),
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
pub fn resolve_provider_routing(
    requested_providers: &[String],
    profile: Option<SearchProfile>,
    adapter_provider_ids: &[String],
    config: &crate::core::config::AppConfig,
    health: &ProviderHealthRegistry,
    strict_explicit: bool,
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
            });
        }

        if any_skipped && !filtered.is_empty() {
            partial = true;
        }

        effective_providers = filtered;
    } else {
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

    // Validate: unknown providers are always errors for explicit lists
    for id in &deduped {
        if !adapter_set.contains(id.as_str()) {
            // Check if it's a known but disabled provider
            let is_known = KNOWN_PROVIDER_IDS.contains(&id.as_str())
                || config.search.providers.contains_key(id)
                || config.search.api.contains_key(id);
            if is_known && !config.provider_is_available(id) {
                if strict {
                    return Err(ProviderRoutingError::DisabledProvider(id.clone()));
                }
            } else if !is_known && strict {
                return Err(ProviderRoutingError::UnknownProvider(id.clone()));
            }
        }
    }

    Ok(ProviderRoutingDecision {
        requested_profile: None,
        requested_providers: deduped.clone(),
        selected_providers: deduped,
        skipped_providers: vec![],
        degraded: false,
        partial: false,
        reason: Some("using explicitly requested providers".to_string()),
    })
}

/// Capability enforcement tracking for search responses.
#[derive(Clone, Debug, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CapabilityEnforcementTelemetry {
    /// Capabilities that were requested (e.g. "repo_filter", "code_search").
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requested: Vec<String>,
    /// Capabilities that were enforced natively by a provider.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub enforced: Vec<String>,
    /// Capabilities that were approximated (e.g. free-text query matched
    /// but provider couldn't enforce server-side).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub approximated: Vec<String>,
    /// Capabilities that were not enforced by any provider.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub not_enforced: Vec<String>,
}

impl CapabilityEnforcementTelemetry {
    /// Build enforcement telemetry for a repo_search request by checking
    /// which providers can enforce which capabilities.
    pub fn for_repo_search(
        req: &crate::core::repo_search::RepoSearchRequest,
        selected_providers: &[String],
    ) -> Self {
        let hints = req.resolved_hints();
        let mut requested = Vec::new();
        let mut enforced = Vec::new();
        let mut approximated = Vec::new();
        let mut not_enforced = Vec::new();

        // Determine what capabilities are requested based on hints and request fields
        let wants_repo = hints.repo.is_some() || hints.owner.is_some();
        let wants_path = hints.path.is_some() || hints.file.is_some();
        let wants_language = hints.language.is_some();
        let wants_symbol = hints.symbol.is_some();
        let wants_issues = req.include_issues.unwrap_or(true);
        let wants_releases = req.include_releases.unwrap_or(true);
        let wants_freshness = req.freshness != crate::core::query::Freshness::Any;
        let wants_package = req.package.is_some() || req.ecosystem.is_some();

        if wants_repo {
            requested.push("repo_filter".to_string());
        }
        if wants_path {
            requested.push("path_filter".to_string());
        }
        if wants_language {
            requested.push("language_filter".to_string());
        }
        if wants_symbol {
            requested.push("symbol_hint".to_string());
        }
        if wants_issues {
            requested.push("issue_search".to_string());
        }
        if wants_releases {
            requested.push("release_search".to_string());
        }
        if wants_freshness {
            requested.push("freshness_filter".to_string());
        }
        if wants_package {
            requested.push("package_search".to_string());
        }

        if requested.is_empty() {
            return Self::default();
        }

        // Check which capabilities are enforced by selected providers
        let mut repo_enforced = false;
        let mut path_enforced = false;
        let mut lang_enforced = false;
        let mut symbol_enforced = false;
        let mut issue_enforced = false;
        let mut release_enforced = false;

        for id in selected_providers {
            if let Some(desc) =
                built_in_provider_descriptor(id, true, false, true, false, None, None)
            {
                if desc.capabilities.supports_repo_filter {
                    repo_enforced = true;
                }
                if desc.capabilities.supports_path_filter {
                    path_enforced = true;
                }
                if desc.capabilities.supports_language_filter {
                    lang_enforced = true;
                }
                if desc.capabilities.supports_symbol_hint {
                    symbol_enforced = true;
                }
                if desc.capabilities.supports_issue_search {
                    issue_enforced = true;
                }
                if desc.capabilities.supports_release_search {
                    release_enforced = true;
                }
            }
        }

        if wants_repo {
            if repo_enforced {
                enforced.push("repo_filter".to_string());
            } else {
                approximated.push("repo_filter".to_string());
            }
        }
        if wants_path {
            if path_enforced {
                enforced.push("path_filter".to_string());
            } else {
                approximated.push("path_filter".to_string());
            }
        }
        if wants_language {
            if lang_enforced {
                enforced.push("language_filter".to_string());
            } else {
                approximated.push("language_filter".to_string());
            }
        }
        if wants_symbol {
            if symbol_enforced {
                enforced.push("symbol_hint".to_string());
            } else {
                not_enforced.push("symbol_hint".to_string());
            }
        }
        if wants_issues {
            if issue_enforced {
                enforced.push("issue_search".to_string());
            } else {
                approximated.push("issue_search".to_string());
            }
        }
        if wants_releases {
            if release_enforced {
                enforced.push("release_search".to_string());
            } else {
                approximated.push("release_search".to_string());
            }
        }
        if wants_freshness {
            // No native repo provider enforces freshness server-side
            not_enforced.push("freshness_filter".to_string());
        }
        if wants_package {
            // Package search is always approximate via generic providers
            approximated.push("package_search".to_string());
        }

        Self {
            requested,
            enforced,
            approximated,
            not_enforced,
        }
    }

    /// Build enforcement telemetry for a security_search request.
    pub fn for_security_search(
        req: &crate::core::security::SecuritySearchRequest,
        selected_providers: &[String],
    ) -> Self {
        let mut requested = Vec::new();
        let mut enforced = Vec::new();
        let mut approximated = Vec::new();
        let mut not_enforced = Vec::new();

        let has_advisory_id = req.cve_id.is_some()
            || req.ghsa_id.is_some()
            || req.osv_id.is_some()
            || req.rustsec_id.is_some();
        let has_package = req.package.is_some();
        let has_version = req.version.is_some();
        let has_severity = req.severity_min.is_some();
        let wants_kev = req.include_kev.unwrap_or(false);
        let wants_exploit = req.include_exploit_context.unwrap_or(false);

        if has_advisory_id {
            requested.push("advisory_lookup".to_string());
        }
        if has_package {
            requested.push("package_filter".to_string());
        }
        if has_version {
            requested.push("version_filter".to_string());
        }
        if has_severity {
            requested.push("severity_filter".to_string());
        }
        if wants_kev {
            requested.push("kev_support".to_string());
        }
        if wants_exploit {
            requested.push("exploit_context".to_string());
        }

        if requested.is_empty() {
            return Self::default();
        }

        let mut advisory_enforced = false;
        let mut package_enforced = false;

        for id in selected_providers {
            if let Some(desc) =
                built_in_provider_descriptor(id, true, false, true, false, None, None)
            {
                if desc.capabilities.supports_security_search {
                    advisory_enforced = true;
                    package_enforced = true;
                }
            }
        }

        if has_advisory_id {
            if advisory_enforced {
                enforced.push("advisory_lookup".to_string());
            } else {
                not_enforced.push("advisory_lookup".to_string());
            }
        }
        if has_package {
            if package_enforced {
                enforced.push("package_filter".to_string());
            } else {
                approximated.push("package_filter".to_string());
            }
        }
        if has_version {
            // Version filtering is always approximate via web search
            approximated.push("version_filter".to_string());
        }
        if has_severity {
            // Severity filtering is always approximate via web search
            approximated.push("severity_filter".to_string());
        }
        if wants_kev {
            // KEV lookup is always approximate — fetched from CISA catalog
            approximated.push("kev_support".to_string());
        }
        if wants_exploit {
            // Exploit context is always approximate — gathered from web search
            approximated.push("exploit_context".to_string());
        }

        Self {
            requested,
            enforced,
            approximated,
            not_enforced,
        }
    }

    /// Build enforcement telemetry for a research_search request.
    pub fn for_research_search(
        req: &crate::core::research::ResearchSearchRequest,
        _selected_providers: &[String],
    ) -> Self {
        let mut requested = Vec::new();
        let enforced = Vec::new();
        let mut approximated = Vec::new();
        let mut not_enforced = Vec::new();

        let wants_source_diversity = !req.desired_source_types.is_empty();
        let wants_primary = req.include_primary_sources.unwrap_or(false);
        let wants_freshness = req.freshness != crate::core::query::Freshness::Any;
        let wants_counterpoints = req.include_counterpoints.unwrap_or(false);

        if wants_source_diversity {
            requested.push("source_diversity".to_string());
        }
        if wants_primary {
            requested.push("primary_source_preference".to_string());
        }
        if wants_freshness {
            requested.push("freshness_filter".to_string());
        }
        if wants_counterpoints {
            requested.push("counterpoint_inclusion".to_string());
        }

        if requested.is_empty() {
            return Self::default();
        }

        // Source diversity is always approximate — research search uses
        // subquery generation, not native source-type filtering.
        if wants_source_diversity {
            approximated.push("source_diversity".to_string());
        }
        // Primary source preference is approximate — boosted via reranking,
        // not native provider filtering.
        if wants_primary {
            approximated.push("primary_source_preference".to_string());
        }
        // Freshness is approximate — no research provider enforces server-side.
        if wants_freshness {
            not_enforced.push("freshness_filter".to_string());
        }
        // Counterpoint inclusion is approximate — subquery generation only.
        if wants_counterpoints {
            approximated.push("counterpoint_inclusion".to_string());
        }

        Self {
            requested,
            enforced,
            approximated,
            not_enforced,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn health_registry_record_success() {
        let registry = ProviderHealthRegistry::new();
        registry.record_success("duckduckgo", 150);

        let snapshot = registry.snapshot("duckduckgo", true, true);
        assert_eq!(snapshot.status, ProviderHealthStatus::Healthy);
        assert_eq!(snapshot.consecutive_failures, 0);
        assert_eq!(snapshot.recent_latency_ms, Some(150));
    }

    #[test]
    fn health_registry_record_failure() {
        let registry = ProviderHealthRegistry::new();
        registry.record_failure("brave", FailureClass::Timeout, "timed out", 5000);

        let snapshot = registry.snapshot("brave", true, true);
        assert_eq!(snapshot.status, ProviderHealthStatus::Degraded);
        assert_eq!(snapshot.consecutive_failures, 1);
        assert_eq!(snapshot.recent_failure_class.as_deref(), Some("timeout"));
    }

    #[test]
    fn health_registry_cooldown_after_threshold() {
        let registry = ProviderHealthRegistry::new();
        for _ in 0..3 {
            registry.record_failure("brave", FailureClass::RateLimited, "429", 100);
        }

        assert!(registry.is_in_cooldown("brave"));
        let snapshot = registry.snapshot("brave", true, true);
        assert_eq!(snapshot.status, ProviderHealthStatus::Cooldown);
    }

    #[test]
    fn health_registry_success_clears_cooldown() {
        let registry = ProviderHealthRegistry::new();
        for _ in 0..3 {
            registry.record_failure("brave", FailureClass::Timeout, "timed out", 100);
        }
        assert!(registry.is_in_cooldown("brave"));

        registry.record_success("brave", 200);
        assert!(!registry.is_in_cooldown("brave"));
        let snapshot = registry.snapshot("brave", true, true);
        assert_eq!(snapshot.status, ProviderHealthStatus::Healthy);
        assert_eq!(snapshot.consecutive_failures, 0);
    }

    #[test]
    fn health_registry_unknown_provider() {
        let registry = ProviderHealthRegistry::new();
        let snapshot = registry.snapshot("nonexistent", true, true);
        assert_eq!(snapshot.status, ProviderHealthStatus::Unknown);
        assert_eq!(snapshot.consecutive_failures, 0);
    }

    #[test]
    fn health_registry_recovers_from_poisoned_mutex() {
        let registry = ProviderHealthRegistry::new();
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _entries = registry.entries.lock().unwrap();
            panic!("poison provider health mutex");
        }));

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            registry.record_success("duckduckgo", 100);
        }));
        assert!(result.is_ok());
    }

    #[test]
    fn failure_class_roundtrip() {
        for class in [
            FailureClass::Timeout,
            FailureClass::HttpStatus,
            FailureClass::ParseError,
            FailureClass::NetworkError,
            FailureClass::RateLimited,
            FailureClass::Panic,
            FailureClass::Unknown,
        ] {
            let s = class.as_str();
            assert_eq!(FailureClass::from_str(s), class);
        }
    }

    #[test]
    fn capability_enforcement_empty_hints() {
        let req = crate::core::repo_search::RepoSearchRequest {
            query: "axum".to_string(),
            include_issues: Some(false),
            include_releases: Some(false),
            ..Default::default()
        };
        let telemetry = CapabilityEnforcementTelemetry::for_repo_search(&req, &[]);
        assert!(telemetry.requested.is_empty());
        assert!(telemetry.enforced.is_empty());
        assert!(telemetry.approximated.is_empty());
        assert!(telemetry.not_enforced.is_empty());
    }

    #[test]
    fn capability_enforcement_with_hints() {
        let req = crate::core::repo_search::RepoSearchRequest {
            query: "repo:tokio/tokio path:src/".to_string(),
            include_issues: Some(false),
            include_releases: Some(false),
            ..Default::default()
        };
        // No native providers — repo/path are approximated, issues/releases not requested
        let telemetry =
            CapabilityEnforcementTelemetry::for_repo_search(&req, &["duckduckgo".to_string()]);
        assert!(telemetry.requested.contains(&"repo_filter".to_string()));
        assert!(telemetry.requested.contains(&"path_filter".to_string()));
        assert!(telemetry.approximated.contains(&"repo_filter".to_string()));
        assert!(telemetry.approximated.contains(&"path_filter".to_string()));
    }

    #[test]
    fn capability_enforcement_native_provider() {
        let req = crate::core::repo_search::RepoSearchRequest {
            query: "repo:tokio/tokio".to_string(),
            include_issues: Some(false),
            include_releases: Some(false),
            ..Default::default()
        };
        let telemetry =
            CapabilityEnforcementTelemetry::for_repo_search(&req, &["github_code".to_string()]);
        assert!(telemetry.enforced.contains(&"repo_filter".to_string()));
    }

    #[test]
    fn capability_enforcement_symbol_not_enforced() {
        let req = crate::core::repo_search::RepoSearchRequest {
            query: "symbol:Router::layer".to_string(),
            include_issues: Some(false),
            include_releases: Some(false),
            ..Default::default()
        };
        let telemetry =
            CapabilityEnforcementTelemetry::for_repo_search(&req, &["duckduckgo".to_string()]);
        assert!(telemetry.not_enforced.contains(&"symbol_hint".to_string()));
    }

    #[test]
    fn capability_enforcement_symbol_enforced() {
        let req = crate::core::repo_search::RepoSearchRequest {
            query: "symbol:Router::layer".to_string(),
            include_issues: Some(false),
            include_releases: Some(false),
            ..Default::default()
        };
        let telemetry =
            CapabilityEnforcementTelemetry::for_repo_search(&req, &["github_code".to_string()]);
        assert!(telemetry.enforced.contains(&"symbol_hint".to_string()));
    }

    #[test]
    fn health_snapshot_serialization() {
        let snapshot = ProviderHealthSnapshot {
            provider_id: "duckduckgo".to_string(),
            enabled: true,
            configured: true,
            status: ProviderHealthStatus::Healthy,
            consecutive_failures: 0,
            recent_failure_class: None,
            recent_failure_message: None,
            recent_latency_ms: Some(123),
            cooldown_until: None,
            cooldown_reason: None,
        };
        let json = serde_json::to_string(&snapshot).unwrap();
        assert!(json.contains("healthy"));
        assert!(json.contains("duckduckgo"));
        let parsed: ProviderHealthSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.status, ProviderHealthStatus::Healthy);
    }

    #[test]
    fn all_snapshots_reflect_configured_state() {
        let registry = ProviderHealthRegistry::new();
        let enabled_ids = vec![
            "duckduckgo".to_string(),
            "searxng".to_string(),
            "brave_api".to_string(),
            "local_workspace".to_string(),
        ];
        let mut api_configured = BTreeMap::new();
        api_configured.insert("brave_api".to_string(), true);
        let snapshots = registry.all_snapshots(&enabled_ids, true, &api_configured, true);

        let duck = snapshots
            .iter()
            .find(|s| s.provider_id == "duckduckgo")
            .expect("duckduckgo snapshot");
        assert!(duck.enabled);
        assert!(duck.configured);

        let searxng = snapshots
            .iter()
            .find(|s| s.provider_id == "searxng")
            .expect("searxng snapshot");
        assert!(searxng.enabled);
        assert!(searxng.configured);

        let brave_api = snapshots
            .iter()
            .find(|s| s.provider_id == "brave_api")
            .expect("brave_api snapshot");
        assert!(brave_api.enabled);
        assert!(brave_api.configured);

        let local = snapshots
            .iter()
            .find(|s| s.provider_id == "local_workspace")
            .expect("local snapshot");
        assert!(local.enabled);
        assert!(local.configured);
    }

    #[test]
    fn routing_decision_serialization() {
        let decision = ProviderRoutingDecision {
            requested_profile: Some(SearchProfile::Coding),
            requested_providers: vec![],
            selected_providers: vec!["github_code".to_string()],
            skipped_providers: vec![ProviderSkipReason {
                provider_id: "gitlab_code".to_string(),
                reason_code: "not_built".to_string(),
                reason: "not built".to_string(),
                failure_class: None,
                cooldown_until: None,
                skip_code: Some(ProviderSkipCode::NotBuilt),
            }],
            degraded: false,
            partial: true,
            reason: Some("coding profile skipped unavailable providers".to_string()),
        };
        let json = serde_json::to_string(&decision).unwrap();
        assert!(json.contains("coding"));
        assert!(json.contains("github_code"));
        assert!(json.contains("gitlab_code"));
        let parsed: ProviderRoutingDecision = serde_json::from_str(&json).unwrap();
        assert!(parsed.partial);
        assert_eq!(parsed.skipped_providers.len(), 1);
    }

    // --- Routing helper tests ---

    fn test_config_with_providers(
        enabled: &[&str],
        disabled: &[&str],
    ) -> crate::core::config::AppConfig {
        let mut cfg = crate::core::config::AppConfig::default();
        for id in enabled {
            cfg.search.providers.insert(id.to_string(), true);
        }
        for id in disabled {
            cfg.search.providers.insert(id.to_string(), false);
        }
        cfg.search.default_providers = enabled.iter().map(|s| s.to_string()).collect();
        cfg
    }

    #[test]
    fn routing_explicit_unknown_provider_fails() {
        let cfg = test_config_with_providers(&["duckduckgo"], &[]);
        let health = ProviderHealthRegistry::new();
        let adapter_ids = vec!["duckduckgo".to_string()];

        let result = resolve_provider_routing(
            &["nonexistent_provider".to_string()],
            None,
            &adapter_ids,
            &cfg,
            &health,
            true,
        );
        assert!(result.is_err());
        match result.unwrap_err() {
            ProviderRoutingError::UnknownProvider(id) => {
                assert_eq!(id, "nonexistent_provider");
            }
            other => panic!("expected UnknownProvider, got {other:?}"),
        }
    }

    #[test]
    fn routing_explicit_disabled_provider_fails() {
        let cfg = test_config_with_providers(&["duckduckgo"], &["brave"]);
        let health = ProviderHealthRegistry::new();
        let adapter_ids = vec!["duckduckgo".to_string()];

        let result = resolve_provider_routing(
            &["brave".to_string()],
            None,
            &adapter_ids,
            &cfg,
            &health,
            true,
        );
        assert!(result.is_err());
        match result.unwrap_err() {
            ProviderRoutingError::DisabledProvider(id) => {
                assert_eq!(id, "brave");
            }
            other => panic!("expected DisabledProvider, got {other:?}"),
        }
    }

    #[test]
    fn routing_profile_partial_when_one_unavailable() {
        let mut cfg = test_config_with_providers(&["duckduckgo", "startpage"], &[]);
        cfg.search.profiles.insert(
            "coding".to_string(),
            crate::core::config::ProfileConfig {
                providers: vec!["duckduckgo".to_string(), "startpage".to_string()],
            },
        );
        let health = ProviderHealthRegistry::new();
        let adapter_ids = vec!["duckduckgo".to_string()];

        let result = resolve_provider_routing(
            &[],
            Some(SearchProfile::Coding),
            &adapter_ids,
            &cfg,
            &health,
            true,
        )
        .unwrap();
        assert!(result.partial);
        assert!(!result.degraded);
        assert!(result
            .selected_providers
            .contains(&"duckduckgo".to_string()));
        assert_eq!(result.skipped_providers.len(), 1);
        assert_eq!(result.skipped_providers[0].provider_id, "startpage");
    }

    #[test]
    fn routing_profile_degrades_to_defaults_when_all_unavailable() {
        let mut cfg = test_config_with_providers(&["duckduckgo"], &[]);
        cfg.search.profiles.insert(
            "coding".to_string(),
            crate::core::config::ProfileConfig {
                providers: vec!["github_code".to_string(), "gitlab_code".to_string()],
            },
        );
        let health = ProviderHealthRegistry::new();
        // Neither github_code nor gitlab_code is built
        let adapter_ids = vec!["duckduckgo".to_string()];

        let result = resolve_provider_routing(
            &[],
            Some(SearchProfile::Coding),
            &adapter_ids,
            &cfg,
            &health,
            true,
        )
        .unwrap();
        assert!(result.degraded);
        assert!(!result.partial);
        // Should fall back to default providers
        assert!(result
            .selected_providers
            .contains(&"duckduckgo".to_string()));
    }

    #[test]
    fn routing_cooled_down_provider_skipped_for_profile() {
        let mut cfg = test_config_with_providers(&["duckduckgo", "brave"], &[]);
        cfg.search.profiles.insert(
            "generic".to_string(),
            crate::core::config::ProfileConfig {
                providers: vec!["brave".to_string(), "duckduckgo".to_string()],
            },
        );
        let health = ProviderHealthRegistry::new();
        // Put brave into cooldown
        for _ in 0..3 {
            health.record_failure("brave", FailureClass::RateLimited, "429", 100);
        }
        let adapter_ids = vec!["brave".to_string(), "duckduckgo".to_string()];

        let result = resolve_provider_routing(
            &[],
            Some(SearchProfile::Generic),
            &adapter_ids,
            &cfg,
            &health,
            true,
        )
        .unwrap();
        // brave should be skipped due to cooldown
        assert!(!result.selected_providers.contains(&"brave".to_string()));
        assert!(result
            .selected_providers
            .contains(&"duckduckgo".to_string()));
        assert_eq!(result.skipped_providers.len(), 1);
        assert_eq!(result.skipped_providers[0].provider_id, "brave");
    }

    #[test]
    fn routing_cooled_down_provider_not_skipped_for_explicit() {
        let cfg = test_config_with_providers(&["duckduckgo", "brave"], &[]);
        let health = ProviderHealthRegistry::new();
        // Put brave into cooldown
        for _ in 0..3 {
            health.record_failure("brave", FailureClass::RateLimited, "429", 100);
        }
        let adapter_ids = vec!["brave".to_string(), "duckduckgo".to_string()];

        let result = resolve_provider_routing(
            &["brave".to_string()],
            None,
            &adapter_ids,
            &cfg,
            &health,
            true,
        )
        .unwrap();
        // Explicit provider should NOT be skipped even if cooled down
        assert!(result.selected_providers.contains(&"brave".to_string()));
        assert!(result.skipped_providers.is_empty());
    }

    #[test]
    fn routing_deterministic_provider_order() {
        let cfg = test_config_with_providers(&["duckduckgo", "brave", "startpage"], &[]);
        let health = ProviderHealthRegistry::new();
        let adapter_ids = vec![
            "startpage".to_string(),
            "duckduckgo".to_string(),
            "brave".to_string(),
        ];

        // Run routing multiple times — order should be deterministic
        let mut results = Vec::new();
        for _ in 0..5 {
            let decision =
                resolve_provider_routing(&[], None, &adapter_ids, &cfg, &health, true).unwrap();
            results.push(decision.selected_providers.clone());
        }
        for window in results.windows(2) {
            assert_eq!(window[0], window[1], "routing order must be deterministic");
        }
    }

    #[test]
    fn capability_enforcement_issues_enforced() {
        let req = crate::core::repo_search::RepoSearchRequest {
            query: "repo:axum".to_string(),
            // include_issues defaults to true
            include_releases: Some(false),
            ..Default::default()
        };
        // github_issues supports issue_search
        let telemetry =
            CapabilityEnforcementTelemetry::for_repo_search(&req, &["github_issues".to_string()]);
        assert!(telemetry.requested.contains(&"issue_search".to_string()));
        assert!(telemetry.enforced.contains(&"issue_search".to_string()));
    }

    #[test]
    fn capability_enforcement_issues_approximated() {
        let req = crate::core::repo_search::RepoSearchRequest {
            query: "repo:axum".to_string(),
            include_releases: Some(false),
            ..Default::default()
        };
        // duckduckgo doesn't support issue_search
        let telemetry =
            CapabilityEnforcementTelemetry::for_repo_search(&req, &["duckduckgo".to_string()]);
        assert!(telemetry.requested.contains(&"issue_search".to_string()));
        assert!(telemetry.approximated.contains(&"issue_search".to_string()));
    }

    #[test]
    fn capability_enforcement_releases_enforced() {
        let req = crate::core::repo_search::RepoSearchRequest {
            query: "repo:axum".to_string(),
            include_issues: Some(false),
            // include_releases defaults to true
            ..Default::default()
        };
        // github_releases supports release_search
        let telemetry =
            CapabilityEnforcementTelemetry::for_repo_search(&req, &["github_releases".to_string()]);
        assert!(telemetry.requested.contains(&"release_search".to_string()));
        assert!(telemetry.enforced.contains(&"release_search".to_string()));
    }

    #[test]
    fn capability_enforcement_releases_approximated() {
        let req = crate::core::repo_search::RepoSearchRequest {
            query: "repo:axum".to_string(),
            include_issues: Some(false),
            // include_releases defaults to true
            ..Default::default()
        };
        let telemetry =
            CapabilityEnforcementTelemetry::for_repo_search(&req, &["duckduckgo".to_string()]);
        assert!(telemetry.requested.contains(&"release_search".to_string()));
        assert!(telemetry
            .approximated
            .contains(&"release_search".to_string()));
    }

    #[test]
    fn capability_enforcement_freshness_not_enforced() {
        let req = crate::core::repo_search::RepoSearchRequest {
            query: "repo:axum".to_string(),
            freshness: crate::core::query::Freshness::Week,
            include_issues: Some(false),
            include_releases: Some(false),
            ..Default::default()
        };
        let telemetry =
            CapabilityEnforcementTelemetry::for_repo_search(&req, &["github_code".to_string()]);
        assert!(telemetry
            .requested
            .contains(&"freshness_filter".to_string()));
        assert!(telemetry
            .not_enforced
            .contains(&"freshness_filter".to_string()));
    }

    #[test]
    fn capability_enforcement_package_approximated() {
        let req = crate::core::repo_search::RepoSearchRequest {
            query: "repo:axum".to_string(),
            package: Some("axum".to_string()),
            include_issues: Some(false),
            include_releases: Some(false),
            ..Default::default()
        };
        let telemetry =
            CapabilityEnforcementTelemetry::for_repo_search(&req, &["github_code".to_string()]);
        assert!(telemetry.requested.contains(&"package_search".to_string()));
        assert!(telemetry
            .approximated
            .contains(&"package_search".to_string()));
    }

    // --- Security search capability enforcement tests ---

    #[test]
    fn security_enforcement_empty_request() {
        let req = crate::core::security::SecuritySearchRequest {
            query: "test".to_string(),
            ..Default::default()
        };
        let telemetry = CapabilityEnforcementTelemetry::for_security_search(&req, &[]);
        assert!(telemetry.requested.is_empty());
        assert!(telemetry.enforced.is_empty());
        assert!(telemetry.approximated.is_empty());
        assert!(telemetry.not_enforced.is_empty());
    }

    #[test]
    fn security_enforcement_cve_with_native_osv() {
        let req = crate::core::security::SecuritySearchRequest {
            query: "CVE-2024-1234".to_string(),
            cve_id: Some("CVE-2024-1234".to_string()),
            package: Some("openssl".to_string()),
            version: Some("3.0.0".to_string()),
            ..Default::default()
        };
        let telemetry =
            CapabilityEnforcementTelemetry::for_security_search(&req, &["osv".to_string()]);
        assert!(telemetry.requested.contains(&"advisory_lookup".to_string()));
        assert!(telemetry.enforced.contains(&"advisory_lookup".to_string()));
        assert!(telemetry.enforced.contains(&"package_filter".to_string()));
        // Version filtering is always approximate
        assert!(telemetry
            .approximated
            .contains(&"version_filter".to_string()));
    }

    #[test]
    fn security_enforcement_cve_without_native_osv() {
        let req = crate::core::security::SecuritySearchRequest {
            query: "CVE-2024-1234".to_string(),
            cve_id: Some("CVE-2024-1234".to_string()),
            ..Default::default()
        };
        let telemetry =
            CapabilityEnforcementTelemetry::for_security_search(&req, &["duckduckgo".to_string()]);
        assert!(telemetry.requested.contains(&"advisory_lookup".to_string()));
        assert!(telemetry
            .not_enforced
            .contains(&"advisory_lookup".to_string()));
    }

    #[test]
    fn security_enforcement_severity_always_approximated() {
        let req = crate::core::security::SecuritySearchRequest {
            query: "test".to_string(),
            severity_min: Some(crate::core::security::SeverityLevel::High),
            ..Default::default()
        };
        let telemetry =
            CapabilityEnforcementTelemetry::for_security_search(&req, &["osv".to_string()]);
        assert!(telemetry.requested.contains(&"severity_filter".to_string()));
        assert!(telemetry
            .approximated
            .contains(&"severity_filter".to_string()));
    }

    #[test]
    fn security_enforcement_kev_always_approximated() {
        let req = crate::core::security::SecuritySearchRequest {
            query: "CVE-2024-1234".to_string(),
            cve_id: Some("CVE-2024-1234".to_string()),
            include_kev: Some(true),
            ..Default::default()
        };
        let telemetry =
            CapabilityEnforcementTelemetry::for_security_search(&req, &["osv".to_string()]);
        assert!(telemetry.requested.contains(&"kev_support".to_string()));
        assert!(telemetry.approximated.contains(&"kev_support".to_string()));
    }

    #[test]
    fn security_enforcement_exploit_context_always_approximated() {
        let req = crate::core::security::SecuritySearchRequest {
            query: "CVE-2024-1234".to_string(),
            cve_id: Some("CVE-2024-1234".to_string()),
            include_exploit_context: Some(true),
            ..Default::default()
        };
        let telemetry =
            CapabilityEnforcementTelemetry::for_security_search(&req, &["duckduckgo".to_string()]);
        assert!(telemetry.requested.contains(&"exploit_context".to_string()));
        assert!(telemetry
            .approximated
            .contains(&"exploit_context".to_string()));
    }

    #[test]
    fn security_enforcement_kev_not_requested() {
        let req = crate::core::security::SecuritySearchRequest {
            query: "test".to_string(),
            include_kev: Some(false),
            include_exploit_context: Some(false),
            ..Default::default()
        };
        let telemetry =
            CapabilityEnforcementTelemetry::for_security_search(&req, &["duckduckgo".to_string()]);
        assert!(!telemetry.requested.contains(&"kev_support".to_string()));
        assert!(!telemetry.requested.contains(&"exploit_context".to_string()));
    }

    // --- Research search capability enforcement tests ---

    #[test]
    fn research_enforcement_empty_request() {
        let req = crate::core::research::ResearchSearchRequest {
            query: "test".to_string(),
            ..Default::default()
        };
        let telemetry = CapabilityEnforcementTelemetry::for_research_search(&req, &[]);
        assert!(telemetry.requested.is_empty());
    }

    #[test]
    fn research_enforcement_source_diversity() {
        let req = crate::core::research::ResearchSearchRequest {
            query: "test".to_string(),
            desired_source_types: vec![
                crate::core::research::ResearchSourceType::PrimarySources,
                crate::core::research::ResearchSourceType::OfficialDocs,
            ],
            ..Default::default()
        };
        let telemetry = CapabilityEnforcementTelemetry::for_research_search(&req, &[]);
        assert!(telemetry
            .requested
            .contains(&"source_diversity".to_string()));
        assert!(telemetry
            .approximated
            .contains(&"source_diversity".to_string()));
    }

    #[test]
    fn research_enforcement_freshness_not_enforced() {
        let req = crate::core::research::ResearchSearchRequest {
            query: "test".to_string(),
            freshness: crate::core::query::Freshness::Week,
            ..Default::default()
        };
        let telemetry = CapabilityEnforcementTelemetry::for_research_search(&req, &[]);
        assert!(telemetry
            .requested
            .contains(&"freshness_filter".to_string()));
        assert!(telemetry
            .not_enforced
            .contains(&"freshness_filter".to_string()));
    }

    #[test]
    fn research_enforcement_primary_source() {
        let req = crate::core::research::ResearchSearchRequest {
            query: "test".to_string(),
            include_primary_sources: Some(true),
            ..Default::default()
        };
        let telemetry = CapabilityEnforcementTelemetry::for_research_search(&req, &[]);
        assert!(telemetry
            .requested
            .contains(&"primary_source_preference".to_string()));
        assert!(telemetry
            .approximated
            .contains(&"primary_source_preference".to_string()));
    }

    // --- Provider health view tests ---

    #[test]
    fn health_view_unknown_when_no_data() {
        let registry = ProviderHealthRegistry::new();
        let view = registry.health_view("duckduckgo");
        assert_eq!(view.status, ProviderHealthStatus::Unknown);
        assert_eq!(view.consecutive_failures, 0);
        assert!(view.last_error_class.is_none());
        assert!(view.last_error_message.is_none());
        assert!(view.cooldown_until.is_none());
    }

    #[test]
    fn health_view_healthy_after_success() {
        let registry = ProviderHealthRegistry::new();
        registry.record_success("brave", 150);
        let view = registry.health_view("brave");
        assert_eq!(view.status, ProviderHealthStatus::Healthy);
        assert_eq!(view.consecutive_failures, 0);
        assert!(view.last_success_at.is_some());
        assert!(view.last_error_class.is_none());
    }

    #[test]
    fn health_view_degraded_after_failure() {
        let registry = ProviderHealthRegistry::new();
        registry.record_failure("brave", FailureClass::Timeout, "timed out", 5000);
        let view = registry.health_view("brave");
        assert_eq!(view.status, ProviderHealthStatus::Degraded);
        assert_eq!(view.consecutive_failures, 1);
        assert_eq!(view.last_error_class.as_deref(), Some("timeout"));
        assert!(view.last_failure_at.is_some());
    }

    #[test]
    fn health_view_cooldown_after_threshold() {
        let registry = ProviderHealthRegistry::new();
        for _ in 0..3 {
            registry.record_failure("brave", FailureClass::RateLimited, "429", 100);
        }
        let view = registry.health_view("brave");
        assert_eq!(view.status, ProviderHealthStatus::Cooldown);
        assert_eq!(view.consecutive_failures, 3);
        assert!(view.cooldown_until.is_some());
        assert_eq!(view.cooldown_reason.as_deref(), Some("rate limited"));
    }

    #[test]
    fn health_view_panic_failure_class() {
        let registry = ProviderHealthRegistry::new();
        registry.record_failure(
            "brave",
            FailureClass::Panic,
            "task panicked during dispatch",
            0,
        );
        let view = registry.health_view("brave");
        assert_eq!(view.last_error_class.as_deref(), Some("panic"));
    }

    #[test]
    fn health_view_success_clears_cooldown() {
        let registry = ProviderHealthRegistry::new();
        for _ in 0..3 {
            registry.record_failure("brave", FailureClass::Timeout, "timed out", 100);
        }
        registry.record_success("brave", 200);
        let view = registry.health_view("brave");
        assert_eq!(view.status, ProviderHealthStatus::Healthy);
        assert!(view.cooldown_until.is_none());
        assert!(view.cooldown_reason.is_none());
    }

    // --- Error message bounding tests ---

    #[test]
    fn bound_error_message_short() {
        assert_eq!(
            bound_error_message("connection refused"),
            Some("connection refused".to_string())
        );
    }

    #[test]
    fn bound_error_message_empty() {
        assert_eq!(bound_error_message(""), None);
        assert_eq!(bound_error_message("   "), None);
    }

    #[test]
    fn bound_error_message_control_chars_stripped() {
        let msg = "error\x00\x01\x02 with controls";
        let result = bound_error_message(msg).unwrap();
        assert!(!result.contains('\x00'));
        assert!(!result.contains('\x01'));
        assert!(!result.contains('\x02'));
        assert!(result.contains("error"));
        assert!(result.contains("with controls"));
    }

    #[test]
    fn bound_error_message_truncated() {
        let msg = "x".repeat(1000);
        let result = bound_error_message(&msg).unwrap();
        assert!(result.len() <= 515);
        assert!(result.ends_with('…'));
    }

    #[test]
    fn bound_error_message_exactly_at_limit() {
        let msg = "a".repeat(512);
        let result = bound_error_message(&msg).unwrap();
        assert_eq!(result.len(), 512);
        assert!(!result.ends_with('…'));
    }

    #[test]
    fn bound_error_message_one_over_limit() {
        let msg = "a".repeat(513);
        let result = bound_error_message(&msg).unwrap();
        assert!(result.len() <= 515);
        assert!(result.ends_with('…'));
    }

    #[test]
    fn record_failure_bounds_message() {
        let registry = ProviderHealthRegistry::new();
        let long_msg = "e".repeat(2000);
        registry.record_failure("brave", FailureClass::NetworkError, &long_msg, 0);
        let view = registry.health_view("brave");
        let msg = view.last_error_message.unwrap();
        assert!(msg.len() <= 515);
        assert!(msg.ends_with('…'));
    }

    // --- ProviderHealthView serialization ---

    #[test]
    fn health_view_serialization() {
        let view = ProviderHealthView {
            status: ProviderHealthStatus::Healthy,
            consecutive_failures: 0,
            last_error_class: None,
            last_error_message: None,
            cooldown_until: None,
            cooldown_reason: None,
            last_latency_ms: Some(150),
            last_success_at: Some("5s ago".to_string()),
            last_failure_at: None,
        };
        let json = serde_json::to_string(&view).unwrap();
        assert!(json.contains("healthy"));
        assert!(json.contains("150"));
        let parsed: ProviderHealthView = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.status, ProviderHealthStatus::Healthy);
        assert_eq!(parsed.last_latency_ms, Some(150));
    }
}
