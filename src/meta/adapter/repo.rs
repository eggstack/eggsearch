use crate::core::SearchWarning;
use crate::core::SourceCard;
use crate::meta::engines::build_http_client;
use tracing::debug;

use super::execution::*;
use super::normalization::*;
use super::{MetadataSearchAdapter, PlannedSubquery};

impl MetadataSearchAdapter {
    /// Run a repo-oriented bundle search. This generates multiple subqueries
    /// from the resolved repo hints, fans out to enabled providers, aggregates
    /// via RRF, groups results into categories, and generates suggested fetches.
    pub async fn repo_search(
        &self,
        req: &crate::core::repo_search::RepoSearchRequest,
        effective_max_results: usize,
        max_results_cap: usize,
        local_backend: Option<&crate::meta::local_backend::LocalWorkspaceBackend>,
        local_inventory: Option<&[crate::meta::local_inventory::LocalRepoIdentity]>,
    ) -> crate::core::repo_search::RepoSearchResponse {
        use crate::core::repo_search::{RepoSearchSubqueryTelemetry, RepoSearchTelemetry};

        let is_exact_error = req.mode == Some(crate::core::repo_search::RepoSearchMode::ExactError);

        // Resolve package metadata if package fields are present
        let package_resolution = if let Some(coord) = req.package_coordinate() {
            let timeout = self.effective_timeout(req.timeout_ms);
            let client_opt = build_http_client(None).ok();
            if let Some(client) = client_opt {
                Some(
                    crate::meta::package_resolver::resolve_package(&client, &coord, Some(timeout))
                        .await,
                )
            } else {
                let mut pr = crate::core::package::PackageResolution {
                    coordinate: coord,
                    ..Default::default()
                };
                pr.warnings
                    .push("failed to build HTTP client for package resolution".to_string());
                Some(pr)
            }
        } else {
            None
        };

        // In exact-error mode, use the error planner for subqueries
        let (plan, error_context) = if is_exact_error {
            let error_config = req.exact_error_config.clone().unwrap_or_default();
            let error_plan =
                crate::meta::error_planner::build_error_plan(&req.query, &error_config);
            let subqueries = crate::meta::error_planner::to_repo_subqueries(&error_plan.subqueries);
            let error_ctx = crate::core::error_query::ErrorSearchContext {
                original_error: error_plan.parts.original.clone(),
                normalized_error: error_plan.parts.normalized.clone(),
                error_codes: error_plan.parts.error_codes.clone(),
                inferred_tools: error_plan.parts.tool_names.clone(),
                inferred_language: error_plan.parts.language_hint.clone(),
                redactions_applied: error_plan.parts.redactions_applied.clone(),
                subqueries: error_plan.subqueries.clone(),
                warnings: error_plan.warnings.clone(),
            };
            (
                crate::meta::repo_planner::RepoSearchPlan {
                    hints: req.resolved_hints(),
                    subqueries,
                },
                Some(error_ctx),
            )
        } else {
            let plan = crate::meta::repo_planner::build_repo_search_plan_with_package(
                req,
                package_resolution.as_ref(),
            );
            (plan, None)
        };

        let effective_timeout = self.effective_timeout(req.timeout_ms);
        let (engines, queried_ids) = self.selected_engines(&req.providers);

        let final_max = effective_max_results;
        let candidate_limit = candidate_pool_size(final_max, max_results_cap);

        debug!(
            query = %req.query,
            providers = ?queried_ids,
            final_max,
            candidate_limit,
            timeout_ms = effective_timeout.as_millis(),
            subqueries = plan.subqueries.len(),
            package_resolved = package_resolution.as_ref().map(|pr| pr.verified).unwrap_or(false),
            "dispatching repo_search"
        );

        let repo_scope = req
            .resolved_repo_locator()
            .and_then(|(owner, repo)| crate::meta::engines::request::RepoScope::new(&owner, &repo));

        let dispatch = dispatch_subqueries(
            &engines,
            plan.subqueries
                .iter()
                .map(|subquery| {
                    let priority = repo_subquery_priority(&subquery.label, is_exact_error);
                    PlannedSubquery {
                        label: subquery.label.to_string(),
                        query: subquery.query.clone(),
                        priority,
                        intended_roles: Vec::new(),
                        repo_scope: repo_scope.clone(),
                        excerpt_count: 0,
                    }
                })
                .collect(),
            candidate_limit,
            effective_timeout,
            "repo_search",
            self.multiquery_concurrency,
            self.multiquery_provider_concurrency,
        )
        .await;

        let attempt_set =
            crate::meta::workflow::RetrievalAttemptSet::from(dispatch.attempts.clone());

        // Record provider health from raw results and failures
        self.record_provider_health(
            &queried_ids,
            &dispatch.raw_results,
            &dispatch.result_latencies_ms,
            &dispatch.raw_failures,
            &dispatch.failure_latencies_ms,
            effective_timeout.as_millis().min(u128::from(u64::MAX)) as u64,
        );

        let providers_failed = provider_failures(
            &queried_ids,
            &dispatch.raw_results,
            &dispatch.raw_failures,
            attempt_set.as_slice(),
        );
        let mut warnings: Vec<SearchWarning> = Vec::new();
        push_failure_warnings(&mut warnings, &dispatch.raw_results, &dispatch.raw_failures);
        for (provider_id, subquery_id, metadata) in &dispatch.retrieval_metadata {
            for scope in metadata.unindexed_scopes() {
                warnings.push(SearchWarning::new(
                    provider_id.clone(),
                    format!(
                        "scope_unindexed: requested scope '{scope}' is not indexed by {provider_id} (subquery '{subquery_id}'); zero results do not mean the scope was searched and contained no match"
                    ),
                ));
            }
        }
        let mut cards =
            aggregate_source_cards(dispatch.raw_results, candidate_limit, self.sanitize_output);

        // Run local workspace search if enabled and requested
        let mut local_warnings: Vec<SearchWarning> = Vec::new();
        let mut local_queried = false;
        if let Some(backend) = local_backend {
            if backend.is_enabled() && req.include_local_enabled() {
                let local_req = crate::core::local::LocalSearchRequest {
                    query: req.query.clone(),
                    path: req.path.clone(),
                    language: req.language.clone(),
                    file: req.file.clone(),
                    symbol: req.symbol.clone(),
                    max_results: Some(local_result_budget(effective_max_results)),
                    timeout_ms: req.timeout_ms,
                };
                let local_result = backend.search(&local_req).await;
                let roots = backend.roots();

                // Discover local repo identities and match to request.
                // Use resolved_repo_locator() so that `repo: "owner/name"`
                // (without explicit owner) correctly triggers local matching.
                let discovered_inventory;
                let inventory: &[crate::meta::local_inventory::LocalRepoIdentity] =
                    match local_inventory {
                        Some(snapshot) => snapshot,
                        None => {
                            discovered_inventory =
                                crate::meta::local_inventory::discover_local_repos(
                                    &crate::core::local::LocalConfig {
                                        enabled: true,
                                        roots: roots.iter().map(|(_, p)| p.clone()).collect(),
                                        ..Default::default()
                                    },
                                    2,
                                );
                            &discovered_inventory
                        }
                    };
                let matched_repo = req.resolved_repo_locator().and_then(|(owner, repo)| {
                    crate::meta::local_inventory::match_local_repo(
                        inventory,
                        req.host.as_ref(),
                        &owner,
                        &repo,
                    )
                });

                if let Some(rid) = matched_repo {
                    local_warnings.push(SearchWarning::new(
                        "local_workspace",
                        format!(
                            "local_repo_match: using local checkout for {}/{}",
                            rid.matched_owner.as_deref().unwrap_or("?"),
                            rid.matched_repo.as_deref().unwrap_or("?"),
                        ),
                    ));
                    if rid.dirty_state == crate::meta::local_inventory::LocalDirtyState::Dirty {
                        local_warnings.push(SearchWarning::new(
                            "local_workspace",
                            "local_repo_dirty: local checkout has uncommitted changes",
                        ));
                    }
                    if rid.dirty_state == crate::meta::local_inventory::LocalDirtyState::Unknown {
                        local_warnings.push(SearchWarning::new(
                            "local_workspace",
                            "local_repo_state_unknown: could not determine working tree state of local checkout",
                        ));
                    }
                }

                let local_cards =
                    crate::meta::local_backend::LocalWorkspaceBackend::to_source_cards(
                        &local_result.matches,
                        &roots,
                        self.sanitize_output,
                        matched_repo,
                    );
                if local_result.timed_out {
                    local_warnings.push(SearchWarning::new(
                        "local_workspace",
                        "local_search_timeout: Local workspace search timed out",
                    ));
                }
                if local_result.truncated {
                    local_warnings.push(SearchWarning::new(
                        "local_workspace",
                        "local_search_truncated: Local workspace search results were truncated",
                    ));
                }
                cards.extend(local_cards);
                local_queried = true;

                // Boost local results that match the requested repo
                // so they rank above remote results in grouping.
                for card in &mut cards {
                    if card
                        .metadata
                        .local_repo_match
                        .as_ref()
                        .is_some_and(|m| m.matched)
                    {
                        if let Some(ref mut score) = card.score {
                            *score += 50.0;
                        }
                    }
                }
            }
        }

        let max_per_group = req.max_per_group.unwrap_or(5);
        let mut groups =
            crate::meta::repo_grouping::group_results_with_hints(cards, max_per_group, &plan.hints);

        // Apply exact-error reranking within each group when in exact-error mode
        if is_exact_error {
            if let Some(ref ec) = error_context {
                for group in groups.iter_mut() {
                    crate::meta::repo_grouping::apply_error_reranking(
                        &mut group.results,
                        &crate::core::error_query::ErrorQueryParts {
                            original: ec.original_error.clone(),
                            normalized: ec.normalized_error.clone(),
                            quoted_exact: ec
                                .subqueries
                                .iter()
                                .find(|s| s.label == "exact_phrase")
                                .map(|s| {
                                    // Strip surrounding quotes from the exact_phrase query
                                    let q = &s.query;
                                    if q.starts_with('"') && q.ends_with('"') && q.len() >= 2 {
                                        q[1..q.len() - 1].to_string()
                                    } else {
                                        q.clone()
                                    }
                                })
                                .unwrap_or_default(),
                            error_codes: ec.error_codes.clone(),
                            tool_names: ec.inferred_tools.clone(),
                            package_names: Vec::new(),
                            language_hint: ec.inferred_language.clone(),
                            stack_frames: Vec::new(),
                            path_fragments: Vec::new(),
                            redactions_applied: ec.redactions_applied.clone(),
                        },
                    );
                }
            }
        }

        let suggested_fetches =
            crate::meta::suggested_fetches::generate_suggested_fetches(&groups, &plan.hints);

        // Local workspace warnings
        warnings.extend(local_warnings);

        // Package resolution warnings
        if let Some(pr) = &package_resolution {
            for w in &pr.warnings {
                warnings.push(SearchWarning::new(
                    "_system",
                    format!("package_resolution: {w}"),
                ));
            }
            if !pr.verified {
                warnings.push(SearchWarning::new(
                    "_system",
                    format!(
                        "package_resolution_fallback: Registry API lookup failed for {}/{}; using deterministic fallback URLs.",
                        pr.coordinate.ecosystem, pr.coordinate.name
                    ),
                ));
            }
        }

        push_deadline_warning(&mut warnings, "repo_search", &dispatch.deadline);

        // Capability-aware warnings
        let has_native_code = any_engine_supports(&engines, |c| c.supports_code_search);
        let has_native_issues = any_engine_supports(&engines, |c| c.supports_issue_search);
        let has_native_releases = any_engine_supports(&engines, |c| c.supports_release_search);
        let has_any_native = has_native_code || has_native_issues || has_native_releases;

        if plan.hints.has_any() && !has_any_native {
            warnings.push(SearchWarning::new(
                "_system",
                "native_code_search_unavailable: Repo hints parsed but no native code-host provider configured; using generic web providers.",
            ));
        }

        // Symbol-aware search warning.
        if plan.hints.symbol.is_some() && !has_native_code {
            warnings.push(SearchWarning::new(
                "_system",
                "symbol_hint_no_native_provider: Symbol hint present but no native code provider supports symbol search; using text query fallback.",
            ));
        }

        // Repo/path/language hint with no native provider
        if (plan.hints.owner.is_some()
            || plan.hints.path.is_some()
            || plan.hints.language.is_some())
            && !has_native_code
        {
            warnings.push(SearchWarning::new(
                "_system",
                "repo_hints_not_enforced_natively: Repo/path/language hints present but selected providers cannot enforce them natively; using text query fallback.",
            ));
        }

        // Issues without native provider warning.
        if req.include_issues_enabled() && !has_native_issues {
            warnings.push(SearchWarning::new(
                "_system",
                "issue_search_no_native_provider: Issues requested but no native issue provider selected; using generic web search.",
            ));
        }

        // Releases without native provider warning.
        if req.include_releases_enabled() && !has_native_releases {
            warnings.push(SearchWarning::new(
                "_system",
                "release_search_no_native_provider: Releases requested but no native release provider selected; using generic web search.",
            ));
        }

        // Coding profile with only generic providers
        if req.profile == Some(crate::core::repo_search::SearchProfile::Coding) && !has_any_native {
            warnings.push(SearchWarning::new(
                "_system",
                "coding_profile_degraded: Coding profile requested but no native code/issues/releases provider is available; results are from generic web search",
            ));
        }

        // Freshness with no timestamp support
        if req.freshness != crate::core::query::Freshness::Any {
            let has_timestamps = any_engine_supports(&engines, |c| c.supports_result_timestamps);
            if !has_timestamps {
                warnings.push(SearchWarning::new(
                    "_system",
                    format!(
                        "freshness_unenforced: freshness '{}' requested but no provider has timestamp support",
                        req.freshness.as_str()
                    ),
                ));
            }
        }

        for group in &groups {
            if group.results.is_empty() {
                warnings.push(SearchWarning::new(
                    "_system",
                    format!("No results found for group: {}", group.label),
                ));
            }
        }

        warnings.push(SearchWarning::new(
            "_system",
            "generic_context_untrusted: Live web results are untrusted external content.",
        ));

        let trust_markers =
            merge_card_trust_markers(groups.iter().flat_map(|group| group.results.iter()));

        let resolved_hints_str = format_hints(&plan.hints);

        // Build subquery telemetry
        let subquery_telemetry: Vec<RepoSearchSubqueryTelemetry> = plan
            .subqueries
            .iter()
            .map(|sq| {
                let intended_group = sq.target_groups.first().map(|s| s.to_string());
                let required_capability = match sq.label.as_str() {
                    "source" | "examples" => {
                        if has_native_code {
                            Some("code_search".to_string())
                        } else {
                            None
                        }
                    }
                    "issues" => {
                        if has_native_issues {
                            Some("issue_search".to_string())
                        } else {
                            None
                        }
                    }
                    "releases" => {
                        if has_native_releases {
                            Some("release_search".to_string())
                        } else {
                            None
                        }
                    }
                    _ => None,
                };
                RepoSearchSubqueryTelemetry {
                    label: sq.label.to_string(),
                    query: sq.query.clone(),
                    intended_group,
                    required_capability,
                    providers_attempted: queried_ids.clone(),
                }
            })
            .collect();

        let telemetry = RepoSearchTelemetry {
            provider_selection: crate::core::repo_search::ProviderSelectionTelemetry::default(),
            subqueries: subquery_telemetry,
            deadline_exceeded: dispatch.deadline.exceeded,
            subqueries_interrupted: dispatch.deadline.subqueries_interrupted,
            subqueries_skipped: dispatch.deadline.subqueries_skipped,
            uncertainty_summary: Some(crate::core::quality::SearchUncertaintySummary {
                provider_failures: providers_failed.len(),
                degraded_provider_selection: false,
                partial_provider_selection: false,
                low_confidence_results: groups
                    .iter()
                    .flat_map(|g| &g.results)
                    .filter(|c| {
                        c.quality.as_ref().is_some_and(|q| {
                            matches!(
                                q.confidence,
                                crate::core::quality::ResultConfidence::Low
                                    | crate::core::quality::ResultConfidence::Unknown
                            )
                        })
                    })
                    .count(),
                warnings: Vec::new(),
            }),
            capability_enforcement: None,
            routing_decision: None,
        };

        // Security context: query advisories when requested and package is present
        let security_context = if req.include_security_context_enabled() {
            if let Some(pr) = &package_resolution {
                if pr.verified {
                    let ecosystem_str = pr.coordinate.ecosystem.osv_ecosystem();
                    match self
                        .query_advisories_by_package(
                            ecosystem_str,
                            &pr.coordinate.name,
                            pr.resolved_version.as_deref(),
                            5,
                        )
                        .await
                    {
                        Ok(vulns) if !vulns.is_empty() => {
                            let highest_severity = vulns
                                .iter()
                                .filter_map(|v| v.severity)
                                .max_by_key(|s| match s {
                                    crate::core::security::SeverityLevel::Critical => 4,
                                    crate::core::security::SeverityLevel::High => 3,
                                    crate::core::security::SeverityLevel::Medium => 2,
                                    crate::core::security::SeverityLevel::Low => 1,
                                    crate::core::security::SeverityLevel::Unknown => 0,
                                });
                            let identifiers = crate::core::security::build_identifier_list(
                                &crate::core::security::SecurityIdentifiers {
                                    package: Some(pr.coordinate.name.clone()),
                                    ecosystem: Some(
                                        pr.coordinate.ecosystem.osv_ecosystem().to_string(),
                                    ),
                                    version: pr.resolved_version.clone(),
                                    ..Default::default()
                                },
                            );
                            let source_quality = crate::core::security::SecuritySourceQuality {
                                tier: crate::core::security::SecuritySourceTier::PackageRegistryAdvisory,
                                tier_reasons: vec!["vulnerabilities sourced from native advisory provider".to_string()],
                            };
                            Some(crate::core::security::CompactSecurityContext {
                                query_kind: crate::core::security::SecurityQueryKind::Package,
                                identifiers,
                                vulnerability_count: vulns.len(),
                                highest_severity,
                                source_quality,
                                warnings: vec![],
                            })
                        }
                        Ok(_) => {
                            warnings.push(SearchWarning::new(
                                "_system",
                                "package_security_no_advisories: No security advisories found for the specified package and version.",
                            ));
                            None
                        }
                        Err(e) => {
                            warnings.push(SearchWarning::new(
                                "_system",
                                format!(
                                    "package_security_lookup_failed: Advisory lookup failed: {e}"
                                ),
                            ));
                            None
                        }
                    }
                } else {
                    warnings.push(SearchWarning::new(
                        "_system",
                        "package_security_skipped: Security context requested but package resolution was not verified; skipping advisory lookup.",
                    ));
                    None
                }
            } else {
                warnings.push(SearchWarning::new(
                    "_system",
                    "package_security_skipped: Security context requested but no package fields provided.",
                ));
                None
            }
        } else {
            None
        };

        let mut providers_queried = queried_ids;
        if local_queried && !providers_queried.contains(&"local_workspace".to_string()) {
            providers_queried.push("local_workspace".to_string());
        }

        let structured_warnings = crate::core::warning::convert_warnings(&warnings);

        for group in groups.iter_mut() {
            crate::core::evidence_postprocess::materialize_evidence_roles(&mut group.results);
        }

        let all_cards: Vec<SourceCard> = groups
            .iter()
            .flat_map(|g| g.results.iter())
            .cloned()
            .collect();

        let (workflow_model, resolution_source) =
            crate::core::evidence_postprocess::resolve_workflow_model_with_context(
                &crate::core::workflow_coverage::WorkflowResolutionContext {
                    tool: "repo_search",
                    workflow: req.workflow,
                    profile: req.profile.as_ref().map(|p| p.as_str()),
                    research_domain: None,
                    exact_error: is_exact_error,
                },
            );
        let retrieval_failures = build_retrieval_failures(
            &providers_failed,
            &providers_queried,
            attempt_set.as_slice(),
            "source",
        );
        let postprocess_result = crate::core::evidence_postprocess::postprocess(
            &all_cards,
            &providers_failed,
            &providers_queried,
            workflow_model.as_ref(),
            &retrieval_failures,
            resolution_source,
            attempt_set.as_slice(),
        );

        let next_actions = postprocess_result
            .workflow_coverage
            .as_ref()
            .map(|wc| {
                let known_ids: Vec<String> = all_cards.iter().map(|c| c.id.clone()).collect();
                crate::core::workflow_coverage::generate_gap_driven_next_actions(
                    wc,
                    &retrieval_failures,
                    &known_ids,
                )
            })
            .unwrap_or_default();

        crate::core::repo_search::RepoSearchResponse {
            query: req.query.clone(),
            mode: if is_exact_error {
                "exact_error".to_string()
            } else {
                "repo_metasearch".to_string()
            },
            resolved_hints: plan.hints.clone(),
            resolved_hints_summary: resolved_hints_str,
            groups,
            suggested_fetches,
            providers_queried,
            providers_failed,
            warnings,
            trust_markers,
            telemetry,
            package_resolution,
            security_context,
            error_context,
            structured_warnings,
            next_actions,
            workflow_coverage: postprocess_result.workflow_coverage,
            retrieval_summary: postprocess_result.retrieval_summary,
            conflict_metadata: postprocess_result.conflict_metadata,
            evidence_role_summary: postprocess_result.evidence_role_summary,
        }
    }
}

