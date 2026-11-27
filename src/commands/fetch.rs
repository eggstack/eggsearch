//! `eggsearch fetch`: fetch and extract content from a URL.

use anyhow::{anyhow, Result};
use eggsearch::core::config::AppConfig;
use eggsearch::core::fetch::{ExtractMode, WebFetchResponse};
use eggsearch::fetch::FetchClient;

#[cfg(feature = "browser")]
use std::sync::Arc;

#[cfg(feature = "browser")]
use eggsearch::fetch::browser::{
    browser_discovery_reason, browser_fetch_with_policy, browser_result_to_response,
    discover_browser, escalate, gated_status, BrowserConfig, BrowserFetchError,
    BrowserFetchOutcome, BrowserLifecycle, EscalationOutcome, EscalationRequest, RenderPolicy,
};

/// CLI-visible link cap; the extractor retains more for programmatic consumers.
const CLI_DISPLAY_MAX_LINKS: usize = 20;

#[allow(clippy::too_many_arguments)]
pub async fn run(
    cfg: &AppConfig,
    url: &str,
    max_chars: Option<usize>,
    timeout_ms: Option<u64>,
    metadata_only: bool,
    markdown: bool,
    include_links: bool,
    as_json: bool,
) -> Result<()> {
    let response = fetch_response(
        cfg,
        url,
        max_chars,
        timeout_ms,
        metadata_only,
        markdown,
        include_links,
    )
    .await?;

    if as_json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json_payload(&response))?
        );
    } else {
        println!("# Fetch: {url}\n");
        println!("Final URL: {}", response.final_url);
        if let Some(title) = &response.title {
            println!("Title: {title}");
        }
        if let Some(desc) = &response.description {
            println!("Description: {desc}");
        }
        println!("Status: {}", response.status);
        println!(
            "Content-Type: {}",
            response.content_type.as_deref().unwrap_or("unknown")
        );
        println!("Fetched: {}", response.fetched);
        println!("Truncated: {}", response.truncated);
        if response.manual_interaction_required {
            println!(
                "Manual interaction required: the response is an anti-bot challenge page, \
                 not the requested document"
            );
        }
        if let Some(text) = &response.text {
            println!("\n--- Content ({} chars) ---", text.chars().count());
            println!("{text}");
        }
        if !response.links.is_empty() {
            println!(
                "\n--- Links ({} returned, {} seen{}) ---",
                response.links.len(),
                response.links_seen.unwrap_or(response.links.len()),
                if response.links_truncated {
                    ", truncated"
                } else {
                    ""
                }
            );
            for link in response.links.iter().take(CLI_DISPLAY_MAX_LINKS) {
                println!(
                    "  - [{}] {}: {}",
                    format!("{:?}", link.link_kind).to_lowercase(),
                    link.text,
                    link.url
                );
            }
        }
        if !response.warnings.is_empty() {
            println!("\nWarnings:");
            for w in &response.warnings {
                println!("  - {w}");
            }
        }
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn fetch_response(
    cfg: &AppConfig,
    url: &str,
    max_chars: Option<usize>,
    timeout_ms: Option<u64>,
    metadata_only: bool,
    markdown: bool,
    include_links: bool,
) -> Result<WebFetchResponse> {
    cfg.validate()
        .map_err(|e| anyhow!("invalid configuration: {e}"))?;
    if !cfg.fetch.enabled {
        anyhow::bail!("fetch is disabled in config; set [fetch].enabled = true to enable");
    }

    if url.trim().is_empty() {
        anyhow::bail!("url must not be empty");
    }
    let parsed = url::Url::parse(url).map_err(|e| anyhow!("invalid URL '{url}': {e}"))?;
    match parsed.scheme() {
        "http" | "https" => {}
        other => anyhow::bail!("URL must be http or https; got '{other}'"),
    }

    #[cfg(not(feature = "browser"))]
    if cfg.fetch.browser.policy == "browser" {
        anyhow::bail!("browser_not_compiled_in: browser rendering is not compiled into this build");
    }

    if let Some(0) = max_chars {
        anyhow::bail!("max_chars must be > 0");
    }

    let mut limits = cfg.fetch_limits();
    if let Some(t) = timeout_ms {
        if t == 0 {
            anyhow::bail!("timeout_ms must be > 0");
        }
        limits.timeout_ms = t;
    }

    #[cfg(feature = "browser")]
    let budgets = eggsearch::fetch::budget::FetchBudgets::resolve(&limits, max_chars);

    let user_agent = cfg.fetch_user_agent();
    let effective_timeout_ms = limits.timeout_ms;
    let client = FetchClient::new(limits, user_agent.clone(), cfg.fetch.sanitize_output)?;

    let extract_mode = if metadata_only {
        ExtractMode::MetadataOnly
    } else if markdown {
        ExtractMode::Markdown
    } else {
        ExtractMode::Text
    };

    let include_links = include_links || cfg.fetch.include_links_default;
    let deadline =
        std::time::Instant::now() + std::time::Duration::from_millis(effective_timeout_ms);

    #[cfg(feature = "browser")]
    let browser_settings = BrowserConfig::from_settings(&cfg.fetch.browser);
    #[cfg(feature = "browser")]
    let render_policy = cli_render_policy(&browser_settings);
    #[cfg(feature = "browser")]
    let browser_discovery_state = discover_browser(browser_settings.executable.as_deref());
    #[cfg(feature = "browser")]
    let browser_unavailable_reason = browser_discovery_reason(&browser_discovery_state)
        .unwrap_or_else(|| "browser rendering is unavailable".to_string());
    #[cfg(feature = "browser")]
    let browser_lifecycle = if matches!(render_policy, RenderPolicy::HttpOnly) {
        None
    } else {
        Some(Arc::new(BrowserLifecycle::new(
            browser_discovery_state.discovery().cloned(),
            browser_settings.clone(),
        )))
    };

    #[cfg(feature = "browser")]
    if matches!(render_policy, RenderPolicy::Browser) {
        let lifecycle = browser_lifecycle
            .as_ref()
            .ok_or_else(|| anyhow!("browser_disabled: browser rendering is disabled in config"))?;
        if !lifecycle.is_available() {
            anyhow::bail!("browser_unavailable: {browser_unavailable_reason}");
        }
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        if remaining.as_millis() < 2000 {
            anyhow::bail!("browser rendering needs at least 2000ms of the call budget");
        }
        let rendered = tokio::time::timeout(
            remaining,
            browser_fetch_with_policy(
                lifecycle,
                url,
                &browser_settings,
                cfg.fetch.sanitize_output,
                &render_policy,
            ),
        )
        .await
        .map_err(|_| anyhow!("browser_unavailable: browser rendering timed out"))?
        .map_err(|error| match error {
            BrowserFetchError::LaunchFailed(reason) => {
                anyhow!("browser_unavailable: browser launch failed: {reason}")
            }
            other => anyhow!("browser_fetch_failed: {other}"),
        })?;
        if rendered.challenge_completed {
            eggsearch::challenge_replay::record_browser_completion();
        }
        return Ok(browser_result_to_response(
            rendered,
            url,
            budgets,
            extract_mode,
            include_links,
            cfg.fetch.sanitize_output,
        ));
    }

    let challenge_plan = eggsearch::challenge_replay::ChallengePlan {
        challenge: &cfg.search.challenge,
        user_agent: &user_agent,
        origin_controller: None,
        max_chars,
        extract_mode,
        include_links,
        pdf_options: None,
        deadline,
    };

    match eggsearch::challenge_replay::fetch_with_challenge_replay(&client, url, &challenge_plan)
        .await
    {
        Ok(response) => {
            #[allow(unused_mut)]
            let mut response = response;
            #[cfg(feature = "browser")]
            if matches!(render_policy, RenderPolicy::Auto) && response_requires_browser(&response) {
                if let Some(lifecycle) = browser_lifecycle.as_ref() {
                    if !lifecycle.is_available() {
                        response.structured_warnings.push(
                            eggsearch::core::warning::AgentWarning::new(
                                eggsearch::core::warning::WarningCode::BrowserUnavailable,
                                browser_unavailable_reason.clone(),
                            ),
                        );
                    } else {
                        let remaining =
                            deadline.saturating_duration_since(std::time::Instant::now());
                        let unavailable_reason = if remaining.as_millis() < 2000 {
                            Some(
                                "insufficient remaining call budget for browser rendering"
                                    .to_string(),
                            )
                        } else {
                            match tokio::time::timeout(
                                remaining,
                                browser_fetch_with_policy(
                                    lifecycle,
                                    url,
                                    &browser_settings,
                                    cfg.fetch.sanitize_output,
                                    &render_policy,
                                ),
                            )
                            .await
                            {
                                Ok(Ok(rendered)) => {
                                    if rendered.challenge_completed {
                                        eggsearch::challenge_replay::record_browser_completion();
                                    }
                                    return Ok(browser_result_to_response(
                                        rendered,
                                        url,
                                        budgets,
                                        extract_mode,
                                        include_links,
                                        cfg.fetch.sanitize_output,
                                    ));
                                }
                                Ok(Err(BrowserFetchError::LaunchFailed(reason))) => {
                                    Some(format!("browser launch failed: {reason}"))
                                }
                                Ok(Err(error)) => {
                                    Some(format!("browser escalation failed: {error}"))
                                }
                                Err(_) => {
                                    Some("browser escalation exceeded the call budget".to_string())
                                }
                            }
                        };
                        if let Some(reason) = unavailable_reason {
                            response.structured_warnings.push(
                                eggsearch::core::warning::AgentWarning::new(
                                    eggsearch::core::warning::WarningCode::BrowserUnavailable,
                                    reason,
                                ),
                            );
                        }
                    }
                }
            }
            #[cfg(not(feature = "browser"))]
            if cfg.fetch.browser.enabled
                && cfg.fetch.browser.policy == "auto"
                && response_requires_browser(&response)
            {
                response
                    .structured_warnings
                    .push(eggsearch::core::warning::AgentWarning::new(
                        eggsearch::core::warning::WarningCode::BrowserUnavailable,
                        "browser rendering is not compiled into this build".to_string(),
                    ));
            }
            Ok(response)
        }
        Err(error) => {
            #[cfg(feature = "browser")]
            if matches!(render_policy, RenderPolicy::Auto) {
                if let Some(status) = gated_status(&error, &render_policy) {
                    let lifecycle = browser_lifecycle.as_ref();
                    let outcome = escalate(
                        EscalationRequest {
                            url,
                            status,
                            policy: &render_policy,
                            browser_available: lifecycle
                                .is_some_and(|lifecycle| lifecycle.is_available()),
                            unavailable_reason: lifecycle
                                .map(|_| browser_unavailable_reason.as_str()),
                            profile_name: None,
                            budgets,
                            extract_mode,
                            include_links,
                            sanitize_output: cfg.fetch.sanitize_output,
                            deadline,
                        },
                        || async {
                            let Some(lifecycle) = lifecycle else {
                                return Err(BrowserFetchError::LaunchFailed(
                                    "no browser is available".to_string(),
                                ));
                            };
                            browser_fetch_with_policy(
                                lifecycle,
                                url,
                                &browser_settings,
                                cfg.fetch.sanitize_output,
                                &render_policy,
                            )
                            .await
                            .map(|result| BrowserFetchOutcome {
                                result,
                                screenshot: None,
                            })
                        },
                    )
                    .await;
                    return match outcome {
                        EscalationOutcome::Served {
                            response,
                            challenge_completed,
                        } => {
                            if challenge_completed {
                                eggsearch::challenge_replay::record_browser_completion();
                            }
                            Ok(*response)
                        }
                        EscalationOutcome::Failed { error, .. } => {
                            Err(anyhow!("fetch failed: {}: {}", error.error_code(), error))
                        }
                        EscalationOutcome::ManualInteraction {
                            origin,
                            message,
                            next_action,
                        } => {
                            let next_action = next_action.unwrap_or_else(|| {
                                format!("eggsearch browser-login {origin} --profile <name>")
                            });
                            let error = eggsearch::fetch::FetchError::HttpStatus(
                                status,
                                format!("browser requires manual interaction for {origin}: {message}; run `{next_action}`"),
                            );
                            Err(anyhow!("fetch failed: {}: {}", error.error_code(), error))
                        }
                    };
                }
            }
            Err(anyhow!("fetch failed: {}: {}", error.error_code(), error))
        }
    }
}

fn response_requires_browser(response: &WebFetchResponse) -> bool {
    let body = response.raw_text.as_deref().unwrap_or("");
    let text_len = response
        .raw_text_chars_returned
        .unwrap_or_else(|| body.chars().count());
    eggsearch::fetch::classify::browser_rendering_is_warranted(
        response.status,
        response.content_type.as_deref(),
        response.title.as_deref(),
        text_len,
        body.as_bytes(),
    )
}

/// Resolve the CLI policy; a disabled browser forces `http_only`, matching MCP behavior.
#[cfg(feature = "browser")]
fn cli_render_policy(settings: &BrowserConfig) -> RenderPolicy {
    if settings.enabled {
        settings.policy.clone()
    } else {
        RenderPolicy::HttpOnly
    }
}

fn json_payload(response: &WebFetchResponse) -> serde_json::Value {
    serde_json::json!({
        "url": response.url,
        "final_url": response.final_url,
        "title": response.title,
        "description": response.description,
        "content_type": response.content_type,
        "status": response.status,
        "fetched": response.fetched,
        "truncated": response.truncated,
        "trust": "external_untrusted",
        "text": response.text,
        "links": response.links,
        "trust_markers": response.trust_markers,
        "document": response.document,
        "warnings": response.warnings,
        "structured_warnings": response.structured_warnings,
        "manual_interaction_required": response.manual_interaction_required,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Mock origin without a cached pass cookie, avoiding loopback-port reuse between tests.
    fn challenge_server() -> httpmock::MockServer {
        let server = httpmock::MockServer::start();
        if let Some(origin) = eggsearch::challenge_replay::cache::origin_key(&server.url("/")) {
            eggsearch::challenge_replay::cache::fetch_pass_cookie_cache().remove(&origin);
        }
        server
    }

    use eggsearch::core::config::AppConfig;

    #[cfg(feature = "browser")]
    #[test]
    fn cli_render_policy_reads_the_operator_setting() {
        use eggsearch::fetch::browser::RenderPolicy;

        let mut section = eggsearch::core::config::FetchBrowserSection {
            enabled: true,
            ..Default::default()
        };
        for (spelling, expected) in [
            ("http_only", RenderPolicy::HttpOnly),
            ("auto", RenderPolicy::Auto),
            ("browser", RenderPolicy::Browser),
            ("unrecognised", RenderPolicy::HttpOnly),
        ] {
            section.policy = spelling.to_string();
            assert_eq!(
                cli_render_policy(&BrowserConfig::from_settings(&section)),
                expected,
                "policy {spelling}"
            );
        }

        section.policy = "browser".to_string();
        section.enabled = false;
        assert_eq!(
            cli_render_policy(&BrowserConfig::from_settings(&section)),
            RenderPolicy::HttpOnly,
            "a disabled browser never renders"
        );
    }

    #[cfg(feature = "browser")]
    fn browser_absent_config() -> AppConfig {
        let mut cfg = AppConfig::default();
        cfg.fetch.allow_localhost = true;
        cfg.fetch.allow_private_network = true;
        cfg.fetch.browser.enabled = true;
        cfg.fetch.browser.executable = Some("/synthetic/missing/chrome".to_string());
        cfg
    }

    #[cfg(feature = "browser")]
    #[tokio::test]
    async fn browser_absent_auto_warns_only_when_rendering_is_warranted() {
        use httpmock::prelude::*;

        let server = MockServer::start();
        let _ordinary = server.mock(|when, then| {
            when.method(GET).path("/ordinary");
            then.status(200)
                .header("content-type", "text/html")
                .body("<html><body>Ordinary page with readable content and no render markers.</body></html>");
        });
        let _verification = server.mock(|when, then| {
            when.method(GET).path("/verification");
            then.status(200)
                .header("content-type", "text/html")
                .body("<html><head><title>Checking your browser</title></head><body>Please wait while we verify.</body></html>");
        });
        let cfg = browser_absent_config();

        let ordinary = fetch_response(
            &cfg,
            &server.url("/ordinary"),
            Some(1000),
            Some(5000),
            false,
            false,
            false,
        )
        .await
        .expect("ordinary HTTP fetch succeeds without Chrome");
        assert!(!ordinary.structured_warnings.iter().any(
            |warning| warning.code == eggsearch::core::warning::WarningCode::BrowserUnavailable
        ));

        let verification = fetch_response(
            &cfg,
            &server.url("/verification"),
            Some(1000),
            Some(5000),
            false,
            false,
            false,
        )
        .await
        .expect("auto fetch retains HTTP response when Chrome is absent");
        assert!(verification
            .structured_warnings
            .iter()
            .any(|warning| warning.code
                == eggsearch::core::warning::WarningCode::BrowserUnavailable
                && warning.message.contains("/synthetic/missing/chrome")));
    }

    #[cfg(not(feature = "browser"))]
    #[tokio::test]
    async fn browser_uncompiled_auto_warns_only_when_rendering_is_warranted() {
        use httpmock::prelude::*;

        let server = MockServer::start();
        let _ordinary = server.mock(|when, then| {
            when.method(GET).path("/ordinary");
            then.status(200)
                .header("content-type", "text/html")
                .body("<html><body>Ordinary readable article content.</body></html>");
        });
        let _verification = server.mock(|when, then| {
            when.method(GET).path("/verification");
            then.status(200)
                .header("content-type", "text/html")
                .body("<html><head><title>Checking your browser</title></head><body>Please wait while we verify.</body></html>");
        });
        let mut cfg = AppConfig::default();
        cfg.fetch.allow_localhost = true;
        cfg.fetch.allow_private_network = true;
        cfg.fetch.browser.enabled = true;
        cfg.fetch.browser.policy = "auto".to_string();

        let ordinary = fetch_response(
            &cfg,
            &server.url("/ordinary"),
            Some(1000),
            Some(5000),
            false,
            false,
            false,
        )
        .await
        .expect("ordinary HTTP fetch succeeds without the browser feature");
        assert!(!ordinary.structured_warnings.iter().any(|warning| {
            warning.code == eggsearch::core::warning::WarningCode::BrowserUnavailable
        }));

        let verification = fetch_response(
            &cfg,
            &server.url("/verification"),
            Some(1000),
            Some(5000),
            false,
            false,
            false,
        )
        .await
        .expect("auto fetch retains HTTP response without the browser feature");
        assert!(verification.structured_warnings.iter().any(|warning| {
            warning.code == eggsearch::core::warning::WarningCode::BrowserUnavailable
                && warning.message.contains("not compiled")
        }));
    }

    #[cfg(not(feature = "browser"))]
    #[tokio::test]
    async fn browser_not_compiled_cli_request_has_stable_code() {
        let mut cfg = AppConfig::default();
        cfg.fetch.browser.policy = "browser".to_string();
        let error = fetch_response(
            &cfg,
            "https://example.test/explicit-browser",
            None,
            Some(5000),
            false,
            false,
            false,
        )
        .await
        .expect_err("explicit browser request requires the browser feature");
        assert!(
            error.to_string().contains("browser_not_compiled_in"),
            "{error}"
        );
    }

    #[cfg(feature = "browser")]
    #[tokio::test]
    async fn browser_absent_explicit_cli_request_names_code_and_reason() {
        let mut cfg = browser_absent_config();
        cfg.fetch.browser.policy = "browser".to_string();
        let error = fetch_response(
            &cfg,
            "https://example.test/explicit-browser",
            None,
            Some(5000),
            false,
            false,
            false,
        )
        .await
        .expect_err("explicit browser request must reject unavailable Chrome");
        assert!(error.to_string().contains("browser_unavailable"));
        assert!(error.to_string().contains("/synthetic/missing/chrome"));
    }

    #[tokio::test]
    async fn run_zero_max_chars_returns_error() {
        let cfg = AppConfig::default();
        let err = run(
            &cfg,
            "https://example.com",
            Some(0),
            None,
            false,
            false,
            false,
            false,
        )
        .await
        .expect_err("expected max_chars validation error");
        assert!(
            err.to_string().contains("max_chars must be > 0"),
            "got: {err}"
        );
    }

    #[tokio::test]
    async fn run_zero_timeout_ms_returns_error() {
        let cfg = AppConfig::default();
        let err = run(
            &cfg,
            "https://example.com",
            None,
            Some(0),
            false,
            false,
            false,
            false,
        )
        .await
        .expect_err("expected timeout_ms validation error");
        assert!(
            err.to_string().contains("timeout_ms must be > 0"),
            "got: {err}"
        );
    }

    #[tokio::test]
    async fn run_disabled_by_config_returns_error() {
        let mut cfg = AppConfig::default();
        cfg.fetch.enabled = false;
        let err = run(
            &cfg,
            "https://example.com",
            None,
            None,
            false,
            false,
            false,
            false,
        )
        .await
        .expect_err("expected fetch-disabled error");
        assert!(err.to_string().contains("disabled"), "got: {err}");
        assert!(err.to_string().contains("[fetch].enabled"), "got: {err}");
    }

    const ANUBIS_PASS_COOKIE: &str = "pass-cookie-123";

    fn anubis_fast_challenge_page() -> String {
        let payload = serde_json::json!({
            "rules": {"algorithm": "fast", "difficulty": 2},
            "challenge": {
                "issuedAt": "2026-11-11T00:00:00Z",
                "metadata": {"User-Agent": "eggsearch"},
                "id": "01a1cc17-0000-7000-8000-000000000000",
                "method": "fast",
                "randomData": "abc123def456",
                "redir": "/",
                "policyRuleHash": "00",
                "difficulty": 2,
                "spent": false
            }
        });
        format!(
            r#"<html><head><script id="anubis_challenge" type="application/json">{payload}</script></head><body><div id="app"></div></body></html>"#
        )
    }

    fn challenge_config(enabled: bool) -> AppConfig {
        let mut cfg = AppConfig::default();
        cfg.fetch.allow_localhost = true;
        cfg.fetch.allow_private_network = true;
        cfg.search.challenge.enabled = enabled;
        cfg
    }

    #[tokio::test]
    async fn fetch_response_solves_a_published_anubis_challenge_when_enabled() {
        use httpmock::prelude::*;
        let server = challenge_server();
        let replay = server.mock(|when, then| {
            when.method(GET)
                .path("/")
                .header("cookie", format!("spchal-auth={ANUBIS_PASS_COOKIE}"));
            then.status(200)
                .header("content-type", "text/html; charset=utf-8")
                .body(
                    "<html><head><title>Rust</title></head><body>                     <h1>Rust programming language</h1></body></html>",
                );
        });
        let wall = server.mock(|when, then| {
            when.method(GET).path("/");
            then.status(200)
                .header("content-type", "text/html; charset=utf-8")
                .header("set-cookie", "sp_pow=pow-1; Path=/")
                .body(anubis_fast_challenge_page());
        });
        let pass = server.mock(|when, then| {
            when.method(GET)
                .path("/.within.website/x/cmd/anubis/api/pass-challenge");
            then.status(200)
                .header(
                    "set-cookie",
                    format!("spchal-auth={ANUBIS_PASS_COOKIE}; Path=/; HttpOnly"),
                )
                .body("");
        });

        let cfg = challenge_config(true);
        let response = fetch_response(&cfg, &server.url("/"), None, None, false, false, false)
            .await
            .expect("cli fetch solves the challenge");

        assert_eq!(wall.calls(), 1, "the wall is fetched once");
        assert_eq!(
            pass.calls(),
            1,
            "the cli must attempt the published challenge"
        );
        assert_eq!(
            replay.calls(),
            1,
            "the cli must replay with the issued cookie"
        );
        assert!(!response.manual_interaction_required);
        let text = response.text.clone().unwrap_or_default();
        assert!(
            text.contains("Rust programming language"),
            "the replayed document must be returned: {text}"
        );
        let payload = json_payload(&response);
        assert_eq!(payload["manual_interaction_required"], false);
    }

    #[tokio::test]
    async fn fetch_response_marks_the_challenge_when_solving_is_disabled() {
        use httpmock::prelude::*;
        let server = challenge_server();
        let wall = server.mock(|when, then| {
            when.method(GET).path("/");
            then.status(200)
                .header("content-type", "text/html; charset=utf-8")
                .body(anubis_fast_challenge_page());
        });
        let pass = server.mock(|when, then| {
            when.method(GET)
                .path("/.within.website/x/cmd/anubis/api/pass-challenge");
            then.status(200).body("");
        });

        let cfg = challenge_config(false);
        let response = fetch_response(&cfg, &server.url("/"), None, None, false, false, false)
            .await
            .expect("cli fetch returns the marked wall");

        assert_eq!(wall.calls(), 1);
        assert_eq!(
            pass.calls(),
            0,
            "a disabled solver must not attempt the challenge"
        );
        assert!(response.manual_interaction_required);
        let payload = json_payload(&response);
        assert_eq!(payload["manual_interaction_required"], true);
        let warnings = payload["structured_warnings"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        assert!(
            warnings
                .iter()
                .any(|warning| warning["code"] == "fetch_challenge_page"),
            "got: {warnings:?}"
        );
    }

    const ALTCHA_PASS_COOKIE: &str = "altcha-pass-cookie";

    const ALTCHA_SOLVABLE_DOCUMENT: &str = r#"{"parameters":{"algorithm":"PBKDF2/SHA-256","cost":100,"keyLength":32,"keyPrefix":"b054884358ba2e5e24a53c818185502e","nonce":"0102030405060708","salt":"0a0b0c0d0e0f1011","keySignature":"00","expiresAt":1794425890},"signature":"00"}"#;

    #[tokio::test]
    async fn fetch_response_solves_a_published_altcha_challenge_when_enabled() {
        use httpmock::prelude::*;
        let server = challenge_server();
        let replay = server.mock(|when, then| {
            when.method(GET)
                .path("/")
                .header("cookie", format!("chllg={ALTCHA_PASS_COOKIE}"));
            then.status(200)
                .header("content-type", "text/html; charset=utf-8")
                .body(
                    "<html><head><title>Rust</title></head><body><h1>Rust programming language</h1></body></html>",
                );
        });
        let wall = server.mock(|when, then| {
            when.method(GET).path("/");
            then.status(200)
                .header("content-type", "text/html; charset=utf-8")
                .body(
                    "<html><body><div class=\"captcha-wrap\"><altcha-widget challenge=\"/captcha/challenge\"></altcha-widget></div></body></html>",
                );
        });
        let challenge = server.mock(|when, then| {
            when.method(GET).path("/captcha/challenge");
            then.status(200)
                .header("content-type", "application/json")
                .body(ALTCHA_SOLVABLE_DOCUMENT);
        });
        let verify = server.mock(|when, then| {
            when.method(POST).path("/captcha/verify");
            then.status(200)
                .header(
                    "set-cookie",
                    format!("chllg={ALTCHA_PASS_COOKIE}; Path=/; HttpOnly; Max-Age=300"),
                )
                .body("");
        });

        let cfg = challenge_config(true);
        let response = fetch_response(&cfg, &server.url("/"), None, None, false, false, false)
            .await
            .expect("cli fetch solves the challenge");

        assert_eq!(wall.calls(), 1, "the wall is fetched once");
        assert_eq!(
            challenge.calls(),
            1,
            "the published challenge is fetched once"
        );
        assert_eq!(verify.calls(), 1, "the solution is verified once");
        assert_eq!(
            replay.calls(),
            1,
            "the cli must replay with the issued cookie"
        );
        assert!(!response.manual_interaction_required);
        let text = response.text.clone().unwrap_or_default();
        assert!(
            text.contains("Rust programming language"),
            "the replayed document must be returned: {text}"
        );
        let payload = json_payload(&response);
        assert_eq!(payload["manual_interaction_required"], false);
    }

    #[tokio::test]
    async fn fetch_response_marks_the_altcha_challenge_when_solving_is_disabled() {
        use httpmock::prelude::*;
        let server = challenge_server();
        let wall = server.mock(|when, then| {
            when.method(GET).path("/");
            then.status(200)
                .header("content-type", "text/html; charset=utf-8")
                .body(
                    "<html><body><div class=\"captcha-wrap\"><altcha-widget challenge=\"/captcha/challenge\"></altcha-widget></div></body></html>",
                );
        });
        let challenge = server.mock(|when, then| {
            when.method(GET).path("/captcha/challenge");
            then.status(200)
                .header("content-type", "application/json")
                .body(ALTCHA_SOLVABLE_DOCUMENT);
        });
        let verify = server.mock(|when, then| {
            when.method(POST).path("/captcha/verify");
            then.status(200).body("");
        });

        let cfg = challenge_config(false);
        let response = fetch_response(&cfg, &server.url("/"), None, None, false, false, false)
            .await
            .expect("cli fetch returns the marked wall");

        assert_eq!(wall.calls(), 1);
        assert_eq!(
            challenge.calls(),
            0,
            "a disabled solver must not fetch the published challenge"
        );
        assert_eq!(verify.calls(), 0);
        assert!(response.manual_interaction_required);
        let payload = json_payload(&response);
        assert_eq!(payload["manual_interaction_required"], true);
        let warnings = payload["structured_warnings"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        assert!(
            warnings
                .iter()
                .any(|warning| warning["code"] == "fetch_challenge_page"),
            "got: {warnings:?}"
        );
    }
}
