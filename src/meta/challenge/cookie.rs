//! Pass-cookie lifetime parsing shared by every challenge scheme.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chrono::NaiveDateTime;

/// Lifetime an upstream published for its pass cookie.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PublishedLifetime {
    /// Neither `Max-Age` nor `Expires` was published.
    Unstated,
    /// `Max-Age` in seconds.
    MaxAge(u64),
    /// `Expires` as an absolute time.
    ExpiresAt(SystemTime),
    /// A lifetime attribute was published but could not be used.
    Malformed,
}

impl PublishedLifetime {
    /// Attribute kind. Safe to print; carries no attribute text.
    #[must_use]
    pub fn kind(self) -> &'static str {
        match self {
            Self::Unstated => "unstated",
            Self::MaxAge(_) => "max-age",
            Self::ExpiresAt(_) => "expires",
            Self::Malformed => "malformed",
        }
    }
}

/// Parse the `name` cookie out of a `Set-Cookie` header set.
///
/// The value comes from the cookie-pair segment only. `Max-Age` takes
/// precedence over `Expires` when both are published.
#[must_use]
pub fn parse_issued(headers: &http::HeaderMap, name: &str) -> Option<(String, PublishedLifetime)> {
    for header in headers.get_all("set-cookie") {
        let Ok(raw) = header.to_str() else {
            continue;
        };
        let mut parts = raw.split(';');
        let pair = parts.next().unwrap_or_default().trim();
        let Some((pair_name, cookie)) = pair.split_once('=') else {
            continue;
        };
        if pair_name.trim() != name {
            continue;
        }
        let cookie = cookie.trim();
        if cookie.is_empty() {
            continue;
        }
        return Some((cookie.to_string(), published_lifetime(parts)));
    }
    None
}

/// Reuse window for a cookie issued with `lifetime` at `now`.
///
/// `Duration::ZERO` means "do not reuse" and matches [`super::PassCookieCache`]'s
/// zero-TTL rule.
#[must_use]
pub fn reuse_ttl(lifetime: PublishedLifetime, now: SystemTime, ceiling: Duration) -> Duration {
    if ceiling.is_zero() {
        return Duration::ZERO;
    }
    match lifetime {
        PublishedLifetime::Unstated => ceiling,
        PublishedLifetime::Malformed => Duration::ZERO,
        PublishedLifetime::MaxAge(0) => Duration::ZERO,
        PublishedLifetime::MaxAge(secs) => Duration::from_secs(secs).min(ceiling),
        PublishedLifetime::ExpiresAt(at) => at
            .duration_since(now)
            .map_or(Duration::ZERO, |remaining| remaining.min(ceiling)),
    }
}

fn published_lifetime<'a>(parts: impl Iterator<Item = &'a str>) -> PublishedLifetime {
    let mut max_age: Option<Option<u64>> = None;
    let mut expires: Option<Option<SystemTime>> = None;
    for part in parts {
        let part = part.trim();
        let Some((attribute, value)) = part.split_once('=') else {
            continue;
        };
        let attribute = attribute.trim();
        let value = value.trim();
        if attribute.eq_ignore_ascii_case("max-age") {
            max_age = Some(value.parse::<u64>().ok());
        } else if attribute.eq_ignore_ascii_case("expires") {
            expires = Some(parse_http_date(value));
        }
    }
    match (max_age, expires) {
        (Some(Some(secs)), _) => PublishedLifetime::MaxAge(secs),
        (Some(None), _) => PublishedLifetime::Malformed,
        (None, Some(Some(at))) => PublishedLifetime::ExpiresAt(at),
        (None, Some(None)) => PublishedLifetime::Malformed,
        (None, None) => PublishedLifetime::Unstated,
    }
}

