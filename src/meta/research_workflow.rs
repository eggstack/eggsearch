//! Workflow-aware research scaffolding: dimensions, coverage, gaps, diversity.
//!
//! This module provides deterministic research workflow support for
//! `research_search`. It generates structured dimensions based on
//! workflow type, computes coverage from grouped results, detects
//! coverage gaps, and applies source diversity caps.

use crate::core::research::{
    ResearchCoverage, ResearchDepth, ResearchDimension, ResearchGap, ResearchGapKind,
    ResearchResultGroup, ResearchResultGroupKind, ResearchSearchRequest, ResearchSourceType,
    ResearchTelemetry, ResearchWorkflow, ResearchWorkflowContext,
};

/// Maximum subqueries per depth level.
fn max_subqueries_for_depth(depth: ResearchDepth) -> usize {
    match depth {
        ResearchDepth::Quick => 4,
        ResearchDepth::Standard => 8,
        ResearchDepth::Deep => 12,
    }
}

/// Build workflow dimensions for the given workflow type.
///
/// Each workflow generates a deterministic set of dimensions with
/// associated source types and subquery templates.
pub fn build_workflow_dimensions(
    workflow: ResearchWorkflow,
    query: &str,
    compare_targets: &[String],
    depth: ResearchDepth,
) -> Vec<ResearchDimension> {
    let max = max_subqueries_for_depth(depth);
    let mut dims = match workflow {
        ResearchWorkflow::ArchitectureDecision => architecture_decision_dimensions(query),
        ResearchWorkflow::ApiEvaluation => api_evaluation_dimensions(query),
        ResearchWorkflow::LibraryComparison => {
            library_comparison_dimensions(query, compare_targets)
        }
        ResearchWorkflow::MigrationPlanning => migration_planning_dimensions(query),
        ResearchWorkflow::SecurityReview => security_review_dimensions(query),
        ResearchWorkflow::PerformanceInvestigation => performance_investigation_dimensions(query),
        ResearchWorkflow::EcosystemSurvey => ecosystem_survey_dimensions(query),
        ResearchWorkflow::General => general_dimensions(query),
    };

    // Bound by depth
    let total_subqueries: usize = dims.iter().map(|d| d.subqueries.len()).sum();
    if total_subqueries > max {
        // Truncate dimensions from the end until we fit
        while dims.len() > 1 {
            dims.pop();
            let remaining: usize = dims.iter().map(|d| d.subqueries.len()).sum();
            if remaining <= max {
                break;
            }
        }
        // If still over, truncate subqueries in the last dimension
        let remaining: usize = dims.iter().map(|d| d.subqueries.len()).sum();
        if remaining > max {
            if let Some(last) = dims.last_mut() {
                let excess = remaining - max;
                last.subqueries
                    .truncate(last.subqueries.len().saturating_sub(excess));
            }
        }
    }

    dims
}

fn architecture_decision_dimensions(query: &str) -> Vec<ResearchDimension> {
    vec![
        ResearchDimension {
            name: "Official Docs & Specs".to_string(),
            purpose: "Authoritative documentation and specifications".to_string(),
            source_types: vec![
                ResearchSourceType::PrimarySources,
                ResearchSourceType::OfficialDocs,
                ResearchSourceType::Specifications,
            ],
            subqueries: vec![
                format!("{query} official documentation specification"),
                format!("{query} API reference guide"),
            ],
        },
        ResearchDimension {
            name: "Reference Implementations".to_string(),
            purpose: "Canonical codebases and production implementations".to_string(),
            source_types: vec![ResearchSourceType::ReferenceImplementations],
            subqueries: vec![format!(
                "{query} reference implementation github source code"
            )],
        },
        ResearchDimension {
            name: "Design Discussions".to_string(),
            purpose: "RFCs, architecture decision records, and design proposals".to_string(),
            source_types: vec![ResearchSourceType::DesignDiscussions],
            subqueries: vec![format!(
                "{query} design discussion RFC proposal architecture"
            )],
        },
        ResearchDimension {
            name: "Benchmarks & Performance".to_string(),
            purpose: "Performance measurements and comparative benchmarks".to_string(),
            source_types: vec![ResearchSourceType::Benchmarks],
            subqueries: vec![format!("{query} benchmark performance latency throughput")],
        },
        ResearchDimension {
            name: "Security & Failure Modes".to_string(),
            purpose: "Security considerations, threat models, and failure modes".to_string(),
            source_types: vec![ResearchSourceType::SecurityConsiderations],
            subqueries: vec![format!(
                "{query} security considerations threat model vulnerability"
            )],
        },
        ResearchDimension {
            name: "Migration & Adoption".to_string(),
            purpose: "Migration guides, adoption stories, and breaking changes".to_string(),
            source_types: vec![ResearchSourceType::ReleaseNotes],
            subqueries: vec![format!("{query} migration guide adoption breaking changes")],
        },
        ResearchDimension {
            name: "Counterpoints & Tradeoffs".to_string(),
            purpose: "Alternative viewpoints, limitations, and tradeoffs".to_string(),
            source_types: vec![ResearchSourceType::Counterpoints],
            subqueries: vec![format!(
                "{query} drawbacks limitations tradeoffs alternatives"
            )],
        },
    ]
}

fn api_evaluation_dimensions(query: &str) -> Vec<ResearchDimension> {
    vec![
        ResearchDimension {
            name: "Official API Documentation".to_string(),
            purpose: "Authoritative API docs, type signatures, and usage guides".to_string(),
            source_types: vec![
                ResearchSourceType::OfficialDocs,
                ResearchSourceType::Specifications,
            ],
            subqueries: vec![format!("{query} official API documentation reference")],
        },
        ResearchDimension {
            name: "Examples & Tutorials".to_string(),
            purpose: "Usage examples, tutorials, and getting-started guides".to_string(),
            source_types: vec![ResearchSourceType::CommunityDiscussion],
            subqueries: vec![format!("{query} tutorial example getting started guide")],
        },
        ResearchDimension {
            name: "Source Implementation".to_string(),
            purpose: "Source code, internal implementation details".to_string(),
            source_types: vec![ResearchSourceType::ReferenceImplementations],
            subqueries: vec![format!("{query} source code implementation github")],
        },
        ResearchDimension {
            name: "Issues & Known Pitfalls".to_string(),
            purpose: "Bug reports, known issues, and common pitfalls".to_string(),
            source_types: vec![ResearchSourceType::IssueThreads],
            subqueries: vec![format!("{query} issues bugs known problems pitfalls")],
        },
        ResearchDimension {
            name: "Version & Release Notes".to_string(),
            purpose: "Version history, changelogs, and breaking changes".to_string(),
            source_types: vec![ResearchSourceType::ReleaseNotes],
            subqueries: vec![format!("{query} release notes changelog version history")],
        },
        ResearchDimension {
            name: "Security & Compatibility".to_string(),
            purpose: "Security advisories, compatibility notes, and deprecations".to_string(),
            source_types: vec![ResearchSourceType::SecurityConsiderations],
            subqueries: vec![format!(
                "{query} security compatibility deprecation advisory"
            )],
        },
    ]
}

