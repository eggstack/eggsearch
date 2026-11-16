//! Anubis proof-of-work solver for the published challenge methods.

use std::time::{Duration, Instant, SystemTime};

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
/// Anubis WASM `sha256` method name, a leading-zero-bit nonce search.
pub const METHOD_SHA256: &str = "sha256";
/// Script element id carrying the `fast` challenge document.
pub const FAST_SCRIPT_ID: &str = "anubis_challenge";
/// Script element id carrying the `preact` challenge document.
pub const PREACT_SCRIPT_ID: &str = "preact_info";
/// Script element id carrying the published path prefix.
pub const BASE_PREFIX_SCRIPT_ID: &str = "anubis_base_prefix";
/// Pass-challenge endpoint path, relative to the upstream origin.
pub const PASS_CHALLENGE_PATH: &str = "/.within.website/x/cmd/anubis/api/pass-challenge";
/// Fixed client timing field submitted with every `fast` solution.
pub const ELAPSED_TIME: u64 = 1;
/// Published minimum elapsed time per difficulty unit, in seconds.
pub const PREACT_MIN_ELAPSED_PER_DIFFICULTY: f64 = 0.08;
/// Published minimum wall clock per `metarefresh` difficulty unit, in seconds.
pub const METAREFRESH_MIN_ELAPSED_PER_DIFFICULTY: f64 = 0.8;
/// Release-build nanoseconds per SHA-256 candidate, from the measured difficulty-6 solve
/// recorded in `plans/closure/agentic-retrieval-integrity/011-status.md`: 23,070,878
/// candidates in 3.05 s on one core is 0.13 µs, rounded to a whole nanosecond.
pub const CANDIDATE_NANOS: u64 = 130;

const MAX_PASS_BYTES: usize = 16 * 1024;
const MAX_RANDOM_BYTES: usize = 4096;
const MAX_EXTENSIONS: usize = 8;
const MAX_EXTENSION_NAME_BYTES: usize = 64;
const MAX_EXTENSION_BODY_BYTES: usize = 4096;
const CSS_LOAD_EXTENSION: &str = "css-load";
const CSS_LOAD_PATH: &str = "/.within.website/x/cmd/anubis/ext/css-load";

/// Anubis challenge method this build solves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnubisMethod {
    /// SHA-256 nonce search over the published `randomData`.
    Fast,
    /// Single SHA-256 digest of the published challenge string.
    Preact,
    /// Meta-refresh time challenge: echo the published token after the published wait.
    Metarefresh,
    /// WASM `sha256`: leading-zero-bit nonce search over the decoded `randomData`.
    WasmSha256,
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
    random_bytes: Vec<u8>,
    challenge: String,
    redir: Option<String>,
    base_prefix: String,
    wait: Option<Duration>,
    target: Option<String>,
    extensions: Vec<String>,
}

