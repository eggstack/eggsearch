use serde::{Deserialize, Serialize};

use crate::core::conflict::{
    detect_entity_scoped_conflicts, detect_mutable_vs_pinned, EvidenceConflict,
};
use crate::core::evidence_role::EvidenceRole;
use crate::core::retrieval_status::{
    summarize_retrieval, EvidenceAbsenceKind, ResponseRetrievalSummary, RetrievalDimensionStatus,
};
use crate::core::source_card::{SourceCard, SourceKind};
use crate::core::workflow_coverage::{
    compute_coverage, CoverageStatus, RetrievalFailure, WorkflowCoverageModel,
    WorkflowCoverageResult,
};

const MAX_CONFLICTS: usize = 20;
const MAX_EVIDENCE_ROLE_SUMMARY_ENTRIES: usize = 30;

#[derive(Clone, Debug, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[allow(missing_docs)]
pub struct EvidenceRoleSummary {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub role_counts: Vec<RoleCount>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_sources: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coverage_status: Option<CoverageStatus>,
}

#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[allow(missing_docs)]
pub struct RoleCount {
    pub role: EvidenceRole,
    pub count: usize,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[allow(missing_docs)]
pub struct EvidencePostprocessResult {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workflow_coverage: Option<WorkflowCoverageResult>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retrieval_summary: Option<ResponseRetrievalSummary>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conflict_metadata: Vec<EvidenceConflict>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence_role_summary: Option<EvidenceRoleSummary>,
}

#[allow(missing_docs)]
pub fn assign_evidence_role(card: &SourceCard) -> EvidenceRole {
    if let Some(role) = card.metadata.evidence_role {
        return role;
    }

    if let Some(ref code) = card.metadata.code_evidence {
        if let Some(source_role) = code.source_role {
            return EvidenceRole::from_source_role(source_role);
        }
    }

    match card.metadata.source_kind {
        SourceKind::SecurityAdvisory => EvidenceRole::AuthoritativeSecurityAdvisory,
        SourceKind::OfficialDocs | SourceKind::Tutorial => EvidenceRole::OfficialDocumentation,
        SourceKind::PackageRegistry => EvidenceRole::ManifestOrDependencyMetadata,
        SourceKind::SourceRepository
        | SourceKind::RepositoryRoot
        | SourceKind::SourceDirectory
        | SourceKind::SourceFile
        | SourceKind::Commit => EvidenceRole::PrimaryImplementation,
        SourceKind::IssueThread => EvidenceRole::IssueOrIncidentDiscussion,
        SourceKind::PullRequest => EvidenceRole::PullRequestOrDesignReview,
        SourceKind::ReleaseNotes | SourceKind::Tag => EvidenceRole::ReleaseNoteOrChangelog,
        SourceKind::Reference => EvidenceRole::InterfaceOrApiDefinition,
        SourceKind::News | SourceKind::Forum => EvidenceRole::CommunityDiscussion,
        SourceKind::Unknown => EvidenceRole::UnknownOrWeakContext,
    }
}

#[allow(missing_docs)]
pub fn materialize_evidence_roles(cards: &mut [SourceCard]) {
    for card in cards.iter_mut() {
        if card.metadata.evidence_role.is_none() {
            card.metadata.evidence_role = Some(assign_evidence_role(card));
        }
    }
}

#[allow(missing_docs)]
pub fn compute_evidence_role_summary(cards: &[SourceCard]) -> EvidenceRoleSummary {
    let mut counts: std::collections::HashMap<EvidenceRole, usize> =
        std::collections::HashMap::new();
    for card in cards {
        let role = assign_evidence_role(card);
        *counts.entry(role).or_insert(0) += 1;
    }

    let mut role_counts: Vec<RoleCount> = counts
        .into_iter()
        .map(|(role, count)| RoleCount { role, count })
        .collect();
    role_counts.sort_by(|a, b| {
        b.count
            .cmp(&a.count)
            .then_with(|| a.role.label().cmp(b.role.label()))
    });
    role_counts.truncate(MAX_EVIDENCE_ROLE_SUMMARY_ENTRIES);

    let total_sources = cards.len();
    let roles_present: Vec<EvidenceRole> = cards.iter().map(assign_evidence_role).collect();
    let coverage_status = if roles_present.is_empty() {
        None
    } else {
        let found_set: std::collections::HashSet<EvidenceRole> =
            roles_present.iter().copied().collect();
        let found_vec: Vec<EvidenceRole> = found_set.into_iter().collect();
        let model = WorkflowCoverageModel {
            workflow_id: "generic".to_string(),
            title: "Generic".to_string(),
            required: vec![EvidenceRole::PrimaryImplementation],
            recommended: vec![EvidenceRole::OfficialDocumentation],
            optional: vec![],
        };
        let result = compute_coverage(&model, &found_vec, &[]);
        Some(result.status)
    };

    EvidenceRoleSummary {
        role_counts,
        total_sources: Some(total_sources),
        coverage_status,
    }
}

#[allow(missing_docs)]
pub fn build_retrieval_summary_for_search(
    providers_failed: &[crate::meta::response::ProviderFailure],
    provider_ids: &[String],
    cards: &[SourceCard],
) -> ResponseRetrievalSummary {
    let mut dimensions = Vec::new();

    let providers_with_results: std::collections::HashSet<String> = cards
        .iter()
        .flat_map(|c| c.providers.iter().cloned())
        .collect();

    for pid in provider_ids {
        if providers_with_results.contains(pid.as_str()) {
            dimensions.push(RetrievalDimensionStatus {
                evidence_role: EvidenceRole::PrimaryImplementation,
                absence_kind: EvidenceAbsenceKind::NotApplicable,
                provider_id: Some(pid.clone()),
                message: "success".to_string(),
                query: None,
            });
        } else if let Some(failure) = providers_failed.iter().find(|f| f.id == *pid) {
            dimensions.push(RetrievalDimensionStatus {
                evidence_role: EvidenceRole::UnknownOrWeakContext,
                absence_kind: if failure.error_class == "timeout" {
                    EvidenceAbsenceKind::DeadlinePreventedCompletion
                } else {
                    EvidenceAbsenceKind::ProviderFailed
                },
                provider_id: Some(pid.clone()),
                message: failure.message.clone(),
                query: None,
            });
        } else {
            dimensions.push(RetrievalDimensionStatus {
                evidence_role: EvidenceRole::UnknownOrWeakContext,
                absence_kind: EvidenceAbsenceKind::ProviderSkippedByPolicy,
                provider_id: Some(pid.clone()),
                message: "provider skipped or not queried".to_string(),
                query: None,
            });
        }
    }

    if dimensions.is_empty() {
        ResponseRetrievalSummary::default()
    } else {
        summarize_retrieval(dimensions)
    }
}

#[allow(missing_docs)]
pub fn detect_structured_conflicts(cards: &[SourceCard]) -> Vec<EvidenceConflict> {
    let mut conflicts = detect_entity_scoped_conflicts(cards);

    let mut mutable_ids: Vec<String> = Vec::new();
    let mut pinned_ids: Vec<String> = Vec::new();
    for card in cards {
        let id = card.stable_id.clone().unwrap_or_default();
        if card
            .metadata
            .code_evidence
            .as_ref()
            .is_some_and(|c| c.commit_sha.is_some())
        {
            pinned_ids.push(id);
        } else {
            mutable_ids.push(id);
        }
    }
    if let Some(conflict) = detect_mutable_vs_pinned(&mutable_ids, &pinned_ids) {
        conflicts.push(conflict);
    }

    conflicts.truncate(MAX_CONFLICTS);
    conflicts
}

#[allow(missing_docs)]
pub fn postprocess(
    cards: &[SourceCard],
    providers_failed: &[crate::meta::response::ProviderFailure],
    provider_ids: &[String],
    workflow_model: Option<&WorkflowCoverageModel>,
    retrieval_failures: &[RetrievalFailure],
) -> EvidencePostprocessResult {
    let evidence_role_summary = compute_evidence_role_summary(cards);
    let retrieval_summary =
        build_retrieval_summary_for_search(providers_failed, provider_ids, cards);
    let conflict_metadata = detect_structured_conflicts(cards);

    let workflow_coverage = workflow_model.map(|model| {
        let found_roles: Vec<EvidenceRole> = cards
            .iter()
            .map(assign_evidence_role)
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        compute_coverage(model, &found_roles, retrieval_failures)
    });

    EvidencePostprocessResult {
        workflow_coverage,
        retrieval_summary: if retrieval_summary.dimensions.is_empty() {
            None
        } else {
            Some(retrieval_summary)
        },
        conflict_metadata,
        evidence_role_summary: Some(evidence_role_summary),
    }
}

#[allow(missing_docs)]
pub fn resolve_workflow_model(
    tool: &str,
    profile: Option<&str>,
    research_domain: Option<&str>,
    exact_error: bool,
) -> Option<WorkflowCoverageModel> {
    use crate::core::workflow_coverage::*;

    match tool {
        "repo_search" => {
            if exact_error {
                Some(error_investigation_model())
            } else {
                match profile {
                    Some("security") => Some(security_review_model()),
                    Some("research") => Some(comparative_research_model()),
                    _ => Some(repo_architecture_model()),
                }
            }
        }
        "research_search" => match research_domain {
            Some("architecture_decision") => Some(comparative_research_model()),
            Some("error_investigation") => Some(error_investigation_model()),
            Some("version_migration") => Some(version_migration_model()),
            Some("security_review") => Some(security_review_model()),
            _ => Some(comparative_research_model()),
        },
        "security_search" => Some(security_review_model()),
        "web_search" => None,
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_card(source_kind: SourceKind, url: &str) -> SourceCard {
        SourceCard {
            id: format!("test_{url}"),
            stable_id: Some(format!("test_{url}")),
            title: "test".to_string(),
            url: url.to_string(),
            providers: vec!["test".to_string()],
            score: Some(1.0),
            trust: crate::core::result::TrustLevel::ExternalUntrusted,
            fetched: false,
            snippet: None,
            trust_markers: crate::core::sanitize::TrustMarkers::default(),
            metadata: SourceMetadata {
                source_kind,
                ..Default::default()
            },
            quality: None,
        }
    }

    use crate::core::source_card::SourceMetadata;

    #[test]
    fn assign_evidence_role_from_source_kind() {
        let card = make_card(SourceKind::SecurityAdvisory, "https://example.com");
        assert_eq!(
            assign_evidence_role(&card),
            EvidenceRole::AuthoritativeSecurityAdvisory
        );

        let card = make_card(SourceKind::OfficialDocs, "https://example.com");
        assert_eq!(
            assign_evidence_role(&card),
            EvidenceRole::OfficialDocumentation
        );

        let card = make_card(SourceKind::IssueThread, "https://example.com");
        assert_eq!(
            assign_evidence_role(&card),
            EvidenceRole::IssueOrIncidentDiscussion
        );
    }

    #[test]
    fn assign_evidence_role_prefers_explicit() {
        let mut card = make_card(SourceKind::Unknown, "https://example.com");
        card.metadata.evidence_role = Some(EvidenceRole::BenchmarkOrPerformanceEvidence);
        assert_eq!(
            assign_evidence_role(&card),
            EvidenceRole::BenchmarkOrPerformanceEvidence
        );
    }

    #[test]
    fn evidence_role_summary_counts_roles() {
        let cards = vec![
            make_card(SourceKind::SecurityAdvisory, "https://a.com"),
            make_card(SourceKind::SecurityAdvisory, "https://b.com"),
            make_card(SourceKind::OfficialDocs, "https://c.com"),
        ];
        let summary = compute_evidence_role_summary(&cards);
        assert_eq!(summary.total_sources, Some(3));
    }

    #[test]
    fn detect_structured_conflicts_empty_for_single_card() {
        let cards = vec![make_card(SourceKind::SecurityAdvisory, "https://a.com")];
        let conflicts = detect_structured_conflicts(&cards);
        assert!(conflicts.is_empty());
    }

    #[test]
    fn retrieval_summary_from_empty_providers() {
        let summary = build_retrieval_summary_for_search(&[], &[], &[]);
        assert!(summary.dimensions.is_empty());
    }

    #[test]
    fn postprocess_provides_all_fields() {
        let cards = vec![
            make_card(SourceKind::SecurityAdvisory, "https://a.com"),
            make_card(SourceKind::OfficialDocs, "https://b.com"),
        ];
        let result = postprocess(&cards, &[], &["test".to_string()], None, &[]);
        assert!(result.evidence_role_summary.is_some());
        assert!(result.conflict_metadata.is_empty());
    }
}