fn library_comparison_dimensions(query: &str, targets: &[String]) -> Vec<ResearchDimension> {
    let mut dims = vec![
        ResearchDimension {
            name: "Official Docs per Target".to_string(),
            purpose: "Authoritative documentation for each library being compared".to_string(),
            source_types: vec![ResearchSourceType::OfficialDocs],
            subqueries: targets
                .iter()
                .map(|t| format!("{t} official documentation API reference"))
                .collect::<Vec<_>>(),
        },
        ResearchDimension {
            name: "Benchmarks".to_string(),
            purpose: "Performance comparisons between the targets".to_string(),
            source_types: vec![ResearchSourceType::Benchmarks],
            subqueries: vec![format!("{query} benchmark performance comparison")],
        },
        ResearchDimension {
            name: "Maintenance & Release Cadence".to_string(),
            purpose: "Project health, release frequency, and maintenance status".to_string(),
            source_types: vec![
                ResearchSourceType::ReleaseNotes,
                ResearchSourceType::IssueThreads,
            ],
            subqueries: vec![
                format!("{query} release cadence maintenance activity"),
                format!("{query} issues bugs open problems"),
            ],
        },
        ResearchDimension {
            name: "Security Advisories".to_string(),
            purpose: "Security vulnerabilities and advisories for each target".to_string(),
            source_types: vec![ResearchSourceType::SecurityConsiderations],
            subqueries: vec![format!("{query} security advisory vulnerability")],
        },
        ResearchDimension {
            name: "Migration & Interoperability".to_string(),
            purpose: "Migration guides and interop between the targets".to_string(),
            source_types: vec![ResearchSourceType::ReleaseNotes],
            subqueries: vec![format!("{query} migration interop compatibility")],
        },
    ];

    // Add per-target source type subqueries if we have room
    if targets.len() > 1 {
        dims.push(ResearchDimension {
            name: "Per-Target Deep Dives".to_string(),
            purpose: "Detailed look at each target's implementation".to_string(),
            source_types: vec![ResearchSourceType::ReferenceImplementations],
            subqueries: targets
                .iter()
                .take(3) // Bound per-target to avoid explosion
                .map(|t| format!("{t} source code implementation github"))
                .collect::<Vec<_>>(),
        });
    }

    dims
}

fn migration_planning_dimensions(query: &str) -> Vec<ResearchDimension> {
    vec![
        ResearchDimension {
            name: "Migration Guides".to_string(),
            purpose: "Official migration guides and upgrade paths".to_string(),
            source_types: vec![ResearchSourceType::ReleaseNotes],
            subqueries: vec![format!("{query} migration guide upgrade path")],
        },
        ResearchDimension {
            name: "Changelogs & Breaking Changes".to_string(),
            purpose: "Changelogs, breaking changes, and deprecation notices".to_string(),
            source_types: vec![ResearchSourceType::ReleaseNotes],
            subqueries: vec![format!("{query} changelog breaking changes deprecation")],
        },
        ResearchDimension {
            name: "Breaking-Change Issues".to_string(),
            purpose: "Issue threads discussing breaking changes and migration pain points"
                .to_string(),
            source_types: vec![ResearchSourceType::IssueThreads],
            subqueries: vec![format!("{query} breaking change issue migration problem")],
        },
        ResearchDimension {
            name: "Before/After Examples".to_string(),
            purpose: "Code examples showing before and after migration".to_string(),
            source_types: vec![
                ResearchSourceType::ReferenceImplementations,
                ResearchSourceType::CommunityDiscussion,
            ],
            subqueries: vec![format!("{query} migration example before after code")],
        },
        ResearchDimension {
            name: "Security Changes".to_string(),
            purpose: "Security-related changes in the new version".to_string(),
            source_types: vec![ResearchSourceType::SecurityConsiderations],
            subqueries: vec![format!("{query} security changes update advisory")],
        },
    ]
}

fn security_review_dimensions(query: &str) -> Vec<ResearchDimension> {
    vec![
        ResearchDimension {
            name: "Security Advisories".to_string(),
            purpose: "Known vulnerabilities, CVEs, and security advisories".to_string(),
            source_types: vec![ResearchSourceType::SecurityConsiderations],
            subqueries: vec![format!("{query} security advisory CVE vulnerability")],
        },
        ResearchDimension {
            name: "Threat Modeling".to_string(),
            purpose: "Threat models, attack surfaces, and risk assessments".to_string(),
            source_types: vec![ResearchSourceType::SecurityConsiderations],
            subqueries: vec![format!("{query} threat model attack surface risk")],
        },
        ResearchDimension {
            name: "Hardening Guides".to_string(),
            purpose: "Security hardening, configuration, and best practices".to_string(),
            source_types: vec![ResearchSourceType::OfficialDocs],
            subqueries: vec![format!(
                "{query} security hardening configuration best practices"
            )],
        },
        ResearchDimension {
            name: "Issue Discussion".to_string(),
            purpose: "Security-related issues and discussions".to_string(),
            source_types: vec![ResearchSourceType::IssueThreads],
            subqueries: vec![format!("{query} security issue vulnerability report")],
        },
        ResearchDimension {
            name: "Community Analysis".to_string(),
            purpose: "Community security analysis and discussion".to_string(),
            source_types: vec![ResearchSourceType::CommunityDiscussion],
            subqueries: vec![format!("{query} security analysis community discussion")],
        },
    ]
}

fn performance_investigation_dimensions(query: &str) -> Vec<ResearchDimension> {
    vec![
        ResearchDimension {
            name: "Benchmarks".to_string(),
            purpose: "Performance benchmarks, latency, and throughput measurements".to_string(),
            source_types: vec![ResearchSourceType::Benchmarks],
            subqueries: vec![format!("{query} benchmark performance latency throughput")],
        },
        ResearchDimension {
            name: "Profiling & Optimization".to_string(),
            purpose: "Profiling tools, optimization techniques, and tuning guides".to_string(),
            source_types: vec![
                ResearchSourceType::OfficialDocs,
                ResearchSourceType::CommunityDiscussion,
            ],
            subqueries: vec![format!("{query} profiling optimization tuning guide")],
        },
        ResearchDimension {
            name: "Performance Issues".to_string(),
            purpose: "Performance-related issues and regression reports".to_string(),
            source_types: vec![ResearchSourceType::IssueThreads],
            subqueries: vec![format!("{query} performance issue regression slow")],
        },
        ResearchDimension {
            name: "Comparative Analysis".to_string(),
            purpose: "Comparative performance analysis across alternatives".to_string(),
            source_types: vec![
                ResearchSourceType::Benchmarks,
                ResearchSourceType::CommunityDiscussion,
            ],
            subqueries: vec![format!("{query} performance comparison alternative")],
        },
    ]
}

fn ecosystem_survey_dimensions(query: &str) -> Vec<ResearchDimension> {
    vec![
        ResearchDimension {
            name: "Ecosystem Overview".to_string(),
            purpose: "Broad overview of the ecosystem landscape".to_string(),
            source_types: vec![ResearchSourceType::OfficialDocs],
            subqueries: vec![format!("{query} ecosystem overview landscape")],
        },
        ResearchDimension {
            name: "Popular Libraries".to_string(),
            purpose: "Popular and well-maintained libraries in the ecosystem".to_string(),
            source_types: vec![ResearchSourceType::OfficialDocs],
            subqueries: vec![format!("{query} popular library recommended")],
        },
        ResearchDimension {
            name: "Community Sentiment".to_string(),
            purpose: "Community opinions, preferences, and experiences".to_string(),
            source_types: vec![ResearchSourceType::CommunityDiscussion],
            subqueries: vec![format!("{query} community opinion experience comparison")],
        },
        ResearchDimension {
            name: "Recent Developments".to_string(),
            purpose: "Recent news, releases, and ecosystem developments".to_string(),
            source_types: vec![
                ResearchSourceType::RecentNews,
                ResearchSourceType::ReleaseNotes,
            ],
            subqueries: vec![format!("{query} recent news update announcement")],
        },
        ResearchDimension {
            name: "Security Landscape".to_string(),
            purpose: "Security advisories and concerns across the ecosystem".to_string(),
            source_types: vec![ResearchSourceType::SecurityConsiderations],
            subqueries: vec![format!("{query} security advisory ecosystem")],
        },
    ]
}

