//! Bounded, same-origin ALTCHA challenge derivation and replay.

use std::time::{Duration, Instant};

use scraper::{Html, Selector};
use url::Url;

use crate::core::fetch::WebFetchResponse;
use crate::fetch::FetchClient;
use crate::meta::challenge::altcha as solver;
use crate::meta::challenge::ledger::{RecordedOutcome, RefusalReason};
use crate::meta::challenge::{
    SolveBudget, SolveOutcome, BOUND_MALFORMED, BOUND_UNSUPPORTED_SCHEME,
};

use super::{response_challenge, ChallengePlan, SolvedReplay, FETCH_LEDGER_LABEL};

/// Maximum characters accepted from a published endpoint attribute.
pub const MAX_ENDPOINT_CHARS: usize = 256;

/// Maximum bytes accepted from a published challenge document.
pub const MAX_DOCUMENT_BYTES: usize = 64 * 1024;

/// Canonical ALTCHA verification path, used when a page publishes none.
pub const DEFAULT_VERIFY_PATH: &str = "/captcha/verify";

/// Maximum path segments in a published endpoint.
const MAX_PATH_SEGMENTS: usize = 4;

/// Maximum characters in one published path segment.
const MAX_SEGMENT_CHARS: usize = 32;

/// The ALTCHA material a challenge page publishes.
pub struct PublishedAltcha {
    /// The published challenge document, when the page carries it inline.
    pub document: Option<String>,
    /// The published challenge endpoint, when the page publishes one.
    pub challenge_url: Option<String>,
    /// The verification endpoint. Always same-origin with the page.
    pub verify_url: String,
}

/// Which endpoint a published value names.
#[derive(Clone, Copy, PartialEq, Eq)]
enum EndpointKind {
    /// The endpoint that serves the published challenge document.
    Challenge,
    /// The endpoint that verifies a solution.
    Verify,
}

impl EndpointKind {
    /// The final path segment this endpoint must carry.
    const fn last_segment(self) -> &'static str {
        match self {
            Self::Challenge => "challenge",
            Self::Verify => "verify",
        }
    }
}

/// Derive the published ALTCHA material from a challenge page.
///
/// `page_url` is the URL the page was served from and `page` its body. Both
/// returned endpoints are same-origin with `page_url`.
pub fn derive(page_url: &str, page: &str) -> Result<PublishedAltcha, &'static str> {
    let base = Url::parse(page_url).map_err(|_| BOUND_UNSUPPORTED_SCHEME)?;
    if !matches!(base.scheme(), "http" | "https") {
        return Err(BOUND_UNSUPPORTED_SCHEME);
    }
    let (challenge_value, verify_value) = widget_attributes(page).ok_or(BOUND_MALFORMED)?;

    let challenge_value = challenge_value
        .as_deref()
        .map(str::trim)
        .unwrap_or_default();
    let (document, challenge_url) = if challenge_value.is_empty() {
        (None, None)
    } else if challenge_value.starts_with('{') {
        (Some(inline_document(challenge_value)?), None)
    } else {
        (
            None,
            Some(resolve_endpoint(
                &base,
                challenge_value,
                EndpointKind::Challenge,
            )?),
        )
    };

    let verify_url = match verify_value.as_deref().map(str::trim) {
        Some(value) if !value.is_empty() => resolve_endpoint(&base, value, EndpointKind::Verify)?,
        _ => derive_verify_url(&base, challenge_url.as_deref())?,
    };

    if document.is_none() && challenge_url.is_none() {
        return Err(BOUND_MALFORMED);
    }

    Ok(PublishedAltcha {
        document,
        challenge_url,
        verify_url,
    })
}

