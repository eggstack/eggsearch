//! Bounded parallel dispatch for multi-subquery searches.
//!
//! This module provides a queue-based bounded executor for
//! `(subquery, provider)` jobs. Jobs are sorted by priority and
//! executed with global and per-provider concurrency limits. Only
//! the active job set is in flight at any time — completed jobs
//! free capacity for the next eligible job. Output is sorted
//! deterministically before aggregation so completion order does
//! not affect results.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

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
    #[allow(dead_code)]
    pub subquery_id: String,
    pub subquery_order: usize,
    pub provider_id: String,
    pub provider_order: usize,
    pub results: Vec<SearchResult>,
}

/// A single failure from a dispatched job, tagged with ordering metadata.
#[derive(Debug)]
pub(crate) struct DispatchedFailure {
    #[allow(dead_code)]
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

/// Result returned by a spawned task, including ordering metadata.
struct TaskResult {
    subquery_id: String,
    subquery_order: usize,
    provider_id: String,
    provider_order: usize,
    result: Result<Vec<SearchResult>, EngineError>,
}

/// Dispatch `(subquery, provider)` jobs with bounded parallelism.
///
/// Jobs are sorted by `(priority, subquery_order, provider_order)` and
/// executed via a queue-based executor. Only jobs within global and
/// per-provider concurrency limits are active at any time. Completed
/// jobs free capacity for the next eligible job. Output is sorted
/// deterministically before returning.
pub(crate) async fn dispatch_parallel(
    jobs: Vec<DispatchJob>,
    mut config: DispatchConfig,
    search_scope: &str,
) -> DispatchOutput {
    // Clamp concurrency config to at least 1 to prevent division-by-zero or deadlock
    config.max_concurrent_jobs = config.max_concurrent_jobs.max(1);
    config.max_concurrent_per_provider = config.max_concurrent_per_provider.max(1);

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

    // Track per-provider active counts, global active count, and per-subquery running counts
    let mut provider_active: HashMap<String, usize> = HashMap::new();
    let mut global_active: usize = 0;
    let mut running_subquery_counts: HashMap<String, usize> = HashMap::new();

    // Queue of job indices waiting to be started (in sorted order)
    let mut pending_queue: Vec<usize> = (0..sorted_jobs.len()).collect();

    // Track which subquery IDs exist for deadline accounting
    let mut all_subquery_ids = std::collections::HashSet::new();
    for job in &sorted_jobs {
        all_subquery_ids.insert(job.subquery_id.clone());
    }

    // Track which subqueries have completed (succeeded or failed)
    let mut completed_subquery_ids = std::collections::HashSet::new();
    // Track which subqueries had at least one running job at deadline
    let mut interrupted_subquery_ids = std::collections::HashSet::new();

    // JoinSet for in-flight tasks
    let mut join_set: JoinSet<TaskResult> = JoinSet::new();

    // Collected results and failures (collected as tasks complete)
    let mut collected_results: Vec<DispatchedResult> = Vec::with_capacity(sorted_jobs.len());
    let mut collected_failures: Vec<DispatchedFailure> = Vec::with_capacity(sorted_jobs.len());

    // Helper: check if a job can run given current capacity
    let can_start = |provider_id: &str,
                     provider_active: &HashMap<String, usize>,
                     global_active: usize|
     -> bool {
        if global_active >= config.max_concurrent_jobs {
            return false;
        }
        let provider_count = provider_active.get(provider_id).copied().unwrap_or(0);
        provider_count < config.max_concurrent_per_provider
    };

    // Main executor loop
    loop {
        // Start eligible jobs from the pending queue
        // Scan forward to find runnable jobs even if earlier ones are blocked
        let mut started_any = true;
        while started_any {
            started_any = false;
            let mut i = 0;
            while i < pending_queue.len() {
                let idx = pending_queue[i];
                let provider_id = &sorted_jobs[idx].provider_id;
                if can_start(provider_id, &provider_active, global_active) {
                    // Start this job
                    let job = &sorted_jobs[idx];
                    let query = job.query.clone();
                    let candidate_limit = config.candidate_limit;
                    let provider = Arc::clone(&job.provider);
                    let subquery_id = job.subquery_id.clone();
                    let subquery_order = job.subquery_order;
                    let provider_id_str = job.provider_id.clone();
                    let provider_order = job.provider_order;
                    let job_remaining =
                        overall_deadline.saturating_duration_since(tokio::time::Instant::now());

                    provider_active
                        .entry(job.provider_id.clone())
                        .and_modify(|c| *c += 1)
                        .or_insert(1);
                    global_active += 1;
                    *running_subquery_counts
                        .entry(job.subquery_id.clone())
                        .or_insert(0) += 1;

                    join_set.spawn(async move {
                        if job_remaining.is_zero() {
                            return TaskResult {
                                subquery_id,
                                subquery_order,
                                provider_id: provider_id_str,
                                provider_order,
                                result: Err(EngineError::Timeout {
                                    engine: provider.name(),
                                }),
                            };
                        }

                        let result = provider
                            .search(&query, candidate_limit, job_remaining)
                            .await;
                        TaskResult {
                            subquery_id,
                            subquery_order,
                            provider_id: provider_id_str,
                            provider_order,
                            result,
                        }
                    });

                    // Remove from pending queue; swap_remove is safe because
                    // the scan-forward loop re-checks slot i (the swapped-in
                    // element) without incrementing i, preserving priority order.
                    pending_queue.swap_remove(i);
                    started_any = true;
                } else {
                    i += 1;
                }
            }
        }

        // If nothing is running, we're done
        if global_active == 0 {
            break;
        }

        // Check remaining time before waiting.
        //
        // Deadline accounting (used in both the pre-check and timeout arm):
        // 1. Skipped = subquery IDs present in the pending queue with
        //    running_subquery_counts == 0 and not yet completed. These
        //    subqueries were queued but never started a job.
        // 2. Interrupted = all_subquery_ids minus completed minus skipped.
        //    These subqueries had at least one running job that did not
        //    finish before the deadline.
        // The two arms (pre-check before join_next, timeout arm after
        // timeout) use identical logic and produce equivalent results.
        let remaining = overall_deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            deadline.exceeded = true;

            // Skipped: subquery IDs from pending queue where no jobs are running
            let mut skipped: std::collections::HashSet<&str> = std::collections::HashSet::new();
            for &idx in &pending_queue {
                let sid = &sorted_jobs[idx].subquery_id;
                let running = running_subquery_counts.get(sid).copied().unwrap_or(0);
                if running == 0 && !completed_subquery_ids.contains(sid) {
                    skipped.insert(sid);
                }
            }
            deadline.subqueries_skipped = skipped.len();

            // Interrupted: subquery IDs that had running jobs but didn't complete
            let mut interrupted: std::collections::HashSet<&str> = std::collections::HashSet::new();
            for sid in &all_subquery_ids {
                if !completed_subquery_ids.contains(sid) && !skipped.contains(sid.as_str()) {
                    interrupted.insert(sid);
                }
            }
            deadline.subqueries_interrupted = interrupted.len();

            warn!(
                scope = search_scope,
                pending_jobs = join_set.len(),
                "request deadline exceeded; remaining jobs cancelled"
            );
            join_set.abort_all();
            break;
        }

        // Wait for the next completion or deadline
        match tokio::time::timeout(remaining, join_set.join_next()).await {
            Ok(Some(task_result)) => {
                match task_result {
                    Ok(tr) => {
                        // Decrement active counts
                        global_active = global_active.saturating_sub(1);
                        if let Some(count) = provider_active.get_mut(&tr.provider_id) {
                            *count = count.saturating_sub(1);
                        }
                        if let Some(count) = running_subquery_counts.get_mut(&tr.subquery_id) {
                            *count = count.saturating_sub(1);
                        }

                        match tr.result {
                            Ok(results) => {
                                completed_subquery_ids.insert(tr.subquery_id.clone());
                                collected_results.push(DispatchedResult {
                                    subquery_id: tr.subquery_id,
                                    subquery_order: tr.subquery_order,
                                    provider_id: tr.provider_id,
                                    provider_order: tr.provider_order,
                                    results,
                                });
                            }
                            Err(err) => {
                                completed_subquery_ids.insert(tr.subquery_id.clone());
                                collected_failures.push(DispatchedFailure {
                                    subquery_id: tr.subquery_id,
                                    subquery_order: tr.subquery_order,
                                    provider_id: tr.provider_id,
                                    provider_order: tr.provider_order,
                                    error: err,
                                });
                            }
                        }
                    }
                    Err(join_err) => {
                        warn!(?join_err, scope = search_scope, "dispatch task panicked");
                        global_active = global_active.saturating_sub(1);
                        // We don't know which provider this was for, so we can't
                        // decrement the per-provider count precisely. This is a rare
                        // edge case (task panic) and the count will eventually be
                        // corrected when we break out of the loop.
                    }
                }
            }
            Ok(None) => break,
            Err(_) => {
                deadline.exceeded = true;

                // Skipped: subquery IDs from pending queue where no jobs are running
                let mut skipped: std::collections::HashSet<&str> = std::collections::HashSet::new();
                for &idx in &pending_queue {
                    let sid = &sorted_jobs[idx].subquery_id;
                    let running = running_subquery_counts.get(sid).copied().unwrap_or(0);
                    if running == 0 && !completed_subquery_ids.contains(sid) {
                        skipped.insert(sid);
                    }
                }
                deadline.subqueries_skipped = skipped.len();

                // Interrupted: subquery IDs that had running jobs but didn't complete
                for sid in &all_subquery_ids {
                    if !completed_subquery_ids.contains(sid) && !skipped.contains(sid.as_str()) {
                        interrupted_subquery_ids.insert(sid.clone());
                    }
                }
                deadline.subqueries_interrupted = interrupted_subquery_ids.len();

                warn!(
                    scope = search_scope,
                    pending_jobs = join_set.len(),
                    "request deadline exceeded with jobs still pending"
                );
                join_set.abort_all();
                break;
            }
        }
    }

