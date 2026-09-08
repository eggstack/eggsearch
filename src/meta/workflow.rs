//! Shared workflow substrate for repo, research, and security discovery.
//!
//! This module expresses common mechanics without domain policy. Repo-specific
//! structured locators, security advisory synthesis, and research source-role
//! taxonomy remain in their typed domain modules. Workflows opt into only the
//! stages they need via small capability-oriented types.

use crate::core::evidence_role::EvidenceRole;
use crate::core::retrieval_status::RetrievalAttempt;
use crate::core::source_card::SourceCard;
use crate::meta::fetch_ranking::FetchCandidate;

/// A planned retrieval lane with an optional domain policy.
///
/// Lanes carry the query text, priority, intended evidence roles, and bounded
/// excerpt demand. The policy type preserves domain-specific planning context
/// without forcing every workflow to implement every stage.
#[derive(Clone, Debug)]
pub struct PlannedLane<TPolicy = ()> {
    /// Stable lane label for telemetry and warnings.
    pub label: String,
    /// Query text for this lane.
    pub query: String,
    /// Lower number means higher priority. Ties break by order.
    pub priority: i32,
    /// Intended evidence roles for retrieval accounting.
    pub intended_roles: Vec<EvidenceRole>,
    /// Provider-neutral repository scope for native filtering.
    pub repo_scope: Option<crate::meta::engines::request::RepoScope>,
    /// Bounded excerpt demand for this lane.
    pub excerpt_count: usize,
    /// Domain policy context.
    pub policy: TPolicy,
}

impl<TPolicy> PlannedLane<TPolicy> {
    /// Create a lane with default priority, roles, scope, and demand.
    pub fn new(label: String, query: String, policy: TPolicy) -> Self {
        Self {
            label,
            query,
            priority: 0,
            intended_roles: Vec::new(),
            repo_scope: None,
            excerpt_count: 0,
            policy,
        }
    }

    /// Set lane priority, returning the updated lane.
    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    /// Set intended evidence roles, returning the updated lane.
    pub fn with_roles(mut self, roles: Vec<EvidenceRole>) -> Self {
        self.intended_roles = roles;
        self
    }
}

impl PlannedLane<()> {
    /// Create a policy-free lane.
    pub fn without_policy(label: String, query: String) -> Self {
        Self::new(label, query, ())
    }
}

/// Ordered retrieval attempts recorded during lane execution.
///
/// This is a thin deterministic wrapper around the attempt ledger. It does not
/// change failure semantics. Callers pass [`RetrievalAttemptSet::as_slice`]
/// to existing failure-conversion helpers.
#[derive(Clone, Debug, Default)]
pub struct RetrievalAttemptSet(Vec<RetrievalAttempt>);

impl RetrievalAttemptSet {
    /// Create an empty set.
    pub fn new() -> Self {
        Self(Vec::new())
    }

    /// Record one attempt.
    pub fn record(&mut self, attempt: RetrievalAttempt) {
        self.0.push(attempt);
    }

    /// Append many attempts.
    pub fn extend(&mut self, attempts: impl IntoIterator<Item = RetrievalAttempt>) {
        self.0.extend(attempts);
    }

    /// Borrow as a slice for existing failure helpers.
    pub fn as_slice(&self) -> &[RetrievalAttempt] {
        &self.0
    }

    /// Number of recorded attempts.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether no attempts were recorded.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Convert into the inner vector.
    pub fn into_vec(self) -> Vec<RetrievalAttempt> {
        self.0
    }
}

impl From<Vec<RetrievalAttempt>> for RetrievalAttemptSet {
    fn from(value: Vec<RetrievalAttempt>) -> Self {
        Self(value)
    }
}

impl std::ops::Deref for RetrievalAttemptSet {
    type Target = [RetrievalAttempt];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for RetrievalAttemptSet {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// Collected source cards for executed lanes.
///
/// This wrapper preserves insertion order and does not deduplicate on its own.
/// Deduplication continues to happen in the existing RRF aggregation path so
/// stable identity rules remain byte-for-byte compatible.
#[derive(Clone, Debug, Default)]
pub struct NormalizedLaneResults(Vec<SourceCard>);

impl NormalizedLaneResults {
    /// Create empty results.
    pub fn new() -> Self {
        Self(Vec::new())
    }

    /// Push one card.
    pub fn push(&mut self, card: SourceCard) {
        self.0.push(card);
    }

    /// Append many cards.
    pub fn extend(&mut self, cards: impl IntoIterator<Item = SourceCard>) {
        self.0.extend(cards);
    }

    /// Borrow as a slice.
    pub fn as_slice(&self) -> &[SourceCard] {
        &self.0
    }

    /// Number of cards.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether no cards were collected.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Convert into the inner vector.
    pub fn into_vec(self) -> Vec<SourceCard> {
        self.0
    }
}

/// Grouped evidence keyed by a domain-specific group identifier.
///
/// Grouping policy remains in domain modules. This type only carries the
/// already-grouped cards for shared fetch-candidate and coverage helpers.
#[derive(Clone, Debug)]
pub struct EvidenceGroup<K> {
    /// Group key.
    pub key: K,
    /// Cards in this group in rank order.
    pub cards: Vec<SourceCard>,
}

impl<K> EvidenceGroup<K> {
    /// Create a group.
    pub fn new(key: K, cards: Vec<SourceCard>) -> Self {
        Self { key, cards }
    }
}

/// Coverage counts for executed lanes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CoverageSummary {
    /// Number of planned lanes.
    pub planned_lanes: usize,
    /// Number of lanes that completed.
    pub completed_lanes: usize,
    /// Number of collected cards.
    pub total_cards: usize,
}

impl CoverageSummary {
    /// Build from counts.
    pub fn new(planned_lanes: usize, completed_lanes: usize, total_cards: usize) -> Self {
        Self {
            planned_lanes,
            completed_lanes,
            total_cards,
        }
    }
}

/// Bounded fetch-candidate collection shared by suggested-fetch builders.
///
/// Ranking and diversification continue to run through the deterministic
/// `fetch_ranking` pipeline. This set only carries candidates.
#[derive(Clone, Debug, Default)]
pub struct FetchCandidateSet(Vec<FetchCandidate>);

impl FetchCandidateSet {
    /// Create an empty set.
    pub fn new() -> Self {
        Self(Vec::new())
    }

