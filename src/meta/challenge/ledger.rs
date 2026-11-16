//! Bounded process-local ledger of challenge solves and pass-cookie lookups.

use std::collections::BTreeMap;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use serde::Serialize;

use super::{
    BOUND_ATTEMPTS, BOUND_BASE_PREFIX, BOUND_COOKIE_REJECTED, BOUND_DIFFICULTY, BOUND_MALFORMED,
    BOUND_MISSING_COOKIE, BOUND_SPENT, BOUND_UNSUPPORTED_SCHEME, BOUND_WALL_CLOCK,
};

/// Provider entries the ledger tracks before it stops growing.
pub const MAX_LEDGER_PROVIDERS: usize = 4;
/// Serialized-byte ceiling for one snapshot.
pub const MAX_LEDGER_BYTES: usize = 4096;

/// Fixed refusal vocabulary. Every key is always reported.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum RefusalReason {
    /// The published difficulty exceeds the configured ceiling.
    DifficultyAboveCeiling,
    /// The attempt ceiling was reached.
    AttemptsExhausted,
    /// The wall-clock ceiling was reached.
    WallClockExhausted,
    /// The challenge scheme is not solvable by this build.
    UnsupportedScheme,
    /// Published challenge parameters were unusable.
    MalformedParameters,
    /// Verification returned no pass cookie.
    PassCookieAbsent,
    /// Verification rejected the submission.
    PassCookieRejected,
    /// The challenge is no longer valid. No producer today.
    ChallengeExpired,
    /// The challenge was already spent.
    AlreadySpent,
    /// A published base prefix could not be used safely.
    UnsafeBasePrefix,
}

impl RefusalReason {
    /// Every refusal reason, in declaration order.
    pub const ALL: [Self; 10] = [
        Self::DifficultyAboveCeiling,
        Self::AttemptsExhausted,
        Self::WallClockExhausted,
        Self::UnsupportedScheme,
        Self::MalformedParameters,
        Self::PassCookieAbsent,
        Self::PassCookieRejected,
        Self::ChallengeExpired,
        Self::AlreadySpent,
        Self::UnsafeBasePrefix,
    ];

    /// Reported key for this reason.
    #[must_use]
    pub fn key(self) -> &'static str {
        match self {
            Self::DifficultyAboveCeiling => BOUND_DIFFICULTY,
            Self::AttemptsExhausted => BOUND_ATTEMPTS,
            Self::WallClockExhausted => BOUND_WALL_CLOCK,
            Self::UnsupportedScheme => BOUND_UNSUPPORTED_SCHEME,
            Self::MalformedParameters => BOUND_MALFORMED,
            Self::PassCookieAbsent => BOUND_MISSING_COOKIE,
            Self::PassCookieRejected => BOUND_COOKIE_REJECTED,
            Self::ChallengeExpired => "challenge expired",
            Self::AlreadySpent => BOUND_SPENT,
            Self::UnsafeBasePrefix => BOUND_BASE_PREFIX,
        }
    }

    /// Reason for a solver `BOUND_*` name, or `None` when the name has no key.
    #[must_use]
    pub fn from_bound(bound: &'static str) -> Option<Self> {
        Self::ALL.into_iter().find(|reason| reason.key() == bound)
    }
}

/// Fixed-key refusal counters.
#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct RefusalCounts {
    #[serde(rename = "difficulty above ceiling")]
    difficulty_above_ceiling: u64,
    #[serde(rename = "attempts exhausted")]
    attempts_exhausted: u64,
    #[serde(rename = "wall clock exhausted")]
    wall_clock_exhausted: u64,
    #[serde(rename = "unsupported scheme")]
    unsupported_scheme: u64,
    #[serde(rename = "malformed parameters")]
    malformed_parameters: u64,
    #[serde(rename = "pass cookie absent")]
    pass_cookie_absent: u64,
    #[serde(rename = "pass cookie rejected")]
    pass_cookie_rejected: u64,
    #[serde(rename = "challenge expired")]
    challenge_expired: u64,
    #[serde(rename = "already spent")]
    already_spent: u64,
    #[serde(rename = "unsafe base prefix")]
    unsafe_base_prefix: u64,
}

