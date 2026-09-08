use super::common::*;
use crate::core::config::Mode;
use crate::core::provider::ProviderDescriptor;
use crate::mcp::policy::{fetch_allowed, live_allowed, Policy};
use crate::mcp::state::ServerState;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ProviderStatusArgs {
    /// Accepted for forward compatibility. The `provider_status` tool
    /// does not currently perform live network probes; when `true`, the
    /// response includes a `probe` field that explicitly states the
    /// probe is reserved for a future bounded implementation. Use
    /// `eggsearch doctor --probe` or the `live-smoke` test target for
    /// real network diagnostics in the meantime.
    #[serde(default)]
    pub probe: bool,
    /// Controls recipe verbosity in the response.
    /// `none`: omit workflow_recipes entirely.
    /// `summary` (default): compact summaries with id, title, goal, support, step_tools.
    /// `full`: full recipe objects with steps, fallbacks, trust_notes.
    #[serde(default)]
    pub recipe_detail: Option<crate::core::workflow::RecipeDetail>,
}

/// Run the `provider_status` tool.
pub fn run_provider_status(
    state: Arc<ServerState>,
    args: ProviderStatusArgs,
) -> Result<serde_json::Value, ToolError> {
    let mut descriptors: Vec<ProviderDescriptor> = state.adapter.provider_status();

    // Update local_workspace descriptor to reflect actual backend state
    if let Some(desc) = descriptors.iter_mut().find(|d| d.id == "local_workspace") {
        let backend_enabled = state.local_backend.is_some();
        desc.enabled = backend_enabled;
        desc.configured = backend_enabled;
        if backend_enabled {
            desc.routable = true;
            desc.skip_reason = None;
            desc.skip_code = None;
        }
    }

    let local_enabled = state.local_backend.is_some();

    // Build code_hosts summary from provider descriptors
    let code_hosts = build_code_hosts_summary(&descriptors);

    // Build health snapshots from the adapter's health registry
    let health_snapshots = state.adapter.health().all_snapshots(
        state.adapter.provider_ids(),
        state.adapter.searxng_configured(),
        state.adapter.api_configured(),
        state.local_backend.is_some(),
    );

    // Build per-provider health views for direct embedding
    let health_registry = state.adapter.health();
    let health_views: std::collections::BTreeMap<String, _> = descriptors
        .iter()
        .map(|d| (d.id.clone(), health_registry.health_view(&d.id)))
        .collect();

    let (browser_compiled, browser_configured, browser_discovered, browser_usable, browser_reason) = {
        #[cfg(feature = "browser")]
        {
            let compiled = true;
            let configured = state.config.fetch.browser.enabled;
            let discovery_state = &state.browser_discovery_state;
            let discovered = discovery_state.is_available();
            let usable = compiled && configured && discovered;
            let reason = if !configured {
                Some("disabled in config".to_string())
            } else {
                match discovery_state {
                    crate::fetch::browser::types::BrowserDiscoveryState::ExplicitPathInvalid {
                        path,
                    } => Some(format!("explicit path invalid: {path}")),
                    crate::fetch::browser::types::BrowserDiscoveryState::NotFound => {
                        Some("no Chrome/Chromium executable found".to_string())
                    }
                    crate::fetch::browser::types::BrowserDiscoveryState::NotConfigured => {
                        Some("no executable configured".to_string())
                    }
                    crate::fetch::browser::types::BrowserDiscoveryState::VersionUnsupported {
                        version,
                    } => Some(format!("browser version unsupported: {version}")),
                    crate::fetch::browser::types::BrowserDiscoveryState::Available(_) => None,
                }
            };
            (compiled, configured, discovered, usable, reason)
        }
        #[cfg(not(feature = "browser"))]
        {
            (false, false, false, false, Some("not compiled".to_string()))
        }
    };

    let (profiles_compiled, profiles_configured, profiles_usable, profiles_reason) = {
        #[cfg(feature = "browser")]
        {
            let compiled = true;
            let configured = state.config.fetch.browser.persistent_profiles.enabled;
            let usable = compiled && configured;
            let reason = if !configured {
                Some("disabled".to_string())
            } else {
                None
            };
            (compiled, configured, usable, reason)
        }
        #[cfg(not(feature = "browser"))]
        {
            (false, false, false, Some("not compiled".to_string()))
        }
    };

    let (pdf_compiled, pdf_configured, pdf_usable) = {
        #[cfg(feature = "pdf")]
        {
            let compiled = true;
            let configured = state.config.fetch.pdf_enabled;
            let usable = compiled && configured;
            (compiled, configured, usable)
        }
        #[cfg(not(feature = "pdf"))]
        {
            (false, false, false)
        }
    };

    let mut payload = serde_json::json!({
        "providers": descriptors,
        "code_hosts": code_hosts,
        "health": health_snapshots,
        "health_views": health_views,
        "probe": if args.probe {
            serde_json::json!({
                "requested": true,
                "implemented": false,
                "message": "provider_status.probe is reserved for a future bounded live probe; use `eggsearch doctor --probe` or the `live-smoke` test target for real network diagnostics in the meantime",
            })
        } else {
            serde_json::json!({
                "requested": false,
                "implemented": false,
            })
        },
        "mode": mode_str(state.config.search.mode),
        "server_capabilities": {
            "generic_search": matches!(live_allowed(state.config.search.mode), Policy::Allow),
            "explicit_fetch": matches!(fetch_allowed(state.config.fetch.enabled), Policy::Allow),
            "repo_search": matches!(live_allowed(state.config.search.mode), Policy::Allow)
                || local_enabled,
            "repo_fetch": matches!(fetch_allowed(state.config.fetch.enabled), Policy::Allow)
                || local_enabled,
            "repo_map": matches!(live_allowed(state.config.search.mode), Policy::Allow)
                || local_enabled,
            "security_search": matches!(live_allowed(state.config.search.mode), Policy::Allow),
            "research_search": matches!(live_allowed(state.config.search.mode), Policy::Allow),
            "batch_fetch": matches!(fetch_allowed(state.config.fetch.enabled), Policy::Allow),
            "evidence_bundle": true,
            "document_fetch": matches!(fetch_allowed(state.config.fetch.enabled), Policy::Allow),
            "pdf_fetch": pdf_compiled,
            "pdf_text": pdf_compiled,
            "pdf_layout": false,
            "pdf_ocr": false,
            "browser_rendering": browser_usable,
            "persistent_browser_profiles": profiles_usable,
            "local_workspace": local_enabled,
        },
        "browser_capabilities": {
            "compiled": browser_compiled,
            "configured": browser_configured,
            "discovered": browser_discovered,
            "usable": browser_usable,
            "reason": browser_reason,
        },
        "persistent_browser_profiles_capabilities": {
            "compiled": profiles_compiled,
            "configured": profiles_configured,
            "usable": profiles_usable,
            "reason": profiles_reason,
        },
        "pdf_capabilities": {
            "compiled": pdf_compiled,
            "configured": pdf_configured,
            "usable": pdf_usable,
            "layout": "deferred",
            "ocr": "deferred",
        },
        "cache_capabilities": {
            "memory_cache_enabled": state.fetch_cache.is_some(),
            "persistent_cache": false,
            "profile_scoping": true,
        },
        "quality_metadata": {
            "enabled": true,
            "per_result": true,
            "group_summary": true,
            "uses_model_judging": false,
        },
        "tool_capabilities": {
            "repo_fetch": {
                "remote_hosts": ["github", "gitlab", "codeberg", "gitea", "forgejo"],
                "workspace": local_enabled,
                "line_ranges": true,
                "context_lines": true,
                "max_chars_enforced": true,
                "symbol_search": true,
                "expand_to_block": true,
                "max_block_lines": true,
            },
            "repo_search": {
                "profiles": ["generic", "coding", "security", "research"],
                "package_resolution": ["crates_io", "pypi", "npm", "go", "maven", "nuget", "rubygems", "packagist", "oci", "github_actions"],
                "local_workspace": local_enabled,
                "repo_search_remote": matches!(live_allowed(state.config.search.mode), Policy::Allow),
                "repo_search_local": local_enabled,
                "subquery_telemetry": true,
                "supported_hosts": ["github", "gitlab", "codeberg", "gitea", "forgejo"],
            },
            "repo_map": {
                "supported_hosts": ["github", "gitlab", "codeberg", "gitea", "forgejo"],
                "local_checkout": local_enabled,
                "repo_map_remote": if matches!(live_allowed(state.config.search.mode), Policy::Allow) {
                    "native"
                } else {
                    "metadata_only"
                },
                "repo_map_local": local_enabled,
            },
            "local_workspace": {
                "enabled": local_enabled,
                "symbol_enrichment": "regex_heuristic",
            },
            "batch_fetch": {
                "enabled": state.config.fetch.enabled,
                "max_items": state.config.fetch.batch_max_items,
                "max_items_cap": state.config.fetch.batch_max_items_cap,
                "max_chars_per_item": state.config.fetch.batch_max_chars_per_item,
                "max_total_chars": state.config.fetch.batch_max_total_chars,
                "max_total_chars_cap": state.config.fetch.batch_max_total_chars_cap,
                "concurrency": state.config.fetch.batch_concurrency,
                "supports_web": true,
                "supports_repo": true,
                "preserves_item_trust": true,
            },
            "evidence_bundle": {
                "enabled": true,
                "summarizes": false,
                "persists": false,
                "max_sources": crate::core::evidence_bundle::MAX_SOURCES_CAP,
                "max_fetched_items": crate::core::evidence_bundle::MAX_FETCHED_ITEMS_CAP,
                "max_total_chars": crate::core::evidence_bundle::MAX_TOTAL_CHARS_CAP,
            },
        },
        "workflow_recipes": match args.recipe_detail.unwrap_or_default() {
            crate::core::workflow::RecipeDetail::None => serde_json::json!([]),
            crate::core::workflow::RecipeDetail::Summary => {
                let recipes = crate::meta::recipe_catalog::build_recipe_catalog(&descriptors, local_enabled);
                serde_json::json!(recipes.iter().map(|r| r.summarize()).collect::<Vec<_>>())
            }
            crate::core::workflow::RecipeDetail::Full => {
                serde_json::json!(crate::meta::recipe_catalog::build_recipe_catalog(&descriptors, local_enabled))
            }
        },
    });
    if let serde_json::Value::Object(map) = &mut payload {
        if matches!(
            args.recipe_detail.unwrap_or_default(),
            crate::core::workflow::RecipeDetail::None
        ) {
            map.remove("workflow_recipes");
        }
    }
    Ok(payload)
}

