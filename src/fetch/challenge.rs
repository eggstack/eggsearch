//! Anti-bot challenge detection for plain HTTP fetches.

/// Anti-bot challenge family a fetched page appears to be.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChallengeKind {
    /// Anubis proof-of-work wall.
    Anubis,
    /// ALTCHA proof-of-work wall.
    Altcha,
    /// Interactive CAPTCHA or human-verification wall.
    Interactive,
    /// Non-interactive browser verification interstitial.
    Verification,
}

impl ChallengeKind {
    /// Stable lowercase family name used in warnings.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Anubis => "anubis",
            Self::Altcha => "altcha",
            Self::Interactive => "interactive",
            Self::Verification => "verification",
        }
    }
}

/// Detect a challenge page from its title and body.
#[must_use]
pub fn detect(title: Option<&str>, body: &str) -> Option<ChallengeKind> {
    let title = title.unwrap_or("").to_lowercase();
    let body = body.to_lowercase();
    if is_anubis(&body) {
        return Some(ChallengeKind::Anubis);
    }
    if is_altcha(&body) {
        return Some(ChallengeKind::Altcha);
    }
    if is_interactive(&title, &body) {
        return Some(ChallengeKind::Interactive);
    }
    if is_verification(&title, &body) {
        return Some(ChallengeKind::Verification);
    }
    None
}

/// Detect a challenge only for an HTML representation.
#[must_use]
pub fn detect_html(is_html: bool, title: Option<&str>, body: &[u8]) -> Option<ChallengeKind> {
    if !is_html {
        return None;
    }
    detect(title, &String::from_utf8_lossy(body))
}

/// Structured warning naming a detected challenge family, if any.
#[must_use]
pub fn warnings(challenge: Option<ChallengeKind>) -> Vec<crate::core::warning::AgentWarning> {
    challenge
        .map(|kind| {
            vec![crate::core::warning::AgentWarning::new(
                crate::core::warning::WarningCode::FetchChallengePage,
                format!(
                    "challenge page detected ({}): the response is an anti-bot challenge, not the requested document",
                    kind.as_str()
                ),
            )]
        })
        .unwrap_or_default()
}

fn is_anubis(body: &str) -> bool {
    const MARKERS: [&str; 4] = [
        "anubis_challenge",
        "anubis_version",
        "anubis_base_prefix",
        "anubis could not load its javascript",
    ];
    MARKERS.iter().any(|marker| body.contains(marker))
}

fn is_altcha(body: &str) -> bool {
    const MARKERS: [&str; 3] = [
        "captcha-wrap",
        "javascript is required to complete this challenge",
        "captcha/challenge",
    ];
    MARKERS.iter().any(|marker| body.contains(marker))
}

fn is_interactive(title: &str, body: &str) -> bool {
    if title.contains("access denied")
        || title.contains("security check")
        || title.contains("verify you are human")
    {
        return true;
    }
    const MARKERS: [&str; 6] = [
        "cf-turnstile",
        "g-recaptcha",
        "h-captcha",
        "challenge-platform",
        "verify you are human",
        "interactive challenge",
    ];
    MARKERS.iter().any(|marker| body.contains(marker))
}

fn is_verification(title: &str, body: &str) -> bool {
    const MARKERS: [&str; 4] = [
        "just a moment",
        "checking your browser",
        "verifying your browser",
        "security verification",
    ];
    MARKERS
        .iter()
        .any(|marker| title.contains(marker) || body.contains(marker))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_each_family() {
        assert_eq!(
            detect(
                None,
                r#"<script id="anubis_challenge" type="application/json">{}</script>"#
            ),
            Some(ChallengeKind::Anubis)
        );
        assert_eq!(
            detect(
                None,
                "Anubis could not load its JavaScript. The server may be overloaded."
            ),
            Some(ChallengeKind::Anubis)
        );
        assert_eq!(
            detect(None, r#"<div class="captcha-wrap">challenge</div>"#),
            Some(ChallengeKind::Altcha)
        );
        assert_eq!(
            detect(None, "JavaScript is required to complete this challenge"),
            Some(ChallengeKind::Altcha)
        );
        assert_eq!(
            detect(None, r#"<div class="cf-turnstile"></div>"#),
            Some(ChallengeKind::Interactive)
        );
        assert_eq!(
            detect(Some("Just a moment..."), "<html><body></body></html>"),
            Some(ChallengeKind::Verification)
        );
    }

    #[test]
    fn leaves_ordinary_pages_alone() {
        assert_eq!(
            detect(
                Some("Rust Programming Language"),
                "<html><body>Rust is fast.</body></html>"
            ),
            None
        );
        assert_eq!(detect(None, "# Heading\n\nOrdinary markdown body."), None);
        assert_eq!(detect(None, "Please wait while the page loads."), None);
        assert_eq!(
            detect(Some("Loading"), "<html><body>Loading...</body></html>"),
            None
        );
    }

    #[test]
    fn html_gate_and_warning_shape() {
        assert_eq!(detect_html(false, None, b"anubis_challenge"), None);
        assert_eq!(
            detect_html(true, None, b"anubis_challenge"),
            Some(ChallengeKind::Anubis)
        );
        let rendered = warnings(Some(ChallengeKind::Anubis));
        assert_eq!(rendered.len(), 1);
        assert_eq!(
            rendered[0].code,
            crate::core::warning::WarningCode::FetchChallengePage
        );
        assert_eq!(rendered[0].code.as_str(), "fetch_challenge_page");
        assert!(rendered[0].message.contains("anubis"));
        assert!(warnings(None).is_empty());
    }

    #[test]
    fn family_names_are_stable() {
        assert_eq!(ChallengeKind::Anubis.as_str(), "anubis");
        assert_eq!(ChallengeKind::Altcha.as_str(), "altcha");
        assert_eq!(ChallengeKind::Interactive.as_str(), "interactive");
        assert_eq!(ChallengeKind::Verification.as_str(), "verification");
    }
}
