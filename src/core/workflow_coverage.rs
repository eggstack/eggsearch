use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::core::evidence_role::EvidenceRole;
use crate::core::workflow::AgentNextAction;

/// Workflow kind for coverage model selection.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowKind {
    /// API comprehension and understanding tasks.
    ApiComprehension,
    /// Repository architecture understanding tasks.
    RepositoryArchitecture,
    /// Error investigation and debugging tasks.
    ErrorInvestigation,
    /// Version migration and upgrade tasks.
    VersionMigration,
    /// Security review and vulnerability assessment tasks.
    SecurityReview,
    /// Dependency evaluation and assessment tasks.
    DependencyEvaluation,
    /// Performance investigation and benchmarking tasks.
    PerformanceInvestigation,
    /// Comparative research across multiple options.
    ComparativeResearch,
    /// Pre-change evidence gathering tasks.
    PreChangeEvidence,
    /// Post-change review and validation tasks.
    PostChangeReview,
}

impl WorkflowKind {
    /// Stable snake-case string form.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ApiComprehension => "api_comprehension",
            Self::RepositoryArchitecture => "repository_architecture",
            Self::ErrorInvestigation => "error_investigation",
            Self::VersionMigration => "version_migration",
            Self::SecurityReview => "security_review",
            Self::DependencyEvaluation => "dependency_evaluation",
            Self::PerformanceInvestigation => "performance_investigation",
            Self::ComparativeResearch => "comparative_research",
            Self::PreChangeEvidence => "pre_change_evidence",
            Self::PostChangeReview => "post_change_review",
        }
    }

    /// Parse a workflow kind string, accepting stable names and short aliases.
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "api_comprehension" | "api" => Some(Self::ApiComprehension),
            "repository_architecture" | "repo_architecture" | "architecture" => {
                Some(Self::RepositoryArchitecture)
            }
            "error_investigation" | "error" => Some(Self::ErrorInvestigation),
            "version_migration" | "migration" => Some(Self::VersionMigration),
            "security_review" | "security" => Some(Self::SecurityReview),
            "dependency_evaluation" | "dependency" => Some(Self::DependencyEvaluation),
            "performance_investigation" | "performance" => Some(Self::PerformanceInvestigation),
            "comparative_research" | "research" | "comparative" => Some(Self::ComparativeResearch),
            "pre_change_evidence" | "pre_change" => Some(Self::PreChangeEvidence),
            "post_change_review" | "post_change" => Some(Self::PostChangeReview),
            _ => None,
        }
    }

    /// Convert this workflow kind to its corresponding coverage model.
    pub fn to_model(self) -> WorkflowCoverageModel {
        match self {
            Self::ApiComprehension => api_comprehension_model(),
            Self::RepositoryArchitecture => repo_architecture_model(),
            Self::ErrorInvestigation => error_investigation_model(),
            Self::VersionMigration => version_migration_model(),
            Self::SecurityReview => security_review_model(),
            Self::DependencyEvaluation => dependency_evaluation_model(),
            Self::PerformanceInvestigation => performance_investigation_model(),
            Self::ComparativeResearch => comparative_research_model(),
            Self::PreChangeEvidence => pre_change_evidence_model(),
            Self::PostChangeReview => post_change_review_model(),
        }
    }
}

/// Indicates which layer of the resolution precedence selected the workflow model.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ResolutionSource {
    /// Explicit workflow parameter from the request.
    ExplicitWorkflow,
    /// Profile-based resolution (e.g. security, research).
    Profile,
    /// Mode-based resolution (e.g. exact_error).
    Mode,
    /// Research domain-based resolution.
    Domain,
    /// Default fallback when no higher-precedence signal is available.
    Default,
}

/// Context for workflow model resolution, carrying all signals that
/// influence which coverage model is selected.
pub struct WorkflowResolutionContext<'a> {
    /// The MCP tool calling the resolver.
    pub tool: &'a str,
    /// Explicit workflow kind from the request, highest precedence.
    pub workflow: Option<WorkflowKind>,
    /// Search profile from the request.
    pub profile: Option<&'a str>,
    /// Research domain from the request.
    pub research_domain: Option<&'a str>,
    /// Whether exact-error mode is active.
    pub exact_error: bool,
}

/// Overall coverage status for a workflow given the evidence that was found.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CoverageStatus {
    /// All required and recommended roles are covered.
    Sufficient,
    /// All required roles are covered but some recommended roles are missing.
    UsableWithGaps,
    /// One or more required roles are missing.
    Insufficient,
    /// Coverage cannot be determined because some retrievals failed.
    IndeterminateDueToFailures,
}

