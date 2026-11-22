use std::future::Future;

use crate::core::fetch::{ExtractMode, WebFetchResponse};
use crate::fetch::browser::{
    browser_result_to_response, rendered_result_serves, BrowserFetchError, BrowserFetchOutcome,
    RenderPolicy,
};
use crate::fetch::budget::FetchBudgets;
use crate::fetch::FetchError;

pub struct EscalationRequest<'a> {
    pub url: &'a str,
    pub status: u16,
    pub policy: &'a RenderPolicy,
    pub browser_available: bool,
    pub unavailable_reason: Option<&'a str>,
    pub profile_name: Option<&'a str>,
    pub budgets: FetchBudgets,
    pub extract_mode: ExtractMode,
    pub include_links: bool,
    pub sanitize_output: bool,
    pub deadline: std::time::Instant,
}

pub enum EscalationOutcome {
    Served {
        response: Box<WebFetchResponse>,
        challenge_completed: bool,
    },
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

pub fn gated_status(error: &FetchError, policy: &RenderPolicy) -> Option<u16> {
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

pub async fn escalate<F, Fut>(request: EscalationRequest<'_>, render: F) -> EscalationOutcome
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<BrowserFetchOutcome, BrowserFetchError>>,
{
    if !request.browser_available {
        return unavailable(
            request.status,
            request
                .unavailable_reason
                .unwrap_or("no browser is available"),
        );
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

    let rendered = match tokio::time::timeout(remaining, render()).await {
        Ok(Ok(outcome)) => outcome,
        Ok(Err(BrowserFetchError::InteractiveChallenge(required))) => {
            let next_action = Some(match request.profile_name {
                Some(profile) => format!(
                    "eggsearch browser-login {} --profile {profile}",
                    required.origin
                ),
                None => format!(
                    "eggsearch browser-login {} --profile <name>",
                    required.origin
                ),
            });
            return EscalationOutcome::ManualInteraction {
                origin: required.origin,
                message: required.message,
                next_action,
            };
        }
        Ok(Err(error)) => {
            return EscalationOutcome::Failed {
                error: FetchError::HttpStatus(
                    request.status,
                    format!("browser escalation failed: {error}"),
                ),
                escalation_unavailable: false,
            };
        }
        Err(_) => {
            return EscalationOutcome::Failed {
                error: FetchError::HttpStatus(
                    request.status,
                    "browser escalation exceeded the call budget".to_string(),
                ),
                escalation_unavailable: false,
            };
        }
    };

    let challenge_completed = rendered.result.challenge_completed;
    let rendered_status = rendered.result.response.status;
    if !rendered_result_serves(rendered_status, challenge_completed) {
        return EscalationOutcome::Failed {
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

    let response = browser_result_to_response(
        rendered.result,
        request.url,
        request.budgets,
        request.extract_mode,
        request.include_links,
        request.sanitize_output,
    );
    EscalationOutcome::Served {
        response: Box::new(response),
        challenge_completed,
    }
}

fn unavailable(status: u16, reason: &str) -> EscalationOutcome {
    EscalationOutcome::Failed {
        error: FetchError::HttpStatus(status, format!("browser escalation unavailable: {reason}")),
        escalation_unavailable: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::sanitize::TrustMarkers;
    use crate::fetch::browser::{
        BrowserFetchResult, FetchDisposition, FetchTransportKind, ManualInteractionReason,
        ManualInteractionRequired, TransportResponse, TransportTiming,
    };
    use std::time::{Duration, Instant};

    static AUTO_POLICY: RenderPolicy = RenderPolicy::Auto;

    fn request<'a>(
        browser_available: bool,
        profile_name: Option<&'a str>,
        deadline: Instant,
    ) -> EscalationRequest<'a> {
        EscalationRequest {
            url: "https://example.com",
            status: 429,
            policy: &AUTO_POLICY,
            browser_available,
            unavailable_reason: None,
            profile_name,
            budgets: FetchBudgets::new(200, 200),
            extract_mode: ExtractMode::Text,
            include_links: false,
            sanitize_output: false,
            deadline,
        }
    }

    fn rendered(status: Option<u16>, challenge_completed: bool) -> BrowserFetchOutcome {
        BrowserFetchOutcome {
            result: BrowserFetchResult {
                response: TransportResponse {
                    transport: FetchTransportKind::Browser,
                    requested_url: "https://example.com".to_string(),
                    final_url: "https://example.com".to_string(),
                    status,
                    headers: Vec::new(),
                    body: b"<html><body>rendered content</body></html>".to_vec(),
                    content_type: Some("text/html".to_string()),
                    redirects: Vec::new(),
                    timing: TransportTiming::default(),
                    classification: Some(FetchDisposition::UsefulContent),
                },
                extracted_title: Some("Example".to_string()),
                extracted_text: "rendered content".to_string(),
                trust_markers: TrustMarkers::default(),
                warnings: Vec::new(),
                challenge_completed,
                challenge: None,
            },
            screenshot: None,
        }
    }

    fn interactive_challenge() -> BrowserFetchError {
        BrowserFetchError::InteractiveChallenge(ManualInteractionRequired {
            origin: "https://example.com".to_string(),
            reason: ManualInteractionReason::InteractiveChallenge,
            browser_profile_supported: true,
            message: "human verification required".to_string(),
        })
    }

    #[test]
    fn gated_status_requires_auto_and_a_gated_http_error() {
        assert_eq!(
            gated_status(
                &FetchError::HttpStatus(429, "blocked".to_string()),
                &AUTO_POLICY
            ),
            Some(429)
        );
        assert_eq!(
            gated_status(
                &FetchError::HttpStatus(403, "refused".to_string()),
                &AUTO_POLICY
            ),
            None
        );
        assert_eq!(
            gated_status(
                &FetchError::HttpStatus(429, "blocked".to_string()),
                &RenderPolicy::HttpOnly
            ),
            None
        );
    }

    #[tokio::test]
    async fn unavailable_browser_returns_unavailable_without_rendering() {
        let mut render_invoked = false;
        let outcome = escalate(
            request(false, None, Instant::now() + Duration::from_secs(5)),
            || {
                render_invoked = true;
                async { Ok(rendered(Some(200), false)) }
            },
        )
        .await;
        assert!(!render_invoked);
        match outcome {
            EscalationOutcome::Failed {
                error: FetchError::HttpStatus(429, message),
                escalation_unavailable,
            } => {
                assert!(escalation_unavailable);
                assert!(message.contains("browser escalation unavailable"));
            }
            _ => panic!("unexpected outcome"),
        }
    }

    #[tokio::test]
    async fn unavailable_reason_is_preserved_for_the_caller() {
        let mut request = request(false, None, Instant::now() + Duration::from_secs(5));
        request.unavailable_reason = Some("no Chrome/Chromium executable was found");
        let outcome = escalate(request, || async { Ok(rendered(Some(200), false)) }).await;
        match outcome {
            EscalationOutcome::Failed {
                error: FetchError::HttpStatus(429, message),
                escalation_unavailable,
            } => {
                assert!(escalation_unavailable);
                assert!(message.contains("no Chrome/Chromium executable was found"));
            }
            _ => panic!("unexpected outcome"),
        }
    }

    #[tokio::test]
    async fn remaining_time_below_two_seconds_skips_rendering() {
        let mut render_invoked = false;
        let outcome = escalate(
            request(true, None, Instant::now() + Duration::from_millis(1999)),
            || {
                render_invoked = true;
                async { Ok(rendered(Some(200), false)) }
            },
        )
        .await;
        assert!(!render_invoked);
        match outcome {
            EscalationOutcome::Failed {
                error: FetchError::HttpStatus(429, message),
                escalation_unavailable,
            } => {
                assert!(escalation_unavailable);
                assert!(message.contains("less than 2000ms"));
            }
            _ => panic!("unexpected outcome"),
        }
    }

    #[tokio::test]
    async fn render_timeout_maps_to_http_status_error() {
        let outcome = escalate(
            request(true, None, Instant::now() + Duration::from_millis(2500)),
            std::future::pending,
        )
        .await;
        match outcome {
            EscalationOutcome::Failed {
                error: FetchError::HttpStatus(429, message),
                escalation_unavailable,
            } => {
                assert!(!escalation_unavailable);
                assert!(message.contains("browser escalation exceeded the call budget"));
            }
            _ => panic!("unexpected outcome"),
        }
    }

    #[tokio::test]
    async fn interactive_challenge_names_selected_profile_in_next_action() {
        let outcome = escalate(
            request(true, Some("work"), Instant::now() + Duration::from_secs(5)),
            || async { Err(interactive_challenge()) },
        )
        .await;
        match outcome {
            EscalationOutcome::ManualInteraction {
                origin,
                message,
                next_action,
            } => {
                assert_eq!(origin, "https://example.com");
                assert_eq!(message, "human verification required");
                assert_eq!(
                    next_action.as_deref(),
                    Some("eggsearch browser-login https://example.com --profile work")
                );
            }
            _ => panic!("unexpected outcome"),
        }
    }

    #[tokio::test]
    async fn interactive_challenge_without_profile_uses_profile_placeholder() {
        let outcome = escalate(
            request(true, None, Instant::now() + Duration::from_secs(5)),
            || async { Err(interactive_challenge()) },
        )
        .await;
        match outcome {
            EscalationOutcome::ManualInteraction {
                origin,
                message,
                next_action,
            } => {
                assert_eq!(origin, "https://example.com");
                assert_eq!(message, "human verification required");
                assert_eq!(
                    next_action.as_deref(),
                    Some("eggsearch browser-login https://example.com --profile <name>")
                );
            }
            _ => panic!("unexpected outcome"),
        }
    }

    #[tokio::test]
    async fn served_render_replaces_the_gated_status() {
        let outcome = escalate(
            request(true, None, Instant::now() + Duration::from_secs(5)),
            || async { Ok(rendered(Some(200), false)) },
        )
        .await;
        match outcome {
            EscalationOutcome::Served {
                response,
                challenge_completed,
            } => {
                assert_eq!(response.transport.as_deref(), Some("browser"));
                assert!(!challenge_completed);
            }
            _ => panic!("unexpected outcome"),
        }
    }

    #[tokio::test]
    async fn unserved_render_keeps_the_original_status_error() {
        let outcome = escalate(
            request(true, None, Instant::now() + Duration::from_secs(5)),
            || async { Ok(rendered(Some(500), false)) },
        )
        .await;
        match outcome {
            EscalationOutcome::Failed {
                error: FetchError::HttpStatus(429, message),
                escalation_unavailable,
            } => {
                assert!(!escalation_unavailable);
                assert!(message.contains("browser render ended on HTTP 500"));
            }
            _ => panic!("unexpected outcome"),
        }
    }

    #[tokio::test]
    async fn served_outcome_surfaces_challenge_completion_for_the_caller() {
        let outcome = escalate(
            request(true, None, Instant::now() + Duration::from_secs(5)),
            || async { Ok(rendered(Some(503), true)) },
        )
        .await;
        match outcome {
            EscalationOutcome::Served {
                challenge_completed,
                ..
            } => assert!(challenge_completed),
            _ => panic!("unexpected outcome"),
        }
    }
}
