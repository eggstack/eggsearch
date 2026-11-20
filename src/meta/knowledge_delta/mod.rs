#![allow(missing_docs)]

pub mod catalog;
mod discovery;
mod execute;
mod index;
mod plan;
mod snapshot;

pub use catalog::{Catalog, CatalogEntry, CatalogProblem};
pub use discovery::{edge_id, extract_subjects, subject_id};
pub use execute::{
    run_knowledge_brief, BriefFailure, BriefQuery, BriefRun, EvidenceFetchFuture, FetchedEvidence,
    KnowledgeEvidenceFetcher, SelectedDelta,
};
pub use index::{
    classify_index_freshness, database_path, valid_partition, EdgeSourceSpan, EvidenceIndex,
    IndexError, IndexFreshness, IndexRecord, IndexStatus,
};
pub use plan::{day_search_window, plan_subqueries, PlannedQuery, ENGLISH_HINTS};
pub use snapshot::BUNDLED_CUTOFF_SNAPSHOT;
