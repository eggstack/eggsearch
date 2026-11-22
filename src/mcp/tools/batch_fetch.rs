use super::batch_fetch_item::{make_batch_fetch_future, BatchFetchItemContext};
use super::common::*;
use crate::fetch::FetchClient;
use crate::mcp::policy::{fetch_allowed, web_fetch_denied_message, Policy};
use crate::mcp::state::ServerState;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone, Debug, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct BatchFetchArgs {
    /// Non-empty list of URL or repository targets.
    pub items: Vec<crate::core::batch_fetch::BatchFetchItem>,
    /// Item limit; defaults to server config.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_items: Option<usize>,
    /// Per-item character cap; defaults to server config.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_chars_per_item: Option<usize>,
    /// Total character cap; defaults to server config.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_total_chars: Option<usize>,
    /// Timeout; defaults to server config.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
    /// Continue after item failure; defaults to true.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub continue_on_error: Option<bool>,
    /// Zero-based first item index.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset: Option<usize>,
    /// Items from offset; clamped to server maximum.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,
    /// Response detail: brief, compact, standard, diagnostic.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response_detail: Option<crate::mcp::projection::ResponseDetail>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ItemDocumentWindow {
    pub offset: Option<usize>,
    pub limit: Option<usize>,
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
    batch_fetch_plan_key_over(args, args.max_items, args.max_total_chars)
}

fn batch_fetch_plan_key_over(
    args: &BatchFetchArgs,
    max_items: Option<usize>,
    max_total_chars: Option<usize>,
) -> String {
    let mut key = crate::mcp::plan_key::PlanKey::new(crate::mcp::plan_key::PlanTool::BatchFetch)
        .opt_usize(max_items)
        .opt_usize(args.max_chars_per_item)
        .opt_usize(max_total_chars)
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
    run_batch_fetch_with_host_scope_and_progress(state, args, None, progress)
        .await
        .map(|outcome| outcome.value)
}

pub(crate) async fn run_batch_fetch_with_host_scope_and_progress(
    state: Arc<ServerState>,
    args: BatchFetchArgs,
    host_local_scope: Option<serde_json::Value>,
    progress: &mut crate::mcp::progress::ProgressSink,
) -> Result<ToolOutcome, ToolError> {
    run_batch_fetch_with_item_windows_and_progress(
        state,
        args,
        Vec::new(),
        host_local_scope,
        progress,
    )
    .await
}

