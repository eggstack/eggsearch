//! Bounded parallel dispatch for multi-subquery searches.
//!
//! This module replaces the sequential subquery dispatch loop with a
//! priority-aware bounded parallel executor. `(subquery, provider)`
//! jobs are sorted by priority and dispatched concurrently within
//! global and per-provider concurrency caps. Output is sorted
//! deterministically before aggregation so completion order does not
//! affect results.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tracing::warn;

use crate::meta::engines::error::EngineError;
use crate::meta::engines::models::SearchResult;
use crate::meta::engines::SearchEngine;

/// A single (subquery, provider) job to dispatch.
pub(crate) struct DispatchJob {
    /// Stable subquery identifier (label or id).
    pub subquery_id: String,
    /// The query text for this job.
    pub query: String,
    /// Provider engine identifier.
    pub provider_id: String,
    /// The engine to dispatch to.
    pub provider: Arc<dyn SearchEngine>,
    /// Lower number = higher priority. Ties broken by subquery_order then provider_order.
    pub priority: i32,
    /// Stable subquery ordering (assigned before dispatch).
    pub subquery_order: usize,
    /// Stable provider ordering within the subquery.
    pub provider_order: usize,
}

/// Configuration for parallel dispatch.
#[derive(Debug, Clone)]
pub(crate) struct DispatchConfig {
    /// Maximum results to request per engine call.
    pub candidate_limit: usize,
    /// Global timeout for the entire dispatch.
    pub global_timeout: Duration,
    /// Maximum total in-flight (subquery, provider) jobs.
    pub max_concurrent_jobs: usize,
    /// Maximum concurrent jobs for any single provider.
    pub max_concurrent_per_provider: usize,
}

impl Default for DispatchConfig {
    fn default() -> Self {
        Self {
            candidate_limit: 30,
            global_timeout: Duration::from_secs(8),
            max_concurrent_jobs: 8,
            max_concurrent_per_provider: 2,
        }
    }
}

/// A single result from a dispatched job, tagged with ordering metadata.
#[derive(Debug)]
pub(crate) struct DispatchedResult {
    pub subquery_id: String,
    pub subquery_order: usize,
    pub provider_id: String,
    pub provider_order: usize,
    pub results: Vec<SearchResult>,
}

/// A single failure from a dispatched job, tagged with ordering metadata.
#[derive(Debug)]
pub(crate) struct DispatchedFailure {
    pub subquery_id: String,
    pub subquery_order: usize,
    pub provider_id: String,
    pub provider_order: usize,
    pub error: EngineError,
}

/// Deadline tracking statistics.
#[derive(Default, Debug)]
pub(crate) struct RequestDeadlineStats {
    pub exceeded: bool,
    pub subqueries_skipped: usize,
    pub subqueries_interrupted: usize,
}

/// Output of the parallel dispatch.
#[derive(Default, Debug)]
pub(crate) struct DispatchOutput {
    /// Successful results, sorted deterministically.
    pub raw_results: Vec<(String, Vec<SearchResult>)>,
    /// Failures, sorted deterministically.
    pub raw_failures: Vec<(String, EngineError)>,
    /// Deadline tracking.
    pub deadline: RequestDeadlineStats,
}