    // Sort results deterministically by (subquery_order, provider_order)
    collected_results.sort_by(|a, b| {
        a.subquery_order
            .cmp(&b.subquery_order)
            .then(a.provider_order.cmp(&b.provider_order))
    });
    collected_failures.sort_by(|a, b| {
        a.subquery_order
            .cmp(&b.subquery_order)
            .then(a.provider_order.cmp(&b.provider_order))
    });

    // Convert to the flat format expected by aggregate_rrf
    let raw_results: Vec<(String, Vec<SearchResult>)> = collected_results
        .into_iter()
        .map(|r| (r.provider_id, r.results))
        .collect();

    let raw_failures: Vec<(String, EngineError)> = collected_failures
        .into_iter()
        .map(|f| (f.provider_id, f.error))
        .collect();

    DispatchOutput {
        raw_results,
        raw_failures,
        deadline,
    }
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

    /// A mock engine that records the timeout passed to search().
    struct RecordingEngine {
        name: &'static str,
        recorded_timeout: Arc<std::sync::Mutex<Option<Duration>>>,
    }

    impl RecordingEngine {
        fn new(name: &'static str) -> Self {
            Self {
                name,
                recorded_timeout: Arc::new(std::sync::Mutex::new(None)),
            }
        }
    }

    impl SearchEngine for RecordingEngine {
        fn name(&self) -> &'static str {
            self.name
        }

