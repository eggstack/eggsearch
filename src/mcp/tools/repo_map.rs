use super::common::*;
use crate::mcp::policy::{live_allowed, live_search_denied_message, Policy};
use crate::mcp::state::ServerState;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RepoMapArgs {
    /// Code host. Optional; accepted values: github (gh), gitlab (gl), codeberg (cb), gitea, forgejo.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    /// Repository owner.
    pub owner: String,
    /// Repository name.
    pub repo: String,
    /// Branch, tag, or commit ref. Defaults to repository default when omitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ref_name: Option<String>,
    /// Full commit SHA for stable permalink construction.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit_sha: Option<String>,
    /// Maximum root entries to return. Defaults to server config.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_entries: Option<usize>,
    /// Maximum directory depth to traverse. Defaults to server config.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_depth: Option<usize>,
    /// Whether to include file entries (default true).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_files: Option<bool>,
    /// Whether to include directory entries (default true).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_directories: Option<bool>,
    /// Whether to include CI configuration details (default true).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_ci: Option<bool>,
    /// Whether to include security policy details (default true).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_security: Option<bool>,
    /// Per-request timeout override in milliseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
    /// Explicit provider ID list.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub providers: Vec<String>,
}

fn build_forge_tree_config(
    state: &ServerState,
    host: crate::core::code_metadata::CodeHost,
) -> crate::meta::forge_adapter::ForgeTreeConfig {
    let (provider_id, default_base) = match host {
        crate::core::code_metadata::CodeHost::Github => ("github_code", None),
        crate::core::code_metadata::CodeHost::Gitlab => ("gitlab_code", None),
        crate::core::code_metadata::CodeHost::Codeberg => (
            "gitea_code",
            Some("https://codeberg.org/api/v1".to_string()),
        ),
        crate::core::code_metadata::CodeHost::Gitea => ("gitea_code", None),
        crate::core::code_metadata::CodeHost::Forgejo => ("gitea_code", None),
        crate::core::code_metadata::CodeHost::Unknown => ("", None),
    };

    let api_config = state.config.search.api.get(provider_id);
    let api_key = api_config
        .and_then(|c| c.api_key_env.as_deref())
        .and_then(|env| std::env::var(env).ok())
        .filter(|k| !k.is_empty());

    let base_url = api_config.and_then(|c| c.base_url.clone()).or(default_base);

    let endpoint_policy = crate::meta::forge_adapter::ForgeEndpointPolicy {
        allow_loopback: state.config.fetch.allow_localhost,
        allow_private_network: state.config.fetch.allow_private_network,
        require_https: true,
    };

    crate::meta::forge_adapter::ForgeTreeConfig {
        api_key,
        base_url,
        endpoint_policy,
        forge_budget_limit: None,
    }
}

