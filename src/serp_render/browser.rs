//! The shipped-browser implementation of [`SerpRenderer`].

use std::sync::Arc;
use std::time::Duration;

use crate::fetch::browser::{
    browser_fetch_with_policy, BrowserConfig, BrowserFetchError, BrowserLifecycle, RenderPolicy,
};
use crate::meta::engines::render::{RenderedSerp, SerpRenderError, SerpRenderer};
use crate::meta::engines::BoxFuture;

/// Renders a provider result page in the shared browser lifecycle.
pub struct BrowserSerpRenderer {
    lifecycle: Arc<BrowserLifecycle>,
    config: BrowserConfig,
}

impl BrowserSerpRenderer {
    /// Build a renderer over the process-wide browser lifecycle.
    #[must_use]
    pub fn new(lifecycle: Arc<BrowserLifecycle>, config: BrowserConfig) -> Self {
        Self { lifecycle, config }
    }

    /// The lifecycle's own waits, clamped to the caller's budget so the render
    /// cannot outlive the search that asked for it.
    fn budgeted(config: &BrowserConfig, budget: Duration) -> BrowserConfig {
        let budget_ms = u64::try_from(budget.as_millis()).unwrap_or(u64::MAX).max(1);
        let mut config = config.clone();
        config.navigation_timeout_ms = config.navigation_timeout_ms.min(budget_ms);
        config.verification_wait_ms = config.verification_wait_ms.min(budget_ms);
        config
    }
}

impl SerpRenderer for BrowserSerpRenderer {
    fn render<'a>(
        &'a self,
        url: &'a str,
        budget: Duration,
    ) -> BoxFuture<'a, Result<RenderedSerp, SerpRenderError>> {
        Box::pin(async move {
            let config = Self::budgeted(&self.config, budget);
            let render = browser_fetch_with_policy(
                &self.lifecycle,
                url,
                &config,
                false,
                &RenderPolicy::Browser,
            );
            let result = tokio::time::timeout(budget, render)
                .await
                .map_err(|_| SerpRenderError::TimedOut)?
                .map_err(classify_error)?;
            Ok(RenderedSerp {
                html: String::from_utf8_lossy(&result.response.body).into_owned(),
                final_url: result.response.final_url,
                challenge_completed: result.challenge_completed,
            })
        })
    }
}

fn classify_error(error: BrowserFetchError) -> SerpRenderError {
    match error {
        BrowserFetchError::InteractiveChallenge(required) => {
            SerpRenderError::Failed(format!("interactive challenge at {}", required.origin))
        }
        BrowserFetchError::HttpOnly => SerpRenderError::Unavailable,
        other => SerpRenderError::Failed(other.to_string()),
    }
}