fn general_dimensions(query: &str) -> Vec<ResearchDimension> {
    vec![
        ResearchDimension {
            name: "Official Documentation".to_string(),
            purpose: "Authoritative documentation and references".to_string(),
            source_types: vec![
                ResearchSourceType::PrimarySources,
                ResearchSourceType::OfficialDocs,
            ],
            subqueries: vec![format!("{query} official documentation reference")],
        },
        ResearchDimension {
            name: "Implementation Evidence".to_string(),
            purpose: "Source code, examples, and reference implementations".to_string(),
            source_types: vec![ResearchSourceType::ReferenceImplementations],
            subqueries: vec![format!("{query} source code implementation github")],
        },
        ResearchDimension {
            name: "Design Discussions".to_string(),
            purpose: "RFCs, proposals, and design discussions".to_string(),
            source_types: vec![ResearchSourceType::DesignDiscussions],
            subqueries: vec![format!("{query} design discussion RFC proposal")],
        },
        ResearchDimension {
            name: "Security Considerations".to_string(),
            purpose: "Security advisories, threat models, and hardening".to_string(),
            source_types: vec![ResearchSourceType::SecurityConsiderations],
            subqueries: vec![format!("{query} security considerations vulnerability")],
        },
        ResearchDimension {
            name: "Counterpoints".to_string(),
            purpose: "Alternative viewpoints, limitations, and tradeoffs".to_string(),
            source_types: vec![ResearchSourceType::Counterpoints],
            subqueries: vec![format!("{query} drawbacks limitations tradeoffs")],
        },
    ]
}

/// Compute coverage from grouped results.
pub fn compute_coverage(groups: &[ResearchResultGroup]) -> ResearchCoverage {
    let mut coverage = ResearchCoverage::default();

    for group in groups {
        let count = group.results.len();
        match group.kind {
            ResearchResultGroupKind::PrimarySources | ResearchResultGroupKind::OfficialDocs => {
                coverage.primary_sources_found += count;
                coverage.official_docs_found += count;
            }
            ResearchResultGroupKind::ReferenceImplementations => {
                coverage.implementation_sources_found += count;
            }
            ResearchResultGroupKind::Benchmarks => {
                coverage.benchmark_sources_found += count;
            }
            ResearchResultGroupKind::SecurityConsiderations => {
                coverage.security_sources_found += count;
            }
            ResearchResultGroupKind::Counterpoints => {
                coverage.counterpoints_found += count;
            }
            ResearchResultGroupKind::RecentNews => {
                coverage.recent_sources_found += count;
            }
            _ => {}
        }
    }

    coverage
}

/// Detect coverage gaps based on workflow, request flags, and coverage.
pub fn detect_gaps(
    workflow: ResearchWorkflow,
    req: &ResearchSearchRequest,
    coverage: &ResearchCoverage,
    providers_queried: &[String],
) -> Vec<ResearchGap> {
    let mut gaps = Vec::new();

    let need_primary = matches!(
        workflow,
        ResearchWorkflow::ArchitectureDecision
            | ResearchWorkflow::ApiEvaluation
            | ResearchWorkflow::SecurityReview
    );
    if need_primary && coverage.primary_sources_found == 0 {
        gaps.push(ResearchGap {
            kind: ResearchGapKind::NoPrimarySources,
            message: "No primary or official documentation sources found. Consider broadening the query or checking provider availability.".to_string(),
            suggested_query: None,
        });
    }

    if coverage.recent_sources_found == 0
        && matches!(
            workflow,
            ResearchWorkflow::EcosystemSurvey | ResearchWorkflow::PerformanceInvestigation
        )
    {
        gaps.push(ResearchGap {
            kind: ResearchGapKind::NoRecentSources,
            message: "No recent sources found. Results may be outdated.".to_string(),
            suggested_query: None,
        });
    }

    if req.include_counterpoints == Some(true) && coverage.counterpoints_found == 0 {
        gaps.push(ResearchGap {
            kind: ResearchGapKind::NoCounterpoints,
            message: "Counterpoints were requested but none found. The topic may have limited critical discussion.".to_string(),
            suggested_query: None,
        });
    }

    if coverage.implementation_sources_found == 0
        && matches!(
            workflow,
            ResearchWorkflow::ArchitectureDecision
                | ResearchWorkflow::ApiEvaluation
                | ResearchWorkflow::LibraryComparison
        )
    {
        gaps.push(ResearchGap {
            kind: ResearchGapKind::NoImplementationEvidence,
            message: "No implementation evidence found. Consider checking reference repositories directly.".to_string(),
            suggested_query: None,
        });
    }

    if coverage.benchmark_sources_found == 0
        && matches!(
            workflow,
            ResearchWorkflow::PerformanceInvestigation | ResearchWorkflow::LibraryComparison
        )
    {
        gaps.push(ResearchGap {
            kind: ResearchGapKind::NoBenchmarks,
            message:
                "No benchmark or performance data found. Performance claims may be unverifiable."
                    .to_string(),
            suggested_query: None,
        });
    }

    if coverage.security_sources_found == 0 && matches!(workflow, ResearchWorkflow::SecurityReview)
    {
        gaps.push(ResearchGap {
            kind: ResearchGapKind::NoSecurityDiscussion,
            message: "No security advisories or discussions found. This may indicate limited security review of the topic.".to_string(),
            suggested_query: None,
        });
    }

    if matches!(workflow, ResearchWorkflow::MigrationPlanning)
        && coverage.primary_sources_found == 0
    {
        gaps.push(ResearchGap {
            kind: ResearchGapKind::NoMigrationDocs,
            message:
                "No migration documentation found. Breaking changes may need manual investigation."
                    .to_string(),
            suggested_query: None,
        });
    }

    if providers_queried.len() <= 1 {
        gaps.push(ResearchGap {
            kind: ResearchGapKind::ProviderCoverageLimited,
            message: "Only one provider was queried. Results may lack diversity.".to_string(),
            suggested_query: None,
        });
    }

    gaps
}

/// Apply diversity caps to prevent one domain, provider, or source type from dominating.
///
/// Returns the capped groups and any diversity warnings.
pub fn apply_diversity_caps(
    groups: Vec<ResearchResultGroup>,
    max_per_group: usize,
) -> (Vec<ResearchResultGroup>, Vec<String>) {
    let mut warnings = Vec::new();
    let mut capped_groups = Vec::new();

    for mut group in groups {
        let original_count = group.results.len();
        group.results.truncate(max_per_group);
        if group.results.len() < original_count {
            warnings.push(format!(
                "diversity_cap: group '{}' capped from {} to {} results",
                group.label,
                original_count,
                group.results.len()
            ));
            group.truncated = true;
        }
        capped_groups.push(group);
    }

    (capped_groups, warnings)
}

