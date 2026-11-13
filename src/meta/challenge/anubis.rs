//! Anubis proof-of-work solver for the `fast` and `preact` methods.

use std::time::{Duration, Instant};

use eggfetch_core::{Client, RedirectPolicy};
use scraper::Html;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use url::Url;

use super::ledger::RefusalReason;
use super::{
    blocked_step, cookie, decided_step, finish_solve, refused_step, SolveBudget, SolveOutcome,
    SolveStep, TerminalOutcome, BOUND_ATTEMPTS, BOUND_BASE_PREFIX, BOUND_COOKIE_REJECTED,
    BOUND_MALFORMED, BOUND_MISSING_COOKIE, BOUND_SPENT, BOUND_UNSUPPORTED_SCHEME, BOUND_WALL_CLOCK,
};
use crate::meta::engines::error::EngineError;
use crate::meta::engines::{engine_timeout, map_request_error, read_bounded_body};

/// Anubis `fast` method name.
pub const METHOD_FAST: &str = "fast";
/// Anubis `slow` method name, the deprecated alias of `fast` with an identical scheme.
pub const METHOD_SLOW: &str = "slow";
/// Anubis `preact` method name.
pub const METHOD_PREACT: &str = "preact";
/// Anubis `metarefresh` method name, a time challenge that performs no hashing.
pub const METHOD_METAREFRESH: &str = "metarefresh";
/// Script element id carrying the `fast` challenge document.
pub const FAST_SCRIPT_ID: &str = "anubis_challenge";
/// Script element id carrying the `preact` challenge document.
pub const PREACT_SCRIPT_ID: &str = "preact_info";
/// Script element id carrying the published path prefix.
pub const BASE_PREFIX_SCRIPT_ID: &str = "anubis_base_prefix";
/// Pass-challenge endpoint path, relative to the upstream origin.
pub const PASS_CHALLENGE_PATH: &str = "/.within.website/x/cmd/anubis/api/pass-challenge";
/// Pass cookie the endpoint sets on success.
pub const PASS_COOKIE_NAME: &str = "spchal-auth";
/// Fixed client timing field submitted with every `fast` solution.
pub const ELAPSED_TIME: u64 = 1;
/// Published minimum elapsed time per difficulty unit, in seconds.
pub const PREACT_MIN_ELAPSED_PER_DIFFICULTY: f64 = 0.08;
/// Release-build nanoseconds per SHA-256 candidate, from the measured difficulty-6 solve
/// recorded in `plans/closure/agentic-retrieval-integrity/011-status.md`: 23,070,878
/// candidates in 3.05 s on one core is 0.13 µs, rounded to a whole nanosecond.
pub const CANDIDATE_NANOS: u64 = 130;

const MAX_PASS_BYTES: usize = 16 * 1024;

/// Anubis challenge method this build solves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnubisMethod {
    /// SHA-256 nonce search over the published `randomData`.
    Fast,
    /// Single SHA-256 digest of the published challenge string.
    Preact,
    /// Meta-refresh time challenge: echo the published token after the published wait.
    Metarefresh,
}

/// One solved Anubis challenge.
#[derive(Clone, PartialEq, Eq)]
pub struct AnubisSolution {
    /// Lowercase hex digest submitted as `response` and `result`.
    pub response: String,
    /// Nonce that produced the digest; `0` for `preact`.
    pub nonce: u64,
}

impl std::fmt::Debug for AnubisSolution {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AnubisSolution")
            .field("response", &"<redacted>")
            .field("nonce", &"<redacted>")
            .finish()
    }
}

/// A parsed, not-yet-solved Anubis challenge.
pub struct AnubisChallenge {
    method: AnubisMethod,
    difficulty: u8,
    id: String,
    random_data: String,
    challenge: String,
    redir: Option<String>,
    base_prefix: String,
    wait: Option<Duration>,
    target: Option<String>,
}

impl std::fmt::Debug for AnubisChallenge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AnubisChallenge")
            .field("method", &self.method)
            .field("difficulty", &self.difficulty)
            .field("id", &"<redacted>")
            .field("random_data", &"<redacted>")
            .field("challenge", &"<redacted>")
            .field("redir", &"<redacted>")
            .field("base_prefix", &"<redacted>")
            .field("target", &"<redacted>")
            .field("wait", &self.wait)
            .finish()
    }
}

