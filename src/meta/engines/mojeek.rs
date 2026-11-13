use std::time::{Duration, SystemTime};

use eggfetch_core::Client;
use scraper::Html;

use super::error::EngineError;
use super::models::SearchResult;
use super::{PageMarker, ScrapeShape};
use crate::core::config::ChallengeSection;
use crate::meta::challenge::{
    altcha, cookie, lookup_pass_cookie, pass_cookie_cache, SolveBudget, SolveOutcome,
    BOUND_COOKIE_REJECTED,
};

const ENGINE: &str = "mojeek";
const MOJEEK_BASE_URL: &str = "https://www.mojeek.com";
const SEARCH_PATH: &str = "/search";
const CHALLENGE_PATH: &str = "/captcha/challenge";
const VERIFY_PATH: &str = "/captcha/verify";
const COOKIE_NAME: &str = "chllg";
const MAX_BODY_BYTES: usize = 2 * 1024 * 1024;

const SHAPE: ScrapeShape = ScrapeShape {
    result_container: "ul.results-standard, ol.results-standard",
    empty_markers: &[
        PageMarker::Selector("div.no-results"),
        PageMarker::Text("No pages found"),
    ],
    blocked_markers: &[
        PageMarker::Selector("div.captcha-wrap"),
        PageMarker::Text("JavaScript is required to complete this challenge"),
        PageMarker::Text("your network appears to be sending automated queries"),
    ],
};

pub async fn search(
    client: &Client,
    query: &str,
    max_results: usize,
    timeout: Duration,
    challenge: &ChallengeSection,
) -> Result<Vec<SearchResult>, EngineError> {
    search_at(
        client,
        MOJEEK_BASE_URL,
        query,
        max_results,
        timeout,
        challenge,
    )
    .await
}

pub(super) async fn search_at(
    client: &Client,
    base_url: &str,
    query: &str,
    max_results: usize,
    timeout: Duration,
    challenge: &ChallengeSection,
) -> Result<Vec<SearchResult>, EngineError> {
    let search_url = endpoint(base_url, SEARCH_PATH);
    let cached_cookie = if challenge.enabled {
        lookup_pass_cookie(ENGINE)
    } else {
        None
    };
    let body = send_search(
        client,
        &search_url,
        query,
        cached_cookie.as_deref(),
        timeout,
    )
    .await?;

    if !challenge.enabled || !is_challenge_page(&body)? {
        return parse(&body, max_results);
    }

    let (cookie, lifetime) = solve_cookie(client, base_url, timeout, challenge).await?;
    let ttl = cookie::reuse_ttl(
        lifetime,
        SystemTime::now(),
        Duration::from_secs(challenge.cookie_ttl_seconds),
    );
    pass_cookie_cache().put(ENGINE, &cookie, ttl);

    let replayed = send_search(client, &search_url, query, Some(&cookie), timeout).await?;
    if is_challenge_page(&replayed)? {
        pass_cookie_cache().remove(ENGINE);
        return Err(blocked(BOUND_COOKIE_REJECTED));
    }
    parse(&replayed, max_results)
}