impl RefusalCounts {
    /// Count recorded for one reason.
    #[must_use]
    pub fn count(&self, reason: RefusalReason) -> u64 {
        match reason {
            RefusalReason::DifficultyAboveCeiling => self.difficulty_above_ceiling,
            RefusalReason::AttemptsExhausted => self.attempts_exhausted,
            RefusalReason::WallClockExhausted => self.wall_clock_exhausted,
            RefusalReason::UnsupportedScheme => self.unsupported_scheme,
            RefusalReason::MalformedParameters => self.malformed_parameters,
            RefusalReason::PassCookieAbsent => self.pass_cookie_absent,
            RefusalReason::PassCookieRejected => self.pass_cookie_rejected,
            RefusalReason::ChallengeExpired => self.challenge_expired,
            RefusalReason::AlreadySpent => self.already_spent,
            RefusalReason::UnsafeBasePrefix => self.unsafe_base_prefix,
        }
    }

    /// Total refusals across every key.
    #[must_use]
    pub fn total(&self) -> u64 {
        RefusalReason::ALL
            .into_iter()
            .fold(0, |total, reason| total.saturating_add(self.count(reason)))
    }

    fn increment(&mut self, reason: RefusalReason) {
        let counter = match reason {
            RefusalReason::DifficultyAboveCeiling => &mut self.difficulty_above_ceiling,
            RefusalReason::AttemptsExhausted => &mut self.attempts_exhausted,
            RefusalReason::WallClockExhausted => &mut self.wall_clock_exhausted,
            RefusalReason::UnsupportedScheme => &mut self.unsupported_scheme,
            RefusalReason::MalformedParameters => &mut self.malformed_parameters,
            RefusalReason::PassCookieAbsent => &mut self.pass_cookie_absent,
            RefusalReason::PassCookieRejected => &mut self.pass_cookie_rejected,
            RefusalReason::ChallengeExpired => &mut self.challenge_expired,
            RefusalReason::AlreadySpent => &mut self.already_spent,
            RefusalReason::UnsafeBasePrefix => &mut self.unsafe_base_prefix,
        };
        *counter = counter.saturating_add(1);
    }
}

/// Solve counters for one provider.
#[derive(Clone, Debug, Default, Serialize)]
pub struct ProviderSolveStats {
    /// Recorded solve attempts.
    pub solves_attempted: u64,
    /// Attempts that produced a pass cookie.
    pub solves_succeeded: u64,
    /// Attempts that ended in an error before a decision.
    pub solves_errored: u64,
    /// Fixed-key refusal counters.
    pub refusals: RefusalCounts,
    /// Pass-cookie cache hits.
    pub cookie_hits: u64,
    /// Pass-cookie cache misses.
    pub cookie_misses: u64,
    /// Attempts consumed by the most recent attempt.
    pub last_candidates: u64,
    /// Bounded-computation milliseconds of the most recent attempt.
    pub last_wall_clock_ms: u64,
}

impl ProviderSolveStats {
    fn record(&mut self, outcome: RecordedOutcome, candidates: u64, elapsed: Duration) {
        self.solves_attempted = self.solves_attempted.saturating_add(1);
        match outcome {
            RecordedOutcome::Solved => {
                self.solves_succeeded = self.solves_succeeded.saturating_add(1);
            }
            RecordedOutcome::Refused(reason) => self.refusals.increment(reason),
            RecordedOutcome::Errored => {
                self.solves_errored = self.solves_errored.saturating_add(1);
            }
        }
        self.last_candidates = candidates;
        self.last_wall_clock_ms = u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX);
    }
}