pub(crate) async fn run_batch_fetch_with_item_windows_and_progress(
    state: Arc<ServerState>,
    args: BatchFetchArgs,
    item_document_windows: Vec<Option<ItemDocumentWindow>>,
    host_local_scope: Option<serde_json::Value>,
    progress: &mut crate::mcp::progress::ProgressSink,
) -> Result<ToolOutcome, ToolError> {
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
    if let Some(0) = args.limit {
        return Err(ToolError::execution_with_repair(
            ToolErrorCode::InvalidSemanticValue,
            "limit must be > 0; an addressed window is a half-open item range".to_string(),
            RepairHint::new(Some("limit"), &[], None),
        ));
    }
    let addressed = args.offset.is_some() || args.limit.is_some();

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
    if addressed {
        let served_items = args
            .max_items
            .map(|value| value.min(state.config.fetch.batch_max_items_cap));
        let served_total = args
            .max_total_chars
            .map(|value| value.min(state.config.fetch.batch_max_total_chars_cap));
        let served = batch_fetch_plan_key_over(&args, served_items, served_total);
        if served != plan_key {
            let (field, supplied, effective) = if served_items != args.max_items {
                (
                    "max_items",
                    args.max_items.unwrap_or_default(),
                    served_items.unwrap_or_default(),
                )
            } else {
                (
                    "max_total_chars",
                    args.max_total_chars.unwrap_or_default(),
                    served_total.unwrap_or_default(),
                )
            };
            return Err(addressing_rejected(AddressError::PlanMismatch {
                field,
                supplied,
                effective,
            }));
        }
    }
    let item_total = args.items.len();
    let requested_limit = args.limit.unwrap_or(batch_max_items);
    let limit = requested_limit
        .min(state.config.fetch.batch_max_items_cap)
        .max(1);
    let limit_clamped = limit < requested_limit;
    progress.phase("plan");
    let window = AddressWindow::resolve(item_total, args.offset, limit);
    let effective_items: Vec<&BatchFetchItem> = args
        .items
        .iter()
        .skip(window.from)
        .take(window.len())
        .collect();
    let mut warnings = Vec::new();
    if window.from > 0 || window.to < item_total {
        warnings.push(format!(
            "batch_item_count_truncated: requested {} items, processing {} from index {} (batch_max_items={})",
            item_total,
            effective_items.len(),
            window.from,
            batch_max_items
        ));
    }

    if !item_document_windows.is_empty() && item_document_windows.len() != args.items.len() {
        return Err(ToolError::internal(
            "document window count does not match fetch item count",
        ));
    }
    for (index, document_window) in item_document_windows.iter().enumerate() {
        let Some(document_window) = document_window else {
            continue;
        };
        if document_window.limit == Some(0) {
            return Err(ToolError::execution_with_repair(
                ToolErrorCode::InvalidSemanticValue,
                "web item limit must be > 0; a document window is a half-open character range"
                    .to_string(),
                RepairHint::new(Some("items.limit"), &[], None),
            ));
        }
        if !matches!(&args.items[index], BatchFetchItem::Web { .. }) {
            return Err(ToolError::Validation(
                "document offset and limit apply only to web items".to_string(),
            ));
        }
    }

    let local_view = if effective_items
        .iter()
        .any(|item| matches!(item, BatchFetchItem::Repo { .. }))
    {
        Some(
            state
                .local_workspace_view(host_local_scope.as_ref())
                .map_err(ToolError::Validation)?,
        )
    } else {
        None
    };

    // Pre-validate all items before launching any fetches.
    // Locator checks use shared fetch-locator and repo-request validators.
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
                max_cache_age_seconds,
                pdf,
                render,
                browser_profile,
                include_screenshot,
                ..
            } => {
                let target = crate::core::fetch_locator::resolve_fetch_target(
                    crate::core::fetch_locator::FetchTarget::WebUrl(url),
                )
                .map_err(|error| ToolError::Validation(format!("item {i}: {error}")))?;
                let crate::core::fetch_locator::FetchLocator::WebUrl(resolved_url) = target else {
                    return Err(ToolError::internal(
                        "web batch item resolved to a non-web target",
                    ));
                };
                if let Some(mc) = max_chars {
                    if *mc == 0 {
                        return Err(ToolError::Validation(format!(
                            "item {i}: max_chars must be > 0"
                        )));
                    }
                }
                crate::core::fetch_policy::validate_cache_age(*max_cache_age_seconds)
                    .map_err(|error| ToolError::Validation(format!("item {i}: {error}")))?;
                super::web_fetch::validate_batch_route_options(
                    &state,
                    &resolved_url,
                    render.as_deref(),
                    browser_profile.as_deref(),
                    include_screenshot.unwrap_or(false),
                )
                .map_err(|error| ToolError::Validation(format!("item {i}: {error}")))?;
                #[cfg(not(feature = "pdf"))]
                let _ = pdf;
                #[cfg(feature = "pdf")]
                if let Some(spec) = pdf.as_ref().and_then(|options| options.pages.as_deref()) {
                    crate::fetch::pdf::parse_pdf_page_spec(spec, state.config.fetch.pdf_max_pages)
                        .map_err(|error| ToolError::Validation(format!("item {i}: {error}")))?;
                }
                let mode = extract_mode.unwrap_or(crate::core::fetch::ExtractMode::Text);
                if let Err(e) =
                    crate::core::fetch_policy::validate_focus_for_extract_mode(focus_q, mode)
                {
                    return Err(ToolError::Validation(format!("item {i}: {e}")));
                }
            }
            BatchFetchItem::Repo { target, host, .. } => {
                let locator = crate::core::fetch_locator::batch_repo_item_to_locator(
                    host.as_deref(),
                    &target.owner,
                    &target.repo,
                    target.ref_name.as_deref(),
                    target.commit_sha.as_deref(),
                    &target.path,
                )
                .map_err(|error| ToolError::Validation(format!("item {i}: {error}")))?;
                let effective_max = target.max_chars.unwrap_or(per_item_cap).min(per_item_cap);
                let _parsed_symbol_kind =
                    super::common::parse_symbol_kind_arg(target.symbol_kind.as_deref())
                        .map_err(|error| ToolError::Validation(format!("item {i}: {error}")))?;
                let workspace_target =
                    matches!(locator, crate::core::fetch_locator::FetchLocator::Local(_));
                let request = match &locator {
                    crate::core::fetch_locator::FetchLocator::Local(local) => {
                        let mut request =
                            crate::core::repo_fetch::RepoFetchRequest::from(target.clone());
                        request.host = None;
                        request.target.owner = local.root.clone();
                        request.target.repo = local.path.clone();
                        request.target.ref_name = None;
                        request.target.commit_sha = None;
                        request.target.path = local.path.clone();
                        request.target.max_chars = Some(effective_max);
                        request.timeout_ms = args.timeout_ms;
                        request.target.prefer_local = None;
                        request
                    }
                    crate::core::fetch_locator::FetchLocator::Repo(remote) => {
                        let mut request =
                            crate::core::repo_fetch::RepoFetchRequest::from(target.clone());
                        request.host = remote.host;
                        request.target.owner = remote.owner.clone().unwrap_or_default();
                        request.target.repo = remote.repo.clone().unwrap_or_default();
                        request.target.ref_name = remote.ref_name.clone();
                        request.target.commit_sha = remote.commit_sha.clone();
                        request.target.path = remote.path.clone();
                        request.target.max_chars = Some(effective_max);
                        request.timeout_ms = args.timeout_ms;
                        request
                    }
                    crate::core::fetch_locator::FetchLocator::WebUrl(_) => {
                        return Err(ToolError::internal(
                            "repo batch item resolved to a web target",
                        ));
                    }
                };
                request
                    .validate(state.config.fetch.max_chars_cap)
                    .map_err(|error| ToolError::Validation(format!("item {i}: {error}")))?;

                let view = local_view
                    .as_ref()
                    .expect("repo items resolve a local view");
                if let crate::core::fetch_locator::FetchLocator::Local(local) = &locator {
                    let backend = view
                        .backend()
                        .filter(|backend| backend.is_enabled())
                        .ok_or_else(|| {
                            ToolError::Validation(
                                "local workspace search is not enabled".to_string(),
                            )
                        })?;
                    let roots = backend.roots();
                    let root_entry = roots.iter().find(|(_, root)| {
                        root.file_name()
                            .and_then(|name| name.to_str())
                            .is_some_and(|name| name == local.root)
                    });
                    let (_, root_path) = root_entry.ok_or_else(|| {
                        ToolError::Validation(format!(
                            "unknown workspace root '{}'; available roots: {}",
                            local.root,
                            roots
                                .iter()
                                .filter_map(|(_, root)| root
                                    .file_name()
                                    .and_then(|name| name.to_str()))
                                .collect::<Vec<_>>()
                                .join(", ")
                        ))
                    })?;
                    super::repo_fetch::validate_workspace_fetch_path(
                        root_path,
                        &local.path,
                        backend.config(),
                    )
                    .await
                    .map_err(|error| ToolError::Validation(format!("item {i}: {error}")))?;
                }

                if !workspace_target
                    && target.prefer_local.unwrap_or(false)
                    && host_local_scope.is_some()
                {
                    if !view.is_enabled() {
                        return Err(ToolError::Validation(
                            "local repository fetch was requested, but the authorized host scope has no local workspace".to_string(),
                        ));
                    }
                    let inventory = view.inventory().await;
                    let parsed_host = super::common::parse_code_host_arg(host.as_deref())
                        .map_err(|error| ToolError::Validation(format!("item {i}: {error}")))?;
                    if crate::meta::local_inventory::match_local_repo(
                        &inventory,
                        parsed_host.as_ref(),
                        &target.owner,
                        &target.repo,
                    )
                    .is_none()
                    {
                        return Err(ToolError::Validation(
                            "local repository fetch was requested, but no matching repository exists in the authorized workspace".to_string(),
                        ));
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
        args.timeout_ms
            .filter(|ms| *ms < state.config.fetch.timeout_ms),
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
    let mut images: Vec<Option<ImageAttachment>> = std::iter::repeat_with(|| None)
        .take(effective_items.len())
        .collect();
    let mut total_chars: usize = 0;
    let mut total_payload_chars: usize = 0;
    let mut budget_exhausted = false;
    let mut budget_warning: Option<String> = None;
    let mut aborted = false;

    for wave_start in (0..effective_items.len()).step_by(effective_concurrency) {
        // Pre-wave checks: skip remaining items if aborted or budget exhausted
        if aborted || budget_exhausted {
            if budget_exhausted && budget_warning.is_none() {
                if let Some(item) = effective_items.get(wave_start) {
                    budget_warning = Some(batch_total_budget_warning(
                        &item.label(),
                        "was skipped",
                        0,
                        total_cap,
                    ));
                }
            }
            for (i, item) in effective_items.iter().enumerate().skip(wave_start) {
                let msg = if budget_exhausted {
                    "total character budget exhausted".to_string()
                } else {
                    "batch aborted due to previous failure".to_string()
                };
                results[i] = Some(BatchFetchResult {
                    index: window.from + i,
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
                    payload_chars: 0,
                    degraded_fields: Vec::new(),
                    truncated: false,
                });
            }
            break;
        }

        let wave_end = (wave_start + effective_concurrency).min(effective_items.len());
        let remaining_budget = total_cap.saturating_sub(total_payload_chars);

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
                if budget_warning.is_none() {
                    budget_warning = Some(batch_total_budget_warning(
                        &item.label(),
                        "was skipped",
                        total_cap.saturating_sub(total_chars),
                        total_cap,
                    ));
                }
                results[i] = Some(BatchFetchResult {
                    index: window.from + i,
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
                    payload_chars: 0,
                    degraded_fields: Vec::new(),
                    truncated: false,
                });
                continue;
            }

            wave_indices.push(i);
            spawned_in_wave += 1;

            let fetch_future = make_batch_fetch_future(BatchFetchItemContext {
                index: window.from + i,
                item,
                item_max_chars: item_budget_cap,
                state: state.clone(),
                client: client.clone(),
                semaphore: semaphore.clone(),
                label: item.label(),
                timeout_ms: args.timeout_ms,
                host_local_scope: host_local_scope.clone(),
                document_window: item_document_windows
                    .get(window.from + i)
                    .copied()
                    .flatten(),
            });

            join_set.spawn(fetch_future);
        }

        // Collect all wave results keyed by their returned index.
        // JoinSet::join_next() returns whichever task completes first,
        // so we must not associate results by iteration order.
        let mut wave_results: std::collections::BTreeMap<
            usize,
            (BatchFetchResult, Option<ImageAttachment>),
        > = std::collections::BTreeMap::new();

        while let Some(join_result) = join_set.join_next().await {
            match join_result {
                Ok(Ok((batch_result, image))) => {
                    let local = batch_result.index.saturating_sub(window.from);
                    wave_results.insert(local, (batch_result, image));
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
                Some((mut batch_result, image)) => {
                    let remaining = total_cap.saturating_sub(total_payload_chars);
                    batch_result =
                        truncate_batch_result_to_budget(batch_result, remaining, total_cap);
                    if !batch_result.ok && !continue_on_error {
                        aborted = true;
                    }
                    total_chars += batch_result.chars_returned;
                    total_payload_chars += batch_result.payload_chars;
                    if batch_result.ok && !batch_result.degraded_fields.is_empty() {
                        warnings.push(batch_payload_budget_warning(
                            &batch_result,
                            remaining,
                            total_cap,
                        ));
                    }
                    let omitted_for_budget = !batch_result.ok
                        && batch_result
                            .error
                            .as_deref()
                            .is_some_and(|error| error.starts_with("batch_total_budget_exhausted"));
                    if !batch_result.degraded_fields.is_empty() || omitted_for_budget {
                        budget_exhausted = true;
                        if budget_warning.is_none() {
                            budget_warning = Some(batch_total_budget_warning(
                                &batch_result.label,
                                if batch_result.ok {
                                    "was degraded"
                                } else {
                                    "was omitted"
                                },
                                remaining,
                                total_cap,
                            ));
                        }
                    }
                    if total_payload_chars >= total_cap && budget_warning.is_none() {
                        budget_warning = Some(batch_total_budget_warning(
                            &batch_result.label,
                            "reached the batch limit",
                            0,
                            total_cap,
                        ));
                    }
                    if batch_result.ok {
                        if let Some(mut image) = image {
                            let (label, _) =
                                crate::core::sanitize::strip_control_chars(&batch_result.label);
                            let (label, _) = crate::core::sanitize::bound_text(&label, 200);
                            image.association =
                                Some(format!("batch_fetch item {}: {label}", batch_result.index));
                            images[*idx] = Some(image);
                        }
                    }
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
                        index: window.from + *idx,
                        item_type,
                        label: effective_items[*idx].label(),
                        stable_id: None,
                        ok: false,
                        response: None,
                        error: Some("task failed or panicked".to_string()),
                        chars_returned: 0,
                        payload_chars: 0,
                        degraded_fields: Vec::new(),
                        truncated: false,
                    });
                }
            }
        }

        progress.dispatch(wave_end, effective_items.len());

        // Check budget after wave completes
        if total_payload_chars >= total_cap {
            budget_exhausted = true;
        }
    }

    // Every item index is filled by the wave loop above (spawned, skipped, or
    // synthesized as a failure), so flattening yields input order.
    let results: Vec<BatchFetchResult> = results.into_iter().flatten().collect();

    if let Some(warning) = budget_warning {
        warnings.push(warning);
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
            window.from + item_index,
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

    let mut response = BatchFetchResponse {
        fetched,
        failed,
        truncated,
        total_chars_returned: total_chars,
        total_payload_chars,
        results,
        structured_warnings: crate::core::warning::convert_fetch_warnings(&warnings),
        warnings,
        telemetry: Some(telemetry),
    };

    if limit_clamped {
        response
            .structured_warnings
            .push(crate::core::warning::AgentWarning::new(
                crate::core::warning::WarningCode::MaxResultsClamped,
                format!(
                    "limit was clamped to the server maximum of {} items",
                    state.config.fetch.batch_max_items_cap
                ),
            ));
    }
    let value = serde_json::to_value(&response)
        .map_err(|e| ToolError::internal(format!("serialization error: {e}")))?;
    progress.phase("merge");
    let value = if addressed {
        with_addressing_keys(value, ITEM_ADDRESS_KEYS, window)
    } else {
        value
    };
    let detail = crate::mcp::projection::ResponseDetail::from_opt_for_profile(
        args.response_detail,
        state.config.mcp.schema_profile,
    );
    progress.phase("project");
    let value = crate::mcp::projection::project_with_budget(
        "batch_fetch",
        value,
        detail,
        state.config.mcp.brief_max_chars,
    );
    Ok(ToolOutcome {
        value,
        images: images.into_iter().flatten().collect(),
    })
}