async fn send_search(
    client: &Client,
    url: &str,
    query: &str,
    cookie: Option<&str>,
    timeout: Duration,
) -> Result<String, EngineError> {
    let mut request = client
        .get(url)
        .map_err(|e| EngineError::Http {
            engine: ENGINE,
            source: e,
        })?
        .query("q", query)
        .header("Accept-Language", "en-US,en;q=0.9")
        .timeout(super::engine_timeout(timeout));
    if let Some(cookie) = cookie {
        request = request.header("Cookie", &format!("{COOKIE_NAME}={cookie}"));
    }
    let resp = request
        .send()
        .await
        .map_err(|e| super::map_request_error(ENGINE, e))?;
    if !resp.status().is_success() {
        return Err(EngineError::BadStatus {
            engine: ENGINE,
            status: resp.status().as_u16(),
        });
    }
    let bytes = super::read_bounded_body(resp, ENGINE, MAX_BODY_BYTES).await?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

async fn solve_cookie(
    client: &Client,
    base_url: &str,
    timeout: Duration,
    challenge: &ChallengeSection,
) -> Result<(String, cookie::PublishedLifetime), EngineError> {
    let solve_timeout = Duration::from_millis(challenge.timeout_ms).min(timeout);
    let mut budget = SolveBudget::new(
        challenge.max_attempts,
        solve_timeout,
        challenge.max_difficulty,
    );
    let outcome = altcha::solve_and_verify(
        client,
        ENGINE,
        &endpoint(base_url, CHALLENGE_PATH),
        &endpoint(base_url, VERIFY_PATH),
        solve_timeout,
        &mut budget,
    )
    .await?;
    match outcome {
        SolveOutcome::Solved { cookie, lifetime } => Ok((cookie, lifetime)),
        SolveOutcome::Refused { bound } => Err(blocked(bound)),
    }
}

fn blocked(bound: &'static str) -> EngineError {
    EngineError::Blocked {
        engine: ENGINE,
        reason: format!("challenge not solved: {bound}"),
    }
}

fn endpoint(base_url: &str, path: &str) -> String {
    format!("{}{path}", base_url.trim_end_matches('/'))
}

fn is_challenge_page(html: &str) -> Result<bool, EngineError> {
    let document = Html::parse_document(html);
    let mut rendered: Option<String> = None;
    for marker in SHAPE.blocked_markers {
        if super::marker_matches(ENGINE, *marker, &document, &mut rendered)? {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(super) fn parse(html: &str, max_results: usize) -> Result<Vec<SearchResult>, EngineError> {
    let document = Html::parse_document(html);

    let list_sel = super::sel(ENGINE, SHAPE.result_container)?;
    let item_sel = super::sel(ENGINE, "li")?;
    let title_sel = super::sel(ENGINE, "h2 a.title, h2 a")?;
    let snippet_sel = super::sel(ENGINE, "p.s")?;

    let mut results = Vec::new();

    let Some(list) = document.select(&list_sel).next() else {
        return super::finalize_scrape(ENGINE, &document, &SHAPE, results);
    };

    for element in list.select(&item_sel) {
        if results.len() >= max_results {
            break;
        }

        let Some(title_el) = element.select(&title_sel).next() else {
            continue;
        };

        let title = title_el
            .text()
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        let title = title.trim().to_string();
        if title.is_empty() {
            continue;
        }

        let href = title_el.value().attr("href").unwrap_or("");
        if !super::is_http_url(href) {
            continue;
        }
        let url = href.to_string();

        let snippet = element
            .select(&snippet_sel)
            .next()
            .map(|el| {
                el.text()
                    .collect::<String>()
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        results.push(SearchResult {
            title,
            url,
            snippet,
            source_engine: ENGINE.to_string(),
            excerpts: Vec::new(),
            published_at: None,
            metadata: Default::default(),
        });
    }

    super::finalize_scrape(ENGINE, &document, &SHAPE, results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meta::challenge::ledger::{ProviderSolveStats, RefusalReason};
    use crate::meta::challenge::solve_ledger;

    fn ledger_stats() -> ProviderSolveStats {
        solve_ledger()
            .snapshot()
            .providers
            .get(ENGINE)
            .cloned()
            .unwrap_or_default()
    }

    fn refusal_delta(
        before: &ProviderSolveStats,
        after: &ProviderSolveStats,
        reason: RefusalReason,
    ) -> u64 {
        after
            .refusals
            .count(reason)
            .saturating_sub(before.refusals.count(reason))
    }

    fn solves_delta(before: &ProviderSolveStats, after: &ProviderSolveStats) -> (u64, u64, u64) {
        (
            after
                .solves_attempted
                .saturating_sub(before.solves_attempted),
            after
                .solves_succeeded
                .saturating_sub(before.solves_succeeded),
            after.solves_errored.saturating_sub(before.solves_errored),
        )
    }

    #[test]
    fn test_parse_extracts_results() {
        let html = r#"
            <ul class="results-standard">
                <li class="r1">
                    <h2><a class="title" href="https://example.com">Example Site</a></h2>
                    <p class="s">An example website for testing.</p>
                </li>
                <li class="r2">
                    <h2><a class="title" href="https://rust-lang.org">Rust Language</a></h2>
                    <p class="s">Systems programming language.</p>
                </li>
            </ul>
        "#;

        let results = parse(html, 10).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].title, "Example Site");
        assert_eq!(results[0].url, "https://example.com");
        assert_eq!(results[1].url, "https://rust-lang.org");
        assert!(results[0].snippet.is_some());
        assert_eq!(results[0].source_engine, "mojeek");
    }

    #[test]
    fn test_parse_collapses_internal_whitespace_in_title() {
        let html = r#"
            <ul class="results-standard">
                <li>
                    <h2><a class="title" href="https://example.com">Rust
                        Programming
                        Language</a></h2>
                </li>
            </ul>
        "#;

        let results = parse(html, 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Rust Programming Language");
    }

    #[test]
    fn test_parse_respects_max_results() {
        let item = r#"
            <li>
                <h2><a class="title" href="https://example.com">Title</a></h2>
                <p class="s">Snippet</p>
            </li>
        "#;
        let html = format!(r#"<ul class="results-standard">{}</ul>"#, item.repeat(5));
        let results = parse(&html, 2).unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_parse_skips_missing_snippet() {
        let html = r#"
            <ul class="results-standard">
                <li>
                    <h2><a class="title" href="https://example.com">No Snippet</a></h2>
                </li>
            </ul>
        "#;
        let results = parse(html, 10).unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].snippet.is_none());
    }

    #[test]
    fn test_parse_skips_non_http_urls() {
        let html = r#"
            <ul class="results-standard">
                <li>
                    <h2><a class="title" href="/relative/path">Relative</a></h2>
                </li>
                <li>
                    <h2><a class="title" href="https://valid.com">Valid</a></h2>
                </li>
            </ul>
        "#;
        let results = parse(html, 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].url, "https://valid.com");
    }

    #[test]
    fn test_parse_skips_missing_title_href() {
        let html = r#"
            <ul class="results-standard">
                <li>
                    <h2><a class="title">No href</a></h2>
                </li>
                <li>
                    <h2><a class="title" href="https://valid.com">Valid</a></h2>
                </li>
            </ul>
        "#;
        let results = parse(html, 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].url, "https://valid.com");
    }

    #[test]
    fn test_parse_returns_empty_when_marker_present() {
        let html = r#"<html><body><div class="no-results">No pages found.</div></body></html>"#;
        let results = parse(html, 10).unwrap();
        assert!(results.is_empty());
    }

    static CHALLENGE_TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

    const CHALLENGE_PAGE: &str = r#"<html><body><div class="captcha-wrap">JavaScript is required to complete this challenge chllg=leaked-cookie-value</div></body></html>"#;

    const RESULT_PAGE: &str = r#"<html><body><ul class="results-standard"><li><h2><a class="title" href="https://example.com">Example Site</a></h2><p class="s">An example website.</p></li></ul></body></html>"#;

    const SOLVABLE_CHALLENGE: &str = r#"{"parameters":{"algorithm":"PBKDF2/SHA-256","cost":100,"keyLength":32,"keyPrefix":"b054884358ba2e5e24a53c818185502e","nonce":"0102030405060708","salt":"0a0b0c0d0e0f1011","keySignature":"00","expiresAt":1794425890},"signature":"00"}"#;

    const UNSOLVABLE_CHALLENGE: &str = r#"{"parameters":{"algorithm":"PBKDF2/SHA-256","cost":100,"keyLength":32,"keyPrefix":"0000000000000000ffffffffffffffff","nonce":"0102030405060708","salt":"0a0b0c0d0e0f1011","keySignature":"00","expiresAt":1794425890},"signature":"00"}"#;

    fn no_cookie_header(request: &httpmock::prelude::HttpMockRequest) -> bool {
        !request.headers.as_ref().is_some_and(|headers| {
            headers
                .iter()
                .any(|(name, _)| name.eq_ignore_ascii_case("cookie"))
        })
    }

    fn test_client() -> Client {
        crate::meta::engines::build_http_client(None).expect("test client")
    }

    #[tokio::test]
    async fn disabled_challenge_keeps_blocked_no_extra_request_no_cookie() {
        use httpmock::prelude::*;

        let _guard = CHALLENGE_TEST_LOCK.lock().await;
        pass_cookie_cache().clear();
        let before = ledger_stats();

        let server = MockServer::start();
        let search_mock = server.mock(|when, then| {
            when.method(GET)
                .path("/search")
                .query_param("q", "rust")
                .matches(no_cookie_header);
            then.status(200)
                .header("content-type", "text/html")
                .body(CHALLENGE_PAGE);
        });
        let verify_mock = server.mock(|when, then| {
            when.method(POST).path("/captcha/verify");
            then.status(200).header("set-cookie", "chllg=unexpected");
        });
        let challenge_mock = server.mock(|when, then| {
            when.method(GET).path("/captcha/challenge");
            then.status(200)
                .header("content-type", "application/json")
                .body(SOLVABLE_CHALLENGE);
        });

        let challenge = ChallengeSection {
            enabled: false,
            ..ChallengeSection::default()
        };
        let err = search_at(
            &test_client(),
            &server.url(""),
            "rust",
            10,
            Duration::from_secs(5),
            &challenge,
        )
        .await
        .expect_err("challenge page blocks");

        assert!(matches!(
            err,
            EngineError::Blocked {
                engine: "mojeek",
                ..
            }
        ));
        assert_eq!(search_mock.hits(), 1);
        assert_eq!(challenge_mock.hits(), 0);
        assert_eq!(verify_mock.hits(), 0);
        let rendered = err.to_string();
        assert!(!rendered.contains("leaked-cookie-value"), "got: {rendered}");

        let after = ledger_stats();
        assert_eq!(solves_delta(&before, &after), (0, 0, 0));
        assert_eq!(after.cookie_hits, before.cookie_hits);
        assert_eq!(after.cookie_misses, before.cookie_misses);
    }

    #[tokio::test]
    async fn enabled_challenge_solves_verifies_and_replays_once() {
        use httpmock::prelude::*;

        let _guard = CHALLENGE_TEST_LOCK.lock().await;
        pass_cookie_cache().clear();
        let before = ledger_stats();

        let server = MockServer::start();
        let replay_mock = server.mock(|when, then| {
            when.method(GET)
                .path("/search")
                .query_param("q", "rust")
                .header("Cookie", "chllg=pass-cookie-123");
            then.status(200)
                .header("content-type", "text/html")
                .body(RESULT_PAGE);
        });
        let challenge_page_mock = server.mock(|when, then| {
            when.method(GET)
                .path("/search")
                .query_param("q", "rust")
                .matches(no_cookie_header);
            then.status(200)
                .header("content-type", "text/html")
                .body(CHALLENGE_PAGE);
        });
        let challenge_mock = server.mock(|when, then| {
            when.method(GET).path("/captcha/challenge");
            then.status(200)
                .header("content-type", "application/json")
                .body(SOLVABLE_CHALLENGE);
        });
        let verify_mock = server.mock(|when, then| {
            when.method(POST).path("/captcha/verify");
            then.status(200)
                .header("set-cookie", "chllg=pass-cookie-123; Path=/; HttpOnly");
        });

        let challenge = ChallengeSection {
            enabled: true,
            max_attempts: 1000,
            timeout_ms: 2000,
            cookie_ttl_seconds: 240,
            ..ChallengeSection::default()
        };
        let results = search_at(
            &test_client(),
            &server.url(""),
            "rust",
            10,
            Duration::from_secs(5),
            &challenge,
        )
        .await
        .expect("solved search returns rows");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].url, "https://example.com");
        assert_eq!(challenge_page_mock.hits(), 1);
        assert_eq!(challenge_mock.hits(), 1);
        assert_eq!(verify_mock.hits(), 1);
        assert_eq!(replay_mock.hits(), 1);
        assert_eq!(
            pass_cookie_cache().get("mojeek").as_deref(),
            Some("pass-cookie-123")
        );

        let after = ledger_stats();
        assert_eq!(solves_delta(&before, &after), (1, 1, 0));
        assert_eq!(after.cookie_misses.saturating_sub(before.cookie_misses), 1);
        assert_eq!(after.cookie_hits, before.cookie_hits);
        assert!(
            after.last_candidates > 0,
            "candidates: {}",
            after.last_candidates
        );
    }

    #[tokio::test]
    async fn enabled_challenge_reuses_cached_cookie_without_solving() {
        use httpmock::prelude::*;

        let _guard = CHALLENGE_TEST_LOCK.lock().await;
        pass_cookie_cache().clear();
        let before = ledger_stats();
        pass_cookie_cache().put("mojeek", "cached-cookie-456", Duration::from_secs(60));

        let server = MockServer::start();
        let search_mock = server.mock(|when, then| {
            when.method(GET)
                .path("/search")
                .query_param("q", "rust")
                .header("Cookie", "chllg=cached-cookie-456");
            then.status(200)
                .header("content-type", "text/html")
                .body(RESULT_PAGE);
        });
        let verify_mock = server.mock(|when, then| {
            when.method(POST).path("/captcha/verify");
            then.status(200).header("set-cookie", "chllg=unexpected");
        });

        let challenge = ChallengeSection {
            enabled: true,
            ..ChallengeSection::default()
        };
        let results = search_at(
            &test_client(),
            &server.url(""),
            "rust",
            10,
            Duration::from_secs(5),
            &challenge,
        )
        .await
        .expect("cached cookie search returns rows");

        assert_eq!(results.len(), 1);
        assert_eq!(search_mock.hits(), 1);
        assert_eq!(verify_mock.hits(), 0);

        let after = ledger_stats();
        assert_eq!(solves_delta(&before, &after), (0, 0, 0));
        assert_eq!(after.cookie_hits.saturating_sub(before.cookie_hits), 1);
        assert_eq!(after.cookie_misses, before.cookie_misses);
    }

    #[tokio::test]
    async fn exhausted_attempts_returns_blocked_naming_the_bound() {
        use httpmock::prelude::*;

        let _guard = CHALLENGE_TEST_LOCK.lock().await;
        pass_cookie_cache().clear();
        let before = ledger_stats();

        let server = MockServer::start();
        let search_mock = server.mock(|when, then| {
            when.method(GET).path("/search");
            then.status(200)
                .header("content-type", "text/html")
                .body(CHALLENGE_PAGE);
        });
        let challenge_mock = server.mock(|when, then| {
            when.method(GET).path("/captcha/challenge");
            then.status(200)
                .header("content-type", "application/json")
                .body(UNSOLVABLE_CHALLENGE);
        });
        let verify_mock = server.mock(|when, then| {
            when.method(POST).path("/captcha/verify");
            then.status(200).header("set-cookie", "chllg=never-used");
        });

        let challenge = ChallengeSection {
            enabled: true,
            max_attempts: 8,
            timeout_ms: 2000,
            ..ChallengeSection::default()
        };
        let err = search_at(
            &test_client(),
            &server.url(""),
            "rust",
            10,
            Duration::from_secs(5),
            &challenge,
        )
        .await
        .expect_err("unsolvable challenge blocks");

        assert!(matches!(
            err,
            EngineError::Blocked {
                engine: "mojeek",
                ..
            }
        ));
        let rendered = err.to_string();
        assert!(rendered.contains("attempts exhausted"), "got: {rendered}");
        assert!(!rendered.contains("leaked-cookie-value"), "got: {rendered}");
        assert!(!rendered.contains("never-used"), "got: {rendered}");
        assert_eq!(search_mock.hits(), 1);
        assert_eq!(challenge_mock.hits(), 1);
        assert_eq!(verify_mock.hits(), 0);

        let after = ledger_stats();
        assert_eq!(
            refusal_delta(&before, &after, RefusalReason::AttemptsExhausted),
            1
        );
        assert_eq!(after.last_candidates, 8);
    }

    #[tokio::test]
    async fn rejected_pass_cookie_returns_blocked_without_leaking_it() {
        use httpmock::prelude::*;

        let _guard = CHALLENGE_TEST_LOCK.lock().await;
        pass_cookie_cache().clear();
        let before = ledger_stats();

        let server = MockServer::start();
        let replay_mock = server.mock(|when, then| {
            when.method(GET)
                .path("/search")
                .header("Cookie", "chllg=rejected-cookie-789");
            then.status(200)
                .header("content-type", "text/html")
                .body(CHALLENGE_PAGE);
        });
        let challenge_page_mock = server.mock(|when, then| {
            when.method(GET).path("/search").matches(no_cookie_header);
            then.status(200)
                .header("content-type", "text/html")
                .body(CHALLENGE_PAGE);
        });
        let challenge_mock = server.mock(|when, then| {
            when.method(GET).path("/captcha/challenge");
            then.status(200)
                .header("content-type", "application/json")
                .body(SOLVABLE_CHALLENGE);
        });
        let verify_mock = server.mock(|when, then| {
            when.method(POST).path("/captcha/verify");
            then.status(200)
                .header("set-cookie", "chllg=rejected-cookie-789; Path=/");
        });

        let challenge = ChallengeSection {
            enabled: true,
            max_attempts: 1000,
            timeout_ms: 2000,
            cookie_ttl_seconds: 60,
            ..ChallengeSection::default()
        };
        let err = search_at(
            &test_client(),
            &server.url(""),
            "rust",
            10,
            Duration::from_secs(5),
            &challenge,
        )
        .await
        .expect_err("replayed challenge still blocks");

        let rendered = err.to_string();
        assert!(rendered.contains("pass cookie rejected"), "got: {rendered}");
        assert!(!rendered.contains("rejected-cookie-789"), "got: {rendered}");
        assert_eq!(challenge_page_mock.hits(), 1);
        assert_eq!(challenge_mock.hits(), 1);
        assert_eq!(verify_mock.hits(), 1);
        assert_eq!(replay_mock.hits(), 1);
        assert_eq!(
            pass_cookie_cache().get("mojeek"),
            None,
            "a rejected replay must evict the rejected cookie"
        );

        let after = ledger_stats();
        assert_eq!(solves_delta(&before, &after), (1, 1, 0));
    }

    #[test]
    fn test_parse_fails_when_no_list_and_no_marker() {
        let html = "<html><body><p>No results</p></body></html>";
        let err = parse(html, 10).expect_err("missing list without marker must fail");
        assert!(matches!(
            err,
            EngineError::ParseFailed {
                engine: "mojeek",
                ..
            }
        ));
    }

    #[tokio::test]
    async fn unsupported_algorithm_is_recorded() {
        use httpmock::prelude::*;

        let _guard = CHALLENGE_TEST_LOCK.lock().await;
        pass_cookie_cache().clear();
        let before = ledger_stats();

        let server = MockServer::start();
        let search_mock = server.mock(|when, then| {
            when.method(GET).path("/search");
            then.status(200)
                .header("content-type", "text/html")
                .body(CHALLENGE_PAGE);
        });
        let challenge_mock = server.mock(|when, then| {
            when.method(GET).path("/captcha/challenge");
            then.status(200)
                .header("content-type", "application/json")
                .body(SOLVABLE_CHALLENGE.replace("PBKDF2/SHA-256", "PBKDF2/SHA-512"));
        });
        let verify_mock = server.mock(|when, then| {
            when.method(POST).path("/captcha/verify");
            then.status(200).header("set-cookie", "chllg=never-used");
        });

        let err = search_at(
            &test_client(),
            &server.url(""),
            "rust",
            10,
            Duration::from_secs(5),
            &ChallengeSection {
                enabled: true,
                ..ChallengeSection::default()
            },
        )
        .await
        .expect_err("an unsupported algorithm blocks");

        assert!(
            err.to_string()
                .contains("challenge not solved: unsupported scheme"),
            "got: {err}"
        );
        assert_eq!(search_mock.hits(), 1);
        assert_eq!(challenge_mock.hits(), 1);
        assert_eq!(verify_mock.hits(), 0);

        let after = ledger_stats();
        assert_eq!(
            refusal_delta(&before, &after, RefusalReason::UnsupportedScheme),
            1
        );
        assert_eq!(solves_delta(&before, &after), (1, 0, 0));
    }

    #[tokio::test]
    async fn malformed_challenge_parameters_are_recorded() {
        use httpmock::prelude::*;

        let _guard = CHALLENGE_TEST_LOCK.lock().await;
        pass_cookie_cache().clear();
        let before = ledger_stats();

        let server = MockServer::start();
        let challenge_page_mock = server.mock(|when, then| {
            when.method(GET).path("/search");
            then.status(200)
                .header("content-type", "text/html")
                .body(CHALLENGE_PAGE);
        });
        let challenge_mock = server.mock(|when, then| {
            when.method(GET).path("/captcha/challenge");
            then.status(200)
                .header("content-type", "application/json")
                .body("{}");
        });
        let verify_mock = server.mock(|when, then| {
            when.method(POST).path("/captcha/verify");
            then.status(200).header("set-cookie", "chllg=never-used");
        });

        let err = search_at(
            &test_client(),
            &server.url(""),
            "rust",
            10,
            Duration::from_secs(5),
            &ChallengeSection {
                enabled: true,
                ..ChallengeSection::default()
            },
        )
        .await
        .expect_err("malformed parameters block");

        assert!(
            err.to_string()
                .contains("challenge not solved: malformed parameters"),
            "got: {err}"
        );
        assert_eq!(challenge_page_mock.hits(), 1);
        assert_eq!(challenge_mock.hits(), 1);
        assert_eq!(verify_mock.hits(), 0);

        let after = ledger_stats();
        assert_eq!(
            refusal_delta(&before, &after, RefusalReason::MalformedParameters),
            1
        );
        assert_eq!(after.last_candidates, 0);
    }

    #[tokio::test]
    async fn missing_pass_cookie_is_recorded() {
        use httpmock::prelude::*;

        let _guard = CHALLENGE_TEST_LOCK.lock().await;
        pass_cookie_cache().clear();
        let before = ledger_stats();

        let server = MockServer::start();
        let challenge_page_mock = server.mock(|when, then| {
            when.method(GET).path("/search");
            then.status(200)
                .header("content-type", "text/html")
                .body(CHALLENGE_PAGE);
        });
        let challenge_mock = server.mock(|when, then| {
            when.method(GET).path("/captcha/challenge");
            then.status(200)
                .header("content-type", "application/json")
                .body(SOLVABLE_CHALLENGE);
        });
        let verify_mock = server.mock(|when, then| {
            when.method(POST).path("/captcha/verify");
            then.status(200)
                .header("content-type", "application/json")
                .body("{}");
        });

        let err = search_at(
            &test_client(),
            &server.url(""),
            "rust",
            10,
            Duration::from_secs(5),
            &ChallengeSection {
                enabled: true,
                max_attempts: 1000,
                timeout_ms: 2000,
                ..ChallengeSection::default()
            },
        )
        .await
        .expect_err("a missing pass cookie blocks");

        assert!(
            err.to_string()
                .contains("challenge not solved: pass cookie absent"),
            "got: {err}"
        );
        assert_eq!(challenge_page_mock.hits(), 1);
        assert_eq!(challenge_mock.hits(), 1);
        assert_eq!(verify_mock.hits(), 1);

        let after = ledger_stats();
        assert_eq!(
            refusal_delta(&before, &after, RefusalReason::PassCookieAbsent),
            1
        );
        assert!(after.last_candidates > 0);
    }

    #[tokio::test]
    async fn rejected_verification_is_recorded_as_a_refusal() {
        use httpmock::prelude::*;

        let _guard = CHALLENGE_TEST_LOCK.lock().await;
        pass_cookie_cache().clear();
        let before = ledger_stats();

        let server = MockServer::start();
        let challenge_page_mock = server.mock(|when, then| {
            when.method(GET).path("/search");
            then.status(200)
                .header("content-type", "text/html")
                .body(CHALLENGE_PAGE);
        });
        let challenge_mock = server.mock(|when, then| {
            when.method(GET).path("/captcha/challenge");
            then.status(200)
                .header("content-type", "application/json")
                .body(SOLVABLE_CHALLENGE);
        });
        let verify_mock = server.mock(|when, then| {
            when.method(POST).path("/captcha/verify");
            then.status(500).body("rejected");
        });

        let err = search_at(
            &test_client(),
            &server.url(""),
            "rust",
            10,
            Duration::from_secs(5),
            &ChallengeSection {
                enabled: true,
                max_attempts: 1000,
                timeout_ms: 2000,
                ..ChallengeSection::default()
            },
        )
        .await
        .expect_err("a rejected verification blocks");

        let rendered = err.to_string();
        assert!(
            rendered.contains("challenge not solved: pass cookie rejected"),
            "got: {rendered}"
        );
        assert!(!rendered.contains("rejected-cookie"), "got: {rendered}");
        assert_eq!(challenge_page_mock.hits(), 1);
        assert_eq!(challenge_mock.hits(), 1);
        assert_eq!(verify_mock.hits(), 1);
        assert_eq!(pass_cookie_cache().get("mojeek"), None);

        let after = ledger_stats();
        assert_eq!(
            refusal_delta(&before, &after, RefusalReason::PassCookieRejected),
            1
        );
        assert_eq!(solves_delta(&before, &after), (1, 0, 0));
    }

    #[tokio::test]
    async fn wall_clock_budget_is_recorded() {
        use httpmock::prelude::*;

        let _guard = CHALLENGE_TEST_LOCK.lock().await;
        pass_cookie_cache().clear();
        let before = ledger_stats();

        let server = MockServer::start();
        let challenge_page_mock = server.mock(|when, then| {
            when.method(GET).path("/search");
            then.status(200)
                .header("content-type", "text/html")
                .body(CHALLENGE_PAGE);
        });
        let challenge_mock = server.mock(|when, then| {
            when.method(GET).path("/captcha/challenge");
            then.status(200)
                .header("content-type", "application/json")
                .body(UNSOLVABLE_CHALLENGE);
        });
        let verify_mock = server.mock(|when, then| {
            when.method(POST).path("/captcha/verify");
            then.status(200).header("set-cookie", "chllg=never-used");
        });

        let err = search_at(
            &test_client(),
            &server.url(""),
            "rust",
            10,
            Duration::from_secs(5),
            &ChallengeSection {
                enabled: true,
                max_attempts: u64::MAX,
                timeout_ms: 100,
                ..ChallengeSection::default()
            },
        )
        .await
        .expect_err("the wall-clock ceiling blocks");

        assert!(
            err.to_string()
                .contains("challenge not solved: wall clock exhausted"),
            "got: {err}"
        );
        assert_eq!(challenge_page_mock.hits(), 1);
        assert_eq!(challenge_mock.hits(), 1);
        assert_eq!(verify_mock.hits(), 0);

        let after = ledger_stats();
        assert_eq!(
            refusal_delta(&before, &after, RefusalReason::WallClockExhausted),
            1
        );
        assert_eq!(solves_delta(&before, &after), (1, 0, 0));
        assert!(after.last_candidates > 0);
    }

    #[tokio::test]
    async fn challenge_transport_failure_is_recorded_as_errored() {
        use httpmock::prelude::*;

        let _guard = CHALLENGE_TEST_LOCK.lock().await;
        pass_cookie_cache().clear();
        let before = ledger_stats();

        let server = MockServer::start();
        let challenge_page_mock = server.mock(|when, then| {
            when.method(GET).path("/search");
            then.status(200)
                .header("content-type", "text/html")
                .body(CHALLENGE_PAGE);
        });
        let challenge_mock = server.mock(|when, then| {
            when.method(GET).path("/captcha/challenge");
            then.status(503).body("unavailable");
        });

        let err = search_at(
            &test_client(),
            &server.url(""),
            "rust",
            10,
            Duration::from_secs(5),
            &ChallengeSection {
                enabled: true,
                ..ChallengeSection::default()
            },
        )
        .await
        .expect_err("a failed challenge fetch is a transport error");

        assert!(
            matches!(
                err,
                EngineError::BadStatus {
                    engine: "mojeek",
                    status: 503
                }
            ),
            "unexpected error: {err:?}"
        );
        assert_eq!(challenge_page_mock.hits(), 1);
        assert_eq!(challenge_mock.hits(), 1);

        let after = ledger_stats();
        assert_eq!(solves_delta(&before, &after), (1, 0, 1));
        assert_eq!(after.refusals.total(), before.refusals.total());
    }

    #[tokio::test]
    async fn pass_cookie_max_age_zero_leaves_no_entry() {
        use httpmock::prelude::*;

        let _guard = CHALLENGE_TEST_LOCK.lock().await;
        pass_cookie_cache().clear();

        let server = MockServer::start();
        let challenge_page_mock = server.mock(|when, then| {
            when.method(GET)
                .path("/search")
                .query_param("q", "rust")
                .matches(no_cookie_header);
            then.status(200)
                .header("content-type", "text/html")
                .body(CHALLENGE_PAGE);
        });
        let challenge_mock = server.mock(|when, then| {
            when.method(GET).path("/captcha/challenge");
            then.status(200)
                .header("content-type", "application/json")
                .body(SOLVABLE_CHALLENGE);
        });
        let verify_mock = server.mock(|when, then| {
            when.method(POST).path("/captcha/verify");
            then.status(200)
                .header("set-cookie", "chllg=pass-cookie-123; Max-Age=0; Path=/");
        });
        let replay_mock = server.mock(|when, then| {
            when.method(GET)
                .path("/search")
                .query_param("q", "rust")
                .header("Cookie", "chllg=pass-cookie-123");
            then.status(200)
                .header("content-type", "text/html")
                .body(RESULT_PAGE);
        });

        let results = search_at(
            &test_client(),
            &server.url(""),
            "rust",
            10,
            Duration::from_secs(5),
            &ChallengeSection {
                enabled: true,
                max_attempts: 1000,
                timeout_ms: 2000,
                cookie_ttl_seconds: 240,
                ..ChallengeSection::default()
            },
        )
        .await
        .expect("a zero-lifetime cookie still answers this search");

        assert_eq!(results.len(), 1);
        assert_eq!(challenge_page_mock.hits(), 1);
        assert_eq!(challenge_mock.hits(), 1);
        assert_eq!(verify_mock.hits(), 1);
        assert_eq!(replay_mock.hits(), 1);
        assert_eq!(
            pass_cookie_cache().get("mojeek"),
            None,
            "Max-Age=0 must store nothing"
        );
    }
}
