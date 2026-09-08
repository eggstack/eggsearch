use crate::core::SearchWarning;
use crate::core::SourceCard;
use crate::meta::research_evidence_analysis::analyze_research_evidence;
use tracing::debug;

use super::execution::*;
use super::normalization::*;
use super::{MetadataSearchAdapter, PlannedSubquery};

impl MetadataSearchAdapter {
    /// Run a research-oriented multi-source evidence search. This generates bounded subqueries
    /// from requested source types and research domain, fans out to enabled providers, aggregates
    /// via RRF, groups results by evidence type, and generates suggested fetches with diversity constraints.
    pub async fn research_search(
        &self,
        req: &crate::core::research::ResearchSearchRequest,
        effective_max_results: usize,
        max_results_cap: usize,
    ) -> crate::core::research::ResearchSearchResponse {
        use crate::core::research::{ResearchDomain, ResearchSearchResponse};
        use crate::meta::research_grouping::group_research_results;
        use crate::meta::research_planner::build_research_search_plan;
        use crate::meta::research_suggested_fetches::generate_research_suggested_fetches;
        use crate::meta::research_workflow::{
            apply_diversity_caps, build_research_telemetry, build_workflow_context,
        };

        let plan = build_research_search_plan(req);

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
            domain = ?plan.domain,
            workflow = ?req.workflow,
            depth = ?req.depth,
            "dispatching research_search"
        );

