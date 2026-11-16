//! ALTCHA PBKDF2 proof-of-work solver.

use std::time::{Duration, Instant};

use base64::Engine as _;
use eggfetch_core::Client;
use pbkdf2::pbkdf2_hmac;
use serde_json::{Map, Value};
use sha2::Sha256;

use super::{
    cookie, decided_step, finish_solve, refused_step, SolveBudget, SolveOutcome, SolveStep,
    TerminalOutcome, BOUND_ATTEMPTS, BOUND_COOKIE_REJECTED, BOUND_MALFORMED, BOUND_MISSING_COOKIE,
    BOUND_UNSUPPORTED_SCHEME,
};
use crate::meta::engines::error::EngineError;
use crate::meta::engines::{engine_timeout, map_request_error, read_bounded_body};

/// The only challenge algorithm this build solves, after normalisation.
pub const SUPPORTED_ALGORITHM: &str = "sha256";

/// Fixed client timing field submitted with every solution.
pub const SOLUTION_TIME: u64 = 150;

const MAX_CHALLENGE_BYTES: usize = 64 * 1024;
const MAX_VERIFY_BYTES: usize = 16 * 1024;
const MAX_KEY_LENGTH: u64 = 64;
const COOKIE_NAME: &str = "chllg";
const MULTIPART_BOUNDARY: &str = "eggsearch-altcha-boundary";

/// Normalise a published algorithm name the way SearXNG does.
#[must_use]
pub fn normalize_algorithm(algorithm: &str) -> String {
    algorithm
        .rsplit('/')
        .next()
        .unwrap_or(algorithm)
        .replace('-', "")
        .to_ascii_lowercase()
}

/// One solved derivation.
#[derive(Clone, PartialEq, Eq)]
pub struct AltchaSolution {
    /// Derived key as lowercase hex.
    pub derived_key: String,
    /// Counter that produced the key.
    pub counter: u32,
}

impl std::fmt::Debug for AltchaSolution {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AltchaSolution")
            .field("derived_key", &"<redacted>")
            .field("counter", &self.counter)
            .finish()
    }
}

struct AltchaParameters {
    algorithm: String,
    cost: u32,
    key_length: usize,
    nonce: Vec<u8>,
    salt: Vec<u8>,
    key_prefix: Vec<u8>,
}

/// A parsed, not-yet-solved ALTCHA challenge.
pub struct AltchaChallenge {
    parameters: AltchaParameters,
    document: Value,
}

impl AltchaChallenge {
    /// Parse a published challenge document.
    pub fn parse(raw: &str) -> Result<Self, &'static str> {
        let document: Value = serde_json::from_str(raw).map_err(|_| BOUND_MALFORMED)?;
        let object = document.as_object().ok_or(BOUND_MALFORMED)?;
        let parameters = object
            .get("parameters")
            .and_then(Value::as_object)
            .ok_or(BOUND_MALFORMED)?;

        let algorithm = parameters
            .get("algorithm")
            .and_then(Value::as_str)
            .ok_or(BOUND_MALFORMED)?
            .to_string();
        let cost = parameters
            .get("cost")
            .and_then(Value::as_u64)
            .ok_or(BOUND_MALFORMED)?;
        if cost == 0 || cost > u64::from(u32::MAX) {
            return Err(BOUND_MALFORMED);
        }
        let key_length = parameters
            .get("keyLength")
            .and_then(Value::as_u64)
            .ok_or(BOUND_MALFORMED)?;
        if !(1..=MAX_KEY_LENGTH).contains(&key_length) {
            return Err(BOUND_MALFORMED);
        }
        let nonce = decode_hex(
            parameters
                .get("nonce")
                .and_then(Value::as_str)
                .ok_or(BOUND_MALFORMED)?,
        )
        .ok_or(BOUND_MALFORMED)?;
        let salt = decode_hex(
            parameters
                .get("salt")
                .and_then(Value::as_str)
                .ok_or(BOUND_MALFORMED)?,
        )
        .ok_or(BOUND_MALFORMED)?;
        let key_prefix = decode_hex(
            parameters
                .get("keyPrefix")
                .and_then(Value::as_str)
                .ok_or(BOUND_MALFORMED)?,
        )
        .ok_or(BOUND_MALFORMED)?;
        if nonce.is_empty() || salt.is_empty() || key_prefix.is_empty() {
            return Err(BOUND_MALFORMED);
        }