/// Assign priority for repo_search subqueries. Lower = higher priority.
///
/// Normal mode: source (with hints) > docs/registry > examples > issues > releases.
/// Exact-error mode: exact_phrase > error_code > error_package > error_issues > error_releases > error_docs.
pub(crate) fn repo_subquery_priority(label: &str, is_exact_error: bool) -> i32 {
    if is_exact_error {
        match label {
            "error_exact" => 0,
            "error_code" => 1,
            "error_package" => 2,
            "error_issues" => 3,
            "error_releases" => 4,
            "error_docs" => 5,
            _ => 10,
        }
    } else {
        match label {
            "source" => 0,
            "docs" => 1,
            "registry" => 2,
            "examples" => 3,
            "issues" => 4,
            "releases" => 5,
            "changelog" => 6,
            _ => 10,
        }
    }
}

pub(crate) fn format_hints(hints: &crate::core::repo_query::RepoQueryHints) -> String {
    let mut parts = Vec::new();
    if let Some(ref h) = hints.host {
        parts.push(format!("host={h:?}"));
    }
    if let Some(ref o) = hints.owner {
        parts.push(format!("owner={o}"));
    }
    if let Some(ref r) = hints.repo {
        parts.push(format!("repo={r}"));
    }
    if let Some(ref o) = hints.org {
        parts.push(format!("org={o}"));
    }
    if let Some(ref p) = hints.path {
        parts.push(format!("path={p}"));
    }
    if let Some(ref f) = hints.file {
        parts.push(format!("file={f}"));
    }
    if let Some(ref l) = hints.language {
        parts.push(format!("lang={l}"));
    }
    if let Some(ref s) = hints.symbol {
        parts.push(format!("symbol={s}"));
    }
    if parts.is_empty() {
        "none".to_string()
    } else {
        parts.join(", ")
    }
}
