//! `eggsearch fetch`: fetch and extract content from a URL.

use anyhow::{anyhow, Result};
use eggsearch::core::config::AppConfig;
use eggsearch::core::fetch::{ExtractMode, WebFetchResponse};
use eggsearch::fetch::FetchClient;

/// CLI display cap for links. Distinct from [`crate::fetch::extract::MAX_LINKS`]
/// (the in-memory extractor cap) because the CLI is human-facing and only
/// needs enough links to be useful for a glance at the page; the extractor
/// returns more so that programmatic consumers can pick from a richer set.
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

    let challenge_plan = eggsearch::challenge_replay::ChallengePlan {
        challenge: &cfg.search.challenge,
        user_agent: &user_agent,
        origin_controller: None,
        max_chars,
        extract_mode,
        include_links,
        pdf_options: None,
        deadline: std::time::Instant::now()
            + std::time::Duration::from_millis(effective_timeout_ms),
    };

    eggsearch::challenge_replay::fetch_with_challenge_replay(&client, url, &challenge_plan)
        .await
        .map_err(|e| anyhow!("fetch failed: {}: {}", e.error_code(), e))
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
    use eggsearch::core::config::AppConfig;

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
        let server = MockServer::start();
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

        assert_eq!(wall.hits(), 1, "the wall is fetched once");
        assert_eq!(
            pass.hits(),
            1,
            "the cli must attempt the published challenge"
        );
        assert_eq!(
            replay.hits(),
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
        let server = MockServer::start();
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

        assert_eq!(wall.hits(), 1);
        assert_eq!(
            pass.hits(),
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
}
