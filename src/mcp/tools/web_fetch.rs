use super::common::*;
use crate::fetch::FetchClient;
use crate::mcp::policy::{fetch_allowed, web_fetch_denied_message, Policy};
use crate::mcp::state::ServerState;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct WebFetchArgs {
    /// The URL to fetch. Must be a valid HTTP(S) URL.
    pub url: String,
    /// Maximum characters to extract. Defaults to server config.
    #[serde(default)]
    pub max_chars: Option<usize>,
    /// Timeout in milliseconds. Defaults to server config.
    #[serde(default)]
    pub timeout_ms: Option<u64>,
    /// Extraction mode: "text" (default), "markdown", or "metadata_only".
    #[serde(default)]
    pub extract_mode: Option<crate::core::fetch::ExtractMode>,
    /// Whether to include extracted links. Defaults to the server's
    /// `[fetch].include_links_default` config value when omitted.
    #[serde(default)]
    pub include_links: Option<bool>,
    /// PDF-specific options. Only applies when fetching a PDF document.
    #[serde(default)]
    pub pdf: Option<crate::core::fetch::PdfFetchOptions>,
    /// Cache policy: "default" (use cache), "bypass" (skip read),
    /// or "refresh" (revalidate even if fresh).
    #[serde(default)]
    pub cache_policy: Option<crate::core::fetch::FetchCachePolicy>,
    /// Caller maximum acceptable cache age in seconds. Only tightens
    /// origin freshness; `0` forces revalidation without disabling
    /// cache storage. Ignored when `cache_policy = "bypass"`.
    #[serde(default)]
    pub max_cache_age_seconds: Option<u64>,
    /// Optional focus query for deterministic query-focused chunk
    /// selection over the extracted document. No extra URL traversal.
    #[serde(default)]
    pub focus: Option<String>,
    /// Maximum focused chunks to return (1-5, default 5).
    #[serde(default)]
    pub focus_max_chunks: Option<usize>,
    /// Maximum focused characters to return. Defaults to the effective
    /// `max_chars` budget.
    #[serde(default)]
    pub focus_max_chars: Option<usize>,
    /// Render policy: "http_only" (default), "auto", or "browser".
    /// Controls whether fetch may escalate to headless browser rendering
    /// for JavaScript-heavy pages. Requires the `browser` feature.
    #[serde(default)]
    pub render: Option<String>,
    /// Named browser profile for persistent session reuse. When set,
    /// the fetch uses a profile-scoped Chrome context with persisted
    /// cookies and storage. The profile must exist and be allowed for
    /// the requested origin. Profile creation is a CLI-only operation
    /// (`eggsearch browser-login`). Omit for ephemeral browser context.
    /// Requires the `browser` feature.
    #[serde(default)]
    pub browser_profile: Option<String>,
}

