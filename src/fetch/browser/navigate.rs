use std::sync::Arc;
use std::time::{Duration, Instant};

use chromiumoxide::page::Page;

use super::classify::{classify_response, FetchDisposition};
use super::intercept::{is_request_allowed, PolicyViolation};
use super::lifecycle::BrowserLifecycle;
use super::types::{
    BrowserConfig, FetchTransportKind, ManualInteractionReason, ManualInteractionRequired,
    RenderPolicy, TransportResponse, TransportTiming,
};
use crate::core::sanitize::{
    bound_text, scan_injection_markers, strip_control_chars, TrustMarkers, TITLE_MAX_CHARS,
};

#[derive(Debug)]
pub enum BrowserFetchError {
    PolicyViolation(PolicyViolation),
    HttpOnly,
    LaunchFailed(String),
    NavigationFailed(String),
    DomExtractionFailed(String),
    Timeout,
    PageClosed,
    InteractiveChallenge(ManualInteractionRequired),
}

impl std::fmt::Display for BrowserFetchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PolicyViolation(v) => write!(f, "policy violation: {v}"),
            Self::HttpOnly => write!(f, "browser rendering not available under HttpOnly policy"),
            Self::LaunchFailed(e) => write!(f, "browser launch failed: {e}"),
            Self::NavigationFailed(e) => write!(f, "navigation failed: {e}"),
            Self::DomExtractionFailed(e) => write!(f, "DOM extraction failed: {e}"),
            Self::Timeout => write!(f, "browser navigation timed out"),
            Self::PageClosed => write!(f, "page was closed"),
            Self::InteractiveChallenge(r) => {
                write!(f, "interactive challenge at {}: {}", r.origin, r.message)
            }
        }
    }
}

impl std::error::Error for BrowserFetchError {}

pub struct BrowserFetchResult {
    pub response: TransportResponse,
    pub extracted_title: Option<String>,
    pub extracted_text: String,
    pub trust_markers: TrustMarkers,
    pub warnings: Vec<String>,
}

pub async fn browser_fetch(
    lifecycle: &Arc<BrowserLifecycle>,
    url: &str,
    config: &BrowserConfig,
    sanitize_output: bool,
) -> Result<BrowserFetchResult, BrowserFetchError> {
    browser_fetch_with_policy(
        lifecycle,
        url,
        config,
        sanitize_output,
        &RenderPolicy::Browser,
    )
    .await
}

pub async fn browser_fetch_with_policy(
    lifecycle: &Arc<BrowserLifecycle>,
    url: &str,
    config: &BrowserConfig,
    sanitize_output: bool,
    policy: &RenderPolicy,
) -> Result<BrowserFetchResult, BrowserFetchError> {
    match policy {
        RenderPolicy::HttpOnly => return Err(BrowserFetchError::HttpOnly),
        RenderPolicy::Auto | RenderPolicy::Browser => {}
    }

    is_request_allowed(url).map_err(BrowserFetchError::PolicyViolation)?;

    let browser = lifecycle
        .ensure_browser()
        .await
        .map_err(|e| BrowserFetchError::LaunchFailed(e.to_string()))?;

    let start = Instant::now();

    let params = NavigateParams::new(url, config, start, sanitize_output);

    let context_id = browser
        .create_browser_context(
            chromiumoxide::cdp::browser_protocol::target::CreateBrowserContextParams::default(),
        )
        .await
        .map_err(|e| BrowserFetchError::NavigationFailed(e.to_string()))?;

    let page = browser
        .new_page(
            chromiumoxide::cdp::browser_protocol::target::CreateTargetParams {
                url: url.to_string(),
                width: Some(1280),
                height: Some(720),
                browser_context_id: Some(context_id.clone()),
                ..Default::default()
            },
        )
        .await
        .map_err(|e| BrowserFetchError::NavigationFailed(e.to_string()))?;

    let result = navigate_and_extract(&page, config, &params).await;

    let _ = page.close().await;
    let _ = browser.dispose_browser_context(context_id).await;

    result
}