/// Apply an item's remaining aggregate payload budget, degrading optional
/// structural response fields before clipping the requested text. The emitted
/// payload is measured as its serialized character count; `chars_returned`
/// keeps the readable-text meaning. An item that cannot fit is omitted as a
/// failed, truncated result.
pub(crate) fn truncate_batch_result_to_budget(
    mut result: crate::core::batch_fetch::BatchFetchResult,
    remaining: usize,
    total_cap: usize,
) -> crate::core::batch_fetch::BatchFetchResult {
    if !result.ok {
        result.payload_chars = 0;
        return result;
    }

    let Some(mut payload) = result.response.take() else {
        return omit_batch_result_for_budget(result, remaining, total_cap);
    };
    let Some(payload_chars) = batch_payload_chars(&payload) else {
        return omit_batch_result_for_budget(result, remaining, total_cap);
    };
    if payload_chars <= remaining {
        return finish_batch_result(result, payload, payload_chars);
    }

    if result.item_type == crate::core::batch_fetch::BatchFetchItemType::Repo {
        if let Some(document) = payload
            .get_mut("document")
            .and_then(serde_json::Value::as_object_mut)
        {
            for field in ["blocks", "chunks"] {
                if document.remove(field).is_some() {
                    result.degraded_fields.push(format!("document.{field}"));
                }
            }
        }
        if !result.degraded_fields.is_empty() {
            let Some(payload_chars) = batch_payload_chars(&payload) else {
                return omit_batch_result_for_budget(result, remaining, total_cap);
            };
            if payload_chars <= remaining {
                return finish_batch_result(result, payload, payload_chars);
            }
        }

        if payload
            .as_object_mut()
            .is_some_and(|object| object.remove("document").is_some())
        {
            result.degraded_fields.push("document".to_string());
            let Some(payload_chars) = batch_payload_chars(&payload) else {
                return omit_batch_result_for_budget(result, remaining, total_cap);
            };
            if payload_chars <= remaining {
                return finish_batch_result(result, payload, payload_chars);
            }
        }
        if payload
            .as_object_mut()
            .is_some_and(|object| object.remove("lines").is_some())
        {
            result.degraded_fields.push("lines".to_string());
            let Some(payload_chars) = batch_payload_chars(&payload) else {
                return omit_batch_result_for_budget(result, remaining, total_cap);
            };
            if payload_chars <= remaining {
                return finish_batch_result(result, payload, payload_chars);
            }
        }

        let text = payload
            .get("text")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned);
        if let Some(text) = text.filter(|text| !text.is_empty()) {
            let chars: Vec<char> = text.chars().collect();
            if let Some(object) = payload.as_object_mut() {
                object.insert("truncated".to_string(), serde_json::Value::Bool(true));
            }
            let mut low = 0usize;
            let mut high = chars.len();
            let mut best = 0usize;
            while low <= high {
                let mid = low + (high - low) / 2;
                if let Some(object) = payload.as_object_mut() {
                    object.insert(
                        "text".to_string(),
                        serde_json::Value::String(chars[..mid].iter().collect()),
                    );
                }
                let Some(measured) = batch_payload_chars(&payload) else {
                    return omit_batch_result_for_budget(result, remaining, total_cap);
                };
                if measured <= remaining {
                    best = mid;
                    low = mid + 1;
                } else if mid == 0 {
                    break;
                } else {
                    high = mid - 1;
                }
            }
            if let Some(object) = payload.as_object_mut() {
                object.insert(
                    "text".to_string(),
                    serde_json::Value::String(chars[..best].iter().collect()),
                );
            }
            result.truncated = true;
            result.degraded_fields.push("text".to_string());
            let Some(payload_chars) = batch_payload_chars(&payload) else {
                return omit_batch_result_for_budget(result, remaining, total_cap);
            };
            if payload_chars <= remaining {
                return finish_batch_result(result, payload, payload_chars);
            }
        }

        return omit_batch_result_for_budget(result, remaining, total_cap);
    }

    let budget = remaining;
    let mut text_clipped = false;
    if let Some(text) = payload
        .get("text")
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
    {
        if text.chars().count() > budget {
            if let Some(obj) = payload.as_object_mut() {
                obj.insert(
                    "text".to_string(),
                    serde_json::Value::String(text.chars().take(budget).collect()),
                );
            }
            result.degraded_fields.push("text".to_string());
            text_clipped = true;
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
                if !result.degraded_fields.iter().any(|field| field == "links") {
                    result.degraded_fields.push("links".to_string());
                }
                current = text_chars(obj) + meta_chars(obj);
                if current <= budget {
                    break;
                }
            }
        }
        if current > budget && obj.remove("description").is_some() {
            result.degraded_fields.push("description".to_string());
            current = text_chars(obj) + meta_chars(obj);
        }
        if current > budget && obj.remove("title").is_some() {
            result.degraded_fields.push("title".to_string());
        }
    }
    if text_clipped {
        result.truncated = true;
    }
    let Some(payload_chars) = batch_payload_chars(&payload) else {
        return omit_batch_result_for_budget(result, remaining, total_cap);
    };
    if payload_chars > remaining {
        return omit_batch_result_for_budget(result, remaining, total_cap);
    }
    finish_batch_result(result, payload, payload_chars)
}

