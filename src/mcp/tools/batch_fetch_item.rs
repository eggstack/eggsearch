use super::batch_fetch::{batch_payload_chars, inject_repo_focus_into_payload};
use super::common::{ImageAttachment, ToolError};
use super::repo_fetch::{run_repo_fetch_with_host_scope, RepoFetchArgs};
use crate::fetch::FetchClient;
use crate::mcp::state::ServerState;
use std::sync::Arc;

pub(crate) struct BatchFetchItemContext<'a> {
    pub(super) index: usize,
    pub(super) item: &'a crate::core::batch_fetch::BatchFetchItem,
    pub(super) item_max_chars: usize,
    pub(super) state: Arc<ServerState>,
    pub(super) client: Arc<FetchClient>,
    pub(super) semaphore: Arc<tokio::sync::Semaphore>,
    pub(super) label: String,
    pub(super) timeout_ms: Option<u64>,
    pub(super) host_local_scope: Option<serde_json::Value>,
}

type BatchFetchItemFuture = std::pin::Pin<
    Box<
        dyn std::future::Future<
                Output = Result<
                    (
                        crate::core::batch_fetch::BatchFetchResult,
                        Option<ImageAttachment>,
                    ),
                    ToolError,
                >,
            > + Send,
    >,
>;

pub(crate) fn make_batch_fetch_future(context: BatchFetchItemContext<'_>) -> BatchFetchItemFuture {
    use crate::core::batch_fetch::{BatchFetchItem, BatchFetchItemType, BatchFetchResult};
    use crate::core::identity::batch_fetch_id;

    let BatchFetchItemContext {
        index: i,
        item,
        item_max_chars,
        state,
        client,
        semaphore,
        label,
        timeout_ms,
        host_local_scope,
    } = context;

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
            pdf,
            render,
            browser_profile,
            include_screenshot,
        } => {
            let stable_id = batch_fetch_id(&label, i);
            let effective_max = max_chars
                .unwrap_or(item_max_chars)
                .min(item_max_chars)
                .max(1);
            let web_args = super::web_fetch::WebFetchArgs {
                url: url.clone(),
                max_chars: Some(effective_max),
                timeout_ms: None,
                extract_mode: *extract_mode,
                include_links: *include_links,
                pdf: pdf.clone(),
                cache_policy: *cache_policy,
                max_cache_age_seconds: *max_cache_age_seconds,
                focus: focus.clone(),
                focus_max_chunks: *focus_max_chunks,
                focus_max_chars: *focus_max_chars,
                render: render.clone(),
                browser_profile: browser_profile.clone(),
                include_screenshot: *include_screenshot,
                offset: None,
                limit: None,
                response_detail: Some(crate::mcp::projection::ResponseDetail::Diagnostic),
            };
            let budget_ms = timeout_ms
                .filter(|ms| *ms < state.config.fetch.timeout_ms)
                .unwrap_or(state.config.fetch.timeout_ms);
            Box::pin(async move {
                let _permit = semaphore
                    .acquire_owned()
                    .await
                    .map_err(|e| ToolError::internal(format!("semaphore closed: {e}")))?;
                match super::web_fetch::run_web_fetch_with_prepared_client(
                    state, web_args, client, budget_ms,
                )
                .await
                {
                    Ok(outcome) => {
                        let image = outcome.images.into_iter().next();
                        let payload = outcome.value;
                        let body_chars = payload
                            .get("document")
                            .and_then(|document| document.get("text_chars_returned"))
                            .and_then(serde_json::Value::as_u64)
                            .map(|chars| chars as usize)
                            .unwrap_or_else(|| {
                                payload
                                    .get("text")
                                    .and_then(serde_json::Value::as_str)
                                    .map(str::chars)
                                    .map(Iterator::count)
                                    .unwrap_or(0)
                            });
                        let meta_chars = payload
                            .get("title")
                            .and_then(serde_json::Value::as_str)
                            .map(str::chars)
                            .map(Iterator::count)
                            .unwrap_or(0)
                            + payload
                                .get("description")
                                .and_then(serde_json::Value::as_str)
                                .map(str::chars)
                                .map(Iterator::count)
                                .unwrap_or(0)
                            + payload
                                .get("links")
                                .and_then(serde_json::Value::as_array)
                                .map(|links| {
                                    links
                                        .iter()
                                        .map(|link| {
                                            link.get("url")
                                                .and_then(serde_json::Value::as_str)
                                                .map(str::chars)
                                                .map(Iterator::count)
                                                .unwrap_or(0)
                                                + link
                                                    .get("text")
                                                    .and_then(serde_json::Value::as_str)
                                                    .map(str::chars)
                                                    .map(Iterator::count)
                                                    .unwrap_or(0)
                                        })
                                        .sum::<usize>()
                                })
                                .unwrap_or(0);
                        let truncated = payload
                            .get("truncated")
                            .and_then(serde_json::Value::as_bool)
                            .unwrap_or(false);
                        Ok((
                            BatchFetchResult {
                                index: i,
                                item_type: BatchFetchItemType::Web,
                                label,
                                stable_id: Some(stable_id),
                                ok: true,
                                response: Some(payload),
                                error: None,
                                chars_returned: body_chars + meta_chars,
                                truncated,
                            },
                            image,
                        ))
                    }
                    Err(error) => {
                        let error_payload = error.error_payload();
                        let code = error_payload
                            .get("code")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or("fetch_failed");
                        Ok((
                            BatchFetchResult {
                                index: i,
                                item_type: BatchFetchItemType::Web,
                                label,
                                stable_id: Some(stable_id),
                                ok: false,
                                response: None,
                                error: Some(format!("{code}: {error}")),
                                chars_returned: 0,
                                truncated: false,
                            },
                            None,
                        ))
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
            symbol,
            symbol_kind,
            match_text,
            expand_to_block,
            max_block_lines,
            prefer_local,
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
                symbol: symbol.clone(),
                symbol_kind: symbol_kind.clone(),
                match_text: match_text.clone(),
                expand_to_block: *expand_to_block,
                max_block_lines: *max_block_lines,
                prefer_local: *prefer_local,
                response_detail: None,
            };
            Box::pin(async move {
                let ok_label = label.clone();
                let _permit = semaphore
                    .acquire_owned()
                    .await
                    .map_err(|e| ToolError::internal(format!("semaphore closed: {e}")))?;
                match run_repo_fetch_with_host_scope(state, repo_args, host_local_scope).await {
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
                        Ok((
                            BatchFetchResult {
                                index: i,
                                item_type: BatchFetchItemType::Repo,
                                label: ok_label,
                                stable_id: Some(stable_id),
                                ok: true,
                                response: Some(payload),
                                error: None,
                                chars_returned: text_len,
                                truncated,
                            },
                            None,
                        ))
                    }
                    Err(error) => Ok((
                        BatchFetchResult {
                            index: i,
                            item_type: BatchFetchItemType::Repo,
                            label,
                            stable_id: Some(stable_id),
                            ok: false,
                            response: None,
                            error: Some(error.to_string()),
                            chars_returned: 0,
                            truncated: false,
                        },
                        None,
                    )),
                }
            })
        }
    }
}
