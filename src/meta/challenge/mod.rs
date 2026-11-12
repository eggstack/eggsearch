//! Bounded challenge-solving framework for keyless HTML upstreams.

pub mod altcha;
pub mod anubis;

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

/// Bound name reported when the attempt ceiling is reached.
pub const BOUND_ATTEMPTS: &str = "attempts exhausted";
/// Bound name reported when the wall-clock ceiling is reached.
pub const BOUND_WALL_CLOCK: &str = "wall clock exhausted";
/// Bound name reported when a challenge exceeds the difficulty ceiling.
pub const BOUND_DIFFICULTY: &str = "difficulty above ceiling";
/// Bound name reported for a challenge scheme this build cannot solve.
pub const BOUND_UNSUPPORTED_SCHEME: &str = "unsupported scheme";
/// Bound name reported when published challenge parameters are unusable.
pub const BOUND_MALFORMED: &str = "malformed parameters";
/// Bound name reported when verification yields no pass cookie.
pub const BOUND_MISSING_COOKIE: &str = "pass cookie absent";
/// Bound name reported when the upstream rejects a pass cookie.
pub const BOUND_COOKIE_REJECTED: &str = "pass cookie rejected";

/// Attempt, wall-clock, and difficulty ceilings for one solve.
#[derive(Clone, Copy, Debug)]
pub struct SolveBudget {
    max_attempts: u64,
    max_difficulty: u8,
    deadline: Instant,
    attempts: u64,
}

impl SolveBudget {
    /// Create a budget with the given attempt, wall-clock, and difficulty ceilings.
    #[must_use]
    pub fn new(max_attempts: u64, timeout: Duration, max_difficulty: u8) -> Self {
        Self {
            max_attempts,
            max_difficulty,
            deadline: Instant::now() + timeout,
            attempts: 0,
        }
    }

    /// Attempts consumed so far. Never exceeds [`Self::max_attempts`].
    #[must_use]
    pub fn attempts_used(&self) -> u64 {
        self.attempts
    }

    /// Configured attempt ceiling.
    #[must_use]
    pub fn max_attempts(&self) -> u64 {
        self.max_attempts
    }

    /// Configured difficulty ceiling.
    #[must_use]
    pub fn max_difficulty(&self) -> u8 {
        self.max_difficulty
    }

    /// Attempts still available, ignoring the wall clock.
    #[must_use]
    pub fn remaining(&self) -> u64 {
        self.max_attempts.saturating_sub(self.attempts)
    }

    /// Whether the published difficulty fits under the configured ceiling.
    #[must_use]
    pub fn allows_difficulty(&self, difficulty: u8) -> bool {
        difficulty <= self.max_difficulty
    }

    /// Refuse a challenge whose published difficulty exceeds the ceiling.
    pub fn check_difficulty(&self, difficulty: u8) -> Result<(), &'static str> {
        if self.allows_difficulty(difficulty) {
            Ok(())
        } else {
            Err(BOUND_DIFFICULTY)
        }
    }

    /// Consume one attempt, or return the bound that stopped the solve.
    pub fn attempt(&mut self) -> Result<(), &'static str> {
        if self.attempts >= self.max_attempts {
            return Err(BOUND_ATTEMPTS);
        }
        if Instant::now() >= self.deadline {
            return Err(BOUND_WALL_CLOCK);
        }
        self.attempts += 1;
        Ok(())
    }
}

/// Result of one bounded solve attempt.
#[derive(Clone, PartialEq, Eq)]
pub enum SolveOutcome {
    /// The challenge was answered and produced a pass cookie.
    Solved {
        /// Pass cookie value. Never write this into an error or a log.
        cookie: String,
    },
    /// The solve stopped at a named ceiling.
    Refused {
        /// One of the `BOUND_*` names.
        bound: &'static str,
    },
}

impl std::fmt::Debug for SolveOutcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Solved { .. } => f.write_str("Solved { cookie: <redacted> }"),
            Self::Refused { bound } => f.debug_struct("Refused").field("bound", bound).finish(),
        }
    }
}

struct CookieEntry {
    value: String,
    expires_at: Instant,
    sequence: u64,
}

struct CacheState {
    entries: HashMap<String, CookieEntry>,
    sequence: u64,
}

/// Process-local, per-provider pass-cookie cache.
pub struct PassCookieCache {
    capacity: usize,
    state: Mutex<CacheState>,
}

impl PassCookieCache {
    /// Default entry cap.
    pub const DEFAULT_CAPACITY: usize = 32;

