//! Markdown-first content negotiation for the fetch client.
//!
//! The public fetch entry points select one fixed `Accept` policy for the
//! target, run the request, and allow at most one bounded fallback to the
//! plain HTML policy when the negotiated representation is unusable.

use super::accept::{self, AcceptPolicy};
use super::client::FetchClient;
use super::detect;
use super::types::FetchError;
use crate::core::document::DocumentKind;
use crate::core::fetch::{ExtractMode, FetchRepresentation, WebFetchResponse};

/// Classification of the response body for the content extraction
/// pipeline. Returned by [`classify_content_type`] so both the
/// pre-body and post-body classification steps share a single source
/// of truth and cannot drift.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ContentKind {
    pub is_html: bool,
    pub is_pdf: bool,
    pub is_text: bool,
}

pub(crate) fn classify_content_type(
    content_type: Option<&str>,
    final_url: &str,
    body_starts_with_pdf: bool,
) -> ContentKind {
    let base = content_type
        .map(|ct| {
            let ct_lower = ct.to_lowercase();
            ct_lower.split(';').next().unwrap_or("").trim().to_string()
        })
        .unwrap_or_default();

    let is_html = base == "text/html" || base == "application/xhtml+xml";
    let is_pdf_by_ct = base == "application/pdf";
    let is_pdf_by_url = !is_pdf_by_ct
        && url::Url::parse(final_url)
            .ok()
            .map(|u| u.path().to_ascii_lowercase().ends_with(".pdf"))
            .unwrap_or(false);
    let mut is_pdf = is_pdf_by_ct || is_pdf_by_url;
    if !is_pdf && body_starts_with_pdf {
        is_pdf = true;
    }
    let is_text = base.starts_with("text/")
        || base == "application/json"
        || base == "application/ld+json"
        || (base.starts_with("application/") && base.ends_with("+json"))
        || base == "application/toml"
        || base == "application/x-yaml"
        || base == "application/yaml"
        || base == "application/javascript"
        || base == "application/typescript"
        || base == "application/x-sh"
        || base == "application/xml";

    ContentKind {
        is_html,
        is_pdf,
        is_text,
    }
}

/// Map a response body to the representation the origin served.
pub(crate) fn representation_of(
    content_type: Option<&str>,
    kind: ContentKind,
) -> Option<FetchRepresentation> {
    if kind.is_pdf {
        return None;
    }
    if detect::is_markdown_content_type(content_type.unwrap_or("")) {
        return Some(FetchRepresentation::Markdown);
    }
    kind.is_html.then_some(FetchRepresentation::Html)
}

pub(crate) fn parse_token_header(
    headers: &std::collections::HashMap<String, String>,
    name: &str,
) -> Option<usize> {
    headers
        .get(name)
        .and_then(|value| value.trim().parse::<usize>().ok())
}

impl FetchClient {
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn fetch_internal(
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
        let accept_policy = accept::select_accept_policy(url_str, extract_mode);
        if accept_policy == AcceptPolicy::Html {
            let (response, cookies) = self
                .fetch_once(
                    url_str,
                    max_chars,
                    extract_mode,
                    include_links,
                    pdf_options,
                    origin_controller,
                    AcceptPolicy::Html,
                    cookie,
                )
                .await?;
            return Ok((finish_fetch(response, AcceptPolicy::Html, 1), cookies));
        }

        match self
            .fetch_once(
                url_str,
                max_chars,
                extract_mode,
                include_links,
                pdf_options,
                origin_controller,
                AcceptPolicy::Markdown,
                cookie,
            )
            .await
        {
            Ok((response, cookies)) => {
                let response = finish_fetch(response, AcceptPolicy::Markdown, 1);
                if !needs_html_fallback(&response) {
                    return Ok((response, cookies));
                }
                match self
                    .fetch_once(
                        url_str,
                        max_chars,
                        extract_mode,
                        include_links,
                        pdf_options,
                        origin_controller,
                        AcceptPolicy::Html,
                        cookie,
                    )
                    .await
                {
                    Ok((fallback, fallback_cookies)) => Ok((
                        finish_fetch(fallback, AcceptPolicy::Html, 2),
                        fallback_cookies,
                    )),
                    Err(error) => {
                        let mut response = finish_fetch(response, AcceptPolicy::Markdown, 2);
                        response
                            .warnings
                            .push(format!("markdown_fallback_failed: {error}"));
                        Ok((response, cookies))
                    }
                }
            }
            Err(error) if falls_back_to_html(&error) => {
                let (fallback, cookies) = self
                    .fetch_once(
                        url_str,
                        max_chars,
                        extract_mode,
                        include_links,
                        pdf_options,
                        origin_controller,
                        AcceptPolicy::Html,
                        cookie,
                    )
                    .await?;
                Ok((finish_fetch(fallback, AcceptPolicy::Html, 2), cookies))
            }
            Err(error) => Err(error),
        }
    }
}

fn finish_fetch(
    mut response: WebFetchResponse,
    accept_policy: AcceptPolicy,
    attempt_count: usize,
) -> WebFetchResponse {
    response.accept_policy = Some(accept_policy);
    response.attempt_count = Some(attempt_count);
    response
}

/// A Markdown-first attempt falls back to the HTML policy only when the
/// negotiated representation is unusable: a negotiation rejection, an
/// unsupported content type, or a Markdown body that extracted to nothing. An
/// origin that ignored the header and served HTML or plain text already
/// returned a usable representation, so no second request is made.
fn needs_html_fallback(response: &WebFetchResponse) -> bool {
    if response.representation != Some(FetchRepresentation::Markdown) {
        return false;
    }
    let has_body_document = response
        .document
        .as_ref()
        .is_some_and(|document| document.kind != DocumentKind::Pdf);
    has_body_document && extracted_body_is_empty(response)
}

fn extracted_body_is_empty(response: &WebFetchResponse) -> bool {
    response
        .document
        .as_ref()
        .is_none_or(|document| document.blocks.is_empty())
        && response
            .raw_text
            .as_deref()
            .is_none_or(|text| text.trim().is_empty())
}

/// Errors where a different `Accept` value can change the outcome: a
/// negotiation rejection (406/415) or a body this build cannot classify at all
/// (for example `application/octet-stream` for what is really a page). Other
/// statuses describe the resource, auth, or origin health and would fail the
/// same way, so they are not retried under a second policy.
fn falls_back_to_html(error: &FetchError) -> bool {
    matches!(
        error,
        FetchError::HttpStatus(406 | 415, _) | FetchError::UnsupportedContentType(..)
    )
}