        Ok(Self {
            parameters: AltchaParameters {
                algorithm,
                cost: cost as u32,
                key_length: key_length as usize,
                nonce,
                salt,
                key_prefix,
            },
            document,
        })
    }

    /// Published algorithm name.
    #[must_use]
    pub fn algorithm(&self) -> &str {
        &self.parameters.algorithm
    }

    /// Published PBKDF2 iteration count.
    #[must_use]
    pub fn cost(&self) -> u32 {
        self.parameters.cost
    }

    /// Whether this challenge uses a supported algorithm.
    #[must_use]
    pub fn is_supported(&self) -> bool {
        normalize_algorithm(&self.parameters.algorithm) == SUPPORTED_ALGORITHM
    }

    /// Search for the counter whose derived key starts with `keyPrefix`.
    pub fn solve(&self, budget: &mut SolveBudget) -> Result<AltchaSolution, &'static str> {
        if !self.is_supported() {
            return Err(BOUND_UNSUPPORTED_SCHEME);
        }
        let nonce_len = self.parameters.nonce.len();
        let mut password = Vec::with_capacity(nonce_len + 4);
        password.extend_from_slice(&self.parameters.nonce);
        password.extend_from_slice(&0u32.to_be_bytes());
        let mut derived = vec![0u8; self.parameters.key_length];
        let mut counter: u32 = 0;

        loop {
            budget.attempt()?;
            password[nonce_len..].copy_from_slice(&counter.to_be_bytes());
            pbkdf2_hmac::<Sha256>(
                &password,
                &self.parameters.salt,
                self.parameters.cost,
                &mut derived,
            );
            if derived.starts_with(&self.parameters.key_prefix) {
                return Ok(AltchaSolution {
                    derived_key: encode_hex(&derived),
                    counter,
                });
            }
            counter = match counter.checked_add(1) {
                Some(next) => next,
                None => return Err(BOUND_ATTEMPTS),
            };
        }
    }

    /// Build the base64 submission payload for a solved challenge.
    pub fn submission(&self, solution: &AltchaSolution) -> Result<String, &'static str> {
        let mut solved = Map::new();
        solved.insert("counter".to_string(), Value::from(solution.counter));
        solved.insert(
            "derivedKey".to_string(),
            Value::from(solution.derived_key.clone()),
        );
        solved.insert("time".to_string(), Value::from(SOLUTION_TIME));

        let mut root = Map::new();
        root.insert("challenge".to_string(), self.document.clone());
        root.insert("solution".to_string(), Value::Object(solved));

        let json = serde_json::to_vec(&Value::Object(root)).map_err(|_| BOUND_MALFORMED)?;
        Ok(base64::engine::general_purpose::STANDARD.encode(json))
    }
}

/// Fetch, solve, and verify one ALTCHA challenge.
pub async fn solve_and_verify(
    client: &Client,
    engine: &'static str,
    challenge_url: &str,
    verify_url: &str,
    timeout: Duration,
    budget: &mut SolveBudget,
) -> Result<SolveOutcome, EngineError> {
    let step =
        solve_and_verify_inner(client, engine, challenge_url, verify_url, timeout, budget).await;
    finish_solve(engine, step, budget.attempts_used())
}

