use std::time::Duration;

use eggfetch_core::Client;
use scraper::Html;

use super::error::EngineError;
use super::models::SearchResult;
use super::{PageMarker, ScrapeShape};
use crate::core::config::ChallengeSection;
use crate::meta::challenge::{
    anubis, pass_cookie_cache, SolveBudget, SolveOutcome, BOUND_COOKIE_REJECTED,
};

const ENGINE: &str = "startpage";
const STARTPAGE_BASE_URL: &str = "https://www.startpage.com";
const SEARCH_PATH: &str = "/search";
const MAX_BODY_BYTES: usize = 2 * 1024 * 1024;

const SHAPE: ScrapeShape = ScrapeShape {
    result_container: "div.result",
    empty_markers: &[
        PageMarker::Selector("div.no-results"),
        PageMarker::Text("No search results were found"),
        PageMarker::Text("No results found for"),
    ],
    blocked_markers: &[
        PageMarker::Text("Access Denied - Startpage"),
        PageMarker::Selector("#anubis_challenge"),
        PageMarker::Selector("#anubis_version"),
    ],
};

struct SearchResponse {
    body: String,
    cookies: String,
}

pub async fn search(
    client: &Client,
    query: &str,
    max_results: usize,
    timeout: Duration,
    challenge: &ChallengeSection,
) -> Result<Vec<SearchResult>, EngineError> {
    search_at(
        client,
        STARTPAGE_BASE_URL,
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
    let request_url = format!("{search_url}?q={}", urlencoding::encode(query));
    let cached_cookie = if challenge.enabled {
        pass_cookie_cache().get(ENGINE)
    } else {
        None
    };
    let first = send_search(
        client,
        &search_url,
        query,
        cached_cookie.as_deref(),
        timeout,
    )
    .await?;

    if !challenge.enabled || !is_challenge_page(&first.body)? {
        return parse(&first.body, max_results);
    }

    let cookie = solve_cookie(
        client,
        base_url,
        &first.body,
        &request_url,
        &first.cookies,
        timeout,
        challenge,
    )
    .await?;
    pass_cookie_cache().put(
        ENGINE,
        &cookie,
        Duration::from_secs(challenge.cookie_ttl_seconds),
    );

    let replayed = send_search(client, &search_url, query, Some(&cookie), timeout).await?;
    if is_challenge_page(&replayed.body)? {
        return Err(blocked(BOUND_COOKIE_REJECTED));
    }
    parse(&replayed.body, max_results)
}

async fn send_search(
    client: &Client,
    url: &str,
    query: &str,
    pass_cookie: Option<&str>,
    timeout: Duration,
) -> Result<SearchResponse, EngineError> {
    let mut request = client
        .get(url)
        .map_err(|e| EngineError::Http {
            engine: ENGINE,
            source: e,
        })?
        .query("q", query)
        .timeout(super::engine_timeout(timeout));
    if let Some(cookie) = pass_cookie {
        request = request.header("Cookie", &format!("{}={cookie}", anubis::PASS_COOKIE_NAME));
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
    let cookies = anubis::challenge_cookies(resp.headers());
    let bytes = super::read_bounded_body(resp, ENGINE, MAX_BODY_BYTES).await?;
    Ok(SearchResponse {
        body: String::from_utf8_lossy(&bytes).into_owned(),
        cookies,
    })
}

async fn solve_cookie(
    client: &Client,
    base_url: &str,
    challenge_page: &str,
    request_url: &str,
    challenge_cookies: &str,
    timeout: Duration,
    challenge: &ChallengeSection,
) -> Result<String, EngineError> {
    let solve_timeout = Duration::from_millis(challenge.timeout_ms).min(timeout);
    let mut budget = SolveBudget::new(
        challenge.max_attempts,
        solve_timeout,
        challenge.max_difficulty,
    );
    let outcome = anubis::solve_and_verify(
        client,
        ENGINE,
        base_url,
        challenge_page,
        request_url,
        challenge_cookies,
        solve_timeout,
        &mut budget,
    )
    .await?;
    match outcome {
        SolveOutcome::Solved { cookie } => Ok(cookie),
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
    let payload = super::startpage_payload::parse_payload(html, max_results);
    let from_payload = payload.is_some();
    let results = match payload {
        Some(results) => results,
        None => parse_dom(&document, max_results)?,
    };
    match super::finalize_scrape(ENGINE, &document, &SHAPE, results) {
        Err(EngineError::ParseFailed { engine, reason }) if from_payload => {
            Err(EngineError::ParseFailed {
                engine,
                reason: format!("payload present but produced no parseable results ({reason})"),
            })
        }
        outcome => outcome,
    }
}

fn parse_dom(document: &Html, max_results: usize) -> Result<Vec<SearchResult>, EngineError> {
    let result_sel = super::sel(ENGINE, SHAPE.result_container)?;
    let link_sel = super::sel(ENGINE, "a.result-title")?;
    let title_sel = super::sel(ENGINE, "h2.wgl-title")?;
    let snippet_sel = super::sel(ENGINE, "p.description")?;

    let mut results = Vec::new();

    for element in document.select(&result_sel) {
        if results.len() >= max_results {
            break;
        }
        let Some(link_el) = element.select(&link_sel).next() else {
            continue;
        };

        let url = link_el.value().attr("href").unwrap_or("").to_string();
        if !super::is_http_url(&url) {
            continue;
        }

        let title = element
            .select(&title_sel)
            .next()
            .map(|el| el.text().collect::<String>().trim().to_string())
            .unwrap_or_default();
        if title.is_empty() {
            continue;
        }

        let snippet = element
            .select(&snippet_sel)
            .next()
            .map(|el| el.text().collect::<String>().trim().to_string())
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

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

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

    const DIFFICULTY_SIX_PAGE: &str = r#"<html><head><script id="anubis_version" type="application/json">"v1.26.4"</script><script id="anubis_challenge" type="application/json">{"rules":{"algorithm":"fast","difficulty":6},"challenge":{"issuedAt":"2026-11-11T00:00:00Z","metadata":{"User-Agent":"eggsearch"},"id":"01a1cc17-0000-7000-8000-000000000000","method":"fast","randomData":"570d0182c0ffee","redir":"/search?q=rust","policyRuleHash":"00","difficulty":6,"spent":false}}</script></head><body><div id="app"></div></body></html>"#;

    const UNKNOWN_METHOD_PAGE: &str = r#"<html><head><script id="anubis_challenge" type="application/json">{"rules":{"algorithm":"quantum","difficulty":2},"challenge":{"id":"01a1cc17","method":"quantum","randomData":"abc123def456","redir":"/search?q=rust"}}</script></head><body></body></html>"#;

    const DIFFICULTY_SEVEN_PAGE: &str = r#"<html><head><script id="anubis_version" type="application/json">"v1.26.4"</script><script id="anubis_challenge" type="application/json">{"rules":{"algorithm":"fast","difficulty":7},"challenge":{"id":"01a1cc17","method":"fast","randomData":"570d0182c0ffee","redir":"/search?q=rust","difficulty":7,"spent":false}}</script></head><body><div id="app"></div></body></html>"#;

    const METAREFRESH_CHALLENGE_PAGE: &str = r#"<html><head><meta http-equiv="refresh" content="0; url=/metarefresh-check"><script id="anubis_version" type="application/json">"v1.26.4"</script><script id="anubis_challenge" type="application/json">{"rules":{"algorithm":"metarefresh","difficulty":1},"challenge":{"id":"01a1cc17-0000-7000-8000-000000000000","method":"metarefresh","challenge":"published-token-123","redir":"/search?q=rust","difficulty":1,"spent":false}}</script></head><body><div id="app"></div></body></html>"#;

    const RESULT_PAGE: &str = r#"<html><body><div class="result css-o7i03b"><a class="result-title result-link" href="https://example.com"><h2 class="wgl-title">Example Site</h2></a><p class="description">An example website.</p></div></body></html>"#;

    const UNPARSEABLE_PAGE: &str = "<html><body><p>Loading</p></body></html>";

    const PASS_CHALLENGE_PATH: &str = "/.within.website/x/cmd/anubis/api/pass-challenge";

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
        cookie_header(request).as_deref()
            == Some("spchal-cookie-verification=verif-1; sp_pow=pow-1")
    }

    fn test_client() -> Client {
        crate::meta::engines::build_http_client(None).expect("test client")
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
            Some("pass-cookie-123")
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
    }

    #[tokio::test]
    async fn projection_breach_is_blocked_without_a_submission() {
        use httpmock::prelude::*;

        let _guard = CHALLENGE_TEST_LOCK.lock().await;
        pass_cookie_cache().clear();

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
                .query_param("result", "published-token-123");
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
        pass_cookie_cache().put(ENGINE, "cached-cookie-456", Duration::from_secs(60));

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
    }

    #[tokio::test]
    async fn above_ceiling_is_blocked_naming_both_values() {
        use httpmock::prelude::*;

        let _guard = CHALLENGE_TEST_LOCK.lock().await;
        pass_cookie_cache().clear();

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
    }

    #[tokio::test]
    async fn unknown_method_is_blocked_without_solving() {
        use httpmock::prelude::*;

        let _guard = CHALLENGE_TEST_LOCK.lock().await;
        pass_cookie_cache().clear();

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
    }

    #[tokio::test]
    async fn rejected_pass_cookie_blocks_without_leaking_it() {
        use httpmock::prelude::*;

        let _guard = CHALLENGE_TEST_LOCK.lock().await;
        pass_cookie_cache().clear();

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
            cookie_ttl_seconds: 0,
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
        assert_eq!(pass_cookie_cache().get(ENGINE), None);
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
}
