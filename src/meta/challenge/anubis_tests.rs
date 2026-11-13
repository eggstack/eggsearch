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

fn with_prefix(page: &str, prefix_json: &str) -> String {
    page.replacen(
            "<script id=\"anubis_challenge\"",
            &format!(
                "<script id=\"anubis_base_prefix\" type=\"application/json\">{prefix_json}</script><script id=\"anubis_challenge\""
            ),
            1,
        )
}

fn with_spent(page: &str, spent_json: &str) -> String {
    page.replace("\"spent\":false", &format!("\"spent\":{spent_json}"))
}

fn with_redir(page: &str, redir: &str) -> String {
    page.replace(
        "\"redir\":\"/search?q=rust\"",
        &format!("\"redir\":{}", serde_json::json!(redir)),
    )
}

fn metarefresh_page(token: &str, difficulty: u8, seconds: &str, target: &str) -> String {
    let payload = serde_json::json!({
        "rules": {"algorithm": "metarefresh", "difficulty": difficulty},
        "challenge": {
            "id": "01a1cc17-0000-7000-8000-000000000000",
            "method": "metarefresh",
            "challenge": token,
            "redir": "/search?q=rust",
            "difficulty": difficulty,
            "spent": false
        }
    });
    format!(
        r#"<html><head><meta http-equiv="refresh" content="{seconds}; url={target}"><script id="anubis_challenge" type="application/json">{payload}</script></head><body><div id="app"></div></body></html>"#
    )
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
        AnubisChallenge::parse(&fast_page("abc123def456", 2, "quantum")).err(),
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
        &fast_page("abc123def456", 2, "quantum"),
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
        challenge.resolved_redir(
            "https://www.startpage.com",
            "https://www.startpage.com/search?q=rust"
        ),
        "https://www.startpage.com/search?q=rust"
    );
}

#[test]
fn published_redir_wins_over_the_request_url() {
    let challenge =
        AnubisChallenge::parse(&fast_page("abc123def456", 2, "fast")).expect("vector parses");
    assert_eq!(
        challenge.resolved_redir(
            "https://www.startpage.com",
            "https://www.startpage.com/search?q=rust"
        ),
        "/search?q=rust"
    );
}

#[test]
fn base_prefix_defaults_to_empty_and_normalises_one_trailing_slash() {
    let page = fast_page("abc123def456", 2, "fast");
    let challenge = AnubisChallenge::parse(&page).expect("parses");
    assert!(challenge.base_prefix.is_empty());
    assert_eq!(
        pass_challenge_url("https://www.startpage.com", &challenge.base_prefix),
        "https://www.startpage.com/.within.website/x/cmd/anubis/api/pass-challenge"
    );
    for (published, expected) in [
        ("\"\"", ""),
        ("\"/anubis\"", "/anubis"),
        ("\"/anubis/\"", "/anubis"),
    ] {
        let prefixed = with_prefix(&page, published);
        let challenge = AnubisChallenge::parse(&prefixed).expect("prefixed parses");
        assert_eq!(challenge.base_prefix, expected, "published {published}");
    }
    assert_eq!(
        pass_challenge_url("https://www.startpage.com", "/anubis"),
        "https://www.startpage.com/anubis/.within.website/x/cmd/anubis/api/pass-challenge"
    );
}

#[test]
fn base_prefix_refuses_unsafe_values() {
    let page = fast_page("abc123def456", 2, "fast");
    for (published, expected) in [
        ("\"//host\"", BOUND_BASE_PREFIX),
        ("\"/a:b\"", BOUND_BASE_PREFIX),
        ("\"/a?b\"", BOUND_BASE_PREFIX),
        ("\"/a#b\"", BOUND_BASE_PREFIX),
        ("\"/a\\\\b\"", BOUND_BASE_PREFIX),
        ("\"/a b\"", BOUND_BASE_PREFIX),
        ("\"/a\\nb\"", BOUND_BASE_PREFIX),
        ("\"/a/../b\"", BOUND_BASE_PREFIX),
        ("\"/a//b\"", BOUND_BASE_PREFIX),
        ("123", BOUND_MALFORMED),
        ("null", BOUND_MALFORMED),
    ] {
        assert_eq!(
            AnubisChallenge::parse(&with_prefix(&page, published)).err(),
            Some(expected),
            "published {published}"
        );
    }
    let too_long = format!("\"/{}\"", "a".repeat(257));
    assert_eq!(
        AnubisChallenge::parse(&with_prefix(&page, &too_long)).err(),
        Some(BOUND_BASE_PREFIX)
    );
}

