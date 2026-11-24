use super::common::run_browser_fetch;
use crate::core::fetch::ExtractMode;
use crate::fetch::browser::{escalate, EscalationOutcome, EscalationRequest, RenderPolicy};
use crate::fetch::budget::FetchBudgets;
use crate::fetch::origin::OriginKey;
use crate::fetch::FetchError;
use crate::mcp::state::ServerState;

pub(crate) struct GatedEscalationRequest<'a> {
    pub(crate) state: &'a ServerState,
    pub(crate) url: &'a str,
    pub(crate) status: u16,
    pub(crate) sanitize_output: bool,
    pub(crate) render_policy: &'a RenderPolicy,
    pub(crate) profile_dir: Option<&'a std::path::Path>,
    pub(crate) browser_available: bool,
    pub(crate) used_profile_name: Option<&'a str>,
    pub(crate) origin_key: &'a OriginKey,
    pub(crate) budgets: FetchBudgets,
    pub(crate) extract_mode: ExtractMode,
    pub(crate) include_links: bool,
    pub(crate) deadline: std::time::Instant,
}

pub(crate) enum GatedEscalation {
    Served(Box<crate::core::fetch::WebFetchResponse>),
    Failed {
        error: FetchError,
        escalation_unavailable: bool,
    },
    ManualInteraction {
        origin: String,
        message: String,
        next_action: Option<String>,
    },
}

pub(crate) async fn escalate_gated_status(request: GatedEscalationRequest<'_>) -> GatedEscalation {
    let GatedEscalationRequest {
        state,
        url,
        status,
        sanitize_output,
        render_policy,
        profile_dir,
        browser_available,
        used_profile_name,
        origin_key,
        budgets,
        extract_mode,
        include_links,
        deadline,
    } = request;

    let unavailable_reason = if !state.config.fetch.browser.enabled {
        Some("browser rendering is disabled in config".to_string())
    } else {
        crate::fetch::browser::browser_discovery_reason(&state.browser_discovery_state)
    }
    .or_else(|| Some("browser rendering is unavailable".to_string()));

    match escalate(
        EscalationRequest {
            url,
            status,
            policy: render_policy,
            browser_available,
            unavailable_reason: unavailable_reason.as_deref(),
            profile_name: used_profile_name,
            budgets,
            extract_mode,
            include_links,
            sanitize_output,
            deadline,
        },
        || {
            run_browser_fetch(
                state,
                url,
                sanitize_output,
                render_policy,
                profile_dir,
                false,
            )
        },
    )
    .await
    {
        EscalationOutcome::Served {
            mut response,
            challenge_completed,
        } => {
            if challenge_completed {
                crate::challenge_replay::record_browser_completion();
            }
            if let Some(controller) = state.origin_controller.as_ref() {
                controller.record_success(origin_key).await;
            }
            response.browser_escalated = true;
            GatedEscalation::Served(response)
        }
        EscalationOutcome::Failed {
            error,
            escalation_unavailable,
        } => GatedEscalation::Failed {
            error,
            escalation_unavailable,
        },
        EscalationOutcome::ManualInteraction {
            origin,
            message,
            next_action,
        } => GatedEscalation::ManualInteraction {
            origin,
            message,
            next_action,
        },
    }
}
