use std::time::Duration;

use eggfetch_core::Client;
use eggsearch::meta::engines::error::EngineError;
use eggsearch::meta::engines::{
    self, gitea_code, gitea_issues, gitea_releases, github_code, github_issues, github_releases,
    gitlab_code, gitlab_issues, gitlab_releases,
};
use httpmock::prelude::*;
use httpmock::Mock;

struct Case {
    engine: &'static str,
    path: &'static str,
    query: &'static str,
    limit_param: &'static str,
    auth_header: &'static str,
    auth_value: &'static str,
    fixture: &'static str,
    empty_fixture: &'static str,
    missing_url_fixture: &'static str,
    expected_len: usize,
    expected_first_title: &'static str,
    expected_first_url: &'static str,
    expected_first_snippet: Option<&'static str>,
    expected_second_title: &'static str,
}

const ENGINES: [&str; 9] = [
    "github_code",
    "github_issues",
    "github_releases",
    "gitlab_code",
    "gitlab_issues",
    "gitlab_releases",
    "gitea_code",
    "gitea_issues",
    "gitea_releases",
];

const CASES: [Case; 9] = [
    Case {
        engine: "github_code",
        path: "/search/code",
        query: "Router",
        limit_param: "per_page",
        auth_header: "Authorization",
        auth_value: "Bearer test-token",
        fixture: r#"{"items":[{"name":"lib.rs","path":"src/lib.rs","html_url":"https://github.com/test/repo/blob/main/src/lib.rs","repository":{"full_name":"test/repo","description":"A web framework"},"score":1.0,"text_matches":[]},{"name":"main.rs","path":"src/main.rs","html_url":"https://github.com/test/repo/blob/main/src/main.rs","repository":{"full_name":"test/repo"},"score":0.8,"text_matches":[]},{"name":"handler.rs","path":"src/handler.rs","html_url":"https://github.com/test/repo/blob/main/src/handler.rs","repository":{"full_name":"test/repo"},"score":0.5,"text_matches":[]}] }"#,
        empty_fixture: r#"{"items":[]}"#,
        missing_url_fixture: r#"{"items":[{"name":"bad.rs","path":"bad.rs","html_url":null,"repository":{"full_name":"test/repo"},"text_matches":[]},{"name":"lib.rs","path":"src/lib.rs","html_url":"https://github.com/test/repo/blob/main/src/lib.rs","repository":{"full_name":"test/repo"},"text_matches":[]}] }"#,
        expected_len: 3,
        expected_first_title: "src/lib.rs - test/repo",
        expected_first_url: "https://github.com/test/repo/blob/main/src/lib.rs",
        expected_first_snippet: Some("A web framework"),
        expected_second_title: "src/main.rs - test/repo",
    },
    Case {
        engine: "github_issues",
        path: "/search/issues",
        query: "parser",
        limit_param: "per_page",
        auth_header: "Authorization",
        auth_value: "Bearer test-token",
        fixture: r#"{"items":[{"number":42,"title":"Fix parser","html_url":"https://github.com/test/repo/issues/42","body":"Parser bug description","state":"open","labels":[{"name":"bug"}],"created_at":"2024-01-15T10:30:00Z","updated_at":"2024-01-20T14:22:00Z","closed_at":null,"pull_request":null},{"number":43,"title":"Add feature","html_url":"https://github.com/test/repo/issues/43","body":"Feature request","state":"open","labels":[],"created_at":"2024-01-16T10:30:00Z","updated_at":"2024-01-21T14:22:00Z","closed_at":null,"pull_request":null},{"number":44,"title":"Handle edge case","html_url":"https://github.com/test/repo/issues/44","body":"Edge case","state":"open","labels":[],"created_at":"2024-01-17T10:30:00Z","updated_at":"2024-01-22T14:22:00Z","closed_at":null,"pull_request":null}]}"#,
        empty_fixture: r#"{"items":[]}"#,
        missing_url_fixture: r#"{"items":[{"number":1,"title":"Missing URL","html_url":null,"state":"open","labels":[],"pull_request":null},{"number":42,"title":"Fix parser","html_url":"https://github.com/test/repo/issues/42","state":"open","labels":[],"pull_request":null}]}"#,
        expected_len: 3,
        expected_first_title: "#42 Fix parser - test/repo",
        expected_first_url: "https://github.com/test/repo/issues/42",
        expected_first_snippet: Some("Parser bug description"),
        expected_second_title: "#43 Add feature - test/repo",
    },
    Case {
        engine: "github_releases",
        path: "/repos/test/repo/releases",
        query: "repo:test/repo",
        limit_param: "per_page",
        auth_header: "Authorization",
        auth_value: "Bearer test-token",
        fixture: r#"{"items":[{"tag_name":"v1.0.0","name":"Release","html_url":"https://github.com/test/repo/releases/tag/v1.0.0","body":"Release notes","draft":false,"prerelease":false,"created_at":"2024-01-15T10:30:00Z","published_at":"2024-01-16T12:00:00Z"},{"tag_name":"v0.9.0","name":"Pre","html_url":"https://github.com/test/repo/releases/tag/v0.9.0","body":"Pre-release notes","draft":false,"prerelease":true,"created_at":"2023-12-01T10:00:00Z","published_at":"2023-12-02T10:00:00Z"},{"tag_name":"v0.8.0","name":null,"html_url":"https://github.com/test/repo/releases/tag/v0.8.0","body":"Older notes","draft":false,"prerelease":false,"created_at":"2023-11-01T10:00:00Z","published_at":"2023-11-02T10:00:00Z"}]}"#,
        empty_fixture: r#"{"items":[]}"#,
        missing_url_fixture: r#"{"items":[{"tag_name":"v0.9.0","name":"Missing URL","html_url":null,"draft":false},{"tag_name":"v1.0.0","name":"Release","html_url":"https://github.com/test/repo/releases/tag/v1.0.0","draft":false}]}"#,
        expected_len: 3,
        expected_first_title: "v1.0.0 Release - test/repo",
        expected_first_url: "https://github.com/test/repo/releases/tag/v1.0.0",
        expected_first_snippet: Some("Release notes"),
        expected_second_title: "v0.9.0 Pre - test/repo",
    },
    Case {
        engine: "gitlab_code",
        path: "/api/v4/search",
        query: "Router",
        limit_param: "per_page",
        auth_header: "PRIVATE-TOKEN",
        auth_value: "test-token",
        fixture: r#"[{"path":"src/lib.rs","filename":"lib.rs","data":"fn router() {}","ref":"main","url":"https://gitlab.com/1/-/blob/main/src/lib.rs","project_id":1},{"path":"src/main.rs","filename":"main.rs","data":"fn main() {}","ref":"main","url":"https://gitlab.com/1/-/blob/main/src/main.rs","project_id":1},{"path":"src/handler.rs","filename":"handler.rs","data":"fn handler() {}","ref":"main","url":"https://gitlab.com/1/-/blob/main/src/handler.rs","project_id":1}]"#,
        empty_fixture: "[]",
        missing_url_fixture: r#"[{"path":"bad.rs","filename":"bad.rs","data":null,"ref":"main","url":null,"project_id":1},{"path":"src/lib.rs","filename":"lib.rs","data":"fn router() {}","ref":"main","url":"https://gitlab.com/1/-/blob/main/src/lib.rs","project_id":1}]"#,
        expected_len: 3,
        expected_first_title: "src/lib.rs - 1",
        expected_first_url: "https://gitlab.com/1/-/blob/main/src/lib.rs",
        expected_first_snippet: Some("fn router() {}"),
        expected_second_title: "src/main.rs - 1",
    },
    Case {
        engine: "gitlab_issues",
        path: "/api/v4/search",
        query: "parser",
        limit_param: "per_page",
        auth_header: "PRIVATE-TOKEN",
        auth_value: "test-token",
        fixture: r#"[{"iid":42,"title":"Fix parser","web_url":"https://gitlab.com/test/repo/-/issues/42","description":"Parser bug description","state":"opened","labels":["bug"],"created_at":"2024-01-15T10:30:00Z","updated_at":"2024-01-20T14:22:00Z","closed_at":null},{"iid":43,"title":"Add feature","web_url":"https://gitlab.com/test/repo/-/issues/43","description":"Feature request","state":"opened","labels":["enhancement"],"created_at":"2024-01-16T10:30:00Z","updated_at":"2024-01-21T14:22:00Z","closed_at":null},{"iid":44,"title":"Handle edge case","web_url":"https://gitlab.com/test/repo/-/issues/44","description":"Edge case","state":"opened","labels":[],"created_at":"2024-01-17T10:30:00Z","updated_at":"2024-01-22T14:22:00Z","closed_at":null}]"#,
        empty_fixture: "[]",
        missing_url_fixture: r#"[{"iid":1,"title":"Missing URL","web_url":null,"state":"opened","labels":[]},{"iid":42,"title":"Fix parser","web_url":"https://gitlab.com/test/repo/-/issues/42","state":"opened","labels":[]}]"#,
        expected_len: 3,
        expected_first_title: "#42 Fix parser",
        expected_first_url: "https://gitlab.com/test/repo/-/issues/42",
        expected_first_snippet: Some("Parser bug description"),
        expected_second_title: "#43 Add feature",
    },
    Case {
        engine: "gitlab_releases",
        path: "/api/v4/projects/12345/releases",
        query: "releases",
        limit_param: "per_page",
        auth_header: "PRIVATE-TOKEN",
        auth_value: "test-token",
        fixture: r#"[{"tag_name":"v1.0.0","name":"Release","web_url":"https://gitlab.com/test/repo/-/releases/v1.0.0","assets":{"links":[{"name":"Source archive","url":"https://gitlab.com/test/repo/archive/v1.0.0.tar.gz"}]},"released_at":"2024-01-16T12:00:00Z"},{"tag_name":"v0.9.0","name":"Pre","web_url":"https://gitlab.com/test/repo/-/releases/v0.9.0","assets":{"links":[{"name":"Source zip","url":"https://gitlab.com/test/repo/archive/v0.9.0.zip"}]},"released_at":"2023-12-02T10:00:00Z"},{"tag_name":"v0.8.0","name":null,"web_url":"https://gitlab.com/test/repo/-/releases/v0.8.0","assets":{"links":[]},"released_at":"2023-11-02T10:00:00Z"}]"#,
        empty_fixture: "[]",
        missing_url_fixture: r#"[{"tag_name":"v0.9.0","name":"Missing URL","web_url":null,"assets":null},{"tag_name":"v1.0.0","name":"Release","web_url":"https://gitlab.com/test/repo/-/releases/v1.0.0","assets":null}]"#,
        expected_len: 3,
        expected_first_title: "v1.0.0 Release",
        expected_first_url: "https://gitlab.com/test/repo/-/releases/v1.0.0",
        expected_first_snippet: Some("Source archive"),
        expected_second_title: "v0.9.0 Pre",
    },
    Case {
        engine: "gitea_code",
        path: "/api/v1/search",
        query: "Router",
        limit_param: "limit",
        auth_header: "Authorization",
        auth_value: "token test-token",
        fixture: r#"{"data":[{"scope":"code","result":[{"name":"lib.rs","path":"src/lib.rs","url":"https://gitea.example.com/test/repo/src/branch/main/src/lib.rs","repository":{"full_name":"test/repo","description":"A web framework"}},{"name":"main.rs","path":"src/main.rs","url":"https://gitea.example.com/test/repo/src/branch/main/src/main.rs","repository":{"full_name":"test/repo"}},{"name":"handler.rs","path":"src/handler.rs","url":"https://gitea.example.com/test/repo/src/branch/main/src/handler.rs","repository":{"full_name":"test/repo"}}]}]}"#,
        empty_fixture: r#"{"data":[]}"#,
        missing_url_fixture: r#"{"data":[{"scope":"code","result":[{"name":"bad.rs","path":"bad.rs","url":null,"repository":{"full_name":"test/repo"}},{"name":"lib.rs","path":"src/lib.rs","url":"https://gitea.example.com/test/repo/src/branch/main/src/lib.rs","repository":{"full_name":"test/repo"}}]}]}"#,
        expected_len: 3,
        expected_first_title: "src/lib.rs - test/repo",
        expected_first_url: "https://gitea.example.com/test/repo/src/branch/main/src/lib.rs",
        expected_first_snippet: Some("A web framework"),
        expected_second_title: "src/main.rs - test/repo",
    },
    Case {
        engine: "gitea_issues",
        path: "/api/v1/search",
        query: "parser",
        limit_param: "limit",
        auth_header: "Authorization",
        auth_value: "token test-token",
        fixture: r#"{"data":[{"scope":"issue","result":[{"title":"Fix parser","url":"https://gitea.example.com/test/repo/issues/42","body":"Parser bug description","state":"open","labels":["bug"],"created_at":"2024-01-15T10:30:00Z","updated_at":"2024-01-20T14:22:00Z","repository":{"full_name":"test/repo"}},{"title":"Add feature","url":"https://gitea.example.com/test/repo/issues/43","body":"Feature request","state":"open","labels":["enhancement"],"repository":{"full_name":"test/repo"}},{"title":"Handle edge case","url":"https://gitea.example.com/test/repo/issues/44","body":"Edge case","state":"open","labels":[],"repository":{"full_name":"test/repo"}}]}]}"#,
        empty_fixture: r#"{"data":[]}"#,
        missing_url_fixture: r#"{"data":[{"scope":"issue","result":[{"title":"Missing URL","url":null,"state":"open","labels":[]},{"title":"Fix parser","url":"https://gitea.example.com/test/repo/issues/42","state":"open","labels":[]}]}]}"#,
        expected_len: 3,
        expected_first_title: "Fix parser",
        expected_first_url: "https://gitea.example.com/test/repo/issues/42",
        expected_first_snippet: Some("Parser bug description"),
        expected_second_title: "Add feature",
    },
    Case {
        engine: "gitea_releases",
        path: "/api/v1/repos/test/repo/releases",
        query: "repo:test/repo",
        limit_param: "limit",
        auth_header: "Authorization",
        auth_value: "token test-token",
        fixture: r#"[{"tag_name":"v1.0.0","name":"Release","html_url":"https://gitea.example.com/test/repo/releases/tag/v1.0.0","body":"Release notes","draft":false,"prerelease":false,"created_at":"2024-01-15T10:30:00Z","published_at":"2024-01-16T12:00:00Z"},{"tag_name":"v0.9.0","name":"Pre","html_url":"https://gitea.example.com/test/repo/releases/tag/v0.9.0","body":"Pre-release notes","draft":false,"prerelease":true,"created_at":"2023-12-01T10:00:00Z","published_at":"2023-12-02T10:00:00Z"},{"tag_name":"v0.8.0","name":null,"html_url":"https://gitea.example.com/test/repo/releases/tag/v0.8.0","body":"Older notes","draft":false,"prerelease":false,"created_at":"2023-11-01T10:00:00Z","published_at":"2023-11-02T10:00:00Z"}]"#,
        empty_fixture: "[]",
        missing_url_fixture: r#"[{"tag_name":"v0.9.0","name":"Missing URL","html_url":null,"draft":false},{"tag_name":"v1.0.0","name":"Release","html_url":"https://gitea.example.com/test/repo/releases/tag/v1.0.0","draft":false}]"#,
        expected_len: 3,
        expected_first_title: "v1.0.0 Release - test/repo",
        expected_first_url: "https://gitea.example.com/test/repo/releases/tag/v1.0.0",
        expected_first_snippet: Some("Release notes"),
        expected_second_title: "v0.9.0 Pre - test/repo",
    },
];