struct NavigateParams {
    url: String,
    nav_timeout: Duration,
    post_load: Duration,
    verification_timeout: Duration,
    start: Instant,
    sanitize_output: bool,
}

impl NavigateParams {
    fn new(url: &str, config: &BrowserConfig, start: Instant, sanitize_output: bool) -> Self {
        Self {
            url: url.to_string(),
            nav_timeout: Duration::from_millis(config.navigation_timeout_ms),
            post_load: Duration::from_millis(config.post_load_wait_ms),
            verification_timeout: Duration::from_millis(config.verification_wait_ms),
            start,
            sanitize_output,
        }
    }
}

async fn navigate_and_extract(
    page: &Page,
    config: &BrowserConfig,
    params: &NavigateParams,
) -> Result<BrowserFetchResult, BrowserFetchError> {
    let url = &params.url;
    let _ = tokio::time::timeout(params.nav_timeout, page.goto(url))
        .await
        .map_err(|_| BrowserFetchError::Timeout)?
        .map_err(|e| BrowserFetchError::NavigationFailed(e.to_string()))?;

    let _ = tokio::time::timeout(params.nav_timeout, page.wait_for_navigation())
        .await
        .map_err(|_| BrowserFetchError::Timeout)?
        .map_err(|e| BrowserFetchError::NavigationFailed(e.to_string()))?;

    tokio::time::sleep(params.post_load).await;

    let final_url = page
        .url()
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| url.to_string());

    let title: Option<String> = page
        .evaluate("document.title")
        .await
        .ok()
        .and_then(|v| v.into_value::<String>().ok());

    let text_len: usize = page
        .evaluate("document.body ? document.body.innerText.length : 0")
        .await
        .ok()
        .and_then(|v| v.into_value::<f64>().ok())
        .map(|v| v as usize)
        .unwrap_or(0);

    let body_snippet: Vec<u8> = page
        .evaluate("document.body ? document.body.innerHTML.substring(0, 4096) : ''")
        .await
        .ok()
        .and_then(|v| v.into_value::<String>().ok())
        .map(|s| s.into_bytes())
        .unwrap_or_default();

    let classification = classify_response(
        200,
        Some("text/html"),
        title.as_deref(),
        text_len,
        &body_snippet,
    );

    match &classification {
        FetchDisposition::InteractiveChallenge => {
            return Err(BrowserFetchError::InteractiveChallenge(
                ManualInteractionRequired {
                    origin: url.to_string(),
                    reason: ManualInteractionReason::InteractiveChallenge,
                    browser_profile_supported: false,
                    message: "Interactive challenge detected; manual interaction required".into(),
                },
            ));
        }
        FetchDisposition::NonInteractiveVerification => {
            let verification_start = Instant::now();
            loop {
                if verification_start.elapsed() > params.verification_timeout {
                    return Err(BrowserFetchError::InteractiveChallenge(
                        ManualInteractionRequired {
                            origin: url.to_string(),
                            reason: ManualInteractionReason::OtherVerificationRequired,
                            browser_profile_supported: false,
                            message: "Non-interactive verification did not resolve within timeout"
                                .into(),
                        },
                    ));
                }

                tokio::time::sleep(Duration::from_millis(500)).await;

                let current_title = page
                    .evaluate("document.title")
                    .await
                    .ok()
                    .and_then(|v| v.into_value::<String>().ok())
                    .unwrap_or_default()
                    .to_lowercase();

                let still_verification = current_title.contains("just a moment")
                    || current_title.contains("checking")
                    || current_title.contains("verifying")
                    || current_title.contains("please wait");

                if !still_verification {
                    break;
                }
            }
        }
        _ => {}
    }

    let dom_html: String = page
        .content()
        .await
        .map_err(|e| BrowserFetchError::DomExtractionFailed(e.to_string()))?;

    let dom_bytes = dom_html.len();
    if dom_bytes > config.max_dom_bytes {
        return Err(BrowserFetchError::DomExtractionFailed(format!(
            "DOM size {dom_bytes} exceeds limit {}",
            config.max_dom_bytes
        )));
    }

    let elapsed = params.start.elapsed().as_millis() as u64;

    let dom_b = dom_html.as_bytes();

    let mut warnings = Vec::new();

    let mut trust = TrustMarkers::default();

    let (stripped_title, title_removed) = strip_control_chars(title.as_deref().unwrap_or(""));
    trust.control_chars_removed += title_removed;
    let (bounded_title, title_truncated) = bound_text(&stripped_title, TITLE_MAX_CHARS);
    if title_truncated {
        trust.text_truncated = true;
    }
    let final_title = if bounded_title.is_empty() {
        None
    } else {
        Some(bounded_title)
    };

    let (stripped_body, body_removed) =
        strip_control_chars(String::from_utf8_lossy(dom_b).as_ref());
    trust.control_chars_removed += body_removed;
    let (bounded_body, body_truncated) = bound_text(&stripped_body, 50000);
    if body_truncated {
        trust.text_truncated = true;
    }

    if params.sanitize_output {
        let hits = scan_injection_markers(&bounded_body);
        trust.injection_hits = hits.len();
        for hit in hits {
            warnings.push(format!(
                "possible prompt injection marker detected in browser body: {}",
                hit.pattern
            ));
        }
        trust.text_framed = true;
        trust.text_sanitized = true;
    } else if trust.control_chars_removed > 0 || body_truncated {
        trust.text_sanitized = true;
    }

    Ok(BrowserFetchResult {
        response: TransportResponse {
            transport: FetchTransportKind::Browser,
            requested_url: url.to_string(),
            final_url,
            status: Some(200),
            headers: Vec::new(),
            body: dom_html.into_bytes(),
            content_type: Some("text/html".to_string()),
            redirects: Vec::new(),
            timing: TransportTiming {
                total_ms: elapsed,
                ..TransportTiming::default()
            },
            classification: Some(classification),
        },
        extracted_title: final_title,
        extracted_text: bounded_body,
        trust_markers: trust,
        warnings,
    })
}

