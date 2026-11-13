use super::batch_fetch::{
    batch_payload_chars, inject_repo_focus_into_payload, inject_web_focus_into_payload,
};
use super::common::{derived_cache_entry, ToolError};
use super::repo_fetch::{run_repo_fetch, RepoFetchArgs};
use crate::fetch::FetchClient;
use crate::mcp::state::ServerState;
use std::sync::Arc;

/// Build a boxed future that fetches a single batch item.
///
/// The future acquires a semaphore permit, executes the fetch, and
/// returns a `BatchFetchResult`. Extracted so both concurrent-wave
/// and sequential-mode paths can share the same fetch logic.
#[allow(clippy::too_many_arguments)]
pub(crate) fn make_batch_fetch_future(
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
            let budgets = crate::fetch::budget::FetchBudgets::new(max_chars_cap, em);
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
                let web_client = client;

                use crate::fetch::accept::select_accept_policy;
                use crate::fetch::cache::{
                    build_raw_cache_key, build_raw_response_hash, should_cache_response, CacheScope,
                };
                use crate::fetch::origin::OriginKey;
                let accept_policy = select_accept_policy(&url, mode);
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
                        let raw_key = build_raw_cache_key(&url, &scope, accept_policy);
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
                                    crate::fetch::cache::ExtractionBudgetClass::for_fetch(
                                        raw_entry.content_type.as_deref(),
                                        budgets.document_chars(),
                                        budgets.window_chars(),
                                    ),
                                    il,
                                    None,
                                    None,
                                    false,
                                    state.config.fetch.sanitize_output,
                                );
                                if let Some(mut derived) =
                                    cache.get_derived_shared(&derived_key).await
                                {
                                    super::common::project_cached_document(
                                        &mut std::sync::Arc::make_mut(&mut derived).response,
                                        &raw_entry.final_url,
                                        budgets.window_chars(),
                                        state.config.fetch.sanitize_output,
                                    );
                                    let raw_payload = serde_json::json!({
                                        "url": url,
                                        "final_url": raw_entry.final_url,
                                        "title": derived.response.title,
                                        "description": derived.response.description,
                                        "content_type": raw_entry.content_type,
                                        "representation": derived.response.representation,
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
                                        derived.response.document.as_ref(),
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
                                        if let Ok((status, _headers, _body, _)) = web_client
                                            .fetch_conditional(
                                                &url,
                                                &conditional,
                                                raw_key.accept_policy,
                                            )
                                            .await
                                        {
                                            if status == 304 {
                                                let derived_key =
                                                    crate::fetch::cache::build_derived_key(
                                                        &scope,
                                                        build_raw_response_hash(&raw_entry.body),
                                                        mode,
                                                        crate::fetch::cache::ExtractionBudgetClass::for_fetch(
                                                            raw_entry.content_type.as_deref(),
                                                            budgets.document_chars(),
                                                            budgets.window_chars(),
                                                        ),
                                                        il,
                                                        None,
                                                        None,
                                                        false,
                                                        state.config.fetch.sanitize_output,
                                                    );
                                                if let Some(mut derived) =
                                                    cache.get_derived_shared(&derived_key).await
                                                {
                                                    super::common::project_cached_document(
                                                        &mut std::sync::Arc::make_mut(&mut derived)
                                                            .response,
                                                        &raw_entry.final_url,
                                                        budgets.window_chars(),
                                                        state.config.fetch.sanitize_output,
                                                    );
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
                                                        "representation": derived.response.representation,
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
                                                        derived.response.document.as_ref(),
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
                            let raw_key = build_raw_cache_key(
                                &url,
                                &scope,
                                resp.accept_policy.unwrap_or(accept_policy),
                            );
                            let raw_body_bytes = resp.raw_body.as_deref().unwrap_or(&[]);
                            let raw_hash = build_raw_response_hash(raw_body_bytes);

                            let (mut cache_freshness, validators) = if let Some(ref headers) =
                                resp.response_headers
                            {
                                let header_map: http::header::HeaderMap = headers
                                    .iter()
                                    .filter_map(|(k, v)| {
                                        let name =
                                            http::header::HeaderName::from_bytes(k.as_bytes())
                                                .ok()?;
                                        let val = http::header::HeaderValue::from_str(v).ok()?;
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
                                    crate::fetch::cache::ExtractionBudgetClass::for_fetch(
                                        resp.content_type.as_deref(),
                                        budgets.document_chars(),
                                        budgets.window_chars(),
                                    ),
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
                        let mut structured = structured;
                        structured.extend(resp.structured_warnings.iter().cloned());
                        let raw_payload = serde_json::json!({
                            "url": resp.url,
                            "final_url": resp.final_url,
                            "stable_id": resp.stable_id,
                            "source_id": resp.source_id,
                            "title": resp.title,
                            "description": resp.description,
                            "content_type": resp.content_type,
                            "representation": resp.representation,
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
                            "attempt_count": attempts_made
                                + resp.attempt_count.unwrap_or(1).saturating_sub(1),
                            "retry_after_ms": retry_after_ms,
                            "origin_backoff_ms": serde_json::Value::Null,
                            "browser_profile": serde_json::Value::Null,
                            "browser_profile_scope": "ephemeral",
                            "manual_interaction_required": resp.manual_interaction_required,
                            "transport": resp.transport.as_deref().unwrap_or("http"),
                            "browser_escalated": resp.browser_escalated,
                        });
                        let payload = inject_web_focus_into_payload(
                            raw_payload,
                            focus_query.as_deref(),
                            focus_chunks,
                            focus_chars,
                            resp.document.as_ref(),
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
                response_detail: None,
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
