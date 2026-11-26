use super::super::*;
use super::*;

#[tokio::test]
async fn a_solve_that_outlives_the_replay_budget_still_caches_the_cookie() {
    use httpmock::prelude::*;

    let _guard = CHALLENGE_TEST_LOCK.lock().await;
    pass_cookie_cache().remove(ENGINE);

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
            .delay(Duration::from_secs(2))
            .body(RESULT_PAGE);
    });

    let first = search_at(
        &test_client(),
        &server.url(""),
        "rust",
        10,
        Duration::from_millis(700),
        &enabled_section(),
    )
    .await;
    assert!(
        first.is_err(),
        "the replay must miss the short request budget: {first:?}"
    );
    assert_eq!(challenge_page_mock.hits(), 1);
    assert_eq!(pass_mock.hits(), 1);
    assert_eq!(
        pass_cookie_cache().get(ENGINE).as_deref(),
        Some("spchal-auth=pass-cookie-123"),
        "the pass cookie must be cached before the replay await"
    );

    let second = search_at(
        &test_client(),
        &server.url(""),
        "rust",
        10,
        Duration::from_secs(5),
        &enabled_section(),
    )
    .await
    .expect("the cached cookie replays without a second solve");
    assert_eq!(second.len(), 1);
    assert_eq!(second[0].url, "https://example.com");
    assert_eq!(
        challenge_page_mock.hits(),
        1,
        "the cached replay must not fetch the wall"
    );
    assert_eq!(
        pass_mock.hits(),
        1,
        "the cached replay must not solve twice"
    );
    assert_eq!(replay_mock.hits(), 2);
}

#[tokio::test]
async fn a_warm_cookie_keeps_the_first_attempt_inside_budget() {
    use httpmock::prelude::*;

    let _guard = CHALLENGE_TEST_LOCK.lock().await;
    pass_cookie_cache().remove(ENGINE);
    pass_cookie_cache().put(
        ENGINE,
        "spchal-auth=pass-cookie-123",
        Duration::from_secs(60),
    );

    let server = MockServer::start();
    let replay_mock = server.mock(|when, then| {
        when.method(GET)
            .path("/search")
            .query_param("q", "rust")
            .header("Cookie", "spchal-auth=pass-cookie-123");
        then.status(200)
            .header("content-type", "text/html")
            .body(RESULT_PAGE);
    });
    let wall_mock = server.mock(|when, then| {
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
        then.status(500);
    });

    let results = search_at(
        &test_client(),
        &server.url(""),
        "rust",
        10,
        Duration::from_millis(600),
        &enabled_section(),
    )
    .await
    .expect("a warm cookie fits the short budget");

    assert_eq!(results.len(), 1);
    assert_eq!(replay_mock.hits(), 1);
    assert_eq!(wall_mock.hits(), 0);
    assert_eq!(pass_mock.hits(), 0);
}

#[tokio::test]
async fn a_solve_never_runs_past_the_request_budget() {
    use httpmock::prelude::*;

    let _guard = CHALLENGE_TEST_LOCK.lock().await;
    pass_cookie_cache().remove(ENGINE);

    let server = MockServer::start();
    let wall_mock = server.mock(|when, then| {
        when.method(GET)
            .path("/search")
            .query_param("q", "rust")
            .matches(no_cookie_header);
        then.status(200)
            .header("content-type", "text/html")
            .header("set-cookie", "sp_pow=pow-1; Max-Age=300; HttpOnly; Path=/")
            .body(CHALLENGE_PAGE);
    });
    let pass_mock = server.mock(|when, then| {
        when.method(GET).path(PASS_CHALLENGE_PATH);
        then.status(302)
            .header("location", "/search?q=rust")
            .header("set-cookie", "spchal-auth=too-late; Path=/; HttpOnly")
            .delay(Duration::from_secs(2));
    });

    let started = std::time::Instant::now();
    let error = search_at(
        &test_client(),
        &server.url(""),
        "rust",
        10,
        Duration::from_millis(400),
        &enabled_section(),
    )
    .await
    .expect_err("a solve that outlives the budget must not complete");
    assert!(
        matches!(
            error,
            EngineError::Timeout { .. }
                | EngineError::Http { .. }
                | EngineError::NetworkError { .. }
        ),
        "unexpected error: {error:?}"
    );
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "the solve must stop at the request budget, took {:?}",
        started.elapsed()
    );
    assert_eq!(wall_mock.hits(), 1);
    assert_eq!(pass_mock.hits(), 1);
    assert!(
        pass_cookie_cache().get(ENGINE).is_none(),
        "a solve that never completed must not cache a cookie"
    );
}