fn case(engine: &str) -> &'static Case {
    CASES
        .iter()
        .find(|case| case.engine == engine)
        .unwrap_or_else(|| panic!("missing case for {engine}"))
}

async fn run(
    engine: &str,
    client: &Client,
    base_url: &str,
    api_key: &str,
    query: &str,
    max_results: usize,
) -> Result<Vec<eggsearch::meta::engines::models::SearchResult>, EngineError> {
    let timeout = Duration::from_secs(5);
    match engine {
        "github_code" => {
            github_code::search(client, api_key, Some(base_url), query, max_results, timeout).await
        }
        "github_issues" => {
            github_issues::search(client, api_key, Some(base_url), query, max_results, timeout)
                .await
        }
        "github_releases" => {
            github_releases::search(client, api_key, Some(base_url), query, max_results, timeout)
                .await
        }
        "gitlab_code" => {
            gitlab_code::search(client, api_key, Some(base_url), query, max_results, timeout).await
        }
        "gitlab_issues" => {
            gitlab_issues::search(client, api_key, Some(base_url), query, max_results, timeout)
                .await
        }
        "gitlab_releases" => {
            gitlab_releases::search_with_project(
                client,
                api_key,
                Some(base_url),
                Some("12345"),
                query,
                max_results,
                timeout,
            )
            .await
        }
        "gitea_code" => {
            gitea_code::search(client, api_key, Some(base_url), query, max_results, timeout).await
        }
        "gitea_issues" => {
            gitea_issues::search(client, api_key, Some(base_url), query, max_results, timeout).await
        }
        "gitea_releases" => {
            gitea_releases::search(client, api_key, Some(base_url), query, max_results, timeout)
                .await
        }
        other => panic!("unknown engine {other}"),
    }
}

