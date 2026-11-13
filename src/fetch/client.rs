//! HTTP client for fetching URLs.

use std::time::Duration;

use eggfetch_core::{Client, Timeout};
use futures::StreamExt;

use super::accept::AcceptPolicy;
use super::detect;
use super::extract::{extract_links_from_html, extract_links_from_markdown, LinkExtractionResult};
use super::limits::{validate_fetch_target_with_resolved_addrs, validate_url, FetchLimits};
use super::negotiate::{classify_content_type, parse_token_header, representation_of};
use super::render;
use super::types::FetchError;
use crate::core::code_host_fetch::resolve_code_host_fetch_target;
use crate::core::document::{
    build_document_chunks, DocumentKind, DocumentOutlineEntry, FetchDocument, FetchRenderMetadata,
    RenderFormat,
};
use crate::core::fetch::{ExtractMode, FetchRepresentation, FetchTrust, WebFetchResponse};
use crate::core::sanitize::{
    bound_text, frame, scan_injection_markers, strip_control_chars, TrustMarkers,
    SNIPPET_MAX_CHARS, TITLE_MAX_CHARS,
};

/// HTTP client for fetching URLs.
pub struct FetchClient {
    client: Client,
    limits: FetchLimits,
    user_agent: String,
    /// Whether to wrap untrusted fetched text in
    /// `<<<EXTERNAL_UNTRUSTED ...>>>` framing and emit per-response
    /// prompt-injection warnings. Tier 1 (control-char stripping +
    /// length bounding) is always on; this flag gates Tier 2
    /// (framing) and Tier 3 (marker scan).
    sanitize_output: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TimeoutOverrideMode {
    Shared,
    Widened,
}

fn timeout_override_mode(base_timeout_ms: u64, requested_timeout_ms: u64) -> TimeoutOverrideMode {
    if requested_timeout_ms <= base_timeout_ms {
        TimeoutOverrideMode::Shared
    } else {
        TimeoutOverrideMode::Widened
    }
}

fn build_transport_client(timeout_ms: u64, user_agent: &str) -> Client {
    Client::builder()
        .user_agent(user_agent)
        .timeout(client_timeout(timeout_ms))
        .follow_redirects(false)
        .build()
}

impl FetchClient {
    /// Creates a new FetchClient with the given limits, user agent,
    /// and sanitize-output flag.
    ///
    /// `sanitize_output = true` enables Tier 2 (framing) and Tier 3
    /// (prompt-injection marker scanning + warnings) on top of the
    /// always-on Tier 1 (control-char stripping + length bounding).
    pub fn new(
        limits: FetchLimits,
        user_agent: String,
        sanitize_output: bool,
    ) -> anyhow::Result<Self> {
        let client = build_transport_client(limits.timeout_ms, &user_agent);
        Ok(Self {
            client,
            limits,
            user_agent,
            sanitize_output,
        })
    }

    /// Clone this client with a different request timeout.
    pub fn with_timeout_ms(&self, timeout_ms: u64) -> anyhow::Result<Self> {
        let mut limits = self.limits.clone();
        limits.timeout_ms = timeout_ms;
        let client = match timeout_override_mode(self.limits.timeout_ms, timeout_ms) {
            TimeoutOverrideMode::Shared => self.client.clone(),
            TimeoutOverrideMode::Widened => build_transport_client(timeout_ms, &self.user_agent),
        };
        Ok(Self {
            client,
            limits,
            user_agent: self.user_agent.clone(),
            sanitize_output: self.sanitize_output,
        })
    }

    /// Fetches a URL and extracts content.
    ///
    /// # Arguments
    ///
    /// * `url_str` - The URL to fetch
    /// * `max_chars` - Maximum characters to extract (None for default)
    /// * `extract_mode` - The extraction mode to use
    /// * `include_links` - Whether to include extracted links
    #[allow(clippy::too_many_arguments)]
    pub async fn fetch(
        &self,
        url_str: &str,
        max_chars: Option<usize>,
        extract_mode: ExtractMode,
        include_links: bool,
        #[cfg_attr(not(feature = "pdf"), allow(unused_variables))] pdf_options: Option<
            &crate::core::fetch::PdfFetchOptions,
        >,
    ) -> Result<WebFetchResponse, FetchError> {
        self.fetch_capturing(
            url_str,
            max_chars,
            extract_mode,
            include_links,
            pdf_options,
            None,
            None,
        )
        .await
        .map(|(response, _)| response)
    }

    #[allow(clippy::too_many_arguments, missing_docs)]
    pub async fn fetch_with_origin_controller(
        &self,
        url_str: &str,
        max_chars: Option<usize>,
        extract_mode: ExtractMode,
        include_links: bool,
        pdf_options: Option<&crate::core::fetch::PdfFetchOptions>,
        origin_controller: &super::origin::OriginController,
    ) -> Result<WebFetchResponse, FetchError> {
        self.fetch_capturing(
            url_str,
            max_chars,
            extract_mode,
            include_links,
            pdf_options,
            Some(origin_controller),
            None,
        )
        .await
        .map(|(response, _)| response)
    }