/// Run the `repo_map` tool.
pub async fn run_repo_map(
    state: Arc<ServerState>,
    args: RepoMapArgs,
) -> Result<serde_json::Value, ToolError> {
    use crate::core::code_metadata::CodeHost;
    use crate::core::repo_map::RepoMapRequest;

    // Permit a local-only path when the local backend is enabled, even in
    // off mode, so air-gapped operators can inspect a configured local
    // checkout without enabling live metasearch.
    let local_only_path = matches!(live_allowed(state.config.search.mode), Policy::Deny)
        && state.local_backend.is_some();

    if matches!(live_allowed(state.config.search.mode), Policy::Deny) && !local_only_path {
        return Err(ToolError::Validation(live_search_denied_message(
            "repo_map",
        )));
    }

    let host = parse_code_host_arg(args.host.as_deref())?;

    let req = RepoMapRequest {
        query: String::new(),
        host,
        owner: args.owner,
        repo: args.repo,
        ref_name: args.ref_name,
        commit_sha: args.commit_sha,
        max_entries: args.max_entries,
        max_depth: args.max_depth,
        include_files: args.include_files,
        include_directories: args.include_directories,
        include_ci: args.include_ci,
        include_security: args.include_security,
        timeout_ms: args.timeout_ms,
        providers: args.providers,
    };

    if let Err(e) = req.validate() {
        return Err(ToolError::Validation(format!("invalid request: {e}")));
    }

    // Attempt native tree retrieval when a supported host is detected.
    let mut response = if let Some(host) = req.host {
        if crate::meta::forge_adapter::is_supported_host(host) {
            let forge_config = build_forge_tree_config(&state, host);
            match crate::meta::forge_adapter::fetch_tree(
                host,
                &req.owner,
                &req.repo,
                &req,
                &forge_config,
            )
            .await
            {
                Ok(forge_response) => {
                    let include_files = req.include_files.unwrap_or(true);
                    let include_directories = req.include_directories.unwrap_or(true);
                    let include_ci = req.include_ci.unwrap_or(true);
                    let include_security = req.include_security.unwrap_or(true);
                    let gitea_base = if matches!(host, CodeHost::Gitea | CodeHost::Forgejo) {
                        forge_config.base_url.as_deref().map(|api_base| {
                            crate::meta::forge_adapter::derive_gitea_instance_root(api_base)
                        })
                    } else {
                        None
                    };
                    crate::meta::forge_adapter::build_response(
                        &req,
                        forge_response,
                        include_files,
                        include_directories,
                        include_ci,
                        include_security,
                        gitea_base.as_deref(),
                    )
                }
                Err(_e) => {
                    let mut fallback = crate::meta::repo_mapper::build_fallback_response(&req);
                    let warning_code = if _e.contains("rate_limited") {
                        crate::core::warning::WarningCode::ForgeRateLimited
                    } else if _e.contains("authentication_required") {
                        crate::core::warning::WarningCode::ForgeAuthRequired
                    } else if _e.contains("repository_not_found") {
                        crate::core::warning::WarningCode::RepoRefNotFound
                    } else {
                        crate::core::warning::WarningCode::NoNativeTreeProvider
                    };
                    let deadline_exceeded = _e.contains("timed out")
                        || _e.contains("timeout")
                        || _e.contains("deadline");
                    fallback
                        .structured_warnings
                        .push(crate::core::warning::AgentWarning::new(
                            warning_code,
                            format!("forge tree adapter failed: {_e}"),
                        ));
                    if deadline_exceeded {
                        fallback.telemetry = Some(crate::core::repo_map::RepoMapTelemetry {
                            providers_queried: Vec::new(),
                            deadline_exceeded: true,
                            mode_reason: Some("forge tree request timed out".to_string()),
                            endpoint_origin: None,
                            redirect_rejected: false,
                            response_bytes_observed: None,
                            response_cap_applied: false,
                            dns_policy_class: None,
                            aggregate_byte_cap_reached: false,
                            aggregate_limit: None,
                            aggregate_remaining: None,
                            request_count: None,
                            exhausted_by: None,
                        });
                    }
                    fallback
                }
            }
        } else {
            let mut fallback = crate::meta::repo_mapper::build_fallback_response(&req);
            if host == CodeHost::Unknown {
                fallback
                    .structured_warnings
                    .push(crate::core::warning::AgentWarning::new(
                        crate::core::warning::WarningCode::ForgeTreeUnsupportedHost,
                        "host is not supported for native tree retrieval",
                    ));
            }
            fallback
        }
    } else {
        crate::meta::repo_mapper::build_fallback_response(&req)
    };

    // Discover local checkout for the requested repo
    let mut local_checkout_root: Option<std::path::PathBuf> = None;
    if let Some(backend) = state.local_backend.as_deref() {
        if backend.is_enabled() {
            let inventory = state.local_inventory().await;
            let matched = crate::meta::local_inventory::match_local_repo(
                &inventory,
                req.host.as_ref(),
                &req.owner,
                &req.repo,
            );
            if let Some(rid) = matched {
                local_checkout_root = Some(rid.root_path.clone());
                response.local_checkout = Some(crate::core::repo_map::RepoMapLocalCheckout {
                    root_name: rid.root_name.clone(),
                    root_path: rid.root_path.display().to_string(),
                    remote_host: rid
                        .matched_host
                        .as_ref()
                        .map(|h| format!("{h:?}").to_lowercase()),
                    remote_owner: rid.matched_owner.clone(),
                    remote_repo: rid.matched_repo.clone(),
                    branch: rid.current_branch.clone(),
                    commit: rid.current_commit.clone(),
                    dirty_state: rid.dirty_state.to_string(),
                    manifests: rid
                        .manifests
                        .iter()
                        .map(|m| crate::core::repo_map::RepoMapLocalManifest {
                            path: m.path.clone(),
                            ecosystem: m.ecosystem.to_string(),
                            package_name: m.package_name.clone(),
                        })
                        .collect(),
                });
                response
                    .warnings
                    .push(crate::core::result::SearchWarning::new(
                        "local_workspace",
                        format!(
                            "local_checkout_match: local checkout found for {}/{} at {}",
                            req.owner,
                            req.repo,
                            rid.root_path.display(),
                        ),
                    ));
                if rid.dirty_state == crate::meta::local_inventory::LocalDirtyState::Dirty {
                    response
                        .warnings
                        .push(crate::core::result::SearchWarning::new(
                            "local_workspace",
                            "local_repo_dirty: local checkout has uncommitted changes",
                        ));
                }
            }
        }
    }

    // Populate repo-map structure from the local checkout when available.
    if let Some(root) = local_checkout_root.as_deref() {
        crate::meta::repo_mapper::populate_from_local_checkout(&mut response, &req, root);
        crate::meta::repo_mapper::populate_structure_from_local_checkout(
            &mut response,
            root,
            &state.config.local,
        );
    }

    // Fallback subqueries are intentionally not generated because no
    // fallback discovery is performed without a native tree provider
    // or a matching local checkout. The single `no_native_tree_provider`
    // warning added by `build_fallback_response` is the authoritative
    // signal for this degraded mode.

    // Populate structured warnings from accumulated string warnings
    response.structured_warnings = crate::core::warning::convert_warnings(&response.warnings);

    let value = serde_json::to_value(&response)
        .map_err(|e| ToolError::internal(format!("serialization error: {e}")))?;
    Ok(value)
}