/// Terminal classification of one solve attempt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecordedOutcome {
    /// The attempt produced a pass cookie.
    Solved,
    /// The attempt stopped at a named key.
    Refused(RefusalReason),
    /// The attempt ended in an error.
    Errored,
}

/// Serialized view of the ledger.
#[derive(Clone, Debug, Default, Serialize)]
pub struct SolveLedgerSnapshot {
    /// Records dropped because every tracked provider slot was taken.
    pub untracked_providers: u64,
    /// Per-provider totals.
    pub providers: BTreeMap<String, ProviderSolveStats>,
}

#[derive(Default)]
struct LedgerState {
    providers: BTreeMap<String, ProviderSolveStats>,
    untracked: u64,
}

/// Process-local, bounded challenge-solve ledger.
pub struct SolveLedger {
    capacity: usize,
    state: Mutex<LedgerState>,
}

impl SolveLedger {
    /// Create a ledger tracking at most `capacity` provider ids.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            state: Mutex::new(LedgerState::default()),
        }
    }

    /// Record one terminal outcome. Increments `solves_attempted` and exactly
    /// one outcome counter under a single lock.
    pub fn record_outcome(
        &self,
        provider: &str,
        outcome: RecordedOutcome,
        candidates: u64,
        elapsed: Duration,
    ) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let Some(stats) = tracked(&mut state, self.capacity, provider) else {
            return;
        };
        stats.record(outcome, candidates, elapsed);
    }

    /// Record one pass-cookie cache lookup.
    pub fn record_cookie_lookup(&self, provider: &str, hit: bool) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let Some(stats) = tracked(&mut state, self.capacity, provider) else {
            return;
        };
        if hit {
            stats.cookie_hits = stats.cookie_hits.saturating_add(1);
        } else {
            stats.cookie_misses = stats.cookie_misses.saturating_add(1);
        }
    }

    /// Copy the current totals.
    #[must_use]
    pub fn snapshot(&self) -> SolveLedgerSnapshot {
        let state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        SolveLedgerSnapshot {
            untracked_providers: state.untracked,
            providers: state.providers.clone(),
        }
    }
}

fn tracked<'a>(
    state: &'a mut LedgerState,
    capacity: usize,
    provider: &str,
) -> Option<&'a mut ProviderSolveStats> {
    if !state.providers.contains_key(provider) {
        if state.providers.len() >= capacity {
            state.untracked = state.untracked.saturating_add(1);
            return None;
        }
        state
            .providers
            .insert(provider.to_string(), ProviderSolveStats::default());
    }
    state.providers.get_mut(provider)
}

impl std::fmt::Debug for SolveLedger {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SolveLedger")
            .field("capacity", &self.capacity)
            .field("providers", &self.snapshot().providers.len())
            .finish()
    }
}

static SOLVE_LEDGER: OnceLock<SolveLedger> = OnceLock::new();

