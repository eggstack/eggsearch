//! Test-only parse entry point for the captured-fixture conformance suite.

use super::*;

/// Parse captured provider HTML with the named HTML scraper.
///
/// Recognised ids are `brave`, `duckduckgo`, `mojeek`, `startpage`, and
/// `yahoo`. Any other id returns [`EngineError::Unsupported`].
pub fn parse_html(
    provider_id: &str,
    html: &str,
    max_results: usize,
) -> Result<Vec<SearchResult>, EngineError> {
    match provider_id {
        "brave" => brave::parse(html, max_results),
        "duckduckgo" => duckduckgo::parse(html, max_results),
        "mojeek" => mojeek::parse(html, max_results),
        "startpage" => startpage::parse(html, max_results),
        "yahoo" => yahoo::parse(html, max_results),
        other => Err(EngineError::Unsupported {
            engine: "test_harness",
            reason: format!("unknown HTML scraper '{other}'"),
        }),
    }
}
