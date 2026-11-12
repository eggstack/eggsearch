use super::batch_fetch_item::make_batch_fetch_future;
use super::common::*;
use crate::fetch::FetchClient;
use crate::mcp::policy::{fetch_allowed, web_fetch_denied_message, Policy};
use crate::mcp::state::ServerState;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    /// Response detail: brief, compact, standard, or diagnostic.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response_detail: Option<crate::mcp::projection::ResponseDetail>,
}

pub(crate) fn prepare_batch_fetch_client(
    client: Arc<FetchClient>,
    timeout_ms: Option<u64>,
) -> Result<Arc<FetchClient>, ToolError> {
    match timeout_ms {
        Some(timeout_ms) => Ok(Arc::new(client.with_timeout_ms(timeout_ms).map_err(
            |e| ToolError::internal(format!("failed to create timeout override: {e}")),
        )?)),
        None => Ok(client),
    }
}

/// Inject deterministic web focus projection into a batch payload.
pub(crate) fn inject_web_focus_into_payload(
    mut payload: serde_json::Value,
    focus_query: Option<&str>,
    focus_max_chunks: Option<usize>,
    focus_max_chars: Option<usize>,
    typed_document: Option<&crate::core::document::FetchDocument>,
    effective_max_chars: usize,
    max_chars_cap: usize,
) -> serde_json::Value {
    let Some(query) = focus_query else {
        if let Some(obj) = payload.as_object_mut() {
            obj.insert("focus".to_string(), serde_json::Value::Null);
        }
        return payload;
    };
    let document_from_payload = if typed_document.is_none() {
        payload
            .get("document")
            .and_then(|d| serde_json::from_value(d.clone()).ok())
    } else {
        None
    };
    let document = typed_document.or(document_from_payload.as_ref());
    let fetched = payload
        .get("fetched")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let selection = crate::core::fetch_policy::apply_focus_to_document(
        document,
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
    let mut progress = crate::mcp::progress::ProgressSink::disabled();
    run_batch_fetch_with_progress(state, args, &mut progress).await
}

pub(crate) fn batch_fetch_plan_key(args: &BatchFetchArgs) -> String {
    let mut key = crate::mcp::cursor::PlanKey::new(crate::mcp::cursor::CursorTool::BatchFetch)
        .opt_usize(args.max_items)
        .opt_usize(args.max_chars_per_item)
        .opt_usize(args.max_total_chars)
        .flag(args.continue_on_error.unwrap_or(true));
    for item in &args.items {
        key = key.str(&serde_json::to_string(item).unwrap_or_default());
    }
    key.finish()
}

pub async fn run_batch_fetch_with_progress(
    state: Arc<ServerState>,
    args: BatchFetchArgs,
    progress: &mut crate::mcp::progress::ProgressSink,
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
    let cursor = crate::mcp::cursor::cursor_args(args.cursor.as_deref()).map_err(|e| {
        ToolError::budget_invalid(format!("cursor is not a valid replay token: {}", e.code()))
    })?;
    if let Some(ref cursor) = cursor {
        cursor
            .require_tool(crate::mcp::cursor::CursorTool::BatchFetch)
            .map_err(cursor_rejected)?;
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

    let plan_key = batch_fetch_plan_key(&args);
    if let Some(ref cursor) = cursor {
        cursor
            .require_plan_key(&plan_key)
            .map_err(cursor_rejected)?;
    }
    let budget_ms = cursor
        .as_ref()
        .map(|cursor| cursor.timeout_ms().min(state.config.fetch.timeout_ms))
        .unwrap_or(state.config.fetch.timeout_ms);
    let item_ids: Vec<String> = args
        .items
        .iter()
        .enumerate()
        .map(|(index, item)| crate::core::identity::batch_fetch_id(&item.label(), index))
        .collect();
    progress.phase("plan");
    let resolved = crate::mcp::cursor::resolve_page(
        item_ids.len(),
        &item_ids,
        cursor.as_ref(),
        batch_max_items,
        |_| 1,
    );
    let base = resolved.window.start;
    let cursor_status = if resolved.stale {
        Some("stale")
    } else if resolved.resumed {
        Some("ok")
    } else {
        None
    };
    let next_cursor = crate::mcp::cursor::encode_next(
        crate::mcp::cursor::CursorTool::BatchFetch,
        &plan_key,
        &item_ids,
        resolved.window,
        budget_ms,
    );
    let effective_items: Vec<&BatchFetchItem> = args
        .items
        .iter()
        .skip(base)
        .take(resolved.window.len())
        .collect();
    let mut warnings = Vec::new();
    if effective_items.len() < args.items.len() {
        warnings.push(format!(
            "batch_item_count_truncated: requested {} items, processing {} from index {} (batch_max_items={})",
            args.items.len(),
            effective_items.len(),
            base,
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
    let client = prepare_batch_fetch_client(
        client,
        if cursor.is_some() {
            Some(budget_ms)
        } else {
            args.timeout_ms
        },
    )?;

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
    // Results are recorded by item index and emitted in index order. Wave
    // results arrive in JoinSet completion order and skipped items are
    // discovered during the spawn loop, so a plain `Vec` of pushes would
    // interleave them and desynchronize the index-based accounting below.
    let mut results: Vec<Option<BatchFetchResult>> = vec![None; effective_items.len()];
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
                results[i] = Some(BatchFetchResult {
                    index: base + i,
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
        // overshooting the total budget. Every spawn reserves at least one
        // character of `remaining_budget`, so the reservation count and the
        // item count share the same unit: when remaining_budget is smaller
        // than the wave length, the tail is skipped before launching.
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
                results[i] = Some(BatchFetchResult {
                    index: base + i,
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
                base + i,
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
                    let local = batch_result.index.saturating_sub(base);
                    wave_results.insert(local, batch_result);
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
                    results[*idx] = Some(batch_result);
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
                    results[*idx] = Some(BatchFetchResult {
                        index: base + *idx,
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

        progress.dispatch(wave_end, effective_items.len());

        // Check budget after wave completes
        if total_chars >= total_cap {
            budget_exhausted = true;
        }
    }

    // Every item index is filled by the wave loop above (spawned, skipped, or
    // synthesized as a failure), so flattening yields input order.
    let results: Vec<BatchFetchResult> = results.into_iter().flatten().collect();

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
    // `results` is in input order, so this pairs each result with its own item.
    for (item_index, (result, item)) in results.iter().zip(effective_items.iter()).enumerate() {
        debug_assert_eq!(
            result.index,
            base + item_index,
            "batch_fetch result out of input order"
        );
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
    progress.phase("merge");
    let value = with_cursor_keys(value, next_cursor, cursor_status);
    let detail = crate::mcp::projection::ResponseDetail::from_opt_for_profile(
        args.response_detail,
        state.config.mcp.schema_profile,
    );
    progress.phase("project");
    Ok(crate::mcp::projection::project_with_budget(
        "batch_fetch",
        value,
        detail,
        state.config.mcp.brief_max_chars,
    ))
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