/// Solve the ALTCHA challenge published on `response` and replay the request once.
///
/// Returns the solved replay when the challenge was solved and the replay is
/// not itself a challenge page. Every other outcome returns `None`, so the
/// caller keeps the original response exactly as it was.
pub async fn replay(
    client: &FetchClient,
    url: &str,
    response: &WebFetchResponse,
    plan: &ChallengePlan<'_>,
) -> Option<SolvedReplay> {
    let settings = plan.challenge;
    let remaining = plan.deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        return None;
    }
    let page = response.raw_body.as_deref().unwrap_or_default();
    let published = match derive(&response.final_url, &String::from_utf8_lossy(page)) {
        Ok(published) => published,
        Err(bound) => {
            record_refusal(bound);
            return None;
        }
    };
    let timeout = Duration::from_millis(settings.timeout_ms).min(remaining);
    let solver_client = crate::meta::engines::build_http_client(Some(plan.user_agent)).ok()?;
    let mut budget = SolveBudget::new(settings.max_attempts, timeout, settings.max_difficulty);
    let outcome = match (&published.document, &published.challenge_url) {
        (Some(document), _) => {
            solver::solve_document_and_verify(
                &solver_client,
                FETCH_LEDGER_LABEL,
                document,
                &published.verify_url,
                timeout,
                &mut budget,
            )
            .await
        }
        (None, Some(challenge_url)) => {
            solver::solve_and_verify(
                &solver_client,
                FETCH_LEDGER_LABEL,
                challenge_url,
                &published.verify_url,
                timeout,
                &mut budget,
            )
            .await
        }
        (None, None) => return None,
    };
    let outcome = match outcome {
        Ok(outcome) => outcome,
        Err(_) => return None,
    };
    let SolveOutcome::Solved { cookie, lifetime } = outcome else {
        return None;
    };
    let replayed = client
        .fetch_with_cookie(
            url,
            plan.max_chars,
            plan.extract_mode,
            plan.include_links,
            plan.pdf_options,
            plan.origin_controller,
            Some(&cookie),
        )
        .await
        .ok()?;
    if response_challenge(&replayed).is_some() {
        return None;
    }
    Some(SolvedReplay {
        response: replayed,
        cookie,
        lifetime,
    })
}

fn record_refusal(bound: &'static str) {
    let Some(reason) = RefusalReason::from_bound(bound) else {
        return;
    };
    crate::meta::challenge::ledger::solve_ledger().record_outcome(
        FETCH_LEDGER_LABEL,
        RecordedOutcome::Refused(reason),
        0,
        Duration::ZERO,
    );
}

fn widget_attributes(page: &str) -> Option<(Option<String>, Option<String>)> {
    let selector = Selector::parse("altcha-widget").ok()?;
    let document = Html::parse_document(page);
    let element = document.select(&selector).next()?;
    let challenge = element
        .value()
        .attr("challenge")
        .or_else(|| element.value().attr("challengeurl"))
        .map(str::to_string);
    let verify = element
        .value()
        .attr("verifyurl")
        .or_else(|| element.value().attr("data-verify-url"))
        .map(str::to_string);
    Some((challenge, verify))
}

fn inline_document(value: &str) -> Result<String, &'static str> {
    if value.len() > MAX_DOCUMENT_BYTES {
        return Err(BOUND_MALFORMED);
    }
    Ok(value.to_string())
}

fn resolve_endpoint(base: &Url, value: &str, kind: EndpointKind) -> Result<String, &'static str> {
    if value.chars().count() > MAX_ENDPOINT_CHARS {
        return Err(BOUND_UNSUPPORTED_SCHEME);
    }
    let resolved = base.join(value).map_err(|_| BOUND_UNSUPPORTED_SCHEME)?;
    if resolved.scheme() != base.scheme() || !matches!(resolved.scheme(), "http" | "https") {
        return Err(BOUND_UNSUPPORTED_SCHEME);
    }
    if !resolved.username().is_empty() || resolved.password().is_some() {
        return Err(BOUND_UNSUPPORTED_SCHEME);
    }
    if resolved.query().is_some() || resolved.fragment().is_some() {
        return Err(BOUND_UNSUPPORTED_SCHEME);
    }
    if !same_origin(base, &resolved) {
        return Err(BOUND_UNSUPPORTED_SCHEME);
    }
    if !allowed_path(resolved.path(), kind) {
        return Err(BOUND_UNSUPPORTED_SCHEME);
    }
    Ok(resolved.to_string())
}

