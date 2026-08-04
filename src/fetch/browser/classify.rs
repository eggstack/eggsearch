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

pub fn classify_response(
    status: u16,
    content_type: Option<&str>,
    title: Option<&str>,
    text_len: usize,
    body_snippet: &[u8],
) -> FetchDisposition {
    match status {
        401 => return FetchDisposition::AuthenticationRequired,
        403 => return FetchDisposition::AccessDenied,
        404 => return FetchDisposition::AccessDenied,
        429 => return FetchDisposition::RateLimited,
        500..=599 => return FetchDisposition::ServerError,
        _ => {}
    }

    let ct = content_type.unwrap_or("");
    if !ct.contains("text/html") && !ct.is_empty() {
        return FetchDisposition::UsefulContent;
    }

    let title_lower = title.unwrap_or("").to_lowercase();
    let body_str = String::from_utf8_lossy(body_snippet);

    if is_interactive_challenge(&title_lower, &body_str) {
        return FetchDisposition::InteractiveChallenge;
    }

    if is_noninteractive_verification(&title_lower, &body_str) {
        return FetchDisposition::NonInteractiveVerification;
    }

    if is_javascript_shell(&title_lower, text_len, &body_str) {
        return FetchDisposition::JavascriptShell;
    }

    FetchDisposition::UsefulContent
}

fn is_interactive_challenge(title_lower: &str, body: &str) -> bool {
    if title_lower.contains("access denied")
        || title_lower.contains("security check")
        || title_lower.contains("verify you are human")
    {
        return true;
    }

    let indicators = [
        "cf-turnstile",
        "captcha",
        "challenge-platform",
        "g-recaptcha",
        "h-captcha",
        "interactive challenge",
        "verify you are human",
    ];
    for indicator in &indicators {
        if body.to_lowercase().contains(indicator) {
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
        if title_lower.contains(marker) || body.to_lowercase().contains(marker) {
            return true;
        }
    }
    false
}

fn is_javascript_shell(_title_lower: &str, text_len: usize, body: &str) -> bool {
    if text_len < 50 && body.contains("<div id=\"root\"")
        || body.contains("<div id=\"app\"")
        || body.contains("<div id=\"__next\"")
    {
        return true;
    }

    if text_len < 100 {
        let script_count = body.matches("<script").count();
        if script_count >= 3 && body.contains("<body") && text_len < 50 {
            return true;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