/// Distinguishes why evidence for a role was not found.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RetrievalFailureKind {
    /// No evidence matched the role query.
    NoMatchingEvidenceFound,
    /// No provider supports the capability needed for this role.
    ProviderCapabilityUnavailable,
    /// The provider was skipped due to policy configuration.
    ProviderSkippedByPolicy,
    /// The provider returned an error.
    ProviderFailed,
    /// The retrieval timed out or hit a deadline.
    DeadlinePreventedCompletion,
    /// Results were truncated by a cap limit.
    ResultTruncatedByCap,
    /// The role was not requested in the original query.
    EvidenceRoleNotRequested,
    /// The role was requested but no evidence was found.
    EvidenceRoleRequestedButNotFound,
    /// The role status is indeterminate because the retrieval itself failed.
    EvidenceRoleIndeterminateBecauseRetrievalFailed,
}

/// Describes why evidence for a specific role could not be retrieved.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct RetrievalFailure {
    /// The kind of failure.
    pub kind: RetrievalFailureKind,
    /// The evidence role that was affected.
    pub role: EvidenceRole,
    /// Human-readable description of the failure.
    pub message: String,
    /// The provider that failed, if applicable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_id: Option<String>,
}

/// Request to evaluate coverage for a workflow.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct WorkflowCoverageRequest {
    /// The workflow identifier.
    pub workflow_id: String,
    /// Roles that must be present for sufficient coverage.
    pub required_roles: Vec<EvidenceRole>,
    /// Roles that improve coverage but are not mandatory.
    pub recommended_roles: Vec<EvidenceRole>,
    /// Roles that are nice to have but not expected.
    pub optional_roles: Vec<EvidenceRole>,
}

/// Result of evaluating coverage for a workflow.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct WorkflowCoverageResult {
    /// The workflow identifier.
    pub workflow_id: String,
    /// Roles that must be present for sufficient coverage.
    pub required_roles: Vec<EvidenceRole>,
    /// Roles that improve coverage but are not mandatory.
    pub recommended_roles: Vec<EvidenceRole>,
    /// Roles that are nice to have but not expected.
    pub optional_roles: Vec<EvidenceRole>,
    /// Evidence roles that were found.
    pub found_roles: Vec<EvidenceRole>,
    /// Required roles that are missing.
    pub missing_required: Vec<EvidenceRole>,
    /// Recommended roles that are missing.
    pub missing_recommended: Vec<EvidenceRole>,
    /// Optional roles that are missing.
    pub missing_optional: Vec<EvidenceRole>,
    /// Failures that occurred during evidence retrieval.
    pub retrieval_failures: Vec<RetrievalFailure>,
    /// Overall coverage status.
    pub status: CoverageStatus,
    /// Confidence in completeness from 0.0 to 1.0.
    pub completion_confidence: f32,
    /// Human-readable reasons for the status.
    pub reasons: Vec<String>,
    /// Structured next-action hints driven by coverage gaps.
    pub next_actions: Vec<AgentNextAction>,
    /// Which layer of the resolution precedence selected this model.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolution_source: Option<ResolutionSource>,
}

/// Defines the evidence roles required, recommended, and optional for a workflow.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct WorkflowCoverageModel {
    /// The workflow identifier.
    pub workflow_id: String,
    /// Human-readable title.
    pub title: String,
    /// Roles that must be present for sufficient coverage.
    pub required: Vec<EvidenceRole>,
    /// Roles that improve coverage but are not mandatory.
    pub recommended: Vec<EvidenceRole>,
    /// Roles that are nice to have but not expected.
    pub optional: Vec<EvidenceRole>,
}

/// Workflow model for API comprehension tasks.
pub fn api_comprehension_model() -> WorkflowCoverageModel {
    WorkflowCoverageModel {
        workflow_id: "api_comprehension".to_string(),
        title: "API Comprehension".to_string(),
        required: vec![
            EvidenceRole::InterfaceOrApiDefinition,
            EvidenceRole::PrimaryImplementation,
        ],
        recommended: vec![
            EvidenceRole::OfficialDocumentation,
            EvidenceRole::UsageExample,
            EvidenceRole::TestOrBehavioralSpecification,
        ],
        optional: vec![],
    }
}

/// Workflow model for repository architecture understanding.
pub fn repo_architecture_model() -> WorkflowCoverageModel {
    WorkflowCoverageModel {
        workflow_id: "repo_architecture".to_string(),
        title: "Repository Architecture".to_string(),
        required: vec![
            EvidenceRole::PrimaryImplementation,
            EvidenceRole::ArchitectureOrDesignDocument,
        ],
        recommended: vec![
            EvidenceRole::OfficialDocumentation,
            EvidenceRole::ConfigurationOrFeatureGate,
            EvidenceRole::ManifestOrDependencyMetadata,
        ],
        optional: vec![],
    }
}

