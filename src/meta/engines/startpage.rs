use std::time::{Duration, SystemTime};

use eggfetch_core::Client;
use scraper::Html;

use super::error::EngineError;
use super::models::SearchResult;
use super::{PageMarker, ScrapeShape};
use crate::core::config::ChallengeSection;
use crate::meta::challenge::{
    anubis, cookie, lookup_pass_cookie, pass_cookie_cache, SolveBudget, SolveOutcome,
    BOUND_COOKIE_REJECTED,
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
        lookup_pass_cookie(ENGINE)
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

    let (cookie, lifetime) = solve_cookie(
        client,
        base_url,
        &first.body,
        &request_url,
        &first.cookies,
        timeout,
        challenge,
    )
    .await?;
    let ttl = cookie::reuse_ttl(
        lifetime,
        SystemTime::now(),
        Duration::from_secs(challenge.cookie_ttl_seconds),
    );
    pass_cookie_cache().put(ENGINE, &cookie, ttl);

    let replayed = send_search(client, &search_url, query, Some(&cookie), timeout).await?;
    if is_challenge_page(&replayed.body)? {
        pass_cookie_cache().remove(ENGINE);
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
        request = request.header("Cookie", cookie);
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
) -> Result<(String, cookie::PublishedLifetime), EngineError> {
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
#[path = "startpage_tests.rs"]
mod tests;
