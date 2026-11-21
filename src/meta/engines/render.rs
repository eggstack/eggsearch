use std::sync::Arc;
use std::time::Duration;

use super::error::EngineError;
use super::models::SearchResult;
use super::BoxFuture;

/// A provider result page as the shipped browser rendered it.
pub struct RenderedSerp {
    /// Rendered DOM the engine's own parser consumes.
    pub html: String,
    /// Where the render finished, after any same-origin redirect.
    pub final_url: String,
    /// Whether the render got past a detected wall.
    pub challenge_completed: bool,
}

/// Why a bounded render could not produce a page.
#[derive(Debug)]
pub enum SerpRenderError {
    /// No browser is available in this build or on this host.
    Unavailable,
    /// The render did not finish inside the caller's budget.
    TimedOut,
    /// The render ran and failed.
    Failed(String),
}

impl std::fmt::Display for SerpRenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unavailable => write!(f, "browser render unavailable"),
            Self::TimedOut => write!(f, "browser render exceeded its budget"),
            Self::Failed(reason) => write!(f, "browser render failed: {reason}"),
        }
    }
}

/// Bounded browser rendering for one provider result page.
///
/// The contract lives in `meta` so engines never import the browser module,
/// and the implementation lives above `meta` and `fetch` so `fetch` stays
/// independent of `meta`. An engine holds it only when a browser was
/// discovered and enabled.
pub trait SerpRenderer: Send + Sync {
    /// Render `url` and return the page the browser ended on, within
    /// `budget`.
    fn render<'a>(
        &'a self,
        url: &'a str,
        budget: Duration,
    ) -> BoxFuture<'a, Result<RenderedSerp, SerpRenderError>>;
}

/// Whether a failed HTTP attempt is one a browser may serve instead.
///
/// A recognised challenge always qualifies. So does the status class these
/// origins answer an automated client with: Yahoo replies `500` with an empty
/// body to a non-browser identity, which carries no marker to classify.
pub(crate) fn is_browser_gated(error: &EngineError) -> bool {
    match error {
        EngineError::Blocked { .. } => true,
        EngineError::BadStatus { status, .. } => {
            crate::fetch::challenge::is_browser_gated_status(*status)
        }
        _ => false,
    }
}

/// Serve a blocked attempt from the browser, keeping the HTTP reason when the
/// render cannot.
pub(crate) async fn render_blocked(
    renderer: &Arc<dyn SerpRenderer>,
    url: &str,
    budget: Duration,
    blocked: EngineError,
    max_results: usize,
    parse: impl Fn(&str, usize) -> Result<Vec<SearchResult>, EngineError>,
) -> Result<Vec<SearchResult>, EngineError> {
    match renderer.render(url, budget).await {
        Ok(rendered) => {
            tracing::debug!(
                url = %rendered.final_url,
                challenge_completed = rendered.challenge_completed,
                "provider result page served by browser render"
            );
            parse(&rendered.html, max_results)
        }
        Err(error) => {
            tracing::debug!(%error, "browser render could not serve the provider result page");
            Err(match blocked {
                EngineError::Blocked { engine, reason } => EngineError::Blocked {
                    engine,
                    reason: format!("{reason}; {error}"),
                },
                other => other,
            })
        }
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    /// A renderer that serves one fixed page, or reports itself unavailable.
    pub(crate) struct FixtureRenderer {
        html: Option<String>,
        calls: AtomicUsize,
    }

    impl FixtureRenderer {
        /// A renderer that serves `html` and records each call.
        pub(crate) fn serving(html: &str) -> Arc<Self> {
            Arc::new(Self {
                html: Some(html.to_string()),
                calls: AtomicUsize::new(0),
            })
        }

        /// A renderer that has nothing to offer.
        pub(crate) fn unavailable() -> Arc<Self> {
            Arc::new(Self {
                html: None,
                calls: AtomicUsize::new(0),
            })
        }

        /// How many renders were requested.
        pub(crate) fn calls(&self) -> usize {
            self.calls.load(Ordering::Relaxed)
        }
    }

    impl SerpRenderer for FixtureRenderer {
        fn render<'a>(
            &'a self,
            _url: &'a str,
            _budget: Duration,
        ) -> BoxFuture<'a, Result<RenderedSerp, SerpRenderError>> {
            Box::pin(async move {
                self.calls.fetch_add(1, Ordering::Relaxed);
                match &self.html {
                    Some(html) => Ok(RenderedSerp {
                        html: html.clone(),
                        final_url: "https://example.test/search".to_string(),
                        challenge_completed: true,
                    }),
                    None => Err(SerpRenderError::Unavailable),
                }
            })
        }
    }
}