/// Workflow model for error investigation tasks.
pub fn error_investigation_model() -> WorkflowCoverageModel {
    WorkflowCoverageModel {
        workflow_id: "error_investigation".to_string(),
        title: "Error Investigation".to_string(),
        required: vec![
            EvidenceRole::IssueOrIncidentDiscussion,
            EvidenceRole::PrimaryImplementation,
        ],
        recommended: vec![
            EvidenceRole::OfficialDocumentation,
            EvidenceRole::TestOrBehavioralSpecification,
        ],
        optional: vec![],
    }
}

/// Workflow model for version migration tasks.
pub fn version_migration_model() -> WorkflowCoverageModel {
    WorkflowCoverageModel {
        workflow_id: "version_migration".to_string(),
        title: "Version Migration".to_string(),
        required: vec![
            EvidenceRole::ReleaseNoteOrChangelog,
            EvidenceRole::MigrationGuidance,
        ],
        recommended: vec![
            EvidenceRole::OfficialDocumentation,
            EvidenceRole::IssueOrIncidentDiscussion,
        ],
        optional: vec![],
    }
}

/// Workflow model for security review tasks.
pub fn security_review_model() -> WorkflowCoverageModel {
    WorkflowCoverageModel {
        workflow_id: "security_review".to_string(),
        title: "Security Review".to_string(),
        required: vec![
            EvidenceRole::AuthoritativeSecurityAdvisory,
            EvidenceRole::VendorSecurityGuidance,
        ],
        recommended: vec![
            EvidenceRole::PrimaryImplementation,
            EvidenceRole::ConfigurationOrFeatureGate,
            EvidenceRole::ManifestOrDependencyMetadata,
        ],
        optional: vec![],
    }
}

/// Workflow model for dependency evaluation tasks.
pub fn dependency_evaluation_model() -> WorkflowCoverageModel {
    WorkflowCoverageModel {
        workflow_id: "dependency_evaluation".to_string(),
        title: "Dependency Evaluation".to_string(),
        required: vec![EvidenceRole::ManifestOrDependencyMetadata],
        recommended: vec![
            EvidenceRole::OfficialDocumentation,
            EvidenceRole::ReleaseNoteOrChangelog,
            EvidenceRole::AuthoritativeSecurityAdvisory,
        ],
        optional: vec![],
    }
}

/// Workflow model for performance investigation tasks.
pub fn performance_investigation_model() -> WorkflowCoverageModel {
    WorkflowCoverageModel {
        workflow_id: "performance_investigation".to_string(),
        title: "Performance Investigation".to_string(),
        required: vec![EvidenceRole::BenchmarkOrPerformanceEvidence],
        recommended: vec![
            EvidenceRole::PrimaryImplementation,
            EvidenceRole::OfficialDocumentation,
            EvidenceRole::IndependentCorroboration,
        ],
        optional: vec![],
    }
}

/// Workflow model for comparative research tasks.
pub fn comparative_research_model() -> WorkflowCoverageModel {
    WorkflowCoverageModel {
        workflow_id: "comparative_research".to_string(),
        title: "Comparative Research".to_string(),
        required: vec![
            EvidenceRole::OfficialDocumentation,
            EvidenceRole::PrimaryImplementation,
        ],
        recommended: vec![
            EvidenceRole::BenchmarkOrPerformanceEvidence,
            EvidenceRole::IndependentCorroboration,
            EvidenceRole::CounterpointOrConflictingEvidence,
        ],
        optional: vec![],
    }
}

/// Workflow model for pre-change evidence gathering.
pub fn pre_change_evidence_model() -> WorkflowCoverageModel {
    WorkflowCoverageModel {
        workflow_id: "pre_change_evidence".to_string(),
        title: "Pre-Change Evidence".to_string(),
        required: vec![
            EvidenceRole::PrimaryImplementation,
            EvidenceRole::TestOrBehavioralSpecification,
        ],
        recommended: vec![
            EvidenceRole::OfficialDocumentation,
            EvidenceRole::ConfigurationOrFeatureGate,
        ],
        optional: vec![],
    }
}

/// Workflow model for post-change review tasks.
pub fn post_change_review_model() -> WorkflowCoverageModel {
    WorkflowCoverageModel {
        workflow_id: "post_change_review".to_string(),
        title: "Post-Change Review".to_string(),
        required: vec![EvidenceRole::TestOrBehavioralSpecification],
        recommended: vec![
            EvidenceRole::PrimaryImplementation,
            EvidenceRole::OfficialDocumentation,
            EvidenceRole::ConfigurationOrFeatureGate,
        ],
        optional: vec![],
    }
}