    /// Fetches a URL replaying one previously issued cookie.
    ///
    /// `cookie` is sent as the `Cookie` header on the first hop only: a
    /// redirect cannot forward it to another origin. Cache keys and content
    /// negotiation are unchanged.
    #[allow(clippy::too_many_arguments)]
    pub async fn fetch_with_cookie(
        &self,
        url_str: &str,
        max_chars: Option<usize>,
        extract_mode: ExtractMode,
        include_links: bool,
        #[cfg_attr(not(feature = "pdf"), allow(unused_variables))] pdf_options: Option<
            &crate::core::fetch::PdfFetchOptions,
        >,
        origin_controller: Option<&super::origin::OriginController>,
        cookie: Option<&str>,
    ) -> Result<WebFetchResponse, FetchError> {
        self.fetch_capturing(
            url_str,
            max_chars,
            extract_mode,
            include_links,
            pdf_options,
            origin_controller,
            cookie,
        )
        .await
        .map(|(response, _)| response)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn fetch_capturing(
        &self,
        url_str: &str,
        max_chars: Option<usize>,
        extract_mode: ExtractMode,
        include_links: bool,
        #[cfg_attr(not(feature = "pdf"), allow(unused_variables))] pdf_options: Option<
            &crate::core::fetch::PdfFetchOptions,
        >,
        origin_controller: Option<&super::origin::OriginController>,
        cookie: Option<&str>,
    ) -> Result<(WebFetchResponse, Option<String>), FetchError> {
        self.fetch_internal(
            url_str,
            max_chars,
            extract_mode,
            include_links,
            pdf_options,
            origin_controller,
            cookie,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn fetch_once(
        &self,
        url_str: &str,
        max_chars: Option<usize>,
        extract_mode: ExtractMode,
        include_links: bool,
        #[cfg_attr(not(feature = "pdf"), allow(unused_variables))] pdf_options: Option<
            &crate::core::fetch::PdfFetchOptions,
        >,
        origin_controller: Option<&super::origin::OriginController>,
        accept_policy: AcceptPolicy,
        cookie: Option<&str>,
    ) -> Result<(WebFetchResponse, Option<String>), FetchError> {
        // Validate the initial URL (scheme, length, localhost literals,
        // obvious private-network literals, credentials).
        let initial_url = validate_url(url_str, &self.limits)?;

        // Check if this is a code-host source-file URL that should be
        // rewritten to a raw content URL for fetching.
        let code_host_target = resolve_code_host_fetch_target(url_str);
        let (fetch_url, fetch_transform) = if let Some(ref target) = code_host_target {
            if let Some(ref raw_url) = target.raw_url {
                // Validate the raw URL through the same safety pipeline.
                let raw_url_parsed = validate_url(raw_url, &self.limits)?;
                let transform = target.to_fetch_transform(raw_url);
                (raw_url_parsed, transform)
            } else {
                (initial_url, None)
            }
        } else {
            (initial_url, None)
        };

        let budgets = crate::fetch::budget::FetchBudgets::resolve(&self.limits, max_chars);

        let mut current_url = fetch_url;
        let mut redirect_count: usize = 0;
        let initial_origin = super::origin::OriginKey::from_url(&current_url);
        // At most one origin permit is held at a time. Holding one permit
        // per redirect hop would let a chain that revisits a single
        // cross-origin host consume the whole per-origin semaphore and
        // block forever on the next acquire.
        let mut permitted_origin: Option<super::origin::OriginKey> = None;
        let mut redirect_permit: Option<tokio::sync::OwnedSemaphorePermit> = None;
        // Addresses resolved at the redirect site, carried into the next hop so
        // the loop head does not resolve the same host twice.
        let mut carried_addrs: Option<Vec<std::net::SocketAddr>> = None;

        let mut response = loop {
            let hop_origin = super::origin::OriginKey::from_url(&current_url);
            let redirected_origin = hop_origin.as_ref() != initial_origin.as_ref();
            if permitted_origin.as_ref() != hop_origin.as_ref() {
                redirect_permit = None;
                permitted_origin = hop_origin.clone();
            }
            if redirected_origin && redirect_permit.is_none() {
                if let (Some(controller), Some(key)) = (origin_controller, hop_origin.as_ref()) {
                    let permit = controller
                        .acquire_bounded(key, Duration::from_millis(self.limits.timeout_ms))
                        .await
                        .map_err(|error| {
                            FetchError::NetworkError(format!("origin backoff: {error}"))
                        })?;
                    redirect_permit = Some(permit);
                }
            }
            // Full validation: credentials, localhost, DNS resolution, IP checks.
            let resolved_addrs = match carried_addrs.take() {
                Some(addrs) => Some(addrs),
                None => {
                    validate_fetch_target_with_resolved_addrs(&current_url, &self.limits).await?
                }
            };
            // Reuse the validated address set so the connect path
            // cannot drift to a different DNS answer for this attempt.
            let builder = self
                .client
                .get(current_url.as_str())
                .map_err(|e| FetchError::InvalidUrl(e.to_string()))?;
            let builder = builder.timeout(client_timeout(self.limits.timeout_ms));
            let builder = builder.header("accept", accept_policy.header_value());
            let builder = match resolved_addrs {
                Some(addrs) if !addrs.is_empty() => builder.resolved_addresses(addrs),
                _ => builder,
            };
            let builder = match cookie {
                Some(value) if redirect_count == 0 => builder.header("cookie", value),
                _ => builder,
            };

            let resp = match builder.send().await {
                Ok(response) => response,
                Err(error) => {
                    if redirected_origin {
                        if let (Some(controller), Some(key)) =
                            (origin_controller, hop_origin.as_ref())
                        {
                            controller
                                .record_failure(key, super::origin::OriginFailureClass::Retryable)
                                .await;
                        }
                    }
                    return Err(map_send_error(error, self.limits.timeout_ms));
                }
            };

            let status = resp.status().as_u16();
            if redirected_origin {
                if let (Some(controller), Some(key)) = (origin_controller, hop_origin.as_ref()) {
                    if status >= 400 {
                        controller
                            .record_failure(key, super::origin::classify_http_status(status))
                            .await;
                    } else {
                        controller.record_success(key).await;
                    }
                }
            }

            if (300..400).contains(&status) {
                let location = resp
                    .headers()
                    .get("location")
                    .and_then(|v| v.to_str().ok())
                    .map(|s| s.to_string());

                let location = match location {
                    Some(loc) if !loc.is_empty() => loc,
                    _ => {
                        return Err(FetchError::InvalidRedirectLocation(format!(
                            "HTTP {status} missing or empty Location header"
                        )));
                    }
                };

                // Resolve relative redirects against the current URL.
                let redirect_url = current_url.join(&location).map_err(|e| {
                    FetchError::InvalidRedirectLocation(format!(
                        "failed to resolve redirect location '{location}': {e}"
                    ))
                })?;

                redirect_count += 1;
                if redirect_count > self.limits.redirect_limit {
                    return Err(FetchError::RedirectLimitExceeded(redirect_count - 1));
                }

                // Validate the redirect target before following, and carry the
                // resolved addresses into the next hop instead of re-resolving.
                carried_addrs = Some(
                    validate_fetch_target_with_resolved_addrs(&redirect_url, &self.limits)
                        .await
                        .map_err(map_redirect_validation_error)?
                        .unwrap_or_default(),
                );

                current_url = redirect_url;
                continue;
            }

            break resp;
        };

        let final_url = response.url().to_string();
        let status = response.status().as_u16();
        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        let cache_headers: std::collections::HashMap<String, String> = response
            .headers()
            .iter()
            .filter(|(k, _)| {
                matches!(
                    k.as_str(),
                    "etag"
                        | "last-modified"
                        | "cache-control"
                        | "expires"
                        | "vary"
                        | "x-markdown-tokens"
                        | "x-original-tokens"
                )
            })
            .filter_map(|(k, v)| {
                v.to_str()
                    .ok()
                    .map(|val| (k.as_str().to_string(), val.to_string()))
            })
            .collect();

        let challenge_cookies = set_cookie_pairs(response.headers());

        // Pre-check Content-Length for honest servers. The streaming
        // body cap below remains the authoritative upper bound for
        // chunked/encoded responses; this is an early bailout.
        let body_cap = self
            .limits
            .body_cap(classify_content_type(content_type.as_deref(), &final_url, false).is_pdf);
        let mut content_length_header: Option<usize> = None;
        let declared_len: Option<u64> = response
            .wire_content_length()
            .and_then(|s| s.parse::<u64>().ok())
            .or_else(|| response.content_length());
        if let Some(content_length_u64) = declared_len {
            if content_length_u64 > body_cap as u64 {
                return Err(FetchError::ContentTooLarge(
                    usize::try_from(content_length_u64).unwrap_or(usize::MAX),
                    body_cap,
                ));
            }
            let content_length = usize::try_from(content_length_u64).unwrap_or(usize::MAX);
            content_length_header = Some(content_length);
        } else if response.wire_content_length().is_some()
            || response.headers().contains_key("content-length")
        {
            tracing::warn!(
                header = ?response.wire_content_length(),
                "unparseable content-length header; proceeding to streaming cap"
            );
        }

        if !(200..300).contains(&status) {
            return Err(FetchError::HttpStatus(status, format!("HTTP {status}")));
        }

        let mut kind = classify_content_type(content_type.as_deref(), &final_url, false);

        // If neither Content-Type nor URL extension indicates PDF,
        // peek at the first 5 bytes of the body for the `%PDF-` magic.
        // This catches misconfigured servers that serve PDFs as
        // application/octet-stream or text/plain.
        let mut stream = response
            .bytes_stream()
            .map_err(|e| FetchError::NetworkError(e.to_string()))?;
        let mut pdf_magic_chunk = None;
        if !kind.is_pdf {
            let mut prefix = Vec::with_capacity(5);
            while prefix.len() < 5 {
                match stream.next().await {
                    Some(Ok(chunk)) => {
                        let remaining = 5 - prefix.len();
                        prefix.extend_from_slice(&chunk[..chunk.len().min(remaining)]);
                        if prefix.len() == 5 {
                            kind.is_pdf = prefix == b"%PDF-";
                        }
                        pdf_magic_chunk = Some(prefix.clone());
                        if chunk.len() > remaining {
                            let mut combined = prefix.clone();
                            combined.extend_from_slice(&chunk[remaining..]);
                            pdf_magic_chunk = Some(combined);
                            break;
                        }
                    }
                    Some(Err(e)) => return Err(FetchError::NetworkError(e.to_string())),
                    None => break,
                }
            }
            if pdf_magic_chunk.is_none() && !prefix.is_empty() {
                pdf_magic_chunk = Some(prefix);
            }
        }

        if !kind.is_html && !kind.is_text && !kind.is_pdf {
            return Err(FetchError::UnsupportedContentType(
                content_type.clone().unwrap_or_else(|| "unknown".into()),
            ));
        }

        // Handle PDF-specific gates before reading the body.
        if kind.is_pdf && !cfg!(feature = "pdf") {
            return Err(FetchError::PdfNotCompiledIn);
        }
        if kind.is_pdf && !self.limits.pdf_enabled {
            return Err(FetchError::PdfDisabled);
        }

        // Read the full body now, incorporating any magic-chunk we
        // peeked for PDF detection.
        let mut body = Vec::new();
        let mut truncated = false;

        if let Some(chunk) = pdf_magic_chunk {
            truncated = append_bounded(&mut body, &chunk, body_cap);
        }
        if !truncated {
            while let Some(chunk_result) = stream.next().await {
                let chunk = chunk_result.map_err(|e| FetchError::NetworkError(e.to_string()))?;
                truncated = append_bounded(&mut body, &chunk, body_cap);
                if truncated {
                    break;
                }
            }
        }

        self.derive_from_raw(
            url_str,
            final_url,
            status,
            content_type,
            cache_headers,
            content_length_header,
            redirect_count,
            body,
            truncated,
            budgets,
            extract_mode,
            include_links,
            pdf_options,
            fetch_transform,
        )
        .map(|response| (response, challenge_cookies))
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn derive_from_raw(
        &self,
        url_str: &str,
        final_url: String,
        status: u16,
        content_type: Option<String>,
        cache_headers: std::collections::HashMap<String, String>,
        content_length_header: Option<usize>,
        redirect_count: usize,
        body: Vec<u8>,
        truncated: bool,
        budgets: crate::fetch::budget::FetchBudgets,
        extract_mode: ExtractMode,
        include_links: bool,
        pdf_options: Option<&crate::core::fetch::PdfFetchOptions>,
        fetch_transform: Option<crate::core::fetch::FetchTransform>,
    ) -> Result<WebFetchResponse, FetchError> {
        #[cfg(not(feature = "pdf"))]
        let _ = pdf_options;

        let document_chars = budgets.document_chars();
        let window_chars = budgets.window_chars();

        let final_url_parsed = url::Url::parse(&final_url).ok();

        let kind = classify_content_type(
            content_type.as_deref(),
            &final_url,
            body.starts_with(b"%PDF-"),
        );

        if !kind.is_html && !kind.is_text && !kind.is_pdf {
            return Err(FetchError::UnsupportedContentType(
                content_type.clone().unwrap_or_else(|| "unknown".into()),
            ));
        }
        if kind.is_pdf && !cfg!(feature = "pdf") {
            return Err(FetchError::PdfNotCompiledIn);
        }
        if kind.is_pdf && !self.limits.pdf_enabled {
            return Err(FetchError::PdfDisabled);
        }

        let representation = representation_of(content_type.as_deref(), kind);
        let markdown_tokens = parse_token_header(&cache_headers, "x-markdown-tokens");
        let original_tokens = parse_token_header(&cache_headers, "x-original-tokens");

        // --- PDF extraction path (early return) ---
        // PDFs are handled as a completely separate path because the
        // extraction, sanitization, and document construction differ
        // from the shared HTML/text pipeline.
        #[cfg(feature = "pdf")]
        if kind.is_pdf {
            // A PDF that was cut at the effective cap is a size outcome, not a
            // document defect: never hand a truncated body to the parser.
            if truncated {
                let cap = self.limits.body_cap(
                    classify_content_type(content_type.as_deref(), &final_url, false).is_pdf,
                );
                return Err(FetchError::ContentTooLarge(cap.saturating_add(1), cap));
            }

            // Metadata-only mode: skip expensive text extraction.
            // Return fetch metadata and a minimal empty document.
            if extract_mode == ExtractMode::MetadataOnly {
                let charset = content_type
                    .as_ref()
                    .and_then(|ct| {
                        ct.split(';')
                            .nth(1)?
                            .trim()
                            .strip_prefix("charset=")?
                            .split(',')
                            .next()
                            .map(|s| s.trim().to_string())
                    })
                    .filter(|c| !c.is_empty());

                let source_extension = final_url_parsed
                    .as_ref()
                    .and_then(|u| {
                        let path = u.path();
                        path.rsplit('.')
                            .next()
                            .filter(|ext| !ext.is_empty())
                            .map(|ext| {
                                if ext.len() <= 10 {
                                    ext.to_string()
                                } else {
                                    String::new()
                                }
                            })
                    })
                    .filter(|e| !e.is_empty());

                let mut warnings = Vec::new();
                let trust_markers = TrustMarkers::default();

                warnings.push(WebFetchResponse::untrusted_warning());

                return Ok(WebFetchResponse {
                    url: url_str.to_string(),
                    final_url: final_url.clone(),
                    stable_id: Some(crate::core::identity::fetch_id(
                        Some(url_str),
                        None,
                        None,
                        None,
                        None,
                    )),
                    source_id: None,
                    title: None,
                    description: None,
                    content_type,
                    representation,
                    status,
                    fetched: true,
                    truncated,
                    trust: FetchTrust::ExternalUntrusted,
                    text: None,
                    raw_text: None,
                    raw_text_chars_returned: None,
                    raw_text_truncated: false,
                    raw_text_cap: None,
                    links: Vec::new(),
                    links_seen: None,
                    links_truncated: false,
                    warnings,
                    trust_markers,
                    document: Some(FetchDocument {
                        doc_id: crate::core::identity::doc_id(
                            Some(&final_url),
                            None,
                            Some(DocumentKind::Pdf.as_str()),
                        ),
                        kind: DocumentKind::Pdf,
                        render_format: RenderFormat::AgentBlocksV1,
                        text_format: "plain".to_string(),
                        text_chars_returned: 0,
                        text_truncated: false,
                        block_truncated: false,
                        document_truncated: false,
                        link_truncated: false,
                        metadata: Some(FetchRenderMetadata {
                            bytes_read: Some(body.len()),
                            content_length: content_length_header,
                            charset,
                            redirects_followed: redirect_count,
                            source_extension,
                            detected_language: None,
                            markdown_tokens: None,
                            original_tokens: None,
                        }),
                        outline: Vec::new(),
                        blocks: Vec::new(),
                        chunks: Vec::new(),
                    }),
                    fetch_transform: None,
                    structured_warnings: Vec::new(),
                    pdf_page_metadata: None,
                    pdf_document_metadata: None,
                    pdf_quality_score: None,
                    pdf_content_ok: None,
                    cache_status: crate::fetch::cache::CacheStatus::default(),
                    attempt_count: None,
                    retry_after_ms: None,
                    origin_backoff_ms: None,
                    response_headers: if cache_headers.is_empty() {
                        None
                    } else {
                        Some(cache_headers.clone())
                    },
                    transport: Some("http".to_string()),
                    browser_escalated: false,
                    accept_policy: None,
                    manual_interaction_required: false,
                    focus: None,
                    raw_body: Some(body),
                });
            }

            let pdf_limits = super::pdf::PdfLimits {
                max_pages: self.limits.pdf_max_pages,
                max_chars_per_page: self.limits.pdf_max_chars_per_page,
                max_total_chars: self.limits.pdf_max_total_chars,
                max_decompressed_bytes: self.limits.pdf_max_decompressed_bytes,
            };

            let pdf_extract_opts = if let Some(opts) = pdf_options {
                let selected_pages = match opts.pages.as_deref() {
                    Some(spec) => Some(super::pdf::parse_pdf_page_spec(
                        spec,
                        self.limits.pdf_max_pages,
                    )?),
                    None => None,
                };
                Some(super::pdf::PdfExtractOptions {
                    selected_pages,
                    password: opts.pdf_password.clone(),
                    include_media: opts.include_media.unwrap_or(false),
                    ocr_policy: match opts.pdf_ocr {
                        Some(crate::core::fetch::PdfOcrPolicy::Never) | None => {
                            super::pdf::PdfOcrPolicy::Never
                        }
                        Some(crate::core::fetch::PdfOcrPolicy::Auto) => {
                            super::pdf::PdfOcrPolicy::Auto
                        }
                        Some(crate::core::fetch::PdfOcrPolicy::Always) => {
                            super::pdf::PdfOcrPolicy::Always
                        }
                    },
                })
            } else {
                None
            };

            let mut pdf_result = super::pdf::extract_pdf_text(
                &body,
                window_chars,
                &pdf_limits,
                pdf_extract_opts.as_ref(),
            )?;

            // Propagate fetch-level context into the PDF document
            // metadata. The extraction function knows nothing about
            // redirects, content-length headers, or body size — patch
            // the metadata here so PDF documents report the same fetch
            // context quality as HTML/text documents.
            if let Some(ref mut meta) = pdf_result.document.metadata {
                meta.bytes_read = Some(body.len());
                meta.content_length = content_length_header;
                meta.redirects_followed = redirect_count;
            }
            pdf_result.document.doc_id = crate::core::identity::doc_id(
                Some(&final_url),
                pdf_result.title.as_deref(),
                Some(pdf_result.document.kind.as_str()),
            );

            // Sanitize the legacy text field (Tier 1 + Tier 2/3).
            let mut warnings = pdf_result.warnings;
            let mut trust_markers = TrustMarkers::default();

            // Sanitize PDF title if present
            let title = if let Some(t) = &pdf_result.title {
                let (s, m) = sanitize_field(
                    t,
                    "title",
                    &final_url,
                    TITLE_MAX_CHARS,
                    self.sanitize_output,
                    &mut warnings,
                );
                trust_markers.merge(&m);
                Some(s)
            } else {
                None
            };

            // Sanitize the legacy text field
            let (stripped_text, _) = strip_control_chars(&pdf_result.text);
            let (bounded_text, _) = bound_text(&stripped_text, window_chars);
            let (text, text_markers) = sanitize_field(
                &bounded_text,
                "text",
                &final_url,
                window_chars,
                self.sanitize_output,
                &mut warnings,
            );
            trust_markers.merge(&text_markers);

            warnings.push(WebFetchResponse::untrusted_warning());

            return Ok(WebFetchResponse {
                url: url_str.to_string(),
                final_url: final_url.clone(),
                stable_id: Some(crate::core::identity::fetch_id(
                    Some(url_str),
                    None,
                    None,
                    None,
                    None,
                )),
                source_id: None,
                title,
                description: None,
                content_type,
                representation,
                status,
                fetched: true,
                truncated,
                trust: FetchTrust::ExternalUntrusted,
                text: Some(text),
                raw_text: None,
                raw_text_chars_returned: None,
                raw_text_truncated: false,
                raw_text_cap: None,
                links: Vec::new(),
                links_seen: None,
                links_truncated: false,
                warnings,
                trust_markers,
                document: Some(pdf_result.document),
                fetch_transform: None,
                structured_warnings: pdf_result.structured_warnings,
                pdf_page_metadata: Some(pdf_result.page_metadata),
                pdf_document_metadata: Some(pdf_result.pdf_metadata),
                pdf_quality_score: Some(pdf_result.quality_score),
                pdf_content_ok: Some(pdf_result.content_ok),
                cache_status: crate::fetch::cache::CacheStatus::default(),
                attempt_count: None,
                retry_after_ms: None,
                origin_backoff_ms: None,
                response_headers: if cache_headers.is_empty() {
                    None
                } else {
                    Some(cache_headers.clone())
                },
                transport: Some("http".to_string()),
                browser_escalated: false,
                accept_policy: None,
                manual_interaction_required: false,
                focus: None,
                raw_body: Some(body),
            });
        }

        let mut cached_html_render: Option<render::blocks::RenderedBlocks> = None;
        let mut raw_capped = false;
        let decoded = String::from_utf8_lossy(&body);

        let (
            mut title,
            mut description,
            mut text,
            links,
            extract_warnings,
            document_truncated,
            text_truncated,
            links_seen,
            links_truncated,
        ) = if extract_mode == ExtractMode::MetadataOnly {
            if kind.is_html {
                // Keep metadata extraction bounded, but skip body text
                // and structured document construction.
                let (t, d, _blocks, w, _) =
                    render::blocks::render_blocks(&body, &final_url, document_chars, false);
                let LinkExtractionResult {
                    links,
                    total_seen,
                    truncated,
                } = if include_links {
                    extract_links_from_html(&body, &final_url)
                } else {
                    LinkExtractionResult {
                        links: Vec::new(),
                        total_seen: 0,
                        truncated: false,
                    }
                };
                (t, d, None, links, w, false, false, total_seen, truncated)
            } else {
                (
                    None,
                    None,
                    None,
                    Vec::new(),
                    Vec::new(),
                    false,
                    false,
                    0,
                    false,
                )
            }
        } else if kind.is_html {
            let is_markdown = extract_mode == ExtractMode::Markdown;
            let (t, d, rendered, w, _non_utf8) =
                render::blocks::render_blocks(&body, &final_url, document_chars, is_markdown);
            let LinkExtractionResult {
                links,
                total_seen,
                truncated,
            } = if include_links {
                extract_links_from_html(&body, &final_url)
            } else {
                LinkExtractionResult {
                    links: Vec::new(),
                    total_seen: 0,
                    truncated: false,
                }
            };
            // Render text based on mode
            let txt = match extract_mode {
                ExtractMode::Markdown => render::markdown::render_blocks_markdown(&rendered.blocks),
                _ => render::text::render_blocks_text(&rendered.blocks),
            };
            let document_truncated = rendered.text_truncated;
            // Truncate text to max_chars
            let (bounded_txt, text_truncated) = bound_text(&txt, window_chars);
            // Cache rendered blocks for document construction (avoids
            // a second render_blocks call below).
            cached_html_render = Some(rendered);
            (
                t,
                d,
                Some(bounded_txt),
                links,
                w,
                document_truncated,
                text_truncated,
                total_seen,
                truncated,
            )
        } else {
            let (full_text, document_truncated) = bound_text(&decoded, document_chars);
            let (text, text_truncated) = bound_text(&full_text, window_chars);
            let LinkExtractionResult {
                links,
                total_seen,
                truncated,
            } = if include_links && representation == Some(FetchRepresentation::Markdown) {
                extract_links_from_markdown(decoded.as_ref(), &final_url)
            } else {
                LinkExtractionResult {
                    links: Vec::new(),
                    total_seen: 0,
                    truncated: false,
                }
            };
            (
                None,
                None,
                Some(text),
                links,
                Vec::new(),
                document_truncated,
                text_truncated,
                total_seen,
                truncated,
            )
        };

        let mut warnings = extract_warnings;

        let pre_framing_text = text.clone();
        let raw_text: Option<String> = if extract_mode != ExtractMode::MetadataOnly {
            let (stripped, _) = strip_control_chars(&decoded);
            let (bounded, raw_bounded) = bound_text(&stripped, document_chars);
            if raw_bounded {
                raw_capped = true;
            }
            Some(bounded)
        } else {
            None
        };
        let raw_title = title.clone();

        // Sanitize each untrusted field. Tier 1 (strip + bound) is
        // always on; Tier 2 (framing) and Tier 3 (marker scan) are
        // gated by `self.sanitize_output`. The `final_url` is used as
        // the per-field `id` in the framing header so the framing
        // identifies which URL the content came from.
        let mut trust_markers = TrustMarkers::default();
        if raw_capped {
            trust_markers.text_truncated = true;
        }

        if let Some(t) = title {
            let (s, m) = sanitize_field(
                &t,
                "title",
                &final_url,
                TITLE_MAX_CHARS,
                self.sanitize_output,
                &mut warnings,
            );
            title = Some(s);
            trust_markers.merge(&m);
        }
        if let Some(d) = description {
            let (s, m) = sanitize_field(
                &d,
                "description",
                &final_url,
                SNIPPET_MAX_CHARS,
                self.sanitize_output,
                &mut warnings,
            );
            description = Some(s);
            trust_markers.merge(&m);
        }
        if let Some(t) = text {
            // The body is already bounded to `max_chars` by the
            // extractor; re-bounding to that cap is a no-op safety
            // net after control-char stripping.
            let (s, m) = sanitize_field(
                &t,
                "text",
                &final_url,
                window_chars,
                self.sanitize_output,
                &mut warnings,
            );
            text = Some(s);
            trust_markers.merge(&m);
        }

        warnings.push(WebFetchResponse::untrusted_warning());

        // Build the structured document from extraction output.
        let document = if extract_mode != ExtractMode::MetadataOnly {
            // Use the detection classifier to determine document kind,
            // language, and rendering strategy.
            let detected = detect::classify(content_type.as_deref(), &final_url, &body);

            let doc_kind = if kind.is_html {
                DocumentKind::Html
            } else {
                detected.kind
            };

            let charset = content_type
                .as_ref()
                .and_then(|ct| {
                    ct.split(';')
                        .nth(1)?
                        .trim()
                        .strip_prefix("charset=")?
                        .split(',')
                        .next()
                        .map(|s| s.trim().to_string())
                })
                .filter(|c| !c.is_empty());

            let source_extension = final_url_parsed
                .as_ref()
                .and_then(|u| {
                    let path = u.path();
                    path.rsplit('.')
                        .next()
                        .filter(|ext| !ext.is_empty())
                        .map(|ext| {
                            if ext.len() <= 10 {
                                ext.to_string()
                            } else {
                                String::new()
                            }
                        })
                })
                .filter(|e| !e.is_empty());

            let mut document_truncated = document_truncated;
            let (blocks, outline, text_chars, block_truncated, window_text_truncated) = if kind
                .is_html
            {
                // Reuse cached render from the extraction phase (avoids
                // a second parse + DOM walk of the same HTML body).
                let rendered = cached_html_render.take().unwrap_or_else(|| {
                    let is_markdown = extract_mode == ExtractMode::Markdown;
                    render::blocks::render_blocks(&body, &final_url, document_chars, is_markdown).2
                });
                // text_chars is computed from the truncated text
                // (`pre_framing_text`, bounded by `max_chars`), not
                // from the new `raw_text` field which is bounded by
                // `max_chars_cap` for internal consumers like
                // `repo_fetch`.
                let block_truncated = rendered.block_truncated;

                // Apply Tier 1 (strip + bound) to each block's text
                let mut blocks = rendered.blocks;
                for block in &mut blocks {
                    let (stripped, _) = strip_control_chars(&block.text);
                    let (bounded, _) = bound_text(&stripped, document_chars);
                    block.text = bounded;
                }

                // If no headings found, populate outline from page title
                let mut outline = rendered.outline;
                for entry in &mut outline {
                    let (stripped, _) = strip_control_chars(&entry.title);
                    let (bounded, _) = bound_text(&stripped, 500);
                    entry.title = bounded;
                }
                if outline.is_empty() {
                    if let Some(ref title_text) = raw_title {
                        let (stripped_title, _) = strip_control_chars(title_text);
                        let (bounded_title, _) = bound_text(&stripped_title, 200);
                        if !bounded_title.is_empty() {
                            outline.push(DocumentOutlineEntry {
                                level: 1,
                                title: bounded_title,
                                anchor: None,
                                block_index: if blocks.is_empty() { None } else { Some(0) },
                                page: None,
                            });
                        }
                    }
                }

                let (projected_text, projected_truncated) =
                    crate::fetch::projection::project_document_text(
                        &blocks,
                        window_chars,
                        extract_mode == ExtractMode::Markdown,
                    );
                let text_chars = projected_text.chars().count();

                (
                    blocks,
                    outline,
                    text_chars,
                    block_truncated,
                    projected_truncated,
                )
            } else if let Some(ref t) = raw_text {
                // Non-HTML path: render from the document-bounded text so the
                // block and chunk set covers the extracted document rather than
                // the caller's window. `text_chars` stays window-scoped because
                // it describes the legacy `text` field, not the addressable set.
                let text_chars = pre_framing_text.as_ref().map_or(0, |t| t.chars().count());

                let rendered = match detected.kind {
                    DocumentKind::Notebook => render::notebook::render_notebook(t, document_chars),
                    DocumentKind::Csv => render::csv::render_csv(t, document_chars),
                    DocumentKind::Xml | DocumentKind::Rst | DocumentKind::AsciiDoc => {
                        render::code::render_plaintext(t, document_chars)
                    }
                    _ if detected.line_preserving => match detected.kind {
                        DocumentKind::Markdown => {
                            let md =
                                render::markdown_source::render_markdown_source(t, document_chars);
                            render::code::RenderedContent {
                                blocks: md.blocks,
                                outline: md.outline,
                                text_truncated: md.text_truncated,
                                block_truncated: md.block_truncated,
                            }
                        }
                        DocumentKind::Diff | DocumentKind::Patch => {
                            render::code::render_diff(t, document_chars)
                        }
                        _ => render::code::render_code(
                            t,
                            detected.language.as_deref(),
                            document_chars,
                        ),
                    },
                    _ => render::code::render_plaintext(t, document_chars),
                };

                // Apply Tier 1 (strip + bound) to each block's text
                let mut blocks = rendered.blocks;
                for block in &mut blocks {
                    let (stripped, _) = strip_control_chars(&block.text);
                    let (bounded, _) = bound_text(&stripped, document_chars);
                    block.text = bounded;
                }

                let mut outline = rendered.outline;
                for entry in &mut outline {
                    let (stripped, _) = strip_control_chars(&entry.title);
                    let (bounded, _) = bound_text(&stripped, 500);
                    entry.title = bounded;
                }
                let block_truncated = rendered.block_truncated;
                document_truncated |= rendered.text_truncated;

                (blocks, outline, text_chars, block_truncated, text_truncated)
            } else {
                (Vec::new(), Vec::new(), 0, false, text_truncated)
            };

            let document_id = crate::core::identity::doc_id(
                Some(&final_url),
                raw_title.as_deref(),
                Some(doc_kind.as_str()),
            );
            let chunks = build_document_chunks(&document_id, &outline, &blocks, document_chars);

            Some(FetchDocument {
                doc_id: document_id.clone(),
                kind: doc_kind,
                render_format: RenderFormat::AgentBlocksV1,
                text_format: if extract_mode == ExtractMode::Markdown {
                    "markdown".to_string()
                } else {
                    "plain".to_string()
                },
                text_chars_returned: text_chars,
                text_truncated: window_text_truncated,
                block_truncated,
                document_truncated,
                link_truncated: links_truncated,
                metadata: Some(FetchRenderMetadata {
                    bytes_read: Some(body.len()),
                    content_length: content_length_header,
                    charset,
                    redirects_followed: redirect_count,
                    source_extension,
                    detected_language: detected.language,
                    markdown_tokens,
                    original_tokens,
                }),
                outline,
                blocks,
                chunks,
            })
        } else {
            None
        };

        let raw_text_chars_returned = raw_text.as_ref().map(|t| t.chars().count());
        let challenge = super::challenge::detect_html(kind.is_html, raw_title.as_deref(), &body);

        Ok(WebFetchResponse {
            url: url_str.to_string(),
            final_url,
            stable_id: Some(crate::core::identity::fetch_id(
                Some(url_str),
                None,
                None,
                None,
                None,
            )),
            source_id: None,
            title,
            description,
            content_type,
            representation,
            status,
            fetched: true,
            truncated,
            trust: FetchTrust::ExternalUntrusted,
            text,
            raw_text,
            raw_text_chars_returned,
            raw_text_truncated: raw_capped,
            raw_text_cap: Some(document_chars),
            links,
            links_seen: if links_seen > 0 {
                Some(links_seen)
            } else {
                None
            },
            links_truncated,
            warnings,
            trust_markers,
            document,
            fetch_transform,
            structured_warnings: super::challenge::warnings(challenge),
            pdf_page_metadata: None,
            pdf_document_metadata: None,
            pdf_quality_score: None,
            pdf_content_ok: None,
            cache_status: crate::fetch::cache::CacheStatus::default(),
            attempt_count: None,
            retry_after_ms: None,
            origin_backoff_ms: None,
            response_headers: if cache_headers.is_empty() {
                None
            } else {
                Some(cache_headers)
            },
            transport: Some("http".to_string()),
            browser_escalated: false,
            accept_policy: None,
            manual_interaction_required: challenge.is_some(),
            focus: None,
            raw_body: Some(body),
        })
    }

    /// Fetches a URL with optional conditional headers for cache revalidation.
    ///
    /// Returns the HTTP status, response headers as a HashMap, and the
    /// raw body bytes. Used by the cache layer for stale revalidation
    /// (304 Not Modified handling). `accept_policy` must be the policy
    /// stored in the raw cache key being revalidated, so the origin
    /// answers for the same negotiated representation.
    pub async fn fetch_conditional(
        &self,
        url_str: &str,
        conditional_headers: &[(String, String)],
        accept_policy: AcceptPolicy,
    ) -> Result<
        (
            u16,
            std::collections::HashMap<String, String>,
            Vec<u8>,
            bool,
        ),
        FetchError,
    > {
        let initial_url = validate_url(url_str, &self.limits)?;
        let code_host_target = resolve_code_host_fetch_target(url_str);
        let fetch_url = if let Some(ref target) = code_host_target {
            if let Some(ref raw_url) = target.raw_url {
                validate_url(raw_url, &self.limits)?
            } else {
                initial_url
            }
        } else {
            initial_url
        };

        let mut current_url = fetch_url;
        let mut redirect_count: usize = 0;
        let initial_origin = super::origin::OriginKey::from_url(&current_url);
        // Addresses resolved at the redirect site, carried into the next hop so
        // the loop head does not resolve the same host twice.
        let mut carried_addrs: Option<Vec<std::net::SocketAddr>> = None;

        let response = loop {
            let hop_origin = super::origin::OriginKey::from_url(&current_url);
            let crossed_origin = hop_origin.as_ref() != initial_origin.as_ref();
            let resolved_addrs = match carried_addrs.take() {
                Some(addrs) => Some(addrs),
                None => {
                    validate_fetch_target_with_resolved_addrs(&current_url, &self.limits).await?
                }
            };
            let builder = self
                .client
                .get(current_url.as_str())
                .map_err(|e| FetchError::InvalidUrl(e.to_string()))?;
            let builder = builder.timeout(client_timeout(self.limits.timeout_ms));
            let builder = match resolved_addrs {
                Some(addrs) if !addrs.is_empty() => builder.resolved_addresses(addrs),
                _ => builder,
            };
            let mut req = builder;
            req = req.header("accept", accept_policy.header_value());
            // ETag and Last-Modified are origin-scoped credentials for the cached
            // entry. Replaying them after a cross-origin redirect would disclose
            // one origin's cache state to another, so send them on the first hop
            // only.
            if !crossed_origin {
                for (name, value) in conditional_headers {
                    req = req.header(name.as_str(), value.as_str());
                }
            }

            let resp = req
                .send()
                .await
                .map_err(|e| map_send_error(e, self.limits.timeout_ms))?;

            let status = resp.status().as_u16();
            if status == 304 {
                if crossed_origin {
                    // A different origin cannot revalidate this origin's cached
                    // entry; treating its 304 as a fresh hit would attribute one
                    // origin's body to another.
                    return Err(FetchError::NetworkError(
                        "cross-origin 304 for a conditional revalidation".to_string(),
                    ));
                }
                break resp;
            }
            if (300..400).contains(&status) {
                let location = resp
                    .headers()
                    .get("location")
                    .and_then(|v| v.to_str().ok())
                    .map(|s| s.to_string());
                let location = match location {
                    Some(loc) if !loc.is_empty() => loc,
                    _ => {
                        return Err(FetchError::InvalidRedirectLocation(format!(
                            "HTTP {status} missing or empty Location header"
                        )));
                    }
                };
                let redirect_url = current_url.join(&location).map_err(|e| {
                    FetchError::InvalidRedirectLocation(format!(
                        "failed to resolve redirect location '{location}': {e}"
                    ))
                })?;
                redirect_count += 1;
                if redirect_count > self.limits.redirect_limit {
                    return Err(FetchError::RedirectLimitExceeded(redirect_count - 1));
                }
                carried_addrs = Some(
                    validate_fetch_target_with_resolved_addrs(&redirect_url, &self.limits)
                        .await
                        .map_err(map_redirect_validation_error)?
                        .unwrap_or_default(),
                );
                current_url = redirect_url;
                continue;
            }
            break resp;
        };

        let status = response.status().as_u16();
        let headers: std::collections::HashMap<String, String> = response
            .headers()
            .iter()
            .filter(|(k, _)| {
                matches!(
                    k.as_str(),
                    "etag"
                        | "last-modified"
                        | "cache-control"
                        | "expires"
                        | "vary"
                        | "x-markdown-tokens"
                        | "x-original-tokens"
                )
            })
            .filter_map(|(k, v)| {
                v.to_str()
                    .ok()
                    .map(|val| (k.as_str().to_string(), val.to_string()))
            })
            .collect();

        if status == 304 {
            return Ok((304, headers, Vec::new(), false));
        }

        if let Some(content_length_u64) = response
            .wire_content_length()
            .and_then(|s| s.parse::<u64>().ok())
            .or_else(|| response.content_length())
        {
            if content_length_u64 > self.limits.max_bytes as u64 {
                return Err(FetchError::ContentTooLarge(
                    usize::try_from(content_length_u64).unwrap_or(usize::MAX),
                    self.limits.max_bytes,
                ));
            }
        } else if response.wire_content_length().is_some()
            || response.headers().contains_key("content-length")
        {
            tracing::warn!(
                header = ?response.wire_content_length(),
                "unparseable content-length header; proceeding to streaming cap"
            );
        }

        let mut body = Vec::new();
        let mut response = response;
        let mut stream = response
            .bytes_stream()
            .map_err(|e| FetchError::NetworkError(e.to_string()))?;
        let mut truncated = false;
        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result.map_err(|e| FetchError::NetworkError(e.to_string()))?;
            truncated = append_bounded(&mut body, &chunk, self.limits.max_bytes);
            if truncated {
                break;
            }
        }
        Ok((status, headers, body, truncated))
    }
}

/// Join every `Set-Cookie` name/value pair into one `Cookie` header value.
fn set_cookie_pairs(headers: &http::HeaderMap) -> Option<String> {
    let mut pairs: Vec<String> = Vec::new();
    for value in headers.get_all("set-cookie") {
        let Ok(raw) = value.to_str() else {
            continue;
        };
        let Some(pair) = raw.split(';').next().map(str::trim) else {
            continue;
        };
        if pair.is_empty() || !pair.contains('=') {
            continue;
        }
        pairs.push(pair.to_string());
    }
    (!pairs.is_empty()).then(|| pairs.join("; "))
}

/// Sanitize a single field of untrusted text.
///
/// Tier 1 (`strip_control_chars` + `bound_text`) is always on. When
/// `sanitize_output = true`, Tier 2 (framing via `frame`) and Tier 3
/// (scanning for prompt-injection markers, with one warning pushed
/// per hit) are also applied.
///
/// Returns the (possibly framed) string and a `TrustMarkers` record
/// describing what was done. Marker warnings are pushed into
/// `warnings` in the form
/// `"possible prompt injection marker detected in {field}: {pattern}"`.
fn sanitize_field(
    text: &str,
    field: &str,
    id: &str,
    max_chars: usize,
    sanitize_output: bool,
    warnings: &mut Vec<String>,
) -> (String, TrustMarkers) {
    let mut m = TrustMarkers::default();

    // Tier 1: always on.
    let (stripped, removed) = strip_control_chars(text);
    m.control_chars_removed = removed;
    let (bounded, truncated) = bound_text(&stripped, max_chars);
    if truncated {
        m.text_truncated = true;
    }

    if sanitize_output {
        // Tier 3: scan the (stripped, bounded) text for injection
        // markers. Scan happens before framing so the warning text
        // describes the actual content, not the framing delimiters.
        let hits = scan_injection_markers(&bounded);
        m.injection_hits = hits.len();
        for hit in hits {
            warnings.push(format!(
                "possible prompt injection marker detected in {field}: {}",
                hit.pattern
            ));
        }

        // Tier 2: wrap in framing delimiters.
        m.text_framed = true;
        m.text_sanitized = true;
        (frame(&bounded, field, id), m)
    } else {
        if removed > 0 || truncated {
            m.text_sanitized = true;
        }
        (bounded, m)
    }
}

fn append_bounded(buf: &mut Vec<u8>, data: &[u8], max_bytes: usize) -> bool {
    let remaining = max_bytes.saturating_sub(buf.len());
    if data.len() > remaining {
        // Back off to the nearest char boundary so the truncated tail
        // does not split a multi-byte UTF-8 sequence.
        let mut take = remaining.min(data.len());
        while take > 0 && (data[take - 1] & 0xC0) == 0x80 {
            take -= 1;
        }
        if take > 0 {
            let lead = data[take - 1];
            let expected = match lead {
                0x00..=0x7f => 1,
                0xc2..=0xdf => 2,
                0xe0..=0xef => 3,
                0xf0..=0xf4 => 4,
                _ => 0,
            };
            if expected == 0 || expected > remaining.saturating_sub(take - 1) {
                take -= 1;
            }
        }
        if take > 0 {
            buf.extend_from_slice(&data[..take]);
        }
        return true;
    }
    buf.extend_from_slice(data);
    false
}

fn client_timeout(timeout_ms: u64) -> Timeout {
    let timeout = Duration::from_millis(timeout_ms);
    Timeout {
        pool: Some(timeout),
        connect: Some(timeout),
        write: Some(timeout),
        read: Some(timeout),
        total: Some(timeout),
    }
}

fn map_send_error(error: eggfetch_core::Error, timeout_ms: u64) -> FetchError {
    if matches!(
        error,
        eggfetch_core::Error::Timeout { .. } | eggfetch_core::Error::TransportIoTimeout { .. }
    ) {
        FetchError::Timeout(timeout_ms)
    } else {
        FetchError::NetworkError(error.to_string())
    }
}

fn map_redirect_validation_error(error: FetchError) -> FetchError {
    match error {
        FetchError::PrivateNetworkBlocked(reason) => {
            FetchError::RedirectTargetBlocked(format!("private network: {reason}"))
        }
        FetchError::EmbeddedCredentialsBlocked(reason) => {
            FetchError::RedirectTargetBlocked(format!("credentials: {reason}"))
        }
        FetchError::UnsupportedScheme(reason) => FetchError::RedirectTargetBlocked(reason),
        other => other,
    }
}

#[cfg(test)]
#[path = "client_tests.rs"]
mod tests;