async fn solve_and_verify_inner(
    client: &Client,
    engine: &'static str,
    challenge_url: &str,
    verify_url: &str,
    timeout: Duration,
    budget: &mut SolveBudget,
) -> SolveStep {
    let request = match client
        .get(challenge_url)
        .map_err(|source| EngineError::Http { engine, source })
    {
        Ok(request) => request,
        Err(error) => return SolveStep::Error(error),
    };
    let resp = match request
        .header("Accept", "application/json")
        .timeout(engine_timeout(timeout))
        .send()
        .await
    {
        Ok(resp) => resp,
        Err(error) => return SolveStep::Error(map_request_error(engine, error)),
    };
    if !resp.status().is_success() {
        return SolveStep::Error(EngineError::BadStatus {
            engine,
            status: resp.status().as_u16(),
        });
    }
    let bytes = match read_bounded_body(resp, engine, MAX_CHALLENGE_BYTES).await {
        Ok(bytes) => bytes,
        Err(error) => return SolveStep::Error(error),
    };
    let raw = match String::from_utf8(bytes) {
        Ok(raw) => raw,
        Err(_) => {
            return SolveStep::Error(EngineError::ParseFailed {
                engine,
                reason: "challenge response is not valid UTF-8".to_string(),
            })
        }
    };

    let challenge = match AltchaChallenge::parse(&raw) {
        Ok(challenge) => challenge,
        Err(bound) => return refused_step(bound),
    };
    let started = Instant::now();
    let solution = match challenge.solve(budget) {
        Ok(solution) => solution,
        Err(bound) => return decided_step(TerminalOutcome::Refused { bound }, started),
    };
    let encoded = match challenge.submission(&solution) {
        Ok(encoded) => encoded,
        Err(bound) => return decided_step(TerminalOutcome::Refused { bound }, started),
    };

    let content_type = format!("multipart/form-data; boundary={MULTIPART_BOUNDARY}");
    let body = format!(
        "--{MULTIPART_BOUNDARY}\r\nContent-Disposition: form-data; name=\"altcha\"\r\n\r\n{encoded}\r\n--{MULTIPART_BOUNDARY}--\r\n"
    );
    let elapsed = started.elapsed();
    let request = match client
        .post(verify_url)
        .map_err(|source| EngineError::Http { engine, source })
    {
        Ok(request) => request,
        Err(error) => return SolveStep::Error(error),
    };
    let resp = match request
        .header("Accept", "application/json")
        .header("Content-Type", &content_type)
        .timeout(engine_timeout(timeout))
        .body(body)
        .send()
        .await
    {
        Ok(resp) => resp,
        Err(error) => return SolveStep::Error(map_request_error(engine, error)),
    };

    let status = resp.status();
    let issued = cookie::parse_issued(resp.headers(), COOKIE_NAME);
    if let Err(error) = read_bounded_body(resp, engine, MAX_VERIFY_BYTES).await {
        return SolveStep::Error(error);
    }

    let terminal = if !status.is_success() {
        TerminalOutcome::Refused {
            bound: BOUND_COOKIE_REJECTED,
        }
    } else {
        match issued {
            Some((cookie, lifetime)) => TerminalOutcome::Solved {
                cookie: format!("{COOKIE_NAME}={cookie}"),
                lifetime,
            },
            None => TerminalOutcome::Refused {
                bound: BOUND_MISSING_COOKIE,
            },
        }
    };
    SolveStep::Terminal { terminal, elapsed }
}

fn decode_hex(value: &str) -> Option<Vec<u8>> {
    let bytes = value.as_bytes();
    if bytes.is_empty() || bytes.len() % 2 != 0 {
        return None;
    }
    let mut out = Vec::with_capacity(bytes.len() / 2);
    for pair in bytes.chunks_exact(2) {
        out.push((hex_digit(pair[0])? << 4) | hex_digit(pair[1])?);
    }
    Some(out)
}

