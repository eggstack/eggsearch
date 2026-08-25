#![no_main]

use libfuzzer_sys::fuzz_target;
use eggsearch::core::evidence_role::EvidenceRole;
use eggsearch::core::evidence_postprocess::build_retrieval_summary_from_attempts;
use eggsearch::core::retrieval_status::{
    RetrievalAttempt, RetrievalAttemptOutcome, TruncationEvidence,
};

fuzz_target!(|data: &[u8]| {
    if data.is_empty() {
        return;
    }

    let attempt_count = (data[0] as usize) % 32;
    let seed = data.get(1).copied().unwrap_or(0);

    let all_roles = [
        EvidenceRole::PrimaryImplementation,
        EvidenceRole::OfficialDocumentation,
        EvidenceRole::AuthoritativeSecurityAdvisory,
        EvidenceRole::InterfaceOrApiDefinition,
        EvidenceRole::ArchitectureOrDesignDocument,
        EvidenceRole::BenchmarkOrPerformanceEvidence,
        EvidenceRole::IssueOrIncidentDiscussion,
        EvidenceRole::ReleaseNoteOrChangelog,
        EvidenceRole::IndependentCorroboration,
        EvidenceRole::CommunityDiscussion,
        EvidenceRole::CounterpointOrConflictingEvidence,
    ];

    let outcomes = [
        RetrievalAttemptOutcome::SuccessWithResults,
        RetrievalAttemptOutcome::SuccessZeroResults,
        RetrievalAttemptOutcome::Failed,
        RetrievalAttemptOutcome::TimedOut,
        RetrievalAttemptOutcome::RateLimited,
        RetrievalAttemptOutcome::InterruptedByDeadline,
        RetrievalAttemptOutcome::TruncatedAfterPartialSuccess,
        RetrievalAttemptOutcome::SkippedByPolicy,
        RetrievalAttemptOutcome::SkippedCapabilityUnavailable,
        RetrievalAttemptOutcome::NotApplicable,
    ];

    let attempts: Vec<RetrievalAttempt> = (0..attempt_count)
        .map(|i| {
            let role_idx = (seed as usize + i) % all_roles.len();
            let outcome_idx = (seed as usize + i * 7) % outcomes.len();
            RetrievalAttempt {
                provider_id: format!("provider_{}", i % 5),
                subquery_id: Some(format!("sq_{}", i % 4)),
                operation_id: None,
                intended_roles: vec![all_roles[role_idx]],
                outcome: outcomes[outcome_idx].clone(),
                result_count: ((seed as usize + i * 13) % 20) as usize,
                error_class: if i % 3 == 0 {
                    Some("timeout".to_string())
                } else {
                    None
                },
                deadline_interrupted: i % 7 == 0,
                truncated: i % 11 == 0,
                truncation_evidence: TruncationEvidence::None,
                query_fingerprint: None,
                duration_ms: Some(((seed as u64) + i as u64 * 100) % 30000),
            }
        })
        .collect();

    let summary = build_retrieval_summary_from_attempts(&attempts);

    assert!(
        summary.dimensions.len() <= attempts.len() * all_roles.len(),
        "dimensions must not exceed attempts * roles"
    );

    for dim in &summary.dimensions {
        assert!(
            !dim.message.is_empty(),
            "dimension message must not be empty"
        );
    }
});