fn mock_response<'a>(
    server: &'a MockServer,
    case: &Case,
    status: u16,
    body: &str,
    max_results: Option<&str>,
) -> Mock<'a> {
    let path = case.path.to_string();
    let auth_header = case.auth_header.to_string();
    let auth_value = case.auth_value.to_string();
    let limit_param = case.limit_param.to_string();
    let max_results = max_results.map(str::to_string);
    let body = body.to_string();
    server.mock(move |when, then| {
        if let Some(max_results) = max_results.as_deref() {
            when.method(GET)
                .path(path.as_str())
                .query_param(limit_param.as_str(), max_results)
                .header(auth_header.as_str(), auth_value.as_str());
        } else {
            when.method(GET)
                .path(path.as_str())
                .header(auth_header.as_str(), auth_value.as_str());
        }
        then.status(status)
            .header("content-type", "application/json")
            .body(body);
    })
}

fn client() -> Client {
    engines::build_http_client(None).expect("test client")
}

#[tokio::test]
async fn conversion_cases_parse_expected_rows() {
    for engine in ENGINES {
        let case = case(engine);
        let server = MockServer::start();
        mock_response(&server, case, 200, case.fixture, None);
        let results = run(
            engine,
            &client(),
            &server.url(""),
            "test-token",
            case.query,
            10,
        )
        .await
        .unwrap_or_else(|error| panic!("{engine}: {error}"));
        assert_eq!(results.len(), case.expected_len, "{engine} row count");
        assert_eq!(
            results[0].title, case.expected_first_title,
            "{engine} first title"
        );
        assert_eq!(
            results[0].url, case.expected_first_url,
            "{engine} first URL"
        );
        assert_eq!(results[0].source_engine, engine, "{engine} source engine");
        if let Some(snippet) = case.expected_first_snippet {
            assert_eq!(
                results[0].snippet.as_deref(),
                Some(snippet),
                "{engine} first snippet"
            );
        }
    }
}

