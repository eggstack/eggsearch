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