#[test]
fn base_prefix_is_redacted_in_debug() {
    let page = with_prefix(
        &fast_page("abc123def456", 2, "fast"),
        "\"/sentinel-prefix\"",
    );
    let challenge = AnubisChallenge::parse(&page).expect("parses");
    let rendered = format!("{challenge:?}");
    assert!(!rendered.contains("sentinel-prefix"), "got: {rendered}");
    assert!(rendered.contains("base_prefix"));
}

#[test]
fn slow_is_an_alias_of_fast() {
    let fast = AnubisChallenge::parse(&fast_page("abc123def456", 2, "fast")).expect("fast");
    let slow = AnubisChallenge::parse(&fast_page("abc123def456", 2, "slow")).expect("slow");
    assert_eq!(slow.method(), AnubisMethod::Fast);
    let mut fast_budget = budget(1000, 5000, 5);
    let mut slow_budget = budget(1000, 5000, 5);
    assert_eq!(fast.solve(&mut fast_budget), slow.solve(&mut slow_budget));
    assert_eq!(fast_budget.attempts_used(), slow_budget.attempts_used());
}

#[test]
fn spent_challenge_is_refused_before_any_work() {
    let page = fast_page("abc123def456", 2, "fast");
    assert_eq!(
        AnubisChallenge::parse(&with_spent(&page, "true")).err(),
        Some(BOUND_SPENT)
    );
    assert!(AnubisChallenge::parse(&with_spent(&page, "false")).is_ok());
    assert_eq!(
        AnubisChallenge::parse(&with_spent(&page, "\"false\"")).err(),
        Some(BOUND_MALFORMED)
    );
    let preact_spent = preact_page("abc", 1, "/search")
        .replace("{\"challenge\":", "{\"spent\":true,\"challenge\":");
    assert_eq!(
        AnubisChallenge::parse(&preact_spent).err(),
        Some(BOUND_SPENT)
    );
}

#[test]
fn projection_arithmetic_is_exact() {
    assert_eq!(expected_candidates(0), 1);
    assert_eq!(expected_candidates(6), 16_777_216);
    assert_eq!(expected_candidates(7), 268_435_456);
    assert_eq!(compact_count(999), "999");
    assert_eq!(compact_count(1_000), "1K");
    assert_eq!(compact_count(999_950), "1M");
    assert_eq!(compact_count(16_777_216), "17M");
    assert_eq!(compact_count(268_435_456), "268M");
}

#[test]
fn projection_refuses_before_hashing() {
    let challenge = AnubisChallenge::parse(&fast_page("abc123def456", 7, "fast")).expect("parses");
    assert_eq!(
            challenge.check_projection(&budget(2_000_000, 5000, 10)),
            Err("anubis difficulty 7 projects 268M candidates, beyond the configured attempt ceiling 2000000".to_string())
        );
    let wall = AnubisChallenge::parse(&fast_page("abc123def456", 5, "fast")).expect("parses");
    assert_eq!(
            wall.check_projection(&budget(2_000_000, 50, 5)),
            Err("anubis difficulty 5 projects 1M candidates (~136 ms), beyond the configured wall-clock budget 50 ms".to_string())
        );
    let admitted = AnubisChallenge::parse(&fast_page("abc123def456", 5, "fast")).expect("parses");
    assert!(admitted
        .check_projection(&budget(1_048_576, 5000, 5))
        .is_ok());
}

