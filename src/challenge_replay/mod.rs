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
//!
//! ADR-0018 refines that contract for one case. A stored pair set the upstream
//! refuses is presented once as a reuse probe; the probe is neither a solve
//! nor a replay of a solved pair set, because no solve produced it. The
//! refusal evicts the entry and the same call continues into the ordinary
//! solve path, so the call's single solve and single replay are spent on the
//! freshly issued pair set.
//!
//! The Anubis and ALTCHA branches share that sequence. The ALTCHA branch
//! lives in `altcha`, which owns the page-shaped endpoint derivation the
//! ALTCHA solve needs.

pub mod altcha;
pub mod cache;

use std::time::{Duration, Instant, SystemTime};

use crate::core::config::ChallengeSection;
use crate::core::fetch::{ExtractMode, FetchRepresentation, PdfFetchOptions, WebFetchResponse};
use crate::fetch::challenge::{detect_html, ChallengeKind};
use crate::fetch::origin::OriginController;
use crate::fetch::{FetchClient, FetchError};
use crate::meta::challenge::anubis;
use crate::meta::challenge::cookie::{self, PublishedLifetime};
use crate::meta::challenge::{SolveBudget, SolveOutcome};

use cache::{fetch_pass_cookie_cache, origin_key};

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

/// A solved challenge that produced a replayable pair set.
pub struct SolvedReplay {
    /// The replayed document.
    pub response: WebFetchResponse,
    /// The replayable `Cookie` header value the pass response issued.
    pub cookie: String,
    /// The lifetime the upstream published for it.
    pub lifetime: PublishedLifetime,
}

/// Solve the challenge published on `response` and replay the request once.
///
/// A stored pair set for the request's own origin is replayed before any
/// solve is attempted. A pair set the upstream refuses is evicted, and the
/// same call continues into the ordinary solve path with the challenge page it
/// already holds, so a caller never has to await a second fetch for the
/// document.
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
    if !plan.challenge.enabled {
        return None;
    }
    let kind = response_challenge(response)?;
    if !matches!(kind, ChallengeKind::Anubis | ChallengeKind::Altcha) {
        return None;
    }
    let origin = reuse_origin(url, response);
    if let Some(origin) = origin.as_deref() {
        if let Some(cookie) = fetch_pass_cookie_cache().get(origin) {
            match replay_cached(client, url, &cookie, origin, plan).await {
                CachedReplay::Replayed(replayed) => return Some(*replayed),
                CachedReplay::Refused => {}
                CachedReplay::Failed => return None,
            }
        }
    }
    let solved = match kind {
        ChallengeKind::Anubis => {
            replay_anubis(client, url, response, challenge_cookies, plan).await
        }
        ChallengeKind::Altcha => altcha::replay(client, url, response, plan).await,
        _ => None,
    }?;
    if let Some(origin) = origin.as_deref() {
        let ttl = cookie::reuse_ttl(
            solved.lifetime,
            SystemTime::now(),
            Duration::from_secs(plan.challenge.cookie_ttl_seconds),
        );
        fetch_pass_cookie_cache().put(origin, &solved.cookie, ttl);
    }
    Some(solved.response)
}

/// Origin key for reuse, when the challenge page came from that same origin.
///
/// A fetch that crossed an origin has no reusable key: the issuing origin
/// cannot be established without following the redirect, so nothing is stored
/// and nothing is reused.
fn reuse_origin(url: &str, response: &WebFetchResponse) -> Option<String> {
    let request = origin_key(url)?;
    let issuing = origin_key(&response.final_url)?;
    (request == issuing).then_some(request)
}

/// The outcome of a reuse attempt with a stored pair set.
enum CachedReplay {
    /// The upstream served the document for the stored pair set.
    Replayed(Box<WebFetchResponse>),
    /// The upstream answered with a challenge page, so the pair set is stale.
    Refused,
    /// The attempt could not complete, so the pair set stays untested.
    Failed,
}

/// Replay the request once with a stored pair set.
///
/// A replay the upstream answers with a challenge page evicts the entry, so
/// the next fetch starts cold and the refused pair set is never offered again.
async fn replay_cached(
    client: &FetchClient,
    url: &str,
    cookie: &str,
    origin: &str,
    plan: &ChallengePlan<'_>,
) -> CachedReplay {
    let replayed = match client
        .fetch_with_cookie(
            url,
            plan.max_chars,
            plan.extract_mode,
            plan.include_links,
            plan.pdf_options,
            plan.origin_controller,
            Some(cookie),
        )
        .await
    {
        Ok(replayed) => replayed,
        Err(_) => return CachedReplay::Failed,
    };
    if response_challenge(&replayed).is_some() {
        fetch_pass_cookie_cache().remove(origin);
        return CachedReplay::Refused;
    }
    CachedReplay::Replayed(Box::new(replayed))
}

async fn replay_anubis(
    client: &FetchClient,
    url: &str,
    response: &WebFetchResponse,
    challenge_cookies: Option<String>,
    plan: &ChallengePlan<'_>,
) -> Option<SolvedReplay> {
    let settings = plan.challenge;
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
