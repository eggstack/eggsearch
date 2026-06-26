//! Core types, source card model, configuration, and error types
//! for eggsearch. This module is intentionally independent of any MCP,
//! HTTP, or search-engine implementation.

pub mod code_host_fetch;
pub mod code_metadata;
pub mod config;
pub mod document;
pub mod error;
pub mod fetch;
pub mod provider;
pub mod query;
/// Repo-oriented query hint parser for structured search.
pub mod repo_query;
pub mod result;
pub mod sanitize;
pub mod source_card;

pub use code_host_fetch::{resolve_code_host_fetch_target, CodeHostFetchTarget};
pub use code_metadata::{CodeHost, CodeMetadata};
pub use config::{AppConfig, LiveConfig, Mode, SearchSection};
pub use document::{
    BlockKind, DocumentChunk, DocumentKind, DocumentOutlineEntry, FetchDocument,
    FetchRenderMetadata, RenderFormat, RenderedBlock,
};
pub use error::{CoreError, CoreResult};
pub use fetch::{
    ExtractMode, ExtractedLink, FetchTransform, FetchTransformKind, FetchTrust, WebFetchRequest,
    WebFetchResponse,
};
pub use provider::{
    built_in_provider_descriptor, ProviderCapabilities, ProviderDescriptor, ProviderKind,
    KNOWN_PROVIDER_IDS,
};
pub use query::{
    resolve_max_results, Freshness, MaxResultsResolution, SafeSearch, SearchIntent,
    WebSearchRequest,
};
pub use repo_query::RepoQueryHints;
pub use result::{SearchWarning, TrustLevel};
pub use sanitize::{
    bound_text, frame, scan_injection_markers, strip_control_chars, MarkerHit, TrustMarkers,
    SNIPPET_MAX_CHARS, TITLE_MAX_CHARS,
};
pub use source_card::{RankReason, SourceCard, SourceKind, SourceMetadata};
