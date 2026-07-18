use std::collections::HashSet;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::core::evidence_role::EvidenceRole;

#[allow(missing_docs)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceAbsenceKind {
    NoMatchingEvidenceFound,
    ProviderCapabilityUnavailable,
    ProviderSkippedByPolicy,
    ProviderFailed,
    DeadlinePreventedCompletion,
    ResultTruncatedByCap,
    EvidenceRoleNotRequested,
    EvidenceRoleRequestedButNotFound,
    EvidenceRoleIndeterminateBecauseRetrievalFailed,
    #[default]
    NotApplicable,
}

#[allow(missing_docs)]
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct RetrievalDimensionStatus {
    pub evidence_role: EvidenceRole,
    pub absence_kind: EvidenceAbsenceKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_id: Option<String>,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
}

#[allow(missing_docs)]
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct ResponseRetrievalSummary {
    pub dimensions: Vec<RetrievalDimensionStatus>,
    pub has_failures: bool,
    pub has_absences: bool,
    pub has_truncation: bool,
}

#[allow(missing_docs)]
pub fn summarize_retrieval(dimensions: Vec<RetrievalDimensionStatus>) -> ResponseRetrievalSummary {
    let mut has_failures = false;
    let mut has_absences = false;
    let mut has_truncation = false;

    for d in &dimensions {
        match d.absence_kind {
            EvidenceAbsenceKind::ProviderFailed
            | EvidenceAbsenceKind::DeadlinePreventedCompletion => {
                has_failures = true;
            }
            EvidenceAbsenceKind::EvidenceRoleRequestedButNotFound => {
                has_absences = true;
            }
            EvidenceAbsenceKind::ResultTruncatedByCap => {
                has_truncation = true;
            }
            _ => {}
        }
    }

    ResponseRetrievalSummary {
        dimensions,
        has_failures,
        has_absences,
        has_truncation,
    }
}

#[allow(missing_docs)]
pub fn is_absence_only(summary: &ResponseRetrievalSummary) -> bool {
    summary.dimensions.iter().all(|d| {
        matches!(
            d.absence_kind,
            EvidenceAbsenceKind::NoMatchingEvidenceFound
                | EvidenceAbsenceKind::EvidenceRoleNotRequested
                | EvidenceAbsenceKind::NotApplicable
        )
    })
}

#[allow(missing_docs)]
pub fn is_failure_only(summary: &ResponseRetrievalSummary) -> bool {
    summary.dimensions.iter().any(|d| {
        matches!(
            d.absence_kind,
            EvidenceAbsenceKind::ProviderFailed | EvidenceAbsenceKind::DeadlinePreventedCompletion
        )
    })
}

#[allow(missing_docs)]
pub fn has_indeterminate(summary: &ResponseRetrievalSummary) -> bool {
    summary.dimensions.iter().any(|d| {
        d.absence_kind == EvidenceAbsenceKind::EvidenceRoleIndeterminateBecauseRetrievalFailed
    })
}

#[allow(missing_docs)]
pub fn absent_roles(summary: &ResponseRetrievalSummary) -> Vec<EvidenceRole> {
    summary
        .dimensions
        .iter()
        .filter(|d| d.absence_kind == EvidenceAbsenceKind::EvidenceRoleRequestedButNotFound)
        .map(|d| d.evidence_role)
        .collect()
}

#[allow(missing_docs)]
pub fn failed_providers(summary: &ResponseRetrievalSummary) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut result = Vec::new();

    for d in &summary.dimensions {
        if matches!(
            d.absence_kind,
            EvidenceAbsenceKind::ProviderFailed | EvidenceAbsenceKind::DeadlinePreventedCompletion
        ) {
            if let Some(ref pid) = d.provider_id {
                if seen.insert(pid.clone()) {
                    result.push(pid.clone());
                }
            }
        }
    }

    result
}