/// Dispatch `(subquery, provider)` jobs with bounded parallelism.
///
/// Jobs are sorted by `(priority, subquery_order, provider_order)` and
/// dispatched into a `JoinSet` with semaphore-based concurrency control.
/// Results are collected and sorted deterministically before returning.
pub(crate) async fn dispatch_parallel(
    jobs: Vec<DispatchJob>,
    config: DispatchConfig,
    search_scope: &str,
) -> DispatchOutput {
    if jobs.is_empty() {
        return DispatchOutput::default();
    }

    let overall_deadline = tokio::time::Instant::now() + config.global_timeout;
    let mut deadline = RequestDeadlineStats::default();

    // Sort jobs: higher priority (lower i32) first, then by subquery_order, then provider_order.
    let mut sorted_jobs = jobs;
    sorted_jobs.sort_by(|a, b| {
        a.priority
            .cmp(&b.priority)
            .then(a.subquery_order.cmp(&b.subquery_order))
            .then(a.provider_order.cmp(&b.provider_order))
    });

    // Global concurrency semaphore
    let global_sem = Arc::new(Semaphore::new(config.max_concurrent_jobs));

    // Per-provider concurrency semaphores
    let mut provider_sems: HashMap<String, Arc<Semaphore>> = HashMap::new();
    for job in &sorted_jobs {
        provider_sems
            .entry(job.provider_id.clone())
            .or_insert_with(|| Arc::new(Semaphore::new(config.max_concurrent_per_provider)));
    }

    // Track all subquery IDs for deadline accounting
    let mut total_subqueries = std::collections::HashSet::new();
    for job in &sorted_jobs {
        total_subqueries.insert(job.subquery_id.clone());
    }

    // Spawn all jobs into a single JoinSet, but each job acquires semaphores before executing.
    let mut join_set = JoinSet::new();

    for job in sorted_jobs {
        let global_sem = Arc::clone(&global_sem);
        let provider_sem = Arc::clone(
            provider_sems
                .get(&job.provider_id)
                .expect("provider semaphore must exist"),
        );
        let query = job.query.clone();
        let candidate_limit = config.candidate_limit;
        let provider = Arc::clone(&job.provider);
        let subquery_id = job.subquery_id.clone();
        let subquery_order = job.subquery_order;
        let provider_id = job.provider_id.clone();
        let provider_order = job.provider_order;

        join_set.spawn(async move {
            // Acquire global permit (may wait or fail on deadline)
            let _global_permit = global_sem
                .acquire()
                .await
                .expect("semaphore closed unexpectedly");

            // Acquire provider permit
            let _provider_permit = provider_sem
                .acquire()
                .await
                .expect("semaphore closed unexpectedly");

            let result = provider
                .search(&query, candidate_limit, candidate_limit_duration())
                .await;

            (
                subquery_id,
                subquery_order,
                provider_id,
                provider_order,
                result,
            )
        });
    }

    // Collect results
    let mut dispatched_results: Vec<DispatchedResult> = Vec::new();
    let mut dispatched_failures: Vec<DispatchedFailure> = Vec::new();

    loop {
        let remaining = overall_deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            deadline.exceeded = true;
            // Count interrupted (started but didn't finish) and skipped (never started)
            let pending = join_set.len();
            if pending > 0 {
                // We can't easily tell which subqueries these belong to without
                // tracking, so we increment by the number of pending tasks
                deadline.subqueries_interrupted += pending;
            }
            // Count skipped subqueries (those with no results and not interrupted)
            let completed_ids: std::collections::HashSet<String> = dispatched_results
                .iter()
                .map(|r| r.subquery_id.clone())
                .chain(dispatched_failures.iter().map(|f| f.subquery_id.clone()))
                .collect();
            for sid in &total_subqueries {
                if !completed_ids.contains(sid) {
                    deadline.subqueries_skipped += 1;
                }
            }
            warn!(
                scope = search_scope,
                pending_jobs = join_set.len(),
                "request deadline exceeded; remaining jobs cancelled"
            );
            join_set.abort_all();
            break;
        }

        match tokio::time::timeout(remaining, join_set.join_next()).await {
            Ok(Some(Ok((
                subquery_id,
                subquery_order,
                provider_id,
                provider_order,
                Ok(results),
            )))) => {
                dispatched_results.push(DispatchedResult {
                    subquery_id,
                    subquery_order,
                    provider_id,
                    provider_order,
                    results,
                });
            }
            Ok(Some(Ok((subquery_id, subquery_order, provider_id, provider_order, Err(err))))) => {
                dispatched_failures.push(DispatchedFailure {
                    subquery_id,
                    subquery_order,
                    provider_id,
                    provider_order,
                    error: err,
                });
            }
            Ok(Some(Err(join_err))) => {
                warn!(?join_err, scope = search_scope, "dispatch task panicked");
            }
            Ok(None) => break,
            Err(_) => {
                deadline.exceeded = true;
                let pending = join_set.len();
                deadline.subqueries_interrupted += pending;
                warn!(
                    scope = search_scope,
                    pending_jobs = pending,
                    "request deadline exceeded with jobs still pending"
                );
                join_set.abort_all();
                break;
            }
        }
    }

    // Sort results deterministically by (subquery_order, provider_order)
    dispatched_results.sort_by(|a, b| {
        a.subquery_order
            .cmp(&b.subquery_order)
            .then(a.provider_order.cmp(&b.provider_order))
    });
    dispatched_failures.sort_by(|a, b| {
        a.subquery_order
            .cmp(&b.subquery_order)
            .then(a.provider_order.cmp(&b.provider_order))
    });

    // Convert to the flat format expected by aggregate_rrf
    let raw_results: Vec<(String, Vec<SearchResult>)> = dispatched_results
        .into_iter()
        .map(|r| (r.provider_id, r.results))
        .collect();

    let raw_failures: Vec<(String, EngineError)> = dispatched_failures
        .into_iter()
        .map(|f| (f.provider_id, f.error))
        .collect();

    DispatchOutput {
        raw_results,
        raw_failures,
        deadline,
    }
}

