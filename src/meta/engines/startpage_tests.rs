use super::*;
use crate::meta::challenge::ledger::{ProviderSolveStats, RefusalReason};
use crate::meta::challenge::solve_ledger;

#[test]
fn test_parse_extracts_results() {
    let html = r#"
        <div class="result css-o7i03b">
            <a class="result-title result-link css-abc" href="https://rust-lang.org/">
                <h2 class="wgl-title css-xyz">Rust Programming Language</h2>
            </a>
            <p class="description css-def">A fast, memory-safe language.</p>
        </div>
        <div class="result css-o7i03b">
            <a class="result-title result-link css-abc" href="https://en.wikipedia.org/wiki/Rust">
                <h2 class="wgl-title css-xyz">Rust - Wikipedia</h2>
            </a>
            <p class="description css-def">Rust is a general-purpose programming language.</p>
        </div>
    "#;

    let results = parse(html, 10).unwrap();
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].title, "Rust Programming Language");
    assert_eq!(results[0].url, "https://rust-lang.org/");
    assert_eq!(results[1].url, "https://en.wikipedia.org/wiki/Rust");
    assert!(results[0].snippet.is_some());
}

#[test]
fn test_parse_respects_max_results() {
    let block = r#"
        <div class="result css-o7i03b">
            <a class="result-title" href="https://example.com">
                <h2 class="wgl-title">Title</h2>
            </a>
        </div>
    "#;
    let html = block.repeat(5);
    let results = parse(&html, 2).unwrap();
    assert_eq!(results.len(), 2);
}

#[test]
fn test_parse_skips_missing_snippet() {
    let html = r#"
        <div class="result css-o7i03b">
            <a class="result-title" href="https://example.com">
                <h2 class="wgl-title">Title</h2>
            </a>
        </div>
    "#;
    let results = parse(html, 10).unwrap();
    assert_eq!(results.len(), 1);
    assert!(results[0].snippet.is_none());
}

#[test]
fn test_parse_skips_non_http_urls() {
    let html = r#"
        <div class="result css-o7i03b">
            <a class="result-title" href="/relative/path">
                <h2 class="wgl-title">Relative</h2>
            </a>
        </div>
        <div class="result css-o7i03b">
            <a class="result-title" href="https://valid.com">
                <h2 class="wgl-title">Valid</h2>
            </a>
        </div>
    "#;
    let results = parse(html, 10).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].url, "https://valid.com");
}

#[test]
fn anubis_challenge_page_is_blocked() {
    let html = r#"
        <html>
          <head>
            <script id="anubis_version" type="application/json">"v1.26.4"</script>
            <script id="anubis_challenge" type="application/json">{"rules":{"algorithm":"fast","difficulty":6}}</script>
          </head>
          <body><div id="app"></div></body>
        </html>
    "#;
    let err = parse(html, 10).expect_err("challenge pages must not parse as empty");
    assert!(
        matches!(
            err,
            EngineError::Blocked {
                engine: "startpage",
                ..
            }
        ),
        "unexpected error: {err:?}"
    );
}

static CHALLENGE_TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

const CHALLENGE_PAGE: &str = r#"<html><head><script id="anubis_version" type="application/json">"v1.26.4"</script><script id="anubis_challenge" type="application/json">{"rules":{"algorithm":"fast","difficulty":2},"challenge":{"issuedAt":"2026-11-11T00:00:00Z","metadata":{"User-Agent":"eggsearch"},"id":"01a1cc17-0000-7000-8000-000000000000","method":"fast","randomData":"abc123def456","redir":"/search?q=rust","policyRuleHash":"00","difficulty":2,"spent":false}}</script></head><body><div id="app"></div></body></html>"#;

const ATTEMPT_BOUND_PAGE: &str = r#"<html><head><script id="anubis_version" type="application/json">"v1.26.4"</script><script id="anubis_challenge" type="application/json">{"rules":{"algorithm":"fast","difficulty":2},"challenge":{"issuedAt":"2026-11-11T00:00:00Z","metadata":{"User-Agent":"eggsearch"},"id":"01a1cc17-0000-7000-8000-000000000000","method":"fast","randomData":"0123456789ab","redir":"/search?q=rust","policyRuleHash":"00","difficulty":2,"spent":false}}</script></head><body><div id="app"></div></body></html>"#;

const DIFFICULTY_SIX_PAGE: &str = r#"<html><head><script id="anubis_version" type="application/json">"v1.26.4"</script><script id="anubis_challenge" type="application/json">{"rules":{"algorithm":"fast","difficulty":6},"challenge":{"issuedAt":"2026-11-11T00:00:00Z","metadata":{"User-Agent":"eggsearch"},"id":"01a1cc17-0000-7000-8000-000000000000","method":"fast","randomData":"570d0182c0ffee","redir":"/search?q=rust","policyRuleHash":"00","difficulty":6,"spent":false}}</script></head><body><div id="app"></div></body></html>"#;

const UNKNOWN_METHOD_PAGE: &str = r#"<html><head><script id="anubis_challenge" type="application/json">{"rules":{"algorithm":"quantum","difficulty":2},"challenge":{"id":"01a1cc17","method":"quantum","randomData":"abc123def456","redir":"/search?q=rust"}}</script></head><body></body></html>"#;

const DIFFICULTY_SEVEN_PAGE: &str = r#"<html><head><script id="anubis_version" type="application/json">"v1.26.4"</script><script id="anubis_challenge" type="application/json">{"rules":{"algorithm":"fast","difficulty":7},"challenge":{"id":"01a1cc17","method":"fast","randomData":"570d0182c0ffee","redir":"/search?q=rust","difficulty":7,"spent":false}}</script></head><body><div id="app"></div></body></html>"#;

const METAREFRESH_CHALLENGE_PAGE: &str = r#"<html><head><meta http-equiv="refresh" content="0; url=/metarefresh-check"><script id="anubis_version" type="application/json">"v1.26.4"</script><script id="anubis_challenge" type="application/json">{"rules":{"algorithm":"metarefresh","difficulty":1},"challenge":{"id":"01a1cc17-0000-7000-8000-000000000000","method":"metarefresh","randomData":"published-token-123","redir":"/search?q=rust","difficulty":1,"spent":false}}</script></head><body><div id="app"></div></body></html>"#;