/// Determine the coverage status from a model, found evidence, and failures.
pub fn coverage_status(
    model: &WorkflowCoverageModel,
    found: &[EvidenceRole],
    failures: &[RetrievalFailure],
) -> CoverageStatus {
    let has_indeterminate = failures.iter().any(|f| {
        f.kind == RetrievalFailureKind::EvidenceRoleIndeterminateBecauseRetrievalFailed
            || (model.required.contains(&f.role)
                && matches!(
                    f.kind,
                    RetrievalFailureKind::ProviderCapabilityUnavailable
                        | RetrievalFailureKind::ProviderSkippedByPolicy
                ))
    });
    if has_indeterminate {
        return CoverageStatus::IndeterminateDueToFailures;
    }

    let found_set: std::collections::HashSet<EvidenceRole> = found.iter().copied().collect();
    let all_required_found = model.required.iter().all(|r| found_set.contains(r));
    if all_required_found {
        let all_recommended_found = model.recommended.iter().all(|r| found_set.contains(r));
        if all_recommended_found {
            return CoverageStatus::Sufficient;
        }
        return CoverageStatus::UsableWithGaps;
    }

    let has_provider_failure = failures.iter().any(|f| {
        matches!(
            f.kind,
            RetrievalFailureKind::ProviderFailed
                | RetrievalFailureKind::DeadlinePreventedCompletion
                | RetrievalFailureKind::ProviderCapabilityUnavailable
                | RetrievalFailureKind::ProviderSkippedByPolicy
        ) && model.required.contains(&f.role)
    });
    if has_provider_failure {
        return CoverageStatus::IndeterminateDueToFailures;
    }

    CoverageStatus::Insufficient
}

/// Determine the most productive MCP tool for filling a missing evidence role.
fn role_to_next_tool(role: &EvidenceRole) -> (&'static str, &'static str) {
    match role {
        EvidenceRole::PrimaryImplementation => ("repo_search", "fetch_primary_source"),
        EvidenceRole::InterfaceOrApiDefinition => ("web_search", "fetch_api_definition"),
        EvidenceRole::UsageExample => ("web_search", "fetch_usage_example"),
        EvidenceRole::TestOrBehavioralSpecification => ("repo_search", "fetch_test_spec"),
        EvidenceRole::ConfigurationOrFeatureGate => ("repo_search", "fetch_config"),
        EvidenceRole::ManifestOrDependencyMetadata => ("repo_search", "fetch_manifest"),
        EvidenceRole::OfficialDocumentation => ("web_search", "fetch_documentation"),
        EvidenceRole::ArchitectureOrDesignDocument => ("web_search", "fetch_architecture_doc"),
        EvidenceRole::ReleaseNoteOrChangelog => ("repo_search", "fetch_release_notes"),
        EvidenceRole::MigrationGuidance => ("web_search", "fetch_migration_guide"),
        EvidenceRole::BenchmarkOrPerformanceEvidence => ("web_search", "fetch_benchmark_evidence"),
        EvidenceRole::IssueOrIncidentDiscussion => ("repo_search", "fetch_issue_discussion"),
        EvidenceRole::PullRequestOrDesignReview => ("repo_search", "fetch_pr_review"),
        EvidenceRole::AuthoritativeSecurityAdvisory => {
            ("security_search", "fetch_security_advisory")
        }
        EvidenceRole::VendorSecurityGuidance => ("security_search", "fetch_vendor_guidance"),
        EvidenceRole::IndependentCorroboration => ("research_search", "fetch_corroboration"),
        EvidenceRole::CounterpointOrConflictingEvidence => {
            ("research_search", "fetch_counterpoint")
        }
        EvidenceRole::CommunityDiscussion => ("web_search", "fetch_community_discussion"),
        EvidenceRole::UnknownOrWeakContext => ("web_search", "fetch_general_context"),
    }
}

