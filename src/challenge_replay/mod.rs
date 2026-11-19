//! Shared challenge solve-and-replay orchestration for every fetch surface.
//!
//! ADR-0017 assigns the detect, solve, replay, and classify sequence to one
//! module outside `src/fetch/**` and `src/meta/**`, so the MCP tool layer and
//! the CLI share a single implementation. `src/fetch/**` keeps detection and
//! marking only and never imports the solver.
//!
//! The contract is unchanged from the solve this module replaced: at most one
//! solve and one replay per fetch call, inside the operator's difficulty,
//! attempt, and wall-clock ceilings; the replayed cookie is sent on the first
//! hop only; and every other outcome leaves the original response untouched.

use std::time::{Duration, Instant};

use crate::core::config::ChallengeSection;
use crate::core::fetch::{ExtractMode, FetchRepresentation, PdfFetchOptions, WebFetchResponse};
use crate::fetch::challenge::{detect_html, ChallengeKind};
use crate::fetch::origin::OriginController;
use crate::fetch::{FetchClient, FetchError};
use crate::meta::challenge::anubis;
use crate::meta::challenge::{SolveBudget, SolveOutcome};

/// Ledger label recorded for every solve performed on the fetch path.
pub const FETCH_LEDGER_LABEL: &str = "fetch";

/// Everything one bounded challenge solve and replay needs.
pub struct ChallengePlan<'a> {
    /// Operator challenge configuration.
    pub challenge: &'a ChallengeSection,
    /// User agent used by the solver's own HTTP client.
    pub user_agent: &'a str,
    /// Origin controller applied to the replay request.
    pub origin_controller: Option<&'a OriginController>,
    /// Character budget for the replay request.
    pub max_chars: Option<usize>,
    /// Extraction mode for the replay request.
    pub extract_mode: ExtractMode,
    /// Whether the replay extracts links.
    pub include_links: bool,
    /// PDF options for the replay request.
    pub pdf_options: Option<&'a PdfFetchOptions>,
    /// Deadline for the whole call.
    pub deadline: Instant,
}

/// The challenge family a response body presents, if any.
///
/// Only an HTML representation can carry a challenge page.
#[must_use]
pub fn detect_challenge(
    representation: Option<FetchRepresentation>,
    title: Option<&str>,
    body: &[u8],
) -> Option<ChallengeKind> {
    detect_html(
        representation == Some(FetchRepresentation::Html),
        title,
        body,
    )
}

/// The challenge family a fetched response presents, if any.
#[must_use]
pub fn response_challenge(response: &WebFetchResponse) -> Option<ChallengeKind> {
    detect_challenge(
        response.representation,
        response.title.as_deref(),
        response.raw_body.as_deref().unwrap_or_default(),
    )
}

/// Solve the challenge published on `response` and replay the request once.
///
/// Returns the replayed response when the challenge was solved and the replay
/// is not itself a challenge page. Every other outcome — solving disabled, no
/// parseable challenge, a refusal at any ceiling, an error, an exhausted
/// deadline, or a challenged replay — returns `None`, so the caller keeps the
/// original response exactly as it was.
pub async fn replay_challenge(
    client: &FetchClient,
    url: &str,
    response: &WebFetchResponse,
    challenge_cookies: Option<String>,
    plan: &ChallengePlan<'_>,
) -> Option<WebFetchResponse> {
    let settings = plan.challenge;
    if !settings.enabled {
        return None;
    }
    if response_challenge(response) != Some(ChallengeKind::Anubis) {
        return None;
    }
    let remaining = plan.deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        return None;
    }
    let challenge_page = response.raw_body.as_deref().unwrap_or_default();
    let parsed = url::Url::parse(&response.final_url).ok()?;
    let base_url = format!("{}://{}", parsed.scheme(), parsed.authority());
    let timeout = Duration::from_millis(settings.timeout_ms).min(remaining);
    let solver_client = crate::meta::engines::build_http_client(Some(plan.user_agent)).ok()?;
    let mut budget = SolveBudget::new(settings.max_attempts, timeout, settings.max_difficulty);
    let outcome = anubis::solve_and_verify(
        &solver_client,
        FETCH_LEDGER_LABEL,
        &base_url,
        &String::from_utf8_lossy(challenge_page),
        &response.final_url,
        challenge_cookies.as_deref().unwrap_or_default(),
        timeout,
        &mut budget,
    )
    .await
    .ok()?;
    let SolveOutcome::Solved { cookie, .. } = outcome else {
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
    Some(replayed)
}

/// Fetch `url`, then attempt one bounded challenge solve and replay.
///
/// The returned response is the replayed document when a solve succeeded, and
/// the original response otherwise.
pub async fn fetch_with_challenge_replay(
    client: &FetchClient,
    url: &str,
    plan: &ChallengePlan<'_>,
) -> Result<WebFetchResponse, FetchError> {
    let (response, challenge_cookies) = client
        .fetch_capturing(
            url,
            plan.max_chars,
            plan.extract_mode,
            plan.include_links,
            plan.pdf_options,
            plan.origin_controller,
            None,
        )
        .await?;
    Ok(
        replay_challenge(client, url, &response, challenge_cookies, plan)
            .await
            .unwrap_or(response),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const ANUBIS_BODY: &[u8] =
        br#"<html><head><script id="anubis_challenge" type="application/json">{}</script></head><body></body></html>"#;

    #[test]
    fn detects_anubis_only_on_html() {
        assert_eq!(
            detect_challenge(Some(FetchRepresentation::Html), None, ANUBIS_BODY),
            Some(ChallengeKind::Anubis)
        );
        assert_eq!(
            detect_challenge(Some(FetchRepresentation::Markdown), None, ANUBIS_BODY),
            None
        );
        assert_eq!(detect_challenge(None, None, ANUBIS_BODY), None);
    }

    #[test]
    fn leaves_ordinary_pages_alone() {
        assert_eq!(
            detect_challenge(
                Some(FetchRepresentation::Html),
                Some("Rust Programming Language"),
                b"<html><body>Rust is fast.</body></html>"
            ),
            None
        );
    }

    #[test]
    fn labels_every_fetch_path_solve_the_same() {
        assert_eq!(FETCH_LEDGER_LABEL, "fetch");
    }
}