fn derive_verify_url(base: &Url, challenge_url: Option<&str>) -> Result<String, &'static str> {
    if let Some(challenge_url) = challenge_url {
        let challenge = Url::parse(challenge_url).map_err(|_| BOUND_UNSUPPORTED_SCHEME)?;
        let mut derived = challenge.clone();
        derived.set_path(&replaced_path(
            challenge.path(),
            EndpointKind::Verify.last_segment(),
        ));
        return Ok(derived.to_string());
    }
    let mut fallback = base.clone();
    fallback.set_path(DEFAULT_VERIFY_PATH);
    fallback.set_query(None);
    fallback.set_fragment(None);
    let _ = fallback.set_username("");
    let _ = fallback.set_password(None);
    Ok(fallback.to_string())
}

fn replaced_path(path: &str, segment: &str) -> String {
    let trimmed = path.strip_suffix('/').unwrap_or(path);
    let mut segments: Vec<&str> = trimmed.split('/').filter(|s| !s.is_empty()).collect();
    if let Some(last) = segments.last_mut() {
        *last = segment;
    }
    format!("/{}", segments.join("/"))
}

fn same_origin(base: &Url, other: &Url) -> bool {
    base.host_str() == other.host_str()
        && base.port_or_known_default() == other.port_or_known_default()
}

fn allowed_path(path: &str, kind: EndpointKind) -> bool {
    let trimmed = path.strip_suffix('/').unwrap_or(path);
    let segments: Vec<&str> = trimmed.split('/').filter(|s| !s.is_empty()).collect();
    if segments.is_empty() || segments.len() > MAX_PATH_SEGMENTS {
        return false;
    }
    if !segments
        .iter()
        .all(|segment| segment.len() <= MAX_SEGMENT_CHARS && segment.bytes().all(is_segment_byte))
    {
        return false;
    }
    segments.last().copied() == Some(kind.last_segment())
}