#[allow(missing_docs)]
pub fn classify_absence(kind: EvidenceAbsenceKind) -> &'static str {
    match kind {
        EvidenceAbsenceKind::NoMatchingEvidenceFound => "no_matching_evidence_found",
        EvidenceAbsenceKind::ProviderCapabilityUnavailable => "provider_capability_unavailable",
        EvidenceAbsenceKind::ProviderSkippedByPolicy => "provider_skipped_by_policy",
        EvidenceAbsenceKind::ProviderFailed => "provider_failed",
        EvidenceAbsenceKind::DeadlinePreventedCompletion => "deadline_prevented_completion",
        EvidenceAbsenceKind::ResultTruncatedByCap => "result_truncated_by_cap",
        EvidenceAbsenceKind::EvidenceRoleNotRequested => "evidence_role_not_requested",
        EvidenceAbsenceKind::EvidenceRoleRequestedButNotFound => {
            "evidence_role_requested_but_not_found"
        }
        EvidenceAbsenceKind::EvidenceRoleIndeterminateBecauseRetrievalFailed => {
            "evidence_role_indeterminate_because_retrieval_failed"
        }
        EvidenceAbsenceKind::NotApplicable => "not_applicable",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_variants_serialize_deserialize() {
        let variants = [
            EvidenceAbsenceKind::NoMatchingEvidenceFound,
            EvidenceAbsenceKind::ProviderCapabilityUnavailable,
            EvidenceAbsenceKind::ProviderSkippedByPolicy,
            EvidenceAbsenceKind::ProviderFailed,
            EvidenceAbsenceKind::DeadlinePreventedCompletion,
            EvidenceAbsenceKind::ResultTruncatedByCap,
            EvidenceAbsenceKind::EvidenceRoleNotRequested,
            EvidenceAbsenceKind::EvidenceRoleRequestedButNotFound,
            EvidenceAbsenceKind::EvidenceRoleIndeterminateBecauseRetrievalFailed,
            EvidenceAbsenceKind::NotApplicable,
        ];
        for v in variants {
            let json = serde_json::to_string(&v).unwrap();
            let parsed: EvidenceAbsenceKind = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, v, "roundtrip failed for {:?}", v);
        }
    }

    #[test]
    fn default_is_not_applicable() {
        assert_eq!(
            EvidenceAbsenceKind::default(),
            EvidenceAbsenceKind::NotApplicable
        );
    }

    #[test]
    fn is_absence_only_true_when_only_absences() {
        let summary = summarize_retrieval(vec![
            RetrievalDimensionStatus {
                evidence_role: EvidenceRole::PrimaryImplementation,
                absence_kind: EvidenceAbsenceKind::NoMatchingEvidenceFound,
                provider_id: None,
                message: "none".into(),
                query: None,
            },
            RetrievalDimensionStatus {
                evidence_role: EvidenceRole::OfficialDocumentation,
                absence_kind: EvidenceAbsenceKind::EvidenceRoleNotRequested,
                provider_id: None,
                message: "not requested".into(),
                query: None,
            },
        ]);
        assert!(is_absence_only(&summary));
    }

    #[test]
    fn is_absence_only_false_when_failures_present() {
        let summary = summarize_retrieval(vec![
            RetrievalDimensionStatus {
                evidence_role: EvidenceRole::PrimaryImplementation,
                absence_kind: EvidenceAbsenceKind::NoMatchingEvidenceFound,
                provider_id: None,
                message: "none".into(),
                query: None,
            },
            RetrievalDimensionStatus {
                evidence_role: EvidenceRole::OfficialDocumentation,
                absence_kind: EvidenceAbsenceKind::ProviderFailed,
                provider_id: Some("duckduckgo".into()),
                message: "failed".into(),
                query: None,
            },
        ]);
        assert!(!is_absence_only(&summary));
    }

    #[test]
    fn is_failure_only_true_when_failures_present() {
        let summary = summarize_retrieval(vec![
            RetrievalDimensionStatus {
                evidence_role: EvidenceRole::PrimaryImplementation,
                absence_kind: EvidenceAbsenceKind::NoMatchingEvidenceFound,
                provider_id: None,
                message: "none".into(),
                query: None,
            },
            RetrievalDimensionStatus {
                evidence_role: EvidenceRole::OfficialDocumentation,
                absence_kind: EvidenceAbsenceKind::ProviderFailed,
                provider_id: Some("duckduckgo".into()),
                message: "failed".into(),
                query: None,
            },
        ]);
        assert!(is_failure_only(&summary));
    }

    #[test]
    fn is_failure_only_false_when_no_failures() {
        let summary = summarize_retrieval(vec![RetrievalDimensionStatus {
            evidence_role: EvidenceRole::PrimaryImplementation,
            absence_kind: EvidenceAbsenceKind::NoMatchingEvidenceFound,
            provider_id: None,
            message: "none".into(),
            query: None,
        }]);
        assert!(!is_failure_only(&summary));
    }

    #[test]
    fn has_indeterminate_works() {
        let summary = summarize_retrieval(vec![RetrievalDimensionStatus {
            evidence_role: EvidenceRole::PrimaryImplementation,
            absence_kind: EvidenceAbsenceKind::EvidenceRoleIndeterminateBecauseRetrievalFailed,
            provider_id: None,
            message: "indeterminate".into(),
            query: None,
        }]);
        assert!(has_indeterminate(&summary));

        let summary2 = summarize_retrieval(vec![RetrievalDimensionStatus {
            evidence_role: EvidenceRole::PrimaryImplementation,
            absence_kind: EvidenceAbsenceKind::NoMatchingEvidenceFound,
            provider_id: None,
            message: "none".into(),
            query: None,
        }]);
        assert!(!has_indeterminate(&summary2));
    }

    #[test]
    fn absent_roles_returns_only_requested_but_not_found() {
        let summary = summarize_retrieval(vec![
            RetrievalDimensionStatus {
                evidence_role: EvidenceRole::PrimaryImplementation,
                absence_kind: EvidenceAbsenceKind::EvidenceRoleRequestedButNotFound,
                provider_id: None,
                message: "not found".into(),
                query: None,
            },
            RetrievalDimensionStatus {
                evidence_role: EvidenceRole::OfficialDocumentation,
                absence_kind: EvidenceAbsenceKind::NoMatchingEvidenceFound,
                provider_id: None,
                message: "none".into(),
                query: None,
            },
            RetrievalDimensionStatus {
                evidence_role: EvidenceRole::UsageExample,
                absence_kind: EvidenceAbsenceKind::EvidenceRoleRequestedButNotFound,
                provider_id: None,
                message: "not found".into(),
                query: None,
            },
        ]);
        let roles = absent_roles(&summary);
        assert_eq!(roles.len(), 2);
        assert!(roles.contains(&EvidenceRole::PrimaryImplementation));
        assert!(roles.contains(&EvidenceRole::UsageExample));
        assert!(!roles.contains(&EvidenceRole::OfficialDocumentation));
    }

    #[test]
    fn failed_providers_returns_unique_ids() {
        let summary = summarize_retrieval(vec![
            RetrievalDimensionStatus {
                evidence_role: EvidenceRole::PrimaryImplementation,
                absence_kind: EvidenceAbsenceKind::ProviderFailed,
                provider_id: Some("duckduckgo".into()),
                message: "failed".into(),
                query: None,
            },
            RetrievalDimensionStatus {
                evidence_role: EvidenceRole::OfficialDocumentation,
                absence_kind: EvidenceAbsenceKind::ProviderFailed,
                provider_id: Some("duckduckgo".into()),
                message: "failed again".into(),
                query: None,
            },
            RetrievalDimensionStatus {
                evidence_role: EvidenceRole::UsageExample,
                absence_kind: EvidenceAbsenceKind::DeadlinePreventedCompletion,
                provider_id: Some("startpage".into()),
                message: "timeout".into(),
                query: None,
            },
            RetrievalDimensionStatus {
                evidence_role: EvidenceRole::BenchmarkOrPerformanceEvidence,
                absence_kind: EvidenceAbsenceKind::ProviderFailed,
                provider_id: None,
                message: "no provider".into(),
                query: None,
            },
        ]);
        let providers = failed_providers(&summary);
        assert_eq!(providers.len(), 2);
        assert!(providers.contains(&"duckduckgo".to_string()));
        assert!(providers.contains(&"startpage".to_string()));
    }

    #[test]
    fn classify_absence_returns_correct_labels() {
        assert_eq!(
            classify_absence(EvidenceAbsenceKind::NoMatchingEvidenceFound),
            "no_matching_evidence_found"
        );
        assert_eq!(
            classify_absence(EvidenceAbsenceKind::ProviderCapabilityUnavailable),
            "provider_capability_unavailable"
        );
        assert_eq!(
            classify_absence(EvidenceAbsenceKind::ProviderSkippedByPolicy),
            "provider_skipped_by_policy"
        );
        assert_eq!(
            classify_absence(EvidenceAbsenceKind::ProviderFailed),
            "provider_failed"
        );
        assert_eq!(
            classify_absence(EvidenceAbsenceKind::DeadlinePreventedCompletion),
            "deadline_prevented_completion"
        );
        assert_eq!(
            classify_absence(EvidenceAbsenceKind::ResultTruncatedByCap),
            "result_truncated_by_cap"
        );
        assert_eq!(
            classify_absence(EvidenceAbsenceKind::EvidenceRoleNotRequested),
            "evidence_role_not_requested"
        );
        assert_eq!(
            classify_absence(EvidenceAbsenceKind::EvidenceRoleRequestedButNotFound),
            "evidence_role_requested_but_not_found"
        );
        assert_eq!(
            classify_absence(EvidenceAbsenceKind::EvidenceRoleIndeterminateBecauseRetrievalFailed),
            "evidence_role_indeterminate_because_retrieval_failed"
        );
        assert_eq!(
            classify_absence(EvidenceAbsenceKind::NotApplicable),
            "not_applicable"
        );
    }

    #[test]
    fn serde_roundtrip_retrieval_dimension_status() {
        let status = RetrievalDimensionStatus {
            evidence_role: EvidenceRole::PrimaryImplementation,
            absence_kind: EvidenceAbsenceKind::ProviderFailed,
            provider_id: Some("duckduckgo".into()),
            message: "connection refused".into(),
            query: Some("rust async runtime".into()),
        };
        let json = serde_json::to_string(&status).unwrap();
        let parsed: RetrievalDimensionStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.evidence_role, EvidenceRole::PrimaryImplementation);
        assert_eq!(parsed.absence_kind, EvidenceAbsenceKind::ProviderFailed);
        assert_eq!(parsed.provider_id.as_deref(), Some("duckduckgo"));
        assert_eq!(parsed.message, "connection refused");
        assert_eq!(parsed.query.as_deref(), Some("rust async runtime"));
    }

    #[test]
    fn serde_roundtrip_response_retrieval_summary() {
        let summary = ResponseRetrievalSummary {
            dimensions: vec![
                RetrievalDimensionStatus {
                    evidence_role: EvidenceRole::PrimaryImplementation,
                    absence_kind: EvidenceAbsenceKind::ProviderFailed,
                    provider_id: Some("duckduckgo".into()),
                    message: "failed".into(),
                    query: None,
                },
                RetrievalDimensionStatus {
                    evidence_role: EvidenceRole::OfficialDocumentation,
                    absence_kind: EvidenceAbsenceKind::EvidenceRoleRequestedButNotFound,
                    provider_id: None,
                    message: "not found".into(),
                    query: None,
                },
            ],
            has_failures: true,
            has_absences: true,
            has_truncation: false,
        };
        let json = serde_json::to_string(&summary).unwrap();
        let parsed: ResponseRetrievalSummary = serde_json::from_str(&json).unwrap();
        assert!(parsed.has_failures);
        assert!(parsed.has_absences);
        assert!(!parsed.has_truncation);
        assert_eq!(parsed.dimensions.len(), 2);
    }

    #[test]
    fn serde_deserializes_snake_case_enum() {
        let kind: EvidenceAbsenceKind =
            serde_json::from_str("\"evidence_role_requested_but_not_found\"").unwrap();
        assert_eq!(kind, EvidenceAbsenceKind::EvidenceRoleRequestedButNotFound);

        let kind: EvidenceAbsenceKind =
            serde_json::from_str("\"deadline_prevented_completion\"").unwrap();
        assert_eq!(kind, EvidenceAbsenceKind::DeadlinePreventedCompletion);
    }

    #[test]
    fn summary_default_has_empty_dimensions() {
        let summary = ResponseRetrievalSummary::default();
        assert!(summary.dimensions.is_empty());
        assert!(!summary.has_failures);
        assert!(!summary.has_absences);
        assert!(!summary.has_truncation);
    }

    #[test]
    fn summarize_retrieval_populates_flags_correctly() {
        let summary = summarize_retrieval(vec![
            RetrievalDimensionStatus {
                evidence_role: EvidenceRole::PrimaryImplementation,
                absence_kind: EvidenceAbsenceKind::ResultTruncatedByCap,
                provider_id: None,
                message: "truncated".into(),
                query: None,
            },
            RetrievalDimensionStatus {
                evidence_role: EvidenceRole::OfficialDocumentation,
                absence_kind: EvidenceAbsenceKind::DeadlinePreventedCompletion,
                provider_id: Some("startpage".into()),
                message: "timeout".into(),
                query: None,
            },
            RetrievalDimensionStatus {
                evidence_role: EvidenceRole::UsageExample,
                absence_kind: EvidenceAbsenceKind::EvidenceRoleRequestedButNotFound,
                provider_id: None,
                message: "missing".into(),
                query: None,
            },
        ]);
        assert!(summary.has_failures);
        assert!(summary.has_absences);
        assert!(summary.has_truncation);
    }
}
