//! Deterministic change snapshots for fetched documents.

mod diff;
mod store;

use crate::core::config::FetchSnapshotSection;
use crate::core::fetch::{ExtractMode, FetchSnapshotRequest, WebFetchResponse};
use crate::fetch::cache::CacheScope;

pub(crate) use store::{
    compare_text, observe_removed, validate_request, SnapshotIdentity, SnapshotOutcome,
};

pub(crate) async fn removed_payload(
    config: FetchSnapshotSection,
    request: FetchSnapshotRequest,
    identity: SnapshotIdentity,
    url: &str,
    status: u16,
) -> serde_json::Value {
    let outcome = observe_removed(config, request, identity).await;
    let mut payload = serde_json::json!({
        "url": url,
        "final_url": url,
        "status": status,
        "fetched": false,
        "truncated": false,
        "trust": "external_untrusted",
        "text": null,
        "warnings": outcome.warnings,
        "structured_warnings": [],
        "cache_status": "miss"
    });
    if let Some(change) = outcome.change {
        payload["change"] = serde_json::to_value(change).unwrap_or(serde_json::Value::Null);
    }
    payload
}

pub(crate) async fn compare_response(
    config: FetchSnapshotSection,
    request: FetchSnapshotRequest,
    scope: CacheScope,
    url: String,
    extract_mode: ExtractMode,
    scoping_fingerprint: Option<u64>,
    response: &WebFetchResponse,
) -> SnapshotOutcome {
    if response.transport.as_deref() == Some("browser") {
        return SnapshotOutcome::warning("snapshot_browser_excluded");
    }
    let text = if let Some(document) = response.document.as_ref() {
        if document.document_truncated || document.block_truncated {
            return SnapshotOutcome::warning("snapshot_content_limit_exceeded");
        }
        let (text, truncated) = crate::fetch::projection::project_document_text(
            &document.blocks,
            response.raw_text_cap.unwrap_or(1_048_576),
            document.text_format == "markdown",
        );
        if truncated {
            return SnapshotOutcome::warning("snapshot_content_limit_exceeded");
        }
        text
    } else if let Some(text) = response.text.as_deref() {
        text.to_string()
    } else {
        return SnapshotOutcome::warning("snapshot_content_unavailable");
    };
    let representation_class = match response.representation {
        Some(crate::core::fetch::FetchRepresentation::Html) => "html",
        Some(crate::core::fetch::FetchRepresentation::Markdown) => "markdown",
        None => "other",
    };
    let identity = SnapshotIdentity {
        scope,
        url,
        extract_mode,
        scoping_fingerprint,
        representation_class: representation_class.to_string(),
    };
    compare_text(config, request, identity, text).await
}