impl std::fmt::Debug for AnubisChallenge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AnubisChallenge")
            .field("method", &self.method)
            .field("difficulty", &self.difficulty)
            .field("id", &"<redacted>")
            .field("random_data", &"<redacted>")
            .field("random_bytes", &"<redacted>")
            .field("challenge", &"<redacted>")
            .field("redir", &"<redacted>")
            .field("base_prefix", &"<redacted>")
            .field("target", &"<redacted>")
            .field("wait", &self.wait)
            .field("extensions", &self.extensions)
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
                    .or_else(|| rules.get("algorithm").and_then(Value::as_str))
                    .ok_or(BOUND_MALFORMED)?;
                refuse_spent(challenge)?;
                let extensions = parse_extensions(challenge.get("extensions"))?;
                match method {
                    METHOD_FAST | METHOD_SLOW => Ok(Self {
                        method: AnubisMethod::Fast,
                        difficulty,
                        id: required_str(challenge, "id")?,
                        random_data: required_str(challenge, "randomData")?,
                        random_bytes: Vec::new(),
                        challenge: String::new(),
                        redir: optional_str(challenge, "redir"),
                        base_prefix,
                        wait: None,
                        target: None,
                        extensions,
                    }),
                    METHOD_SHA256 => {
                        let random_data = required_str(challenge, "randomData")?;
                        let random_bytes = decode_hex(&random_data)?;
                        Ok(Self {
                            method: AnubisMethod::WasmSha256,
                            difficulty,
                            id: required_str(challenge, "id")?,
                            random_data,
                            random_bytes,
                            challenge: String::new(),
                            redir: optional_str(challenge, "redir"),
                            base_prefix,
                            wait: None,
                            target: None,
                            extensions,
                        })
                    }
                    METHOD_PREACT => Self::from_preact_info(
                        preact.ok_or(BOUND_MALFORMED)?,
                        base_prefix,
                        extensions,
                    ),
                    METHOD_METAREFRESH => Self::from_metarefresh(
                        challenge,
                        base_prefix,
                        difficulty,
                        &document,
                        extensions,
                    ),
                    _ => Err(BOUND_UNSUPPORTED_SCHEME),
                }
            }
            (None, Some(preact)) => Self::from_preact_info(preact, base_prefix, Vec::new()),
            (None, None) => Err(BOUND_MALFORMED),
        }
    }

    fn from_preact_info(
        value: Value,
        base_prefix: String,
        extensions: Vec<String>,
    ) -> Result<Self, &'static str> {
        let object = value.as_object().ok_or(BOUND_MALFORMED)?;
        refuse_spent(object)?;
        Ok(Self {
            method: AnubisMethod::Preact,
            difficulty: numeric_difficulty(object.get("difficulty"))?,
            id: String::new(),
            random_data: String::new(),
            random_bytes: Vec::new(),
            challenge: required_str(object, "challenge")?,
            redir: optional_str(object, "redir"),
            base_prefix,
            wait: None,
            target: None,
            extensions,
        })
    }

    fn from_metarefresh(
        object: &Map<String, Value>,
        base_prefix: String,
        difficulty: u8,
        document: &Html,
        extensions: Vec<String>,
    ) -> Result<Self, &'static str> {
        let presentation = published_meta_refresh(document)?;
        let token = optional_str(object, "randomData")
            .or_else(|| optional_str(object, "challenge"))
            .ok_or(BOUND_MALFORMED)?;
        let wait = match presentation {
            Some((wait, _)) => {
                if let Some(value) = object.get("wait") {
                    let seconds = value.as_f64().ok_or(BOUND_MALFORMED)?;
                    let published =
                        Duration::try_from_secs_f64(seconds).map_err(|_| BOUND_MALFORMED)?;
                    if published != wait {
                        return Err(BOUND_MALFORMED);
                    }
                }
                wait
            }
            None => match object.get("wait") {
                Some(value) => {
                    let seconds = value.as_f64().ok_or(BOUND_MALFORMED)?;
                    Duration::try_from_secs_f64(seconds).map_err(|_| BOUND_MALFORMED)?
                }
                None => Duration::from_secs(u64::from(difficulty) + 1),
            },
        };
        Ok(Self {
            method: AnubisMethod::Metarefresh,
            difficulty,
            id: optional_str(object, "id").unwrap_or_default(),
            random_data: String::new(),
            random_bytes: Vec::new(),
            challenge: token,
            redir: optional_str(object, "redir"),
            base_prefix,
            wait: Some(wait),
            target: presentation.and_then(|(_, target)| target),
            extensions,
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

    /// Build the `metarefresh` pass-challenge URL from published values only.
    fn metarefresh_submission(
        &self,
        base_url: &str,
        request_url: &str,
    ) -> Result<String, &'static str> {
        let base = Url::parse(base_url).map_err(|_| BOUND_MALFORMED)?;
        let mut url = match self.target.as_deref() {
            Some(target) => {
                let resolved = base.join(target).map_err(|_| BOUND_MALFORMED)?;
                if !same_origin(&resolved, &base) {
                    return Err(BOUND_MALFORMED);
                }
                resolved
            }
            None => base
                .join(&format!("{}{PASS_CHALLENGE_PATH}", self.base_prefix))
                .map_err(|_| BOUND_MALFORMED)?,
        };
        let redir = self.resolved_redir(base_url, request_url);
        let mut params: Vec<(String, String)> = url.query_pairs().into_owned().collect();
        push_missing(&mut params, "redir", &redir);
        push_missing(&mut params, "challenge", &self.challenge);
        if !self.id.is_empty() {
            push_missing(&mut params, "id", &self.id);
        }
        url.set_query(None);
        url.query_pairs_mut().extend_pairs(params);
        Ok(url.to_string())
    }

    /// Compare the published difficulty with the configured ceiling.
    ///
    /// The failure names both values: it is the operator-visible knob.
    pub fn check_ceiling(&self, budget: &SolveBudget) -> Result<(), String> {
        let units = self.ceiling_units();
        if budget.allows_difficulty(units) {
            Ok(())
        } else {
            Err(self.ceiling_refusal(budget.max_difficulty()))
        }
    }

    /// Difficulty re-expressed in the operator's fast-equivalent nibbles.
    fn ceiling_units(&self) -> u8 {
        match self.method {
            AnubisMethod::WasmSha256 => self.difficulty.div_ceil(4),
            _ => self.difficulty,
        }
    }

    fn ceiling_refusal(&self, ceiling: u8) -> String {
        match self.method {
            AnubisMethod::WasmSha256 => format!(
                "anubis difficulty {} bits ({} fast-equivalent) exceeds configured ceiling {ceiling}",
                self.difficulty,
                self.ceiling_units()
            ),
            _ => ceiling_refusal(self.difficulty, ceiling),
        }
    }

    /// Refuse a cost the configured budget cannot pay before any hashing starts.
    pub fn check_projection(&self, budget: &SolveBudget) -> Result<(), String> {
        let expected = match self.method {
            AnubisMethod::WasmSha256 => {
                expected_bit_candidates(self.difficulty).ok_or_else(|| {
                    format!(
                        "anubis difficulty {} bits cannot be projected",
                        self.difficulty
                    )
                })?
            }
            _ => expected_candidates(self.difficulty),
        };
        let difficulty = match self.method {
            AnubisMethod::WasmSha256 => format!("{} bits", self.difficulty),
            _ => self.difficulty.to_string(),
        };
        if expected > u128::from(budget.max_attempts()) {
            return Err(format!(
                "anubis difficulty {difficulty} projects {} candidates, beyond the configured attempt ceiling {}",
                compact_count(expected),
                budget.max_attempts()
            ));
        }
        let projected_nanos = expected.saturating_mul(u128::from(CANDIDATE_NANOS));
        if projected_nanos > budget.remaining_time().as_nanos() {
            return Err(format!(
                "anubis difficulty {difficulty} projects {} candidates (~{} ms), beyond the configured wall-clock budget {} ms",
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
            AnubisMethod::WasmSha256 => self.solve_sha256(budget),
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

    fn solve_sha256(&self, budget: &mut SolveBudget) -> Result<AnubisSolution, &'static str> {
        let data = &self.random_bytes;
        let use_le = data.last().copied().unwrap_or(0) >= 128;
        let mut hasher = Sha256::new();
        let mut nonce: u32 = 0;
        loop {
            budget.attempt()?;
            hasher.update(data);
            hasher.update(if use_le {
                nonce.to_le_bytes()
            } else {
                nonce.to_be_bytes()
            });
            let digest = hasher.finalize_reset();
            if leading_zero_bits(&digest, self.difficulty) {
                return Ok(AnubisSolution {
                    response: encode_hex(&digest),
                    nonce: u64::from(nonce),
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

/// Expected SHA-256 candidates for a leading-zero-bit difficulty, as `2^bits`.
///
/// `None` means the count does not fit `u128`, so it cannot be projected.
#[must_use]
pub fn expected_bit_candidates(bits: u8) -> Option<u128> {
    1u128.checked_shl(u32::from(bits))
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
    let started = Instant::now();
    let deadline = started + timeout;
    if let Some(step) =
        satisfy_extensions(client, engine, base_url, &challenge, deadline, started).await
    {
        return step;
    }
    if let Err(reason) = challenge.check_ceiling(budget) {
        return blocked_step(reason, RefusalReason::DifficultyAboveCeiling);
    }
    if matches!(
        challenge.method,
        AnubisMethod::Fast | AnubisMethod::WasmSha256
    ) {
        if let Err(reason) = challenge.check_projection(budget) {
            return blocked_step(reason, RefusalReason::UnsafeBasePrefix);
        }
    }

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
            match metarefresh_wait(requirement, challenge.difficulty, deadline) {
                Ok(wait) => tokio::time::sleep(wait).await,
                Err(bound) => return decided_step(TerminalOutcome::Refused { bound }, started),
            }
        }
        AnubisMethod::Fast | AnubisMethod::WasmSha256 => {}
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
) -> Result<TerminalOutcome, EngineError> {
    let redir = challenge.resolved_redir(base_url, request_url);
    let url = match challenge.method {
        AnubisMethod::Fast | AnubisMethod::WasmSha256 => {
            pass_challenge_url(base_url, &challenge.base_prefix)
        }
        AnubisMethod::Preact => preact_url(base_url, &redir, &solution.response),
        AnubisMethod::Metarefresh => {
            match challenge.metarefresh_submission(base_url, request_url) {
                Ok(url) => url,
                Err(bound) => return Ok(TerminalOutcome::Refused { bound }),
            }
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
    if matches!(
        challenge.method,
        AnubisMethod::Fast | AnubisMethod::WasmSha256
    ) {
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
    let issued = cookie::parse_issued_pairs(resp.headers());
    read_bounded_body(resp, engine, MAX_PASS_BYTES).await?;

    if !issued.is_empty() {
        return Ok(TerminalOutcome::Solved {
            cookie: issued.header(),
            lifetime: issued.reuse_lifetime(SystemTime::now()),
        });
    }
    let bound = if status.is_success() {
        BOUND_MISSING_COOKIE
    } else {
        BOUND_COOKIE_REJECTED
    };
    Ok(TerminalOutcome::Refused { bound })
}

async fn satisfy_extensions(
    client: &Client,
    engine: &'static str,
    base_url: &str,
    challenge: &AnubisChallenge,
    deadline: Instant,
    started: Instant,
) -> Option<SolveStep> {
    if challenge.extensions.is_empty() {
        return None;
    }
    if let Some(reason) = unsupported_extensions(&challenge.extensions) {
        return Some(blocked_step(reason, RefusalReason::UnsupportedScheme));
    }
    if !is_challenge_id(&challenge.id) {
        return Some(decided_step(
            TerminalOutcome::Refused {
                bound: BOUND_MALFORMED,
            },
            started,
        ));
    }
    let Ok(url) = css_load_url(base_url, &challenge.base_prefix, &challenge.id) else {
        return Some(decided_step(
            TerminalOutcome::Refused {
                bound: BOUND_MALFORMED,
            },
            started,
        ));
    };
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        return Some(decided_step(
            TerminalOutcome::Refused {
                bound: BOUND_WALL_CLOCK,
            },
            started,
        ));
    }
    let Ok(request) = client.get(&url) else {
        return Some(blocked_step(
            css_load_failure(),
            RefusalReason::UnsupportedScheme,
        ));
    };
    let response = match request
        .redirect_policy(RedirectPolicy::new(false, 0))
        .timeout(engine_timeout(remaining))
        .send()
        .await
    {
        Ok(response) => response,
        Err(_) => {
            return Some(blocked_step(
                css_load_failure(),
                RefusalReason::UnsupportedScheme,
            ))
        }
    };
    let status = response.status();
    if !status.is_success() {
        return Some(blocked_step(
            format!(
                "anubis css-load extension was not satisfied (status {})",
                status.as_u16()
            ),
            RefusalReason::UnsupportedScheme,
        ));
    }
    let _ = read_bounded_body(response, engine, MAX_EXTENSION_BODY_BYTES).await;
    None
}

fn unsupported_extensions(names: &[String]) -> Option<String> {
    if names.iter().all(|name| name == CSS_LOAD_EXTENSION) {
        return None;
    }
    Some(format!(
        "anubis challenge declares unsupported extensions [{}]",
        names.join(", ")
    ))
}

fn css_load_failure() -> String {
    "anubis css-load extension was not satisfied".to_string()
}

fn css_load_url(base_url: &str, base_prefix: &str, id: &str) -> Result<String, &'static str> {
    let base = Url::parse(base_url).map_err(|_| BOUND_MALFORMED)?;
    let path = format!("{base_prefix}{CSS_LOAD_PATH}/{id}");
    let url = base.join(&path).map_err(|_| BOUND_MALFORMED)?;
    if !same_origin(&url, &base) {
        return Err(BOUND_MALFORMED);
    }
    Ok(url.to_string())
}

fn is_challenge_id(id: &str) -> bool {
    const HYPHENS: [usize; 4] = [8, 13, 18, 23];
    id.len() == 36
        && id.bytes().enumerate().all(|(index, byte)| {
            if HYPHENS.contains(&index) {
                byte == b'-'
            } else {
                byte.is_ascii_hexdigit()
            }
        })
}

fn parse_extensions(value: Option<&Value>) -> Result<Vec<String>, &'static str> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    if value.is_null() {
        return Ok(Vec::new());
    }
    let array = value.as_array().ok_or(BOUND_MALFORMED)?;
    if array.len() > MAX_EXTENSIONS {
        return Err(BOUND_MALFORMED);
    }
    let mut names = Vec::with_capacity(array.len());
    for item in array {
        let name = item.as_str().ok_or(BOUND_MALFORMED)?;
        if name.is_empty()
            || name.len() > MAX_EXTENSION_NAME_BYTES
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
        {
            return Err(BOUND_MALFORMED);
        }
        names.push(name.to_string());
    }
    Ok(names)
}

fn decode_hex(value: &str) -> Result<Vec<u8>, &'static str> {
    if value.len() % 2 != 0 {
        return Err(BOUND_MALFORMED);
    }
    let mut bytes = Vec::with_capacity(value.len() / 2);
    for pair in value.as_bytes().chunks_exact(2) {
        let high = hex_value(pair[0]).ok_or(BOUND_MALFORMED)?;
        let low = hex_value(pair[1]).ok_or(BOUND_MALFORMED)?;
        bytes.push((high << 4) | low);
        if bytes.len() > MAX_RANDOM_BYTES {
            return Err(BOUND_MALFORMED);
        }
    }
    if bytes.is_empty() {
        return Err(BOUND_MALFORMED);
    }
    Ok(bytes)
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn metarefresh_wait(
    requirement: Duration,
    difficulty: u8,
    deadline: Instant,
) -> Result<Duration, &'static str> {
    let floor =
        Duration::from_secs_f64(METAREFRESH_MIN_ELAPSED_PER_DIFFICULTY * f64::from(difficulty));
    let requirement = requirement.max(floor);
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() || requirement > remaining {
        return Err(BOUND_WALL_CLOCK);
    }
    Ok(requirement)
}

fn published_meta_refresh(
    document: &Html,
) -> Result<Option<(Duration, Option<String>)>, &'static str> {
    let selector = scraper::Selector::parse("meta[http-equiv]").map_err(|_| BOUND_MALFORMED)?;
    let Some(content) = document
        .select(&selector)
        .find(|element| {
            element
                .value()
                .attr("http-equiv")
                .is_some_and(|value| value.eq_ignore_ascii_case("refresh"))
        })
        .and_then(|element| element.value().attr("content"))
    else {
        return Ok(None);
    };
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
    Ok(Some((wait, target)))
}

fn push_missing(params: &mut Vec<(String, String)>, name: &str, value: &str) {
    if params.iter().any(|(key, _)| key == name) {
        return;
    }
    params.push((name.to_string(), value.to_string()));
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

fn leading_zero_bits(digest: &[u8], bits: u8) -> bool {
    let mut remaining = usize::from(bits);
    for &byte in digest {
        if remaining == 0 {
            return true;
        }
        if remaining >= 8 {
            if byte != 0 {
                return false;
            }
            remaining -= 8;
        } else {
            let mask = 0xffu8 << (8 - remaining);
            if byte & mask != 0 {
                return false;
            }
            remaining = 0;
        }
    }
    remaining == 0
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