const RESULT_PAGE: &str = r#"<html><body><div class="result css-o7i03b"><a class="result-title result-link" href="https://example.com"><h2 class="wgl-title">Example Site</h2></a><p class="description">An example website.</p></div></body></html>"#;

const UNPARSEABLE_PAGE: &str = "<html><body><p>Loading</p></body></html>";

const PASS_CHALLENGE_PATH: &str = "/.within.website/x/cmd/anubis/api/pass-challenge";

const CSS_LOAD_CHALLENGE_PATH: &str =
    "/.within.website/x/cmd/anubis/ext/css-load/01a1cc17-0000-7000-8000-000000000000";

fn cookie_header(request: &httpmock::prelude::HttpMockRequest) -> Option<String> {
    request.headers.as_ref().and_then(|headers| {
        headers
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case("cookie"))
            .map(|(_, value)| value.clone())
    })
}

fn no_cookie_header(request: &httpmock::prelude::HttpMockRequest) -> bool {
    cookie_header(request).is_none()
}

fn challenge_cookie_header(request: &httpmock::prelude::HttpMockRequest) -> bool {
    cookie_header(request).as_deref() == Some("spchal-cookie-verification=verif-1; sp_pow=pow-1")
}

fn test_client() -> Client {
    crate::meta::engines::build_http_client(None).expect("test client")
}

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

fn enabled_section() -> ChallengeSection {
    ChallengeSection {
        enabled: true,
        max_attempts: 1000,
        timeout_ms: 2000,
        cookie_ttl_seconds: 240,
        ..ChallengeSection::default()
    }
}

#[tokio::test]
async fn disabled_challenge_keeps_blocked_with_one_request() {
    use httpmock::prelude::*;

    let _guard = CHALLENGE_TEST_LOCK.lock().await;
    pass_cookie_cache().clear();
    pass_cookie_cache().put(ENGINE, "disabled-should-not-send", Duration::from_secs(60));

    let server = MockServer::start();
    let search_mock = server.mock(|when, then| {
        when.method(GET)
            .path("/search")
            .query_param("q", "rust")
            .matches(no_cookie_header);
        then.status(200)
            .header("content-type", "text/html")
            .header("set-cookie", "sp_pow=pow-1; Path=/")
            .body(CHALLENGE_PAGE);
    });
    let pass_mock = server.mock(|when, then| {
        when.method(GET).path(PASS_CHALLENGE_PATH);
        then.status(302)
            .header("set-cookie", "spchal-auth=unexpected");
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
            engine: "startpage",
            ..
        }
    ));
    assert_eq!(search_mock.hits(), 1);
    assert_eq!(pass_mock.hits(), 0);
    let rendered = err.to_string();
    assert!(!rendered.contains("pow-1"), "got: {rendered}");
    assert!(!rendered.contains("unexpected"), "got: {rendered}");
}