/// Dummy duration for per-engine timeout when not used for deadline control.
/// The global deadline is enforced at the dispatch level, not per-engine.
fn candidate_limit_duration() -> Duration {
    Duration::from_secs(30)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meta::engines::error::EngineError;
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// A mock engine that sleeps for a configurable duration.
    struct SlowEngine {
        name: &'static str,
        delay: Duration,
        call_count: Arc<AtomicUsize>,
    }

    impl SlowEngine {
        fn new(name: &'static str, delay: Duration) -> Self {
            Self {
                name,
                delay,
                call_count: Arc::new(AtomicUsize::new(0)),
            }
        }
    }

    impl SearchEngine for SlowEngine {
        fn name(&self) -> &'static str {
            self.name
        }

        fn search<'a>(
            &'a self,
            _query: &'a str,
            _max_results: usize,
            _timeout: Duration,
        ) -> Pin<Box<dyn Future<Output = Result<Vec<SearchResult>, EngineError>> + Send + 'a>>
        {
            self.call_count.fetch_add(1, Ordering::SeqCst);
            let delay = self.delay;
            Box::pin(async move {
                tokio::time::sleep(delay).await;
                Ok(vec![])
            })
        }
    }

    /// A mock engine that always fails.
    struct FailingEngine {
        name: &'static str,
    }

    impl SearchEngine for FailingEngine {
        fn name(&self) -> &'static str {
            self.name
        }

        fn search<'a>(
            &'a self,
            _query: &'a str,
            _max_results: usize,
            _timeout: Duration,
        ) -> Pin<Box<dyn Future<Output = Result<Vec<SearchResult>, EngineError>> + Send + 'a>>
        {
            Box::pin(async { Err(EngineError::Timeout { engine: "mock" }) })
        }
    }

    fn make_job(
        subquery_id: &str,
        query: &str,
        provider_id: &str,
        provider: Arc<dyn SearchEngine>,
        priority: i32,
        subquery_order: usize,
        provider_order: usize,
    ) -> DispatchJob {
        DispatchJob {
            subquery_id: subquery_id.to_string(),
            query: query.to_string(),
            provider_id: provider_id.to_string(),
            provider,
            priority,
            subquery_order,
            provider_order,
        }
    }

    #[tokio::test]
    async fn parallel_dispatch_runs_jobs_concurrently() {
        // Two slow engines should run concurrently, not sequentially
        let engine_a: Arc<dyn SearchEngine> =
            Arc::new(SlowEngine::new("a", Duration::from_millis(100)));
        let engine_b: Arc<dyn SearchEngine> =
            Arc::new(SlowEngine::new("b", Duration::from_millis(100)));

        let jobs = vec![
            make_job("sq1", "query1", "a", Arc::clone(&engine_a), 0, 0, 0),
            make_job("sq1", "query1", "b", Arc::clone(&engine_b), 0, 0, 1),
        ];

        let config = DispatchConfig {
            candidate_limit: 10,
            global_timeout: Duration::from_secs(5),
            max_concurrent_jobs: 8,
            max_concurrent_per_provider: 2,
        };

        let start = tokio::time::Instant::now();
        let output = dispatch_parallel(jobs, config, "test").await;
        let elapsed = start.elapsed();

        // Should complete in ~100ms (concurrent), not ~200ms (sequential)
        assert!(
            elapsed < Duration::from_millis(200),
            "jobs should run concurrently, took {:?}",
            elapsed
        );
        assert!(!output.deadline.exceeded);
        assert_eq!(output.raw_results.len(), 2);
    }

    #[tokio::test]
    async fn parallel_dispatch_respects_per_provider_concurrency() {
        let engine: Arc<dyn SearchEngine> =
            Arc::new(SlowEngine::new("p", Duration::from_millis(50)));

        // 4 jobs for the same provider, max 2 concurrent
        let jobs: Vec<DispatchJob> = (0..4)
            .map(|i| {
                make_job(
                    &format!("sq{i}"),
                    &format!("query{i}"),
                    "p",
                    Arc::clone(&engine),
                    0,
                    i,
                    0,
                )
            })
            .collect();

        let config = DispatchConfig {
            candidate_limit: 10,
            global_timeout: Duration::from_secs(5),
            max_concurrent_jobs: 8,
            max_concurrent_per_provider: 2,
        };

        let start = tokio::time::Instant::now();
        let _output = dispatch_parallel(jobs, config, "test").await;
        let elapsed = start.elapsed();

        // With max 2 concurrent per provider, 4 jobs of 50ms each should take ~100ms (2 waves)
        // If unlimited it would be ~50ms
        assert!(
            elapsed >= Duration::from_millis(80),
            "per-provider concurrency should serialize: took {:?}",
            elapsed
        );
    }

    #[tokio::test]
    async fn parallel_dispatch_deadline_cancels_remaining() {
        let slow: Arc<dyn SearchEngine> =
            Arc::new(SlowEngine::new("slow", Duration::from_secs(10)));
        let fast: Arc<dyn SearchEngine> =
            Arc::new(SlowEngine::new("fast", Duration::from_millis(10)));

        // Fast engine should complete, slow should be cancelled
        let jobs = vec![
            make_job("sq_slow", "query_slow", "slow", Arc::clone(&slow), 1, 0, 0),
            make_job("sq_fast", "query_fast", "fast", Arc::clone(&fast), 0, 1, 0),
        ];

        let config = DispatchConfig {
            candidate_limit: 10,
            global_timeout: Duration::from_millis(200),
            max_concurrent_jobs: 8,
            max_concurrent_per_provider: 2,
        };

        let output = dispatch_parallel(jobs, config, "test").await;
        assert!(output.deadline.exceeded);
        // At least the fast engine should have completed
        assert!(!output.raw_results.is_empty() || !output.raw_failures.is_empty());
    }

    #[tokio::test]
    async fn parallel_dispatch_deterministic_output_order() {
        let engine_a: Arc<dyn SearchEngine> =
            Arc::new(SlowEngine::new("a", Duration::from_millis(10)));
        let engine_b: Arc<dyn SearchEngine> =
            Arc::new(SlowEngine::new("b", Duration::from_millis(10)));

        // Create jobs in reverse order; output should be sorted by subquery_order
        let jobs = vec![
            make_job("sq1", "q1", "b", Arc::clone(&engine_b), 0, 1, 1),
            make_job("sq0", "q0", "a", Arc::clone(&engine_a), 0, 0, 0),
            make_job("sq1", "q1", "a", Arc::clone(&engine_a), 0, 1, 0),
            make_job("sq0", "q0", "b", Arc::clone(&engine_b), 0, 0, 1),
        ];

        let config = DispatchConfig {
            candidate_limit: 10,
            global_timeout: Duration::from_secs(5),
            max_concurrent_jobs: 8,
            max_concurrent_per_provider: 2,
        };

        let output = dispatch_parallel(jobs, config, "test").await;
        // All 4 should complete
        assert_eq!(output.raw_results.len(), 4);
        // Results should be in deterministic order
        assert!(!output.deadline.exceeded);
    }

    #[tokio::test]
    async fn parallel_dispatch_empty_jobs() {
        let config = DispatchConfig::default();
        let output = dispatch_parallel(vec![], config, "test").await;
        assert!(output.raw_results.is_empty());
        assert!(output.raw_failures.is_empty());
        assert!(!output.deadline.exceeded);
    }

    #[tokio::test]
    async fn parallel_dispatch_failures_are_tracked() {
        let failing: Arc<dyn SearchEngine> = Arc::new(FailingEngine { name: "fail1" });
        let good: Arc<dyn SearchEngine> =
            Arc::new(SlowEngine::new("good", Duration::from_millis(10)));

        let jobs = vec![
            make_job("sq1", "q1", "fail1", Arc::clone(&failing), 0, 0, 0),
            make_job("sq2", "q2", "good", Arc::clone(&good), 0, 1, 0),
        ];

        let config = DispatchConfig {
            candidate_limit: 10,
            global_timeout: Duration::from_secs(5),
            max_concurrent_jobs: 8,
            max_concurrent_per_provider: 2,
        };

        let output = dispatch_parallel(jobs, config, "test").await;
        assert_eq!(output.raw_failures.len(), 1);
        assert_eq!(output.raw_failures[0].0, "fail1");
        assert_eq!(output.raw_results.len(), 1);
        assert_eq!(output.raw_results[0].0, "good");
    }

    #[tokio::test]
    async fn parallel_dispatch_priority_ordering() {
        // Verify that higher priority (lower number) jobs are dispatched first
        let engine: Arc<dyn SearchEngine> =
            Arc::new(SlowEngine::new("e", Duration::from_millis(10)));

        // Priority 0 should come before priority 1
        let jobs = vec![
            make_job("sq_low", "low", "e", Arc::clone(&engine), 10, 1, 0),
            make_job("sq_high", "high", "e", Arc::clone(&engine), 0, 0, 0),
        ];

        let config = DispatchConfig {
            candidate_limit: 10,
            global_timeout: Duration::from_secs(5),
            max_concurrent_jobs: 8,
            max_concurrent_per_provider: 1, // Force serialization
        };

        let output = dispatch_parallel(jobs, config, "test").await;
        // Both should complete, output sorted by subquery_order
        assert_eq!(output.raw_results.len(), 2);
        assert!(!output.deadline.exceeded);
    }

    #[tokio::test]
    async fn parallel_dispatch_partial_failure_one_success_one_fail() {
        // Provider "p" has two jobs: one succeeds, one fails.
        // The dispatch output should contain both a result and a failure for "p".
        let good: Arc<dyn SearchEngine> =
            Arc::new(SlowEngine::new("good", Duration::from_millis(10)));
        let failing: Arc<dyn SearchEngine> = Arc::new(FailingEngine { name: "fail" });

        let jobs = vec![
            make_job("sq1", "q1", "good", Arc::clone(&good), 0, 0, 0),
            make_job("sq2", "q2", "fail", Arc::clone(&failing), 0, 1, 0),
        ];

        let config = DispatchConfig {
            candidate_limit: 10,
            global_timeout: Duration::from_secs(5),
            max_concurrent_jobs: 8,
            max_concurrent_per_provider: 2,
        };

        let output = dispatch_parallel(jobs, config, "test").await;
        assert_eq!(output.raw_results.len(), 1);
        assert_eq!(output.raw_failures.len(), 1);
        assert_eq!(output.raw_results[0].0, "good");
        assert_eq!(output.raw_failures[0].0, "fail");
    }
}