/// Build the workflow context block from request, groups, and coverage.
pub fn build_workflow_context(
    req: &ResearchSearchRequest,
    groups: &[ResearchResultGroup],
    suggested_fetches: &[crate::core::research::ResearchSuggestedFetch],
    providers_queried: &[String],
) -> ResearchWorkflowContext {
    let workflow = req.effective_workflow();
    let coverage = compute_coverage(groups);
    let gaps = detect_gaps(workflow, req, &coverage, providers_queried);

    let dimensions = build_workflow_dimensions(
        workflow,
        &req.query,
        &req.compare_targets,
        req.effective_depth(),
    );

    // Interpreted question: incorporate workflow context
    let interpreted_question = build_interpreted_question(req, workflow);

    // Recommended next fetches: take from suggested_fetches, add workflow-specific ones
    let recommended = suggested_fetches.iter().take(5).cloned().collect();

    let warnings: Vec<String> = gaps
        .iter()
        .map(|g| {
            format!(
                "{}_gap: {}",
                serde_json::to_string(&g.kind).unwrap_or_default(),
                g.message
            )
        })
        .collect();

    ResearchWorkflowContext {
        workflow,
        interpreted_question,
        dimensions,
        coverage,
        gaps,
        recommended_next_fetches: recommended,
        warnings,
    }
}

/// Build an interpreted question string incorporating workflow context.
fn build_interpreted_question(req: &ResearchSearchRequest, workflow: ResearchWorkflow) -> String {
    let base = req.query.clone();
    match workflow {
        ResearchWorkflow::LibraryComparison if !req.compare_targets.is_empty() => {
            format!(
                "Compare {} regarding: {}",
                req.compare_targets.join(" vs "),
                base
            )
        }
        ResearchWorkflow::LibraryComparison => format!("Library comparison: {base}"),
        ResearchWorkflow::MigrationPlanning => format!("Migration planning: {base}"),
        ResearchWorkflow::SecurityReview => format!("Security review: {base}"),
        ResearchWorkflow::PerformanceInvestigation => format!("Performance investigation: {base}"),
        ResearchWorkflow::ArchitectureDecision => format!("Architecture decision: {base}"),
        ResearchWorkflow::ApiEvaluation => format!("API evaluation: {base}"),
        ResearchWorkflow::EcosystemSurvey => format!("Ecosystem survey: {base}"),
        ResearchWorkflow::General => base,
    }
}