#[tokio::test]
async fn enabled_challenge_solves_and_replays_once() {
    use httpmock::prelude::*;

    let _guard = CHALLENGE_TEST_LOCK.lock().await;
    pass_cookie_cache().clear();
    let before = ledger_stats();

    let server = MockServer::start();
    let challenge_page_mock = server.mock(|when, then| {
        when.method(GET)
            .path("/search")
            .query_param("q", "rust")
            .matches(no_cookie_header);
        then.status(200)
            .header("content-type", "text/html")
            .header(
                "set-cookie",
                "spchal-cookie-verification=verif-1; Path=/; Domain=startpage.com; Secure; SameSite=None",
            )
            .header("set-cookie", "sp_pow=pow-1; Max-Age=300; HttpOnly; Path=/")
            .body(CHALLENGE_PAGE);
    });
    let pass_mock = server.mock(|when, then| {
        when.method(GET)
            .path(PASS_CHALLENGE_PATH)
            .query_param("id", "01a1cc17-0000-7000-8000-000000000000")
            .query_param(
                "response",
                "002786c8b408cf14bf6ddc2aeb6b6f1fcdf6b453986796fbec919a88de759708",
            )
            .query_param("nonce", "185")
            .query_param("redir", "/search?q=rust")
            .query_param("elapsedTime", "1")
            .matches(challenge_cookie_header);
        then.status(302)
            .header("location", "/search?q=rust")
            .header(
                "set-cookie",
                "spchal-auth=pass-cookie-123; Path=/; HttpOnly",
            );
    });
    let replay_mock = server.mock(|when, then| {
        when.method(GET)
            .path("/search")
            .query_param("q", "rust")
            .header("Cookie", "spchal-auth=pass-cookie-123");
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
        &enabled_section(),
    )
    .await
    .expect("solved search returns rows");

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].url, "https://example.com");
    assert_eq!(challenge_page_mock.hits(), 1);
    assert_eq!(pass_mock.hits(), 1);
    assert_eq!(replay_mock.hits(), 1);
    assert_eq!(
        pass_cookie_cache().get(ENGINE).as_deref(),
        Some("spchal-auth=pass-cookie-123")
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
async fn prefixed_challenge_solves_under_the_published_prefix() {
    use httpmock::prelude::*;

    let _guard = CHALLENGE_TEST_LOCK.lock().await;
    pass_cookie_cache().clear();

    let server = MockServer::start();
    let page = CHALLENGE_PAGE.replacen(
        "<script id=\"anubis_version\"",
        "<script id=\"anubis_base_prefix\" type=\"application/json\">\"/anubis\"</script><script id=\"anubis_version\"",
        1,
    );
    let challenge_page_mock = server.mock(|when, then| {
        when.method(GET)
            .path("/search")
            .query_param("q", "rust")
            .matches(no_cookie_header);
        then.status(200)
            .header("content-type", "text/html")
            .body(page);
    });
    let pass_mock = server.mock(|when, then| {
        when.method(GET)
            .path("/anubis/.within.website/x/cmd/anubis/api/pass-challenge")
            .query_param("nonce", "185");
        then.status(302)
            .header("set-cookie", "spchal-auth=prefixed-pass; Path=/");
    });
    let replay_mock = server.mock(|when, then| {
        when.method(GET)
            .path("/search")
            .query_param("q", "rust")
            .header("Cookie", "spchal-auth=prefixed-pass");
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
        &enabled_section(),
    )
    .await
    .expect("prefixed challenge solves");

    assert_eq!(results.len(), 1);
    assert_eq!(challenge_page_mock.hits(), 1);
    assert_eq!(pass_mock.hits(), 1);
    assert_eq!(replay_mock.hits(), 1);
}

#[tokio::test]
async fn slow_method_solves_through_the_fast_path() {
    use httpmock::prelude::*;

    let _guard = CHALLENGE_TEST_LOCK.lock().await;
    pass_cookie_cache().clear();

    let server = MockServer::start();
    let page = CHALLENGE_PAGE.replace("\"method\":\"fast\"", "\"method\":\"slow\"");
    let challenge_page_mock = server.mock(|when, then| {
        when.method(GET)
            .path("/search")
            .query_param("q", "rust")
            .matches(no_cookie_header);
        then.status(200)
            .header("content-type", "text/html")
            .body(page);
    });
    let pass_mock = server.mock(|when, then| {
        when.method(GET)
            .path(PASS_CHALLENGE_PATH)
            .query_param("nonce", "185")
            .query_param(
                "response",
                "002786c8b408cf14bf6ddc2aeb6b6f1fcdf6b453986796fbec919a88de759708",
            );
        then.status(302)
            .header("set-cookie", "spchal-auth=slow-pass; Path=/");
    });
    let replay_mock = server.mock(|when, then| {
        when.method(GET)
            .path("/search")
            .query_param("q", "rust")
            .header("Cookie", "spchal-auth=slow-pass");
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
        &enabled_section(),
    )
    .await
    .expect("slow challenge solves");

    assert_eq!(results.len(), 1);
    assert_eq!(challenge_page_mock.hits(), 1);
    assert_eq!(pass_mock.hits(), 1);
    assert_eq!(replay_mock.hits(), 1);
}

#[tokio::test]
async fn spent_challenge_is_blocked_without_a_submission() {
    use httpmock::prelude::*;

    let _guard = CHALLENGE_TEST_LOCK.lock().await;
    pass_cookie_cache().clear();
    let before = ledger_stats();

    let server = MockServer::start();
    let page = CHALLENGE_PAGE.replace("\"spent\":false", "\"spent\":true");
    let challenge_page_mock = server.mock(|when, then| {
        when.method(GET).path("/search").query_param("q", "rust");
        then.status(200)
            .header("content-type", "text/html")
            .body(page);
    });
    let pass_mock = server.mock(|when, then| {
        when.method(GET).path(PASS_CHALLENGE_PATH);
        then.status(302)
            .header("set-cookie", "spchal-auth=unexpected");
    });

    let err = search_at(
        &test_client(),
        &server.url(""),
        "rust",
        10,
        Duration::from_secs(5),
        &enabled_section(),
    )
    .await
    .expect_err("spent challenge blocks");

    assert!(
        err.to_string()
            .contains("challenge not solved: already spent"),
        "got: {err}"
    );
    assert_eq!(challenge_page_mock.hits(), 1);
    assert_eq!(pass_mock.hits(), 0);

    let after = ledger_stats();
    assert_eq!(
        refusal_delta(&before, &after, RefusalReason::AlreadySpent),
        1
    );
    assert_eq!(solves_delta(&before, &after), (1, 0, 0));
}

#[tokio::test]
async fn projection_breach_is_blocked_without_a_submission() {
    use httpmock::prelude::*;

    let _guard = CHALLENGE_TEST_LOCK.lock().await;
    pass_cookie_cache().clear();
    let before = ledger_stats();

    let server = MockServer::start();
    let challenge_page_mock = server.mock(|when, then| {
        when.method(GET).path("/search").query_param("q", "rust");
        then.status(200)
            .header("content-type", "text/html")
            .body(DIFFICULTY_SEVEN_PAGE);
    });
    let pass_mock = server.mock(|when, then| {
        when.method(GET).path(PASS_CHALLENGE_PATH);
        then.status(302)
            .header("set-cookie", "spchal-auth=unexpected");
    });

    let challenge = ChallengeSection {
        enabled: true,
        max_difficulty: 7,
        max_attempts: 2_000_000,
        timeout_ms: 20000,
        cookie_ttl_seconds: 240,
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
    .expect_err("projection breach blocks");

    assert!(
        err.to_string().contains(
            "anubis difficulty 7 projects 268M candidates, beyond the configured attempt ceiling 2000000"
        ),
        "got: {err}"
    );
    assert_eq!(challenge_page_mock.hits(), 1);
    assert_eq!(pass_mock.hits(), 0);

    let after = ledger_stats();
    assert_eq!(
        refusal_delta(&before, &after, RefusalReason::UnsafeBasePrefix),
        1,
        "a dynamic projection refusal still records an attributable key"
    );
    assert_eq!(solves_delta(&before, &after), (1, 0, 0));
    assert_eq!(after.last_candidates, 0);
}

#[tokio::test]
async fn metarefresh_challenge_solves_and_replays_once() {
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
            .body(METAREFRESH_CHALLENGE_PAGE);
    });
    let submit_mock = server.mock(|when, then| {
        when.method(GET)
            .path("/metarefresh-check")
            .query_param("id", "01a1cc17-0000-7000-8000-000000000000")
            .query_param("challenge", "published-token-123");
        then.status(200)
            .header("set-cookie", "spchal-auth=metarefresh-pass; Path=/");
    });
    let replay_mock = server.mock(|when, then| {
        when.method(GET)
            .path("/search")
            .query_param("q", "rust")
            .header("Cookie", "spchal-auth=metarefresh-pass");
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
        &enabled_section(),
    )
    .await
    .expect("metarefresh challenge solves");

    assert_eq!(results.len(), 1);
    assert_eq!(challenge_page_mock.hits(), 1);
    assert_eq!(submit_mock.hits(), 1);
    assert_eq!(replay_mock.hits(), 1);
}

#[tokio::test]
async fn metarefresh_wait_beyond_budget_is_blocked_without_a_submission() {
    use httpmock::prelude::*;

    let _guard = CHALLENGE_TEST_LOCK.lock().await;
    pass_cookie_cache().clear();
    let before = ledger_stats();

    let server = MockServer::start();
    let page = METAREFRESH_CHALLENGE_PAGE.replace(
        "content=\"0; url=/metarefresh-check\"",
        "content=\"5; url=/metarefresh-check\"",
    );
    let challenge_page_mock = server.mock(|when, then| {
        when.method(GET).path("/search").query_param("q", "rust");
        then.status(200)
            .header("content-type", "text/html")
            .body(page);
    });
    let submit_mock = server.mock(|when, then| {
        when.method(GET).path("/metarefresh-check");
        then.status(200)
            .header("set-cookie", "spchal-auth=unexpected");
    });

    let err = search_at(
        &test_client(),
        &server.url(""),
        "rust",
        10,
        Duration::from_secs(5),
        &enabled_section(),
    )
    .await
    .expect_err("wait beyond the budget blocks");

    assert!(
        err.to_string()
            .contains("challenge not solved: wall clock exhausted"),
        "got: {err}"
    );
    assert_eq!(challenge_page_mock.hits(), 1);
    assert_eq!(submit_mock.hits(), 0);

    let after = ledger_stats();
    assert_eq!(
        refusal_delta(&before, &after, RefusalReason::WallClockExhausted),
        1
    );
    assert_eq!(after.last_candidates, 0);
}

#[tokio::test]
async fn disabled_metarefresh_keeps_blocked_with_one_request() {
    use httpmock::prelude::*;

    let _guard = CHALLENGE_TEST_LOCK.lock().await;
    pass_cookie_cache().clear();

    let server = MockServer::start();
    let challenge_page_mock = server.mock(|when, then| {
        when.method(GET).path("/search").query_param("q", "rust");
        then.status(200)
            .header("content-type", "text/html")
            .body(METAREFRESH_CHALLENGE_PAGE);
    });
    let submit_mock = server.mock(|when, then| {
        when.method(GET).path("/metarefresh-check");
        then.status(200)
            .header("set-cookie", "spchal-auth=unexpected");
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

    assert!(matches!(err, EngineError::Blocked { .. }));
    assert_eq!(challenge_page_mock.hits(), 1);
    assert_eq!(submit_mock.hits(), 0);
}

#[tokio::test]
async fn cached_pass_cookie_is_sent_proactively() {
    use httpmock::prelude::*;

    let _guard = CHALLENGE_TEST_LOCK.lock().await;
    pass_cookie_cache().clear();
    let before = ledger_stats();
    pass_cookie_cache().put(
        ENGINE,
        "spchal-auth=cached-cookie-456",
        Duration::from_secs(60),
    );

    let server = MockServer::start();
    let search_mock = server.mock(|when, then| {
        when.method(GET)
            .path("/search")
            .query_param("q", "rust")
            .header("Cookie", "spchal-auth=cached-cookie-456");
        then.status(200)
            .header("content-type", "text/html")
            .body(RESULT_PAGE);
    });
    let pass_mock = server.mock(|when, then| {
        when.method(GET).path(PASS_CHALLENGE_PATH);
        then.status(302)
            .header("set-cookie", "spchal-auth=unexpected");
    });

    let results = search_at(
        &test_client(),
        &server.url(""),
        "rust",
        10,
        Duration::from_secs(5),
        &enabled_section(),
    )
    .await
    .expect("cached cookie search returns rows");

    assert_eq!(results.len(), 1);
    assert_eq!(search_mock.hits(), 1);
    assert_eq!(pass_mock.hits(), 0);

    let after = ledger_stats();
    assert_eq!(solves_delta(&before, &after), (0, 0, 0));
    assert_eq!(after.cookie_hits.saturating_sub(before.cookie_hits), 1);
    assert_eq!(after.cookie_misses, before.cookie_misses);
}

#[tokio::test]
async fn above_ceiling_is_blocked_naming_both_values() {
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
            .header("set-cookie", "sp_pow=pow-1; Path=/")
            .body(DIFFICULTY_SIX_PAGE);
    });
    let pass_mock = server.mock(|when, then| {
        when.method(GET).path(PASS_CHALLENGE_PATH);
        then.status(302).header("set-cookie", "spchal-auth=never");
    });

    let challenge = ChallengeSection {
        enabled: true,
        max_difficulty: 5,
        max_attempts: 100_000_000,
        timeout_ms: 8000,
        ..ChallengeSection::default()
    };
    let err = search_at(
        &test_client(),
        &server.url(""),
        "rust",
        10,
        Duration::from_secs(10),
        &challenge,
    )
    .await
    .expect_err("above-ceiling challenge blocks");

    match &err {
        EngineError::Blocked { engine, reason } => {
            assert_eq!(*engine, "startpage");
            assert_eq!(reason, "anubis difficulty 6 exceeds configured ceiling 5");
        }
        other => panic!("unexpected error: {other:?}"),
    }
    assert_eq!(search_mock.hits(), 1);
    assert_eq!(pass_mock.hits(), 0);

    let after = ledger_stats();
    assert_eq!(solves_delta(&before, &after), (1, 0, 0));
    assert_eq!(
        refusal_delta(&before, &after, RefusalReason::DifficultyAboveCeiling),
        1
    );
    assert_eq!(after.last_candidates, 0);
}

#[tokio::test]
async fn unknown_method_is_blocked_without_solving() {
    use httpmock::prelude::*;

    let _guard = CHALLENGE_TEST_LOCK.lock().await;
    pass_cookie_cache().clear();
    let before = ledger_stats();

    let server = MockServer::start();
    let search_mock = server.mock(|when, then| {
        when.method(GET).path("/search").query_param("q", "rust");
        then.status(200)
            .header("content-type", "text/html")
            .body(UNKNOWN_METHOD_PAGE);
    });
    let pass_mock = server.mock(|when, then| {
        when.method(GET).path(PASS_CHALLENGE_PATH);
        then.status(302).header("set-cookie", "spchal-auth=never");
    });

    let err = search_at(
        &test_client(),
        &server.url(""),
        "rust",
        10,
        Duration::from_secs(5),
        &enabled_section(),
    )
    .await
    .expect_err("unknown method blocks");

    let rendered = err.to_string();
    assert!(rendered.contains("unsupported scheme"), "got: {rendered}");
    assert_eq!(search_mock.hits(), 1);
    assert_eq!(pass_mock.hits(), 0);

    let after = ledger_stats();
    assert_eq!(
        refusal_delta(&before, &after, RefusalReason::UnsupportedScheme),
        1
    );
    assert_eq!(solves_delta(&before, &after), (1, 0, 0));
}

#[tokio::test]
async fn rejected_pass_cookie_blocks_without_leaking_it() {
    use httpmock::prelude::*;

    let _guard = CHALLENGE_TEST_LOCK.lock().await;
    pass_cookie_cache().clear();
    let before = ledger_stats();

    let server = MockServer::start();
    let first_mock = server.mock(|when, then| {
        when.method(GET)
            .path("/search")
            .query_param("q", "rust")
            .matches(no_cookie_header);
        then.status(200)
            .header("content-type", "text/html")
            .header("set-cookie", "sp_pow=pow-1; Path=/")
            .body(CHALLENGE_PAGE);
    });
    let pass_mock = server.mock(|when, then| {
        when.method(GET).path(PASS_CHALLENGE_PATH);
        then.status(302)
            .header("set-cookie", "spchal-auth=rejected-cookie-789; Path=/");
    });
    let replay_mock = server.mock(|when, then| {
        when.method(GET)
            .path("/search")
            .query_param("q", "rust")
            .header("Cookie", "spchal-auth=rejected-cookie-789");
        then.status(200)
            .header("content-type", "text/html")
            .body(CHALLENGE_PAGE);
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
    assert!(!rendered.contains("pow-1"), "got: {rendered}");
    assert_eq!(first_mock.hits(), 1);
    assert_eq!(pass_mock.hits(), 1);
    assert_eq!(replay_mock.hits(), 1);
    assert_eq!(
        pass_cookie_cache().get(ENGINE),
        None,
        "a rejected replay must evict the rejected cookie"
    );

    let after = ledger_stats();
    assert_eq!(solves_delta(&before, &after), (1, 1, 0));
    assert_eq!(
        refusal_delta(&before, &after, RefusalReason::PassCookieRejected),
        0,
        "the engine-level replay rejection is not a solver refusal"
    );
}

#[tokio::test]
async fn solved_page_that_does_not_parse_stays_parse_failed() {
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
            .header("set-cookie", "sp_pow=pow-1; Path=/")
            .body(CHALLENGE_PAGE);
    });
    let pass_mock = server.mock(|when, then| {
        when.method(GET).path(PASS_CHALLENGE_PATH);
        then.status(302)
            .header("set-cookie", "spchal-auth=pass-cookie-123");
    });
    let replay_mock = server.mock(|when, then| {
        when.method(GET)
            .path("/search")
            .query_param("q", "rust")
            .header("Cookie", "spchal-auth=pass-cookie-123");
        then.status(200)
            .header("content-type", "text/html")
            .body(UNPARSEABLE_PAGE);
    });

    let err = search_at(
        &test_client(),
        &server.url(""),
        "rust",
        10,
        Duration::from_secs(5),
        &enabled_section(),
    )
    .await
    .expect_err("solved but unparseable page is a parse failure");

    assert!(
        matches!(
            err,
            EngineError::ParseFailed {
                engine: "startpage",
                ..
            }
        ),
        "unexpected error: {err:?}"
    );
    assert_eq!(challenge_page_mock.hits(), 1);
    assert_eq!(pass_mock.hits(), 1);
    assert_eq!(replay_mock.hits(), 1);
}

fn payload_page(payload: &str) -> String {
    format!(
        r#"<html><body><div id="app"></div><script>window.__DATA__ = React.createElement(UIStartpage.AppSerpWeb, {payload})</script></body></html>"#
    )
}

const PAYLOAD_RESULTS: &str = r#"{"render":{"presenter":{"regions":{"mainline":[{"display_type":"web-google","results":[{"clickUrl":"https://www.rust-lang.org/en-US","title":"<b>Rust Programming Language</b>","description":"<b>Rust</b> is blazingly fast &amp; efficient&nbsp;..."},{"clickUrl":"https://en.wikipedia.org/wiki/Rust_(programming_language)","title":"<b>Rust</b> (programming language) - Wikipedia","description":"A general-purpose programming language."}]}]}}}}"#;

#[test]
fn payload_page_parses_rows() {
    let results = parse(&payload_page(PAYLOAD_RESULTS), 10).expect("payload rows");
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].title, "Rust Programming Language");
    assert_eq!(results[0].url, "https://www.rust-lang.org/en-US");
    assert_eq!(
        results[0].snippet.as_deref(),
        Some("Rust is blazingly fast & efficient ...")
    );
    assert_eq!(
        results[1].url,
        "https://en.wikipedia.org/wiki/Rust_(programming_language)"
    );
}

#[test]
fn payload_takes_precedence_over_dom_container() {
    let html = format!(
        r#"<html><body><div class="result"><a class="result-title" href="https://dom.example.com"><h2 class="wgl-title">Dom</h2></a></div><script>React.createElement(UIStartpage.AppSerpWeb, {PAYLOAD_RESULTS})</script></body></html>"#
    );
    let results = parse(&html, 10).expect("payload rows");
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].url, "https://www.rust-lang.org/en-US");
}