fn parse_http_date(value: &str) -> Option<SystemTime> {
    let parsed = NaiveDateTime::parse_from_str(value, "%a, %d %b %Y %H:%M:%S GMT").ok()?;
    let seconds = parsed.and_utc().timestamp();
    let magnitude = Duration::from_secs(seconds.unsigned_abs());
    if seconds >= 0 {
        UNIX_EPOCH.checked_add(magnitude)
    } else {
        UNIX_EPOCH.checked_sub(magnitude)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CEILING: Duration = Duration::from_secs(240);
    const ISSUED: &str = "Wed, 11 Nov 2026 00:00:00 GMT";
    const ISSUED_EPOCH: u64 = 1_794_355_200;

    fn issued_at() -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(ISSUED_EPOCH)
    }

    fn headers(value: &'static str) -> http::HeaderMap {
        let mut headers = http::HeaderMap::new();
        headers.append("set-cookie", http::HeaderValue::from_static(value));
        headers
    }

    fn lifetime(value: &'static str) -> PublishedLifetime {
        parse_issued(&headers(value), "spchal-auth")
            .expect("cookie is published")
            .1
    }

    fn resolved(value: &'static str, now: SystemTime, ceiling: Duration) -> Duration {
        reuse_ttl(lifetime(value), now, ceiling)
    }

    #[test]
    fn unstated_lifetime_falls_back_to_the_ceiling() {
        assert_eq!(
            resolved("spchal-auth=abc; Path=/; HttpOnly", issued_at(), CEILING),
            CEILING
        );
        assert_eq!(lifetime("spchal-auth=abc"), PublishedLifetime::Unstated);
    }

    #[test]
    fn max_age_shorter_than_the_ceiling_wins() {
        assert_eq!(
            resolved("spchal-auth=abc; Max-Age=45; Path=/", issued_at(), CEILING),
            Duration::from_secs(45)
        );
    }

    #[test]
    fn max_age_longer_than_the_ceiling_is_clamped() {
        assert_eq!(
            resolved("spchal-auth=abc; Max-Age=3600", issued_at(), CEILING),
            CEILING
        );
    }

    #[test]
    fn zero_ceiling_disables_reuse_for_a_long_lived_cookie() {
        assert_eq!(
            resolved(
                "spchal-auth=abc; Max-Age=3600; Expires=Wed, 11 Nov 2026 00:00:00 GMT",
                issued_at(),
                Duration::ZERO
            ),
            Duration::ZERO
        );
        assert_eq!(
            resolved("spchal-auth=abc", issued_at(), Duration::ZERO),
            Duration::ZERO
        );
    }

    #[test]
    fn max_age_zero_stores_nothing() {
        assert_eq!(
            resolved("spchal-auth=abc; Max-Age=0", issued_at(), CEILING),
            Duration::ZERO
        );
    }

    #[test]
    fn expires_in_the_future_is_honoured_and_clamped() {
        let now = issued_at() - Duration::from_secs(60);
        assert_eq!(
            resolved(
                "spchal-auth=abc; Expires=Wed, 11 Nov 2026 00:00:00 GMT",
                now,
                CEILING
            ),
            Duration::from_secs(60)
        );
        let now = issued_at() - Duration::from_secs(600);
        assert_eq!(
            resolved(
                "spchal-auth=abc; Expires=Wed, 11 Nov 2026 00:00:00 GMT",
                now,
                CEILING
            ),
            CEILING
        );
    }

    #[test]
    fn expires_in_the_past_stores_nothing() {
        let now = issued_at() + Duration::from_secs(1);
        assert_eq!(
            resolved(
                "spchal-auth=abc; Expires=Wed, 11 Nov 2026 00:00:00 GMT",
                now,
                CEILING
            ),
            Duration::ZERO
        );
        assert_eq!(
            resolved(
                "spchal-auth=abc; Expires=Wed, 11 Nov 2026 00:00:00 GMT",
                issued_at(),
                CEILING
            ),
            Duration::ZERO
        );
    }

    #[test]
    fn malformed_max_age_stores_nothing() {
        assert_eq!(
            resolved("spchal-auth=abc; Max-Age=abc", issued_at(), CEILING),
            Duration::ZERO
        );
        assert_eq!(
            resolved("spchal-auth=abc; Max-Age=-1", issued_at(), CEILING),
            Duration::ZERO
        );
        assert_eq!(
            resolved("spchal-auth=abc; Max-Age=", issued_at(), CEILING),
            Duration::ZERO
        );
        assert_eq!(
            lifetime("spchal-auth=abc; Max-Age=abc"),
            PublishedLifetime::Malformed
        );
    }

    #[test]
    fn malformed_expires_stores_nothing() {
        assert_eq!(
            resolved("spchal-auth=abc; Expires=tomorrow", issued_at(), CEILING),
            Duration::ZERO
        );
        assert_eq!(
            resolved("spchal-auth=abc; Expires=", issued_at(), CEILING),
            Duration::ZERO
        );
        assert_eq!(
            lifetime("spchal-auth=abc; Expires=tomorrow"),
            PublishedLifetime::Malformed
        );
    }

    #[test]
    fn max_age_wins_over_a_past_expires() {
        assert_eq!(
            resolved(
                "spchal-auth=abc; Max-Age=3600; Expires=Wed, 11 Nov 2020 00:00:00 GMT",
                issued_at(),
                CEILING
            ),
            CEILING
        );
        assert_eq!(
            resolved(
                "spchal-auth=abc; Expires=Wed, 11 Nov 2020 00:00:00 GMT; Max-Age=45",
                issued_at(),
                CEILING
            ),
            Duration::from_secs(45)
        );
    }

    #[test]
    fn attribute_names_match_case_insensitively() {
        assert_eq!(
            resolved("spchal-auth=abc; max-age=45", issued_at(), CEILING),
            Duration::from_secs(45)
        );
        assert_eq!(
            resolved("spchal-auth=abc; MAX-AGE=45", issued_at(), CEILING),
            Duration::from_secs(45)
        );
        let now = issued_at() - Duration::from_secs(60);
        assert_eq!(
            resolved(
                "spchal-auth=abc; eXpIrEs=Wed, 11 Nov 2026 00:00:00 GMT",
                now,
                CEILING
            ),
            Duration::from_secs(60)
        );
    }

    #[test]
    fn value_comes_from_the_cookie_pair_only() {
        assert_eq!(
            parse_issued(
                &headers("other=1; spchal-auth=attribute-value"),
                "spchal-auth"
            ),
            None,
            "an attribute that names the cookie must not be read as the pair"
        );
        assert_eq!(
            parse_issued(&headers("spchal-auth-extra=abc; Path=/"), "spchal-auth"),
            None,
            "a longer cookie name must not satisfy a shorter one"
        );
        let (value, _) = parse_issued(
            &headers("spchal-auth=real; spchal-auth=later"),
            "spchal-auth",
        )
        .expect("pair is published");
        assert_eq!(value, "real");
    }

    #[test]
    fn unrelated_set_cookie_headers_are_skipped() {
        let mut headers = http::HeaderMap::new();
        headers.append(
            "set-cookie",
            http::HeaderValue::from_static("spchal-cookie-verification=verif-1; Path=/"),
        );
        headers.append(
            "set-cookie",
            http::HeaderValue::from_static("sp_pow=pow-1; Max-Age=300"),
        );
        assert_eq!(parse_issued(&headers, "spchal-auth"), None);
        headers.append(
            "set-cookie",
            http::HeaderValue::from_static("spchal-auth=pass-123; Max-Age=300"),
        );
        let (value, lifetime) =
            parse_issued(&headers, "spchal-auth").expect("pass cookie is published");
        assert_eq!(value, "pass-123");
        assert_eq!(lifetime, PublishedLifetime::MaxAge(300));
    }

    #[test]
    fn empty_value_is_not_a_published_cookie() {
        assert_eq!(parse_issued(&headers("spchal-auth="), "spchal-auth"), None);
        assert_eq!(
            parse_issued(&headers("spchal-auth= ; Path=/"), "spchal-auth"),
            None
        );
    }

    #[test]
    fn expires_is_parsed_as_imf_fixdate() {
        assert_eq!(
            lifetime("spchal-auth=abc; Expires=Wed, 11 Nov 2026 00:00:00 GMT"),
            PublishedLifetime::ExpiresAt(issued_at())
        );
        assert_eq!(issued_at(), UNIX_EPOCH + Duration::from_secs(ISSUED_EPOCH));
        assert_eq!(ISSUED, "Wed, 11 Nov 2026 00:00:00 GMT");
    }

    #[test]
    fn lifetime_kind_carries_no_values() {
        assert_eq!(PublishedLifetime::Unstated.kind(), "unstated");
        assert_eq!(PublishedLifetime::MaxAge(45).kind(), "max-age");
        assert_eq!(PublishedLifetime::ExpiresAt(issued_at()).kind(), "expires");
        assert_eq!(PublishedLifetime::Malformed.kind(), "malformed");
        assert!(!PublishedLifetime::MaxAge(45).kind().contains("45"));
    }
}
