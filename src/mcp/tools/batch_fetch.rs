use super::common::*;
use super::repo_fetch::{run_repo_fetch, RepoFetchArgs};
use crate::fetch::FetchClient;
use crate::mcp::policy::{fetch_allowed, web_fetch_denied_message, Policy};
use crate::mcp::state::ServerState;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct BatchFetchArgs {
    /// Items to fetch. Must be non-empty.
    pub items: Vec<crate::core::batch_fetch::BatchFetchItem>,
    /// Maximum number of items to process. Defaults to server config.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_items: Option<usize>,
    /// Per-item character extraction cap. Defaults to server config.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_chars_per_item: Option<usize>,
    /// Total character budget across all items. Defaults to server config.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_total_chars: Option<usize>,
    /// Timeout in milliseconds. Defaults to server config.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
    /// Whether to continue fetching remaining items after a failure.
    /// Defaults to `true`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub continue_on_error: Option<bool>,
}

/// Inject deterministic web focus projection into a batch payload.
pub(crate) fn inject_web_focus_into_payload(
    mut payload: serde_json::Value,
    focus_query: Option<&str>,
    focus_max_chunks: Option<usize>,
    focus_max_chars: Option<usize>,
    effective_max_chars: usize,
    max_chars_cap: usize,
) -> serde_json::Value {
    let Some(query) = focus_query else {
        if let Some(obj) = payload.as_object_mut() {
            obj.insert("focus".to_string(), serde_json::Value::Null);
        }
        return payload;
    };
    let document: Option<crate::core::document::FetchDocument> = payload
        .get("document")
        .and_then(|d| serde_json::from_value(d.clone()).ok());
    let fetched = payload
        .get("fetched")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let selection = crate::core::fetch_policy::apply_focus_to_document(
        document.as_ref(),
        fetched,
        Some(query),
        focus_max_chunks,
        focus_max_chars,
        effective_max_chars,
        max_chars_cap,
    );
    if let Some(obj) = payload.as_object_mut() {
        match selection {
            Some(sel) => {
                obj.insert(
                    "focus".to_string(),
                    serde_json::to_value(&sel).unwrap_or(serde_json::Value::Null),
                );
            }
            None => {
                obj.insert("focus".to_string(), serde_json::Value::Null);
            }
        }
    }
    payload
}

/// Inject deterministic repo focus projection into a batch payload.
pub(crate) fn inject_repo_focus_into_payload(
    mut payload: serde_json::Value,
    focus_query: Option<&str>,
    focus_max_chunks: Option<usize>,
    focus_max_chars: Option<usize>,
    effective_max_chars: usize,
    max_chars_cap: usize,
    label: &str,
) -> serde_json::Value {
    let Some(query) = focus_query else {
        if let Some(obj) = payload.as_object_mut() {
            obj.insert("focus".to_string(), serde_json::Value::Null);
        }
        return payload;
    };
    let document: Option<crate::core::document::FetchDocument> =
        payload.get("document").and_then(|d| {
            if d.is_null() {
                None
            } else {
                serde_json::from_value(d.clone()).ok()
            }
        });
    let selection = if let Some(doc) = document.as_ref() {
        crate::core::fetch_policy::apply_focus_to_document(
            Some(doc),
            true,
            Some(query),
            focus_max_chunks,
            focus_max_chars,
            effective_max_chars,
            max_chars_cap,
        )
    } else {
        let text = payload.get("text").and_then(|v| v.as_str()).unwrap_or("");
        if text.trim().is_empty() {
            None
        } else {
            let max_chunks =
                crate::core::fetch_policy::focus_max_chunks_or_default(focus_max_chunks);
            let max_chars = crate::core::fetch_policy::focus_max_chars_or_default(
                focus_max_chars,
                effective_max_chars,
                max_chars_cap,
            );
            Some(crate::core::focus::select_focus_for_text(
                text, label, query, max_chunks, max_chars,
            ))
        }
    };
    if let Some(obj) = payload.as_object_mut() {
        match selection {
            Some(sel) => {
                obj.insert(
                    "focus".to_string(),
                    serde_json::to_value(&sel).unwrap_or(serde_json::Value::Null),
                );
            }
            None => {
                obj.insert("focus".to_string(), serde_json::Value::Null);
            }
        }
    }
    payload
}

