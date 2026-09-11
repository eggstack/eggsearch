//! Local workspace code intelligence subsystem.
//!
//! This facade documents the ownership boundaries for the local
//! workspace backend. The implementation currently lives in the
//! sibling `local_*` modules; future slices will move them under
//! this directory without changing the stable `crate::meta::local_*`
//! paths (preserved via re-exports).
//!
//! | Responsibility | Owner | Must not own |
//! |---|---|---|
//! | Bounded search orchestration | `super::local_backend` | Git discovery, inventory caching, symbol parsing |
//! | Git worktree discovery + identity | `super::local_inventory` | File scoring, symbol parsing, cache policy |
//! | File inventory cache + git runner | `super::local_inventory_cache` | Scoring, symbol parsing, identity matching |
//! | Structured symbol parsing | `super::local_symbols` | Filesystem walking, cache policy, scoring |
//! | Ignore matching | `super::local_ignore` | Anything else |
//! | Race-resistant opening | `super::safe_open` | Scoring, parsing |
//!
//! Constraints preserved across slices:
//!
//! - no workspace code execution;
//! - bounded file size/work budgets (breach degrades to partial/regex evidence);
//! - root containment/symlink safety;
//! - structured symbol backend with regex fallback;
//! - a single cache abstraction (reuse `local_inventory_cache` semantics).
//!
//! See `architecture/local-workspace.md` for the deep dive and
//! `architecture/maintenance.md` for the 007 responsibility map.
