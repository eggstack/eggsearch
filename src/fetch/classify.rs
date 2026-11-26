#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FetchDisposition {
    UsefulContent,
    JavascriptShell,
    NonInteractiveVerification,
    InteractiveChallenge,
    RateLimited,
    AccessDenied,
    AuthenticationRequired,
    ServerError,
    Unsupported,
}

impl FetchDisposition {
    /// Whether the browser should wait for a scripted wall to clear.
    #[must_use]
    pub fn is_script_wall(&self) -> bool {
        matches!(
            self,
            Self::JavascriptShell | Self::NonInteractiveVerification
        )
    }
}

/// Rendered text below this length never overrides an interstitial status.
const RENDERED_CONTENT_MIN_CHARS: usize = 200;

/// Longest page that may be classified by an explicit gate instruction.
const JAVASCRIPT_GATE_MAX_CHARS: usize = 200;

/// Instructions a short page uses when it needs scripting to render.
const JAVASCRIPT_GATE_MARKERS: [&str; 4] = [
    "enable javascript",
    "javascript is required",
    "requires javascript",
    "use a headless browser",
];

pub fn classify_response(
    status: u16,
    content_type: Option<&str>,
    title: Option<&str>,
    text_len: usize,
    body_snippet: &[u8],
) -> FetchDisposition {
    if status == 401 {
        return FetchDisposition::AuthenticationRequired;
    }

    let ct = content_type.unwrap_or("");
    let is_html = ct.is_empty() || ct.contains("text/html");
    if is_html {
        let title_lower = title.unwrap_or("").to_lowercase();
        let body_lower = String::from_utf8_lossy(body_snippet).to_lowercase();

        // Classify challenge markers before status; some solvable walls arrive as 429.
        if is_interactive_challenge(&title_lower, &body_lower) {
            return FetchDisposition::InteractiveChallenge;
        }
        if is_noninteractive_verification(&title_lower, &body_lower) {
            return FetchDisposition::NonInteractiveVerification;
        }
        if is_javascript_shell(&title_lower, text_len, &body_lower) {
            return FetchDisposition::JavascriptShell;
        }
        // After rendering, accept substantial content only for 202/429; other failures remain failures.
        if matches!(status, 202 | 429) && text_len >= RENDERED_CONTENT_MIN_CHARS {
            return FetchDisposition::UsefulContent;
        }
    }

    match status {
        403 | 404 => FetchDisposition::AccessDenied,
        429 => FetchDisposition::RateLimited,
        500..=599 => FetchDisposition::ServerError,
        _ => FetchDisposition::UsefulContent,
    }
}

/// Accept rendered content only on success/redirect or after a recognized wall completes.
#[must_use]
pub fn rendered_result_serves(status: Option<u16>, challenge_completed: bool) -> bool {
    challenge_completed || status.is_some_and(|status| (200..400).contains(&status))
}

fn is_interactive_challenge(title_lower: &str, body: &str) -> bool {
    if title_lower.contains("access denied")
        || title_lower.contains("security check")
        || title_lower.contains("verify you are human")
    {
        return true;
    }

    // Match explicit widget markers; generic "captcha" text may accompany a solvable wall.
    let indicators = [
        "cf-turnstile",
        "challenge-platform",
        "g-recaptcha",
        "h-captcha",
        "hcaptcha.com",
        "recaptcha/api",
        "interactive challenge",
        "verify you are human",
    ];
    for indicator in &indicators {
        if body.contains(indicator) {
            return true;
        }
    }

    false
}

fn is_noninteractive_verification(title_lower: &str, body: &str) -> bool {
    let markers = [
        "just a moment",
        "checking your browser",
        "please wait",
        "verifying",
        "security verification",
    ];
    for marker in &markers {
        if title_lower.contains(marker) || body.contains(marker) {
            return true;
        }
    }
    false
}

fn is_javascript_shell(_title_lower: &str, text_len: usize, body: &str) -> bool {
    let body_lower = body.to_lowercase();

    if text_len < JAVASCRIPT_GATE_MAX_CHARS
        && JAVASCRIPT_GATE_MARKERS
            .iter()
            .any(|marker| body_lower.contains(marker))
    {
        return true;
    }

    if text_len < 50
        && (body_lower.contains("<div id=\"root\"")
            || body_lower.contains("<div id=\"app\"")
            || body_lower.contains("<div id=\"__next\""))
    {
        return true;
    }

    let script_count = body_lower.matches("<script").count();
    let tag_count = body_lower.matches('<').count();

    if tag_count > 0 {
        let ratio = script_count as f64 / tag_count as f64;
        if ratio > 0.3 && text_len < 100 && body_lower.contains("<body") {
            return true;
        }
    }

    if text_len < 50 && script_count >= 3 && body_lower.contains("<body") {
        return true;
    }

    false
}

