//! Anubis proof-of-work solver for the `fast` and `preact` methods.

use std::time::{Duration, Instant};

use eggfetch_core::{Client, RedirectPolicy};
use scraper::Html;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use super::{
    SolveBudget, SolveOutcome, BOUND_ATTEMPTS, BOUND_COOKIE_REJECTED, BOUND_MALFORMED,
    BOUND_MISSING_COOKIE, BOUND_UNSUPPORTED_SCHEME, BOUND_WALL_CLOCK,
};
use crate::meta::engines::error::EngineError;
use crate::meta::engines::{engine_timeout, map_request_error, read_bounded_body};

/// Anubis `fast` method name.
pub const METHOD_FAST: &str = "fast";
/// Anubis `preact` method name.
pub const METHOD_PREACT: &str = "preact";
/// Script element id carrying the `fast` challenge document.
pub const FAST_SCRIPT_ID: &str = "anubis_challenge";
/// Script element id carrying the `preact` challenge document.
pub const PREACT_SCRIPT_ID: &str = "preact_info";
/// Pass-challenge endpoint path, relative to the upstream origin.
pub const PASS_CHALLENGE_PATH: &str = "/.within.website/x/cmd/anubis/api/pass-challenge";
/// Pass cookie the endpoint sets on success.
pub const PASS_COOKIE_NAME: &str = "spchal-auth";
/// Fixed client timing field submitted with every `fast` solution.
pub const ELAPSED_TIME: u64 = 1;
/// Published minimum elapsed time per difficulty unit, in seconds.
pub const PREACT_MIN_ELAPSED_PER_DIFFICULTY: f64 = 0.08;

const MAX_PASS_BYTES: usize = 16 * 1024;

/// Anubis challenge method this build solves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnubisMethod {
    /// SHA-256 nonce search over the published `randomData`.
    Fast,
    /// Single SHA-256 digest of the published challenge string.
    Preact,
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
            .finish()
    }
}