fn is_segment_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'~' | b'-')
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    const DOCUMENT: &str = r#"{"parameters":{"algorithm":"PBKDF2/SHA-256","cost":100,"keyLength":32,"keyPrefix":"aa","nonce":"0102030405060708","salt":"0a0b0c0d0e0f1011","keySignature":"00"},"signature":"00"}"#;

    fn page(attributes: &str) -> String {
        format!(
            r#"<html><body><div class="captcha-wrap"><altcha-widget {attributes}></altcha-widget></div></body></html>"#
        )
    }

    #[test]
    fn derives_an_inline_challenge_document() {
        let published = derive(
            "http://host.test/dir/page.html",
            &page(&format!(r#"challenge='{DOCUMENT}'"#)),
        )
        .expect("an inline document derives");
        assert_eq!(published.document.as_deref(), Some(DOCUMENT));
        assert!(published.challenge_url.is_none());
        assert_eq!(published.verify_url, "http://host.test/captcha/verify");
    }

    #[test]
    fn derives_a_published_challenge_endpoint() {
        let published = derive(
            "http://host.test/",
            &page(r#"challenge="/captcha/challenge""#),
        )
        .expect("derives");
        assert!(published.document.is_none());
        assert_eq!(
            published.challenge_url.as_deref(),
            Some("http://host.test/captcha/challenge")
        );
        assert_eq!(published.verify_url, "http://host.test/captcha/verify");
    }

    #[test]
    fn derives_the_verify_endpoint_from_the_published_prefix() {
        let published = derive(
            "http://host.test/",
            &page(r#"challenge="/altcha/challenge""#),
        )
        .expect("derives");
        assert_eq!(
            published.challenge_url.as_deref(),
            Some("http://host.test/altcha/challenge")
        );
        assert_eq!(published.verify_url, "http://host.test/altcha/verify");
    }

    #[test]
    fn accepts_a_published_verify_endpoint() {
        let published = derive(
            "http://host.test/",
            &page(r#"challenge="/captcha/challenge" verifyurl="/altcha/verify""#),
        )
        .expect("derives");
        assert_eq!(published.verify_url, "http://host.test/altcha/verify");
    }

    #[test]
    fn resolves_a_relative_challenge_path_against_the_page() {
        let published = derive(
            "http://host.test/dir/page.html",
            &page(r#"challenge="captcha/challenge""#),
        )
        .expect("derives");
        assert_eq!(
            published.challenge_url.as_deref(),
            Some("http://host.test/dir/captcha/challenge")
        );
    }

    #[test]
    fn rejects_every_cross_origin_or_malformed_endpoint() {
        for attribute in [
            r#"challenge="https://other.test/captcha/challenge""#,
            r#"challenge="//other.test/captcha/challenge""#,
            r#"challenge="ftp://host.test/captcha/challenge""#,
            r#"challenge="javascript:alert(1)""#,
            r#"challenge="http://user:secret@host.test/captcha/challenge""#,
            r#"challenge="/captcha/challenge?token=1""#,
            r#"challenge="/captcha/challenge#fragment""#,
            r#"challenge="/captcha/pow""#,
            r#"challenge="/captcha/challenge/extra""#,
            r#"challenge="/a/b/c/d/e/challenge""#,
            r#"challenge="/captcha/challenge.json""#,
        ] {
            assert_eq!(
                derive("http://host.test/", &page(attribute)).err(),
                Some(BOUND_UNSUPPORTED_SCHEME),
                "expected a refusal for {attribute}"
            );
        }
    }

    #[test]
    fn rejects_an_over_long_endpoint() {
        let attribute = format!(
            r#"challenge="/{}/challenge""#,
            "a".repeat(MAX_ENDPOINT_CHARS)
        );
        assert_eq!(
            derive("http://host.test/", &page(&attribute)).err(),
            Some(BOUND_UNSUPPORTED_SCHEME)
        );
    }

    #[test]
    fn rejects_a_page_without_the_widget() {
        let page = "<html><body><div class=\"captcha-wrap\">JavaScript is required to complete this challenge</div></body></html>";
        assert_eq!(
            derive("http://host.test/", page).err(),
            Some(BOUND_MALFORMED)
        );
    }

    #[test]
    fn rejects_a_widget_that_publishes_no_challenge() {
        assert_eq!(
            derive("http://host.test/", &page(r#"name="altcha""#)).err(),
            Some(BOUND_MALFORMED)
        );
    }

    #[test]
    fn rejects_an_over_long_inline_document() {
        let document = format!("{{{}}}", "a".repeat(MAX_DOCUMENT_BYTES));
        assert_eq!(
            derive(
                "http://host.test/",
                &page(&format!(r#"challenge='{document}'"#))
            )
            .err(),
            Some(BOUND_MALFORMED)
        );
    }

    #[test]
    fn rejects_a_non_http_page_url() {
        assert_eq!(
            derive(
                "ftp://host.test/",
                &page(r#"challenge="/captcha/challenge""#)
            )
            .err(),
            Some(BOUND_UNSUPPORTED_SCHEME)
        );
    }

    #[test]
    fn the_fallback_verify_path_drops_the_page_query() {
        let published = derive(
            "http://host.test/dir/page.html?q=1",
            &page(&format!(r#"challenge='{DOCUMENT}'"#)),
        )
        .expect("derives");
        assert_eq!(published.verify_url, "http://host.test/captcha/verify");
    }

    fn escape_attribute(value: &str) -> String {
        value.replace('&', "&amp;").replace('"', "&quot;")
    }

    fn widget_page(value: &str) -> String {
        format!(
            r#"<html><body><altcha-widget challenge="{}"></altcha-widget></body></html>"#,
            escape_attribute(value)
        )
    }

    proptest::proptest! {
        #[test]
        fn a_derived_endpoint_is_never_cross_origin(value in any::<String>()) {
            let page_url = "http://host.test/dir/page.html";
            let Ok(published) = derive(page_url, &widget_page(&value)) else {
                return Ok(());
            };
            let base = Url::parse(page_url).expect("page url parses");
            let endpoints = published
                .challenge_url
                .iter()
                .chain(std::iter::once(&published.verify_url));
            for endpoint in endpoints {
                let parsed = Url::parse(endpoint).expect("derived endpoint parses");
                prop_assert_eq!(parsed.scheme(), base.scheme());
                prop_assert_eq!(parsed.host_str(), base.host_str());
                prop_assert_eq!(
                    parsed.port_or_known_default(),
                    base.port_or_known_default()
                );
            }
        }
    }
}