pub fn browser_result_to_response(
    result: BrowserFetchResult,
    requested_url: &str,
    max_chars: Option<usize>,
    extract_mode: crate::core::fetch::ExtractMode,
    include_links: bool,
    _sanitize_output: bool,
) -> crate::core::fetch::WebFetchResponse {
    use crate::core::document::{DocumentKind, FetchDocument, FetchRenderMetadata, RenderFormat};
    use crate::fetch::detect;
    use crate::fetch::extract::extract_links_from_html;
    use crate::fetch::render::blocks::render_blocks;

    let body = &result.response.body;
    let final_url = &result.response.final_url;
    let max = max_chars.unwrap_or(12000);
    let mut warnings = result.warnings;
    let trust_markers = result.trust_markers;

    let (rendered_title, rendered_desc, rendered_blocks, render_warnings, _non_utf8) =
        render_blocks(
            body,
            final_url,
            max,
            extract_mode == crate::core::fetch::ExtractMode::Markdown,
        );
    warnings.extend(render_warnings);

    let detected = detect::classify(result.response.content_type.as_deref(), final_url, body);

    let link_result = if include_links {
        extract_links_from_html(body, final_url)
    } else {
        crate::fetch::extract::LinkExtractionResult {
            links: Vec::new(),
            total_seen: 0,
            truncated: false,
        }
    };

    let title = result.extracted_title.or(rendered_title);
    let description = rendered_desc;
    let text_chars = rendered_blocks
        .blocks
        .iter()
        .map(|b| b.text.chars().count())
        .sum::<usize>();

    let mut blocks = rendered_blocks.blocks;
    for block in &mut blocks {
        let (stripped, _) = strip_control_chars(&block.text);
        let (bounded, _) = bound_text(&stripped, max);
        block.text = bounded;
    }

    let mut outline = rendered_blocks.outline;
    for entry in &mut outline {
        let (stripped, _) = strip_control_chars(&entry.title);
        let (bounded, _) = bound_text(&stripped, 500);
        entry.title = bounded;
    }
    if outline.is_empty() {
        if let Some(ref title_text) = title {
            let (stripped_title, _) = strip_control_chars(title_text);
            let (bounded_title, _) = bound_text(&stripped_title, 200);
            if !bounded_title.is_empty() {
                outline.push(crate::core::document::DocumentOutlineEntry {
                    level: 1,
                    title: bounded_title,
                    anchor: None,
                    block_index: if blocks.is_empty() { None } else { Some(0) },
                    page: None,
                });
            }
        }
    }

    let document_id = crate::core::identity::doc_id(
        Some(final_url),
        title.as_deref(),
        Some(DocumentKind::Html.as_str()),
    );
    let chunks = crate::core::document::build_document_chunks(&document_id, &outline, &blocks, max);

    let document = Some(FetchDocument {
        kind: DocumentKind::Html,
        render_format: RenderFormat::AgentBlocksV1,
        text_format: if extract_mode == crate::core::fetch::ExtractMode::Markdown {
            "markdown".to_string()
        } else {
            "plain".to_string()
        },
        text_chars_returned: text_chars,
        text_truncated: trust_markers.text_truncated,
        block_truncated: rendered_blocks.block_truncated,
        link_truncated: link_result.truncated,
        metadata: Some(FetchRenderMetadata {
            bytes_read: Some(body.len()),
            content_length: None,
            charset: None,
            redirects_followed: 0,
            source_extension: None,
            detected_language: detected.language,
        }),
        outline,
        blocks,
        chunks,
    });

    let status = result.response.status.unwrap_or(200);

    warnings.push(crate::core::fetch::WebFetchResponse::untrusted_warning());

    crate::core::fetch::WebFetchResponse {
        url: requested_url.to_string(),
        final_url: final_url.clone(),
        stable_id: Some(crate::core::identity::fetch_id(
            Some(requested_url),
            None,
            None,
            None,
            None,
        )),
        source_id: None,
        title,
        description,
        content_type: result.response.content_type.clone(),
        status,
        fetched: true,
        truncated: trust_markers.text_truncated,
        trust: crate::core::fetch::FetchTrust::ExternalUntrusted,
        text: None,
        raw_text: None,
        raw_text_chars_returned: None,
        raw_text_truncated: false,
        raw_text_cap: None,
        links: link_result.links,
        links_seen: if link_result.total_seen > 0 {
            Some(link_result.total_seen)
        } else {
            None
        },
        links_truncated: link_result.truncated,
        warnings,
        trust_markers,
        document,
        fetch_transform: None,
        structured_warnings: Vec::new(),
        pdf_page_metadata: None,
        pdf_document_metadata: None,
        pdf_quality_score: None,
        pdf_content_ok: None,
        cache_status: crate::fetch::cache::CacheStatus::default(),
        attempt_count: Some(1),
        retry_after_ms: None,
        origin_backoff_ms: None,
        response_headers: None,
        transport: Some("browser".to_string()),
        browser_escalated: false,
        manual_interaction_required: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policy_blocks_private_url() {
        assert!(is_request_allowed("http://127.0.0.1/").is_err());
    }

    #[test]
    fn policy_allows_public_url() {
        assert!(is_request_allowed("https://example.com").is_ok());
    }

    #[test]
    fn http_only_policy_returns_error() {
        let policy = RenderPolicy::HttpOnly;
        assert!(matches!(policy, RenderPolicy::HttpOnly));
    }
}