    /// Create a cache holding at most `capacity` provider entries.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            state: Mutex::new(CacheState {
                entries: HashMap::new(),
                sequence: 0,
            }),
        }
    }

    /// Return the unexpired cookie for `provider`, dropping an expired entry.
    #[must_use]
    pub fn get(&self, provider: &str) -> Option<String> {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let now = Instant::now();
        if state
            .entries
            .get(provider)
            .is_some_and(|entry| entry.expires_at <= now)
        {
            state.entries.remove(provider);
        }
        state.entries.get(provider).map(|entry| entry.value.clone())
    }

    /// Store a cookie for `provider`. A zero `ttl` stores nothing.
    pub fn put(&self, provider: &str, cookie: &str, ttl: Duration) {
        if ttl.is_zero() || cookie.is_empty() {
            return;
        }
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let now = Instant::now();
        state.entries.retain(|_, entry| entry.expires_at > now);
        if !state.entries.contains_key(provider) && state.entries.len() >= self.capacity {
            let victim = state
                .entries
                .iter()
                .min_by(|(left_key, left), (right_key, right)| {
                    left.expires_at
                        .cmp(&right.expires_at)
                        .then(left.sequence.cmp(&right.sequence))
                        .then(left_key.cmp(right_key))
                })
                .map(|(key, _)| key.clone());
            if let Some(victim) = victim {
                state.entries.remove(&victim);
            }
        }
        state.sequence += 1;
        let sequence = state.sequence;
        state.entries.insert(
            provider.to_string(),
            CookieEntry {
                value: cookie.to_string(),
                expires_at: now.checked_add(ttl).unwrap_or(now),
                sequence,
            },
        );
    }

    /// Number of live entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .entries
            .len()
    }

    /// Whether the cache holds no entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Drop every entry.
    pub fn clear(&self) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        state.entries.clear();
    }
}

impl std::fmt::Debug for PassCookieCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PassCookieCache")
            .field("capacity", &self.capacity)
            .field("entries", &self.len())
            .finish()
    }
}

static PASS_COOKIE_CACHE: OnceLock<PassCookieCache> = OnceLock::new();

/// Process-wide pass-cookie cache shared by every engine instance.
#[must_use]
pub fn pass_cookie_cache() -> &'static PassCookieCache {
    PASS_COOKIE_CACHE.get_or_init(|| PassCookieCache::new(PassCookieCache::DEFAULT_CAPACITY))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attempt_counter_stops_at_max_attempts() {
        let mut budget = SolveBudget::new(3, Duration::from_secs(30), 5);
        for _ in 0..3 {
            budget.attempt().expect("attempt within ceiling");
        }
        assert_eq!(budget.attempts_used(), 3);
        assert_eq!(budget.attempt(), Err(BOUND_ATTEMPTS));
        assert_eq!(budget.attempts_used(), 3);
        assert_eq!(budget.remaining(), 0);
    }

    #[test]
    fn wall_clock_ceiling_refuses_without_sleeping() {
        let mut budget = SolveBudget::new(u64::MAX, Duration::from_millis(1), 5);
        std::thread::sleep(Duration::from_millis(2));
        assert_eq!(budget.attempt(), Err(BOUND_WALL_CLOCK));
        assert_eq!(budget.attempts_used(), 0);
    }

    #[test]
    fn difficulty_ceiling_is_named() {
        let budget = SolveBudget::new(10, Duration::from_secs(1), 4);
        assert!(budget.allows_difficulty(4));
        assert!(!budget.allows_difficulty(5));
        assert_eq!(budget.check_difficulty(5), Err(BOUND_DIFFICULTY));
    }

    #[test]
    fn cache_honours_expiry() {
        let cache = PassCookieCache::new(4);
        cache.put("mojeek", "chllg=abc", Duration::from_millis(10));
        assert_eq!(cache.get("mojeek").as_deref(), Some("chllg=abc"));
        std::thread::sleep(Duration::from_millis(20));
        assert_eq!(cache.get("mojeek"), None);
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn cache_zero_ttl_stores_nothing() {
        let cache = PassCookieCache::new(4);
        cache.put("mojeek", "chllg=abc", Duration::ZERO);
        assert_eq!(cache.get("mojeek"), None);
        assert!(cache.is_empty());
    }

    #[test]
    fn cache_evicts_at_capacity() {
        let cache = PassCookieCache::new(2);
        cache.put("first", "a", Duration::from_secs(60));
        cache.put("second", "b", Duration::from_secs(60));
        cache.put("third", "c", Duration::from_secs(60));
        assert_eq!(cache.len(), 2);
        assert_eq!(cache.get("first"), None);
        assert_eq!(cache.get("second").as_deref(), Some("b"));
        assert_eq!(cache.get("third").as_deref(), Some("c"));
    }

    #[test]
    fn cache_is_provider_isolated() {
        let cache = PassCookieCache::new(4);
        cache.put("mojeek", "mojeek-cookie", Duration::from_secs(60));
        assert_eq!(cache.get("mojeek").as_deref(), Some("mojeek-cookie"));
        assert_eq!(cache.get("startpage"), None);
        cache.clear();
        assert!(cache.is_empty());
    }

    #[test]
    fn outcome_debug_redacts_cookie_values() {
        let outcome = SolveOutcome::Solved {
            cookie: "chllg=super-secret".to_string(),
        };
        let rendered = format!("{outcome:?}");
        assert!(!rendered.contains("super-secret"), "got: {rendered}");
        assert!(rendered.contains("redacted"));
    }

    #[test]
    fn cache_debug_redacts_cookie_values() {
        let cache = PassCookieCache::new(4);
        cache.put("mojeek", "chllg=super-secret", Duration::from_secs(60));
        let rendered = format!("{cache:?}");
        assert!(!rendered.contains("super-secret"), "got: {rendered}");
    }

    #[test]
    fn process_cache_is_shared() {
        let first = pass_cookie_cache();
        let second = pass_cookie_cache();
        assert!(std::ptr::eq(first, second));
    }
}
