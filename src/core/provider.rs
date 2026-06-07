//! Provider capability model and descriptors.
//!
//! Each built-in search engine is described by a [`ProviderDescriptor`]
//! that captures its kind, configuration state, and feature capabilities.
//! MCP `provider_status` and the `eggsearch providers` CLI both
//! serialize these descriptors.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The set of provider ids that ship with the vendored engine
/// implementations.
pub const KNOWN_PROVIDER_IDS: &[&str] = &[
    "duckduckgo",
    "brave",
    "startpage",
    "yahoo",
    "mojeek",
    "searxng",
    "brave_api",
];

/// Whether the provider scrapes HTML or speaks a JSON API, or
/// requires an API key.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    /// HTML scraping (DuckDuckGo, Brave, Startpage, Yahoo, Mojeek).
    HtmlScrape,
    /// JSON API (SearXNG).
    JsonApi,
    /// Requires an operator-supplied API key (reserved for future use).
    ApiKey,
}

/// Feature capabilities that a provider may or may not support.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct ProviderCapabilities {
    /// Provider enforces safe-search filtering.
    pub supports_safe_search: bool,
    /// Provider supports a freshness / time-range parameter.
    pub supports_freshness: bool,
    /// Provider supports a language parameter.
    pub supports_language: bool,
    /// Provider supports a region / locale parameter.
    pub supports_region: bool,
    /// Provider supports restricting results to specific domains.
    pub supports_include_domains: bool,
    /// Provider supports excluding specific domains from results.
    pub supports_exclude_domains: bool,
    /// Provider supports a news-specific category.
    pub supports_news: bool,
}

impl ProviderCapabilities {
    /// A capabilities record where every field is `false`.
    pub fn none() -> Self {
        Self {
            supports_safe_search: false,
            supports_freshness: false,
            supports_language: false,
            supports_region: false,
            supports_include_domains: false,
            supports_exclude_domains: false,
            supports_news: false,
        }
    }

    /// Return a comma-separated list of enabled capability names.
    pub fn summary(&self) -> String {
        let mut caps = Vec::new();
        if self.supports_safe_search {
            caps.push("safe_search");
        }
        if self.supports_freshness {
            caps.push("freshness");
        }
        if self.supports_language {
            caps.push("language");
        }
        if self.supports_region {
            caps.push("region");
        }
        if self.supports_include_domains {
            caps.push("include_domains");
        }
        if self.supports_exclude_domains {
            caps.push("exclude_domains");
        }
        if self.supports_news {
            caps.push("news");
        }
        if caps.is_empty() {
            "basic".to_string()
        } else {
            caps.join(", ")
        }
    }
}

/// Full descriptor for a built-in provider, returned by
/// `provider_status` and the `eggsearch providers` CLI.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct ProviderDescriptor {
    /// Stable provider id, e.g. `"duckduckgo"`.
    pub id: String,
    /// Human-readable display name, e.g. `"DuckDuckGo"`.
    pub display_name: String,
    /// Kind of engine.
    pub kind: ProviderKind,
    /// Whether the provider is enabled in the server's effective config.
    pub enabled: bool,
    /// Whether the provider appears in the server's `default_providers`.
    pub default: bool,
    /// Whether the provider requires an API key.
    pub requires_api_key: bool,
    /// Whether the provider is fully configured (e.g. SearXNG has a
    /// non-empty `base_url`). Disabled providers are always reported as
    /// `configured: false`.
    pub configured: bool,
    /// Feature capabilities.
    pub capabilities: ProviderCapabilities,
}

