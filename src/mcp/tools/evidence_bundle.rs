use serde::{Deserialize, Serialize};

use super::common::*;

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct EvidenceBundleArgs {
    /// Optional goal description for this bundle.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub goal: Option<String>,
    /// Source cards from search responses to include in the bundle.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sources: Vec<crate::core::evidence_bundle::EvidenceSourceInput>,
    /// Fetched items from fetch responses to include in the bundle.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fetches: Vec<crate::core::evidence_bundle::EvidenceFetchInput>,
    /// Whether to include unfetched sources (default true).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_unfetched_sources: Option<bool>,
    /// Maximum number of sources (default 50, cap 200).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_sources: Option<usize>,
    /// Maximum number of fetched items (default 20, cap 100).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_fetched_items: Option<usize>,
    /// Maximum total characters across all fetched text (default 100000, cap 500000).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_total_chars: Option<usize>,
    /// Response detail: compact, standard, or diagnostic. Bundle identity is
    /// unchanged across all modes; the canonical bundle is always returned.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response_detail: Option<crate::mcp::projection::ResponseDetail>,
}

/// Run the `build_evidence_bundle` tool. Packages already-selected
/// evidence from search and fetch responses into a deterministic,
/// non-summarizing bundle for multi-agent handoff.
pub fn run_build_evidence_bundle(args: EvidenceBundleArgs) -> Result<serde_json::Value, ToolError> {
    use crate::core::evidence_bundle::EvidenceBundleRequest;

    if let Some(max_sources) = args.max_sources {
        if max_sources > crate::core::evidence_bundle::MAX_SOURCES_CAP {
            return Err(ToolError::Validation(format!(
                "max_sources must not exceed {}",
                crate::core::evidence_bundle::MAX_SOURCES_CAP
            )));
        }
    }
    if let Some(max_fetched_items) = args.max_fetched_items {
        if max_fetched_items > crate::core::evidence_bundle::MAX_FETCHED_ITEMS_CAP {
            return Err(ToolError::Validation(format!(
                "max_fetched_items must not exceed {}",
                crate::core::evidence_bundle::MAX_FETCHED_ITEMS_CAP
            )));
        }
    }
    if let Some(max_total_chars) = args.max_total_chars {
        if max_total_chars > crate::core::evidence_bundle::MAX_TOTAL_CHARS_CAP {
            return Err(ToolError::Validation(format!(
                "max_total_chars must not exceed {}",
                crate::core::evidence_bundle::MAX_TOTAL_CHARS_CAP
            )));
        }
    }

    if args.sources.is_empty() && args.fetches.is_empty() {
        return Err(ToolError::Validation(
            "at least one source or fetch input is required to build an evidence bundle"
                .to_string(),
        ));
    }

    let request = EvidenceBundleRequest {
        goal: args.goal,
        sources: args.sources,
        fetches: args.fetches,
        include_unfetched_sources: args.include_unfetched_sources,
        max_sources: args.max_sources,
        max_fetched_items: args.max_fetched_items,
        max_total_chars: args.max_total_chars,
        warnings: vec![],
        research_claims: None,
        research_conflicts: None,
    };

    let bundle = crate::meta::evidence_bundle::build_evidence_bundle(request);

    let value = serde_json::to_value(&bundle)
        .map_err(|e| ToolError::internal(format!("serialization error: {e}")))?;
    let detail = crate::mcp::projection::ResponseDetail::from_opt(args.response_detail);
    Ok(crate::mcp::projection::project(
        "build_evidence_bundle",
        value,
        detail,
    ))
}