    /// Push one candidate.
    pub fn push(&mut self, candidate: FetchCandidate) {
        self.0.push(candidate);
    }

    /// Append many candidates.
    pub fn extend(&mut self, candidates: impl IntoIterator<Item = FetchCandidate>) {
        self.0.extend(candidates);
    }

    /// Number of candidates.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether no candidates were collected.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Convert into the inner vector.
    pub fn into_vec(self) -> Vec<FetchCandidate> {
        self.0
    }
}

/// Shared execution context for a single workflow invocation.
///
/// This context owns the retrieval ledger and collected cards for one
/// repo, research, or security call. It does not change dispatch, grouping,
/// or trust semantics. Callers continue to invoke existing planners,
/// dispatchers, and grouping before recording through this context.
#[derive(Debug, Default)]
pub struct WorkflowExecution {
    /// Workflow scope label for telemetry.
    pub scope: String,
    /// Recorded retrieval attempts.
    attempts: RetrievalAttemptSet,
    /// Collected cards.
    cards: NormalizedLaneResults,
}

impl WorkflowExecution {
    /// Create an execution context for a scope.
    pub fn new(scope: String) -> Self {
        Self {
            scope,
            attempts: RetrievalAttemptSet::new(),
            cards: NormalizedLaneResults::new(),
        }
    }

    /// Record one retrieval attempt.
    pub fn record_attempt(&mut self, attempt: RetrievalAttempt) {
        self.attempts.record(attempt);
    }

    /// Append many attempts.
    pub fn extend_attempts(&mut self, attempts: impl IntoIterator<Item = RetrievalAttempt>) {
        self.attempts.extend(attempts);
    }

    /// Borrow recorded attempts for failure conversion.
    pub fn attempts(&self) -> &[RetrievalAttempt] {
        self.attempts.as_slice()
    }

    /// Push collected cards.
    pub fn push_cards(&mut self, cards: impl IntoIterator<Item = SourceCard>) {
        self.cards.extend(cards);
    }

    /// Borrow collected cards.
    pub fn cards(&self) -> &[SourceCard] {
        self.cards.as_slice()
    }

    /// Summarize lane coverage.
    pub fn coverage(&self, planned_lanes: usize, completed_lanes: usize) -> CoverageSummary {
        CoverageSummary::new(planned_lanes, completed_lanes, self.cards.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attempt_set_records_and_exposes_slice() {
        let set = RetrievalAttemptSet::new();
        assert!(set.is_empty());
        assert_eq!(set.len(), 0);
        assert!(set.as_slice().is_empty());
        let _ = set.into_vec();
    }

    #[test]
    fn lane_builder_sets_priority_and_roles() {
        let lane = PlannedLane::without_policy("rq_1".to_string(), "query".to_string())
            .with_priority(3)
            .with_roles(vec![EvidenceRole::AuthoritativeSecurityAdvisory]);
        assert_eq!(lane.priority, 3);
        assert_eq!(lane.intended_roles.len(), 1);
    }

    #[test]
    fn execution_collects_cards_and_reports_coverage() {
        let mut exec = WorkflowExecution::new("repo".to_string());
        assert!(exec.attempts().is_empty());
        assert!(exec.cards().is_empty());
        let summary = exec.coverage(2, 1);
        assert_eq!(summary.planned_lanes, 2);
        assert_eq!(summary.completed_lanes, 1);
        assert_eq!(summary.total_cards, 0);
        exec.push_cards(Vec::new());
        assert!(exec.cards().is_empty());
    }

    #[test]
    fn candidate_set_push_and_len() {
        let set = FetchCandidateSet::new();
        assert!(set.is_empty());
        assert_eq!(set.len(), 0);
        let _ = set.into_vec();
    }
}
