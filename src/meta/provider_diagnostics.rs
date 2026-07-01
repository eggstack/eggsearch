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

use crate::core::provider::{built_in_provider_descriptor, KNOWN_PROVIDER_IDS};
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
    /// Unclassified failure.
    Unknown,
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
        let mut entries = self.entries.lock().unwrap();
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
        let mut entries = self.entries.lock().unwrap();
        let entry = entries
            .entry(provider_id.to_string())
            .or_insert_with(ProviderHealthEntry::new);
        entry.last_failure_at = Some(now);
        entry.last_failure_class = Some(failure_class);
        entry.last_failure_message = Some(message.to_string());
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
        let entries = self.entries.lock().unwrap();
        if let Some(entry) = entries.get(provider_id) {
            if let Some(until) = entry.cooldown_until {
                return Instant::now() < until;
            }
        }
        false
    }

    /// Get a serializable health snapshot for a single provider.
    pub fn snapshot(
        &self,
        provider_id: &str,
        enabled: bool,
        configured: bool,
    ) -> ProviderHealthSnapshot {
        let entries = self.entries.lock().unwrap();
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
        api_configured: &BTreeMap<String, bool>,
    ) -> Vec<ProviderHealthSnapshot> {
        let mut snapshots = Vec::new();
        for id in KNOWN_PROVIDER_IDS {
            let enabled = enabled_ids.iter().any(|s| s.as_str() == *id);
            let configured = if *id == "searxng" {
                enabled
            } else {
                api_configured.get(*id).copied().unwrap_or(false)
            };
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
        let entries = self.entries.lock().unwrap();
        f.debug_struct("ProviderHealthRegistry")
            .field("entries_count", &entries.len())
            .finish()
    }
}

/// A skip reason for a single provider in the routing decision.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProviderSkipReason {
    /// The provider id that was skipped.
    pub provider_id: String,
    /// Human-readable reason for skipping.
    pub reason: String,
    /// The failure class if skipped due to a recent failure.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure_class: Option<String>,
    /// When cooldown expires, if skipped due to cooldown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cooldown_until: Option<String>,
}

/// Result of provider routing resolution.
#[derive(Clone, Debug, Serialize, Deserialize)]
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
                        reason: format!(
                            "in cooldown after {}",
                            snapshot.cooldown_reason.unwrap_or_default()
                        ),
                        failure_class: snapshot.recent_failure_class,
                        cooldown_until: snapshot.cooldown_until,
                    });
                    any_skipped = true;
                } else {
                    filtered.push(id.clone());
                }
            } else {
                skipped.push(ProviderSkipReason {
                    provider_id: id.clone(),
                    reason: "provider not built (missing API key or not configured)".to_string(),
                    failure_class: None,
                    cooldown_until: None,
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
                    reason: format!(
                        "in cooldown after {}",
                        snapshot.cooldown_reason.unwrap_or_default()
                    ),
                    failure_class: snapshot.recent_failure_class,
                    cooldown_until: snapshot.cooldown_until,
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
            if is_known && !config.enabled_provider_ids().contains(id) {
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
        hints: &crate::core::repo_query::RepoQueryHints,
        selected_providers: &[String],
    ) -> Self {
        let mut requested = Vec::new();
        let mut enforced = Vec::new();
        let mut approximated = Vec::new();
        let mut not_enforced = Vec::new();

        // Determine what capabilities are requested based on hints
        let wants_repo = hints.repo.is_some() || hints.owner.is_some();
        let wants_path = hints.path.is_some() || hints.file.is_some();
        let wants_language = hints.language.is_some();
        let wants_symbol = hints.symbol.is_some();

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

        if requested.is_empty() {
            return Self::default();
        }

        // Check which capabilities are enforced by selected providers
        let mut repo_enforced = false;
        let mut path_enforced = false;
        let mut lang_enforced = false;
        let mut symbol_enforced = false;

        for id in selected_providers {
            if let Some(desc) = built_in_provider_descriptor(id, true, false, true) {
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

        if requested.is_empty() {
            return Self::default();
        }

        let mut advisory_enforced = false;
        let mut package_enforced = false;

        for id in selected_providers {
            if let Some(desc) = built_in_provider_descriptor(id, true, false, true) {
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
    fn failure_class_roundtrip() {
        for class in [
            FailureClass::Timeout,
            FailureClass::HttpStatus,
            FailureClass::ParseError,
            FailureClass::NetworkError,
            FailureClass::RateLimited,
            FailureClass::Unknown,
        ] {
            let s = class.as_str();
            assert_eq!(FailureClass::from_str(s), class);
        }
    }

    #[test]
    fn capability_enforcement_empty_hints() {
        let hints = crate::core::repo_query::RepoQueryHints {
            ..Default::default()
        };
        let telemetry = CapabilityEnforcementTelemetry::for_repo_search(&hints, &[]);
        assert!(telemetry.requested.is_empty());
        assert!(telemetry.enforced.is_empty());
        assert!(telemetry.approximated.is_empty());
        assert!(telemetry.not_enforced.is_empty());
    }

    #[test]
    fn capability_enforcement_with_hints() {
        let hints = crate::core::repo_query::RepoQueryHints {
            repo: Some("axum".to_string()),
            path: Some("src/".to_string()),
            ..Default::default()
        };
        // No native providers — everything is approximated
        let telemetry =
            CapabilityEnforcementTelemetry::for_repo_search(&hints, &["duckduckgo".to_string()]);
        assert!(telemetry.requested.contains(&"repo_filter".to_string()));
        assert!(telemetry.requested.contains(&"path_filter".to_string()));
        assert!(telemetry.approximated.contains(&"repo_filter".to_string()));
        assert!(telemetry.approximated.contains(&"path_filter".to_string()));
    }

    #[test]
    fn capability_enforcement_native_provider() {
        let hints = crate::core::repo_query::RepoQueryHints {
            repo: Some("axum".to_string()),
            ..Default::default()
        };
        let telemetry =
            CapabilityEnforcementTelemetry::for_repo_search(&hints, &["github_code".to_string()]);
        assert!(telemetry.enforced.contains(&"repo_filter".to_string()));
    }

    #[test]
    fn capability_enforcement_symbol_not_enforced() {
        let hints = crate::core::repo_query::RepoQueryHints {
            symbol: Some("Router::layer".to_string()),
            ..Default::default()
        };
        let telemetry =
            CapabilityEnforcementTelemetry::for_repo_search(&hints, &["duckduckgo".to_string()]);
        assert!(telemetry.not_enforced.contains(&"symbol_hint".to_string()));
    }

    #[test]
    fn capability_enforcement_symbol_enforced() {
        let hints = crate::core::repo_query::RepoQueryHints {
            symbol: Some("Router::layer".to_string()),
            ..Default::default()
        };
        let telemetry =
            CapabilityEnforcementTelemetry::for_repo_search(&hints, &["github_code".to_string()]);
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
    fn routing_decision_serialization() {
        let decision = ProviderRoutingDecision {
            requested_profile: Some(SearchProfile::Coding),
            requested_providers: vec![],
            selected_providers: vec!["github_code".to_string()],
            skipped_providers: vec![ProviderSkipReason {
                provider_id: "gitlab_code".to_string(),
                reason: "not built".to_string(),
                failure_class: None,
                cooldown_until: None,
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
        assert_eq!(parsed.partial, true);
        assert_eq!(parsed.skipped_providers.len(), 1);
    }
}
