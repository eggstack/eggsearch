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
    /// Whether a rendered page is still a script-driven wall.
    ///
    /// The two dispositions a browser render waits on and reports when it cannot
    /// get past them. A detected challenge family is a wall whatever its
    /// classification says, so callers check detection as well.
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

        // A wall marker outranks the document status. Origins serve their
        // challenge with a non-success status (Brave answers 429 with its
        // proof-of-work page), so a status-first classification would settle
        // the render before the page's own script has had a chance to run.
        if is_interactive_challenge(&title_lower, &body_lower) {
            return FetchDisposition::InteractiveChallenge;
        }
        if is_noninteractive_verification(&title_lower, &body_lower) {
            return FetchDisposition::NonInteractiveVerification;
        }
        if is_javascript_shell(&title_lower, text_len, &body_lower) {
            return FetchDisposition::JavascriptShell;
        }
        // After client-side rendering the DOM is authoritative: an
        // interstitial status whose document now carries a real page is a
        // served page. The rule is limited to the two statuses an
        // interstitial is actually served with, so a denial or an outage is
        // still reported as such.
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

/// Whether a rendered page replaces the status error that caused the render.
///
/// A render is accepted when it ended on a success or redirect status, or when
/// it completed a recognised wall. A `5xx` page the browser only re-rendered is
/// still the origin's outage, so the caller keeps its original error rather
/// than returning an error page as a document.
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

    // Widget and endpoint markers only. A bare "captcha" substring also
    // matches a page that merely *offers* a traditional CAPTCHA beside its
    // scripted challenge (Brave's proof-of-work page does), and treating that
    // as interactive would refuse a page the origin serves to a browser.
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

#[cfg(test)]
mod tests {
    use super::*;

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
