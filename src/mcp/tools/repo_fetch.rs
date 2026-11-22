use super::common::*;
use crate::fetch::FetchClient;
use crate::mcp::policy::{fetch_allowed, web_fetch_denied_message, Policy};
use crate::mcp::state::ServerState;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub(crate) fn workspace_relative_path_arg(args: &RepoFetchArgs) -> Result<String, ToolError> {
    let path = args.path.trim();
    let legacy_repo_path = args.repo.trim();

    if !path.is_empty() {
        Ok(path.to_string())
    } else if !legacy_repo_path.is_empty() {
        Ok(legacy_repo_path.to_string())
    } else {
        Err(ToolError::Validation(
            "workspace fetch path must not be empty".to_string(),
        ))
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RepoFetchArgs {
    /// Code host. Optional; accepted values: github (gh), gitlab (gl), codeberg (cb), gitea, forgejo.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(description = "Code host: github, gitlab, codeberg, gitea, forgejo, or workspace.")]
    pub host: Option<String>,
    #[serde(flatten)]
    pub target: crate::core::repo_fetch::RepoTarget,
    /// Timeout in milliseconds. Defaults to server config.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(skip)]
    pub timeout_ms: Option<u64>,
    /// Override URL for the actual fetch (internal/test-only). When
    /// set, this URL is fetched instead of the internally-constructed
    /// raw URL. Hidden from the MCP tool schema.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(skip)]
    pub test_fetch_url: Option<String>,
    /// Response detail: brief, compact, standard, or diagnostic.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(skip)]
    pub response_detail: Option<crate::mcp::projection::ResponseDetail>,
}

impl std::ops::Deref for RepoFetchArgs {
    type Target = crate::core::repo_fetch::RepoTarget;

    fn deref(&self) -> &Self::Target {
        &self.target
    }
}

impl std::ops::DerefMut for RepoFetchArgs {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.target
    }
}

/// Run the `repo_fetch` tool.
pub async fn run_repo_fetch(
    state: Arc<ServerState>,
    args: RepoFetchArgs,
) -> Result<serde_json::Value, ToolError> {
    run_repo_fetch_with_host_scope(state, args, None).await
}

pub async fn run_repo_fetch_with_host_scope(
    state: Arc<ServerState>,
    mut args: RepoFetchArgs,
    host_local_scope: Option<serde_json::Value>,
) -> Result<serde_json::Value, ToolError> {
    use crate::core::code_evidence::infer_source_role;
    use crate::core::code_metadata::CodeHost;
    use crate::core::fetch::ExtractMode;
    use crate::core::repo_fetch::{
        apply_line_range, clamp_lines_to_max_chars, codeberg_browser_url, codeberg_raw_url,
        gitea_browser_url, gitea_raw_url, github_browser_url, github_permalink_url,
        github_raw_permalink_url, github_raw_url, gitlab_browser_url, gitlab_raw_url, FetchTrust,
        RepoFetchRequest, RepoFetchResponse,
    };

    let local_view = state
        .local_workspace_view(host_local_scope.as_ref())
        .map_err(ToolError::Validation)?;

    // --- workspace:// local file fetch (bypasses fetch policy) ---
    if args
        .host
        .as_deref()
        .is_some_and(|host| host.trim().eq_ignore_ascii_case("workspace"))
    {
        let path = workspace_relative_path_arg(&args)?;
        let target = crate::core::fetch_locator::resolve_fetch_target(
            crate::core::fetch_locator::FetchTarget::Repo {
                host: crate::core::fetch_locator::BatchRepoHost::Workspace,
                owner: &args.owner,
                repo: &args.repo,
                ref_name: args.ref_name.as_deref(),
                commit_sha: args.commit_sha.as_deref(),
                path: &path,
            },
        )
        .map_err(ToolError::Validation)?;
        let crate::core::fetch_locator::FetchLocator::Local(locator) = target else {
            return Err(ToolError::internal(
                "workspace fetch resolved to a non-local target",
            ));
        };
        args.owner = locator.root;
        args.path = locator.path;
        let detail = crate::mcp::projection::ResponseDetail::from_opt_for_profile(
            args.response_detail,
            state.config.mcp.schema_profile,
        );
        let max_chars = state.config.mcp.brief_max_chars;
        let value = run_workspace_fetch(state, args, local_view).await?;
        return Ok(crate::mcp::projection::project_with_budget(
            "repo_fetch",
            value,
            detail,
            max_chars,
        ));
    }

    // --- prefer_local: resolve to local workspace when enabled ---
    let mut prefer_local_skip_reason = None;
    if args.prefer_local.unwrap_or(false) {
        if host_local_scope.is_some() && !local_view.is_enabled() {
            return Err(ToolError::Validation(
                "local repository fetch was requested, but the authorized host scope has no local workspace".to_string(),
            ));
        }
        if let Some(backend) = local_view.backend() {
            if backend.is_enabled() {
                let inventory = local_view.inventory().await;
                let parsed_host = parse_code_host_arg(args.host.as_deref())?;
                let matched = crate::meta::local_inventory::match_local_repo(
                    &inventory,
                    parsed_host.as_ref(),
                    &args.owner,
                    &args.repo,
                );
                if let Some(rid) = matched {
                    let detail = crate::mcp::projection::ResponseDetail::from_opt_for_profile(
                        args.response_detail,
                        state.config.mcp.schema_profile,
                    );
                    let max_chars = state.config.mcp.brief_max_chars;
                    let ws_args = RepoFetchArgs {
                        host: Some("workspace".to_string()),
                        target: crate::core::repo_fetch::RepoTarget {
                            owner: rid.root_name.clone(),
                            repo: args.path.clone(),
                            path: args.path.clone(),
                            line_start: args.line_start,
                            line_end: args.line_end,
                            context_before: args.context_before,
                            context_after: args.context_after,
                            max_chars: args.max_chars,
                            symbol: args.symbol.clone(),
                            symbol_kind: args.symbol_kind.clone(),
                            match_text: args.match_text.clone(),
                            expand_to_block: args.expand_to_block,
                            max_block_lines: args.max_block_lines,
                            ..Default::default()
                        },
                        timeout_ms: args.timeout_ms,
                        test_fetch_url: None,
                        response_detail: None,
                    };
                    let value = run_workspace_fetch(state, ws_args, local_view).await?;
                    return Ok(crate::mcp::projection::project_with_budget(
                        "repo_fetch",
                        value,
                        detail,
                        max_chars,
                    ));
                }
                if host_local_scope.is_some() {
                    return Err(ToolError::Validation(
                        "local repository fetch was requested, but no matching repository exists in the authorized workspace".to_string(),
                    ));
                }
                prefer_local_skip_reason = Some("no matching local repository was found");
            } else {
                prefer_local_skip_reason = Some("the local workspace backend is disabled");
            }
        } else {
            prefer_local_skip_reason = Some("no local workspace backend is available");
        }
    }

    if matches!(fetch_allowed(state.config.fetch.enabled), Policy::Deny) {
        return Err(ToolError::Validation(web_fetch_denied_message()));
    }

    // Parse host.
    let host = parse_code_host_arg(args.host.as_deref())?;

    // Determine effective host: infer from owner/repo if not explicit.
    // For now we require an explicit host or default to GitHub.
    let effective_host = host.unwrap_or(CodeHost::Github);
    let ref_name = args.ref_name.clone().unwrap_or_else(|| "main".to_string());
    let parsed_symbol_kind = parse_symbol_kind_arg(args.symbol_kind.as_deref())?;

    let mut req = RepoFetchRequest::from(args.target.clone());
    req.host = Some(effective_host);
    req.timeout_ms = args.timeout_ms;
    req.target.ref_name = Some(ref_name.clone());

    req.validate(state.config.fetch.max_chars_cap)
        .map_err(ToolError::Validation)?;

    let target = crate::core::fetch_locator::resolve_fetch_target(
        crate::core::fetch_locator::FetchTarget::Repo {
            host: crate::core::fetch_locator::BatchRepoHost::Remote(effective_host),
            owner: &req.owner,
            repo: &req.repo,
            ref_name: req.ref_name.as_deref(),
            commit_sha: req.commit_sha.as_deref(),
            path: &req.path,
        },
    )
    .map_err(ToolError::Validation)?;
    let crate::core::fetch_locator::FetchLocator::Repo(locator) = target else {
        return Err(ToolError::internal(
            "remote repo fetch resolved to a non-repository target",
        ));
    };
    let effective_host = locator
        .host
        .ok_or_else(|| ToolError::internal("remote repo locator has no code host"))?;
    let owner = locator
        .owner
        .as_deref()
        .ok_or_else(|| ToolError::internal("remote repo locator has no owner"))?;
    let repo = locator
        .repo
        .as_deref()
        .ok_or_else(|| ToolError::internal("remote repo locator has no repository"))?;
    let path = locator.path.as_str();
    let rn = locator.ref_name.as_deref().unwrap_or("main");

    // Resolve a Gitea/Forgejo base URL from configured API providers.
    // Reused for normal URLs, browser permalinks, and raw permalinks so
    // a self-hosted instance configured as e.g. `gitea_code` produces
    // matching URLs across all three call sites.
    let gitea_or_forgejo_base_url = |host: CodeHost| -> Option<String> {
        let provider_id = match host {
            CodeHost::Gitea => "gitea",
            CodeHost::Forgejo => "forgejo",
            _ => return None,
        };
        state
            .config
            .search
            .api
            .get(provider_id)
            .and_then(|c| c.base_url.clone())
            .or_else(|| {
                // Fallback: try any gitea/forgejo provider with a base_url.
                state
                    .config
                    .search
                    .api
                    .iter()
                    .find(|(k, _)| k.starts_with("gitea_") || k.starts_with("forgejo_"))
                    .and_then(|(_, c)| c.base_url.clone())
            })
    };

    // Build URLs based on host.
    let (browser_url, raw_url) = match effective_host {
        CodeHost::Github => {
            let browser = github_browser_url(owner, repo, rn, path);
            let raw = github_raw_url(owner, repo, rn, path);
            (browser, raw)
        }
        CodeHost::Gitlab => {
            let browser = gitlab_browser_url(owner, repo, rn, path);
            let raw = gitlab_raw_url(owner, repo, rn, path);
            (browser, raw)
        }
        CodeHost::Codeberg => {
            let browser = codeberg_browser_url(owner, repo, rn, path);
            let raw = codeberg_raw_url(owner, repo, rn, path);
            (browser, raw)
        }
        CodeHost::Gitea | CodeHost::Forgejo => {
            let base_url = gitea_or_forgejo_base_url(effective_host).ok_or_else(|| {
                let provider_id = match effective_host {
                    CodeHost::Gitea => "gitea",
                    CodeHost::Forgejo => "forgejo",
                    _ => "gitea",
                };
                ToolError::Validation(format!(
                    "host '{effective_host:?}' requires a configured base_url in [search.api.{provider_id}] or [search.api.<id>] with a base_url"
                ))
            })?;
            let browser = gitea_browser_url(&base_url, owner, repo, rn, path);
            let raw = gitea_raw_url(&base_url, owner, repo, rn, path);
            (browser, raw)
        }
        CodeHost::Unknown => {
            return Err(ToolError::Validation(format!(
                "host '{effective_host:?}' is not supported for repo_fetch"
            )));
        }
    };

    let permalink_url = req.commit_sha.as_ref().map(|sha| {
        match effective_host {
            CodeHost::Github => github_permalink_url(owner, repo, sha, path),
            CodeHost::Gitlab => {
                // GitLab permalink uses the browser URL pattern with SHA.
                gitlab_browser_url(owner, repo, sha, path)
            }
            CodeHost::Codeberg => {
                // Codeberg permalink uses the browser URL pattern with commit SHA.
                format!("https://codeberg.org/{owner}/{repo}/src/commit/{sha}/{path}")
            }
            CodeHost::Gitea | CodeHost::Forgejo => {
                // Gitea/Forgejo permalink uses the browser URL pattern with commit SHA.
                let base = gitea_or_forgejo_base_url(effective_host)
                    .unwrap_or_default()
                    .trim_end_matches('/')
                    .to_string();
                format!("{base}/{owner}/{repo}/src/commit/{sha}/{path}")
            }
            _ => raw_url.clone(),
        }
    });

    let raw_permalink_url = req.commit_sha.as_ref().map(|sha| {
        match effective_host {
            CodeHost::Github => github_raw_permalink_url(owner, repo, sha, path),
            CodeHost::Gitlab => {
                // GitLab raw permalink uses the raw URL pattern with SHA.
                gitlab_raw_url(owner, repo, sha, path)
            }
            CodeHost::Codeberg => {
                // Codeberg raw permalink uses the raw URL pattern with commit SHA.
                format!("https://codeberg.org/{owner}/{repo}/raw/commit/{sha}/{path}")
            }
            CodeHost::Gitea | CodeHost::Forgejo => {
                // Gitea/Forgejo raw permalink uses the raw URL pattern with commit SHA.
                let base = gitea_or_forgejo_base_url(effective_host)
                    .unwrap_or_default()
                    .trim_end_matches('/')
                    .to_string();
                format!("{base}/{owner}/{repo}/raw/commit/{sha}/{path}")
            }
            _ => raw_url.clone(),
        }
    });

    let locator = locator.clone();

    let language = crate::core::code_metadata::language_from_extension(path).map(String::from);
    let source_role = infer_source_role(path);

    let base_client: Arc<FetchClient> = state.fetch_client().ok_or_else(|| {
        ToolError::internal("fetch client unavailable; is [fetch].enabled = true?".to_string())
    })?;

    // Use per-request timeout override when provided.
    let client: Arc<FetchClient> =
        if let Some(ms) = req.timeout_ms {
            Arc::new(base_client.with_timeout_ms(ms).map_err(|e| {
                ToolError::internal(format!("failed to create timeout override: {e}"))
            })?)
        } else {
            base_client
        };

    // When commit_sha is provided, prefer the stable raw permalink URL
    // for exact evidence retrieval. Test override always wins.
    let cloned_permalink = raw_permalink_url.clone();
    let canonical_fetch_url = cloned_permalink.as_deref().unwrap_or(&raw_url);
    let fetch_url = args
        .test_fetch_url
        .as_deref()
        .unwrap_or(canonical_fetch_url);

    // Fetch up to the configured `max_chars_cap` so line/span
    // selection operates on full source text. The user-requested
    // `max_chars` is applied as an *output* budget via
    // `clamp_lines_to_max_chars` after span slicing. Source lines
    // must be parsed from `resp.raw_text` (Tier 1 only) rather than
    // `resp.text` (Tier 2 framed) so trust markers don't shift line
    // numbers.
    let fetch_max_chars = state.config.fetch.max_chars_cap;
    let challenge_deadline = std::time::Instant::now()
        + std::time::Duration::from_millis(req.timeout_ms.unwrap_or(state.config.fetch.timeout_ms));
    let challenge_plan = crate::challenge_replay::ChallengePlan {
        challenge: &state.config.search.challenge,
        user_agent: &state.config.fetch.user_agent,
        origin_controller: state.origin_controller.as_deref(),
        max_chars: Some(fetch_max_chars),
        extract_mode: ExtractMode::Text,
        include_links: false,
        pdf_options: None,
        deadline: challenge_deadline,
    };
    let response = crate::challenge_replay::fetch_with_challenge_replay(
        client.as_ref(),
        fetch_url,
        &challenge_plan,
    )
    .await;

    match response {
        Ok(resp) => {
            let status = resp.status;
            let content_type = resp.content_type.clone();
            let mut truncated = resp.truncated;
            let warnings = resp.warnings.clone();
            let mut trust_markers = resp.trust_markers.clone();

            // Parse lines from raw (Tier-1, unframed) text for line
            // slicing. Falls back to empty when raw_text is absent
            // (e.g. MetadataOnly).
            let raw_text = resp.raw_text.clone();
            let all_lines: Vec<String> = raw_text
                .as_deref()
                .unwrap_or("")
                .lines()
                .map(String::from)
                .collect();
            let total_lines = if all_lines.is_empty() {
                None
            } else {
                Some(u32::try_from(all_lines.len()).unwrap_or(u32::MAX))
            };

            // Apply span selection: resolve symbol/match_text/explicit range
            // to a concrete line span before slicing.
            let selected_span = crate::fetch::span::select_span(
                &all_lines,
                language.as_deref(),
                req.symbol.as_deref(),
                parsed_symbol_kind,
                req.match_text.as_deref(),
                req.line_start,
                req.line_end,
                req.expand_to_block.unwrap_or(false),
                req.max_block_lines,
            );

            // Use selected span line range when span selection produced
            // a result, overriding explicit request line range.
            let (effective_line_start, effective_line_end) = if let Some(ref span) = selected_span {
                (Some(span.line_start), Some(span.line_end))
            } else {
                (req.line_start, req.line_end)
            };

            // Apply line range.
            let (sliced_lines, _returned_start, _returned_end, line_truncated, line_warning) =
                apply_line_range(
                    &all_lines,
                    effective_line_start,
                    effective_line_end,
                    req.context_before.unwrap_or(0),
                    req.context_after.unwrap_or(0),
                );

            // Clamp sliced lines to user-requested `max_chars` budget.
            // When omitted, fall back to the configured `fetch.max_chars_default`
            // so callers cannot bypass the documented output budget by
            // omitting the field.
            let output_max_chars = req.max_chars.or(Some(state.config.fetch.max_chars_default));
            let (clamped_lines, char_truncated) = {
                let (lines, _txt, ct) = clamp_lines_to_max_chars(&sliced_lines, output_max_chars);
                (lines, ct)
            };

            let fetch_text_truncated = trust_markers.text_truncated;

            // Build text from clamped lines (unframed source, like
            // workspace_fetch).
            let sliced_text = if clamped_lines.is_empty() {
                None
            } else {
                let t: String = clamped_lines
                    .iter()
                    .map(|l| l.text.as_str())
                    .collect::<Vec<_>>()
                    .join("\n");
                Some(t)
            };

            // Detect explicit line-range clamps even when span
            // selection absorbed them before `apply_line_range` saw
            // the original out-of-bounds request. This covers both the
            // `(Some, Some)` and one-sided override cases.
            let requested_range_clamped = match (req.line_start, req.line_end) {
                (Some(s), Some(e)) => s > total_lines.unwrap_or(0) || e > total_lines.unwrap_or(0),
                (Some(s), None) => s > total_lines.unwrap_or(0),
                (None, Some(e)) => e > total_lines.unwrap_or(0),
                (None, None) => false,
            };

            let mut warnings = warnings;
            if let Some(reason) = prefer_local_skip_reason {
                warnings.push(format!(
                    "prefer_local_skipped: local copy was skipped because {reason}"
                ));
            }
            if fetch_text_truncated {
                warnings.push("remote_repo_fetch_truncated_by_fetch_cap".to_string());
            }
            if let Some(w) = line_warning {
                warnings.push(w);
            }
            if requested_range_clamped {
                warnings.push("remote_repo_fetch_line_range_clamped".to_string());
            }
            if char_truncated {
                warnings.push("remote_repo_fetch_truncated_by_max_chars".to_string());
            }
            if selected_span.is_none() && (req.symbol.is_some() || req.match_text.is_some()) {
                warnings.push(format!(
                    "span_selection: no match found for {}",
                    if req.symbol.is_some() {
                        format!("symbol '{}'", req.symbol.as_deref().unwrap_or(""))
                    } else {
                        format!("match_text '{}'", req.match_text.as_deref().unwrap_or(""))
                    }
                ));
            }

            let target_line = effective_line_start.or(req.line_start);
            let code_context = Some(crate::core::code_context::extract_code_context(
                sliced_text.as_deref().unwrap_or(""),
                path,
                target_line,
            ));

            // Propagate text-level extraction truncation (Tier 1
            // length bounding at `fetch_max_chars`) into the boolean
            // `truncated` flag so callers know the file may have been
            // longer than what was sliced. Also OR in line-range
            // clamping so callers who rely on `truncated` to decide
            // whether the evidence is complete see true when either
            // the source was capped or the requested line range was
            // clamped at EOF.
            if char_truncated {
                truncated = true;
                trust_markers.text_truncated = true;
            }
            truncated =
                truncated || fetch_text_truncated || line_truncated || requested_range_clamped;

            // Build deterministic code span evidence when span selection produced a result.
            let locator_str_for_span = format!("{locator:?}");
            let code_span = selected_span.as_ref().map(|span| {
                use crate::core::identity::code_span_id;
                use crate::core::repo_fetch::CodeSpanEvidence;
                let id = code_span_id(
                    &locator_str_for_span,
                    Some(span.line_start),
                    Some(span.line_end),
                    span.symbol.as_deref(),
                );
                let imports = code_context
                    .as_ref()
                    .map(|c| c.imports.clone())
                    .unwrap_or_default();
                CodeSpanEvidence {
                    span_id: id,
                    language: code_context.as_ref().and_then(|c| c.language.clone()),
                    line_start: Some(span.line_start),
                    line_end: Some(span.line_end),
                    symbol: span.symbol.clone(),
                    symbol_kind: span.symbol_kind.as_ref().map(|k| format!("{k:?}")),
                    selection_kind: format!("{:?}", span.selection_kind),
                    confidence: format!("{:?}", span.confidence),
                    source_id: None,
                    fetch_id: None,
                    path: Some(path.to_string()),
                    source_role: Some(source_role),
                    imports,
                    trust: Some(FetchTrust::ExternalUntrusted),
                    permalink_url: permalink_url.clone(),
                    raw_permalink_url: raw_permalink_url.clone(),
                }
            });

            let mut structured_warnings = crate::core::warning::convert_fetch_warnings(&warnings);
            structured_warnings.extend(
                resp.structured_warnings
                    .iter()
                    .filter(|warning| {
                        warning.code == crate::core::warning::WarningCode::FetchChallengePage
                    })
                    .cloned(),
            );

            let fetch_response = RepoFetchResponse {
                locator: locator.clone(),
                stable_id: Some(crate::core::identity::fetch_id(
                    None,
                    Some(&locator),
                    clamped_lines.first().map(|l| l.number),
                    clamped_lines.last().map(|l| l.number),
                    sliced_text.as_deref(),
                )),
                source_id: None,
                fetched: resp.fetched,
                status: Some(status),
                content_type,
                language,
                source_role: Some(source_role),
                browser_url,
                raw_url: raw_url.clone(),
                permalink_url,
                raw_permalink_url,
                fetched_url: Some(fetch_url.to_string()),
                ref_resolved: Some(rn.to_string()),
                line_start: req.line_start,
                line_end: req.line_end,
                returned_line_start: clamped_lines.first().map(|l| l.number),
                returned_line_end: clamped_lines.last().map(|l| l.number),
                total_lines,
                text: sliced_text,
                lines: clamped_lines,
                document: resp.document,
                truncated,
                structured_warnings,
                warnings,
                manual_interaction_required: resp.manual_interaction_required,
                trust: FetchTrust::ExternalUntrusted,
                trust_markers,
                selected_span,
                code_span,
                code_context,
            };

            let value = serde_json::to_value(&fetch_response)
                .map_err(|e| ToolError::internal(format!("serialization error: {e}")))?;
            let detail = crate::mcp::projection::ResponseDetail::from_opt_for_profile(
                args.response_detail,
                state.config.mcp.schema_profile,
            );
            Ok(crate::mcp::projection::project_with_budget(
                "repo_fetch",
                value,
                detail,
                state.config.mcp.brief_max_chars,
            ))
        }
        Err(e) => Err(ToolError::internal(match prefer_local_skip_reason {
            Some(reason) => format!(
                "{}: {}; prefer_local_skipped: local copy was skipped because {reason}",
                e.error_code(),
                e
            ),
            None => format!("{}: {}", e.error_code(), e),
        })),
    }
}

pub(crate) async fn validate_workspace_fetch_path(
    root_path: &std::path::Path,
    relative_path: &str,
    local_config: &crate::core::local::LocalConfig,
) -> Result<Option<u64>, ToolError> {
    crate::core::local::validate_local_fetch_path_structure(root_path, relative_path, local_config)
        .map_err(|error| ToolError::Validation(error.to_string()))?;

    let probe_root = root_path.to_path_buf();
    let probe_relative_path = relative_path.to_string();
    let probe_config = local_config.clone();
    let (prefix, file_size) = tokio::task::spawn_blocking(move || {
        crate::meta::safe_open::safe_read_file_prefix(
            &probe_root,
            &probe_relative_path,
            &probe_config,
            usize::MAX,
            5,
        )
    })
    .await
    .map_err(|error| ToolError::internal(format!("failed to join file inspection task: {error}")))?
    .map_err(|error| ToolError::internal(format!("failed to inspect file: {error}")))?;

    if crate::core::local::is_pdf_extension(relative_path) || prefix.starts_with(b"%PDF-") {
        let max_file_bytes = local_config.pdf_max_file_bytes;
        if file_size > max_file_bytes as u64 {
            return Err(ToolError::Validation(format!(
                "file size {file_size} exceeds max {max_file_bytes}"
            )));
        }
        return Ok(Some(file_size));
    }

    crate::core::local::validate_local_fetch_path(root_path, relative_path, local_config)
        .map_err(|error| ToolError::Validation(error.to_string()))?;
    Ok(None)
}

async fn run_workspace_fetch(
    state: Arc<ServerState>,
    args: RepoFetchArgs,
    local_view: Arc<crate::mcp::state::LocalWorkspaceView>,
) -> Result<serde_json::Value, ToolError> {
    use crate::core::code_evidence::infer_source_role;
    use crate::core::repo_fetch::{
        apply_line_range, clamp_lines_to_max_chars, FetchTrust, RepoFetchRequest, RepoFetchResponse,
    };
    use crate::core::sanitize::TrustMarkers;

    let backend = local_view.backend().ok_or_else(|| {
        ToolError::Validation("local workspace search is not enabled".to_string())
    })?;

    if !backend.is_enabled() {
        return Err(ToolError::Validation(
            "local workspace search is not enabled".to_string(),
        ));
    }

    let root_name = args.owner.clone();
    let relative_path = workspace_relative_path_arg(&args)?;

    let parsed_symbol_kind = parse_symbol_kind_arg(args.symbol_kind.as_deref())?;

    // Share budget/span validation with the remote repo_fetch path
    // so the workspace host enforces identical constraints (line ranges,
    // context bounds, max_chars cap, max_block_lines, timeout_ms).
    let mut ws_target = args.target.clone();
    ws_target.owner = root_name.clone();
    ws_target.repo = relative_path.clone();
    ws_target.ref_name = None;
    ws_target.commit_sha = None;
    ws_target.path = relative_path.clone();
    ws_target.prefer_local = None;
    let mut ws_req = RepoFetchRequest::from(ws_target);
    ws_req.host = None;
    ws_req.timeout_ms = args.timeout_ms;
    ws_req
        .validate(state.config.fetch.max_chars_cap)
        .map_err(ToolError::Validation)?;

    // Find the root by name
    let roots = backend.roots();
    let root_entry = roots.iter().find(|(_, p)| {
        p.file_name()
            .and_then(|n| n.to_str())
            .map(|n| n == root_name)
            .unwrap_or(false)
    });

    let (_, root_path) = root_entry.ok_or_else(|| {
        ToolError::Validation(format!(
            "unknown workspace root '{root_name}'; available roots: {}",
            roots
                .iter()
                .filter_map(|(_, p)| p.file_name().and_then(|n| n.to_str()))
                .collect::<Vec<_>>()
                .join(", ")
        ))
    })?;

    let root_path = root_path.clone();
    let local_config = backend.config().clone();
    if let Some(file_size) =
        validate_workspace_fetch_path(&root_path, &relative_path, &local_config).await?
    {
        return run_workspace_pdf_fetch(
            state,
            args,
            root_path,
            relative_path,
            local_config,
            file_size,
        )
        .await;
    }

    let relative_path_for_read = relative_path.clone();
    let max_file_bytes = local_config.max_file_bytes;
    let content = tokio::task::spawn_blocking(move || {
        let bytes = crate::meta::safe_open::safe_read_file(
            &root_path,
            &relative_path_for_read,
            &local_config,
            max_file_bytes,
        )
        .map_err(|error| std::io::Error::other(error.to_string()))?;
        String::from_utf8(bytes)
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))
    })
    .await
    .map_err(|e| ToolError::internal(format!("failed to join read task: {e}")))?
    .map_err(|e| ToolError::internal(format!("failed to read file: {e}")))?;

    let all_lines: Vec<String> = content.lines().map(String::from).collect();
    let total_lines = if all_lines.is_empty() {
        None
    } else {
        Some(u32::try_from(all_lines.len()).unwrap_or(u32::MAX))
    };

    let language =
        crate::core::code_metadata::language_from_extension(&relative_path).map(String::from);

    // Apply span selection: resolve symbol/match_text/explicit range
    // to a concrete line span before slicing.
    let selected_span = crate::fetch::span::select_span(
        &all_lines,
        language.as_deref(),
        args.symbol.as_deref(),
        parsed_symbol_kind,
        args.match_text.as_deref(),
        args.line_start,
        args.line_end,
        args.expand_to_block.unwrap_or(false),
        args.max_block_lines,
    );

    // Use selected span line range when span selection produced
    // a result, overriding explicit request line range.
    let (effective_line_start, effective_line_end) = if let Some(ref span) = selected_span {
        (Some(span.line_start), Some(span.line_end))
    } else {
        (args.line_start, args.line_end)
    };

    // Apply line range
    let (sliced_lines, _returned_start, _returned_end, _line_truncated, line_warning) =
        apply_line_range(
            &all_lines,
            effective_line_start,
            effective_line_end,
            args.context_before.unwrap_or(0),
            args.context_after.unwrap_or(0),
        );

    // Build text from sliced lines, enforcing max_chars budget.
    // When omitted, fall back to the configured `fetch.max_chars_default`
    // so callers cannot bypass the documented output budget by
    // omitting the field.
    let output_max_chars = args
        .max_chars
        .or(Some(state.config.fetch.max_chars_default));
    let (mut clamped_lines, _initial_text, char_truncated) =
        clamp_lines_to_max_chars(&sliced_lines, output_max_chars);

    let mut warnings: Vec<String> = Vec::new();
    if let Some(w) = line_warning {
        warnings.push(w);
    }
    if char_truncated {
        warnings.push("workspace_fetch_truncated_by_max_chars".to_string());
    }
    if selected_span.is_none() && (args.symbol.is_some() || args.match_text.is_some()) {
        warnings.push(format!(
            "span_selection: no match found for {}",
            if args.symbol.is_some() {
                format!("symbol '{}'", args.symbol.as_deref().unwrap_or(""))
            } else {
                format!("match_text '{}'", args.match_text.as_deref().unwrap_or(""))
            }
        ));
    }

    let source_role = infer_source_role(&relative_path);

    let pseudo_url = format!("workspace://{root_name}/{relative_path}");

    let locator = crate::core::repo_fetch::RepoLocator {
        kind: crate::core::repo_fetch::RepoLocatorKind::Workspace,
        host: None,
        owner: None,
        repo: None,
        ref_name: None,
        commit_sha: None,
        path: relative_path.clone(),
        workspace_root: Some(root_name.clone()),
    };

    let truncated = char_truncated;

    // Apply local trust-marker scanning: strip control chars and
    // scan for injection markers. Do NOT frame local source code
    // (no <<<EXTERNAL_UNTRUSTED>>> wrappers) — source lines must
    // remain intact for agent copy-paste.
    let mut trust_markers = TrustMarkers::default();
    // Strip control chars from individual lines
    let mut total_control_removed = 0usize;
    for line in &mut clamped_lines {
        let (cleaned, removed) = crate::core::sanitize::strip_control_chars(&line.text);
        total_control_removed += removed;
        line.text = cleaned;
    }
    trust_markers.control_chars_removed = total_control_removed;
    if total_control_removed > 0 {
        trust_markers.text_sanitized = true;
    }
    // Rebuild text from cleaned lines
    let sliced_text = if clamped_lines.is_empty() {
        None
    } else {
        Some(
            clamped_lines
                .iter()
                .map(|l| l.text.as_str())
                .collect::<Vec<_>>()
                .join("\n"),
        )
    };

    let target_line = effective_line_start.or(args.line_start);
    let code_context = Some(crate::core::code_context::extract_code_context(
        sliced_text.as_deref().unwrap_or(""),
        &relative_path,
        target_line,
    ));

    // Scan for injection markers in the full text
    if state.config.fetch.sanitize_output {
        if let Some(ref text) = sliced_text {
            let hits = crate::core::sanitize::scan_injection_markers(text);
            trust_markers.injection_hits = hits.len();
            if !hits.is_empty() {
                warnings.push(format!(
                    "local_content_marker_warning: possible prompt injection \
                     markers detected in local workspace content ({} hits)",
                    hits.len()
                ));
            }
        }
    }

    // Build deterministic code span evidence when span selection produced a result.
    let locator_str_for_span = format!("{locator:?}");
    let code_span = selected_span.as_ref().map(|span| {
        use crate::core::identity::code_span_id;
        use crate::core::repo_fetch::CodeSpanEvidence;
        let id = code_span_id(
            &locator_str_for_span,
            Some(span.line_start),
            Some(span.line_end),
            span.symbol.as_deref(),
        );
        let imports = code_context
            .as_ref()
            .map(|c| c.imports.clone())
            .unwrap_or_default();
        CodeSpanEvidence {
            span_id: id,
            language: code_context.as_ref().and_then(|c| c.language.clone()),
            line_start: Some(span.line_start),
            line_end: Some(span.line_end),
            symbol: span.symbol.clone(),
            symbol_kind: span.symbol_kind.as_ref().map(|k| format!("{k:?}")),
            selection_kind: format!("{:?}", span.selection_kind),
            confidence: format!("{:?}", span.confidence),
            source_id: None,
            fetch_id: None,
            path: Some(relative_path.clone()),
            source_role: Some(source_role),
            imports,
            trust: Some(FetchTrust::LocalTrusted),
            permalink_url: None,
            raw_permalink_url: None,
        }
    });

    let fetch_response = RepoFetchResponse {
        locator: locator.clone(),
        stable_id: Some(crate::core::identity::fetch_id(
            None,
            Some(&locator),
            clamped_lines.first().map(|l| l.number),
            clamped_lines.last().map(|l| l.number),
            sliced_text.as_deref(),
        )),
        source_id: None,
        fetched: true,
        status: None,
        content_type: None,
        language,
        source_role: Some(source_role),
        browser_url: pseudo_url.clone(),
        raw_url: pseudo_url.clone(),
        permalink_url: None,
        raw_permalink_url: None,
        fetched_url: None,
        ref_resolved: None,
        line_start: args.line_start,
        line_end: args.line_end,
        returned_line_start: clamped_lines.first().map(|l| l.number),
        returned_line_end: clamped_lines.last().map(|l| l.number),
        total_lines,
        text: sliced_text,
        lines: clamped_lines,
        document: None,
        truncated,
        structured_warnings: crate::core::warning::convert_fetch_warnings(&warnings),
        warnings,
        manual_interaction_required: false,
        trust: FetchTrust::LocalTrusted,
        trust_markers,
        selected_span,
        code_span,
        code_context,
    };

    let value = serde_json::to_value(&fetch_response)
        .map_err(|e| ToolError::internal(format!("serialization error: {e}")))?;
    Ok(value)
}