/// Generate gap-driven next actions from a coverage result, considering
/// retrieval history and tool context.
///
/// For each missing/indeterminate role, selects the most productive MCP tool,
/// populates a valid input template, and avoids repeating failed calls unless
/// scope changes.
pub fn generate_gap_driven_next_actions(
    result: &WorkflowCoverageResult,
    retrieval_history: &[RetrievalFailure],
    known_source_ids: &[String],
) -> Vec<AgentNextAction> {
    let mut actions = Vec::new();
    let mut attempted_roles: std::collections::HashSet<EvidenceRole> =
        std::collections::HashSet::new();

    // Record which roles have already failed (to avoid repeating)
    for failure in retrieval_history {
        attempted_roles.insert(failure.role);
    }

    // Prioritize missing required roles first
    for role in &result.missing_required {
        let (tool, reason_code) = role_to_next_tool(role);
        let input_template = match tool {
            "repo_search" => serde_json::json!({
                "query": "<search_query>",
                "owner": "<owner>",
                "repo": "<repo>"
            }),
            "security_search" => serde_json::json!({
                "query": "<vulnerability_or_package>"
            }),
            "research_search" => serde_json::json!({
                "query": "<research_question>"
            }),
            _ => serde_json::json!({
                "query": "<search_query>"
            }),
        };

        let mut action = AgentNextAction::new(
            tool,
            reason_code,
            1,
            input_template,
            known_source_ids.to_vec(),
            Some(*role),
        )
        .with_evidence_gap(format!("missing_required_{role:?}"))
        .with_rationale(format!(
            "Required role {:?} is missing; {}",
            role,
            if attempted_roles.contains(role) {
                "retry with different provider or query scope"
            } else {
                "search for evidence fulfilling this role"
            }
        ));

        // Avoid repeating failed calls unless scope changes
        if attempted_roles.contains(role) {
            action = action.with_evidence_gap(format!("retry_required_{role:?}_after_failure"));
        }

        actions.push(action);
    }

    // Then missing recommended roles
    for role in &result.missing_recommended {
        let (tool, reason_code) = role_to_next_tool(role);
        let input_template = match tool {
            "repo_search" => serde_json::json!({
                "query": "<search_query>",
                "owner": "<owner>",
                "repo": "<repo>"
            }),
            "security_search" => serde_json::json!({
                "query": "<vulnerability_or_package>"
            }),
            "research_search" => serde_json::json!({
                "query": "<research_question>"
            }),
            _ => serde_json::json!({
                "query": "<search_query>"
            }),
        };

        let action = AgentNextAction::new(
            tool,
            reason_code,
            3,
            input_template,
            known_source_ids.to_vec(),
            Some(*role),
        )
        .with_evidence_gap(format!("missing_recommended_{role:?}"))
        .with_rationale(format!("Recommended role {role:?} would improve coverage"));

        actions.push(action);
    }

    actions
}