/// Build research telemetry from the request and results.
pub fn build_research_telemetry(
    req: &ResearchSearchRequest,
    dimensions: &[ResearchDimension],
    subquery_count: usize,
    diversity_caps: &[String],
    gaps: &[ResearchGap],
) -> ResearchTelemetry {
    ResearchTelemetry {
        workflow: req.workflow,
        depth: req.effective_depth(),
        dimensions_generated: dimensions.len(),
        subqueries_generated: subquery_count,
        source_diversity_caps_applied: diversity_caps.to_vec(),
        coverage_gaps: gaps.iter().map(|g| g.kind).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::research::ResearchSearchRequest;
    use crate::core::result::TrustLevel;
    use crate::core::source_card::{SourceCard, SourceKind, SourceMetadata};

    fn make_card(source_kind: SourceKind, url: &str) -> SourceCard {
        let mut card = SourceCard::new(
            "Test",
            url,
            vec!["test".to_string()],
            None,
            TrustLevel::ExternalUntrusted,
        );
        card.metadata = SourceMetadata {
            source_kind,
            ..Default::default()
        };
        card
    }

    fn make_group(kind: ResearchResultGroupKind, count: usize) -> ResearchResultGroup {
        let results: Vec<SourceCard> = (0..count)
            .map(|i| {
                make_card(
                    SourceKind::OfficialDocs,
                    &format!("https://example.com/{i}"),
                )
            })
            .collect();
        ResearchResultGroup {
            kind,
            label: format!("{kind:?}"),
            results,
            truncated: false,
            quality_summary: None,
        }
    }

    #[test]
    fn architecture_decision_generates_7_dimensions() {
        let dims = build_workflow_dimensions(
            ResearchWorkflow::ArchitectureDecision,
            "microservices vs monolith",
            &[],
            ResearchDepth::Standard,
        );
        assert_eq!(dims.len(), 7);
        assert_eq!(dims[0].name, "Official Docs & Specs");
        assert_eq!(dims[1].name, "Reference Implementations");
        assert_eq!(dims[6].name, "Counterpoints & Tradeoffs");
    }

    #[test]
    fn api_evaluation_generates_6_dimensions() {
        let dims = build_workflow_dimensions(
            ResearchWorkflow::ApiEvaluation,
            "axum web framework",
            &[],
            ResearchDepth::Standard,
        );
        assert_eq!(dims.len(), 6);
    }

    #[test]
    fn library_comparison_includes_per_target() {
        let dims = build_workflow_dimensions(
            ResearchWorkflow::LibraryComparison,
            "web framework comparison",
            &["axum".to_string(), "actix-web".to_string()],
            ResearchDepth::Standard,
        );
        assert!(dims.len() >= 5);
        // First dimension should have per-target subqueries
        assert_eq!(dims[0].subqueries.len(), 2);
        assert!(dims[0].subqueries[0].contains("axum"));
        assert!(dims[0].subqueries[1].contains("actix-web"));
    }

    #[test]
    fn migration_planning_generates_5_dimensions() {
        let dims = build_workflow_dimensions(
            ResearchWorkflow::MigrationPlanning,
            "migrate to v2",
            &[],
            ResearchDepth::Standard,
        );
        assert_eq!(dims.len(), 5);
    }

    #[test]
    fn security_review_generates_5_dimensions() {
        let dims = build_workflow_dimensions(
            ResearchWorkflow::SecurityReview,
            "auth library security",
            &[],
            ResearchDepth::Standard,
        );
        assert_eq!(dims.len(), 5);
    }

    #[test]
    fn performance_investigation_generates_4_dimensions() {
        let dims = build_workflow_dimensions(
            ResearchWorkflow::PerformanceInvestigation,
            "async runtime performance",
            &[],
            ResearchDepth::Standard,
        );
        assert_eq!(dims.len(), 4);
    }

    #[test]
    fn ecosystem_survey_generates_5_dimensions() {
        let dims = build_workflow_dimensions(
            ResearchWorkflow::EcosystemSurvey,
            "rust web frameworks",
            &[],
            ResearchDepth::Standard,
        );
        assert_eq!(dims.len(), 5);
    }

    #[test]
    fn general_generates_5_dimensions() {
        let dims = build_workflow_dimensions(
            ResearchWorkflow::General,
            "consensus algorithms",
            &[],
            ResearchDepth::Standard,
        );
        assert_eq!(dims.len(), 5);
    }

    #[test]
    fn quick_depth_limits_subqueries() {
        let dims = build_workflow_dimensions(
            ResearchWorkflow::ArchitectureDecision,
            "test",
            &[],
            ResearchDepth::Quick,
        );
        let total: usize = dims.iter().map(|d| d.subqueries.len()).sum();
        assert!(
            total <= 4,
            "quick depth should limit to 4 subqueries, got {total}"
        );
    }

    #[test]
    fn deep_depth_allows_more_subqueries() {
        let dims = build_workflow_dimensions(
            ResearchWorkflow::ArchitectureDecision,
            "test",
            &[],
            ResearchDepth::Deep,
        );
        let total: usize = dims.iter().map(|d| d.subqueries.len()).sum();
        assert!(
            total <= 12,
            "deep depth should allow up to 12 subqueries, got {total}"
        );
    }

    #[test]
    fn coverage_counts_results() {
        let groups = vec![
            make_group(ResearchResultGroupKind::PrimarySources, 3),
            make_group(ResearchResultGroupKind::OfficialDocs, 2),
            make_group(ResearchResultGroupKind::Benchmarks, 1),
            make_group(ResearchResultGroupKind::SecurityConsiderations, 4),
            make_group(ResearchResultGroupKind::Counterpoints, 1),
            make_group(ResearchResultGroupKind::RecentNews, 2),
        ];
        let coverage = compute_coverage(&groups);
        // Both PrimarySources and OfficialDocs contribute to primary_sources_found
        // and official_docs_found (they are both "primary" in the broader sense)
        assert_eq!(coverage.primary_sources_found, 5);
        assert_eq!(coverage.official_docs_found, 5);
        assert_eq!(coverage.implementation_sources_found, 0);
        assert_eq!(coverage.benchmark_sources_found, 1);
        assert_eq!(coverage.security_sources_found, 4);
        assert_eq!(coverage.counterpoints_found, 1);
        assert_eq!(coverage.recent_sources_found, 2);
    }

    #[test]
    fn gap_no_primary_sources_for_architecture_decision() {
        let req = ResearchSearchRequest {
            query: "test".to_string(),
            ..Default::default()
        };
        let coverage = ResearchCoverage::default();
        let gaps = detect_gaps(
            ResearchWorkflow::ArchitectureDecision,
            &req,
            &coverage,
            &["duckduckgo".to_string()],
        );
        assert!(gaps
            .iter()
            .any(|g| g.kind == ResearchGapKind::NoPrimarySources));
    }

    #[test]
    fn gap_no_counterpoints_when_requested() {
        let req = ResearchSearchRequest {
            query: "test".to_string(),
            include_counterpoints: Some(true),
            ..Default::default()
        };
        let coverage = ResearchCoverage::default();
        let gaps = detect_gaps(
            ResearchWorkflow::General,
            &req,
            &coverage,
            &["duckduckgo".to_string()],
        );
        assert!(gaps
            .iter()
            .any(|g| g.kind == ResearchGapKind::NoCounterpoints));
    }

    #[test]
    fn gap_no_benchmarks_for_performance() {
        let req = ResearchSearchRequest {
            query: "test".to_string(),
            ..Default::default()
        };
        let coverage = ResearchCoverage::default();
        let gaps = detect_gaps(
            ResearchWorkflow::PerformanceInvestigation,
            &req,
            &coverage,
            &["duckduckgo".to_string()],
        );
        assert!(gaps.iter().any(|g| g.kind == ResearchGapKind::NoBenchmarks));
    }

    #[test]
    fn gap_no_security_for_security_review() {
        let req = ResearchSearchRequest {
            query: "test".to_string(),
            ..Default::default()
        };
        let coverage = ResearchCoverage::default();
        let gaps = detect_gaps(
            ResearchWorkflow::SecurityReview,
            &req,
            &coverage,
            &["duckduckgo".to_string()],
        );
        assert!(gaps
            .iter()
            .any(|g| g.kind == ResearchGapKind::NoSecurityDiscussion));
    }

    #[test]
    fn gap_provider_limited_when_single_provider() {
        let req = ResearchSearchRequest {
            query: "test".to_string(),
            ..Default::default()
        };
        let coverage = ResearchCoverage::default();
        let gaps = detect_gaps(
            ResearchWorkflow::General,
            &req,
            &coverage,
            &["duckduckgo".to_string()],
        );
        assert!(gaps
            .iter()
            .any(|g| g.kind == ResearchGapKind::ProviderCoverageLimited));
    }

    #[test]
    fn no_gap_provider_limited_with_multiple_providers() {
        let req = ResearchSearchRequest {
            query: "test".to_string(),
            ..Default::default()
        };
        let coverage = ResearchCoverage::default();
        let gaps = detect_gaps(
            ResearchWorkflow::General,
            &req,
            &coverage,
            &["duckduckgo".to_string(), "brave".to_string()],
        );
        assert!(!gaps
            .iter()
            .any(|g| g.kind == ResearchGapKind::ProviderCoverageLimited));
    }

    #[test]
    fn diversity_cap_truncates_groups() {
        let groups = vec![make_group(ResearchResultGroupKind::OfficialDocs, 10)];
        let (capped, warnings) = apply_diversity_caps(groups, 3);
        assert_eq!(capped[0].results.len(), 3);
        assert!(capped[0].truncated);
        assert!(!warnings.is_empty());
    }

    #[test]
    fn diversity_cap_no_truncation_when_under() {
        let groups = vec![make_group(ResearchResultGroupKind::OfficialDocs, 2)];
        let (capped, warnings) = apply_diversity_caps(groups, 5);
        assert_eq!(capped[0].results.len(), 2);
        assert!(!capped[0].truncated);
        assert!(warnings.is_empty());
    }

    #[test]
    fn build_workflow_context_shapes() {
        let req = ResearchSearchRequest {
            query: "compare axum vs actix".to_string(),
            workflow: Some(ResearchWorkflow::LibraryComparison),
            depth: Some(ResearchDepth::Standard),
            compare_targets: vec!["axum".to_string(), "actix-web".to_string()],
            include_counterpoints: Some(true),
            ..Default::default()
        };
        let groups = vec![
            make_group(ResearchResultGroupKind::OfficialDocs, 3),
            make_group(ResearchResultGroupKind::Benchmarks, 1),
        ];
        let ctx = build_workflow_context(&req, &groups, &[], &["duckduckgo".to_string()]);
        assert_eq!(ctx.workflow, ResearchWorkflow::LibraryComparison);
        assert!(ctx.interpreted_question.contains("axum"));
        assert!(ctx.interpreted_question.contains("actix-web"));
        assert!(!ctx.dimensions.is_empty());
        assert_eq!(ctx.coverage.official_docs_found, 3);
        assert_eq!(ctx.coverage.benchmark_sources_found, 1);
    }

    #[test]
    fn build_interpreted_question_variants() {
        let req = ResearchSearchRequest {
            query: "test".to_string(),
            compare_targets: vec!["a".to_string(), "b".to_string()],
            ..Default::default()
        };
        assert!(build_interpreted_question(&req, ResearchWorkflow::General).contains("test"));
        assert!(
            build_interpreted_question(&req, ResearchWorkflow::LibraryComparison)
                .contains("a vs b")
        );
        assert!(
            build_interpreted_question(&req, ResearchWorkflow::MigrationPlanning)
                .contains("Migration planning")
        );
        assert!(
            build_interpreted_question(&req, ResearchWorkflow::SecurityReview)
                .contains("Security review")
        );
    }

    #[test]
    fn telemetry_builds_correctly() {
        let req = ResearchSearchRequest {
            query: "test".to_string(),
            workflow: Some(ResearchWorkflow::ApiEvaluation),
            depth: Some(ResearchDepth::Deep),
            ..Default::default()
        };
        let dims = vec![ResearchDimension {
            name: "test".to_string(),
            purpose: "test".to_string(),
            source_types: vec![],
            subqueries: vec!["q1".to_string(), "q2".to_string()],
        }];
        let gaps = vec![ResearchGap {
            kind: ResearchGapKind::NoPrimarySources,
            message: "test".to_string(),
            suggested_query: None,
        }];
        let telem = build_research_telemetry(&req, &dims, 2, &["cap1".to_string()], &gaps);
        assert_eq!(telem.workflow, Some(ResearchWorkflow::ApiEvaluation));
        assert_eq!(telem.depth, ResearchDepth::Deep);
        assert_eq!(telem.dimensions_generated, 1);
        assert_eq!(telem.subqueries_generated, 2);
        assert_eq!(telem.source_diversity_caps_applied, vec!["cap1"]);
        assert_eq!(telem.coverage_gaps, vec![ResearchGapKind::NoPrimarySources]);
    }

    // === Task 7: Backward compatibility and deterministic behavior ===

    #[test]
    fn request_without_workflow_defaults_to_general() {
        let req = ResearchSearchRequest {
            query: "test".to_string(),
            ..Default::default()
        };
        assert_eq!(req.effective_workflow(), ResearchWorkflow::General);
        assert_eq!(req.effective_depth(), ResearchDepth::Standard);
    }

    #[test]
    fn build_workflow_context_without_workflow_field() {
        let req = ResearchSearchRequest {
            query: "consensus algorithms".to_string(),
            ..Default::default()
        };
        let groups = vec![make_group(ResearchResultGroupKind::OfficialDocs, 2)];
        let ctx = build_workflow_context(&req, &groups, &[], &["duckduckgo".to_string()]);
        assert_eq!(ctx.workflow, ResearchWorkflow::General);
        assert!(!ctx.dimensions.is_empty());
    }

    #[test]
    fn architecture_decision_dimensions_are_deterministic() {
        let query = "microservices vs monolith";
        let targets: Vec<String> = vec![];
        let d1 = build_workflow_dimensions(
            ResearchWorkflow::ArchitectureDecision,
            query,
            &targets,
            ResearchDepth::Standard,
        );
        let d2 = build_workflow_dimensions(
            ResearchWorkflow::ArchitectureDecision,
            query,
            &targets,
            ResearchDepth::Standard,
        );
        assert_eq!(d1.len(), d2.len());
        for (a, b) in d1.iter().zip(d2.iter()) {
            assert_eq!(a.name, b.name);
            assert_eq!(a.purpose, b.purpose);
            assert_eq!(a.source_types, b.source_types);
            assert_eq!(a.subqueries, b.subqueries);
        }
    }

    #[test]
    fn api_evaluation_dimensions_are_deterministic() {
        let query = "axum web framework";
        let targets: Vec<String> = vec![];
        let d1 = build_workflow_dimensions(
            ResearchWorkflow::ApiEvaluation,
            query,
            &targets,
            ResearchDepth::Deep,
        );
        let d2 = build_workflow_dimensions(
            ResearchWorkflow::ApiEvaluation,
            query,
            &targets,
            ResearchDepth::Deep,
        );
        assert_eq!(d1.len(), d2.len());
        for (a, b) in d1.iter().zip(d2.iter()) {
            assert_eq!(a.name, b.name);
            assert_eq!(a.subqueries, b.subqueries);
        }
    }

    #[test]
    fn library_comparison_multiple_compare_targets() {
        let targets = vec![
            "axum".to_string(),
            "actix-web".to_string(),
            "rocket".to_string(),
        ];
        let dims = build_workflow_dimensions(
            ResearchWorkflow::LibraryComparison,
            "web framework comparison",
            &targets,
            ResearchDepth::Deep,
        );
        // First dimension "Official Docs per Target" should have one subquery per target
        assert_eq!(dims[0].subqueries.len(), 3);
        assert!(dims[0].subqueries[0].contains("axum"));
        assert!(dims[0].subqueries[1].contains("actix-web"));
        assert!(dims[0].subqueries[2].contains("rocket"));
        // "Per-Target Deep Dives" dimension should exist when targets > 1 and depth allows
        let deep_dive = dims.iter().find(|d| d.name == "Per-Target Deep Dives");
        assert!(deep_dive.is_some());
        // Deep dives bounded to 3 even if more targets
        assert!(deep_dive.unwrap().subqueries.len() <= 3);
    }

    #[test]
    fn depth_affects_subquery_count_ordering() {
        let query = "test query";
        let targets: Vec<String> = vec![];

        let quick = build_workflow_dimensions(
            ResearchWorkflow::ArchitectureDecision,
            query,
            &targets,
            ResearchDepth::Quick,
        );
        let standard = build_workflow_dimensions(
            ResearchWorkflow::ArchitectureDecision,
            query,
            &targets,
            ResearchDepth::Standard,
        );
        let deep = build_workflow_dimensions(
            ResearchWorkflow::ArchitectureDecision,
            query,
            &targets,
            ResearchDepth::Deep,
        );

        let q_total: usize = quick.iter().map(|d| d.subqueries.len()).sum();
        let s_total: usize = standard.iter().map(|d| d.subqueries.len()).sum();
        let d_total: usize = deep.iter().map(|d| d.subqueries.len()).sum();

        assert!(q_total <= 4, "quick should be <= 4, got {q_total}");
        assert!(s_total <= 8, "standard should be <= 8, got {s_total}");
        assert!(d_total <= 12, "deep should be <= 12, got {d_total}");
        assert!(
            q_total < s_total,
            "quick ({q_total}) should have fewer than standard ({s_total})"
        );
        // Standard and deep may both reach the workflow's natural dimension count
        // (ArchitectureDecision has 7 dims / 8 subqueries, both ≤ 8 and ≤ 12)
        assert!(
            s_total <= d_total,
            "standard ({s_total}) should be <= deep ({d_total})"
        );
    }

    #[test]
    fn depth_strict_ordering_for_high_subquery_workflow() {
        let targets = vec!["alpha".to_string(), "beta".to_string(), "gamma".to_string()];
        // LibraryComparison with 3 targets generates 11 subqueries at full depth
        let quick = build_workflow_dimensions(
            ResearchWorkflow::LibraryComparison,
            "compare",
            &targets,
            ResearchDepth::Quick,
        );
        let standard = build_workflow_dimensions(
            ResearchWorkflow::LibraryComparison,
            "compare",
            &targets,
            ResearchDepth::Standard,
        );
        let deep = build_workflow_dimensions(
            ResearchWorkflow::LibraryComparison,
            "compare",
            &targets,
            ResearchDepth::Deep,
        );

        let q: usize = quick.iter().map(|d| d.subqueries.len()).sum();
        let s: usize = standard.iter().map(|d| d.subqueries.len()).sum();
        let d: usize = deep.iter().map(|d| d.subqueries.len()).sum();

        assert!(q <= 4, "quick should be <= 4, got {q}");
        assert!(s <= 8, "standard should be <= 8, got {s}");
        assert!(d <= 12, "deep should be <= 12, got {d}");
        assert!(q < s, "quick ({q}) < standard ({s})");
        assert!(s < d, "standard ({s}) < deep ({d})");
    }

    #[test]
    fn no_primary_sources_gap_only_when_absent() {
        let req = ResearchSearchRequest {
            query: "test".to_string(),
            ..Default::default()
        };
        // With no primary sources
        let empty_coverage = ResearchCoverage::default();
        let gaps = detect_gaps(
            ResearchWorkflow::ArchitectureDecision,
            &req,
            &empty_coverage,
            &["duckduckgo".to_string()],
        );
        assert!(
            gaps.iter()
                .any(|g| g.kind == ResearchGapKind::NoPrimarySources),
            "should emit NoPrimarySources when none found"
        );

        // With primary sources present
        let mut coverage = ResearchCoverage::default();
        coverage.primary_sources_found = 3;
        let gaps = detect_gaps(
            ResearchWorkflow::ArchitectureDecision,
            &req,
            &coverage,
            &["duckduckgo".to_string()],
        );
        assert!(
            !gaps
                .iter()
                .any(|g| g.kind == ResearchGapKind::NoPrimarySources),
            "should NOT emit NoPrimarySources when sources exist"
        );
    }

    #[test]
    fn no_counterpoints_gap_only_when_requested() {
        let req_not_requested = ResearchSearchRequest {
            query: "test".to_string(),
            include_counterpoints: Some(false),
            ..Default::default()
        };
        let coverage = ResearchCoverage::default();
        let gaps = detect_gaps(
            ResearchWorkflow::General,
            &req_not_requested,
            &coverage,
            &["duckduckgo".to_string()],
        );
        assert!(
            !gaps
                .iter()
                .any(|g| g.kind == ResearchGapKind::NoCounterpoints),
            "should NOT emit NoCounterpoints when not requested"
        );

        let req_requested = ResearchSearchRequest {
            query: "test".to_string(),
            include_counterpoints: Some(true),
            ..Default::default()
        };
        let gaps = detect_gaps(
            ResearchWorkflow::General,
            &req_requested,
            &coverage,
            &["duckduckgo".to_string()],
        );
        assert!(
            gaps.iter()
                .any(|g| g.kind == ResearchGapKind::NoCounterpoints),
            "should emit NoCounterpoints when requested but none found"
        );
    }

    #[test]
    fn no_counterpoints_not_emitted_when_not_requested() {
        let req = ResearchSearchRequest {
            query: "test".to_string(),
            include_counterpoints: None,
            ..Default::default()
        };
        let coverage = ResearchCoverage::default();
        let gaps = detect_gaps(
            ResearchWorkflow::General,
            &req,
            &coverage,
            &["duckduckgo".to_string()],
        );
        assert!(
            !gaps
                .iter()
                .any(|g| g.kind == ResearchGapKind::NoCounterpoints),
            "should NOT emit NoCounterpoints when include_counterpoints is None"
        );
    }

    #[test]
    fn no_benchmarks_only_for_relevant_workflows() {
        let req = ResearchSearchRequest {
            query: "test".to_string(),
            ..Default::default()
        };
        let coverage = ResearchCoverage::default();

        // Should appear for PerformanceInvestigation
        let gaps = detect_gaps(
            ResearchWorkflow::PerformanceInvestigation,
            &req,
            &coverage,
            &["duckduckgo".to_string()],
        );
        assert!(
            gaps.iter().any(|g| g.kind == ResearchGapKind::NoBenchmarks),
            "should emit NoBenchmarks for PerformanceInvestigation"
        );

        // Should appear for LibraryComparison
        let gaps = detect_gaps(
            ResearchWorkflow::LibraryComparison,
            &req,
            &coverage,
            &["duckduckgo".to_string()],
        );
        assert!(
            gaps.iter().any(|g| g.kind == ResearchGapKind::NoBenchmarks),
            "should emit NoBenchmarks for LibraryComparison"
        );

        // Should NOT appear for General
        let gaps = detect_gaps(
            ResearchWorkflow::General,
            &req,
            &coverage,
            &["duckduckgo".to_string()],
        );
        assert!(
            !gaps.iter().any(|g| g.kind == ResearchGapKind::NoBenchmarks),
            "should NOT emit NoBenchmarks for General workflow"
        );

        // Should NOT appear for SecurityReview
        let gaps = detect_gaps(
            ResearchWorkflow::SecurityReview,
            &req,
            &coverage,
            &["duckduckgo".to_string()],
        );
        assert!(
            !gaps.iter().any(|g| g.kind == ResearchGapKind::NoBenchmarks),
            "should NOT emit NoBenchmarks for SecurityReview"
        );
    }

    #[test]
    fn diversity_cap_truncation_is_deterministic() {
        let groups = vec![
            make_group(ResearchResultGroupKind::OfficialDocs, 10),
            make_group(ResearchResultGroupKind::Benchmarks, 5),
            make_group(ResearchResultGroupKind::SecurityConsiderations, 8),
        ];
        let (capped1, warnings1) = apply_diversity_caps(groups.clone(), 3);
        let (capped2, warnings2) = apply_diversity_caps(groups, 3);

        assert_eq!(capped1.len(), capped2.len());
        for (a, b) in capped1.iter().zip(capped2.iter()) {
            assert_eq!(a.results.len(), b.results.len());
            assert_eq!(a.truncated, b.truncated);
            assert_eq!(a.label, b.label);
        }
        assert_eq!(warnings1, warnings2);
    }

    #[test]
    fn diversity_cap_emits_warnings_per_capped_group() {
        let groups = vec![
            make_group(ResearchResultGroupKind::OfficialDocs, 10),
            make_group(ResearchResultGroupKind::Benchmarks, 2),
            make_group(ResearchResultGroupKind::SecurityConsiderations, 8),
        ];
        let (capped, warnings) = apply_diversity_caps(groups, 3);
        // Two groups exceeded cap
        assert_eq!(warnings.len(), 2);
        assert!(warnings[0].contains("OfficialDocs"));
        assert!(warnings[0].contains("10"));
        assert!(warnings[0].contains("3"));
        assert!(warnings[1].contains("SecurityConsiderations"));
        // The benchmarks group was not capped
        assert!(!capped[1].truncated);
        assert_eq!(capped[1].results.len(), 2);
    }

    #[test]
    fn suggested_fetches_not_all_from_same_group() {
        let req = ResearchSearchRequest {
            query: "compare axum vs actix".to_string(),
            workflow: Some(ResearchWorkflow::LibraryComparison),
            depth: Some(ResearchDepth::Standard),
            compare_targets: vec!["axum".to_string(), "actix-web".to_string()],
            include_counterpoints: Some(true),
            ..Default::default()
        };
        let groups = vec![
            make_group(ResearchResultGroupKind::OfficialDocs, 3),
            make_group(ResearchResultGroupKind::Benchmarks, 2),
            make_group(ResearchResultGroupKind::SecurityConsiderations, 1),
        ];
        // Create suggested fetches from different groups
        let fetches = vec![
            crate::core::research::ResearchSuggestedFetch {
                url: "https://docs.rs/axum".to_string(),
                group: ResearchResultGroupKind::OfficialDocs,
                expected_kind: SourceKind::OfficialDocs,
                evidence_quality: crate::core::research::EvidenceQuality::OfficialPrimary,
                reason: "official docs".to_string(),
                recommended_extract_mode: None,
                priority: 1,
            },
            crate::core::research::ResearchSuggestedFetch {
                url: "https://benchmarks.example.com".to_string(),
                group: ResearchResultGroupKind::Benchmarks,
                expected_kind: SourceKind::Reference,
                evidence_quality: crate::core::research::EvidenceQuality::BenchmarkOrMeasurement,
                reason: "benchmark".to_string(),
                recommended_extract_mode: None,
                priority: 2,
            },
        ];
        let ctx = build_workflow_context(&req, &groups, &fetches, &["duckduckgo".to_string()]);
        // Recommended next fetches should come from the provided fetches
        assert_eq!(ctx.recommended_next_fetches.len(), 2);
        let groups_in_fetches: Vec<_> = ctx
            .recommended_next_fetches
            .iter()
            .map(|f| f.group)
            .collect();
        assert!(groups_in_fetches.contains(&ResearchResultGroupKind::OfficialDocs));
        assert!(groups_in_fetches.contains(&ResearchResultGroupKind::Benchmarks));
    }

    #[test]
    fn telemetry_reports_workflow_depth_dimensions_gaps() {
        let req = ResearchSearchRequest {
            query: "test telemetry".to_string(),
            workflow: Some(ResearchWorkflow::ArchitectureDecision),
            depth: Some(ResearchDepth::Deep),
            ..Default::default()
        };
        let dims = build_workflow_dimensions(
            ResearchWorkflow::ArchitectureDecision,
            &req.query,
            &req.compare_targets,
            req.effective_depth(),
        );
        let total_subqueries: usize = dims.iter().map(|d| d.subqueries.len()).sum();

        let gaps = vec![ResearchGap {
            kind: ResearchGapKind::NoBenchmarks,
            message: "test gap".to_string(),
            suggested_query: None,
        }];

        let telem = build_research_telemetry(&req, &dims, total_subqueries, &[], &gaps);

        assert_eq!(telem.workflow, Some(ResearchWorkflow::ArchitectureDecision));
        assert_eq!(telem.depth, ResearchDepth::Deep);
        assert_eq!(telem.dimensions_generated, dims.len());
        assert_eq!(telem.subqueries_generated, total_subqueries);
        assert!(telem.source_diversity_caps_applied.is_empty());
        assert_eq!(telem.coverage_gaps, vec![ResearchGapKind::NoBenchmarks]);
    }

    #[test]
    fn telemetry_without_workflow_has_none() {
        let req = ResearchSearchRequest {
            query: "test".to_string(),
            ..Default::default()
        };
        let dims = vec![];
        let telem = build_research_telemetry(&req, &dims, 0, &[], &[]);
        assert_eq!(telem.workflow, None);
        assert_eq!(telem.depth, ResearchDepth::Standard);
        assert_eq!(telem.dimensions_generated, 0);
        assert_eq!(telem.subqueries_generated, 0);
    }

    #[test]
    fn all_workflows_produce_deterministic_dimensions() {
        let workflows = [
            ResearchWorkflow::ArchitectureDecision,
            ResearchWorkflow::ApiEvaluation,
            ResearchWorkflow::LibraryComparison,
            ResearchWorkflow::MigrationPlanning,
            ResearchWorkflow::SecurityReview,
            ResearchWorkflow::PerformanceInvestigation,
            ResearchWorkflow::EcosystemSurvey,
            ResearchWorkflow::General,
        ];
        let query = "deterministic test query";
        let targets = vec!["alpha".to_string(), "beta".to_string()];

        for workflow in &workflows {
            let d1 = build_workflow_dimensions(*workflow, query, &targets, ResearchDepth::Deep);
            let d2 = build_workflow_dimensions(*workflow, query, &targets, ResearchDepth::Deep);
            assert_eq!(
                d1.len(),
                d2.len(),
                "dimension count mismatch for {workflow:?}"
            );
            for (i, (a, b)) in d1.iter().zip(d2.iter()).enumerate() {
                assert_eq!(a.name, b.name, "name mismatch at dim {i} for {workflow:?}");
                assert_eq!(
                    a.subqueries, b.subqueries,
                    "subqueries mismatch at dim {i} for {workflow:?}"
                );
            }
        }
    }

    #[test]
    fn library_comparison_deterministic_with_targets() {
        let targets = vec![
            "axum".to_string(),
            "actix-web".to_string(),
            "rocket".to_string(),
        ];
        let d1 = build_workflow_dimensions(
            ResearchWorkflow::LibraryComparison,
            "web framework comparison",
            &targets,
            ResearchDepth::Deep,
        );
        let d2 = build_workflow_dimensions(
            ResearchWorkflow::LibraryComparison,
            "web framework comparison",
            &targets,
            ResearchDepth::Deep,
        );
        assert_eq!(d1.len(), d2.len());
        for (i, (a, b)) in d1.iter().zip(d2.iter()).enumerate() {
            assert_eq!(a.name, b.name, "name mismatch at dim {i}");
            assert_eq!(a.subqueries, b.subqueries, "subqueries mismatch at dim {i}");
        }
    }

    #[test]
    fn no_recent_sources_gap_for_ecosystem_survey() {
        let req = ResearchSearchRequest {
            query: "test".to_string(),
            ..Default::default()
        };
        let coverage = ResearchCoverage::default();
        let gaps = detect_gaps(
            ResearchWorkflow::EcosystemSurvey,
            &req,
            &coverage,
            &["duckduckgo".to_string()],
        );
        assert!(
            gaps.iter()
                .any(|g| g.kind == ResearchGapKind::NoRecentSources),
            "should emit NoRecentSources for EcosystemSurvey with no recent sources"
        );
    }

    #[test]
    fn no_recent_sources_gap_for_performance_investigation() {
        let req = ResearchSearchRequest {
            query: "test".to_string(),
            ..Default::default()
        };
        let coverage = ResearchCoverage::default();
        let gaps = detect_gaps(
            ResearchWorkflow::PerformanceInvestigation,
            &req,
            &coverage,
            &["duckduckgo".to_string()],
        );
        assert!(
            gaps.iter()
                .any(|g| g.kind == ResearchGapKind::NoRecentSources),
            "should emit NoRecentSources for PerformanceInvestigation with no recent sources"
        );
    }

    #[test]
    fn no_recent_sources_not_emitted_for_general() {
        let req = ResearchSearchRequest {
            query: "test".to_string(),
            ..Default::default()
        };
        let coverage = ResearchCoverage::default();
        let gaps = detect_gaps(
            ResearchWorkflow::General,
            &req,
            &coverage,
            &["duckduckgo".to_string()],
        );
        assert!(
            !gaps
                .iter()
                .any(|g| g.kind == ResearchGapKind::NoRecentSources),
            "should NOT emit NoRecentSources for General workflow"
        );
    }

    #[test]
    fn no_implementation_evidence_for_architecture_decision() {
        let req = ResearchSearchRequest {
            query: "test".to_string(),
            ..Default::default()
        };
        let coverage = ResearchCoverage::default();
        let gaps = detect_gaps(
            ResearchWorkflow::ArchitectureDecision,
            &req,
            &coverage,
            &["duckduckgo".to_string()],
        );
        assert!(
            gaps.iter()
                .any(|g| g.kind == ResearchGapKind::NoImplementationEvidence),
            "should emit NoImplementationEvidence for ArchitectureDecision"
        );
    }

    #[test]
    fn no_implementation_evidence_not_emitted_for_general() {
        let req = ResearchSearchRequest {
            query: "test".to_string(),
            ..Default::default()
        };
        let coverage = ResearchCoverage::default();
        let gaps = detect_gaps(
            ResearchWorkflow::General,
            &req,
            &coverage,
            &["duckduckgo".to_string()],
        );
        assert!(
            !gaps
                .iter()
                .any(|g| g.kind == ResearchGapKind::NoImplementationEvidence),
            "should NOT emit NoImplementationEvidence for General workflow"
        );
    }

    #[test]
    fn no_migration_docs_for_migration_planning() {
        let req = ResearchSearchRequest {
            query: "test".to_string(),
            ..Default::default()
        };
        let coverage = ResearchCoverage::default();
        let gaps = detect_gaps(
            ResearchWorkflow::MigrationPlanning,
            &req,
            &coverage,
            &["duckduckgo".to_string()],
        );
        assert!(
            gaps.iter()
                .any(|g| g.kind == ResearchGapKind::NoMigrationDocs),
            "should emit NoMigrationDocs for MigrationPlanning"
        );
    }

    #[test]
    fn no_migration_docs_not_emitted_for_general() {
        let req = ResearchSearchRequest {
            query: "test".to_string(),
            ..Default::default()
        };
        let coverage = ResearchCoverage::default();
        let gaps = detect_gaps(
            ResearchWorkflow::General,
            &req,
            &coverage,
            &["duckduckgo".to_string()],
        );
        assert!(
            !gaps
                .iter()
                .any(|g| g.kind == ResearchGapKind::NoMigrationDocs),
            "should NOT emit NoMigrationDocs for General workflow"
        );
    }
}