async fn run_workspace_pdf_fetch(
    state: Arc<ServerState>,
    args: RepoFetchArgs,
    root_path: std::path::PathBuf,
    relative_path: String,
    local_config: crate::core::local::LocalConfig,
    file_size: u64,
) -> Result<serde_json::Value, ToolError> {
    let max_file_bytes = local_config.pdf_max_file_bytes;
    if file_size > max_file_bytes as u64 {
        return Err(ToolError::Validation(format!(
            "file size {file_size} exceeds max {max_file_bytes}"
        )));
    }

    #[cfg(feature = "pdf")]
    {
        if !state.config.fetch.pdf_enabled {
            return Err(ToolError::capability_unavailable(
                "pdf_disabled: set [fetch].pdf_enabled = true; see docs/features.md#pdf-extraction",
            ));
        }

        let mut read_config = local_config.clone();
        read_config.max_file_bytes = max_file_bytes;
        let read_root = root_path.clone();
        let read_relative_path = relative_path.clone();
        let bytes = tokio::task::spawn_blocking(move || {
            crate::meta::safe_open::safe_read_file(
                &read_root,
                &read_relative_path,
                &read_config,
                max_file_bytes,
            )
        })
        .await
        .map_err(|e| ToolError::internal(format!("failed to join PDF read task: {e}")))?
        .map_err(|error| match error {
            crate::meta::safe_open::SafeOpenError::FileTooLarge(size, max) => {
                ToolError::Validation(format!("file size {size} exceeds max {max}"))
            }
            crate::meta::safe_open::SafeOpenError::FileContentLimitExceeded(max, observed) => {
                ToolError::Validation(format!(
                    "file content exceeds max {max} bytes (observed {observed})"
                ))
            }
            other => ToolError::internal(format!("failed to read local PDF: {other}")),
        })?;

        let output_max_chars = args
            .max_chars
            .unwrap_or(state.config.fetch.max_chars_default)
            .min(state.config.fetch.max_chars_cap);
        let fetch_limits = state.config.fetch_limits();
        let pdf_limits = crate::fetch::pdf::PdfLimits {
            max_pages: fetch_limits.pdf_max_pages,
            max_chars_per_page: fetch_limits.pdf_max_chars_per_page,
            max_total_chars: fetch_limits.pdf_max_total_chars,
            max_decompressed_bytes: fetch_limits.pdf_max_decompressed_bytes,
        };
        let options = crate::fetch::pdf::PdfExtractOptions {
            selected_pages: None,
            password: None,
            include_media: false,
            ocr_policy: crate::fetch::pdf::PdfOcrPolicy::Never,
        };
        let (
            content,
            document,
            mut warnings,
            mut structured_warnings,
            pdf_text_truncated,
            extractor_warning_count,
        ) = match crate::fetch::pdf::extract_pdf_text(
            &bytes,
            output_max_chars,
            &pdf_limits,
            Some(&options),
        ) {
            Ok(result) => {
                let warning_count = result.warnings.len();
                (
                    result.text,
                    Some(result.document),
                    result.warnings,
                    result.structured_warnings,
                    result.text_truncated,
                    warning_count,
                )
            }
            Err(crate::fetch::FetchError::PdfNoExtractableText) => (
                String::new(),
                None,
                vec!["pdf_no_extractable_text: PDF has no extractable text".to_string()],
                Vec::new(),
                false,
                0,
            ),
            Err(error) => {
                return Err(ToolError::document_unavailable(format!(
                    "{}: {error}",
                    error.error_code()
                )))
            }
        };

        let all_lines: Vec<String> = content.lines().map(String::from).collect();
        let total_lines = if all_lines.is_empty() {
            None
        } else {
            Some(u32::try_from(all_lines.len()).unwrap_or(u32::MAX))
        };
        let (sliced_lines, _, _, _, line_warning) = crate::core::repo_fetch::apply_line_range(
            &all_lines,
            args.line_start,
            args.line_end,
            args.context_before.unwrap_or(0),
            args.context_after.unwrap_or(0),
        );
        let (mut clamped_lines, _, char_truncated) =
            crate::core::repo_fetch::clamp_lines_to_max_chars(
                &sliced_lines,
                Some(output_max_chars),
            );

        if let Some(warning) = line_warning {
            warnings.push(warning);
        }
        if char_truncated {
            warnings.push("workspace_fetch_truncated_by_max_chars".to_string());
        }
        if args.symbol.is_some() || args.match_text.is_some() {
            warnings.push(
                "span_selection: PDF text does not support source-span selection".to_string(),
            );
        }

        let mut trust_markers = crate::core::sanitize::TrustMarkers::default();
        let mut total_control_removed = 0usize;
        for line in &mut clamped_lines {
            let (cleaned, removed) = crate::core::sanitize::strip_control_chars(&line.text);
            total_control_removed += removed;
            line.text = cleaned;
        }
        trust_markers.control_chars_removed = total_control_removed;
        if total_control_removed > 0 {
            trust_markers.text_sanitized = true;
        }
        let sliced_text = if clamped_lines.is_empty() {
            None
        } else {
            Some(
                clamped_lines
                    .iter()
                    .map(|line| line.text.as_str())
                    .collect::<Vec<_>>()
                    .join("\n"),
            )
        };
        if state.config.fetch.sanitize_output {
            if let Some(text) = sliced_text.as_deref() {
                let hits = crate::core::sanitize::scan_injection_markers(text);
                trust_markers.injection_hits = hits.len();
                if !hits.is_empty() {
                    warnings.push(format!(
                        "local_content_marker_warning: possible prompt injection markers detected in local workspace content ({} hits)",
                        hits.len()
                    ));
                }
            }
        }
        structured_warnings.extend(crate::core::warning::convert_fetch_warnings(
            &warnings[extractor_warning_count..],
        ));

        let source_role = crate::core::code_evidence::infer_source_role(&relative_path);
        let root_name = args.owner.clone();
        let pseudo_url = format!("workspace://{root_name}/{relative_path}");
        let locator = crate::core::repo_fetch::RepoLocator {
            kind: crate::core::repo_fetch::RepoLocatorKind::Workspace,
            host: None,
            owner: None,
            repo: None,
            ref_name: None,
            commit_sha: None,
            path: relative_path,
            workspace_root: Some(root_name),
        };
        let fetch_response = crate::core::repo_fetch::RepoFetchResponse {
            locator: locator.clone(),
            stable_id: Some(crate::core::identity::fetch_id(
                None,
                Some(&locator),
                clamped_lines.first().map(|line| line.number),
                clamped_lines.last().map(|line| line.number),
                sliced_text.as_deref(),
            )),
            source_id: None,
            fetched: true,
            status: None,
            content_type: Some("application/pdf".to_string()),
            language: None,
            source_role: Some(source_role),
            browser_url: pseudo_url.clone(),
            raw_url: pseudo_url,
            permalink_url: None,
            raw_permalink_url: None,
            fetched_url: None,
            ref_resolved: None,
            line_start: args.line_start,
            line_end: args.line_end,
            returned_line_start: clamped_lines.first().map(|line| line.number),
            returned_line_end: clamped_lines.last().map(|line| line.number),
            total_lines,
            text: sliced_text,
            lines: clamped_lines,
            document,
            truncated: char_truncated || pdf_text_truncated,
            structured_warnings,
            warnings,
            manual_interaction_required: false,
            trust: crate::core::repo_fetch::FetchTrust::LocalTrusted,
            trust_markers,
            selected_span: None,
            code_span: None,
            code_context: None,
        };
        serde_json::to_value(&fetch_response)
            .map_err(|e| ToolError::internal(format!("serialization error: {e}")))
    }

    #[cfg(not(feature = "pdf"))]
    {
        let _ = (state, args, root_path, relative_path);
        Err(ToolError::capability_unavailable(
            "pdf_not_compiled_in: enable the `pdf` Cargo feature; see docs/features.md#pdf-extraction; build with `cargo build --features pdf`",
        ))
    }
}