impl AnubisChallenge {
    /// Parse a published Anubis challenge page.
    pub fn parse(html: &str) -> Result<Self, &'static str> {
        let document = Html::parse_document(html);
        let base_prefix = parse_base_prefix(&document)?;
        let fast = script_payload(&document, FAST_SCRIPT_ID)?;
        let preact = script_payload(&document, PREACT_SCRIPT_ID)?;

        match (fast, preact) {
            (Some(fast), preact) => {
                let object = fast.as_object().ok_or(BOUND_MALFORMED)?;
                let rules = object
                    .get("rules")
                    .and_then(Value::as_object)
                    .ok_or(BOUND_MALFORMED)?;
                let difficulty = numeric_difficulty(rules.get("difficulty"))?;
                let challenge = object
                    .get("challenge")
                    .and_then(Value::as_object)
                    .ok_or(BOUND_MALFORMED)?;
                let method = challenge
                    .get("method")
                    .and_then(Value::as_str)
                    .ok_or(BOUND_MALFORMED)?;
                refuse_spent(challenge)?;
                match method {
                    METHOD_FAST | METHOD_SLOW => Ok(Self {
                        method: AnubisMethod::Fast,
                        difficulty,
                        id: required_str(challenge, "id")?,
                        random_data: required_str(challenge, "randomData")?,
                        challenge: String::new(),
                        redir: optional_str(challenge, "redir"),
                        base_prefix,
                        wait: None,
                        target: None,
                    }),
                    METHOD_PREACT => {
                        Self::from_preact_info(preact.ok_or(BOUND_MALFORMED)?, base_prefix)
                    }
                    METHOD_METAREFRESH => {
                        let (wait, target) = published_meta_refresh(&document)?;
                        Self::from_metarefresh(challenge, base_prefix, difficulty, wait, target)
                    }
                    _ => Err(BOUND_UNSUPPORTED_SCHEME),
                }
            }
            (None, Some(preact)) => Self::from_preact_info(preact, base_prefix),
            (None, None) => Err(BOUND_MALFORMED),
        }
    }

    fn from_preact_info(value: Value, base_prefix: String) -> Result<Self, &'static str> {
        let object = value.as_object().ok_or(BOUND_MALFORMED)?;
        refuse_spent(object)?;
        Ok(Self {
            method: AnubisMethod::Preact,
            difficulty: numeric_difficulty(object.get("difficulty"))?,
            id: String::new(),
            random_data: String::new(),
            challenge: required_str(object, "challenge")?,
            redir: optional_str(object, "redir"),
            base_prefix,
            wait: None,
            target: None,
        })
    }

    fn from_metarefresh(
        object: &Map<String, Value>,
        base_prefix: String,
        difficulty: u8,
        wait: Duration,
        target: Option<String>,
    ) -> Result<Self, &'static str> {
        refuse_spent(object)?;
        if let Some(value) = object.get("wait") {
            let seconds = value.as_f64().ok_or(BOUND_MALFORMED)?;
            let published = Duration::try_from_secs_f64(seconds).map_err(|_| BOUND_MALFORMED)?;
            if published != wait {
                return Err(BOUND_MALFORMED);
            }
        }
        Ok(Self {
            method: AnubisMethod::Metarefresh,
            difficulty,
            id: optional_str(object, "id").unwrap_or_default(),
            random_data: String::new(),
            challenge: required_str(object, "challenge")?,
            redir: optional_str(object, "redir"),
            base_prefix,
            wait: Some(wait),
            target,
        })
    }

    /// Method this challenge publishes.
    #[must_use]
    pub fn method(&self) -> AnubisMethod {
        self.method
    }

    /// Difficulty the upstream published for this request.
    #[must_use]
    pub fn difficulty(&self) -> u8 {
        self.difficulty
    }

    /// Redirect target for this attempt, validated before it reaches a query string.
    ///
    /// A live Anubis `fast` challenge publishes no `redir` at all, so the
    /// original request URL is the only correct fallback; an absent value is
    /// not a malformed challenge. A published value is accepted only when it is
    /// same-origin absolute or a root-relative path.
    fn resolved_redir(&self, base_url: &str, request_url: &str) -> String {
        let Some(candidate) = self.redir.as_deref().filter(|value| !value.is_empty()) else {
            return request_url.to_string();
        };
        if candidate
            .chars()
            .any(|character| character.is_control() || character.is_whitespace())
        {
            return request_url.to_string();
        }
        if let Ok(base) = Url::parse(base_url) {
            if let Ok(absolute) = Url::parse(candidate) {
                return if same_origin(&absolute, &base) {
                    candidate.to_string()
                } else {
                    request_url.to_string()
                };
            }
        }
        if candidate.starts_with('/')
            && !candidate.starts_with("//")
            && !candidate.contains('\\')
            && !candidate.split('/').any(|segment| segment == "..")
        {
            return candidate.to_string();
        }
        request_url.to_string()
    }

    /// Resolve the published submission target against the challenge origin.
    fn resolved_target(&self, base_url: &str) -> Result<String, &'static str> {
        let target = self
            .target
            .as_deref()
            .or(self.redir.as_deref())
            .ok_or(BOUND_MALFORMED)?;
        let base = Url::parse(base_url).map_err(|_| BOUND_MALFORMED)?;
        let resolved = base.join(target).map_err(|_| BOUND_MALFORMED)?;
        if !same_origin(&resolved, &base) {
            return Err(BOUND_MALFORMED);
        }
        Ok(resolved.to_string())
    }

    /// Echo only the published token, identifier, and target.
    fn metarefresh_url(&self, target: &str) -> String {
        let mut url = target.to_string();
        if !self.id.is_empty() && !target.contains("id=") {
            url.push(if url.contains('?') { '&' } else { '?' });
            url.push_str("id=");
            url.push_str(&urlencoding::encode(&self.id));
        }
        url.push(if url.contains('?') { '&' } else { '?' });
        url.push_str("result=");
        url.push_str(&urlencoding::encode(&self.challenge));
        url
    }

    /// Compare the published difficulty with the configured ceiling.
    ///
    /// The failure names both values: it is the operator-visible knob.
    pub fn check_ceiling(&self, budget: &SolveBudget) -> Result<(), String> {
        if budget.allows_difficulty(self.difficulty) {
            Ok(())
        } else {
            Err(ceiling_refusal(self.difficulty, budget.max_difficulty()))
        }
    }

    /// Refuse a cost the configured budget cannot pay before any hashing starts.
    pub fn check_projection(&self, budget: &SolveBudget) -> Result<(), String> {
        let expected = expected_candidates(self.difficulty);
        if expected > u128::from(budget.max_attempts()) {
            return Err(format!(
                "anubis difficulty {} projects {} candidates, beyond the configured attempt ceiling {}",
                self.difficulty,
                compact_count(expected),
                budget.max_attempts()
            ));
        }
        let projected_nanos = expected.saturating_mul(u128::from(CANDIDATE_NANOS));
        if projected_nanos > budget.remaining_time().as_nanos() {
            return Err(format!(
                "anubis difficulty {} projects {} candidates (~{} ms), beyond the configured wall-clock budget {} ms",
                self.difficulty,
                compact_count(expected),
                projected_nanos / 1_000_000,
                budget.timeout().as_millis()
            ));
        }
        Ok(())
    }

    /// Solve the challenge under `budget`.
    pub fn solve(&self, budget: &mut SolveBudget) -> Result<AnubisSolution, &'static str> {
        match self.method {
            AnubisMethod::Fast => self.solve_fast(budget),
            AnubisMethod::Preact => Ok(AnubisSolution {
                response: sha256_hex(self.challenge.as_bytes()),
                nonce: 0,
            }),
            AnubisMethod::Metarefresh => Ok(AnubisSolution {
                response: self.challenge.clone(),
                nonce: 0,
            }),
        }
    }

    fn solve_fast(&self, budget: &mut SolveBudget) -> Result<AnubisSolution, &'static str> {
        let seed = self.random_data.as_bytes();
        let mut digits = [0u8; 20];
        let mut hasher = Sha256::new();
        let mut nonce: u64 = 0;
        loop {
            budget.attempt()?;
            hasher.update(seed);
            hasher.update(decimal_digits(nonce, &mut digits));
            let digest = hasher.finalize_reset();
            if has_leading_zero_hex(&digest, self.difficulty) {
                return Ok(AnubisSolution {
                    response: encode_hex(&digest),
                    nonce,
                });
            }
            nonce = nonce.checked_add(1).ok_or(BOUND_ATTEMPTS)?;
        }
    }
}