/// Build a `code_hosts` summary grouping providers by host kind.
///
/// Each host kind gets an entry with aggregated capability flags.
fn build_code_hosts_summary(descriptors: &[ProviderDescriptor]) -> Vec<serde_json::Value> {
    struct HostSummary {
        kind: String,
        id: String,
        enabled: bool,
        configured: bool,
        code_search: bool,
        issue_search: bool,
        release_search: bool,
    }

    let mut hosts: std::collections::BTreeMap<String, HostSummary> =
        std::collections::BTreeMap::new();

    for desc in descriptors {
        let kind = match desc.id.as_str() {
            "github_code" | "github_issues" | "github_releases" => "github",
            "gitlab_code" | "gitlab_issues" | "gitlab_releases" => "gitlab",
            "gitea_code" | "gitea_issues" | "gitea_releases" => "gitea",
            "forgejo_code" | "forgejo_issues" | "forgejo_releases" => "forgejo",
            _ => continue,
        };

        let host_kind = kind.to_string();
        let entry = hosts
            .entry(host_kind.clone())
            .or_insert_with(|| HostSummary {
                kind: host_kind,
                id: kind.to_string(),
                enabled: false,
                configured: false,
                code_search: false,
                issue_search: false,
                release_search: false,
            });

        entry.enabled = entry.enabled || desc.enabled;
        entry.configured = entry.configured || desc.configured;
        entry.code_search = entry.code_search || desc.capabilities.supports_code_search;
        entry.issue_search = entry.issue_search || desc.capabilities.supports_issue_search;
        entry.release_search = entry.release_search || desc.capabilities.supports_release_search;
    }

    hosts
        .into_values()
        .map(|h| {
            serde_json::json!({
                "kind": h.kind,
                "id": h.id,
                "enabled": h.enabled,
                "configured": h.configured,
                "capabilities": {
                    "code_search": h.code_search,
                    "issue_search": h.issue_search,
                    "release_search": h.release_search,
                },
            })
        })
        .collect()
}

fn mode_str(mode: Mode) -> &'static str {
    match mode {
        Mode::Off => "off",
        Mode::Live => "live",
    }
}