/// Build a [`ProviderDescriptor`] for a known provider id.
///
/// Returns `None` for unknown ids. The `enabled`, `is_default`, and
/// `configured` flags are caller-supplied so the descriptor reflects
/// the actual server config state.
pub fn built_in_provider_descriptor(
    id: &str,
    enabled: bool,
    is_default: bool,
    configured: bool,
) -> Option<ProviderDescriptor> {
    match id {
        "duckduckgo" => Some(ProviderDescriptor {
            id: "duckduckgo".into(),
            display_name: "DuckDuckGo".into(),
            kind: ProviderKind::HtmlScrape,
            enabled,
            default: is_default,
            requires_api_key: false,
            configured,
            capabilities: ProviderCapabilities::none(),
        }),
        "brave" => Some(ProviderDescriptor {
            id: "brave".into(),
            display_name: "Brave".into(),
            kind: ProviderKind::HtmlScrape,
            enabled,
            default: is_default,
            requires_api_key: false,
            configured,
            capabilities: ProviderCapabilities::none(),
        }),
        "startpage" => Some(ProviderDescriptor {
            id: "startpage".into(),
            display_name: "Startpage".into(),
            kind: ProviderKind::HtmlScrape,
            enabled,
            default: is_default,
            requires_api_key: false,
            configured,
            capabilities: ProviderCapabilities::none(),
        }),
        "yahoo" => Some(ProviderDescriptor {
            id: "yahoo".into(),
            display_name: "Yahoo".into(),
            kind: ProviderKind::HtmlScrape,
            enabled,
            default: is_default,
            requires_api_key: false,
            configured,
            capabilities: ProviderCapabilities::none(),
        }),
        "mojeek" => Some(ProviderDescriptor {
            id: "mojeek".into(),
            display_name: "Mojeek".into(),
            kind: ProviderKind::HtmlScrape,
            enabled,
            default: is_default,
            requires_api_key: false,
            configured,
            capabilities: ProviderCapabilities::none(),
        }),
        "searxng" => Some(ProviderDescriptor {
            id: "searxng".into(),
            display_name: "SearXNG".into(),
            kind: ProviderKind::JsonApi,
            enabled,
            default: is_default,
            requires_api_key: false,
            configured: configured && enabled,
            capabilities: ProviderCapabilities {
                supports_safe_search: true,
                supports_freshness: true,
                supports_language: true,
                supports_region: true,
                supports_include_domains: false,
                supports_exclude_domains: false,
                supports_news: true,
            },
        }),
        "brave_api" => Some(ProviderDescriptor {
            id: "brave_api".into(),
            display_name: "Brave Search API".into(),
            kind: ProviderKind::ApiKey,
            enabled,
            default: is_default,
            requires_api_key: true,
            configured: configured && enabled,
            capabilities: ProviderCapabilities {
                supports_safe_search: true,
                supports_freshness: true,
                supports_language: true,
                supports_region: true,
                supports_include_domains: false,
                supports_exclude_domains: false,
                supports_news: false,
            },
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_provider_ids_are_all_describable() {
        for id in KNOWN_PROVIDER_IDS {
            let desc = built_in_provider_descriptor(id, true, false, true)
                .expect("known id should have descriptor");
            assert_eq!(desc.id, *id);
        }
    }

    #[test]
    fn unknown_provider_returns_none() {
        assert!(built_in_provider_descriptor("ghost", true, false, true).is_none());
    }

    #[test]
    fn capabilities_summary_basic() {
        let caps = ProviderCapabilities::none();
        assert_eq!(caps.summary(), "basic");
    }

    #[test]
    fn capabilities_summary_searxng() {
        let desc = built_in_provider_descriptor("searxng", true, false, true).unwrap();
        let summary = desc.capabilities.summary();
        assert!(summary.contains("safe_search"));
        assert!(summary.contains("language"));
        assert!(summary.contains("news"));
        assert!(!summary.contains("include_domains"));
    }

    #[test]
    fn searxng_configured_false_when_disabled() {
        let desc = built_in_provider_descriptor("searxng", false, false, true).unwrap();
        assert!(!desc.configured);
    }

    #[test]
    fn searxng_configured_true_when_enabled_and_configured() {
        let desc = built_in_provider_descriptor("searxng", true, false, true).unwrap();
        assert!(desc.configured);
    }

    #[test]
    fn provider_kind_serde_roundtrip() {
        let kind = ProviderKind::HtmlScrape;
        let json = serde_json::to_string(&kind).unwrap();
        let parsed: ProviderKind = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, kind);
    }

    #[test]
    fn provider_descriptor_serde_roundtrip() {
        let desc = built_in_provider_descriptor("duckduckgo", true, true, true).unwrap();
        let json = serde_json::to_string(&desc).unwrap();
        let parsed: ProviderDescriptor = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, desc.id);
        assert_eq!(parsed.kind, desc.kind);
        assert_eq!(parsed.enabled, desc.enabled);
        assert_eq!(parsed.default, desc.default);
        assert_eq!(parsed.capabilities, desc.capabilities);
    }

    #[test]
    fn brave_api_descriptor_is_api_key_kind() {
        let desc = built_in_provider_descriptor("brave_api", true, false, true)
            .expect("brave_api should have descriptor");
        assert_eq!(desc.id, "brave_api");
        assert_eq!(desc.display_name, "Brave Search API");
        assert_eq!(desc.kind, ProviderKind::ApiKey);
        assert!(desc.requires_api_key);
        assert!(desc.configured);
        assert!(desc.enabled);
        assert!(!desc.default);
    }

    #[test]
    fn brave_api_descriptor_configured_false_when_disabled() {
        let desc = built_in_provider_descriptor("brave_api", false, false, true).unwrap();
        assert!(!desc.configured);
        assert!(!desc.enabled);
    }

    #[test]
    fn brave_api_descriptor_capabilities() {
        let desc = built_in_provider_descriptor("brave_api", true, false, true).unwrap();
        assert!(desc.capabilities.supports_safe_search);
        assert!(desc.capabilities.supports_freshness);
        assert!(desc.capabilities.supports_language);
        assert!(desc.capabilities.supports_region);
        assert!(!desc.capabilities.supports_include_domains);
        assert!(!desc.capabilities.supports_exclude_domains);
        assert!(!desc.capabilities.supports_news);
    }

    #[test]
    fn brave_api_capabilities_summary() {
        let desc = built_in_provider_descriptor("brave_api", true, false, true).unwrap();
        let summary = desc.capabilities.summary();
        assert!(summary.contains("safe_search"));
        assert!(summary.contains("freshness"));
        assert!(summary.contains("language"));
        assert!(summary.contains("region"));
        assert!(!summary.contains("news"));
    }
}