#[tokio::test]
async fn bad_statuses_preserve_variant_status_and_engine() {
    for engine in ENGINES {
        let case = case(engine);
        for status in [401, 403, 500] {
            let server = MockServer::start();
            mock_response(&server, case, status, "status response", None);
            let error = run(
                engine,
                &client(),
                &server.url(""),
                "test-token",
                case.query,
                10,
            )
            .await
            .expect_err("non-success status must fail");
            match error {
                EngineError::BadStatus {
                    engine: actual_engine,
                    status: actual_status,
                } => {
                    assert_eq!(actual_engine, engine, "{engine} error engine");
                    assert_eq!(actual_status, status, "{engine} error status");
                }
                other => panic!("{engine} status {status} returned {other:?}"),
            }
        }
    }
}

#[tokio::test]
async fn malformed_json_reports_parse_failed() {
    for engine in ENGINES {
        let case = case(engine);
        let server = MockServer::start();
        mock_response(&server, case, 200, "this is not json", None);
        let error = run(
            engine,
            &client(),
            &server.url(""),
            "test-token",
            case.query,
            10,
        )
        .await
        .expect_err("malformed JSON must fail");
        match error {
            EngineError::ParseFailed {
                engine: actual_engine,
                reason,
            } => {
                assert_eq!(actual_engine, engine, "{engine} error engine");
                assert!(reason.contains("invalid JSON"), "{engine} reason: {reason}");
            }
            other => panic!("{engine} malformed JSON returned {other:?}"),
        }
    }
}

