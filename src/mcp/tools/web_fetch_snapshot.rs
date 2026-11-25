use crate::core::config::{FetchSnapshotSection, McpSchemaProfile};
use crate::core::fetch::{FetchSnapshotRequest, WebFetchResponse};
use crate::fetch::cache::CacheScope;
use crate::mcp::projection::{project_with_budget, ResponseDetail};

pub(super) struct RemovedPayloadContext<'a> {
    pub request: Option<&'a FetchSnapshotRequest>,
    pub error: &'a crate::fetch::FetchError,
    pub config: FetchSnapshotSection,
    pub identity: crate::fetch::snapshot::SnapshotIdentity,
    pub url: &'a str,
    pub response_detail: Option<ResponseDetail>,
    pub schema_profile: McpSchemaProfile,
    pub brief_max_chars: usize,
}

pub(super) async fn removed_payload(
    context: RemovedPayloadContext<'_>,
) -> Option<serde_json::Value> {
    let RemovedPayloadContext {
        request,
        error,
        config,
        identity,
        url,
        response_detail,
        schema_profile,
        brief_max_chars,
    } = context;
    let (Some(request), crate::fetch::FetchError::HttpStatus(status, _)) = (request, error) else {
        return None;
    };
    if !matches!(*status, 404 | 410) {
        return None;
    }
    let payload =
        crate::fetch::snapshot::removed_payload(config, request.clone(), identity, url, *status)
            .await;
    let detail = ResponseDetail::from_opt_for_profile(response_detail, schema_profile);
    Some(project_with_budget(
        "web_fetch",
        payload,
        detail,
        brief_max_chars,
    ))
}

pub(super) async fn compare_response(
    config: FetchSnapshotSection,
    request: FetchSnapshotRequest,
    scope: CacheScope,
    url: String,
    extract_mode: crate::core::fetch::ExtractMode,
    scoping_fingerprint: Option<u64>,
    response: &mut WebFetchResponse,
) -> Option<serde_json::Value> {
    let outcome = crate::fetch::snapshot::compare_response(
        config,
        request,
        scope,
        url,
        extract_mode,
        scoping_fingerprint,
        response,
    )
    .await;
    response.warnings.extend(outcome.warnings);
    outcome
        .change
        .and_then(|change| serde_json::to_value(change).ok())
}
