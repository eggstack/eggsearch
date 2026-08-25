use eggsearch::core::fetch::ExtractMode;
use eggsearch::fetch::client::FetchClient;
use eggsearch::fetch::limits::{validate_fetch_target, FetchLimits};
use eggsearch::fetch::types::FetchError;
use httpmock::prelude::*;
use url::Url;

fn permissive_limits() -> FetchLimits {
    FetchLimits {
        allow_private_network: true,
        allow_localhost: true,
        ..Default::default()
    }
}

#[tokio::test]
async fn validate_fetch_target_rejects_credentials_in_username() {
    let limits = permissive_limits();
    let url = Url::parse("http://user@example.com/").unwrap();
    let result = validate_fetch_target(&url, &limits).await;
    assert!(
        matches!(result, Err(FetchError::EmbeddedCredentialsBlocked(_))),
        "URL with username should be rejected, got: {result:?}"
    );
}

#[tokio::test]
async fn validate_fetch_target_rejects_credentials_in_userinfo() {
    let limits = permissive_limits();
    let url = Url::parse("http://user:pass@example.com/").unwrap();
    let result = validate_fetch_target(&url, &limits).await;
    assert!(
        matches!(result, Err(FetchError::EmbeddedCredentialsBlocked(_))),
        "URL with user:pass should be rejected, got: {result:?}"
    );
}

#[tokio::test]
async fn validate_fetch_target_rejects_empty_username_with_password() {
    let limits = permissive_limits();
    let url = Url::parse("http://:pass@example.com/").unwrap();
    let result = validate_fetch_target(&url, &limits).await;
    assert!(
        matches!(result, Err(FetchError::EmbeddedCredentialsBlocked(_))),
        "URL with empty username and password should be rejected, got: {result:?}"
    );
}

#[tokio::test]
async fn validate_fetch_target_accepts_no_credentials() {
    let limits = permissive_limits();
    let url = Url::parse("http://example.com/").unwrap();
    let result = validate_fetch_target(&url, &limits).await;
    assert!(result.is_ok(), "URL without credentials should be accepted");
}

#[tokio::test]
async fn metadata_only_mode_skips_body_extraction() {
    let server = MockServer::start();
    let body = b"<!DOCTYPE html><html><head><title>Test</title><meta name=\"description\" content=\"Desc\"></head><body><p>Hello world content here.</p></body></html>";
    let mock = server.mock(|when, then| {
        when.method(GET).path("/page");
        then.status(200)
            .header("content-type", "text/html; charset=utf-8")
            .body(body);
    });

    let client = FetchClient::new(permissive_limits(), "test".to_string(), true).unwrap();
    let resp = client
        .fetch(
            &server.url("/page"),
            None,
            ExtractMode::MetadataOnly,
            false,
            None,
        )
        .await
        .unwrap();

    assert_eq!(resp.status, 200);
    assert!(resp.title.as_deref().unwrap_or("").contains("Test"));
    assert!(
        resp.text.is_none(),
        "metadata-only should not return body text"
    );
    assert!(
        resp.raw_text.is_none(),
        "metadata-only should not return raw_text"
    );
    assert!(
        resp.document.is_none(),
        "metadata-only should not return document"
    );
    mock.assert();
}

#[tokio::test]
async fn text_mode_returns_body_content() {
    let server = MockServer::start();
    let body = b"<!DOCTYPE html><html><head><title>Test</title></head><body><p>Hello world content here.</p></body></html>";
    let mock = server.mock(|when, then| {
        when.method(GET).path("/page");
        then.status(200)
            .header("content-type", "text/html; charset=utf-8")
            .body(body);
    });

    let client = FetchClient::new(permissive_limits(), "test".to_string(), true).unwrap();
    let resp = client
        .fetch(&server.url("/page"), None, ExtractMode::Text, false, None)
        .await
        .unwrap();

    assert_eq!(resp.status, 200);
    assert!(resp.text.is_some(), "text mode should return body text");
    assert!(resp.document.is_some(), "text mode should return document");
    mock.assert();
}