pub fn browser_rendering_is_warranted(
    status: u16,
    content_type: Option<&str>,
    title: Option<&str>,
    text_len: usize,
    body: &[u8],
) -> bool {
    let body_text = String::from_utf8_lossy(body);
    crate::fetch::challenge::detect(title, &body_text).is_some()
        || classify_response(status, content_type, title, text_len, body).is_script_wall()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browser_warrant_classification_is_feature_independent() {
        assert!(browser_rendering_is_warranted(
            200,
            Some("text/html"),
            Some("Checking your browser"),
            10,
            b"<html><body>Please wait</body></html>",
        ));
        assert!(browser_rendering_is_warranted(
            200,
            Some("text/html"),
            Some("App"),
            10,
            br#"<html><body><div id="root"></div><script src="a.js"></script><script src="b.js"></script><script src="c.js"></script></body></html>"#,
        ));
        assert!(!browser_rendering_is_warranted(
            200,
            Some("text/html"),
            Some("Article"),
            500,
            b"<html><body>Ordinary readable article content.</body></html>",
        ));
    }

    #[test]
    fn only_the_two_script_wall_dispositions_wait_for_completion() {
        assert!(FetchDisposition::JavascriptShell.is_script_wall());
        assert!(FetchDisposition::NonInteractiveVerification.is_script_wall());
        for settled in [
            FetchDisposition::UsefulContent,
            FetchDisposition::InteractiveChallenge,
            FetchDisposition::RateLimited,
            FetchDisposition::AccessDenied,
            FetchDisposition::AuthenticationRequired,
            FetchDisposition::ServerError,
            FetchDisposition::Unsupported,
        ] {
            assert!(!settled.is_script_wall(), "{settled:?} must not wait");
        }
    }

    #[test]
    fn a_render_serves_only_when_it_improved_on_the_status() {
        assert!(rendered_result_serves(Some(200), false));
        assert!(rendered_result_serves(Some(302), false));
        assert!(
            rendered_result_serves(Some(503), true),
            "a completed wall is served whatever the document status says"
        );
        assert!(
            !rendered_result_serves(Some(500), false),
            "an error page the browser only re-rendered is not the document"
        );
        assert!(!rendered_result_serves(None, false));
        assert!(
            rendered_result_serves(None, true),
            "a completed wall is served even without an observed status"
        );
    }

    #[test]
    fn useful_content() {
        assert_eq!(
            classify_response(
                200,
                Some("text/html"),
                Some("My Page"),
                500,
                b"<html><body>Hello world</body></html>"
            ),
            FetchDisposition::UsefulContent
        );
    }

    #[test]
    fn http_401_returns_auth_required() {
        assert_eq!(
            classify_response(401, Some("text/html"), None, 0, b""),
            FetchDisposition::AuthenticationRequired
        );
    }

    #[test]
    fn http_403_returns_access_denied() {
        assert_eq!(
            classify_response(403, Some("text/html"), None, 0, b""),
            FetchDisposition::AccessDenied
        );
    }

    #[test]
    fn http_429_returns_rate_limited() {
        assert_eq!(
            classify_response(429, Some("text/html"), None, 0, b""),
            FetchDisposition::RateLimited
        );
    }

    #[test]
    fn http_500_returns_server_error() {
        assert_eq!(
            classify_response(500, Some("text/html"), None, 0, b""),
            FetchDisposition::ServerError
        );
    }

    #[test]
    fn interstitial_status_with_a_rendered_page_is_useful_content() {
        assert_eq!(
            classify_response(
                429,
                Some("text/html"),
                Some("rust programming language - Brave Search"),
                12000,
                b"<html><body><div data-type=\"web\">results</div></body></html>"
            ),
            FetchDisposition::UsefulContent
        );
    }

    #[test]
    fn a_short_interstitial_document_keeps_its_status() {
        assert_eq!(
            classify_response(
                429,
                Some("text/html"),
                Some("Brave Search"),
                26,
                b"<html></html>"
            ),
            FetchDisposition::RateLimited
        );
        assert_eq!(
            classify_response(
                500,
                Some("text/html"),
                Some("Server"),
                900,
                b"<html></html>"
            ),
            FetchDisposition::ServerError
        );
        assert_eq!(
            classify_response(
                403,
                Some("text/html"),
                Some("Denied"),
                900,
                b"<html></html>"
            ),
            FetchDisposition::AccessDenied
        );
    }

    #[test]
    fn scripted_wall_beats_its_status() {
        let body = b"<html><body>Verifying you're not a bot. Quick check before you continue searching.</body></html>";
        assert_eq!(
            classify_response(429, Some("text/html"), Some("Brave Search"), 64, body),
            FetchDisposition::NonInteractiveVerification
        );
    }

    #[test]
    fn offering_a_traditional_captcha_is_not_an_interactive_challenge() {
        let body = b"<html><body><button>Verify</button><a>Switch to traditional CAPTCHA</a></body></html>";
        assert_eq!(
            classify_response(429, Some("text/html"), Some("Brave Search"), 500, body),
            FetchDisposition::UsefulContent
        );
    }

    #[test]
    fn non_html_returns_useful() {
        assert_eq!(
            classify_response(200, Some("application/json"), None, 0, b"{}"),
            FetchDisposition::UsefulContent
        );
    }

    #[test]
    fn interactive_challenge_detected() {
        assert_eq!(
            classify_response(
                200,
                Some("text/html"),
                Some("Access Denied"),
                200,
                b"<html><body>Access Denied</body></html>"
            ),
            FetchDisposition::InteractiveChallenge
        );
    }

    #[test]
    fn captcha_body_marker() {
        assert_eq!(
            classify_response(
                200,
                Some("text/html"),
                Some("Just a moment"),
                300,
                b"<html><body><div class='cf-turnstile'></div></body></html>"
            ),
            FetchDisposition::InteractiveChallenge
        );
    }

    #[test]
    fn noninteractive_verification_detected() {
        assert_eq!(
            classify_response(
                200,
                Some("text/html"),
                Some("Just a moment..."),
                200,
                b"<html><body>Please wait while we verify your browser</body></html>"
            ),
            FetchDisposition::NonInteractiveVerification
        );
    }

    #[test]
    fn js_shell_empty_root() {
        let body = r#"<html><head><title>App</title></head><body><div id="root"></div><script src="app.js"></script><script src="chunk.js"></script><script src="vendor.js"></script></body></html>"#;
        assert_eq!(
            classify_response(200, Some("text/html"), Some("App"), 20, body.as_bytes()),
            FetchDisposition::JavascriptShell
        );
    }

    #[test]
    fn js_shell_high_script_density() {
        let body = r#"<html><body><script></script><script></script><script></script><div></div></body></html>"#;
        assert_eq!(
            classify_response(200, Some("text/html"), Some("App"), 10, body.as_bytes()),
            FetchDisposition::JavascriptShell
        );
    }

    #[test]
    fn a_short_gate_instruction_is_a_javascript_shell() {
        let body = br#"<html><head><title>JavaScript required</title></head><body><p>Either use a headless browser or enable JavaScript to continue.</p></body></html>"#;
        assert_eq!(
            classify_response(
                200,
                Some("text/html"),
                Some("JavaScript required"),
                66,
                body
            ),
            FetchDisposition::JavascriptShell
        );
    }

    #[test]
    fn a_long_page_that_mentions_javascript_stays_useful() {
        let text = "This guide explains how JavaScript runs in the browser. ".repeat(8);
        let body = format!("<html><body><p>{text}</p></body></html>");
        assert_eq!(
            classify_response(
                200,
                Some("text/html"),
                Some("Guide"),
                text.chars().count(),
                body.as_bytes()
            ),
            FetchDisposition::UsefulContent
        );
    }

    #[test]
    fn an_interactive_challenge_keeps_precedence_over_a_gate_instruction() {
        let body = br#"<html><body><div class="cf-turnstile"></div><p>Enable JavaScript to continue.</p></body></html>"#;
        assert_eq!(
            classify_response(200, Some("text/html"), None, 32, body),
            FetchDisposition::InteractiveChallenge
        );
    }

    #[test]
    fn useful_content_with_scripts_and_text() {
        let body = r#"<html><head><script src="a.js"></script></head><body><p>This is a real page with lots of text content that makes it useful.</p></body></html>"#;
        assert_eq!(
            classify_response(
                200,
                Some("text/html"),
                Some("Article"),
                500,
                body.as_bytes()
            ),
            FetchDisposition::UsefulContent
        );
    }
}
