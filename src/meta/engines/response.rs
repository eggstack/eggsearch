use eggfetch_core::Response;

use super::error::EngineError;
use super::{marker_matches, read_bounded_body, ScrapeShape};

/// Classify a non-success search response, preferring a recognised challenge
/// page over a bare status failure.
///
/// Brave answers a bot check with HTTP 429 while serving a proof-of-work page,
/// so a status-only classification would report `rate_limited` for something a
/// browser can complete. `raw_markers` covers script-rendered shells whose
/// visible text never appears as a node and is checked on non-success responses
/// only, so a normal page's embedded string table cannot match.
pub(crate) fn classify_search_failure(
    engine: &'static str,
    status: u16,
    body: &str,
    shape: &ScrapeShape,
    raw_markers: &[&'static str],
) -> EngineError {
    let document = scraper::Html::parse_document(body);
    let mut rendered: Option<String> = None;
    for marker in shape.blocked_markers {
        match marker_matches(engine, *marker, &document, &mut rendered) {
            Ok(true) => {
                return EngineError::Blocked {
                    engine,
                    reason: format!(
                        "challenge page detected ({}) on HTTP {status}",
                        marker.label()
                    ),
                }
            }
            Ok(false) => continue,
            Err(error) => return error,
        }
    }
    for marker in raw_markers {
        if body.contains(marker) {
            return EngineError::Blocked {
                engine,
                reason: format!("challenge page detected ({marker}) on HTTP {status}"),
            };
        }
    }
    EngineError::BadStatus { engine, status }
}

/// Read a search response body for any status, so a challenge served with a
/// non-success status is classified as blocked instead of a transport failure.
pub(crate) async fn read_search_body(
    resp: Response,
    engine: &'static str,
    shape: &ScrapeShape,
    max_bytes: usize,
    raw_markers: &[&'static str],
) -> Result<String, EngineError> {
    let status = resp.status().as_u16();
    let bytes = read_bounded_body(resp, engine, max_bytes).await?;
    let body = String::from_utf8_lossy(&bytes).into_owned();
    if (200..300).contains(&status) {
        Ok(body)
    } else {
        Err(classify_search_failure(
            engine,
            status,
            &body,
            shape,
            raw_markers,
        ))
    }
}