/// Exact operator-visible sentence for a challenge above the configured ceiling.
#[must_use]
pub fn ceiling_refusal(difficulty: u8, ceiling: u8) -> String {
    format!("anubis difficulty {difficulty} exceeds configured ceiling {ceiling}")
}

/// Expected SHA-256 candidates for a difficulty, as `16^difficulty`.
#[must_use]
pub fn expected_candidates(difficulty: u8) -> u128 {
    16u128.pow(u32::from(difficulty))
}

/// Render a candidate count with the largest unit that keeps a mantissa below 1000.
#[must_use]
pub fn compact_count(count: u128) -> String {
    const UNITS: [(u128, &str); 4] = [
        (1_000, "K"),
        (1_000_000, "M"),
        (1_000_000_000, "G"),
        (1_000_000_000_000, "T"),
    ];
    if count < 1_000 {
        return count.to_string();
    }
    let index = UNITS
        .iter()
        .rposition(|(unit, _)| count >= *unit)
        .unwrap_or(0);
    let (mut unit, mut suffix) = UNITS[index];
    let mut mantissa = (count + unit / 2) / unit;
    if mantissa >= 1_000 {
        if let Some((next_unit, next_suffix)) = UNITS.get(index + 1) {
            unit = *next_unit;
            suffix = next_suffix;
            mantissa = (count + unit / 2) / unit;
        }
    }
    format!("{mantissa}{suffix}")
}

