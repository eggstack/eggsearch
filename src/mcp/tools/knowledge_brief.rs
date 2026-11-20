use std::sync::Arc;

use serde::{Deserialize, Serialize};

use super::common::{ToolError, ToolErrorCode};
use crate::core::config::default_config_path;
use crate::core::knowledge_delta::{
    bound_chars, bound_items, parse_caller_override, parse_refresh, BriefInputError,
    CutoffOverrideInput,
};
use crate::mcp::policy::{live_allowed, live_search_denied_message, Policy};
use crate::mcp::state::ServerState;
use crate::meta::knowledge_delta::{
    EvidenceFetchFuture, FetchedEvidence, KnowledgeEvidenceFetcher,
};

#[derive(Clone, Debug, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct KnowledgeBriefArgs {
    /// Topic passed through unchanged as the first subquery.
    pub topic: String,
    /// Model identifier. An unrecognized model resolves to unknown and is not an error.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,
    /// Caller cutoff. Must include date, precision, source, and reason, and must not be labeled provider-official.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cutoff_override: Option<CutoffOverrideInput>,
    /// Maximum emitted deltas. Defaults to 8 and is capped at 8.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_items: Option<usize>,
    /// Maximum Unicode scalars across emitted delta objects. Defaults to 6000.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_chars: Option<usize>,
    /// `live`, `prefer_live`, or `index`. Omitted means live and writes nothing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refresh: Option<String>,
    /// Write selected public evidence into the local index.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub retain: bool,
    /// Read related indexed topics. Does not search live by itself.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub related_topics: bool,
    /// Language hint passed through to search.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
}

struct BriefToolFetcher {
    state: Arc<ServerState>,
}

impl KnowledgeEvidenceFetcher for BriefToolFetcher {
    fn fetch<'a>(&'a self, url: &'a str, topic: &'a str) -> EvidenceFetchFuture<'a> {
        let state = Arc::clone(&self.state);
        let url = url.to_string();
        let topic = topic.to_string();
        Box::pin(async move {
            let value = super::web_fetch::run_web_fetch_for_knowledge_brief(
                state,
                url,
                topic,
                crate::core::knowledge_delta::EXCERPT_CHAR_CAP,
            )
            .await
            .map_err(|_| "bounded web fetch failed".to_string())?;
            let mut excerpt = String::new();
            let mut spans = Vec::new();
            if let Some(chunks) = value["document"]["chunks"].as_array() {
                for chunk in chunks.iter().take(2) {
                    let Some(text) = chunk["text"].as_str() else {
                        continue;
                    };
                    if !excerpt.is_empty() {
                        excerpt.push('\n');
                    }
                    excerpt.push_str(text);
                    spans.push(serde_json::json!({
                        "kind": "document_chunk",
                        "chunk_id": chunk["chunk_id"],
                        "block_start": chunk["block_start"],
                        "block_end": chunk["block_end"],
                        "heading_path": chunk["heading_path"],
                    }));
                }
            }
            if excerpt.is_empty() {
                excerpt = value["text"].as_str().unwrap_or_default().to_string();
            }
            if spans.is_empty() && !excerpt.is_empty() {
                spans.push(serde_json::json!({
                    "kind": "fetched_text",
                    "offset_start": 0,
                    "offset_end": excerpt.chars().count(),
                }));
            }
            let status = value["status"].as_u64().unwrap_or_default();
            let cache_status = value["cache_status"].as_str().unwrap_or_default();
            let cacheable = (200..300).contains(&status)
                && matches!(cache_status, "miss" | "hit" | "revalidated")
                && value["trust"] == "external_untrusted"
                && value["browser_profile"].is_null()
                && value["transport"] == "http"
                && value["manual_interaction_required"].is_null();
            let source_truncated = value["truncated"].as_bool().unwrap_or(false)
                || value["document"]["document_truncated"]
                    .as_bool()
                    .unwrap_or(false);
            Ok(FetchedEvidence {
                excerpt,
                source_title: value["title"].as_str().map(ToString::to_string),
                spans,
                cacheable,
                source_truncated,
            })
        })
    }
}

pub async fn run_knowledge_brief(
    state: Arc<ServerState>,
    args: KnowledgeBriefArgs,
) -> Result<serde_json::Value, ToolError> {
    let refresh = parse_refresh(args.refresh.as_deref()).map_err(input_error)?;
    if refresh.searches_live() && matches!(live_allowed(state.config.search.mode), Policy::Deny) {
        return Err(ToolError::Validation(live_search_denied_message(
            "knowledge_brief",
        )));
    }
    if args.topic.trim().is_empty() {
        return Err(input_error(BriefInputError::EmptyTopic));
    }
    if args.retain && !state.config.knowledge_delta.index.enabled {
        return Err(ToolError::Validation(
            "retain requires [knowledge_delta.index].enabled; a live call does not open the index"
                .to_string(),
        ));
    }
    let caller_cutoff = match args.cutoff_override.as_ref() {
        Some(input) => Some(parse_caller_override(input).map_err(input_error)?),
        None => None,
    };
    let max_items = bound_items(args.max_items).map_err(input_error)?;
    let max_chars = bound_chars(args.max_chars).map_err(input_error)?;
    let config_dir = default_config_path()
        .parent()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    let query = crate::meta::knowledge_delta::BriefQuery {
        topic: args.topic,
        model_id: args.model_id.filter(|value| !value.trim().is_empty()),
        caller_cutoff,
        max_items,
        max_chars,
        refresh,
        retain: args.retain,
        related_topics: args.related_topics,
        language: args.language.filter(|value| !value.trim().is_empty()),
        partition: "anonymous".into(),
    };
    let fetcher = BriefToolFetcher {
        state: Arc::clone(&state),
    };
    let run = crate::meta::knowledge_delta::BriefRun {
        adapter: state.adapter.as_ref(),
        config: state.config.as_ref(),
        max_results_cap: state.config.search.max_results_cap,
        config_dir,
        evidence_fetcher: Some(&fetcher),
    };
    crate::meta::knowledge_delta::run_knowledge_brief(&run, &query)
        .await
        .map_err(|failure| match failure {
            crate::meta::knowledge_delta::BriefFailure::RetrievalFailed(message) => {
                ToolError::execution_with_data(
                    ToolErrorCode::UpstreamFailed,
                    message,
                    serde_json::json!({
                        "no_temporal_claim": true,
                        "not_evidence_of_absence": true,
                    }),
                )
            }
        })
}

fn input_error(error: BriefInputError) -> ToolError {
    match error {
        BriefInputError::UnknownRefresh(value) => {
            ToolError::invalid_semantic_value("refresh", &value, &["live", "prefer_live", "index"])
        }
        other => ToolError::Validation(other.message()),
    }
}
