//! Browser escalation at the gated-status error seam.
//!
//! `src/fetch/client.rs` rejects every non-`2xx` status with
//! `FetchError::HttpStatus` before a response exists to classify, so
//! wall-shaped classification alone can never fire for the status class these
//! origins serve an automated client. This module owns the render and the
//! return rule at that seam; `web_fetch` keeps its permits, profile, and
//! screenshot handling.

use super::common::run_browser_fetch;
use crate::core::fetch::ExtractMode;
use crate::fetch::browser::{
    browser_result_to_response, rendered_result_serves, BrowserFetchError, RenderPolicy,
};
use crate::fetch::budget::FetchBudgets;
use crate::fetch::origin::OriginKey;
use crate::fetch::FetchError;
use crate::mcp::state::ServerState;

/// Everything one gated-status escalation needs.
pub(crate) struct GatedEscalationRequest<'a> {
    /// Server state owning the shared browser lifecycle and origin policy.
    pub(crate) state: &'a ServerState,
    /// The requested URL, as the caller resolved it.
    pub(crate) url: &'a str,
    /// The status the HTTP attempt was answered with.
    pub(crate) status: u16,
    /// Whether rendered output is sanitized like every other fetch.
    pub(crate) sanitize_output: bool,
    /// The policy in effect, which is `Auto` for every caller of this module.
    pub(crate) render_policy: &'a RenderPolicy,
    /// A persistent profile's data directory, when one is bound.
    pub(crate) profile_dir: Option<&'a std::path::Path>,
    /// Whether a render can run at all.
    pub(crate) browser_available: bool,
    /// The profile display name, used only to word the next action.
    pub(crate) used_profile_name: Option<&'a str>,
    /// The origin the HTTP attempt was charged against.
    pub(crate) origin_key: &'a OriginKey,
    /// Character budgets for the rendered document.
    pub(crate) budgets: FetchBudgets,
    /// Extraction mode for the rendered document.
    pub(crate) extract_mode: ExtractMode,
    /// Whether the rendered document extracts links.
    pub(crate) include_links: bool,
    /// The call's deadline.
    pub(crate) deadline: std::time::Instant,
}

/// What one gated-status escalation produced.
pub(crate) enum GatedEscalation {
    /// A rendered page that replaces the status error.
    Served(Box<crate::core::fetch::WebFetchResponse>),
    /// The status error, annotated with what the escalation did or why it
    /// could not run.
    Failed {
        /// The error the caller reports.
        error: FetchError,
        /// Whether the failure was an unavailable render rather than a render
        /// that ran and did not improve on the status.
        escalation_unavailable: bool,
    },
    /// The page needs a human; the caller raises the manual-interaction error.
    ManualInteraction {
        /// Origin that demands interaction.
        origin: String,
        /// Why interaction is required.
        message: String,
        /// The command that would let the operator sign in.
        next_action: Option<String>,
    },
}

/// The gated status an error carries, when a browser may serve it under `auto`.
///
/// `None` for every other policy and for every status outside the gated class,
/// so a refusal or an authentication demand is never a reason to render.
pub(crate) fn gated_status(error: &FetchError, policy: &RenderPolicy) -> Option<u16> {
    if !matches!(policy, RenderPolicy::Auto) {
        return None;
    }
    match error {
        FetchError::HttpStatus(status, _)
            if crate::fetch::challenge::is_browser_gated_status(*status) =>
        {
            Some(*status)
        }
        _ => None,
    }
}

/// Render one gated status and apply the shared return rule.
pub(crate) async fn escalate_gated_status(request: GatedEscalationRequest<'_>) -> GatedEscalation {
    if !request.browser_available {
        return unavailable(request.status, "no browser is available");
    }
    let remaining = request
        .deadline
        .saturating_duration_since(std::time::Instant::now());
    if remaining.as_millis() < 2000 {
        return unavailable(
            request.status,
            "less than 2000ms of the call budget remained",
        );
    }

    let rendered = match tokio::time::timeout(
        remaining,
        run_browser_fetch(
            request.state,
            request.url,
            request.sanitize_output,
            request.render_policy,
            request.profile_dir,
            false,
        ),
    )
    .await
    {
        Ok(Ok(outcome)) => outcome,
        Ok(Err(BrowserFetchError::InteractiveChallenge(required))) => {
            let next_action = Some(match request.used_profile_name {
                Some(profile) => format!(
                    "eggsearch browser-login {} --profile {profile}",
                    required.origin
                ),
                None => format!(
                    "eggsearch browser-login {} --profile <name>",
                    required.origin
                ),
            });
            return GatedEscalation::ManualInteraction {
                origin: required.origin,
                message: required.message,
                next_action,
            };
        }
        Ok(Err(error)) => {
            return GatedEscalation::Failed {
                error: FetchError::HttpStatus(
                    request.status,
                    format!("browser escalation failed: {error}"),
                ),
                escalation_unavailable: false,
            };
        }
        Err(_) => {
            return GatedEscalation::Failed {
                error: FetchError::HttpStatus(
                    request.status,
                    "browser escalation exceeded the call budget".to_string(),
                ),
                escalation_unavailable: false,
            };
        }
    };

    let rendered_status = rendered.result.response.status;
    if !rendered_result_serves(rendered_status, rendered.result.challenge_completed) {
        return GatedEscalation::Failed {
            error: FetchError::HttpStatus(
                request.status,
                format!(
                    "browser render ended on {} without clearing the challenge",
                    rendered_status.map_or_else(
                        || "no observed status".to_string(),
                        |status| format!("HTTP {status}")
                    )
                ),
            ),
            escalation_unavailable: false,
        };
    }

    if rendered.result.challenge_completed {
        crate::challenge_replay::record_browser_completion();
    }
    if let Some(controller) = request.state.origin_controller.as_ref() {
        controller.record_success(request.origin_key).await;
    }
    let mut response = browser_result_to_response(
        rendered.result,
        request.url,
        request.budgets,
        request.extract_mode,
        request.include_links,
        request.sanitize_output,
    );
    response.browser_escalated = true;
    GatedEscalation::Served(Box::new(response))
}

fn unavailable(status: u16, reason: &str) -> GatedEscalation {
    GatedEscalation::Failed {
        error: FetchError::HttpStatus(status, format!("browser escalation unavailable: {reason}")),
        escalation_unavailable: true,
    }
}