fn batch_payload_chars(response_value: &serde_json::Value) -> Option<usize> {
    let serialized = serde_json::to_string(response_value);
    let payload_chars = serialized.as_ref().map(|s| s.chars().count()).unwrap_or(0);
    serialized.map(|_| payload_chars).ok()
}

fn finish_batch_result(
    mut result: crate::core::batch_fetch::BatchFetchResult,
    payload: serde_json::Value,
    payload_chars: usize,
) -> crate::core::batch_fetch::BatchFetchResult {
    result.payload_chars = payload_chars;
    result.chars_returned = batch_returned_chars(&payload, result.item_type);
    result.response = Some(payload);
    result
}

fn batch_string_chars(value: Option<&serde_json::Value>) -> usize {
    value
        .and_then(serde_json::Value::as_str)
        .map(str::chars)
        .map(Iterator::count)
        .unwrap_or(0)
}

fn batch_payload_metadata_chars(
    payload: &serde_json::Value,
    item_type: crate::core::batch_fetch::BatchFetchItemType,
) -> usize {
    let mut chars = batch_string_chars(payload.get("title"))
        .saturating_add(batch_string_chars(payload.get("description")));
    if let Some(links) = payload.get("links").and_then(serde_json::Value::as_array) {
        for link in links {
            chars = chars
                .saturating_add(batch_string_chars(link.get("url")))
                .saturating_add(batch_string_chars(link.get("text")));
        }
    }
    if item_type == crate::core::batch_fetch::BatchFetchItemType::Repo {
        if let Some(locator) = payload
            .get("locator")
            .and_then(serde_json::Value::as_object)
        {
            for value in locator.values() {
                chars = chars.saturating_add(batch_string_chars(Some(value)));
            }
        }
        for key in [
            "browser_url",
            "raw_url",
            "permalink_url",
            "raw_permalink_url",
            "fetched_url",
        ] {
            chars = chars.saturating_add(batch_string_chars(payload.get(key)));
        }
    }
    chars
}