/// Run the `batch_fetch` tool.
pub async fn run_batch_fetch(
    state: Arc<ServerState>,
    args: BatchFetchArgs,
) -> Result<serde_json::Value, ToolError> {
    use crate::core::batch_fetch::{
        BatchFetchItem, BatchFetchItemType, BatchFetchResponse, BatchFetchResult,
    };

    // Policy check
    if matches!(fetch_allowed(state.config.fetch.enabled), Policy::Deny) {
        return Err(ToolError::Validation(web_fetch_denied_message()));
    }

    // Validate items non-empty
    if args.items.is_empty() {
        return Err(ToolError::Validation("items must not be empty".to_string()));
    }

    // Validate top-level budget arguments. Per-item `max_chars` is
    // validated below in the pre-validation loop. A zero top-level
    // budget is rejected here because silently promoting it to 1
    // (via `.max(1)` later) would contradict the bounded-budget
    // contract and could return content when the caller requested
    // zero total output.
    if let Some(0) = args.max_items {
        return Err(ToolError::Validation("max_items must be > 0".to_string()));
    }
    if let Some(0) = args.max_chars_per_item {
        return Err(ToolError::Validation(
            "max_chars_per_item must be > 0".to_string(),
        ));
    }
    if let Some(0) = args.max_total_chars {
        return Err(ToolError::Validation(
            "max_total_chars must be > 0".to_string(),
        ));
    }
    if let Some(0) = args.timeout_ms {
        return Err(ToolError::Validation("timeout_ms must be > 0".to_string()));
    }

    // Resolve effective limits
    let batch_max_items = args
        .max_items
        .unwrap_or(state.config.fetch.batch_max_items)
        .min(state.config.fetch.batch_max_items_cap);
    let per_item_cap = args
        .max_chars_per_item
        .unwrap_or(state.config.fetch.batch_max_chars_per_item);
    let total_cap = args
        .max_total_chars
        .unwrap_or(state.config.fetch.batch_max_total_chars)
        .min(state.config.fetch.batch_max_total_chars_cap);
    let continue_on_error = args.continue_on_error.unwrap_or(true);

    // Validate item count
    if args.items.len() > state.config.fetch.batch_max_items_cap {
        return Err(ToolError::Validation(format!(
            "items count ({}) exceeds batch_max_items_cap ({})",
            args.items.len(),
            state.config.fetch.batch_max_items_cap
        )));
    }

    // Clamp to effective max_items
    let effective_items: Vec<&BatchFetchItem> = args.items.iter().take(batch_max_items).collect();
    let mut warnings = Vec::new();
    if effective_items.len() < args.items.len() {
        warnings.push(format!(
            "batch_item_count_truncated: requested {} items, processing {} (batch_max_items={})",
            args.items.len(),
            effective_items.len(),
            batch_max_items
        ));
    }

    // Pre-validate all items before launching any fetches.
    // Locator checks use the shared fetch-locator helpers so batch,
    // repo_fetch, and suggested-fetch conversions share host/ref/path
    // semantics. Focus checks reuse the exact web_fetch validation.
    for (i, item) in effective_items.iter().enumerate() {
        let focus_q = item.focus_query();
        let focus_chunks = item.focus_max_chunks();
        let focus_chars = item.focus_max_chars();
        if let Err(e) = crate::core::fetch_policy::validate_focus_query(focus_q) {
            return Err(ToolError::Validation(format!("item {i}: {e}")));
        }
        if let Err(e) = crate::core::fetch_policy::validate_focus_max_chunks(focus_chunks) {
            return Err(ToolError::Validation(format!("item {i}: {e}")));
        }
        if let Err(e) = crate::core::fetch_policy::validate_focus_max_chars(focus_chars) {
            return Err(ToolError::Validation(format!("item {i}: {e}")));
        }
        match item {
            BatchFetchItem::Web {
                url,
                max_chars,
                extract_mode,
                ..
            } => {
                if let Err(e) = crate::core::fetch_locator::validate_web_url(url) {
                    return Err(ToolError::Validation(format!("item {i}: {e}")));
                }
                if let Some(mc) = max_chars {
                    if *mc == 0 {
                        return Err(ToolError::Validation(format!(
                            "item {i}: max_chars must be > 0"
                        )));
                    }
                }
                let mode = extract_mode.unwrap_or(crate::core::fetch::ExtractMode::Text);
                if let Err(e) =
                    crate::core::fetch_policy::validate_focus_for_extract_mode(focus_q, mode)
                {
                    return Err(ToolError::Validation(format!("item {i}: {e}")));
                }
            }
            BatchFetchItem::Repo {
                owner,
                repo,
                path,
                host,
                max_chars,
                ..
            } => {
                if owner.trim().is_empty() {
                    return Err(ToolError::Validation(format!(
                        "item {i}: owner must not be empty"
                    )));
                }
                if repo.trim().is_empty() {
                    return Err(ToolError::Validation(format!(
                        "item {i}: repo must not be empty"
                    )));
                }
                if let Err(e) = crate::core::fetch_locator::validate_repo_path(path) {
                    return Err(ToolError::Validation(format!("item {i}: {e}")));
                }
                if let Err(e) = crate::core::fetch_locator::parse_batch_repo_host(host.as_deref()) {
                    return Err(ToolError::Validation(format!("item {i}: {e}")));
                }
                if let Some(mc) = max_chars {
                    if *mc == 0 {
                        return Err(ToolError::Validation(format!(
                            "item {i}: max_chars must be > 0"
                        )));
                    }
                }
            }
        }
    }

    let client: Arc<FetchClient> = state.fetch_client().ok_or_else(|| {
        ToolError::internal("fetch client unavailable; is [fetch].enabled = true?".to_string())
    })?;

    let concurrency = state.config.fetch.batch_concurrency;
    let semaphore = Arc::new(tokio::sync::Semaphore::new(concurrency));

    // Execute fetches in ordered bounded waves, preserving input order.
    //
    // When continue_on_error=true (default): each wave spawns up to
    // `concurrency` tasks concurrently via JoinSet. Budget tracking
    // and abort checks happen between waves.
    //
    // When continue_on_error=false: effective_concurrency is set to 1
    // so items are fetched one at a time, preserving strict
    // abort-on-first-failure semantics.
    let effective_concurrency = if continue_on_error { concurrency } else { 1 };
    let mut results: Vec<BatchFetchResult> = Vec::with_capacity(effective_items.len());
    let mut total_chars: usize = 0;
    let mut budget_exhausted = false;
    let mut aborted = false;

    for wave_start in (0..effective_items.len()).step_by(effective_concurrency) {
        // Pre-wave checks: skip remaining items if aborted or budget exhausted
        if aborted || budget_exhausted {
            for (i, item) in effective_items.iter().enumerate().skip(wave_start) {
                let msg = if budget_exhausted {
                    "total character budget exhausted".to_string()
                } else {
                    "batch aborted due to previous failure".to_string()
                };
                results.push(BatchFetchResult {
                    index: i,
                    item_type: match item {
                        BatchFetchItem::Web { .. } => BatchFetchItemType::Web,
                        BatchFetchItem::Repo { .. } => BatchFetchItemType::Repo,
                    },
                    label: item.label(),
                    stable_id: None,
                    ok: false,
                    response: None,
                    error: Some(msg),
                    chars_returned: 0,
                    truncated: false,
                });
            }
            break;
        }

        let wave_end = (wave_start + effective_concurrency).min(effective_items.len());
        let remaining_budget = total_cap.saturating_sub(total_chars);

        // Divide remaining budget across wave items to prevent concurrent
        // overshoot. Each item gets at most per_wave_item_budget chars.
        let wave_len = wave_end - wave_start;
        let per_wave_item_budget = remaining_budget
            .checked_div(wave_len)
            .unwrap_or(remaining_budget);
        let item_budget_cap = per_item_cap.max(1).min(per_wave_item_budget.max(1));
        // Maximum number of wave items that can be safely spawned without
        // overshooting the total budget. When remaining_budget is smaller
        // than wave_len, the remaining items are skipped before launching.
        let launchable = remaining_budget;

        let mut join_set = tokio::task::JoinSet::new();
        let mut wave_indices = Vec::new();
        // Number of items already spawned in this wave. Each spawn
        // reserves at least 1 character of the remaining budget so the
        // aggregate response cannot exceed max_total_chars.
        let mut spawned_in_wave: usize = 0;

        for (i, item) in effective_items
            .iter()
            .enumerate()
            .take(wave_end)
            .skip(wave_start)
        {
            if budget_exhausted || spawned_in_wave >= launchable {
                results.push(BatchFetchResult {
                    index: i,
                    item_type: match item {
                        BatchFetchItem::Web { .. } => BatchFetchItemType::Web,
                        BatchFetchItem::Repo { .. } => BatchFetchItemType::Repo,
                    },
                    label: item.label(),
                    stable_id: None,
                    ok: false,
                    response: None,
                    error: Some("total character budget exhausted".to_string()),
                    chars_returned: 0,
                    truncated: false,
                });
                continue;
            }

            wave_indices.push(i);
            spawned_in_wave += 1;

            let fetch_future = make_batch_fetch_future(
                i,
                item,
                item_budget_cap,
                state.clone(),
                client.clone(),
                semaphore.clone(),
                item.label(),
                args.timeout_ms,
                state.config.fetch.include_links_default,
            );

            join_set.spawn(fetch_future);
        }

        // Collect all wave results keyed by their returned index.
        // JoinSet::join_next() returns whichever task completes first,
        // so we must not associate results by iteration order.
        let mut wave_results: std::collections::BTreeMap<usize, BatchFetchResult> =
            std::collections::BTreeMap::new();

        while let Some(join_result) = join_set.join_next().await {
            match join_result {
                Ok(Ok(batch_result)) => {
                    wave_results.insert(batch_result.index, batch_result);
                }
                Ok(Err(tool_err)) => {
                    // Tool error without an index — cannot know which item.
                    // This should be rare; make_batch_fetch_future returns
                    // BatchFetchResult for known failures. Record as a
                    // special internal error that will be attached to a
                    // synthesized failure after collection.
                    tracing::warn!("batch_fetch tool error without index: {tool_err}");
                }
                Err(join_err) => {
                    // Task panic/cancellation — index is lost. Will be
                    // synthesized as a failure for missing indices below.
                    tracing::warn!("batch_fetch task panicked: {join_err}");
                }
            }
        }

        // Push results in input order, synthesizing failures for any
        // indices that are missing (panic, cancellation, or tool error).
        for idx in &wave_indices {
            match wave_results.remove(idx) {
                Some(mut batch_result) => {
                    if !batch_result.ok && !continue_on_error {
                        aborted = true;
                    }
                    // Enforce the aggregate total_chars budget per result:
                    // metadata fields (title/description/links) accounted in
                    // chars_returned may push the running total past
                    // max_total_chars even though the per-item cap was respected.
                    let remaining = total_cap.saturating_sub(total_chars);
                    if batch_result.chars_returned > remaining {
                        batch_result =
                            truncate_batch_result_to_budget(batch_result, remaining, total_cap);
                    }
                    total_chars += batch_result.chars_returned;
                    // The result's index is already correct from the future.
                    // No mutation needed.
                    results.push(batch_result);
                }
                None => {
                    // Index was not returned — task panicked or tool error.
                    if !continue_on_error {
                        aborted = true;
                    }
                    let item_type = match &effective_items[*idx] {
                        BatchFetchItem::Web { .. } => BatchFetchItemType::Web,
                        BatchFetchItem::Repo { .. } => BatchFetchItemType::Repo,
                    };
                    results.push(BatchFetchResult {
                        index: *idx,
                        item_type,
                        label: effective_items[*idx].label(),
                        stable_id: None,
                        ok: false,
                        response: None,
                        error: Some("task failed or panicked".to_string()),
                        chars_returned: 0,
                        truncated: false,
                    });
                }
            }
        }

        // Check budget after wave completes
        if total_chars >= total_cap {
            budget_exhausted = true;
        }
    }

    if budget_exhausted {
        warnings.push(format!(
            "batch_total_budget_exhausted: total character budget of {total_cap} was reached; remaining items skipped"
        ));
    }

    let fetched = results.iter().filter(|r| r.ok).count();
    let failed = results.iter().filter(|r| !r.ok).count();
    let truncated = results.iter().any(|r| r.truncated);
    let items_truncated = results.iter().filter(|r| r.truncated).count();
    let mut focused_items = 0usize;
    let mut focused_chunks_selected = 0usize;
    let mut focused_chars_returned = 0usize;
    let mut cache_hits = 0usize;
    let mut cache_revalidated = 0usize;
    let mut cache_misses = 0usize;
    let mut cache_bypassed = 0usize;
    let mut cache_not_cacheable = 0usize;
    for (result, item) in results.iter().zip(effective_items.iter()) {
        if item.focus_query().is_some() && result.ok {
            if let Some(payload) = result.response.as_ref() {
                if let Some(focus) = payload.get("focus") {
                    if !focus.is_null() {
                        focused_items += 1;
                        if let Some(chunks) = focus.get("chunks").and_then(|c| c.as_array()) {
                            focused_chunks_selected += chunks.len();
                        }
                        if let Some(total) = focus.get("total_chars").and_then(|c| c.as_u64()) {
                            focused_chars_returned += total as usize;
                        }
                    }
                }
            }
        }
        if result.ok && matches!(item, crate::core::batch_fetch::BatchFetchItem::Web { .. }) {
            if let Some(payload) = result.response.as_ref() {
                match payload
                    .get("cache_status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("miss")
                {
                    "hit" => cache_hits += 1,
                    "revalidated" => cache_revalidated += 1,
                    "bypassed" => cache_bypassed += 1,
                    "not_cacheable" => cache_not_cacheable += 1,
                    _ => cache_misses += 1,
                }
            } else {
                cache_misses += 1;
            }
        }
    }

    let telemetry = crate::core::batch_fetch::BatchFetchTelemetry {
        items_requested: effective_items.len(),
        items_completed: fetched,
        items_failed: failed,
        items_truncated,
        total_chars_returned: total_chars,
        focused_items,
        focused_chunks_selected,
        focused_chars_returned,
        aggregate_budget_exhausted: budget_exhausted,
        cache_hits,
        cache_revalidated,
        cache_misses,
        cache_bypassed,
        cache_not_cacheable,
    };

    let response = BatchFetchResponse {
        fetched,
        failed,
        truncated,
        total_chars_returned: total_chars,
        results,
        structured_warnings: crate::core::warning::convert_fetch_warnings(&warnings),
        warnings,
        telemetry: Some(telemetry),
    };

    let value = serde_json::to_value(&response)
        .map_err(|e| ToolError::internal(format!("serialization error: {e}")))?;
    Ok(value)
}

/// Trim a `BatchFetchResult`'s embedded `response` payload so that
/// `chars_returned` does not exceed `remaining` and the aggregate
/// total stays within `total_cap`. Text is truncated first (cheapest
/// and typically the largest field), then metadata fields are dropped
/// in priority order until the budget is satisfied. When the payload
/// cannot be trimmed (e.g. zero remaining budget), the embedded
/// response is replaced with `null` and `truncated` is set so callers
/// can see the item was omitted from the aggregate budget.
pub(crate) fn truncate_batch_result_to_budget(
    mut result: crate::core::batch_fetch::BatchFetchResult,
    remaining: usize,
    total_cap: usize,
) -> crate::core::batch_fetch::BatchFetchResult {
    if remaining == 0 {
        result.response = None;
        result.error = Some(format!(
            "batch_total_budget_exhausted: item truncated to fit remaining budget of 0 of max_total_chars={total_cap}"
        ));
        result.chars_returned = 0;
        result.truncated = true;
        return result;
    }

    let Some(mut payload) = result.response.take() else {
        result.chars_returned = 0;
        return result;
    };

    let budget = remaining;

    if let Some(text) = payload
        .get_mut("text")
        .and_then(|v| v.as_str().map(String::from))
    {
        let len = text.chars().count();
        if len > budget {
            let trimmed: String = text.chars().take(budget).collect();
            if let Some(obj) = payload.as_object_mut() {
                obj.insert("text".to_string(), serde_json::Value::String(trimmed));
            }
        }
    }

    let meta_chars = |obj: &serde_json::Map<String, serde_json::Value>| -> usize {
        obj.get("title")
            .and_then(|v| v.as_str())
            .map(|s| s.chars().count())
            .unwrap_or(0)
            + obj
                .get("description")
                .and_then(|v| v.as_str())
                .map(|s| s.chars().count())
                .unwrap_or(0)
            + obj
                .get("links")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .map(|l| {
                            l.get("url")
                                .and_then(|u| u.as_str())
                                .map(|s| s.chars().count())
                                .unwrap_or(0)
                                + l.get("text")
                                    .and_then(|t| t.as_str())
                                    .map(|s| s.chars().count())
                                    .unwrap_or(0)
                        })
                        .sum::<usize>()
                })
                .unwrap_or(0)
    };

    let text_chars = |obj: &serde_json::Map<String, serde_json::Value>| -> usize {
        obj.get("text")
            .and_then(|v| v.as_str())
            .map(|s| s.chars().count())
            .unwrap_or(0)
    };

    if let Some(obj) = payload.as_object_mut() {
        let mut current = text_chars(obj) + meta_chars(obj);
        if current > budget {
            loop {
                let popped = obj
                    .get_mut("links")
                    .and_then(|v| v.as_array_mut())
                    .and_then(|arr| {
                        if arr.is_empty() {
                            None
                        } else {
                            arr.pop();
                            Some(())
                        }
                    });
                if popped.is_none() {
                    break;
                }
                current = text_chars(obj) + meta_chars(obj);
                if current <= budget {
                    break;
                }
            }
        }
        if current > budget {
            obj.remove("description");
            current = text_chars(obj) + meta_chars(obj);
        }
        if current > budget {
            obj.remove("title");
            current = text_chars(obj) + meta_chars(obj);
        }
        result.chars_returned =
            if result.item_type == crate::core::batch_fetch::BatchFetchItemType::Repo {
                batch_payload_chars(&payload)
            } else {
                current
            };
    } else {
        result.chars_returned = 0;
    }

    if result.chars_returned > budget {
        result.response = None;
        result.error = Some(format!(
            "batch_total_budget_exhausted: item omitted because its response metadata exceeds the remaining budget of {budget} of max_total_chars={total_cap}"
        ));
        result.chars_returned = 0;
        result.truncated = true;
        return result;
    }

    result.response = Some(payload);
    result.truncated = true;
    result
}