/// Collect every `Set-Cookie` name/value pair into one `Cookie` header value.
#[must_use]
pub fn challenge_cookies(headers: &http::HeaderMap) -> String {
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
    pairs.join("; ")
}

/// Parse, solve, and verify one Anubis challenge, capturing the pass cookie.
///
/// `base_url` is the origin of the request that received the challenge and
/// `challenge_cookies` every cookie that response set. A challenge above the
/// configured ceiling is refused before any hashing work starts.
#[allow(clippy::too_many_arguments)]
pub async fn solve_and_verify(
    client: &Client,
    engine: &'static str,
    base_url: &str,
    challenge_page: &str,
    request_url: &str,
    challenge_cookies: &str,
    timeout: Duration,
    budget: &mut SolveBudget,
) -> Result<SolveOutcome, EngineError> {
    let step = solve_and_verify_inner(
        client,
        engine,
        base_url,
        challenge_page,
        request_url,
        challenge_cookies,
        timeout,
        budget,
    )
    .await;
    finish_solve(engine, step, budget.attempts_used())
}

#[allow(clippy::too_many_arguments)]
async fn solve_and_verify_inner(
    client: &Client,
    engine: &'static str,
    base_url: &str,
    challenge_page: &str,
    request_url: &str,
    challenge_cookies: &str,
    timeout: Duration,
    budget: &mut SolveBudget,
) -> SolveStep {
    let challenge = match AnubisChallenge::parse(challenge_page) {
        Ok(challenge) => challenge,
        Err(bound) => return refused_step(bound),
    };
    if let Err(reason) = challenge.check_ceiling(budget) {
        return blocked_step(reason, RefusalReason::DifficultyAboveCeiling);
    }
    if challenge.method == AnubisMethod::Fast {
        if let Err(reason) = challenge.check_projection(budget) {
            return blocked_step(reason, RefusalReason::UnsafeBasePrefix);
        }
    }

    let started = Instant::now();
    let deadline = started + timeout;
    let target = match challenge.method {
        AnubisMethod::Metarefresh => match challenge.resolved_target(base_url) {
            Ok(target) => Some(target),
            Err(bound) => return decided_step(TerminalOutcome::Refused { bound }, started),
        },
        AnubisMethod::Fast | AnubisMethod::Preact => None,
    };
    match challenge.method {
        AnubisMethod::Preact => match preact_wait(challenge.difficulty, deadline) {
            Ok(wait) => tokio::time::sleep(wait).await,
            Err(bound) => return decided_step(TerminalOutcome::Refused { bound }, started),
        },
        AnubisMethod::Metarefresh => {
            let Some(requirement) = challenge.wait else {
                return decided_step(
                    TerminalOutcome::Refused {
                        bound: BOUND_MALFORMED,
                    },
                    started,
                );
            };
            match metarefresh_wait(requirement, deadline) {
                Ok(wait) => tokio::time::sleep(wait).await,
                Err(bound) => return decided_step(TerminalOutcome::Refused { bound }, started),
            }
        }
        AnubisMethod::Fast => {}
    }

    let solution = match challenge.solve(budget) {
        Ok(solution) => solution,
        Err(bound) => return decided_step(TerminalOutcome::Refused { bound }, started),
    };
    let elapsed = started.elapsed();
    match verify(
        client,
        engine,
        base_url,
        &challenge,
        request_url,
        &solution,
        challenge_cookies,
        deadline,
        target.as_deref(),
    )
    .await
    {
        Ok(terminal) => SolveStep::Terminal { terminal, elapsed },
        Err(error) => SolveStep::Error(error),
    }
}

