//! Bounded parallel dispatch for multi-subquery searches.
//!
//! This module provides a queue-based bounded executor for
//! `(subquery, provider)` jobs. Jobs are sorted by priority and
//! executed with global and per-provider concurrency limits. Only
//! the active job set is in flight at any time — completed jobs
//! free capacity for the next eligible job. Output is sorted
//! deterministically before aggregation so completion order does
//! not affect results.

mod execution;
mod types;

pub(crate) use execution::dispatch_parallel;
pub use types::{partition_roles_for_engine, RoleCapabilityPartition};
pub(crate) use types::{
    CapabilityDisposition, DispatchConfig, DispatchJob, DispatchOutput, RequestDeadlineStats,
};