/// Process-wide ledger shared by every engine instance.
#[must_use]
pub fn solve_ledger() -> &'static SolveLedger {
    SOLVE_LEDGER.get_or_init(|| SolveLedger::new(MAX_LEDGER_PROVIDERS))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meta::challenge::{
        BOUND_ATTEMPTS, BOUND_BASE_PREFIX, BOUND_COOKIE_REJECTED, BOUND_DIFFICULTY,
        BOUND_MALFORMED, BOUND_MISSING_COOKIE, BOUND_SPENT, BOUND_UNSUPPORTED_SCHEME,
        BOUND_WALL_CLOCK,
    };

    const ALL_BOUNDS: [&str; 9] = [
        BOUND_ATTEMPTS,
        BOUND_WALL_CLOCK,
        BOUND_DIFFICULTY,
        BOUND_UNSUPPORTED_SCHEME,
        BOUND_MALFORMED,
        BOUND_MISSING_COOKIE,
        BOUND_COOKIE_REJECTED,
        BOUND_BASE_PREFIX,
        BOUND_SPENT,
    ];

    const ALL_BOUND_NAMES: [&str; 9] = [
        "BOUND_ATTEMPTS",
        "BOUND_WALL_CLOCK",
        "BOUND_DIFFICULTY",
        "BOUND_UNSUPPORTED_SCHEME",
        "BOUND_MALFORMED",
        "BOUND_MISSING_COOKIE",
        "BOUND_COOKIE_REJECTED",
        "BOUND_BASE_PREFIX",
        "BOUND_SPENT",
    ];

    fn stats(ledger: &SolveLedger, provider: &str) -> ProviderSolveStats {
        ledger
            .snapshot()
            .providers
            .get(provider)
            .cloned()
            .expect("provider is tracked")
    }

    #[test]
    fn empty_snapshot_serializes_with_no_providers() {
        let ledger = SolveLedger::new(MAX_LEDGER_PROVIDERS);
        let snapshot = ledger.snapshot();
        assert!(snapshot.providers.is_empty());
        assert_eq!(snapshot.untracked_providers, 0);
        assert_eq!(
            serde_json::to_string(&snapshot).unwrap(),
            r#"{"untracked_providers":0,"providers":{}}"#
        );
    }

    #[test]
    fn every_outcome_moves_one_counter_and_keeps_the_identity() {
        let ledger = SolveLedger::new(MAX_LEDGER_PROVIDERS);
        ledger.record_outcome(
            "mojeek",
            RecordedOutcome::Solved,
            291,
            Duration::from_millis(420),
        );
        ledger.record_outcome(
            "mojeek",
            RecordedOutcome::Refused(RefusalReason::PassCookieRejected),
            0,
            Duration::ZERO,
        );
        ledger.record_outcome("mojeek", RecordedOutcome::Errored, 4, Duration::ZERO);

        let stats = stats(&ledger, "mojeek");
        assert_eq!(stats.solves_attempted, 3);
        assert_eq!(stats.solves_succeeded, 1);
        assert_eq!(stats.solves_errored, 1);
        assert_eq!(stats.refusals.total(), 1);
        assert_eq!(
            stats.solves_attempted,
            stats.solves_succeeded + stats.refusals.total() + stats.solves_errored
        );
        assert_eq!(stats.last_candidates, 4);
        assert_eq!(stats.last_wall_clock_ms, 0);
    }

    #[test]
    fn every_row_of_the_table_is_one_attempt_and_one_outcome() {
        let ledger = SolveLedger::new(MAX_LEDGER_PROVIDERS);
        let outcomes = [
            RecordedOutcome::Solved,
            RecordedOutcome::Refused(RefusalReason::DifficultyAboveCeiling),
            RecordedOutcome::Refused(RefusalReason::AttemptsExhausted),
            RecordedOutcome::Refused(RefusalReason::WallClockExhausted),
            RecordedOutcome::Errored,
        ];
        for outcome in outcomes {
            ledger.record_outcome("startpage", outcome, 1, Duration::ZERO);
        }
        let stats = stats(&ledger, "startpage");
        assert_eq!(stats.solves_attempted, 5);
        assert_eq!(stats.solves_succeeded, 1);
        assert_eq!(stats.solves_errored, 1);
        assert_eq!(stats.refusals.total(), 3);
    }

    #[test]
    fn every_refusal_reason_is_distinguishable() {
        for reason in RefusalReason::ALL {
            let ledger = SolveLedger::new(MAX_LEDGER_PROVIDERS);
            ledger.record_outcome(
                "mojeek",
                RecordedOutcome::Refused(reason),
                0,
                Duration::ZERO,
            );
            let counts = stats(&ledger, "mojeek").refusals;
            assert_eq!(counts.count(reason), 1, "{reason:?} did not increment");
            assert_eq!(counts.total(), 1, "{reason:?} moved another key");
            assert_eq!(stats(&ledger, "mojeek").solves_attempted, 1);
        }
    }

    #[test]
    fn refusal_keys_are_fixed_and_unique() {
        let keys: std::collections::BTreeSet<&str> = RefusalReason::ALL
            .into_iter()
            .map(RefusalReason::key)
            .collect();
        assert_eq!(keys.len(), RefusalReason::ALL.len());
        let ledger = SolveLedger::new(MAX_LEDGER_PROVIDERS);
        ledger.record_outcome(
            "mojeek",
            RecordedOutcome::Refused(RefusalReason::AlreadySpent),
            0,
            Duration::ZERO,
        );
        let snapshot = serde_json::to_value(ledger.snapshot()).unwrap();
        let reported: std::collections::BTreeSet<String> = snapshot["providers"]["mojeek"]
            ["refusals"]
            .as_object()
            .expect("refusals is an object")
            .keys()
            .cloned()
            .collect();
        let expected: std::collections::BTreeSet<String> =
            keys.iter().map(|key| (*key).to_string()).collect();
        assert_eq!(reported, expected);
        assert_eq!(
            snapshot["providers"]["mojeek"]["refusals"]["already spent"],
            1
        );
        assert_eq!(
            snapshot["providers"]["mojeek"]["refusals"]["challenge expired"],
            0
        );
    }

    #[test]
    fn every_bound_constant_maps_to_a_documented_key() {
        let source = include_str!("mod.rs");
        let declared: std::collections::BTreeSet<&str> = source
            .lines()
            .filter_map(|line| line.trim().strip_prefix("pub const "))
            .filter_map(|rest| rest.split_once(':'))
            .map(|(name, _)| name)
            .filter(|name| name.starts_with("BOUND_"))
            .collect();
        let known: std::collections::BTreeSet<&str> = ALL_BOUND_NAMES.into_iter().collect();
        assert_eq!(
            declared, known,
            "every BOUND_* constant needs a refusal key in ledger.rs"
        );
        for bound in ALL_BOUNDS {
            let reason = RefusalReason::from_bound(bound).expect("bound has a key");
            assert_eq!(reason.key(), bound, "key must round-trip the bound name");
        }
        assert_eq!(RefusalReason::from_bound("not a bound"), None);
        assert_eq!(
            ALL_BOUNDS.len(),
            RefusalReason::ALL.len() - 1,
            "challenge expired is the only forward-declared key"
        );
    }

    #[test]
    fn cookie_lookups_are_counted_per_provider() {
        let ledger = SolveLedger::new(MAX_LEDGER_PROVIDERS);
        ledger.record_cookie_lookup("mojeek", true);
        ledger.record_cookie_lookup("mojeek", false);
        ledger.record_cookie_lookup("mojeek", false);
        ledger.record_cookie_lookup("startpage", true);
        let mojeek = stats(&ledger, "mojeek");
        assert_eq!(mojeek.cookie_hits, 1);
        assert_eq!(mojeek.cookie_misses, 2);
        assert_eq!(mojeek.solves_attempted, 0);
        assert_eq!(stats(&ledger, "startpage").cookie_hits, 1);
    }

    #[test]
    fn counters_saturate_instead_of_wrapping() {
        let ledger = SolveLedger::new(MAX_LEDGER_PROVIDERS);
        {
            let mut state = ledger.state.lock().unwrap_or_else(|e| e.into_inner());
            let stats = state.providers.entry("mojeek".to_string()).or_default();
            stats.solves_attempted = u64::MAX;
            stats.solves_succeeded = u64::MAX;
            stats.cookie_misses = u64::MAX;
            stats.refusals.attempts_exhausted = u64::MAX;
        }
        ledger.record_outcome("mojeek", RecordedOutcome::Solved, 0, Duration::ZERO);
        ledger.record_outcome(
            "mojeek",
            RecordedOutcome::Refused(RefusalReason::AttemptsExhausted),
            0,
            Duration::ZERO,
        );
        ledger.record_cookie_lookup("mojeek", false);
        let stats = stats(&ledger, "mojeek");
        assert_eq!(stats.solves_attempted, u64::MAX);
        assert_eq!(stats.solves_succeeded, u64::MAX);
        assert_eq!(
            stats.refusals.count(RefusalReason::AttemptsExhausted),
            u64::MAX
        );
        assert_eq!(stats.cookie_misses, u64::MAX);
    }

    #[test]
    fn entry_cap_keeps_totals_and_counts_dropped_records() {
        let ledger = SolveLedger::new(MAX_LEDGER_PROVIDERS);
        for provider in ["a", "b", "c", "d"] {
            ledger.record_outcome(provider, RecordedOutcome::Solved, 1, Duration::ZERO);
        }
        ledger.record_outcome("e", RecordedOutcome::Solved, 1, Duration::ZERO);
        ledger.record_cookie_lookup("e", true);
        let snapshot = ledger.snapshot();
        assert_eq!(snapshot.providers.len(), MAX_LEDGER_PROVIDERS);
        assert!(!snapshot.providers.contains_key("e"));
        assert_eq!(snapshot.untracked_providers, 2);
        assert_eq!(snapshot.providers["a"].solves_attempted, 1);
        ledger.record_outcome("a", RecordedOutcome::Solved, 1, Duration::ZERO);
        assert_eq!(stats(&ledger, "a").solves_attempted, 2);
    }

    #[test]
    fn saturated_snapshot_stays_inside_the_byte_ceiling() {
        let ledger = SolveLedger::new(MAX_LEDGER_PROVIDERS);
        for provider in ["startpage", "mojeek", "a", "bbbbbbbbbbbbbbbbbbbb"] {
            ledger.record_outcome(
                provider,
                RecordedOutcome::Refused(RefusalReason::UnsafeBasePrefix),
                u64::MAX,
                Duration::from_millis(u64::MAX),
            );
        }
        {
            let mut state = ledger.state.lock().unwrap_or_else(|e| e.into_inner());
            for stats in state.providers.values_mut() {
                stats.solves_attempted = u64::MAX;
                stats.solves_succeeded = u64::MAX;
                stats.solves_errored = u64::MAX;
                stats.cookie_hits = u64::MAX;
                stats.cookie_misses = u64::MAX;
            }
            state.untracked = u64::MAX;
        }
        let rendered = serde_json::to_string(&ledger.snapshot()).unwrap();
        assert!(
            rendered.len() <= MAX_LEDGER_BYTES,
            "snapshot is {} bytes",
            rendered.len()
        );
    }

    #[test]
    fn snapshot_carries_no_secret_material() {
        let ledger = SolveLedger::new(MAX_LEDGER_PROVIDERS);
        ledger.record_outcome(
            "mojeek",
            RecordedOutcome::Solved,
            7,
            Duration::from_millis(3),
        );
        ledger.record_cookie_lookup("mojeek", true);
        let rendered = serde_json::to_string(&ledger.snapshot()).unwrap();
        for canary in ["secret", "nonce", "salt", "derived", "canary"] {
            assert!(!rendered.contains(canary), "got: {rendered}");
        }
        let snapshot = serde_json::to_value(ledger.snapshot()).unwrap();
        for (_, value) in snapshot["providers"]["mojeek"]
            .as_object()
            .expect("entry is an object")
        {
            if let Some(object) = value.as_object() {
                assert!(
                    object.values().all(serde_json::Value::is_u64),
                    "refusal counters must be numbers"
                );
            } else {
                assert!(value.is_u64(), "provider fields must be numbers");
            }
        }
    }

    #[test]
    fn process_ledger_is_shared() {
        let first = solve_ledger();
        let second = solve_ledger();
        assert!(std::ptr::eq(first, second));
    }
}