fn hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
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
    use crate::meta::challenge::{BOUND_ATTEMPTS, BOUND_WALL_CLOCK};

    fn vector(prefix: &str) -> String {
        format!(
            r#"{{"parameters":{{"algorithm":"PBKDF2/SHA-256","cost":100,"keyLength":32,"keyPrefix":"{prefix}","nonce":"0102030405060708","salt":"0a0b0c0d0e0f1011","keySignature":"00","expiresAt":1794425890}},"signature":"00"}}"#
        )
    }

    #[test]
    fn solves_published_test_vector() {
        let challenge = AltchaChallenge::parse(&vector("b054884358ba2e5e24a53c818185502e"))
            .expect("vector parses");
        let mut budget = SolveBudget::new(1000, Duration::from_secs(5), 5);
        let solution = challenge.solve(&mut budget).expect("vector solves");
        assert_eq!(solution.counter, 7);
        assert_eq!(
            solution.derived_key,
            "b054884358ba2e5e24a53c818185502ee18f0600dc0590c51b9759228fcc7bc8"
        );
        assert_eq!(budget.attempts_used(), 8);
    }

    #[test]
    fn negative_vector_exhausts_attempts() {
        let challenge = AltchaChallenge::parse(&vector("0000000000000000ffffffffffffffff"))
            .expect("negative vector parses");
        let mut budget = SolveBudget::new(64, Duration::from_secs(5), 5);
        assert_eq!(challenge.solve(&mut budget), Err(BOUND_ATTEMPTS));
        assert_eq!(budget.attempts_used(), 64);
        assert!(budget.attempts_used() <= budget.max_attempts());
    }

    #[test]
    fn wall_clock_ceiling_is_enforced_by_cost() {
        let challenge = AltchaChallenge::parse(&vector("0000000000000000ffffffffffffffff"))
            .expect("vector parses");
        let mut budget = SolveBudget::new(u64::MAX, Duration::from_millis(1), 5);
        let started = std::time::Instant::now();
        assert_eq!(challenge.solve(&mut budget), Err(BOUND_WALL_CLOCK));
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "wall-clock refusal took {:?}",
            started.elapsed()
        );
        assert!(budget.attempts_used() >= 1);
    }

    #[test]
    fn unsupported_algorithm_is_named() {
        let raw =
            vector("b054884358ba2e5e24a53c818185502e").replace("PBKDF2/SHA-256", "PBKDF2/SHA-512");
        let challenge = AltchaChallenge::parse(&raw).expect("challenge parses");
        let mut budget = SolveBudget::new(10, Duration::from_secs(1), 5);
        assert_eq!(challenge.solve(&mut budget), Err(BOUND_UNSUPPORTED_SCHEME));
        assert_eq!(budget.attempts_used(), 0);
    }

    #[test]
    fn malformed_parameters_are_refused() {
        for raw in [
            "not json",
            "{}",
            r#"{"parameters":{"algorithm":"PBKDF2/SHA-256"}}"#,
            r#"{"parameters":{"algorithm":"PBKDF2/SHA-256","cost":0,"keyLength":32,"keyPrefix":"aa","nonce":"01","salt":"02"}}"#,
            r#"{"parameters":{"algorithm":"PBKDF2/SHA-256","cost":100,"keyLength":0,"keyPrefix":"aa","nonce":"01","salt":"02"}}"#,
            r#"{"parameters":{"algorithm":"PBKDF2/SHA-256","cost":100,"keyLength":32,"keyPrefix":"zz","nonce":"01","salt":"02"}}"#,
            r#"{"parameters":{"algorithm":"PBKDF2/SHA-256","cost":100,"keyLength":32,"keyPrefix":"a","nonce":"01","salt":"02"}}"#,
            r#"{"parameters":{"algorithm":"PBKDF2/SHA-256","cost":100,"keyLength":32,"keyPrefix":"aa","nonce":"","salt":"02"}}"#,
            r#"{"parameters":{"algorithm":"PBKDF2/SHA-256","cost":100,"keyLength":32,"keyPrefix":"aa","nonce":"01","salt":""}}"#,
        ] {
            assert_eq!(
                AltchaChallenge::parse(raw).err(),
                Some(BOUND_MALFORMED),
                "expected malformed for {raw}"
            );
        }
    }

    #[test]
    fn algorithm_normalization_matches_searxng() {
        assert_eq!(normalize_algorithm("PBKDF2/SHA-256"), "sha256");
        assert_eq!(normalize_algorithm("PBKDF2/SHA256"), "sha256");
        assert_eq!(normalize_algorithm("sha-256"), "sha256");
        assert_eq!(normalize_algorithm("PBKDF2/SHA-512"), "sha512");
    }

    #[test]
    fn submission_is_deterministic_and_uses_fixed_time() {
        let challenge = AltchaChallenge::parse(&vector("b054884358ba2e5e24a53c818185502e"))
            .expect("vector parses");
        let mut budget = SolveBudget::new(1000, Duration::from_secs(5), 5);
        let solution = challenge.solve(&mut budget).expect("vector solves");

        let first = challenge.submission(&solution).expect("submission");
        let second = challenge.submission(&solution).expect("submission");
        assert_eq!(first, second);

        let decoded = base64::engine::general_purpose::STANDARD
            .decode(&first)
            .expect("base64 decodes");
        let payload: Value = serde_json::from_slice(&decoded).expect("json");
        assert_eq!(payload["solution"]["counter"].as_u64(), Some(7));
        assert_eq!(
            payload["solution"]["derivedKey"],
            "b054884358ba2e5e24a53c818185502ee18f0600dc0590c51b9759228fcc7bc8"
        );
        assert_eq!(payload["solution"]["time"].as_u64(), Some(150));
        assert_eq!(
            payload["challenge"]["parameters"]["algorithm"],
            "PBKDF2/SHA-256"
        );
    }

    #[test]
    fn set_cookie_extraction_reads_only_the_named_cookie() {
        let mut headers = http::HeaderMap::new();
        headers.append(
            "set-cookie",
            http::HeaderValue::from_static("other=value; Path=/"),
        );
        headers.append(
            "set-cookie",
            http::HeaderValue::from_static("chllg=abc123; Path=/; HttpOnly; Max-Age=120"),
        );
        assert_eq!(
            cookie::parse_issued(&headers, "chllg"),
            Some(("abc123".to_string(), cookie::PublishedLifetime::MaxAge(120)))
        );
        assert_eq!(cookie::parse_issued(&headers, "missing"), None);
    }
}