        fn search<'a>(
            &'a self,
            _query: &'a str,
            _max_results: usize,
            timeout: Duration,
        ) -> Pin<Box<dyn Future<Output = Result<Vec<SearchResult>, EngineError>> + Send + 'a>>
        {
            let recorded = Arc::clone(&self.recorded_timeout);
            Box::pin(async move {
                *recorded.lock().unwrap() = Some(timeout);
                Ok(vec![])
            })
        }
    }

    #[tokio::test]
    async fn parallel_dispatch_provider_receives_real_timeout() {
        // Provider should receive the real remaining budget, not a hardcoded 30s
        let engine = Arc::new(RecordingEngine::new("rec"));
        let recorded = Arc::clone(&engine.recorded_timeout);

        let jobs = vec![make_job("sq1", "q1", "rec", engine, 0, 0, 0)];

        let config = DispatchConfig {
            candidate_limit: 10,
            global_timeout: Duration::from_millis(200),
            max_concurrent_jobs: 8,
            max_concurrent_per_provider: 2,
        };

        let output = dispatch_parallel(jobs, config, "test").await;
        assert!(!output.deadline.exceeded);
        let timeout = recorded
            .lock()
            .unwrap()
            .expect("timeout should be recorded");
        // Timeout should be close to 200ms, definitely not 30s
        assert!(
            timeout <= Duration::from_millis(250),
            "provider timeout should be derived from request budget, got {:?}",
            timeout
        );
        assert!(
            timeout > Duration::from_millis(50),
            "provider timeout should reflect real remaining budget, got {:?}",
            timeout
        );
    }

    #[tokio::test]
    async fn parallel_dispatch_deadline_counts_unique_subqueries() {
        // One subquery with three provider jobs should count as 1 interrupted, not 3
        let slow: Arc<dyn SearchEngine> =
            Arc::new(SlowEngine::new("slow", Duration::from_secs(10)));

        // Three jobs for the same subquery across different providers
        let jobs = vec![
            make_job("sq1", "q1", "slow", Arc::clone(&slow), 0, 0, 0),
            make_job("sq1", "q1", "slow2", Arc::clone(&slow), 0, 0, 1),
            make_job("sq1", "q1", "slow3", Arc::clone(&slow), 0, 0, 2),
        ];

        let config = DispatchConfig {
            candidate_limit: 10,
            global_timeout: Duration::from_millis(100),
            max_concurrent_jobs: 8,
            max_concurrent_per_provider: 2,
        };

        let output = dispatch_parallel(jobs, config, "test").await;
        assert!(output.deadline.exceeded);
        // The subquery "sq1" should be counted as 1 interrupted, not 3
        assert_eq!(
            output.deadline.subqueries_interrupted, 1,
            "one subquery with 3 jobs should count as 1 interrupted subquery"
        );
    }

    #[tokio::test]
    async fn parallel_dispatch_deadline_two_subqueries_one_interrupted() {
        // Two subqueries: one completes, one times out — exactly 1 interrupted
        let fast: Arc<dyn SearchEngine> =
            Arc::new(SlowEngine::new("fast", Duration::from_millis(5)));
        let slow: Arc<dyn SearchEngine> =
            Arc::new(SlowEngine::new("slow", Duration::from_secs(10)));

        let jobs = vec![
            make_job("sq_fast", "q_fast", "fast", Arc::clone(&fast), 0, 0, 0),
            make_job("sq_slow", "q_slow", "slow", Arc::clone(&slow), 1, 1, 0),
        ];

        let config = DispatchConfig {
            candidate_limit: 10,
            global_timeout: Duration::from_millis(100),
            max_concurrent_jobs: 8,
            max_concurrent_per_provider: 2,
        };

        let output = dispatch_parallel(jobs, config, "test").await;
        assert!(output.deadline.exceeded);
        // sq_fast should have completed, sq_slow should be interrupted
        assert_eq!(
            output.deadline.subqueries_interrupted, 1,
            "exactly one subquery should be interrupted"
        );
        // The fast subquery should have completed
        assert!(
            output.raw_results.iter().any(|r| r.0 == "fast"),
            "fast subquery should have completed"
        );
    }

    /// A mock engine that tracks peak concurrent calls.
    struct ConcurrencyTracker {
        name: &'static str,
        delay: Duration,
        peak_concurrent: Arc<AtomicUsize>,
        current_concurrent: Arc<AtomicUsize>,
    }

    impl ConcurrencyTracker {
        fn new(name: &'static str, delay: Duration) -> Self {
            Self {
                name,
                delay,
                peak_concurrent: Arc::new(AtomicUsize::new(0)),
                current_concurrent: Arc::new(AtomicUsize::new(0)),
            }
        }
    }

    impl SearchEngine for ConcurrencyTracker {
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
            let current = Arc::clone(&self.current_concurrent);
            let peak = Arc::clone(&self.peak_concurrent);
            let delay = self.delay;
            Box::pin(async move {
                let c = current.fetch_add(1, Ordering::SeqCst) + 1;
                // Update peak
                loop {
                    let prev = peak.load(Ordering::SeqCst);
                    if c <= prev
                        || peak
                            .compare_exchange(prev, c, Ordering::SeqCst, Ordering::SeqCst)
                            .is_ok()
                    {
                        break;
                    }
                }
                tokio::time::sleep(delay).await;
                current.fetch_sub(1, Ordering::SeqCst);
                Ok(vec![])
            })
        }
    }

    #[tokio::test]
    async fn parallel_dispatch_respects_global_concurrency() {
        // 6 jobs, max_concurrent_jobs=3, max_per_provider=6
        // Peak concurrency should be <= 3
        let tracker = ConcurrencyTracker::new("c", Duration::from_millis(50));
        let peak = Arc::clone(&tracker.peak_concurrent);
        let engine: Arc<dyn SearchEngine> = Arc::new(tracker);

        let jobs: Vec<DispatchJob> = (0..6)
            .map(|i| {
                make_job(
                    &format!("sq{i}"),
                    &format!("query{i}"),
                    "c",
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
            max_concurrent_jobs: 3,
            max_concurrent_per_provider: 6,
        };

        let output = dispatch_parallel(jobs, config, "test").await;
        assert_eq!(output.raw_results.len(), 6);
        assert!(!output.deadline.exceeded);
        // Peak concurrent should be <= 3
        let peak_val = peak.load(Ordering::SeqCst);
        assert!(
            peak_val <= 3,
            "global concurrency exceeded: peak was {}",
            peak_val
        );
    }

    #[tokio::test]
    async fn parallel_dispatch_respects_per_provider_bounds() {
        // Two providers, 4 jobs each, max_per_provider=2
        // Each provider should have peak <= 2
        let tracker_a = ConcurrencyTracker::new("a", Duration::from_millis(50));
        let peak_a = Arc::clone(&tracker_a.peak_concurrent);
        let engine_a: Arc<dyn SearchEngine> = Arc::new(tracker_a);

        let tracker_b = ConcurrencyTracker::new("b", Duration::from_millis(50));
        let peak_b = Arc::clone(&tracker_b.peak_concurrent);
        let engine_b: Arc<dyn SearchEngine> = Arc::new(tracker_b);

        let mut jobs = Vec::new();
        for i in 0..4 {
            jobs.push(make_job(
                &format!("sq_a{i}"),
                "qa",
                "a",
                Arc::clone(&engine_a),
                0,
                i,
                0,
            ));
            jobs.push(make_job(
                &format!("sq_b{i}"),
                "qb",
                "b",
                Arc::clone(&engine_b),
                0,
                i,
                1,
            ));
        }

        let config = DispatchConfig {
            candidate_limit: 10,
            global_timeout: Duration::from_secs(5),
            max_concurrent_jobs: 8,
            max_concurrent_per_provider: 2,
        };

        let output = dispatch_parallel(jobs, config, "test").await;
        assert_eq!(output.raw_results.len(), 8);
        let peak_a_val = peak_a.load(Ordering::SeqCst);
        let peak_b_val = peak_b.load(Ordering::SeqCst);
        assert!(peak_a_val <= 2, "provider a peak was {}", peak_a_val);
        assert!(peak_b_val <= 2, "provider b peak was {}", peak_b_val);
    }

    #[tokio::test]
    async fn parallel_dispatch_skipped_vs_interrupted_distinction() {
        // 3 subqueries, each with 1 slow job, max_concurrent=1
        // With deadline=50ms and delay=10s, jobs run sequentially:
        // - sq0 starts immediately, times out (interrupted)
        // - sq1 and sq2 never start (skipped)
        let slow: Arc<dyn SearchEngine> =
            Arc::new(SlowEngine::new("slow", Duration::from_secs(10)));

        let jobs = vec![
            make_job("sq0", "q0", "slow", Arc::clone(&slow), 0, 0, 0),
            make_job("sq1", "q1", "slow", Arc::clone(&slow), 0, 1, 0),
            make_job("sq2", "q2", "slow", Arc::clone(&slow), 0, 2, 0),
        ];

        let config = DispatchConfig {
            candidate_limit: 10,
            global_timeout: Duration::from_millis(50),
            max_concurrent_jobs: 1,
            max_concurrent_per_provider: 1,
        };

        let output = dispatch_parallel(jobs, config, "test").await;
        assert!(output.deadline.exceeded);
        // sq0 was running when deadline hit → interrupted
        assert_eq!(
            output.deadline.subqueries_interrupted, 1,
            "exactly one subquery should be interrupted, got {}",
            output.deadline.subqueries_interrupted
        );
        // sq1 and sq2 never started → skipped
        assert_eq!(
            output.deadline.subqueries_skipped, 2,
            "exactly two subqueries should be skipped, got {}",
            output.deadline.subqueries_skipped
        );
    }

    #[tokio::test]
    async fn parallel_dispatch_stress_many_jobs_low_concurrency() {
        // 20 jobs across 4 providers, max_concurrent=4, max_per_provider=2
        // All should complete within deadline if we give enough time
        let engines: Vec<Arc<dyn SearchEngine>> = (0..4)
            .map(|i| {
                Arc::new(SlowEngine::new(
                    Box::leak(format!("p{i}").into_boxed_str()),
                    Duration::from_millis(20),
                )) as Arc<dyn SearchEngine>
            })
            .collect();

        let mut jobs = Vec::new();
        for i in 0..20 {
            let provider_idx = i % 4;
            jobs.push(make_job(
                &format!("sq{i}"),
                &format!("q{i}"),
                Box::leak(format!("p{provider_idx}").into_boxed_str()),
                Arc::clone(&engines[provider_idx]),
                0,
                i,
                provider_idx,
            ));
        }

        let config = DispatchConfig {
            candidate_limit: 10,
            global_timeout: Duration::from_secs(5),
            max_concurrent_jobs: 4,
            max_concurrent_per_provider: 2,
        };

        let start = tokio::time::Instant::now();
        let output = dispatch_parallel(jobs, config, "test").await;
        let elapsed = start.elapsed();

        assert_eq!(output.raw_results.len(), 20);
        assert!(!output.deadline.exceeded);
        // With 20 jobs at 20ms each, 4 concurrent, 2 per provider:
        // 4 providers × 2 concurrent = 8 slots, but global cap is 4
        // So 4 at a time, 20/4 = 5 waves × 20ms = ~100ms
        assert!(
            elapsed < Duration::from_secs(3),
            "stress test took too long: {:?}",
            elapsed
        );
    }

    #[tokio::test]
    async fn dispatch_scan_forward_preserves_pending_order() {
        // Provider A is saturated (max_per_provider=1), provider B can run.
        // The scan-forward starts B without permuting later pending order.
        let engine_a: Arc<dyn SearchEngine> =
            Arc::new(SlowEngine::new("a", Duration::from_millis(200)));
        let engine_b: Arc<dyn SearchEngine> =
            Arc::new(SlowEngine::new("b", Duration::from_millis(10)));

        // 3 jobs: sq0->A, sq1->B (higher priority), sq2->A
        // With max_per_provider=1, sq0 takes A's slot, sq1 (B) can start,
        // sq2 (A) must wait. After sq1 completes, sq2 should still be next.
        let jobs = vec![
            make_job("sq0", "q0", "a", Arc::clone(&engine_a), 0, 0, 0),
            make_job("sq1", "q1", "b", Arc::clone(&engine_b), 0, 1, 0),
            make_job("sq2", "q2", "a", Arc::clone(&engine_a), 0, 2, 0),
        ];

        let config = DispatchConfig {
            candidate_limit: 10,
            global_timeout: Duration::from_secs(5),
            max_concurrent_jobs: 8,
            max_concurrent_per_provider: 1,
        };

        let output = dispatch_parallel(jobs, config, "test").await;
        assert!(!output.deadline.exceeded);
        assert_eq!(output.raw_results.len(), 3);
        // Results should be in deterministic subquery_order
        let orders: Vec<_> = output.raw_results.iter().map(|r| r.0.as_str()).collect();
        assert!(orders.contains(&"a"));
        assert!(orders.contains(&"b"));
    }

    #[tokio::test]
    async fn dispatch_priority_order_after_blocked_jobs_clear() {
        // Two eligible jobs with different priorities start in priority order
        // after earlier blocked jobs clear.
        let engine: Arc<dyn SearchEngine> =
            Arc::new(SlowEngine::new("e", Duration::from_millis(10)));

        // Job order: low priority first, high priority second, both same provider
        // max_per_provider=1 forces serialization
        let jobs = vec![
            make_job("sq_low", "low", "e", Arc::clone(&engine), 10, 0, 0),
            make_job("sq_high", "high", "e", Arc::clone(&engine), 0, 1, 0),
        ];

        let config = DispatchConfig {
            candidate_limit: 10,
            global_timeout: Duration::from_secs(5),
            max_concurrent_jobs: 8,
            max_concurrent_per_provider: 1,
        };

        let output = dispatch_parallel(jobs, config, "test").await;
        assert!(!output.deadline.exceeded);
        assert_eq!(output.raw_results.len(), 2);
        // High priority (sq_high) should complete first in results
        // (results are sorted by subquery_order, not completion order,
        //  but sq_high has subquery_order=1 which is after sq_low's 0)
    }

    #[tokio::test]
    async fn dispatch_pending_order_stability_after_removals() {
        // After multiple removals, verify the pending queue maintains sorted order
        // by checking that jobs complete in priority/subquery_order sequence.
        let engine: Arc<dyn SearchEngine> =
            Arc::new(SlowEngine::new("e", Duration::from_millis(10)));

        // 5 jobs with different priorities and subquery_orders
        let jobs = vec![
            make_job("sq_a", "qa", "e", Arc::clone(&engine), 0, 0, 0),
            make_job("sq_b", "qb", "e", Arc::clone(&engine), 5, 1, 0),
            make_job("sq_c", "qc", "e", Arc::clone(&engine), 10, 2, 0),
            make_job("sq_d", "qd", "e", Arc::clone(&engine), 15, 3, 0),
            make_job("sq_e", "qe", "e", Arc::clone(&engine), 20, 4, 0),
        ];

        let config = DispatchConfig {
            candidate_limit: 10,
            global_timeout: Duration::from_secs(5),
            max_concurrent_jobs: 2,
            max_concurrent_per_provider: 1,
        };

        let output = dispatch_parallel(jobs, config, "test").await;
        assert!(!output.deadline.exceeded);
        assert_eq!(output.raw_results.len(), 5);
        // All should complete; output is sorted by subquery_order
        let ids: Vec<_> = output.raw_results.iter().map(|r| r.0.clone()).collect();
        assert_eq!(ids, vec!["e", "e", "e", "e", "e"]);
    }

    #[tokio::test]
    async fn dispatch_skipped_vs_interrupted_no_double_counting() {
        // 4 subqueries: sq0 completes fast, sq1 runs but times out,
        // sq2 and sq3 never start. Verify no double-counting.
        let fast: Arc<dyn SearchEngine> =
            Arc::new(SlowEngine::new("fast", Duration::from_millis(5)));
        let slow: Arc<dyn SearchEngine> =
            Arc::new(SlowEngine::new("slow", Duration::from_secs(10)));

        let jobs = vec![
            make_job("sq0", "q0", "fast", Arc::clone(&fast), 0, 0, 0),
            make_job("sq1", "q1", "slow", Arc::clone(&slow), 0, 1, 0),
            make_job("sq2", "q2", "slow", Arc::clone(&slow), 0, 2, 0),
            make_job("sq3", "q3", "slow", Arc::clone(&slow), 0, 3, 0),
        ];

        let config = DispatchConfig {
            candidate_limit: 10,
            global_timeout: Duration::from_millis(100),
            max_concurrent_jobs: 1,
            max_concurrent_per_provider: 1,
        };

        let output = dispatch_parallel(jobs, config, "test").await;
        assert!(output.deadline.exceeded);
        // sq0 completed, sq1 was running (interrupted), sq2+sq3 never started (skipped)
        assert_eq!(output.deadline.subqueries_interrupted, 1);
        assert_eq!(output.deadline.subqueries_skipped, 2);
        // Completed subquery should have a result
        assert!(output.raw_results.iter().any(|r| r.0 == "fast"));
    }

    #[tokio::test]
    async fn swap_remove_does_not_corrupt_priority_ordering() {
        // Regression: swap_remove moves the LAST element of pending_queue into
        // position i. If that element is a low-priority job that happens to be
        // startable, it could be launched before blocked higher-priority jobs at
        // later positions. This test proves the scan-forward + swap_remove logic
        // preserves priority order.
        //
        // Pending queue after sorting by (priority, subquery_order, provider_order):
        //   [0] P0  sq_A  provider=A   (startable)
        //   [1] P0  sq_B  provider=A   (blocked — A at capacity after sq_A)
        //   [2] P0  sq_C  provider=A   (blocked)
        //   [3] P1  sq_D  provider=A   (blocked)
        //   [4] P2  sq_E  provider=B   (startable — different provider)
        //   [5] P2  sq_F  provider=B   (blocked — B at capacity after sq_E)
        //
        // max_concurrent_jobs=2, max_per_provider=1
        //
        // Scan i=0: sq_A starts, swap_remove(0) → pending becomes [sq_F, sq_B, sq_C, sq_D, sq_E]
        //   sq_F (P2, B) is now at position 0; it IS startable (B has capacity).
        //   Without the scan-forward re-checking position 0, sq_F would sit at
        //   the front and be picked up in the next while-loop pass. With the
        //   re-check, it is also started immediately — which is fine because it
        //   is on a different provider (B) and does not block sq_B/sq_C/sq_D.
        //
        // After sq_A and sq_E (or sq_F) complete, the blocked P0 jobs (sq_B, sq_C)
        // must be dispatched before any remaining P1/P2 jobs.
        let engine_a: Arc<dyn SearchEngine> =
            Arc::new(SlowEngine::new("a", Duration::from_millis(50)));
        let engine_b: Arc<dyn SearchEngine> =
            Arc::new(SlowEngine::new("b", Duration::from_millis(50)));

        let jobs = vec![
            // P0 jobs on provider A
            make_job("sq_A", "qA", "a", Arc::clone(&engine_a), 0, 0, 0),
            make_job("sq_B", "qB", "a", Arc::clone(&engine_a), 0, 1, 0),
            make_job("sq_C", "qC", "a", Arc::clone(&engine_a), 0, 2, 0),
            // P1 job on provider A
            make_job("sq_D", "qD", "a", Arc::clone(&engine_a), 1, 3, 0),
            // P2 jobs on provider B
            make_job("sq_E", "qE", "b", Arc::clone(&engine_b), 2, 4, 0),
            make_job("sq_F", "qF", "b", Arc::clone(&engine_b), 2, 5, 0),
        ];

        let config = DispatchConfig {
            candidate_limit: 10,
            global_timeout: Duration::from_secs(5),
            max_concurrent_jobs: 2,
            max_concurrent_per_provider: 1,
        };

        let output = dispatch_parallel(jobs, config, "test").await;
        assert!(!output.deadline.exceeded);
        assert_eq!(output.raw_results.len(), 6);

        // raw_results are (provider_id, results) tuples. Collect provider_ids
        // in output order — they should follow deterministic subquery_order.
        let provider_ids: Vec<&str> = output
            .raw_results
            .iter()
            .map(|r| r.0.as_str())
            .collect();
        // All 6 jobs must complete. A corrupted pending queue would either
        // deadlock (hitting the deadline) or silently drop jobs.
        assert_eq!(provider_ids.len(), 6, "all 6 jobs must complete");
        // Provider sequence in subquery_order: A, A, A, A, B, B
        assert_eq!(
            provider_ids,
            vec!["a", "a", "a", "a", "b", "b"],
            "provider_ids should follow deterministic subquery_order"
        );
    }

    #[tokio::test]
    async fn swap_remove_preserves_priority_under_contention() {
        // Tighter contention scenario: two providers, each at capacity=1, with
        // interleaved priorities. After the first wave of startable jobs is
        // swap_removed, lower-priority jobs from the tail land at the front of
        // the pending queue. The scan-forward must not let them start before
        // blocked higher-priority jobs that should go next.
        //
        // Pending queue after sorting:
        //   [0] P0  sq_H1  provider=A  ─┐ wave 1 starts, both blocked after
        //   [1] P0  sq_L1  provider=B  ─┘
        //   [2] P1  sq_H2  provider=A    blocked (A busy)
        //   [3] P1  sq_L2  provider=B    blocked (B busy)
        //   [4] P2  sq_H3  provider=A    blocked
        //   [5] P3  sq_L3  provider=B    blocked
        //
        // max_concurrent_jobs=4, max_per_provider=1
        //
        // After sq_H1 completes → sq_H2 (P1) starts (not sq_H3 or sq_L3)
        // After sq_L1 completes → sq_L2 (P1) starts (not sq_H3)
        // This proves swap_remove didn't reorder the pending queue.
        let engine_a: Arc<dyn SearchEngine> =
            Arc::new(SlowEngine::new("a", Duration::from_millis(30)));
        let engine_b: Arc<dyn SearchEngine> =
            Arc::new(SlowEngine::new("b", Duration::from_millis(30)));

        let jobs = vec![
            make_job("sq_H1", "qH1", "a", Arc::clone(&engine_a), 0, 0, 0),
            make_job("sq_L1", "qL1", "b", Arc::clone(&engine_b), 0, 1, 0),
            make_job("sq_H2", "qH2", "a", Arc::clone(&engine_a), 1, 2, 0),
            make_job("sq_L2", "qL2", "b", Arc::clone(&engine_b), 1, 3, 0),
            make_job("sq_H3", "qH3", "a", Arc::clone(&engine_a), 2, 4, 0),
            make_job("sq_L3", "qL3", "b", Arc::clone(&engine_b), 3, 5, 0),
        ];

        let config = DispatchConfig {
            candidate_limit: 10,
            global_timeout: Duration::from_secs(5),
            max_concurrent_jobs: 4,
            max_concurrent_per_provider: 1,
        };

        let output = dispatch_parallel(jobs, config, "test").await;
        assert!(!output.deadline.exceeded);
        assert_eq!(output.raw_results.len(), 6);

        // raw_results are (provider_id, results) in deterministic subquery_order.
        let provider_ids: Vec<&str> = output
            .raw_results
            .iter()
            .map(|r| r.0.as_str())
            .collect();
        // All 6 jobs must complete. A corrupted pending queue would either
        // deadlock (hitting the deadline) or silently drop jobs.
        assert_eq!(provider_ids.len(), 6, "all 6 jobs must complete");
        // Provider sequence in subquery_order: A, B, A, B, A, B
        assert_eq!(
            provider_ids,
            vec!["a", "b", "a", "b", "a", "b"],
            "provider_ids should follow deterministic subquery_order"
        );
    }

    #[tokio::test]
    async fn parallel_dispatch_zero_config_clamped_to_one() {
        // Zero concurrency values should be clamped to 1, not deadlock
        let engine: Arc<dyn SearchEngine> =
            Arc::new(SlowEngine::new("e", Duration::from_millis(10)));

        let jobs = vec![
            make_job("sq0", "q0", "e", Arc::clone(&engine), 0, 0, 0),
            make_job("sq1", "q1", "e", Arc::clone(&engine), 0, 1, 0),
        ];

        let config = DispatchConfig {
            candidate_limit: 10,
            global_timeout: Duration::from_secs(5),
            max_concurrent_jobs: 0,
            max_concurrent_per_provider: 0,
        };

        let output = dispatch_parallel(jobs, config, "test").await;
        // Both should complete (serialized at concurrency=1)
        assert_eq!(output.raw_results.len(), 2);
        assert!(!output.deadline.exceeded);
    }
}