impl AnubisChallenge {
    /// Parse a published Anubis challenge page.
    pub fn parse(html: &str) -> Result<Self, &'static str> {
        let document = Html::parse_document(html);
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
                match method {
                    METHOD_FAST => Ok(Self {
                        method: AnubisMethod::Fast,
                        difficulty,
                        id: required_str(challenge, "id")?,
                        random_data: required_str(challenge, "randomData")?,
                        challenge: String::new(),
                        redir: optional_str(challenge, "redir"),
                    }),
                    METHOD_PREACT => Self::from_preact_info(preact.ok_or(BOUND_MALFORMED)?),
                    _ => Err(BOUND_UNSUPPORTED_SCHEME),
                }
            }
            (None, Some(preact)) => Self::from_preact_info(preact),
            (None, None) => Err(BOUND_MALFORMED),
        }
    }

    fn from_preact_info(value: Value) -> Result<Self, &'static str> {
        let object = value.as_object().ok_or(BOUND_MALFORMED)?;
        Ok(Self {
            method: AnubisMethod::Preact,
            difficulty: numeric_difficulty(object.get("difficulty"))?,
            id: String::new(),
            random_data: String::new(),
            challenge: required_str(object, "challenge")?,
            redir: optional_str(object, "redir"),
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

    /// Redirect target for this attempt.
    ///
    /// A live Anubis `fast` challenge publishes no `redir` at all, so the
    /// original request URL is the only correct fallback; an absent value is
    /// not a malformed challenge.
    fn resolved_redir(&self, request_url: &str) -> String {
        self.redir
            .as_deref()
            .filter(|value| !value.is_empty())
            .unwrap_or(request_url)
            .to_string()
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

    /// Solve the challenge under `budget`.
    pub fn solve(&self, budget: &mut SolveBudget) -> Result<AnubisSolution, &'static str> {
        match self.method {
            AnubisMethod::Fast => self.solve_fast(budget),
            AnubisMethod::Preact => Ok(AnubisSolution {
                response: sha256_hex(self.challenge.as_bytes()),
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
    let challenge = match AnubisChallenge::parse(challenge_page) {
        Ok(challenge) => challenge,
        Err(bound) => return Ok(SolveOutcome::Refused { bound }),
    };
    if let Err(reason) = challenge.check_ceiling(budget) {
        return Err(EngineError::Blocked { engine, reason });
    }

    let deadline = Instant::now() + timeout;
    if challenge.method == AnubisMethod::Preact {
        match preact_wait(challenge.difficulty, deadline) {
            Ok(wait) => tokio::time::sleep(wait).await,
            Err(bound) => return Ok(SolveOutcome::Refused { bound }),
        }
    }

    let solution = match challenge.solve(budget) {
        Ok(solution) => solution,
        Err(bound) => return Ok(SolveOutcome::Refused { bound }),
    };
    verify(
        client,
        engine,
        base_url,
        &challenge,
        request_url,
        &solution,
        challenge_cookies,
        timeout,
    )
    .await
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
    timeout: Duration,
) -> Result<SolveOutcome, EngineError> {
    let redir = challenge.resolved_redir(request_url);
    let url = match challenge.method {
        AnubisMethod::Fast => endpoint(base_url, PASS_CHALLENGE_PATH),
        AnubisMethod::Preact => preact_url(base_url, &redir, &solution.response),
    };
    let mut request = client
        .get(&url)
        .map_err(|source| EngineError::Http { engine, source })?
        .redirect_policy(RedirectPolicy::new(false, 0))
        .timeout(engine_timeout(timeout));
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
    let cookie = extract_cookie(resp.headers(), PASS_COOKIE_NAME);
    read_bounded_body(resp, engine, MAX_PASS_BYTES).await?;

    if let Some(cookie) = cookie {
        return Ok(SolveOutcome::Solved { cookie });
    }
    let bound = if status.is_success() {
        BOUND_MISSING_COOKIE
    } else {
        BOUND_COOKIE_REJECTED
    };
    Ok(SolveOutcome::Refused { bound })
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

fn endpoint(base_url: &str, path: &str) -> String {
    format!("{}{path}", base_url.trim_end_matches('/'))
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

fn extract_cookie(headers: &http::HeaderMap, name: &str) -> Option<String> {
    for value in headers.get_all("set-cookie") {
        let Ok(raw) = value.to_str() else {
            continue;
        };
        for part in raw.split(';') {
            let part = part.trim();
            let Some(rest) = part.strip_prefix(name) else {
                continue;
            };
            let Some(cookie) = rest.strip_prefix('=') else {
                continue;
            };
            let cookie = cookie.trim();
            if !cookie.is_empty() {
                return Some(cookie.to_string());
            }
        }
    }
    None
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
mod tests {
    use super::*;

    const FAST_DIGEST: &str = "002786c8b408cf14bf6ddc2aeb6b6f1fcdf6b453986796fbec919a88de759708";
    const PREACT_DIGEST: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

    fn fast_page(random_data: &str, difficulty: u8, method: &str) -> String {
        let payload = serde_json::json!({
            "rules": {"algorithm": "fast", "difficulty": difficulty},
            "challenge": {
                "issuedAt": "2026-11-11T00:00:00Z",
                "metadata": {"User-Agent": "eggsearch"},
                "id": "01a1cc17-0000-7000-8000-000000000000",
                "method": method,
                "randomData": random_data,
                "redir": "/search?q=rust",
                "policyRuleHash": "00",
                "difficulty": difficulty,
                "spent": false
            }
        });
        format!(
            r#"<html><head><script id="anubis_challenge" type="application/json">{payload}</script></head><body><div id="app"></div></body></html>"#
        )
    }

    fn preact_page(challenge: &str, difficulty: u8, redir: &str) -> String {
        let payload = serde_json::json!({
            "challenge": challenge,
            "difficulty": difficulty,
            "redir": redir
        });
        format!(
            r#"<html><head><script id="preact_info" type="application/json">{payload}</script></head><body></body></html>"#
        )
    }

    fn budget(max_attempts: u64, timeout_ms: u64, max_difficulty: u8) -> SolveBudget {
        SolveBudget::new(
            max_attempts,
            Duration::from_millis(timeout_ms),
            max_difficulty,
        )
    }

    fn test_client() -> Client {
        crate::meta::engines::build_http_client(None).expect("test client")
    }

    #[test]
    fn fast_vector_solves_with_the_lowest_nonce() {
        let challenge =
            AnubisChallenge::parse(&fast_page("abc123def456", 2, "fast")).expect("vector parses");
        assert_eq!(challenge.method(), AnubisMethod::Fast);
        assert_eq!(challenge.difficulty(), 2);

        let mut budget = budget(1000, 5000, 5);
        let solution = challenge.solve(&mut budget).expect("vector solves");
        assert_eq!(solution.nonce, 185);
        assert_eq!(solution.response, FAST_DIGEST);
        assert_eq!(budget.attempts_used(), 186);
        assert!(budget.attempts_used() <= budget.max_attempts());
    }

    #[test]
    fn fast_negative_control_rejects_an_unsatisfied_digest() {
        let challenge =
            AnubisChallenge::parse(&fast_page("abc123def456", 2, "fast")).expect("vector parses");
        let mut hasher = Sha256::new();
        hasher.update(b"abc123def456");
        hasher.update(b"0");
        let zero_nonce = hasher.finalize();
        assert_eq!(
            encode_hex(&zero_nonce),
            "6416fa1aa73b88d0e2908967bc4dfc949186e2bd80a3f9a5d5a72bb927e41a72"
        );
        assert!(!has_leading_zero_hex(&zero_nonce, 2));

        let mut budget = budget(185, 5000, 5);
        assert_eq!(challenge.solve(&mut budget), Err(BOUND_ATTEMPTS));
        assert_eq!(budget.attempts_used(), 185);
    }

    #[test]
    fn ceiling_allows_below_and_at_then_names_the_breach() {
        for (difficulty, ceiling, allowed) in [
            (1u8, 5u8, true),
            (4, 5, true),
            (5, 5, true),
            (6, 5, false),
            (6, 6, true),
        ] {
            let challenge = AnubisChallenge::parse(&fast_page("abc123def456", difficulty, "fast"))
                .expect("vector parses");
            let budget = budget(10, 1000, ceiling);
            assert_eq!(
                challenge.check_ceiling(&budget).is_ok(),
                allowed,
                "difficulty {difficulty} against ceiling {ceiling}"
            );
        }

        let challenge =
            AnubisChallenge::parse(&fast_page("abc123def456", 6, "fast")).expect("vector parses");
        let budget = budget(10, 1000, 5);
        assert_eq!(
            challenge.check_ceiling(&budget),
            Err("anubis difficulty 6 exceeds configured ceiling 5".to_string())
        );
        assert_eq!(
            ceiling_refusal(6, 5),
            "anubis difficulty 6 exceeds configured ceiling 5"
        );
    }

    #[tokio::test]
    async fn ceiling_breach_is_blocked_before_any_hashing() {
        use httpmock::prelude::*;

        let server = MockServer::start();
        let any = server.mock(|when, then| {
            when.method(GET);
            then.status(200)
                .header("set-cookie", "spchal-auth=leaked-value");
        });

        let mut budget = budget(2_000_000, 5000, 5);
        let err = solve_and_verify(
            &test_client(),
            "startpage",
            &server.url(""),
            &fast_page("abc123def456", 6, "fast"),
            "/search?q=rust",
            "sp_pow=leaked-value",
            Duration::from_secs(5),
            &mut budget,
        )
        .await
        .expect_err("ceiling breach is blocked");

        match &err {
            EngineError::Blocked { engine, reason } => {
                assert_eq!(*engine, "startpage");
                assert_eq!(reason, "anubis difficulty 6 exceeds configured ceiling 5");
            }
            other => panic!("unexpected error: {other:?}"),
        }
        assert!(!err.to_string().contains("leaked-value"), "got: {err}");
        assert_eq!(budget.attempts_used(), 0);
        assert_eq!(any.hits(), 0);
    }

    #[test]
    fn unknown_method_fails_closed() {
        assert_eq!(
            AnubisChallenge::parse(&fast_page("abc123def456", 2, "slow")).err(),
            Some(BOUND_UNSUPPORTED_SCHEME)
        );
    }

    #[tokio::test]
    async fn unknown_method_is_refused_without_a_request() {
        use httpmock::prelude::*;

        let server = MockServer::start();
        let any = server.mock(|when, then| {
            when.method(GET);
            then.status(200).header("set-cookie", "spchal-auth=never");
        });

        let mut budget = budget(1000, 5000, 5);
        let outcome = solve_and_verify(
            &test_client(),
            "startpage",
            &server.url(""),
            &fast_page("abc123def456", 2, "slow"),
            "/search?q=rust",
            "",
            Duration::from_secs(5),
            &mut budget,
        )
        .await
        .expect("refusal is not an engine error");
        assert_eq!(
            outcome,
            SolveOutcome::Refused {
                bound: BOUND_UNSUPPORTED_SCHEME
            }
        );
        assert_eq!(budget.attempts_used(), 0);
        assert_eq!(any.hits(), 0);
    }

    #[test]
    fn malformed_payloads_fail_closed() {
        let mut missing_difficulty = serde_json::json!({
            "rules": {"algorithm": "fast"},
            "challenge": {
                "id": "01a1cc17",
                "method": "fast",
                "randomData": "abc123def456",
                "redir": "/search?q=rust"
            }
        });
        let non_numeric = serde_json::json!({
            "rules": {"algorithm": "fast", "difficulty": "six"},
            "challenge": {
                "id": "01a1cc17",
                "method": "fast",
                "randomData": "abc123def456",
                "redir": "/search?q=rust"
            }
        });
        let missing_random_data = serde_json::json!({
            "rules": {"algorithm": "fast", "difficulty": 2},
            "challenge": {"id": "01a1cc17", "method": "fast", "redir": "/search?q=rust"}
        });
        missing_difficulty["rules"]
            .as_object_mut()
            .expect("rules")
            .remove("difficulty");

        let cases = [
            String::new(),
            "<html><body><p>no script</p></body></html>".to_string(),
            r#"<script id="anubis_challenge" type="application/json">not json</script>"#
                .to_string(),
            r#"<script id="anubis_challenge" type="application/json">{}</script>"#.to_string(),
            format!(
                r#"<script id="anubis_challenge" type="application/json">{missing_difficulty}</script>"#
            ),
            format!(
                r#"<script id="anubis_challenge" type="application/json">{non_numeric}</script>"#
            ),
            format!(
                r#"<script id="anubis_challenge" type="application/json">{missing_random_data}</script>"#
            ),
            r#"<script id="preact_info" type="application/json">{"difficulty":1,"redir":"/search"}</script>"#
                .to_string(),
            r#"<script id="preact_info" type="application/json">{"challenge":"abc","redir":"/search"}</script>"#
                .to_string(),
        ];
        for raw in cases {
            assert_eq!(
                AnubisChallenge::parse(&raw).err(),
                Some(BOUND_MALFORMED),
                "expected malformed for {raw}"
            );
        }
    }

    #[test]
    fn live_fast_challenge_without_redir_parses_and_falls_back_to_the_request_url() {
        let page = r#"<script id="anubis_challenge" type="application/json">{"rules":{"algorithm":"fast","difficulty":6},"challenge":{"issuedAt":"2026-11-11T19:30:51Z","id":"01a1cc17-757b-7711-aca3-71c6109b9665","method":"fast","randomData":"570d0182","policyRuleHash":"ac980f49c4d35fab","difficulty":6,"spent":false}}</script>"#;
        let challenge = AnubisChallenge::parse(page).expect("live shape must parse");
        assert_eq!(challenge.difficulty(), 6);
        assert_eq!(
            challenge.resolved_redir("https://www.startpage.com/search?q=rust"),
            "https://www.startpage.com/search?q=rust"
        );
    }

    #[test]
    fn published_redir_wins_over_the_request_url() {
        let challenge =
            AnubisChallenge::parse(&fast_page("abc123def456", 2, "fast")).expect("vector parses");
        assert_eq!(
            challenge.resolved_redir("https://www.startpage.com/search?q=rust"),
            "/search?q=rust"
        );
    }

    #[test]
    fn attempts_ceiling_refuses_and_never_loops() {
        let challenge =
            AnubisChallenge::parse(&fast_page("abc123def456", 2, "fast")).expect("vector parses");
        let mut budget = budget(100, 5000, 5);
        assert_eq!(challenge.solve(&mut budget), Err(BOUND_ATTEMPTS));
        assert_eq!(budget.attempts_used(), 100);
        assert!(budget.attempts_used() <= budget.max_attempts());
    }

    #[test]
    fn wall_clock_ceiling_refuses_and_never_loops() {
        let challenge =
            AnubisChallenge::parse(&fast_page("abc123def456", 6, "fast")).expect("parses");
        let mut budget = budget(u64::MAX, 1, 6);
        let started = Instant::now();
        assert_eq!(challenge.solve(&mut budget), Err(BOUND_WALL_CLOCK));
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "wall-clock refusal took {:?}",
            started.elapsed()
        );
        assert!(budget.attempts_used() >= 1);
        assert!(budget.attempts_used() <= budget.max_attempts());
    }

    #[test]
    fn preact_computes_the_published_digest() {
        let challenge = AnubisChallenge::parse(&preact_page("abc", 1, "/search?q=rust"))
            .expect("preact page parses");
        assert_eq!(challenge.method(), AnubisMethod::Preact);
        assert_eq!(challenge.difficulty(), 1);

        let mut budget = budget(10, 1000, 5);
        let solution = challenge.solve(&mut budget).expect("preact solves");
        assert_eq!(solution.response, PREACT_DIGEST);
        assert_eq!(solution.nonce, 0);
        assert_eq!(budget.attempts_used(), 0);
    }

    #[test]
    fn preact_wait_is_bounded_by_the_remaining_budget() {
        let tight = Instant::now() + Duration::from_millis(100);
        assert_eq!(preact_wait(10, tight), Err(BOUND_WALL_CLOCK));

        let expired = Instant::now() - Duration::from_millis(1);
        assert_eq!(preact_wait(0, expired), Err(BOUND_WALL_CLOCK));

        let roomy = Instant::now() + Duration::from_secs(2);
        let wait = preact_wait(1, roomy).expect("difficulty 1 fits");
        assert!(wait >= Duration::from_millis(79), "wait: {wait:?}");
        assert!(wait <= Duration::from_millis(81), "wait: {wait:?}");

        let capped = preact_wait(4, Instant::now() + Duration::from_millis(100));
        assert_eq!(capped, Err(BOUND_WALL_CLOCK));
    }

    #[tokio::test]
    async fn preact_waits_then_submits_the_digest() {
        use httpmock::prelude::*;

        let server = MockServer::start();
        let submit = server.mock(|when, then| {
            when.method(GET)
                .path("/search")
                .query_param("q", "rust")
                .query_param("result", PREACT_DIGEST);
            then.status(302)
                .header("set-cookie", "spchal-auth=preact-pass");
        });

        let mut budget = budget(10, 2000, 5);
        let started = Instant::now();
        let outcome = solve_and_verify(
            &test_client(),
            "startpage",
            &server.url(""),
            &preact_page("abc", 1, "/search?q=rust"),
            "/search?q=rust",
            "sp_pow=pow-1",
            Duration::from_secs(2),
            &mut budget,
        )
        .await
        .expect("preact verifies");
        assert_eq!(
            outcome,
            SolveOutcome::Solved {
                cookie: "preact-pass".to_string()
            }
        );
        assert!(
            started.elapsed() >= Duration::from_millis(70),
            "minimum elapsed time was not honoured: {:?}",
            started.elapsed()
        );
        assert_eq!(submit.hits(), 1);
    }

    #[tokio::test]
    async fn preact_wait_never_exceeds_the_timeout() {
        use httpmock::prelude::*;

        let server = MockServer::start();
        let submit = server.mock(|when, then| {
            when.method(GET);
            then.status(200).header("set-cookie", "spchal-auth=never");
        });

        let mut budget = budget(10, 100, 10);
        let started = Instant::now();
        let outcome = solve_and_verify(
            &test_client(),
            "startpage",
            &server.url(""),
            &preact_page("abc", 10, "/search?q=rust"),
            "/search?q=rust",
            "",
            Duration::from_millis(100),
            &mut budget,
        )
        .await
        .expect("refusal is not an engine error");
        assert_eq!(
            outcome,
            SolveOutcome::Refused {
                bound: BOUND_WALL_CLOCK
            }
        );
        assert!(
            started.elapsed() < Duration::from_millis(400),
            "wait exceeded the budget: {:?}",
            started.elapsed()
        );
        assert_eq!(submit.hits(), 0);
    }

    #[test]
    fn solution_and_challenge_debug_redact_secrets() {
        let challenge =
            AnubisChallenge::parse(&fast_page("abc123def456", 2, "fast")).expect("vector parses");
        let mut budget = budget(1000, 5000, 5);
        let solution = challenge.solve(&mut budget).expect("vector solves");

        let challenge_debug = format!("{challenge:?}");
        assert!(!challenge_debug.contains("abc123def456"));
        assert!(!challenge_debug.contains("01a1cc17"));
        assert!(challenge_debug.contains("redacted"));

        let solution_debug = format!("{solution:?}");
        assert!(!solution_debug.contains(FAST_DIGEST));
        assert!(!solution_debug.contains("185"));
        assert!(solution_debug.contains("redacted"));

        let outcome_debug = format!(
            "{:?}",
            SolveOutcome::Solved {
                cookie: "spchal-auth=secret-value".to_string()
            }
        );
        assert!(!outcome_debug.contains("secret-value"));
    }

    #[test]
    fn challenge_cookies_join_every_set_cookie_pair() {
        let mut headers = http::HeaderMap::new();
        headers.append(
            "set-cookie",
            http::HeaderValue::from_static(
                "spchal-cookie-verification=verif-1; Path=/; Domain=startpage.com; Secure; SameSite=None",
            ),
        );
        headers.append(
            "set-cookie",
            http::HeaderValue::from_static("sp_pow=pow-1; Expires=Wed, 11 Nov 2026 00:00:00 GMT; Max-Age=300; HttpOnly; Path=/"),
        );
        assert_eq!(
            challenge_cookies(&headers),
            "spchal-cookie-verification=verif-1; sp_pow=pow-1"
        );
        assert_eq!(challenge_cookies(&http::HeaderMap::new()), "");
        assert_eq!(
            extract_cookie(&headers, PASS_COOKIE_NAME),
            None,
            "the challenge response does not carry the pass cookie"
        );
    }

    #[test]
    fn decimal_digits_match_the_submitted_nonce() {
        let mut buf = [0u8; 20];
        assert_eq!(decimal_digits(0, &mut buf), b"0");
        assert_eq!(decimal_digits(185, &mut buf), b"185");
        assert_eq!(decimal_digits(u64::MAX, &mut buf), b"18446744073709551615");
    }

    #[test]
    fn leading_zero_hex_matches_byte_prefixes() {
        let digest = [
            0x00u8, 0x0f, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
        ];
        assert!(has_leading_zero_hex(&digest, 0));
        assert!(has_leading_zero_hex(&digest, 1));
        assert!(has_leading_zero_hex(&digest, 2));
        assert!(has_leading_zero_hex(&digest, 3));
        assert!(!has_leading_zero_hex(&digest, 4));
        assert!(!has_leading_zero_hex(&digest, 65));
    }
}