/// Run the `web_fetch` tool.
pub async fn run_web_fetch(
    state: Arc<ServerState>,
    args: WebFetchArgs,
) -> Result<serde_json::Value, ToolError> {
    use crate::core::fetch::ExtractMode;
    use crate::fetch::cache::{
        build_raw_cache_key, build_raw_response_hash, should_cache_response, CacheScope,
        CacheStatus, FetchCacheMetadata,
    };
    use crate::fetch::origin::{classify_network_error, OriginKey};

    if matches!(fetch_allowed(state.config.fetch.enabled), Policy::Deny) {
        return Err(ToolError::Validation(web_fetch_denied_message()));
    }

    if args.url.trim().is_empty() {
        return Err(ToolError::Validation("url must not be empty".into()));
    }

    let trimmed_url = args.url.trim();
    let lower = trimmed_url.to_ascii_lowercase();
    if !lower.starts_with("http://") && !lower.starts_with("https://") {
        return Err(ToolError::Validation(format!(
            "url scheme must be http or https, got: {}",
            trimmed_url.chars().take(20).collect::<String>()
        )));
    }

    if let Some(0) = args.max_chars {
        return Err(ToolError::Validation("max_chars must be > 0".to_string()));
    }

    if let Some(0) = args.timeout_ms {
        return Err(ToolError::Validation("timeout_ms must be > 0".to_string()));
    }

    let extract_mode = args.extract_mode.unwrap_or(ExtractMode::Text);

    let base_client: Arc<FetchClient> = state.fetch_client().ok_or_else(|| {
        ToolError::internal("fetch client unavailable; is [fetch].enabled = true?".to_string())
    })?;

    let client =
        if let Some(ms) = args.timeout_ms {
            Arc::new(base_client.with_timeout_ms(ms).map_err(|e| {
                ToolError::internal(format!("failed to create timeout override: {e}"))
            })?)
        } else {
            base_client
        };

    let include_links = args
        .include_links
        .unwrap_or(state.config.fetch.include_links_default);

    let cache_policy = args.cache_policy.unwrap_or_default();

    if let Some(age) = args.max_cache_age_seconds {
        if age > crate::core::fetch::MAX_CACHE_AGE_SECONDS {
            return Err(ToolError::Validation(format!(
                "max_cache_age_seconds must be <= {}",
                crate::core::fetch::MAX_CACHE_AGE_SECONDS
            )));
        }
    }
    let focus_query = match args.focus.as_deref() {
        None => None,
        Some(q) if q.trim().is_empty() => {
            return Err(ToolError::Validation("focus must not be empty".to_string()));
        }
        Some(q) if q.chars().count() > crate::core::fetch::MAX_FOCUS_QUERY_CHARS => {
            return Err(ToolError::Validation(format!(
                "focus must be <= {} characters",
                crate::core::fetch::MAX_FOCUS_QUERY_CHARS
            )));
        }
        Some(q) => Some(q.trim().to_string()),
    };
    if let Some(0) = args.focus_max_chunks {
        return Err(ToolError::Validation(
            "focus_max_chunks must be > 0".to_string(),
        ));
    }
    if let Some(n) = args.focus_max_chunks {
        if n > crate::core::fetch::MAX_FOCUS_CHUNKS {
            return Err(ToolError::Validation(format!(
                "focus_max_chunks must be <= {}",
                crate::core::fetch::MAX_FOCUS_CHUNKS
            )));
        }
    }
    if let Some(0) = args.focus_max_chars {
        return Err(ToolError::Validation(
            "focus_max_chars must be > 0".to_string(),
        ));
    }
    if focus_query.is_some() && extract_mode == ExtractMode::MetadataOnly {
        return Err(ToolError::Validation(
            "focus requires extracted content; it is not valid with extract_mode = \"metadata_only\""
                .to_string(),
        ));
    }

    #[cfg(feature = "browser")]
    #[allow(unused_mut)]
    let mut used_profile_name: Option<String> = None;
    #[cfg(not(feature = "browser"))]
    let used_profile_name: Option<String> = None;
    #[cfg(feature = "browser")]
    #[allow(unused_mut)]
    let mut profile_cache_scope_id: Option<String> = None;
    #[cfg(not(feature = "browser"))]
    let profile_cache_scope_id: Option<String> = None;
    #[cfg(feature = "browser")]
    #[allow(unused_mut)]
    let mut profile_chrome_data_dir: Option<std::path::PathBuf> = None;
    #[cfg(not(feature = "browser"))]
    let _profile_chrome_data_dir: Option<std::path::PathBuf> = None;
    #[cfg(feature = "browser")]
    #[allow(unused_mut)]
    let mut _profile_lock: Option<crate::fetch::browser::ProfileLock> = None;
    #[cfg(not(feature = "browser"))]
    let _profile_lock: Option<()> = None;

    #[cfg(feature = "browser")]
    let render_policy_str = args
        .render
        .clone()
        .unwrap_or_else(|| "http_only".to_string());
    #[cfg(not(feature = "browser"))]
    let _render_policy_str = args
        .render
        .clone()
        .unwrap_or_else(|| "http_only".to_string());
    #[cfg(feature = "browser")]
    let mut browser_available = false;
    #[cfg(not(feature = "browser"))]
    let browser_available = false;

    #[cfg(feature = "browser")]
    let render_policy = {
        let rp: crate::fetch::browser::RenderPolicy = serde_json::from_value(
            serde_json::Value::String(render_policy_str.clone()),
        )
        .map_err(|e| {
            ToolError::Validation(format!("invalid render policy '{render_policy_str}': {e}"))
        })?;
        rp
    };

    #[cfg(feature = "browser")]
    {
        if matches!(render_policy, crate::fetch::browser::RenderPolicy::HttpOnly)
            && args.browser_profile.is_some()
        {
            return Err(ToolError::Validation(
                "browser_profile is not valid with render=http_only".to_string(),
            ));
        }

        if !matches!(render_policy, crate::fetch::browser::RenderPolicy::HttpOnly) {
            if state.browser_lifecycle().is_some() {
                browser_available = true;
            } else if matches!(render_policy, crate::fetch::browser::RenderPolicy::Browser) {
                return Err(browser_unavailable_error(
                    "browser rendering is enabled but no Chrome/Chromium executable was found; \
                     set [fetch.browser].executable or install Chrome/Chromium",
                ));
            }
        }

        if let Some(ref profile_name) = args.browser_profile {
            let mgr = state.profile_manager.as_ref().ok_or_else(|| {
                ToolError::Validation(
                    "browser profiles are not enabled; \
                     set [fetch.browser].persistent_profiles_enabled = true"
                        .to_string(),
                )
            })?;
            let parsed_url = url::Url::parse(trimmed_url)
                .map_err(|e| ToolError::Validation(format!("invalid URL: {e}")))?;
            let request_origin = format!("{}://{}", parsed_url.scheme(), parsed_url.authority());
            let meta = mgr
                .resolve_for_origin(profile_name, &request_origin)
                .map_err(|e| match e {
                    crate::fetch::browser::ProfileError::ProfileNotFound(msg) => {
                        ToolError::Validation(format!("browser_profile: {msg}"))
                    }
                    crate::fetch::browser::ProfileError::ProfilesDisabled => {
                        ToolError::Validation("browser profiles are not enabled".to_string())
                    }
                    other => ToolError::internal(format!("browser_profile: {other}")),
                })?;
            let lock = mgr.acquire_lock(&meta.id).map_err(|e| match e {
                crate::fetch::browser::ProfileError::ProfileBusy(name) => ToolError::Validation(
                    format!("browser_profile '{name}' is busy (locked by another process)"),
                ),
                other => ToolError::internal(format!("browser_profile lock: {other}")),
            })?;
            _profile_lock = Some(lock);
            profile_cache_scope_id = Some(meta.id.clone());
            used_profile_name = Some(meta.display_name.clone());
            profile_chrome_data_dir = Some(mgr.chrome_data_dir_for(&meta.id));
        }
    }

    let scope = if let Some(ref id) = profile_cache_scope_id {
        CacheScope::Profile(crate::fetch::cache::ProfileId::opaque(id.clone()))
    } else {
        CacheScope::Anonymous
    };

    let origin_key = OriginKey::from_url(
        &url::Url::parse(trimmed_url)
            .map_err(|e| ToolError::Validation(format!("invalid URL: {e}")))?,
    )
    .ok_or_else(|| ToolError::Validation("URL must be http or https".into()))?;

    let mut metadata = FetchCacheMetadata::default();

    let pdf_pages = args.pdf.as_ref().and_then(|p| p.pages.as_deref());
    let pdf_ocr = args
        .pdf
        .as_ref()
        .and_then(|p| p.pdf_ocr.as_ref())
        .map(|o| format!("{o:?}"));
    let include_media = args
        .pdf
        .as_ref()
        .and_then(|p| p.include_media.unwrap_or(false).then_some(true))
        .unwrap_or(false);
    let requested_max_chars = args
        .max_chars
        .unwrap_or(state.config.fetch.max_chars_default)
        .min(state.config.fetch.max_chars_cap);

    let caller_max_age = args
        .max_cache_age_seconds
        .map(std::time::Duration::from_secs);
    let entry_satisfies_caller_max_age = |entry: &crate::fetch::cache::RawFetchCacheEntry| -> bool {
        let Some(max) = caller_max_age else {
            return true;
        };
        let age = std::time::SystemTime::now()
            .duration_since(entry.fetched_at)
            .unwrap_or(std::time::Duration::ZERO);
        age <= max
    };

    let mut cached_response: Option<crate::core::fetch::WebFetchResponse> = None;
    if cache_policy != crate::core::fetch::FetchCachePolicy::Bypass {
        if let Some(ref cache) = state.fetch_cache {
            let raw_key = build_raw_cache_key(trimmed_url, &scope);
            if let Some(raw_entry) = cache.get_raw(&raw_key).await {
                let browser_cache_allowed = {
                    #[cfg(feature = "browser")]
                    {
                        match render_policy {
                            crate::fetch::browser::RenderPolicy::HttpOnly => matches!(
                                raw_entry.representation,
                                crate::fetch::cache::RawRepresentation::Http
                            ),
                            crate::fetch::browser::RenderPolicy::Browser => matches!(
                                raw_entry.representation,
                                crate::fetch::cache::RawRepresentation::BrowserDom
                            ),
                            crate::fetch::browser::RenderPolicy::Auto => true,
                        }
                    }
                    #[cfg(not(feature = "browser"))]
                    {
                        true
                    }
                };
                if browser_cache_allowed {
                    let raw_hash = build_raw_response_hash(&raw_entry.body);
                    let derived_key = crate::fetch::cache::build_derived_key(
                        &scope,
                        raw_hash,
                        extract_mode,
                        requested_max_chars,
                        include_links,
                        pdf_pages,
                        pdf_ocr.as_deref(),
                        include_media,
                        state.config.fetch.sanitize_output,
                    );
                    let derive = || async {
                        let mut derived = client.derive_from_raw(
                            trimmed_url,
                            raw_entry.final_url.clone(),
                            raw_entry.status,
                            raw_entry.content_type.clone(),
                            raw_entry.headers.clone(),
                            raw_entry.content_length_header,
                            raw_entry.redirect_count,
                            raw_entry.body.to_vec(),
                            raw_entry.truncated,
                            requested_max_chars,
                            state.config.fetch.max_chars_cap,
                            extract_mode,
                            include_links,
                            args.pdf.as_ref(),
                            None,
                        )?;
                        derived.transport = Some(
                            match raw_entry.representation {
                                crate::fetch::cache::RawRepresentation::Http => "http",
                                crate::fetch::cache::RawRepresentation::BrowserDom => "browser",
                            }
                            .to_string(),
                        );
                        derived.browser_escalated = raw_entry.browser_escalated;
                        cache
                            .insert_derived(
                                derived_key.clone(),
                                derived_cache_entry(raw_hash, &derived_key, &derived),
                            )
                            .await;
                        Ok::<_, crate::fetch::FetchError>(derived)
                    };
                    let locally_fresh = raw_entry.freshness.is_fresh()
                        && entry_satisfies_caller_max_age(&raw_entry);
                    let force_revalidation =
                        cache_policy == crate::core::fetch::FetchCachePolicy::Refresh;
                    if locally_fresh && !force_revalidation {
                        metadata.cache_status = CacheStatus::Hit;
                        cached_response =
                            if let Some(derived) = cache.get_derived(&derived_key).await {
                                Some(cached_document_response(
                                    trimmed_url,
                                    &raw_entry,
                                    &derived.response,
                                ))
                            } else {
                                derive().await.ok()
                            };
                    } else if matches!(
                        raw_entry.representation,
                        crate::fetch::cache::RawRepresentation::Http
                    ) && !raw_entry.freshness.no_store
                        && !raw_entry.freshness.no_cache
                        && (raw_entry.validators.etag.is_some()
                            || raw_entry.validators.last_modified.is_some())
                    {
                        let circuit_blocked = if let Some(ref ctrl) = state.origin_controller {
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
                                if let Ok((status, reval_headers, _, _)) =
                                    client.fetch_conditional(trimmed_url, &conditional).await
                                {
                                    if status == 304 {
                                        metadata.cache_status = CacheStatus::Revalidated;
                                        let mut updated_freshness = raw_entry.freshness.clone();
                                        let mut updated_validators = raw_entry.validators.clone();
                                        crate::fetch::cache::apply_304_headers(
                                            &mut updated_freshness,
                                            &mut updated_validators,
                                            &reval_headers,
                                        );
                                        updated_freshness.fetched_at =
                                            Some(std::time::SystemTime::now());
                                        cache
                                            .insert_raw(
                                                raw_key,
                                                crate::fetch::cache::RawFetchCacheEntry {
                                                    freshness: updated_freshness,
                                                    validators: updated_validators,
                                                    ..raw_entry.clone()
                                                },
                                            )
                                            .await;
                                        cached_response = if let Some(derived) =
                                            cache.get_derived(&derived_key).await
                                        {
                                            let mut resp = cached_document_response(
                                                trimmed_url,
                                                &raw_entry,
                                                &derived.response,
                                            );
                                            resp.cache_status =
                                                crate::fetch::cache::CacheStatus::Revalidated;
                                            Some(resp)
                                        } else {
                                            derive().await.ok()
                                        };
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
    let mut response = cached_response;
    let mut attempt_count: usize = if response.is_some() { 1 } else { 0 };
    let mut retry_after_ms: Option<u64> = None;
    let deadline =
        std::time::Instant::now() + std::time::Duration::from_millis(state.config.fetch.timeout_ms);

    #[cfg(feature = "browser")]
    let browser_direct = matches!(render_policy, crate::fetch::browser::RenderPolicy::Browser);
    #[cfg(not(feature = "browser"))]
    let browser_direct = false;

    let ran_browser_direct = if browser_direct && browser_available && response.is_none() {
        #[cfg(feature = "browser")]
        {
            if state.browser_lifecycle().is_some() {
                let remaining = deadline.saturating_duration_since(std::time::Instant::now());
                if remaining.as_millis() < 2000 {
                    return Err(browser_deadline_exceeded_error());
                }
                match tokio::time::timeout(
                    remaining,
                    run_browser_fetch(
                        state.as_ref(),
                        trimmed_url,
                        state.config.fetch.sanitize_output,
                        &render_policy,
                        profile_chrome_data_dir.as_deref(),
                    ),
                )
                .await
                {
                    Ok(Ok(result)) => {
                        let resp = crate::fetch::browser::browser_result_to_response(
                            result,
                            trimmed_url,
                            args.max_chars,
                            extract_mode,
                            include_links,
                            state.config.fetch.sanitize_output,
                        );
                        attempt_count = 1;
                        response = Some(resp);
                        true
                    }
                    Ok(Err(crate::fetch::browser::BrowserFetchError::InteractiveChallenge(
                        mir,
                    ))) => {
                        let next_action = used_profile_name
                            .as_deref()
                            .map(|pn| {
                                format!("eggsearch browser-login {} --profile {}", mir.origin, pn)
                            })
                            .or_else(|| {
                                Some(format!(
                                    "eggsearch browser-login {} --profile <name>",
                                    mir.origin
                                ))
                            });
                        return Err(browser_manual_interaction_error(
                            &mir.origin,
                            &mir.message,
                            used_profile_name.as_deref(),
                            next_action.as_deref(),
                        ));
                    }
                    Ok(Err(e)) => {
                        return Err(ToolError::internal(format!("browser_fetch_failed: {e}")));
                    }
                    Err(_) => {
                        return Err(ToolError::internal(
                            "browser rendering timed out".to_string(),
                        ));
                    }
                }
            } else {
                false
            }
        }
        #[cfg(not(feature = "browser"))]
        {
            false
        }
    } else {
        false
    };

    if !ran_browser_direct && response.is_none() {
        for attempt in 0..max_attempts {
            attempt_count = attempt + 1;

            let _permit = if let Some(ref controller) = state.origin_controller {
                match controller.acquire(&origin_key).await {
                    Ok(permit) => Some(permit),
                    Err(e) => {
                        return Err(ToolError::internal(format!("origin_backoff: {e}")));
                    }
                }
            } else {
                None
            };

            match client
                .fetch(
                    trimmed_url,
                    args.max_chars,
                    extract_mode,
                    include_links,
                    args.pdf.as_ref(),
                )
                .await
            {
                Ok(resp) => {
                    if let Some(ref ctrl) = state.origin_controller {
                        ctrl.record_success(&origin_key).await;
                    }

                    #[cfg(feature = "browser")]
                    let mut escalated = false;
                    #[cfg(not(feature = "browser"))]
                    let escalated = false;

                    #[cfg(feature = "browser")]
                    if browser_available
                        && matches!(render_policy, crate::fetch::browser::RenderPolicy::Auto)
                    {
                        let body_bytes = resp.raw_text.as_deref().unwrap_or("").as_bytes();
                        let text_len = body_bytes.len();
                        let title_str = resp.title.as_deref().unwrap_or("");
                        let classification = crate::fetch::browser::classify_response(
                            resp.status,
                            resp.content_type.as_deref(),
                            Some(title_str),
                            text_len,
                            body_bytes,
                        );
                        let should_escalate = matches!(
                            classification,
                            crate::fetch::browser::FetchDisposition::JavascriptShell
                                | crate::fetch::browser::FetchDisposition::NonInteractiveVerification
                        );
                        if should_escalate {
                            let remaining =
                                deadline.saturating_duration_since(std::time::Instant::now());
                            if remaining.as_millis() >= 2000 && state.browser_lifecycle().is_some()
                            {
                                match tokio::time::timeout(
                                        remaining,
                                        run_browser_fetch(
                                            state.as_ref(),
                                            trimmed_url,
                                            state.config.fetch.sanitize_output,
                                            &render_policy,
                                            profile_chrome_data_dir.as_deref(),
                                        ),
                                    )
                                    .await
                                    {
                                        Ok(Ok(result)) => {
                                            let mut browser_resp =
                                                crate::fetch::browser::browser_result_to_response(
                                                    result,
                                                    trimmed_url,
                                                    args.max_chars,
                                                    extract_mode,
                                                    include_links,
                                                    state.config.fetch.sanitize_output,
                                                );
                                            browser_resp.browser_escalated = true;
                                            response = Some(browser_resp);
                                            escalated = true;
                                        }
                                        Ok(Err(
                                            crate::fetch::browser::BrowserFetchError::InteractiveChallenge(
                                                mir,
                                            ),
                                        )) => {
                                            let next_action = used_profile_name.as_deref().map(|pn| {
                                                format!(
                                                    "eggsearch browser-login {} --profile {}",
                                                    mir.origin, pn
                                                )
                                            }).or_else(|| Some(format!(
                                                "eggsearch browser-login {} --profile <name>",
                                                mir.origin
                                            )));
                                            return Err(browser_manual_interaction_error(
                                                &mir.origin,
                                                &mir.message,
                                                used_profile_name.as_deref(),
                                                next_action.as_deref(),
                                            ));
                                        }
                                        Ok(Err(_)) | Err(_) => {}
                                    }
                            }
                        }
                    }

                    if !escalated {
                        response = Some(resp);
                    }
                    break;
                }
                Err(e) => {
                    let kind = e.kind();
                    let class = match &e {
                        crate::fetch::FetchError::HttpStatus(status, _) => {
                            crate::fetch::origin::classify_http_status(*status)
                        }
                        _ => classify_network_error(&e.to_string()),
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
                                return Err(ToolError::internal(format!(
                                    "origin_circuit_open: {e}, retry in {delay_ms}ms"
                                )));
                            }
                            crate::fetch::origin::OriginBackoffDecision::Backoff {
                                delay_ms,
                                retry_after_ms: ra,
                                ..
                            } if is_retryable && attempt + 1 < max_attempts => {
                                retry_after_ms = ra;
                                drop(_permit);
                                let remaining =
                                    deadline.saturating_duration_since(std::time::Instant::now());
                                let sleep_dur =
                                    std::time::Duration::from_millis(
                                        delay_ms
                                            .min(state.config.fetch.timeout_ms / 2)
                                            .min(remaining.as_millis().min(u128::from(u64::MAX))
                                                as u64),
                                    );
                                if !sleep_dur.is_zero() {
                                    tokio::time::sleep(sleep_dur).await;
                                }
                                continue;
                            }
                            crate::fetch::origin::OriginBackoffDecision::Backoff { .. } => {}
                            _ => {}
                        }
                    }

                    last_err = Some(e);
                    break;
                }
            }
        }
    }

    let resp: crate::core::fetch::WebFetchResponse = match response {
        Some(r) => r,
        None => {
            let err = last_err.unwrap_or(crate::fetch::FetchError::Unknown(
                "fetch failed after all attempts".into(),
            ));
            if matches!(
                err,
                crate::fetch::FetchError::BrowserInteractiveChallenge(_)
            ) {
                let parsed_url = url::Url::parse(trimmed_url).ok();
                let origin = parsed_url
                    .as_ref()
                    .map(|u| format!("{}://{}", u.scheme(), u.authority()))
                    .unwrap_or_else(|| "<origin>".to_string());
                #[cfg(feature = "browser")]
                if let Some(ref pn) = used_profile_name {
                    return Err(browser_profile_requires_attention_error(&origin, pn));
                }
                let next_action = format!("eggsearch browser-login {origin} --profile <name>");
                let data = serde_json::json!({
                    "code": "browser_profile_requires_attention",
                    "message": format!(
                        "browser profile requires manual login for origin {origin}"
                    ),
                    "origin": origin,
                    "manual_interaction_required": true,
                    "next_action": next_action,
                });
                return Err(ToolError::internal_with_data(
                    format!(
                        "browser_profile_requires_attention: profile requires manual login for {origin}; \
                         reopen with: {next_action}"
                    ),
                    data,
                ));
            }
            return Err(ToolError::internal(format!(
                "{}: {}",
                err.error_code(),
                err
            )));
        }
    };

    metadata.attempt_count = attempt_count;
    metadata.retry_after_ms = retry_after_ms;

    if cache_policy == crate::core::fetch::FetchCachePolicy::Bypass {
        metadata.cache_status = CacheStatus::Bypassed;
    } else if metadata.cache_status == CacheStatus::default() {
        let default_freshness = crate::fetch::cache::CacheFreshness::default();
        metadata.cache_status = if should_cache_response(
            resp.status,
            resp.content_type.as_deref(),
            &default_freshness,
            &scope,
        ) {
            CacheStatus::Miss
        } else {
            CacheStatus::NotCacheable
        };
    }

    if metadata.cache_status == CacheStatus::Miss || metadata.cache_status == CacheStatus::Bypassed
    {
        if let Some(ref cache) = state.fetch_cache {
            let raw_key = build_raw_cache_key(trimmed_url, &scope);
            let raw_body_bytes = resp.raw_body.as_deref().unwrap_or(&[]);
            let raw_hash = build_raw_response_hash(raw_body_bytes);

            let (mut cache_freshness, validators) = if let Some(ref headers) = resp.response_headers
            {
                let header_map: reqwest::header::HeaderMap = headers
                    .iter()
                    .filter_map(|(k, v)| {
                        let name = reqwest::header::HeaderName::from_bytes(k.as_bytes()).ok()?;
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
            cache_freshness.fetched_at = Some(std::time::SystemTime::now());
            if cache_freshness.max_age.is_none() && cache_freshness.expires.is_none() {
                let ttl =
                    std::time::Duration::from_secs(state.config.fetch.cache.default_ttl_seconds);
                cache_freshness.max_age = Some(ttl);
            }
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
                    representation: if resp.transport.as_deref() == Some("browser") {
                        crate::fetch::cache::RawRepresentation::BrowserDom
                    } else {
                        crate::fetch::cache::RawRepresentation::Http
                    },
                    truncated: resp.truncated,
                    browser_escalated: resp.browser_escalated,
                };
                cache.insert_raw(raw_key, raw_entry).await;

                let derived_key = crate::fetch::cache::build_derived_key(
                    &scope,
                    raw_hash,
                    extract_mode,
                    requested_max_chars,
                    include_links,
                    pdf_pages,
                    pdf_ocr.as_deref(),
                    include_media,
                    state.config.fetch.sanitize_output,
                );
                cache
                    .insert_derived(
                        derived_key.clone(),
                        derived_cache_entry(raw_hash, &derived_key, &resp),
                    )
                    .await;
            } else {
                metadata.cache_status = CacheStatus::NotCacheable;
            }
        }
    }

    let mut structured = crate::core::warning::convert_fetch_warnings(&resp.warnings);
    if resp.links_truncated {
        structured.push(crate::core::warning::AgentWarning::new(
            crate::core::warning::WarningCode::FetchLinksTruncated,
            "link list was truncated; not all links are included".to_string(),
        ));
    }
    let focus_selection = match (&focus_query, &resp.document) {
        (Some(query), Some(document)) if resp.fetched => {
            let max_chunks = args
                .focus_max_chunks
                .unwrap_or(crate::core::fetch::MAX_FOCUS_CHUNKS)
                .clamp(1, crate::core::fetch::MAX_FOCUS_CHUNKS);
            let max_chars = args
                .focus_max_chars
                .unwrap_or(requested_max_chars)
                .min(state.config.fetch.max_chars_cap)
                .max(1);
            Some(crate::core::focus::select_focus_chunks(
                document, query, max_chunks, max_chars,
            ))
        }
        _ => None,
    };
    let payload = serde_json::json!({
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
        "focus": focus_selection,
        "fetch_transform": resp.fetch_transform,
        "structured_warnings": structured,
        "cache_status": serde_json::to_value(metadata.cache_status).unwrap_or(serde_json::json!("miss")),
        "attempt_count": metadata.attempt_count,
        "retry_after_ms": metadata.retry_after_ms,
        "origin_backoff_ms": metadata.origin_backoff_ms,
        "browser_profile": used_profile_name,
        "browser_profile_scope": if used_profile_name.is_some() { "persistent" } else { "ephemeral" },
        "manual_interaction_required": false,
        "transport": resp.transport.as_deref().unwrap_or("http"),
        "browser_escalated": resp.browser_escalated,
    });
    Ok(payload)
}
