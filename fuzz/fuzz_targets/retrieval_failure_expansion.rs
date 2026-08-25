#![no_main]

use libfuzzer_sys::fuzz_target;
use eggsearch::core::evidence_role::EvidenceRole;
use eggsearch::core::retrieval_status::{
    RetrievalAttempt, RetrievalAttemptOutcome, TruncationEvidence, attempts_to_failures,
};

fuzz_target!(|data: &[u8]| {
    if data.len() < 3 {
        return;
    }

    let role_count = (data[0] as usize) % 8 + 1;
    let attempt_count = (data[1] as usize) % 16;
    let seed = data[2];

    let all_roles = [
        EvidenceRole::PrimaryImplementation,
        EvidenceRole::OfficialDocumentation,
        EvidenceRole::AuthoritativeSecurityAdvisory,
        EvidenceRole::InterfaceOrApiDefinition,
        EvidenceRole::ArchitectureOrDesignDocument,
        EvidenceRole::BenchmarkOrPerformanceEvidence,
        EvidenceRole::IssueOrIncidentDiscussion,
        EvidenceRole::ReleaseNoteOrChangelog,
    ];

    let roles: Vec<EvidenceRole> = (0..role_count)
        .map(|i| all_roles[(seed as usize + i) % all_roles.len()].clone())
        .collect();

    let outcomes = [
        RetrievalAttemptOutcome::SuccessWithResults,
        RetrievalAttemptOutcome::SuccessZeroResults,
        RetrievalAttemptOutcome::Failed,
        RetrievalAttemptOutcome::TimedOut,
        RetrievalAttemptOutcome::RateLimited,
        RetrievalAttemptOutcome::InterruptedByDeadline,
        RetrievalAttemptOutcome::TruncatedAfterPartialSuccess,
    ];

    let attempts: Vec<RetrievalAttempt> = (0..attempt_count)
        .map(|i| {
            let role_idx = (seed as usize + i) % roles.len();
            let outcome_idx = (seed as usize + i * 3) % outcomes.len();
            RetrievalAttempt {
                provider_id: format!("provider_{}", i % 4),
                subquery_id: Some(format!("subquery_{}", i % 3)),
                operation_id: None,
                intended_roles: vec![roles[role_idx].clone()],
                outcome: outcomes[outcome_idx].clone(),
                result_count: 0,
                error_class: None,
                deadline_interrupted: false,
                truncated: false,
                truncation_evidence: TruncationEvidence::None,
                query_fingerprint: None,
                duration_ms: None,
            }
        })
        .collect();

    let failures = attempts_to_failures(&attempts);

    for failure in &failures {
        assert!(
            !failure.message.is_empty(),
            "failure message must not be empty"
        );
    }

    let mut seen = std::collections::HashSet::new();
    for failure in &failures {
        let key = (
            failure.provider_id.clone(),
            format!("{:?}", failure.role),
            format!("{:?}", failure.kind),
        );
        assert!(
            seen.insert(key),
            "duplicate failure detected in expansion"
        );
    }
});