#[test]
fn missing_payload_falls_back_to_dom_parser() {
    let results = parse(RESULT_PAGE, 10).expect("dom rows");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].url, "https://example.com");
}

#[test]
fn renamed_click_url_is_parse_failed() {
    let html = payload_page(PAYLOAD_RESULTS).replace("clickUrl", "destinationUrl");
    let err = parse(&html, 10).expect_err("renamed payload field is a structural break");
    match err {
        EngineError::ParseFailed { engine, reason } => {
            assert_eq!(engine, "startpage");
            assert!(
                reason.contains("payload"),
                "reason must name the payload: {reason}"
            );
            assert!(
                reason.contains("div.result"),
                "reason must name the container: {reason}"
            );
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn renamed_title_is_parse_failed() {
    let html = payload_page(PAYLOAD_RESULTS).replace("\"title\"", "\"headline\"");
    let err = parse(&html, 10).expect_err("renamed title field is a structural break");
    assert!(
        matches!(
            err,
            EngineError::ParseFailed {
                engine: "startpage",
                ..
            }
        ),
        "unexpected error: {err:?}"
    );
}

#[test]
fn truncated_payload_is_parse_failed() {
    let html = payload_page(PAYLOAD_RESULTS);
    let truncated = &html[..html.len() - 40];
    let err = parse(truncated, 10).expect_err("truncated payload is a structural break");
    assert!(
        matches!(
            err,
            EngineError::ParseFailed {
                engine: "startpage",
                ..
            }
        ),
        "unexpected error: {err:?}"
    );
}

#[test]
fn payload_without_rows_or_marker_is_parse_failed() {
    let html = payload_page(r#"{"render":{"presenter":{"regions":{"mainline":[]}}}}"#);
    let err = parse(&html, 10).expect_err("no rows and no marker is a structural break");
    match err {
        EngineError::ParseFailed { engine, reason } => {
            assert_eq!(engine, "startpage");
            assert!(
                reason.contains("payload") && reason.contains("div.result"),
                "reason must name payload and container: {reason}"
            );
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn payload_without_rows_and_empty_marker_is_empty_state() {
    let html = r#"<html><body><div class="no-results">No search results were found for <strong>zqxjvwkkplmnrtbqzz</strong>.</div><script>React.createElement(UIStartpage.AppSerpWeb, {"render":{"presenter":{"regions":{"mainline":[{"display_type":"web-google","results":[]}]}}}})</script></body></html>"#;
    let results = parse(html, 10).expect("documented empty state");
    assert!(results.is_empty());
}

#[test]
fn payload_only_page_with_no_marker_names_the_container() {
    let err = parse(UNPARSEABLE_PAGE, 10).expect_err("no payload and no container");
    match err {
        EngineError::ParseFailed { reason, .. } => {
            assert!(reason.contains("div.result"), "got: {reason}");
            assert!(!reason.contains("payload"), "got: {reason}");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[tokio::test]
async fn malformed_challenge_parameters_are_recorded() {
    use httpmock::prelude::*;

    let _guard = CHALLENGE_TEST_LOCK.lock().await;
    pass_cookie_cache().clear();
    let before = ledger_stats();

    let server = MockServer::start();
    let page = r#"<html><head><script id="anubis_challenge" type="application/json">{not-json</script></head><body></body></html>"#;
    let search_mock = server.mock(|when, then| {
        when.method(GET).path("/search").query_param("q", "rust");
        then.status(200)
            .header("content-type", "text/html")
            .body(page);
    });
    let pass_mock = server.mock(|when, then| {
        when.method(GET).path(PASS_CHALLENGE_PATH);
        then.status(302)
            .header("set-cookie", "spchal-auth=unexpected");
    });

    let err = search_at(
        &test_client(),
        &server.url(""),
        "rust",
        10,
        Duration::from_secs(5),
        &enabled_section(),
    )
    .await
    .expect_err("malformed challenge parameters block");

    assert!(
        err.to_string()
            .contains("challenge not solved: malformed parameters"),
        "got: {err}"
    );
    assert_eq!(search_mock.hits(), 1);
    assert_eq!(pass_mock.hits(), 0);

    let after = ledger_stats();
    assert_eq!(
        refusal_delta(&before, &after, RefusalReason::MalformedParameters),
        1
    );
    assert_eq!(solves_delta(&before, &after), (1, 0, 0));
    assert_eq!(after.last_candidates, 0);
}

#[tokio::test]
async fn missing_pass_cookie_is_recorded() {
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
    let pass_mock = server.mock(|when, then| {
        when.method(GET).path(PASS_CHALLENGE_PATH);
        then.status(200)
            .header("content-type", "text/html")
            .body("no cookie here");
    });

    let err = search_at(
        &test_client(),
        &server.url(""),
        "rust",
        10,
        Duration::from_secs(5),
        &enabled_section(),
    )
    .await
    .expect_err("a missing pass cookie blocks");

    assert!(
        err.to_string()
            .contains("challenge not solved: pass cookie absent"),
        "got: {err}"
    );
    assert_eq!(search_mock.hits(), 1);
    assert_eq!(pass_mock.hits(), 1);
    assert_eq!(pass_cookie_cache().get(ENGINE), None);

    let after = ledger_stats();
    assert_eq!(
        refusal_delta(&before, &after, RefusalReason::PassCookieAbsent),
        1
    );
    assert_eq!(solves_delta(&before, &after), (1, 0, 0));
    assert!(after.last_candidates > 0);
}

#[tokio::test]
async fn exhausted_attempts_are_recorded() {
    use httpmock::prelude::*;

    let _guard = CHALLENGE_TEST_LOCK.lock().await;
    pass_cookie_cache().clear();
    let before = ledger_stats();

    let server = MockServer::start();
    let search_mock = server.mock(|when, then| {
        when.method(GET).path("/search").query_param("q", "rust");
        then.status(200)
            .header("content-type", "text/html")
            .body(ATTEMPT_BOUND_PAGE);
    });
    let pass_mock = server.mock(|when, then| {
        when.method(GET).path(PASS_CHALLENGE_PATH);
        then.status(302)
            .header("set-cookie", "spchal-auth=unexpected");
    });

    let challenge = ChallengeSection {
        enabled: true,
        max_attempts: 256,
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
    .expect_err("the attempt ceiling blocks");

    assert!(
        err.to_string()
            .contains("challenge not solved: attempts exhausted"),
        "got: {err}"
    );
    assert_eq!(search_mock.hits(), 1);
    assert_eq!(pass_mock.hits(), 0);

    let after = ledger_stats();
    assert_eq!(
        refusal_delta(&before, &after, RefusalReason::AttemptsExhausted),
        1
    );
    assert_eq!(after.last_candidates, 256);
}

#[tokio::test]
async fn rejected_verification_is_recorded_as_a_refusal() {
    use httpmock::prelude::*;

    let _guard = CHALLENGE_TEST_LOCK.lock().await;
    pass_cookie_cache().clear();
    let before = ledger_stats();

    let server = MockServer::start();
    let search_mock = server.mock(|when, then| {
        when.method(GET).path("/search").query_param("q", "rust");
        then.status(200)
            .header("content-type", "text/html")
            .body(CHALLENGE_PAGE);
    });
    let pass_mock = server.mock(|when, then| {
        when.method(GET).path(PASS_CHALLENGE_PATH);
        then.status(500).body("rejected");
    });

    let err = search_at(
        &test_client(),
        &server.url(""),
        "rust",
        10,
        Duration::from_secs(5),
        &enabled_section(),
    )
    .await
    .expect_err("a rejected verification blocks");

    let rendered = err.to_string();
    assert!(
        rendered.contains("challenge not solved: pass cookie rejected"),
        "got: {rendered}"
    );
    assert!(!rendered.contains("rejected-cookie"), "got: {rendered}");
    assert_eq!(search_mock.hits(), 1);
    assert_eq!(pass_mock.hits(), 1);

    let after = ledger_stats();
    assert_eq!(
        refusal_delta(&before, &after, RefusalReason::PassCookieRejected),
        1
    );
    assert_eq!(solves_delta(&before, &after), (1, 0, 0));
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
    let pass_mock = server.mock(|when, then| {
        when.method(GET).path(PASS_CHALLENGE_PATH);
        then.status(302).header(
            "set-cookie",
            "spchal-auth=pass-cookie-123; Max-Age=0; Path=/",
        );
    });
    let replay_mock = server.mock(|when, then| {
        when.method(GET)
            .path("/search")
            .query_param("q", "rust")
            .header("Cookie", "spchal-auth=pass-cookie-123");
        then.status(200)
            .header("content-type", "text/html")
            .body(RESULT_PAGE);
    });

    let err = search_at(
        &test_client(),
        &server.url(""),
        "rust",
        10,
        Duration::from_secs(5),
        &enabled_section(),
    )
    .await
    .expect_err("a deletion cookie is never replayed");

    assert!(
        err.to_string()
            .contains("challenge not solved: pass cookie rejected"),
        "got: {err}"
    );
    assert_eq!(challenge_page_mock.hits(), 1);
    assert_eq!(pass_mock.hits(), 1);
    assert_eq!(replay_mock.hits(), 0);
    assert_eq!(
        pass_cookie_cache().get(ENGINE),
        None,
        "Max-Age=0 must store nothing"
    );
}

#[tokio::test]
async fn pass_cookie_max_age_beyond_the_ceiling_is_stored() {
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
    let pass_mock = server.mock(|when, then| {
        when.method(GET).path(PASS_CHALLENGE_PATH);
        then.status(302).header(
            "set-cookie",
            "spchal-auth=pass-cookie-123; Max-Age=300; HttpOnly; Path=/",
        );
    });
    let replay_mock = server.mock(|when, then| {
        when.method(GET)
            .path("/search")
            .query_param("q", "rust")
            .header("Cookie", "spchal-auth=pass-cookie-123");
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
        &enabled_section(),
    )
    .await
    .expect("a long-lived cookie answers this search");

    assert_eq!(results.len(), 1);
    assert_eq!(challenge_page_mock.hits(), 1);
    assert_eq!(pass_mock.hits(), 1);
    assert_eq!(replay_mock.hits(), 1);
    assert_eq!(
        pass_cookie_cache().get(ENGINE).as_deref(),
        Some("spchal-auth=pass-cookie-123"),
        "the clamp must cap the published lifetime, not drop it"
    );
}

#[tokio::test]
async fn zero_cookie_ttl_solves_on_every_search() {
    use httpmock::prelude::*;

    let _guard = CHALLENGE_TEST_LOCK.lock().await;
    pass_cookie_cache().clear();
    let before = ledger_stats();

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
    let pass_mock = server.mock(|when, then| {
        when.method(GET).path(PASS_CHALLENGE_PATH);
        then.status(302)
            .header("set-cookie", "spchal-auth=pass-cookie-123; Max-Age=3600");
    });
    let replay_mock = server.mock(|when, then| {
        when.method(GET)
            .path("/search")
            .query_param("q", "rust")
            .header("Cookie", "spchal-auth=pass-cookie-123");
        then.status(200)
            .header("content-type", "text/html")
            .body(RESULT_PAGE);
    });

    let challenge = ChallengeSection {
        enabled: true,
        max_attempts: 1000,
        timeout_ms: 2000,
        cookie_ttl_seconds: 0,
        ..ChallengeSection::default()
    };
    for _ in 0..2 {
        let results = search_at(
            &test_client(),
            &server.url(""),
            "rust",
            10,
            Duration::from_secs(5),
            &challenge,
        )
        .await
        .expect("a zero-ttl search still returns rows");
        assert_eq!(results.len(), 1);
    }

    assert_eq!(challenge_page_mock.hits(), 2);
    assert_eq!(pass_mock.hits(), 2);
    assert_eq!(replay_mock.hits(), 2);
    assert_eq!(pass_cookie_cache().get(ENGINE), None);

    let after = ledger_stats();
    assert_eq!(solves_delta(&before, &after), (2, 2, 0));
    assert_eq!(after.cookie_misses.saturating_sub(before.cookie_misses), 2);
    assert_eq!(after.cookie_hits, before.cookie_hits);
}

#[tokio::test]
async fn disabled_solver_never_requests_a_css_load_extension() {
    use httpmock::prelude::*;

    let _guard = CHALLENGE_TEST_LOCK.lock().await;
    pass_cookie_cache().clear();

    let server = MockServer::start();
    let page = CHALLENGE_PAGE.replace(
        "\"spent\":false",
        "\"spent\":false,\"extensions\":[\"css-load\"]",
    );
    let search_mock = server.mock(|when, then| {
        when.method(GET)
            .path("/search")
            .query_param("q", "rust")
            .matches(no_cookie_header);
        then.status(200)
            .header("content-type", "text/html")
            .body(page);
    });
    let extension_mock = server.mock(|when, then| {
        when.method(GET).path(CSS_LOAD_CHALLENGE_PATH);
        then.status(200).header("content-type", "text/css");
    });
    let pass_mock = server.mock(|when, then| {
        when.method(GET).path(PASS_CHALLENGE_PATH);
        then.status(302)
            .header("set-cookie", "spchal-auth=unexpected");
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
            engine: "startpage",
            ..
        }
    ));
    assert_eq!(search_mock.hits(), 1);
    assert_eq!(extension_mock.hits(), 0);
    assert_eq!(pass_mock.hits(), 0);
}

#[tokio::test]
async fn css_load_challenge_fetches_the_extension_then_replays() {
    use httpmock::prelude::*;

    let _guard = CHALLENGE_TEST_LOCK.lock().await;
    pass_cookie_cache().clear();

    let server = MockServer::start();
    let page = CHALLENGE_PAGE.replace(
        "\"spent\":false",
        "\"spent\":false,\"extensions\":[\"css-load\"]",
    );
    let challenge_page_mock = server.mock(|when, then| {
        when.method(GET)
            .path("/search")
            .query_param("q", "rust")
            .matches(no_cookie_header);
        then.status(200)
            .header("content-type", "text/html")
            .body(page);
    });
    let extension_mock = server.mock(|when, then| {
        when.method(GET).path(CSS_LOAD_CHALLENGE_PATH);
        then.status(200)
            .header("content-type", "text/css")
            .body("@import url(\"./css-load\");");
    });
    let pass_mock = server.mock(|when, then| {
        when.method(GET)
            .path(PASS_CHALLENGE_PATH)
            .query_param("id", "01a1cc17-0000-7000-8000-000000000000")
            .query_param("nonce", "185")
            .query_param(
                "response",
                "002786c8b408cf14bf6ddc2aeb6b6f1fcdf6b453986796fbec919a88de759708",
            );
        then.status(302)
            .header("set-cookie", "spchal-auth=css-pass; Path=/");
    });
    let replay_mock = server.mock(|when, then| {
        when.method(GET)
            .path("/search")
            .query_param("q", "rust")
            .header("Cookie", "spchal-auth=css-pass");
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
        &enabled_section(),
    )
    .await
    .expect("css-load challenge solves");

    assert_eq!(results.len(), 1);
    assert_eq!(challenge_page_mock.hits(), 1);
    assert_eq!(extension_mock.hits(), 1);
    assert_eq!(pass_mock.hits(), 1);
    assert_eq!(replay_mock.hits(), 1);
    assert_eq!(
        pass_cookie_cache().get(ENGINE).as_deref(),
        Some("spchal-auth=css-pass")
    );
}

#[tokio::test]
async fn metarefresh_challenge_without_a_meta_tag_solves() {
    use httpmock::prelude::*;

    let _guard = CHALLENGE_TEST_LOCK.lock().await;
    pass_cookie_cache().clear();

    let page = r#"<html><head><script id="anubis_challenge" type="application/json">{"rules":{"algorithm":"metarefresh","difficulty":0},"challenge":{"id":"01a1cc17-0000-7000-8000-000000000000","method":"metarefresh","randomData":"header-token","redir":"/search?q=rust","difficulty":0,"spent":false}}</script></head><body><div id="app"></div></body></html>"#;

    let server = MockServer::start();
    let challenge_page_mock = server.mock(|when, then| {
        when.method(GET)
            .path("/search")
            .query_param("q", "rust")
            .matches(no_cookie_header);
        then.status(200)
            .header("content-type", "text/html")
            .body(page);
    });
    let submit_mock = server.mock(|when, then| {
        when.method(GET)
            .path(PASS_CHALLENGE_PATH)
            .query_param("challenge", "header-token")
            .query_param("id", "01a1cc17-0000-7000-8000-000000000000");
        then.status(200)
            .header("set-cookie", "spchal-auth=meta-pass; Path=/");
    });
    let replay_mock = server.mock(|when, then| {
        when.method(GET)
            .path("/search")
            .query_param("q", "rust")
            .header("Cookie", "spchal-auth=meta-pass");
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
        &enabled_section(),
    )
    .await
    .expect("a header-only metarefresh challenge solves");

    assert_eq!(results.len(), 1);
    assert_eq!(challenge_page_mock.hits(), 1);
    assert_eq!(submit_mock.hits(), 1);
    assert_eq!(replay_mock.hits(), 1);
}

#[tokio::test]
async fn sha256_challenge_solves_and_replays_once() {
    use httpmock::prelude::*;

    let _guard = CHALLENGE_TEST_LOCK.lock().await;
    pass_cookie_cache().clear();

    let page = r#"<html><head><script id="anubis_challenge" type="application/json">{"rules":{"algorithm":"sha256","difficulty":8},"challenge":{"id":"01a1cc17-0000-7000-8000-000000000000","method":"sha256","randomData":"abcd","redir":"/search?q=rust","difficulty":8,"spent":false}}</script></head><body><div id="app"></div></body></html>"#;

    let server = MockServer::start();
    let challenge_page_mock = server.mock(|when, then| {
        when.method(GET)
            .path("/search")
            .query_param("q", "rust")
            .matches(no_cookie_header);
        then.status(200)
            .header("content-type", "text/html")
            .body(page);
    });
    let pass_mock = server.mock(|when, then| {
        when.method(GET)
            .path(PASS_CHALLENGE_PATH)
            .query_param("id", "01a1cc17-0000-7000-8000-000000000000")
            .query_param("nonce", "42")
            .query_param(
                "response",
                "00ebbd2e622cc5a6c39c48707ddfd0bd679515e63fd28cbb0426e68448af0714",
            )
            .query_param("redir", "/search?q=rust");
        then.status(302)
            .header("set-cookie", "spchal-auth=sha-pass; Path=/");
    });
    let replay_mock = server.mock(|when, then| {
        when.method(GET)
            .path("/search")
            .query_param("q", "rust")
            .header("Cookie", "spchal-auth=sha-pass");
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
        &enabled_section(),
    )
    .await
    .expect("sha256 challenge solves");

    assert_eq!(results.len(), 1);
    assert_eq!(challenge_page_mock.hits(), 1);
    assert_eq!(pass_mock.hits(), 1);
    assert_eq!(replay_mock.hits(), 1);
    assert_eq!(ledger_stats().last_candidates, 43);
}