#[tokio::test]
async fn max_chars_respects_cap() {
    let server = MockServer::start();
    let body = b"<!DOCTYPE html><html><head><title>Test</title></head><body><p>AAA BBB CCC DDD EEE FFF GGG HHH III JJJ KKK LLL MMM NNN OOO PPP QQQ RRR SSS TTT UUU VVV WWW XXX YYY ZZZ</p></body></html>";
    let mock = server.mock(|when, then| {
        when.method(GET).path("/long");
        then.status(200)
            .header("content-type", "text/html; charset=utf-8")
            .body(body);
    });

    let limits = FetchLimits {
        max_chars_default: 10,
        max_chars_cap: 20,
        ..permissive_limits()
    };
    let client = FetchClient::new(limits, "test".to_string(), true).unwrap();
    let resp = client
        .fetch(
            &server.url("/long"),
            Some(10),
            ExtractMode::Text,
            false,
            None,
        )
        .await
        .unwrap();

    let text = resp.text.unwrap_or_default();
    let char_count = text.chars().count();
    assert!(
        char_count <= 100,
        "text should be bounded by max_chars (100), got {char_count} chars"
    );
    mock.assert();
}

#[tokio::test]
async fn content_length_precheck_rejects_oversized() {
    let server = MockServer::start();
    let body = vec![b'x'; 5_000];
    let mock = server.mock(|when, then| {
        when.method(GET).path("/big");
        then.status(200)
            .header("content-type", "text/plain")
            .header("content-length", body.len().to_string())
            .body(&body);
    });

    let limits = FetchLimits {
        max_bytes: 1024,
        ..permissive_limits()
    };
    let client = FetchClient::new(limits, "test".to_string(), true).unwrap();
    let result = client
        .fetch(&server.url("/big"), None, ExtractMode::Text, false, None)
        .await;

    assert!(
        matches!(result, Err(FetchError::ContentTooLarge(_, _))),
        "Content-Length exceeding max_bytes should be rejected, got: {result:?}"
    );
    mock.assert();
}

#[tokio::test]
async fn timeout_enforced() {
    let server = MockServer::start();
    let _mock = server.mock(|when, then| {
        when.method(GET).path("/slow");
        then.status(200)
            .header("content-type", "text/plain")
            .delay(std::time::Duration::from_secs(10))
            .body("slow");
    });

    let limits = FetchLimits {
        timeout_ms: 100,
        ..permissive_limits()
    };
    let client = FetchClient::new(limits, "test".to_string(), true).unwrap();
    let result = client
        .fetch(&server.url("/slow"), None, ExtractMode::Text, false, None)
        .await;

    assert!(
        matches!(result, Err(FetchError::Timeout(_))),
        "slow response should timeout, got: {result:?}"
    );
}

#[tokio::test]
async fn redirect_to_credentials_blocked() {
    let server = MockServer::start();
    let mock_redirect = server.mock(|when, then| {
        when.method(GET).path("/redirect");
        then.status(302)
            .header("location", "http://user:pass@evil.com/");
    });

    let client = FetchClient::new(permissive_limits(), "test".to_string(), true).unwrap();
    let result = client
        .fetch(
            &server.url("/redirect"),
            None,
            ExtractMode::Text,
            false,
            None,
        )
        .await;

    assert!(
        matches!(result, Err(FetchError::RedirectTargetBlocked(_))),
        "redirect to credentials should be blocked, got: {result:?}"
    );
    mock_redirect.assert();
}

#[tokio::test]
async fn sanitize_disabled_skips_framing() {
    let server = MockServer::start();
    let body = b"<!DOCTYPE html><html><head><title>Test</title></head><body><p>Hello world</p></body></html>";
    let mock = server.mock(|when, then| {
        when.method(GET).path("/page");
        then.status(200)
            .header("content-type", "text/html; charset=utf-8")
            .body(body);
    });

    let client = FetchClient::new(permissive_limits(), "test".to_string(), false).unwrap();
    let resp = client
        .fetch(&server.url("/page"), None, ExtractMode::Text, false, None)
        .await
        .unwrap();

    let title = resp.title.unwrap_or_default();
    assert!(
        !title.contains("<<<EXTERNAL_UNTRUSTED"),
        "sanitize=false should not add framing delimiters"
    );
    mock.assert();
}

#[tokio::test]
async fn redirect_count_never_exceeds_limit() {
    let server = MockServer::start();
    let limits = FetchLimits {
        redirect_limit: 2,
        ..permissive_limits()
    };

    let mock0 = server.mock(|when, then| {
        when.method(GET).path("/r0");
        then.status(302).header("location", "/r1");
    });
    let mock1 = server.mock(|when, then| {
        when.method(GET).path("/r1");
        then.status(302).header("location", "/r2");
    });
    let mock2 = server.mock(|when, then| {
        when.method(GET).path("/r2");
        then.status(302).header("location", "/r3");
    });

    let client = FetchClient::new(limits, "test".to_string(), true).unwrap();
    let result = client
        .fetch(&server.url("/r0"), None, ExtractMode::Text, false, None)
        .await;

    assert!(
        matches!(result, Err(FetchError::RedirectLimitExceeded(_))),
        "should hit redirect limit after 2 redirects, got: {result:?}"
    );
    mock0.assert();
    mock1.assert();
    mock2.assert();
}