/// Compute full coverage result from a model definition, found evidence, and failures.
pub fn compute_coverage(
    model: &WorkflowCoverageModel,
    found_roles: &[EvidenceRole],
    failures: &[RetrievalFailure],
) -> WorkflowCoverageResult {
    let found_set: std::collections::HashSet<EvidenceRole> = found_roles.iter().copied().collect();

    let missing_required: Vec<EvidenceRole> = model
        .required
        .iter()
        .copied()
        .filter(|r| !found_set.contains(r))
        .collect();

    let missing_recommended: Vec<EvidenceRole> = model
        .recommended
        .iter()
        .copied()
        .filter(|r| !found_set.contains(r))
        .collect();

    let missing_optional: Vec<EvidenceRole> = model
        .optional
        .iter()
        .copied()
        .filter(|r| !found_set.contains(r))
        .collect();

    let status = coverage_status(model, found_roles, failures);

    let total_required = model.required.len() as f32;
    let total_recommended = model.recommended.len() as f32;
    let total_optional = model.optional.len() as f32;

    let required_score = if total_required > 0.0 {
        (model.required.len() - missing_required.len()) as f32 / total_required
    } else {
        1.0
    };

    let recommended_score = if total_recommended > 0.0 {
        (model.recommended.len() - missing_recommended.len()) as f32 / total_recommended
    } else {
        1.0
    };

    let optional_score = if total_optional > 0.0 {
        (model.optional.len() - missing_optional.len()) as f32 / total_optional
    } else {
        1.0
    };

    let completion_confidence =
        (required_score * 0.6) + (recommended_score * 0.3) + (optional_score * 0.1);

    let mut reasons = Vec::new();
    if status == CoverageStatus::Sufficient {
        reasons.push("All required and recommended roles are covered".to_string());
    } else if status == CoverageStatus::UsableWithGaps {
        reasons.push("All required roles covered; some recommended roles missing".to_string());
    } else if status == CoverageStatus::Insufficient {
        reasons.push(format!(
            "{} required role(s) missing",
            missing_required.len()
        ));
    } else {
        reasons.push("Coverage indeterminate due to retrieval failures".to_string());
    }

    let mut next_actions = Vec::new();
    for role in &missing_required {
        let (tool, reason_code) = role_to_next_tool(role);
        next_actions.push(
            AgentNextAction::new(
                tool,
                reason_code,
                1,
                serde_json::json!({"query": "<search_query>"}),
                vec![],
                Some(*role),
            )
            .with_evidence_gap(format!("missing_required_{role:?}"))
            .with_rationale(format!("Required role {role:?} is missing from evidence")),
        );
    }
    for role in &missing_recommended {
        let (tool, reason_code) = role_to_next_tool(role);
        next_actions.push(
            AgentNextAction::new(
                tool,
                reason_code,
                3,
                serde_json::json!({"query": "<search_query>"}),
                vec![],
                Some(*role),
            )
            .with_evidence_gap(format!("missing_recommended_{role:?}"))
            .with_rationale(format!(
                "Recommended role {role:?} is missing from evidence"
            )),
        );
    }
    for failure in failures {
        if matches!(
            failure.kind,
            RetrievalFailureKind::ProviderFailed
                | RetrievalFailureKind::DeadlinePreventedCompletion
        ) {
            next_actions.push(
                AgentNextAction::new(
                    "web_search",
                    "retry_provider",
                    2,
                    serde_json::json!({"query": "<search_query>"}),
                    vec![],
                    Some(failure.role),
                )
                .with_evidence_gap(format!("provider_failure_{:?}", failure.role))
                .with_rationale(format!(
                    "Retry or try alternative provider for {:?}: {}",
                    failure.role, failure.message
                )),
            );
        }
    }

    WorkflowCoverageResult {
        workflow_id: model.workflow_id.clone(),
        required_roles: model.required.clone(),
        recommended_roles: model.recommended.clone(),
        optional_roles: model.optional.clone(),
        found_roles: found_roles.to_vec(),
        missing_required,
        missing_recommended,
        missing_optional,
        retrieval_failures: failures.to_vec(),
        status,
        completion_confidence,
        reasons,
        next_actions,
        resolution_source: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sufficient_coverage() {
        let model = api_comprehension_model();
        let found = vec![
            EvidenceRole::InterfaceOrApiDefinition,
            EvidenceRole::PrimaryImplementation,
            EvidenceRole::OfficialDocumentation,
            EvidenceRole::UsageExample,
            EvidenceRole::TestOrBehavioralSpecification,
        ];
        let result = compute_coverage(&model, &found, &[]);
        assert_eq!(result.status, CoverageStatus::Sufficient);
        assert_eq!(result.missing_required, Vec::<EvidenceRole>::new());
        assert_eq!(result.missing_recommended, Vec::<EvidenceRole>::new());
        assert!(result.completion_confidence >= 0.99);
    }

    #[test]
    fn usable_with_gaps_coverage() {
        let model = api_comprehension_model();
        let found = vec![
            EvidenceRole::InterfaceOrApiDefinition,
            EvidenceRole::PrimaryImplementation,
            EvidenceRole::OfficialDocumentation,
        ];
        let result = compute_coverage(&model, &found, &[]);
        assert_eq!(result.status, CoverageStatus::UsableWithGaps);
        assert!(result
            .missing_recommended
            .contains(&EvidenceRole::UsageExample));
        assert!(result
            .missing_recommended
            .contains(&EvidenceRole::TestOrBehavioralSpecification));
    }

    #[test]
    fn insufficient_coverage() {
        let model = api_comprehension_model();
        let found = vec![EvidenceRole::InterfaceOrApiDefinition];
        let result = compute_coverage(&model, &found, &[]);
        assert_eq!(result.status, CoverageStatus::Insufficient);
        assert!(result
            .missing_required
            .contains(&EvidenceRole::PrimaryImplementation));
    }

    #[test]
    fn indeterminate_due_to_failures() {
        let model = api_comprehension_model();
        let found = vec![
            EvidenceRole::InterfaceOrApiDefinition,
            EvidenceRole::PrimaryImplementation,
        ];
        let failures = vec![RetrievalFailure {
            kind: RetrievalFailureKind::EvidenceRoleIndeterminateBecauseRetrievalFailed,
            role: EvidenceRole::OfficialDocumentation,
            message: "Provider timed out".to_string(),
            provider_id: Some("startpage".to_string()),
        }];
        let result = compute_coverage(&model, &found, &failures);
        assert_eq!(result.status, CoverageStatus::IndeterminateDueToFailures);
    }

    #[test]
    fn all_predefined_models_have_required_roles() {
        let models = vec![
            api_comprehension_model(),
            repo_architecture_model(),
            error_investigation_model(),
            version_migration_model(),
            security_review_model(),
            dependency_evaluation_model(),
            performance_investigation_model(),
            comparative_research_model(),
            pre_change_evidence_model(),
            post_change_review_model(),
        ];
        for model in &models {
            assert!(
                !model.required.is_empty(),
                "{} has no required roles",
                model.workflow_id
            );
            assert!(
                !model.recommended.is_empty(),
                "{} has no recommended roles",
                model.workflow_id
            );
        }
    }

    #[test]
    fn serialization_roundtrip() {
        let model = api_comprehension_model();
        let json = serde_json::to_string(&model).unwrap();
        let restored: WorkflowCoverageModel = serde_json::from_str(&json).unwrap();
        assert_eq!(model.workflow_id, restored.workflow_id);
        assert_eq!(model.required, restored.required);

        let result = compute_coverage(&model, &model.required, &[]);
        let json = serde_json::to_string(&result).unwrap();
        let restored: WorkflowCoverageResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result.status, restored.status);
        assert_eq!(result.completion_confidence, restored.completion_confidence);
    }

    #[test]
    fn status_determination_direct() {
        let model = WorkflowCoverageModel {
            workflow_id: "test".to_string(),
            title: "Test".to_string(),
            required: vec![EvidenceRole::PrimaryImplementation],
            recommended: vec![EvidenceRole::OfficialDocumentation],
            optional: vec![EvidenceRole::UsageExample],
        };

        assert_eq!(
            coverage_status(
                &model,
                &[
                    EvidenceRole::PrimaryImplementation,
                    EvidenceRole::OfficialDocumentation,
                    EvidenceRole::UsageExample
                ],
                &[]
            ),
            CoverageStatus::Sufficient
        );
        assert_eq!(
            coverage_status(&model, &[EvidenceRole::PrimaryImplementation], &[]),
            CoverageStatus::UsableWithGaps
        );
        assert_eq!(
            coverage_status(&model, &[], &[]),
            CoverageStatus::Insufficient
        );
        assert_eq!(
            coverage_status(
                &model,
                &[EvidenceRole::PrimaryImplementation],
                &[RetrievalFailure {
                    kind: RetrievalFailureKind::EvidenceRoleIndeterminateBecauseRetrievalFailed,
                    role: EvidenceRole::OfficialDocumentation,
                    message: "fail".to_string(),
                    provider_id: None,
                }]
            ),
            CoverageStatus::IndeterminateDueToFailures
        );
    }

    #[test]
    fn next_actions_for_missing_roles() {
        let model = api_comprehension_model();
        let found = vec![EvidenceRole::InterfaceOrApiDefinition];
        let result = compute_coverage(&model, &found, &[]);
        assert!(!result.next_actions.is_empty());
        assert!(result
            .next_actions
            .iter()
            .any(|a| a.evidence_role == Some(EvidenceRole::PrimaryImplementation)));
    }

    #[test]
    fn empty_model_coverage() {
        let model = WorkflowCoverageModel {
            workflow_id: "empty".to_string(),
            title: "Empty".to_string(),
            required: vec![],
            recommended: vec![],
            optional: vec![],
        };
        let result = compute_coverage(&model, &[], &[]);
        assert_eq!(result.status, CoverageStatus::Sufficient);
        assert_eq!(result.completion_confidence, 1.0);
    }

    #[test]
    fn explicit_workflow_maps_to_expected_model() {
        let cases: Vec<(WorkflowKind, &str)> = vec![
            (WorkflowKind::ApiComprehension, "api_comprehension"),
            (WorkflowKind::RepositoryArchitecture, "repo_architecture"),
            (WorkflowKind::ErrorInvestigation, "error_investigation"),
            (WorkflowKind::VersionMigration, "version_migration"),
            (WorkflowKind::SecurityReview, "security_review"),
            (WorkflowKind::DependencyEvaluation, "dependency_evaluation"),
            (
                WorkflowKind::PerformanceInvestigation,
                "performance_investigation",
            ),
            (WorkflowKind::ComparativeResearch, "comparative_research"),
            (WorkflowKind::PreChangeEvidence, "pre_change_evidence"),
            (WorkflowKind::PostChangeReview, "post_change_review"),
        ];
        for (kind, expected_id) in cases {
            let model = kind.to_model();
            assert_eq!(model.workflow_id, expected_id);
        }
    }

    #[test]
    fn explicit_workflow_wins_over_domain() {
        let (model, source) =
            crate::core::evidence_postprocess::resolve_workflow_model_with_context(
                &WorkflowResolutionContext {
                    tool: "research_search",
                    workflow: Some(WorkflowKind::SecurityReview),
                    profile: None,
                    research_domain: Some("architecture_decision"),
                    exact_error: false,
                },
            );
        assert_eq!(model.unwrap().workflow_id, "security_review");
        assert_eq!(source, Some(ResolutionSource::ExplicitWorkflow));
    }

    #[test]
    fn profile_honored_by_repo_search() {
        let (model, source) =
            crate::core::evidence_postprocess::resolve_workflow_model_with_context(
                &WorkflowResolutionContext {
                    tool: "repo_search",
                    workflow: None,
                    profile: Some("security"),
                    research_domain: None,
                    exact_error: false,
                },
            );
        assert_eq!(model.unwrap().workflow_id, "security_review");
        assert_eq!(source, Some(ResolutionSource::Profile));
    }

    #[test]
    fn exact_error_is_deterministic() {
        let (model1, src1) = crate::core::evidence_postprocess::resolve_workflow_model_with_context(
            &WorkflowResolutionContext {
                tool: "repo_search",
                workflow: None,
                profile: Some("security"),
                research_domain: None,
                exact_error: true,
            },
        );
        let (model2, src2) = crate::core::evidence_postprocess::resolve_workflow_model_with_context(
            &WorkflowResolutionContext {
                tool: "repo_search",
                workflow: None,
                profile: None,
                research_domain: None,
                exact_error: true,
            },
        );
        assert_eq!(model1.unwrap().workflow_id, "error_investigation");
        assert_eq!(src1, Some(ResolutionSource::Mode));
        assert_eq!(model2.unwrap().workflow_id, "error_investigation");
        assert_eq!(src2, Some(ResolutionSource::Mode));
    }

    #[test]
    fn omitted_fields_preserve_defaults() {
        let (model, source) =
            crate::core::evidence_postprocess::resolve_workflow_model_with_context(
                &WorkflowResolutionContext {
                    tool: "repo_search",
                    workflow: None,
                    profile: None,
                    research_domain: None,
                    exact_error: false,
                },
            );
        assert_eq!(model.unwrap().workflow_id, "repo_architecture");
        assert_eq!(source, Some(ResolutionSource::Default));
    }

    #[test]
    fn coverage_changes_between_workflows_for_identical_cards() {
        let found = vec![
            EvidenceRole::PrimaryImplementation,
            EvidenceRole::InterfaceOrApiDefinition,
            EvidenceRole::OfficialDocumentation,
            EvidenceRole::UsageExample,
            EvidenceRole::TestOrBehavioralSpecification,
        ];
        let api_result = compute_coverage(&api_comprehension_model(), &found, &[]);
        let security_result = compute_coverage(&security_review_model(), &found, &[]);
        assert_ne!(api_result.status, security_result.status);
    }

    #[test]
    fn workflow_kind_parse_roundtrip() {
        assert_eq!(
            WorkflowKind::parse("api_comprehension"),
            Some(WorkflowKind::ApiComprehension)
        );
        assert_eq!(
            WorkflowKind::parse("api"),
            Some(WorkflowKind::ApiComprehension)
        );
        assert_eq!(
            WorkflowKind::parse("security"),
            Some(WorkflowKind::SecurityReview)
        );
        assert_eq!(
            WorkflowKind::parse("migration"),
            Some(WorkflowKind::VersionMigration)
        );
        assert_eq!(
            WorkflowKind::parse("comparative"),
            Some(WorkflowKind::ComparativeResearch)
        );
        assert_eq!(WorkflowKind::parse("bogus"), None);
    }

    #[test]
    fn resolution_source_serialization() {
        let result = WorkflowCoverageResult {
            workflow_id: "test".to_string(),
            required_roles: vec![],
            recommended_roles: vec![],
            optional_roles: vec![],
            found_roles: vec![],
            missing_required: vec![],
            missing_recommended: vec![],
            missing_optional: vec![],
            retrieval_failures: vec![],
            status: CoverageStatus::Sufficient,
            completion_confidence: 1.0,
            reasons: vec![],
            next_actions: vec![],
            resolution_source: Some(ResolutionSource::ExplicitWorkflow),
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("resolution_source"));
        assert!(json.contains("explicit_workflow"));
        let restored: WorkflowCoverageResult = serde_json::from_str(&json).unwrap();
        assert_eq!(
            restored.resolution_source,
            Some(ResolutionSource::ExplicitWorkflow)
        );
    }

    #[test]
    fn research_domain_selects_correct_model() {
        let cases: Vec<(Option<&str>, &str)> = vec![
            (Some("architecture_decision"), "comparative_research"),
            (Some("error_investigation"), "error_investigation"),
            (Some("version_migration"), "version_migration"),
            (Some("security_review"), "security_review"),
            (
                Some("performance_investigation"),
                "performance_investigation",
            ),
            (None, "comparative_research"),
        ];
        for (domain, expected_id) in cases {
            let (model, source) =
                crate::core::evidence_postprocess::resolve_workflow_model_with_context(
                    &WorkflowResolutionContext {
                        tool: "research_search",
                        workflow: None,
                        profile: None,
                        research_domain: domain,
                        exact_error: false,
                    },
                );
            assert_eq!(model.unwrap().workflow_id, expected_id);
            if domain.is_some() {
                assert_eq!(source, Some(ResolutionSource::Domain));
            } else {
                assert_eq!(source, Some(ResolutionSource::Default));
            }
        }
    }
}