pub(crate) fn batch_payload_chars(payload: &serde_json::Value) -> usize {
    serde_json::to_string(payload)
        .map(|serialized| serialized.chars().count())
        .unwrap_or(usize::MAX)
}

/// Build a boxed future that fetches a single batch item.
///
/// The future acquires a semaphore permit, executes the fetch, and
/// returns a `BatchFetchResult`. Extracted so both concurrent-wave
/// and sequential-mode paths can share the same fetch logic.
#[allow(clippy::too_many_arguments)]
fn make_batch_fetch_future(
    i: usize,
    item: &crate::core::batch_fetch::BatchFetchItem,
    item_max_chars: usize,
    state: Arc<ServerState>,
    client: Arc<FetchClient>,
    semaphore: Arc<tokio::sync::Semaphore>,
    label: String,
    timeout_ms: Option<u64>,
    include_links_default: bool,
) -> std::pin::Pin<
    Box<
        dyn std::future::Future<
                Output = Result<crate::core::batch_fetch::BatchFetchResult, ToolError>,
            > + Send,
    >,
> {
    use crate::core::batch_fetch::{BatchFetchItem, BatchFetchItemType, BatchFetchResult};
    use crate::core::identity::batch_fetch_id;

    match item {
        BatchFetchItem::Web {
            url,
            extract_mode,
            include_links,
            max_chars,
            cache_policy,
            max_cache_age_seconds,
            focus,
            focus_max_chunks,
            focus_max_chars,
        } => {
            let stable_id = batch_fetch_id(&label, i);
            let effective_max = max_chars.unwrap_or(item_max_chars).min(item_max_chars);
            let em = effective_max.max(1);
            let mode = extract_mode.unwrap_or(crate::core::fetch::ExtractMode::Text);
            let il = include_links.unwrap_or(include_links_default);
            let item_cache_policy =
                crate::core::fetch_policy::cache_policy_or_default(*cache_policy);
            let item_max_age = *max_cache_age_seconds;
            let focus_query = focus.clone();
            let focus_chunks = *focus_max_chunks;
            let focus_chars = *focus_max_chars;
            let max_chars_cap = state.config.fetch.max_chars_cap;
            if let Some(age) = item_max_age {
                if age > crate::core::fetch::MAX_CACHE_AGE_SECONDS {
                    return Box::pin(async move {
                        Ok(BatchFetchResult {
                            index: i,
                            item_type: BatchFetchItemType::Web,
                            label,
                            stable_id: Some(stable_id),
                            ok: false,
                            response: None,
                            error: Some(format!(
                                "max_cache_age_seconds must be <= {}",
                                crate::core::fetch::MAX_CACHE_AGE_SECONDS
                            )),
                            chars_returned: 0,
                            truncated: false,
                        })
                    });
                }
            }
            let url = url.clone();
            Box::pin(async move {
                let _permit = semaphore
                    .acquire_owned()
                    .await
                    .map_err(|e| ToolError::internal(format!("semaphore closed: {e}")))?;
                let web_client: Arc<FetchClient> = if let Some(ms) = timeout_ms {
                    Arc::new(client.with_timeout_ms(ms).map_err(|e| {
                        ToolError::internal(format!("failed to create timeout override: {e}"))
                    })?)
                } else {
                    client
                };

                use crate::fetch::cache::{
                    build_raw_cache_key, build_raw_response_hash, should_cache_response, CacheScope,
                };
                use crate::fetch::origin::OriginKey;
                let origin_key = match OriginKey::from_url(
                    &url::Url::parse(&url)
                        .map_err(|e| ToolError::internal(format!("invalid URL: {e}")))?,
                ) {
                    Some(k) => k,
                    None => {
                        return Ok(BatchFetchResult {
                            index: i,
                            item_type: BatchFetchItemType::Web,
                            label,
                            stable_id: Some(stable_id),
                            ok: false,
                            response: None,
                            error: Some("URL must be http or https".into()),
                            chars_returned: 0,
                            truncated: false,
                        });
                    }
                };

                let scope = CacheScope::Anonymous;

                let bypass_cache =
                    item_cache_policy == crate::core::fetch::FetchCachePolicy::Bypass;
                let caller_max_age = item_max_age.map(std::time::Duration::from_secs);
                if !bypass_cache {
                    if let Some(ref cache) = state.fetch_cache {
                        let raw_key = build_raw_cache_key(&url, &scope);
                        if let Some(raw_entry) = cache.get_raw(&raw_key).await {
                            let age_ok = match caller_max_age {
                                None => true,
                                Some(max) => {
                                    std::time::SystemTime::now()
                                        .duration_since(raw_entry.fetched_at)
                                        .unwrap_or(std::time::Duration::ZERO)
                                        <= max
                                }
                            };
                            let serve_hit = raw_entry.freshness.is_fresh()
                                && age_ok
                                && item_cache_policy
                                    == crate::core::fetch::FetchCachePolicy::Default;
                            if serve_hit {
                                let derived_key = crate::fetch::cache::build_derived_key(
                                    &scope,
                                    build_raw_response_hash(&raw_entry.body),
                                    mode,
                                    em,
                                    il,
                                    None,
                                    None,
                                    false,
                                    state.config.fetch.sanitize_output,
                                );
                                if let Some(derived) = cache.get_derived(&derived_key).await {
                                    let raw_payload = serde_json::json!({
                                        "url": url,
                                        "final_url": raw_entry.final_url,
                                        "title": derived.response.title,
                                        "description": derived.response.description,
                                        "content_type": raw_entry.content_type,
                                        "status": raw_entry.status,
                                        "fetched": true,
                                        "truncated": derived.response.truncated,
                                        "trust": "external_untrusted",
                                        "text": derived.response.text,
                                        "links": derived.response.links,
                                        "links_seen": derived.response.links_seen,
                                        "links_truncated": derived.response.links_truncated,
                                        "warnings": Vec::<String>::new(),
                                        "trust_markers": serde_json::to_value(&derived.response.trust_markers)
                                            .unwrap_or(serde_json::json!({})),
                                        "document": derived.response.document,
                                        "fetch_transform": serde_json::Value::Null,
                                        "structured_warnings": Vec::<serde_json::Value>::new(),
                                        "cache_status": "hit",
                                        "attempt_count": 1,
                                        "retry_after_ms": serde_json::Value::Null,
                                        "origin_backoff_ms": serde_json::Value::Null,
                                        "browser_profile": serde_json::Value::Null,
                                        "browser_profile_scope": "ephemeral",
                                        "manual_interaction_required": false,
                                        "transport": if raw_entry.representation == crate::fetch::cache::RawRepresentation::BrowserDom { "browser" } else { "http" },
                                        "browser_escalated": raw_entry.browser_escalated,
                                    });
                                    let payload = inject_web_focus_into_payload(
                                        raw_payload,
                                        focus_query.as_deref(),
                                        focus_chunks,
                                        focus_chars,
                                        em,
                                        max_chars_cap,
                                    );
                                    let body_chars = derived
                                        .response
                                        .document
                                        .as_ref()
                                        .map(|d| d.text_chars_returned)
                                        .unwrap_or_else(|| {
                                            derived
                                                .response
                                                .text
                                                .as_ref()
                                                .map(|t| t.chars().count())
                                                .unwrap_or(0)
                                        });
                                    let meta_chars = derived
                                        .response
                                        .title
                                        .as_ref()
                                        .map(|s| s.chars().count())
                                        .unwrap_or(0)
                                        + derived
                                            .response
                                            .description
                                            .as_ref()
                                            .map(|s| s.chars().count())
                                            .unwrap_or(0)
                                        + derived
                                            .response
                                            .links
                                            .iter()
                                            .map(|l| l.url.chars().count() + l.text.chars().count())
                                            .sum::<usize>();
                                    return Ok(BatchFetchResult {
                                        index: i,
                                        item_type: BatchFetchItemType::Web,
                                        label,
                                        stable_id: Some(stable_id),
                                        ok: true,
                                        response: Some(payload),
                                        error: None,
                                        chars_returned: body_chars + meta_chars,
                                        truncated: derived.response.truncated,
                                    });
                                }
                            } else if !raw_entry.freshness.no_store
                                && !raw_entry.freshness.no_cache
                                && (raw_entry.validators.etag.is_some()
                                    || raw_entry.validators.last_modified.is_some())
                            {
                                let circuit_blocked =
                                    if let Some(ref ctrl) = state.origin_controller {
                                        ctrl.circuit_is_open(&origin_key).await.is_some()
                                    } else {
                                        false
                                    };
                                if !circuit_blocked {
                                    let conditional =
                                        crate::fetch::cache::build_request_conditional_headers(
                                            &raw_entry.validators,
                                        );
                                    if !conditional.is_empty() {
                                        if let Ok((status, _headers, _body, _)) =
                                            web_client.fetch_conditional(&url, &conditional).await
                                        {
                                            if status == 304 {
                                                let derived_key =
                                                    crate::fetch::cache::build_derived_key(
                                                        &scope,
                                                        build_raw_response_hash(&raw_entry.body),
                                                        mode,
                                                        em,
                                                        il,
                                                        None,
                                                        None,
                                                        false,
                                                        state.config.fetch.sanitize_output,
                                                    );
                                                if let Some(derived) =
                                                    cache.get_derived(&derived_key).await
                                                {
                                                    let mut updated_freshness =
                                                        raw_entry.freshness.clone();
                                                    updated_freshness.fetched_at =
                                                        Some(std::time::SystemTime::now());
                                                    let updated_entry =
                                                        crate::fetch::cache::RawFetchCacheEntry {
                                                            freshness: updated_freshness,
                                                            ..raw_entry.clone()
                                                        };
                                                    cache.insert_raw(raw_key, updated_entry).await;

                                                    let raw_payload = serde_json::json!({
                                                        "url": url,
                                                        "final_url": raw_entry.final_url,
                                                        "title": derived.response.title,
                                                        "description": derived.response.description,
                                                        "content_type": raw_entry.content_type,
                                                        "status": raw_entry.status,
                                                        "fetched": true,
                                                        "truncated": derived.response.truncated,
                                                        "trust": "external_untrusted",
                                                        "text": derived.response.text,
                                                        "links": derived.response.links,
                                                        "links_seen": derived.response.links_seen,
                                                        "links_truncated": derived.response.links_truncated,
                                                        "warnings": Vec::<String>::new(),
                                                        "trust_markers": serde_json::to_value(&derived.response.trust_markers)
                                                            .unwrap_or(serde_json::json!({})),
                                                        "document": derived.response.document,
                                                        "fetch_transform": serde_json::Value::Null,
                                                        "structured_warnings": Vec::<serde_json::Value>::new(),
                                                        "cache_status": "revalidated",
                                                        "attempt_count": 1,
                                                        "retry_after_ms": serde_json::Value::Null,
                                                        "origin_backoff_ms": serde_json::Value::Null,
                                                        "browser_profile": serde_json::Value::Null,
                                                        "browser_profile_scope": "ephemeral",
                                                        "manual_interaction_required": false,
                                                        "transport": if raw_entry.representation == crate::fetch::cache::RawRepresentation::BrowserDom { "browser" } else { "http" },
                                                        "browser_escalated": raw_entry.browser_escalated,
                                                    });
                                                    let payload = inject_web_focus_into_payload(
                                                        raw_payload,
                                                        focus_query.as_deref(),
                                                        focus_chunks,
                                                        focus_chars,
                                                        em,
                                                        max_chars_cap,
                                                    );
                                                    let body_chars = derived
                                                        .response
                                                        .document
                                                        .as_ref()
                                                        .map(|d| d.text_chars_returned)
                                                        .unwrap_or_else(|| {
                                                            derived
                                                                .response
                                                                .text
                                                                .as_ref()
                                                                .map(|t| t.chars().count())
                                                                .unwrap_or(0)
                                                        });
                                                    let meta_chars = derived
                                                        .response
                                                        .title
                                                        .as_ref()
                                                        .map(|s| s.chars().count())
                                                        .unwrap_or(0)
                                                        + derived
                                                            .response
                                                            .description
                                                            .as_ref()
                                                            .map(|s| s.chars().count())
                                                            .unwrap_or(0)
                                                        + derived
                                                            .response
                                                            .links
                                                            .iter()
                                                            .map(|l| {
                                                                l.url.chars().count()
                                                                    + l.text.chars().count()
                                                            })
                                                            .sum::<usize>();
                                                    return Ok(BatchFetchResult {
                                                        index: i,
                                                        item_type: BatchFetchItemType::Web,
                                                        label,
                                                        stable_id: Some(stable_id),
                                                        ok: true,
                                                        response: Some(payload),
                                                        error: None,
                                                        chars_returned: body_chars + meta_chars,
                                                        truncated: derived.response.truncated,
                                                    });
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                let max_attempts = state.config.fetch.retry_max_attempts.max(1);
                let mut last_err: Option<crate::fetch::FetchError> = None;
                let mut response = None;
                let mut attempts_made = 0usize;
                let mut retry_after_ms: Option<u64> = None;
                let mut cache_status = if bypass_cache || state.fetch_cache.is_none() {
                    "bypassed"
                } else {
                    "miss"
                };
                let deadline = std::time::Instant::now()
                    + std::time::Duration::from_millis(state.config.fetch.timeout_ms);

                for attempt in 0..max_attempts {
                    attempts_made += 1;
                    let _permit = if let Some(ref controller) = state.origin_controller {
                        match controller.acquire(&origin_key).await {
                            Ok(p) => Some(p),
                            Err(e) => {
                                return Ok(BatchFetchResult {
                                    index: i,
                                    item_type: BatchFetchItemType::Web,
                                    label,
                                    stable_id: Some(stable_id),
                                    ok: false,
                                    response: None,
                                    error: Some(format!("origin_backoff: {e}")),
                                    chars_returned: 0,
                                    truncated: false,
                                });
                            }
                        }
                    } else {
                        None
                    };

                    match web_client.fetch(&url, Some(em), mode, il, None).await {
                        Ok(resp) => {
                            if let Some(ref ctrl) = state.origin_controller {
                                ctrl.record_success(&origin_key).await;
                            }
                            response = Some(resp);
                            break;
                        }
                        Err(e) => {
                            let kind = e.kind();
                            let class = match &e {
                                crate::fetch::FetchError::HttpStatus(status, _) => {
                                    crate::fetch::origin::classify_http_status(*status)
                                }
                                _ => crate::fetch::origin::classify_network_error(&e.to_string()),
                            };
                            let is_retryable = matches!(
                                kind,
                                crate::fetch::FetchErrorKind::Timeout
                                    | crate::fetch::FetchErrorKind::NetworkError
                            ) || matches!(
                                class,
                                crate::fetch::origin::OriginFailureClass::Retryable
                                    | crate::fetch::origin::OriginFailureClass::RateLimited
                            );
                            if let Some(ref ctrl) = state.origin_controller {
                                let decision = ctrl.record_failure(&origin_key, class).await;
                                match decision {
                                    crate::fetch::origin::OriginBackoffDecision::CircuitOpened {
                                        delay_ms,
                                        ..
                                    } => {
                                        return Ok(BatchFetchResult {
                                            index: i,
                                            item_type: BatchFetchItemType::Web,
                                            label,
                                            stable_id: Some(stable_id),
                                            ok: false,
                                            response: None,
                                            error: Some(format!(
                                                "origin_circuit_open: {e}, retry in {delay_ms}ms"
                                            )),
                                            chars_returned: 0,
                                            truncated: false,
                                        });
                                    }
                                    crate::fetch::origin::OriginBackoffDecision::Backoff {
                                        delay_ms,
                                        retry_after_ms: ra,
                                        ..
                                    } if is_retryable && attempt + 1 < max_attempts => {
                                        retry_after_ms = ra;
                                        let remaining = deadline.saturating_duration_since(
                                            std::time::Instant::now(),
                                        );
                                        let sleep_dur = std::time::Duration::from_millis(
                                            delay_ms
                                                .min(state.config.fetch.timeout_ms / 2)
                                                .min(remaining.as_millis().min(u128::from(u64::MAX)) as u64),
                                        );
                                        if !sleep_dur.is_zero() {
                                            tokio::time::sleep(sleep_dur).await;
                                        }
                                        continue;
                                    }
                                    _ => {}
                                }
                            }
                            last_err = Some(e);
                            break;
                        }
                    }
                }

                let ok_label = label.clone();
                match response {
                    Some(resp) => {
                        if let Some(ref cache) = state.fetch_cache {
                            let raw_key = build_raw_cache_key(&url, &scope);
                            let raw_body_bytes = resp.raw_body.as_deref().unwrap_or(&[]);
                            let raw_hash = build_raw_response_hash(raw_body_bytes);

                            let (mut cache_freshness, validators) = if let Some(ref headers) =
                                resp.response_headers
                            {
                                let header_map: reqwest::header::HeaderMap = headers
                                    .iter()
                                    .filter_map(|(k, v)| {
                                        let name =
                                            reqwest::header::HeaderName::from_bytes(k.as_bytes())
                                                .ok()?;
                                        let val = reqwest::header::HeaderValue::from_str(v).ok()?;
                                        Some((name, val))
                                    })
                                    .collect();
                                crate::fetch::cache::CacheFreshness::from_headers(&header_map)
                            } else {
                                (
                                    crate::fetch::cache::CacheFreshness::default(),
                                    crate::fetch::cache::CacheValidators {
                                        etag: None,
                                        last_modified: None,
                                    },
                                )
                            };
                            if cache_freshness.max_age.is_none()
                                && cache_freshness.expires.is_none()
                            {
                                let ttl = std::time::Duration::from_secs(
                                    state.config.fetch.cache.default_ttl_seconds,
                                );
                                cache_freshness.max_age = Some(ttl);
                            }
                            cache_freshness.fetched_at = Some(std::time::SystemTime::now());
                            if should_cache_response(
                                resp.status,
                                resp.content_type.as_deref(),
                                &cache_freshness,
                                &scope,
                            ) {
                                let raw_entry = crate::fetch::cache::RawFetchCacheEntry {
                                    final_url: resp.final_url.clone(),
                                    status: resp.status,
                                    headers: resp.response_headers.clone().unwrap_or_default(),
                                    body: Arc::from(raw_body_bytes),
                                    fetched_at: std::time::SystemTime::now(),
                                    freshness: cache_freshness,
                                    validators,
                                    scope: scope.clone(),
                                    content_type: resp.content_type.clone(),
                                    content_length_header: resp
                                        .document
                                        .as_ref()
                                        .and_then(|document| document.metadata.as_ref())
                                        .and_then(|metadata| metadata.content_length),
                                    redirect_count: resp
                                        .document
                                        .as_ref()
                                        .and_then(|document| document.metadata.as_ref())
                                        .map(|metadata| metadata.redirects_followed)
                                        .unwrap_or(0),
                                    representation: if resp.transport.as_deref() == Some("browser")
                                    {
                                        crate::fetch::cache::RawRepresentation::BrowserDom
                                    } else {
                                        crate::fetch::cache::RawRepresentation::Http
                                    },
                                    truncated: resp.truncated,
                                    browser_escalated: resp.browser_escalated,
                                };
                                cache.insert_raw(raw_key.clone(), raw_entry).await;

                                let derived_key = crate::fetch::cache::build_derived_key(
                                    &scope,
                                    raw_hash,
                                    mode,
                                    em,
                                    il,
                                    None,
                                    None,
                                    false,
                                    state.config.fetch.sanitize_output,
                                );
                                cache
                                    .insert_derived(
                                        derived_key.clone(),
                                        derived_cache_entry(raw_hash, &derived_key, &resp),
                                    )
                                    .await;
                            } else {
                                cache_status = "not_cacheable";
                            }
                        }

                        let truncated = resp.truncated;
                        let structured =
                            crate::core::warning::convert_fetch_warnings(&resp.warnings);
                        let raw_payload = serde_json::json!({
                            "url": resp.url,
                            "final_url": resp.final_url,
                            "stable_id": resp.stable_id,
                            "source_id": resp.source_id,
                            "title": resp.title,
                            "description": resp.description,
                            "content_type": resp.content_type,
                            "status": resp.status,
                            "fetched": resp.fetched,
                            "truncated": resp.truncated,
                            "trust": "external_untrusted",
                            "text": resp.text,
                            "links": resp.links,
                            "links_seen": resp.links_seen,
                            "links_truncated": resp.links_truncated,
                            "warnings": resp.warnings,
                            "trust_markers": serde_json::to_value(&resp.trust_markers)
                                .unwrap_or(serde_json::json!({})),
                            "document": resp.document,
                            "fetch_transform": resp.fetch_transform,
                            "structured_warnings": structured,
                            "cache_status": cache_status,
                            "attempt_count": attempts_made,
                            "retry_after_ms": retry_after_ms,
                            "origin_backoff_ms": serde_json::Value::Null,
                            "browser_profile": serde_json::Value::Null,
                            "browser_profile_scope": "ephemeral",
                            "manual_interaction_required": false,
                            "transport": resp.transport.as_deref().unwrap_or("http"),
                            "browser_escalated": resp.browser_escalated,
                        });
                        let payload = inject_web_focus_into_payload(
                            raw_payload,
                            focus_query.as_deref(),
                            focus_chunks,
                            focus_chars,
                            em,
                            max_chars_cap,
                        );
                        let body_chars = resp
                            .document
                            .as_ref()
                            .map(|d| d.text_chars_returned)
                            .unwrap_or_else(|| {
                                resp.text.as_ref().map(|t| t.chars().count()).unwrap_or(0)
                            });
                        let meta_chars =
                            resp.title.as_ref().map(|s| s.chars().count()).unwrap_or(0)
                                + resp
                                    .description
                                    .as_ref()
                                    .map(|s| s.chars().count())
                                    .unwrap_or(0)
                                + resp
                                    .links
                                    .iter()
                                    .map(|l| l.url.chars().count() + l.text.chars().count())
                                    .sum::<usize>();
                        let text_len = body_chars + meta_chars;
                        Ok(BatchFetchResult {
                            index: i,
                            item_type: BatchFetchItemType::Web,
                            label: ok_label,
                            stable_id: Some(stable_id),
                            ok: true,
                            response: Some(payload),
                            error: None,
                            chars_returned: text_len,
                            truncated,
                        })
                    }
                    None => {
                        let err = last_err.unwrap_or(crate::fetch::FetchError::Unknown(
                            "fetch failed after all attempts".into(),
                        ));
                        Ok(BatchFetchResult {
                            index: i,
                            item_type: BatchFetchItemType::Web,
                            label: ok_label,
                            stable_id: Some(stable_id),
                            ok: false,
                            response: None,
                            error: Some(format!("{}: {}", err.error_code(), err)),
                            chars_returned: 0,
                            truncated: false,
                        })
                    }
                }
            })
        }
        BatchFetchItem::Repo {
            host,
            owner,
            repo,
            ref_name,
            commit_sha,
            path,
            line_start,
            line_end,
            context_before,
            context_after,
            max_chars,
            focus,
            focus_max_chunks,
            focus_max_chars,
        } => {
            let stable_id = batch_fetch_id(&label, i);
            let effective_max = max_chars.unwrap_or(item_max_chars).min(item_max_chars);
            let repo_focus_query = focus.clone();
            let repo_focus_chunks = *focus_max_chunks;
            let repo_focus_chars = *focus_max_chars;
            let repo_max_cap = state.config.fetch.max_chars_cap;
            let repo_label = label.clone();
            let repo_args = RepoFetchArgs {
                host: host.clone(),
                owner: owner.clone(),
                repo: repo.clone(),
                ref_name: ref_name.clone(),
                commit_sha: commit_sha.clone(),
                path: path.clone(),
                line_start: *line_start,
                line_end: *line_end,
                context_before: *context_before,
                context_after: *context_after,
                max_chars: Some(effective_max),
                timeout_ms,
                test_fetch_url: None,
                symbol: None,
                symbol_kind: None,
                match_text: None,
                expand_to_block: None,
                max_block_lines: None,
                prefer_local: None,
            };
            Box::pin(async move {
                let ok_label = label.clone();
                let _permit = semaphore
                    .acquire_owned()
                    .await
                    .map_err(|e| ToolError::internal(format!("semaphore closed: {e}")))?;
                match run_repo_fetch(state, repo_args).await {
                    Ok(raw_payload) => {
                        let payload = inject_repo_focus_into_payload(
                            raw_payload,
                            repo_focus_query.as_deref(),
                            repo_focus_chunks,
                            repo_focus_chars,
                            effective_max.max(1),
                            repo_max_cap,
                            &repo_label,
                        );
                        let text_len = batch_payload_chars(&payload);
                        let truncated = payload
                            .get("truncated")
                            .and_then(|t| t.as_bool())
                            .unwrap_or(false);
                        Ok(BatchFetchResult {
                            index: i,
                            item_type: BatchFetchItemType::Repo,
                            label: ok_label,
                            stable_id: Some(stable_id),
                            ok: true,
                            response: Some(payload),
                            error: None,
                            chars_returned: text_len,
                            truncated,
                        })
                    }
                    Err(e) => {
                        let err_stable_id = batch_fetch_id(&label, i);
                        Ok(BatchFetchResult {
                            index: i,
                            item_type: BatchFetchItemType::Repo,
                            label,
                            stable_id: Some(err_stable_id),
                            ok: false,
                            response: None,
                            error: Some(e.to_string()),
                            chars_returned: 0,
                            truncated: false,
                        })
                    }
                }
            })
        }
    }
}