#[tokio::test]
async fn projection_breach_blocks_with_zero_requests() {
    use httpmock::prelude::*;

    let server = MockServer::start();
    let any = server.mock(|when, then| {
        when.method(GET);
        then.status(200).header("set-cookie", "spchal-auth=never");
    });

    let mut budget = budget(2_000_000, 5000, 10);
    let err = solve_and_verify(
        &test_client(),
        "startpage",
        &server.url(""),
        &fast_page("abc123def456", 7, "fast"),
        "/search?q=rust",
        "",
        Duration::from_secs(5),
        &mut budget,
    )
    .await
    .expect_err("projection breach is blocked");

    match &err {
        EngineError::Blocked { engine, reason } => {
            assert_eq!(*engine, "startpage");
            assert_eq!(
                    reason,
                    "anubis difficulty 7 projects 268M candidates, beyond the configured attempt ceiling 2000000"
                );
        }
        other => panic!("unexpected error: {other:?}"),
    }
    assert_eq!(budget.attempts_used(), 0);
    assert_eq!(any.hits(), 0);
}

#[test]
fn redir_is_validated_before_it_reaches_the_query() {
    let base = "https://www.startpage.com";
    let request = "https://www.startpage.com/search?q=rust";
    let page = fast_page("abc123def456", 2, "fast");
    for (published, expected) in [
        ("javascript:alert(1)", request),
        ("data:text/html,x", request),
        ("file:///etc/passwd", request),
        ("//evil.example/x", request),
        ("/a/../b", request),
        ("/a b", request),
        ("https://evil.example/search", request),
        ("http://www.startpage.com/search", request),
        (
            "https://www.startpage.com/search?q=rust",
            "https://www.startpage.com/search?q=rust",
        ),
        ("/search?q=rust", "/search?q=rust"),
    ] {
        let challenge = AnubisChallenge::parse(&with_redir(&page, published)).expect("parses");
        assert_eq!(
            challenge.resolved_redir(base, request),
            expected,
            "published {published}"
        );
    }
}

#[test]
fn preact_cross_origin_redir_falls_back_to_the_request_url() {
    let challenge = AnubisChallenge::parse(&preact_page("abc", 1, "https://evil.example/search"))
        .expect("parses");
    assert_eq!(
        challenge.resolved_redir(
            "https://www.startpage.com",
            "https://www.startpage.com/search?q=rust"
        ),
        "https://www.startpage.com/search?q=rust"
    );
}

#[test]
fn metarefresh_dispatches_and_echoes_only_published_values() {
    let page = metarefresh_page("published-token-123", 2, "0", "/metarefresh-check");
    let challenge = AnubisChallenge::parse(&page).expect("parses");
    assert_eq!(challenge.method(), AnubisMethod::Metarefresh);
    assert_eq!(challenge.difficulty(), 2);
    assert_eq!(challenge.wait, Some(Duration::ZERO));

    let mut budget = budget(1000, 5000, 5);
    let solution = challenge.solve(&mut budget).expect("solves");
    assert_eq!(solution.response, "published-token-123");
    assert_eq!(solution.nonce, 0);
    assert_eq!(budget.attempts_used(), 0);

    let target = challenge
        .resolved_target("https://www.startpage.com")
        .expect("published target resolves");
    assert_eq!(
            challenge.metarefresh_url(&target),
            "https://www.startpage.com/metarefresh-check?id=01a1cc17-0000-7000-8000-000000000000&result=published-token-123"
        );
}

#[test]
fn metarefresh_malformed_pages_fail_closed() {
    let base = metarefresh_page("token", 2, "0", "/metarefresh-check");
    let no_meta = base.replace(
        "<meta http-equiv=\"refresh\" content=\"0; url=/metarefresh-check\">",
        "",
    );
    let mismatch = base.replace(
        "\"method\":\"metarefresh\"",
        "\"method\":\"metarefresh\",\"wait\":5",
    );
    let non_numeric_wait = base.replace(
        "\"method\":\"metarefresh\"",
        "\"method\":\"metarefresh\",\"wait\":\"5\"",
    );
    let empty_token = base.replace("\"challenge\":\"token\"", "\"challenge\":\"\"");
    for page in [
        no_meta,
        metarefresh_page("token", 2, "abc", "/metarefresh-check"),
        metarefresh_page("token", 2, "-1", "/metarefresh-check"),
        metarefresh_page("token", 2, "NaN", "/metarefresh-check"),
        mismatch,
        non_numeric_wait,
        empty_token,
    ] {
        assert_eq!(
            AnubisChallenge::parse(&page).err(),
            Some(BOUND_MALFORMED),
            "expected malformed for {page}"
        );
    }

    let challenge =
        AnubisChallenge::parse(&metarefresh_page("token", 2, "0", "/ok")).expect("parses");
    assert_eq!(
        challenge.resolved_target("https://www.startpage.com"),
        Ok("https://www.startpage.com/ok".to_string())
    );

    let cross_origin = AnubisChallenge::parse(&metarefresh_page(
        "token",
        2,
        "0",
        "https://evil.example/steal",
    ))
    .expect("parses");
    assert_eq!(
        cross_origin.resolved_target("https://www.startpage.com"),
        Err(BOUND_MALFORMED)
    );
}

