#![allow(missing_docs)]

mod discovery;
mod execute;
mod index;
mod plan;
mod snapshot;

pub use discovery::{edge_id, extract_subjects, subject_id};
pub use execute::{
    run_knowledge_brief, BriefFailure, BriefQuery, BriefRun, EvidenceFetchFuture, FetchedEvidence,
    KnowledgeEvidenceFetcher, SelectedDelta,
};
pub use index::{
    database_path, valid_partition, EdgeSourceSpan, EvidenceIndex, IndexError, IndexRecord,
};
pub use plan::{day_search_window, plan_subqueries, PlannedQuery, ENGLISH_HINTS};
pub use snapshot::BUNDLED_CUTOFF_SNAPSHOT;