#[allow(clippy::too_many_arguments)]
async fn verify(
    client: &Client,
    engine: &'static str,
    base_url: &str,
    challenge: &AnubisChallenge,
    request_url: &str,
    solution: &AnubisSolution,
    challenge_cookies: &str,
    deadline: Instant,
    target: Option<&str>,
) -> Result<TerminalOutcome, EngineError> {
    let redir = challenge.resolved_redir(base_url, request_url);
    let url = match challenge.method {
        AnubisMethod::Fast => pass_challenge_url(base_url, &challenge.base_prefix),
        AnubisMethod::Preact => preact_url(base_url, &redir, &solution.response),
        AnubisMethod::Metarefresh => {
            let Some(target) = target else {
                return Ok(TerminalOutcome::Refused {
                    bound: BOUND_MALFORMED,
                });
            };
            challenge.metarefresh_url(target)
        }
    };
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        return Ok(TerminalOutcome::Refused {
            bound: BOUND_WALL_CLOCK,
        });
    }
    let mut request = client
        .get(&url)
        .map_err(|source| EngineError::Http { engine, source })?
        .redirect_policy(RedirectPolicy::new(false, 0))
        .timeout(engine_timeout(remaining));
    if challenge.method == AnubisMethod::Fast {
        let nonce = solution.nonce.to_string();
        let elapsed = ELAPSED_TIME.to_string();
        request = request
            .query("id", &challenge.id)
            .query("response", &solution.response)
            .query("nonce", &nonce)
            .query("redir", &redir)
            .query("elapsedTime", &elapsed);
    }
    if !challenge_cookies.is_empty() {
        request = request.header("Cookie", challenge_cookies);
    }

    let resp = request
        .send()
        .await
        .map_err(|error| map_request_error(engine, error))?;
    let status = resp.status();
    let issued = cookie::parse_issued(resp.headers(), PASS_COOKIE_NAME);
    read_bounded_body(resp, engine, MAX_PASS_BYTES).await?;

    if let Some((cookie, lifetime)) = issued {
        return Ok(TerminalOutcome::Solved { cookie, lifetime });
    }
    let bound = if status.is_success() {
        BOUND_MISSING_COOKIE
    } else {
        BOUND_COOKIE_REJECTED
    };
    Ok(TerminalOutcome::Refused { bound })
}

fn metarefresh_wait(requirement: Duration, deadline: Instant) -> Result<Duration, &'static str> {
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() || requirement > remaining {
        return Err(BOUND_WALL_CLOCK);
    }
    Ok(requirement)
}

fn published_meta_refresh(document: &Html) -> Result<(Duration, Option<String>), &'static str> {
    let selector = scraper::Selector::parse("meta[http-equiv]").map_err(|_| BOUND_MALFORMED)?;
    let content = document
        .select(&selector)
        .find(|element| {
            element
                .value()
                .attr("http-equiv")
                .is_some_and(|value| value.eq_ignore_ascii_case("refresh"))
        })
        .and_then(|element| element.value().attr("content"))
        .ok_or(BOUND_MALFORMED)?;
    let (seconds, tail) = content
        .split_once(';')
        .map_or((content, None), |(first, rest)| (first, Some(rest)));
    let seconds = seconds.trim().parse::<f64>().map_err(|_| BOUND_MALFORMED)?;
    let wait = Duration::try_from_secs_f64(seconds).map_err(|_| BOUND_MALFORMED)?;
    let target = tail.and_then(|rest| {
        let lower = rest.to_ascii_lowercase();
        let index = lower.find("url=")?;
        let value = rest[index + 4..]
            .trim()
            .trim_matches(|character| character == '"' || character == '\'');
        (!value.is_empty()).then(|| value.to_string())
    });
    Ok((wait, target))
}

fn preact_wait(difficulty: u8, deadline: Instant) -> Result<Duration, &'static str> {
    let published =
        Duration::from_secs_f64(PREACT_MIN_ELAPSED_PER_DIFFICULTY * f64::from(difficulty));
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() || published > remaining {
        return Err(BOUND_WALL_CLOCK);
    }
    Ok(published)
}