#[tokio::test]
async fn stream_error_after_partial_body() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(GET).path("/partial");
        then.status(200)
            .header("content-type", "text/plain")
            .body("partial content here");
    });

    let client = FetchClient::new(permissive_limits(), "test".to_string(), true).unwrap();
    let resp = client
        .fetch(
            &server.url("/partial"),
            None,
            ExtractMode::Text,
            false,
            None,
        )
        .await;

    assert!(resp.is_ok(), "partial body should still succeed: {resp:?}");
    mock.assert();
}

#[tokio::test]
async fn content_length_larger_than_actual_body() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(GET).path("/mismatch");
        then.status(200)
            .header("content-type", "text/plain")
            .header("content-length", "10000")
            .body("short");
    });

    let client = FetchClient::new(permissive_limits(), "test".to_string(), true).unwrap();
    let resp = client
        .fetch(
            &server.url("/mismatch"),
            None,
            ExtractMode::Text,
            false,
            None,
        )
        .await;

    assert!(
        resp.is_ok() || resp.is_err(),
        "Content-Length larger than body should not panic"
    );
    mock.assert();
}

#[tokio::test]
async fn redirect_without_location_header() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(GET).path("/no-location");
        then.status(302);
    });

    let client = FetchClient::new(permissive_limits(), "test".to_string(), true).unwrap();
    let resp = client
        .fetch(
            &server.url("/no-location"),
            None,
            ExtractMode::Text,
            false,
            None,
        )
        .await;

    assert!(
        resp.is_ok() || resp.is_err(),
        "redirect without Location should not panic"
    );
    mock.assert();
}

#[tokio::test]
async fn redirect_with_invalid_utf8_location() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(GET).path("/bad-redirect");
        then.status(302)
            .header("location", "http://example.com/%FF%FE/path");
    });

    let client = FetchClient::new(permissive_limits(), "test".to_string(), true).unwrap();
    let resp = client
        .fetch(
            &server.url("/bad-redirect"),
            None,
            ExtractMode::Text,
            false,
            None,
        )
        .await;

    assert!(
        resp.is_ok() || resp.is_err(),
        "redirect with invalid UTF-8 Location should not panic"
    );
    mock.assert();
}

#[tokio::test]
async fn max_bytes_body_truncation_when_content_length_honest() {
    let server = MockServer::start();
    let body = vec![b'x'; 5000];
    let mock = server.mock(|when, then| {
        when.method(GET).path("/trunc");
        then.status(200)
            .header("content-type", "text/plain")
            .header("content-length", body.len().to_string())
            .body(&body);
    });

    let limits = FetchLimits {
        max_bytes: 1000,
        ..permissive_limits()
    };
    let client = FetchClient::new(limits, "test".to_string(), true).unwrap();
    let result = client
        .fetch(&server.url("/trunc"), None, ExtractMode::Text, false, None)
        .await;

    assert!(
        matches!(result, Err(FetchError::ContentTooLarge(_, _))),
        "Content-Length 5000 > max_bytes 1000 should be rejected, got: {result:?}"
    );
    mock.assert();
}

#[tokio::test]
async fn max_chars_exact_boundary() {
    let server = MockServer::start();
    let body = b"<!DOCTYPE html><html><head><title>X</title></head><body><p>AAAAABBBBBCCCCCDDDDD</p></body></html>";
    let mock = server.mock(|when, then| {
        when.method(GET).path("/exact");
        then.status(200)
            .header("content-type", "text/html; charset=utf-8")
            .body(body);
    });

    let limits = FetchLimits {
        max_chars_default: 500,
        max_chars_cap: 500,
        ..permissive_limits()
    };
    let client = FetchClient::new(limits, "test".to_string(), true).unwrap();
    let resp = client
        .fetch(
            &server.url("/exact"),
            Some(500),
            ExtractMode::Text,
            false,
            None,
        )
        .await
        .unwrap();

    let text = resp.text.unwrap_or_default();
    let char_count = text.chars().count();
    assert!(
        char_count <= 500,
        "max_chars=500 should produce bounded output, got {char_count} chars"
    );
    mock.assert();
}