pub(crate) fn batch_returned_chars(
    payload: &serde_json::Value,
    item_type: crate::core::batch_fetch::BatchFetchItemType,
) -> usize {
    batch_string_chars(payload.get("text"))
        .saturating_add(batch_payload_metadata_chars(payload, item_type))
}

fn safe_batch_item_label(label: &str) -> String {
    let (label, _) = crate::core::sanitize::strip_control_chars(label);
    let (label, _) = crate::core::sanitize::bound_text(&label, 200);
    label
}

fn batch_total_budget_warning(
    label: &str,
    outcome: &str,
    remaining: usize,
    total_cap: usize,
) -> String {
    format!(
        "batch_total_budget_exhausted: item {} {outcome}; remaining budget={remaining}; max_total_chars={total_cap}",
        safe_batch_item_label(label)
    )
}

fn batch_payload_budget_warning(
    result: &crate::core::batch_fetch::BatchFetchResult,
    remaining: usize,
    total_cap: usize,
) -> String {
    format!(
        "batch_payload_budget_degraded: item {} dropped {}; payload_chars={}; remaining budget={remaining}; max_total_chars={total_cap}",
        safe_batch_item_label(&result.label),
        result.degraded_fields.join(","),
        result.payload_chars,
    )
}

fn omit_batch_result_for_budget(
    mut result: crate::core::batch_fetch::BatchFetchResult,
    remaining: usize,
    total_cap: usize,
) -> crate::core::batch_fetch::BatchFetchResult {
    let label = safe_batch_item_label(&result.label);
    result.ok = false;
    result.response = Some(serde_json::Value::Null);
    result.error = Some(format!(
        "batch_total_budget_exhausted: item {label} omitted because its locator and metadata exceed remaining budget of {remaining} (max_total_chars={total_cap})"
    ));
    result.chars_returned = 0;
    result.payload_chars = 0;
    result.degraded_fields.clear();
    result.truncated = true;
    result
}