fn pass_challenge_url(base_url: &str, base_prefix: &str) -> String {
    format!(
        "{}{base_prefix}{PASS_CHALLENGE_PATH}",
        base_url.trim_end_matches('/')
    )
}

fn same_origin(candidate: &Url, base: &Url) -> bool {
    candidate.scheme() == base.scheme()
        && candidate.host_str() == base.host_str()
        && candidate.port_or_known_default() == base.port_or_known_default()
}

fn parse_base_prefix(document: &Html) -> Result<String, &'static str> {
    let Some(value) = script_payload(document, BASE_PREFIX_SCRIPT_ID)? else {
        return Ok(String::new());
    };
    let raw = value.as_str().ok_or(BOUND_MALFORMED)?;
    validate_base_prefix(raw)
}

fn validate_base_prefix(raw: &str) -> Result<String, &'static str> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(String::new());
    }
    if trimmed.len() > 256
        || !trimmed.starts_with('/')
        || trimmed.starts_with("//")
        || trimmed.contains("//")
        || trimmed.contains([':', '?', '#', '\\'])
        || trimmed
            .chars()
            .any(|character| character.is_ascii_control() || character.is_whitespace())
        || trimmed.split('/').any(|segment| segment == "..")
    {
        return Err(BOUND_BASE_PREFIX);
    }
    Ok(trimmed.strip_suffix('/').unwrap_or(trimmed).to_string())
}

fn refuse_spent(object: &Map<String, Value>) -> Result<(), &'static str> {
    match object.get("spent") {
        None | Some(Value::Bool(false)) => Ok(()),
        Some(Value::Bool(true)) => Err(BOUND_SPENT),
        Some(_) => Err(BOUND_MALFORMED),
    }
}

fn preact_url(base_url: &str, redir: &str, result: &str) -> String {
    let mut url = if redir.starts_with("http://") || redir.starts_with("https://") {
        redir.to_string()
    } else {
        format!("{}{redir}", base_url.trim_end_matches('/'))
    };
    url.push(if url.contains('?') { '&' } else { '?' });
    url.push_str("result=");
    url.push_str(result);
    url
}

fn script_payload(document: &Html, id: &str) -> Result<Option<Value>, &'static str> {
    let selector =
        scraper::Selector::parse(&format!("script#{id}")).map_err(|_| BOUND_MALFORMED)?;
    let Some(element) = document.select(&selector).next() else {
        return Ok(None);
    };
    let raw = element.text().collect::<String>();
    let value = serde_json::from_str(raw.trim()).map_err(|_| BOUND_MALFORMED)?;
    Ok(Some(value))
}

fn numeric_difficulty(value: Option<&Value>) -> Result<u8, &'static str> {
    let number = value
        .and_then(Value::as_u64)
        .and_then(|n| u8::try_from(n).ok())
        .ok_or(BOUND_MALFORMED)?;
    Ok(number)
}

fn required_str(object: &Map<String, Value>, key: &str) -> Result<String, &'static str> {
    let value = object
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or(BOUND_MALFORMED)?;
    Ok(value.to_string())
}

fn optional_str(object: &Map<String, Value>, key: &str) -> Option<String> {
    object
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn decimal_digits(mut nonce: u64, buf: &mut [u8; 20]) -> &[u8] {
    if nonce == 0 {
        buf[19] = b'0';
        return &buf[19..];
    }
    let mut index = buf.len();
    while nonce > 0 {
        index -= 1;
        buf[index] = b'0' + (nonce % 10) as u8;
        nonce /= 10;
    }
    &buf[index..]
}

fn has_leading_zero_hex(digest: &[u8], zeros: u8) -> bool {
    let zeros = usize::from(zeros);
    let full_bytes = zeros / 2;
    if full_bytes > digest.len() {
        return false;
    }
    if digest[..full_bytes].iter().any(|byte| *byte != 0) {
        return false;
    }
    if zeros % 2 == 0 {
        return true;
    }
    digest.get(full_bytes).is_some_and(|byte| byte >> 4 == 0)
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    encode_hex(&hasher.finalize())
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        out.push(char::from(HEX[usize::from(byte >> 4)]));
        out.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    out
}

#[cfg(test)]
#[path = "anubis_tests.rs"]
mod tests;
