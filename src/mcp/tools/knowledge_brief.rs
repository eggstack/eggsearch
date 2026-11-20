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
            let (mut excerpt, mut spans) = value["document"]["chunks"]
                .as_array()
                .map(|chunks| extract_brief_excerpt(chunks))
                .unwrap_or_default();
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
            let cacheable = fetched_evidence_is_cacheable(&value);
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

fn extract_brief_excerpt(chunks: &[serde_json::Value]) -> (String, Vec<serde_json::Value>) {
    let mut ordered: Vec<_> = chunks
        .iter()
        .enumerate()
        .filter(|(_, chunk)| {
            chunk["text"]
                .as_str()
                .is_some_and(|text| !text.trim().is_empty())
        })
        .collect();
    ordered.sort_by_key(|(index, chunk)| {
        let has_heading_path = chunk["heading_path"]
            .as_array()
            .is_some_and(|path| !path.is_empty());
        (
            !has_heading_path,
            chunk["block_start"].as_u64().unwrap_or(u64::MAX),
            *index,
        )
    });
    let mut excerpt = String::new();
    let mut spans = Vec::new();
    for (_, chunk) in ordered.into_iter().take(2) {
        let Some(text) = chunk["text"].as_str() else {
            continue;
        };
        if !excerpt.is_empty() {
            excerpt.push('\n');
        }
        excerpt.push_str(text);
        let has_heading_path = chunk["heading_path"]
            .as_array()
            .is_some_and(|path| !path.is_empty());
        spans.push(serde_json::json!({
            "kind": "document_chunk",
            "chunk_id": chunk["chunk_id"],
            "block_start": chunk["block_start"],
            "block_end": chunk["block_end"],
            "heading_path": chunk["heading_path"],
            "selection_basis": if has_heading_path { "heading_path" } else { "block_order" },
            "chrome_may_remain": !has_heading_path,
        }));
    }
    (excerpt, spans)
}

fn fetched_evidence_is_cacheable(value: &serde_json::Value) -> bool {
    let status = value["status"].as_u64().unwrap_or_default();
    let cache_status = value["cache_status"].as_str().unwrap_or_default();
    (200..300).contains(&status)
        && matches!(cache_status, "miss" | "hit" | "revalidated")
        && value["trust"] == "external_untrusted"
        && value["browser_profile"].is_null()
        && value["transport"] == "http"
        && value["manual_interaction_required"].as_bool() == Some(false)
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
            crate::meta::knowledge_delta::BriefFailure::InvalidInput(message) => {
                ToolError::Validation(message)
            }
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

#[cfg(test)]
mod tests {
    use super::extract_brief_excerpt;

    #[test]
    fn brief_fetcher_prefers_heading_chunks_and_records_residual_chrome() {
        let chunks = vec![
            serde_json::json!({
                "text": "| Hacker Newsnew|past|comments|ask|show|jobs|submitlogin |",
                "chunk_id": "chunk_nav",
                "heading_path": [],
                "block_start": 0,
                "block_end": 0,
            }),
            serde_json::json!({
                "text": "Story: A bounded article passage.",
                "chunk_id": "chunk_story",
                "heading_path": ["Story"],
                "block_start": 4,
                "block_end": 4,
            }),
        ];
        let (excerpt, spans) = extract_brief_excerpt(&chunks);
        assert!(excerpt.starts_with("Story: A bounded article passage."));
        assert_eq!(spans[0]["selection_basis"], "heading_path");
        assert_eq!(spans[0]["chrome_may_remain"], false);
        assert_eq!(spans[1]["chrome_may_remain"], true);
    }

    #[test]
    fn ordinary_http_response_with_false_manual_interaction_flag_is_cacheable() {
        let response = serde_json::json!({
            "status": 200,
            "cache_status": "miss",
            "trust": "external_untrusted",
            "browser_profile": null,
            "transport": "http",
            "manual_interaction_required": false,
        });
        assert!(super::fetched_evidence_is_cacheable(&response));
    }
}