#[test]
fn metarefresh_wait_never_exceeds_the_budget() {
    let deadline = Instant::now() + Duration::from_secs(1);
    assert_eq!(
        metarefresh_wait(Duration::from_secs(5), deadline),
        Err(BOUND_WALL_CLOCK)
    );
    let deadline = Instant::now() + Duration::from_secs(5);
    assert_eq!(
        metarefresh_wait(Duration::from_secs(1), deadline),
        Ok(Duration::from_secs(1))
    );
}

#[tokio::test]
async fn metarefresh_above_ceiling_is_blocked_without_a_request() {
    use httpmock::prelude::*;

    let server = MockServer::start();
    let any = server.mock(|when, then| {
        when.method(GET);
        then.status(200).header("set-cookie", "spchal-auth=never");
    });

    let mut budget = budget(2_000_000, 5000, 5);
    let err = solve_and_verify(
        &test_client(),
        "startpage",
        &server.url(""),
        &metarefresh_page("token", 6, "0", "/metarefresh-check"),
        "/search?q=rust",
        "",
        Duration::from_secs(5),
        &mut budget,
    )
    .await
    .expect_err("ceiling breach is blocked");

    match &err {
        EngineError::Blocked { reason, .. } => {
            assert_eq!(reason, "anubis difficulty 6 exceeds configured ceiling 5");
        }
        other => panic!("unexpected error: {other:?}"),
    }
    assert_eq!(budget.attempts_used(), 0);
    assert_eq!(any.hits(), 0);
}

#[tokio::test]
async fn metarefresh_submits_the_published_echo_once() {
    use httpmock::prelude::*;

    let server = MockServer::start();
    let page = metarefresh_page("published-token-123", 1, "0", "/metarefresh-check");
    let submit_mock = server.mock(|when, then| {
        when.method(GET)
            .path("/metarefresh-check")
            .query_param("result", "published-token-123")
            .query_param("id", "01a1cc17-0000-7000-8000-000000000000");
        then.status(200)
            .header("set-cookie", "spchal-auth=pass-123; Path=/");
    });

    let mut budget = budget(1000, 5000, 5);
    let outcome = solve_and_verify(
        &test_client(),
        "startpage",
        &server.url(""),
        &page,
        "/search?q=rust",
        "sp_pow=pow-1",
        Duration::from_secs(5),
        &mut budget,
    )
    .await
    .expect("submission is not an engine error");

    assert_eq!(
        outcome,
        SolveOutcome::Solved {
            cookie: "pass-123".to_string()
        }
    );
    assert_eq!(submit_mock.hits(), 1);
    assert_eq!(budget.attempts_used(), 0);
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
    let challenge = AnubisChallenge::parse(&fast_page("abc123def456", 6, "fast")).expect("parses");
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
        http::HeaderValue::from_static(
            "sp_pow=pow-1; Expires=Wed, 11 Nov 2026 00:00:00 GMT; Max-Age=300; HttpOnly; Path=/",
        ),
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
        0x00u8, 0x0f, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00,
    ];
    assert!(has_leading_zero_hex(&digest, 0));
    assert!(has_leading_zero_hex(&digest, 1));
    assert!(has_leading_zero_hex(&digest, 2));
    assert!(has_leading_zero_hex(&digest, 3));
    assert!(!has_leading_zero_hex(&digest, 4));
    assert!(!has_leading_zero_hex(&digest, 65));
}