#[tokio::test]
async fn zero_results_returns_empty_without_request() {
    for engine in ENGINES {
        let case = case(engine);
        let server = MockServer::start();
        let mock = mock_response(&server, case, 200, case.fixture, None);
        let results = run(
            engine,
            &client(),
            &server.url(""),
            "test-token",
            case.query,
            0,
        )
        .await
        .unwrap_or_else(|error| panic!("{engine}: {error}"));
        assert!(results.is_empty(), "{engine} zero results");
        mock.assert_calls(0);
    }
}

#[tokio::test]
async fn max_results_limits_three_rows_to_two() {
    for engine in ENGINES {
        let case = case(engine);
        let server = MockServer::start();
        mock_response(&server, case, 200, case.fixture, Some("2"));
        let results = run(
            engine,
            &client(),
            &server.url(""),
            "test-token",
            case.query,
            2,
        )
        .await
        .unwrap_or_else(|error| panic!("{engine}: {error}"));
        assert_eq!(results.len(), 2, "{engine} max_results");
        assert_eq!(
            results[0].title, case.expected_first_title,
            "{engine} first result"
        );
        assert_eq!(
            results[1].title, case.expected_second_title,
            "{engine} second result"
        );
    }
}

#[tokio::test]
async fn missing_url_rows_are_skipped_without_panicking() {
    for engine in ENGINES {
        let case = case(engine);
        let server = MockServer::start();
        mock_response(&server, case, 200, case.missing_url_fixture, None);
        let results = run(
            engine,
            &client(),
            &server.url(""),
            "test-token",
            case.query,
            10,
        )
        .await
        .unwrap_or_else(|error| panic!("{engine}: {error}"));
        assert_eq!(results.len(), 1, "{engine} missing URL row");
        assert_eq!(
            results[0].title, case.expected_first_title,
            "{engine} surviving title"
        );
        assert_eq!(
            results[0].url, case.expected_first_url,
            "{engine} surviving URL"
        );
    }
}

#[tokio::test]
async fn empty_payloads_return_empty_rows() {
    for engine in ENGINES {
        let case = case(engine);
        let server = MockServer::start();
        mock_response(&server, case, 200, case.empty_fixture, None);
        let results = run(
            engine,
            &client(),
            &server.url(""),
            "test-token",
            case.query,
            10,
        )
        .await
        .unwrap_or_else(|error| panic!("{engine}: {error}"));
        assert!(results.is_empty(), "{engine} empty payload");
    }
}