        let dispatch = dispatch_subqueries(
            &engines,
            plan.subqueries
                .iter()
                .map(|subquery| {
                    let priority = research_subquery_priority(&subquery.source_type);
                    let intended_roles =
                        intended_roles_for_research_source_type(subquery.source_type);
                    PlannedSubquery {
                        label: subquery.id.clone(),
                        query: subquery.query.clone(),
                        priority,
                        intended_roles,
                        repo_scope: None,
                        excerpt_count: 0,
                    }
                })
                .collect(),
            candidate_limit,
            effective_timeout,
            "research_search",
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
        let cards =
            aggregate_source_cards(dispatch.raw_results, candidate_limit, self.sanitize_output);

        let max_per_group = req.effective_max_per_group(5);
        let max_groups = req.effective_max_groups(14);
        let groups = group_research_results(cards, max_per_group, max_groups);

        // Apply diversity caps
        let (mut groups, diversity_warnings) = apply_diversity_caps(groups, max_per_group);

        let suggested_fetches = generate_research_suggested_fetches(&groups);

        push_deadline_warning(&mut warnings, "research_search", &dispatch.deadline);

        // Subquery cap warning
        if plan.subqueries.len() >= 8 {
            warnings.push(SearchWarning::new(
                "_system",
                "subquery_cap_applied: desired source types exceeded bounded query cap of 8",
            ));
        }

        // Freshness approximate warning
        if req.freshness != crate::core::query::Freshness::Any {
            let has_timestamps = any_engine_supports(&engines, |c| c.supports_result_timestamps);
            if !has_timestamps {
                warnings.push(SearchWarning::new(
                    "_system",
                    format!(
                        "freshness_unenforced: freshness '{}' requested but only some provider results have timestamps",
                        req.freshness.as_str()
                    ),
                ));
            }
        }

        // Diversity cap warnings
        for w in &diversity_warnings {
            warnings.push(SearchWarning::new("_system", w.clone()));
        }

        // Empty group warnings
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

        // Build workflow context if workflow is specified
        let workflow_context = if req.workflow.is_some() {
            Some(build_workflow_context(
                req,
                &groups,
                &suggested_fetches,
                &queried_ids,
            ))
        } else {
            None
        };

        // Build telemetry
        let dimensions = workflow_context
            .as_ref()
            .map(|ctx| ctx.dimensions.clone())
            .unwrap_or_default();
        let gaps = workflow_context
            .as_ref()
            .map(|ctx| ctx.gaps.clone())
            .unwrap_or_default();
        let telemetry = Some(build_research_telemetry(
            req,
            &dimensions,
            plan.subqueries.len(),
            &diversity_warnings,
            &gaps,
        ));

        let structured_warnings = crate::core::warning::convert_warnings(&warnings);

        let analysis = analyze_research_evidence(&groups, Some(&req.query));

        for group in groups.iter_mut() {
            crate::core::evidence_postprocess::materialize_evidence_roles(&mut group.results);
        }

        let all_cards: Vec<SourceCard> = groups
            .iter()
            .flat_map(|g| g.results.iter())
            .cloned()
            .collect();

        let research_domain_str = match req.research_domain.unwrap_or(ResearchDomain::General) {
            ResearchDomain::SoftwareArchitecture | ResearchDomain::ApiDesign => {
                Some("architecture_decision")
            }
            ResearchDomain::Security => Some("security_review"),
            ResearchDomain::Performance => Some("performance_investigation"),
            _ => None,
        };
        let (workflow_model, resolution_source) =
            crate::core::evidence_postprocess::resolve_workflow_model_with_context(
                &crate::core::workflow_coverage::WorkflowResolutionContext {
                    tool: "research_search",
                    workflow: req.workflow.and_then(|w| match w {
                        crate::core::research::ResearchWorkflow::ApiEvaluation => {
                            Some(crate::core::workflow_coverage::WorkflowKind::ApiComprehension)
                        }
                        crate::core::research::ResearchWorkflow::ArchitectureDecision => {
                            Some(crate::core::workflow_coverage::WorkflowKind::ComparativeResearch)
                        }
                        crate::core::research::ResearchWorkflow::SecurityReview => {
                            Some(crate::core::workflow_coverage::WorkflowKind::SecurityReview)
                        }
                        crate::core::research::ResearchWorkflow::PerformanceInvestigation => Some(
                            crate::core::workflow_coverage::WorkflowKind::PerformanceInvestigation,
                        ),
                        crate::core::research::ResearchWorkflow::MigrationPlanning => {
                            Some(crate::core::workflow_coverage::WorkflowKind::VersionMigration)
                        }
                        _ => None,
                    }),
                    profile: None,
                    research_domain: research_domain_str,
                    exact_error: false,
                },
            );
        let retrieval_failures = build_retrieval_failures(
            &providers_failed,
            &queried_ids,
            attempt_set.as_slice(),
            "research",
        );
        let postprocess_result = crate::core::evidence_postprocess::postprocess(
            &all_cards,
            &providers_failed,
            &queried_ids,
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

        ResearchSearchResponse {
            query: req.query.clone(),
            mode: "research_metasearch".to_string(),
            research_domain: req.research_domain.unwrap_or(ResearchDomain::General),
            subqueries: plan.subqueries,
            groups,
            suggested_fetches,
            providers_queried: queried_ids,
            providers_failed,
            warnings,
            trust_markers,
            workflow_context,
            telemetry,
            structured_warnings,
            next_actions,
            claims: analysis.0,
            conflicts: analysis.1,
            source_quality: analysis.2,
            evidence_gaps: analysis.3,
            workflow_coverage: postprocess_result.workflow_coverage,
            retrieval_summary: postprocess_result.retrieval_summary,
            conflict_metadata: postprocess_result.conflict_metadata,
            evidence_role_summary: postprocess_result.evidence_role_summary,
        }
    }
}

/// Assign priority for research_search subqueries. Lower = higher priority.
pub(crate) fn research_subquery_priority(
    source_type: &crate::core::research::ResearchSourceType,
) -> i32 {
    use crate::core::research::ResearchSourceType;
    match source_type {
        ResearchSourceType::PrimarySources => 0,
        ResearchSourceType::OfficialDocs => 1,
        ResearchSourceType::Specifications => 2,
        ResearchSourceType::ReferenceImplementations => 3,
        ResearchSourceType::SecurityConsiderations => 4,
        ResearchSourceType::Benchmarks => 5,
        ResearchSourceType::DesignDiscussions => 6,
        ResearchSourceType::IssueThreads => 7,
        ResearchSourceType::ReleaseNotes => 8,
        ResearchSourceType::RecentNews => 9,
        ResearchSourceType::CommunityDiscussion => 10,
        ResearchSourceType::Counterpoints => 11,
        ResearchSourceType::AcademicOrFormalSources => 12,
    }
}

pub(crate) fn intended_roles_for_research_source_type(
    st: crate::core::research::ResearchSourceType,
) -> Vec<crate::core::evidence_role::EvidenceRole> {
    vec![crate::core::evidence_role::EvidenceRole::from_research_source_type(st)]
}
