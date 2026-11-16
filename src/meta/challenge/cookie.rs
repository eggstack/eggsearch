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

/// One cookie pair a response published.
#[derive(Clone, PartialEq, Eq)]
pub struct IssuedCookie {
    /// Cookie name, replayed verbatim.
    pub name: String,
    /// Cookie value, replayed verbatim.
    pub value: String,
    /// Lifetime the response published for this pair.
    pub lifetime: PublishedLifetime,
}

impl std::fmt::Debug for IssuedCookie {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IssuedCookie")
            .field("name", &self.name)
            .field("value", &"<redacted>")
            .field("lifetime", &self.lifetime)
            .finish()
    }
}

/// Every non-deletion cookie pair a response set, in replay order.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct IssuedCookies {
    cookies: Vec<IssuedCookie>,
}

impl std::fmt::Debug for IssuedCookies {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IssuedCookies")
            .field("cookies", &self.cookies)
            .finish()
    }
}

impl IssuedCookies {
    /// Replayable `Cookie` header value: surviving pairs joined with `; `.
    ///
    /// The value stays within [`MAX_ISSUED_HEADER_BYTES`] because the bound is
    /// applied while the pairs are parsed.
    #[must_use]
    pub fn header(&self) -> String {
        let mut header = String::new();
        for (index, cookie) in self.cookies.iter().enumerate() {
            if index > 0 {
                header.push_str("; ");
            }
            header.push_str(&cookie.name);
            header.push('=');
            header.push_str(&cookie.value);
        }
        header
    }

    /// One lifetime per surviving pair, in header order.
    pub fn lifetimes(&self) -> impl Iterator<Item = PublishedLifetime> + '_ {
        self.cookies.iter().map(|cookie| cookie.lifetime)
    }

    /// Lifetime of the pair with the largest remaining window at `now`.
    ///
    /// Ties keep the earliest pair. When no pair carries a numeric window,
    /// `Unstated` wins if any pair published no attribute, and `Malformed`
    /// reports the remaining cases.
    #[must_use]
    pub fn reuse_lifetime(&self, now: SystemTime) -> PublishedLifetime {
        let mut best: Option<(Duration, PublishedLifetime)> = None;
        let mut unstated = false;
        for cookie in &self.cookies {
            let window = match cookie.lifetime {
                PublishedLifetime::MaxAge(secs) => Some(Duration::from_secs(secs)),
                PublishedLifetime::ExpiresAt(at) => at.duration_since(now).ok(),
                PublishedLifetime::Unstated => {
                    unstated = true;
                    None
                }
                PublishedLifetime::Malformed => None,
            };
            let Some(window) = window else {
                continue;
            };
            let wins = match best {
                Some((best_window, _)) => window > best_window,
                None => true,
            };
            if wins {
                best = Some((window, cookie.lifetime));
            }
        }
        match best {
            Some((_, lifetime)) => lifetime,
            None if unstated => PublishedLifetime::Unstated,
            None => PublishedLifetime::Malformed,
        }
    }

    /// Whether the response published no replayable pair.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.cookies.is_empty()
    }

    /// Number of replayable pairs the joined header carries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.cookies.len()
    }
}

/// Bound on the joined `Cookie` header this parser will produce.
pub const MAX_ISSUED_HEADER_BYTES: usize = 4096;

/// Parse every replayable cookie pair out of a `Set-Cookie` header set.
///
/// A pair survives only when its name is a valid cookie-name token, its value is
/// a non-empty `Cookie` octet sequence, and the response did not mark it for
/// deletion. Deletion compares `Max-Age` first and falls back to `Expires`
/// against the current clock, exactly as [`parse_issued`] resolves a lifetime.
/// Duplicate names keep the first position and the last published value, and
/// the joined header keeps the longest prefix that fits
/// [`MAX_ISSUED_HEADER_BYTES`].
#[must_use]
pub fn parse_issued_pairs(headers: &http::HeaderMap) -> IssuedCookies {
    let now = SystemTime::now();
    let mut cookies: Vec<IssuedCookie> = Vec::new();
    for header in headers.get_all("set-cookie") {
        let Ok(raw) = header.to_str() else {
            continue;
        };
        let mut parts = raw.split(';');
        let pair = parts.next().unwrap_or_default().trim();
        let Some((name, value)) = pair.split_once('=') else {
            continue;
        };
        let name = name.trim();
        let value = value.trim();
        if !is_cookie_name(name) || !is_cookie_value(value) {
            continue;
        }
        let lifetime = published_lifetime(parts);
        if is_deletion(lifetime, now) {
            continue;
        }
        match cookies.iter_mut().find(|cookie| cookie.name == name) {
            Some(cookie) => {
                cookie.value = value.to_owned();
                cookie.lifetime = lifetime;
            }
            None => cookies.push(IssuedCookie {
                name: name.to_owned(),
                value: value.to_owned(),
                lifetime,
            }),
        }
    }
    IssuedCookies {
        cookies: bounded_pairs(cookies),
    }
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

fn is_deletion(lifetime: PublishedLifetime, now: SystemTime) -> bool {
    match lifetime {
        PublishedLifetime::MaxAge(0) => true,
        PublishedLifetime::ExpiresAt(at) => at <= now,
        _ => false,
    }
}

fn is_cookie_name(name: &str) -> bool {
    !name.is_empty() && name.bytes().all(is_cookie_name_byte)
}

fn is_cookie_name_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte)
}

fn is_cookie_value(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(is_cookie_value_byte)
}

fn is_cookie_value_byte(byte: u8) -> bool {
    (0x21..0x7f).contains(&byte) && !b";,\"\\".contains(&byte)
}

fn bounded_pairs(cookies: Vec<IssuedCookie>) -> Vec<IssuedCookie> {
    let mut kept: Vec<IssuedCookie> = Vec::new();
    let mut length = 0usize;
    for cookie in cookies {
        let pair = cookie.name.len() + cookie.value.len() + 1;
        let separator = if kept.is_empty() { 0 } else { 2 };
        if length + separator + pair > MAX_ISSUED_HEADER_BYTES {
            break;
        }
        length += separator + pair;
        kept.push(cookie);
    }
    kept
}

#[cfg(test)]
mod tests {
    use super::*;

    const CEILING: Duration = Duration::from_secs(240);
    const ISSUED: &str = "Wed, 11 Nov 2026 00:00:00 GMT";
    const ISSUED_EPOCH: u64 = 1_794_355_200;
    const EXPIRED_COOKIE: &str = "spchal-auth=pass-123; Expires=Wed, 11 Nov 2020 00:00:00 GMT";
    const LONG_LIVED_COOKIE: &str = "sp_pow=pow-1; Expires=Fri, 13 Nov 2099 00:00:00 GMT";
    const FUTURE_DATE: &str = "Fri, 13 Nov 2099 00:00:00 GMT";

    fn issued_at() -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(ISSUED_EPOCH)
    }

    fn headers(value: &'static str) -> http::HeaderMap {
        let mut headers = http::HeaderMap::new();
        headers.append("set-cookie", http::HeaderValue::from_static(value));
        headers
    }

    fn header_set(values: &[&'static str]) -> http::HeaderMap {
        let mut headers = http::HeaderMap::new();
        for &value in values {
            headers.append("set-cookie", http::HeaderValue::from_static(value));
        }
        headers
    }

    fn owned_header_set(values: &[String]) -> http::HeaderMap {
        let mut headers = http::HeaderMap::new();
        for value in values {
            let header = http::HeaderValue::from_str(value.as_str()).expect("valid header");
            headers.append("set-cookie", header);
        }
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

    #[test]
    fn startpage_pass_pair_is_replayed_verbatim() {
        let issued = parse_issued_pairs(&headers("spchal-auth=pass-123; Max-Age=300"));
        assert_eq!(issued.header(), "spchal-auth=pass-123");
        assert_eq!(issued.len(), 1);
        assert_eq!(
            issued.lifetimes().next(),
            Some(PublishedLifetime::MaxAge(300))
        );
    }

    #[test]
    fn default_anubis_names_and_a_dynamic_suffix_survive_verbatim() {
        let cases = [
            "techaro.lol-anubis-auth=pass-123; Max-Age=300",
            "techaro.lol-anubis-cookie-verification=verif-1; Max-Age=300",
            "techaro.lol-anubis-auth-347ddb4a=pass-456; Max-Age=300",
        ];
        for pair in cases {
            let expected = pair.split(';').next().unwrap_or_default();
            let issued = parse_issued_pairs(&headers(pair));
            assert_eq!(issued.header(), expected);
            assert_eq!(issued.len(), 1);
        }
    }

    #[test]
    fn pairs_join_in_wire_order_and_lifetimes_follow_header_order() {
        let issued = parse_issued_pairs(&header_set(&[
            "spchal-auth=pass-123; Max-Age=300",
            LONG_LIVED_COOKIE,
            "spchal-cookie-verification=verif-1",
        ]));
        assert_eq!(
            issued.header(),
            "spchal-auth=pass-123; sp_pow=pow-1; spchal-cookie-verification=verif-1"
        );
        let future = parse_http_date(FUTURE_DATE).expect("date parses");
        assert_eq!(
            issued.lifetimes().collect::<Vec<_>>(),
            vec![
                PublishedLifetime::MaxAge(300),
                PublishedLifetime::ExpiresAt(future),
                PublishedLifetime::Unstated,
            ]
        );
    }

    #[test]
    fn reuse_lifetime_picks_the_longest_published_window() {
        let issued = parse_issued_pairs(&header_set(&[
            "spchal-referer=ref-1; Max-Age=60",
            "spchal-auth=pass-123; Max-Age=3600",
        ]));
        assert_eq!(
            issued.header(),
            "spchal-referer=ref-1; spchal-auth=pass-123"
        );
        assert_eq!(
            issued.reuse_lifetime(issued_at()),
            PublishedLifetime::MaxAge(3600)
        );
        assert_eq!(issued.len(), 2);
    }

    #[test]
    fn deletion_pairs_are_dropped() {
        assert!(parse_issued_pairs(&headers("spchal-auth=pass-123; Max-Age=0")).is_empty());
        assert!(parse_issued_pairs(&headers(EXPIRED_COOKIE)).is_empty());
        let max_age = parse_issued_pairs(&header_set(&["spchal-auth=pass-123; Max-Age=0"]));
        assert!(max_age.is_empty());
        let mixed = parse_issued_pairs(&header_set(&[
            "spchal-auth=pass-123; Max-Age=0",
            "sp_pow=pow-1",
        ]));
        assert_eq!(mixed.header(), "sp_pow=pow-1");
        assert_eq!(mixed.len(), 1);
        let expired_mixed = parse_issued_pairs(&header_set(&[EXPIRED_COOKIE, "sp_pow=pow-1"]));
        assert_eq!(expired_mixed.header(), "sp_pow=pow-1");
        assert_eq!(expired_mixed.len(), 1);
    }

    #[test]
    fn duplicate_names_keep_the_first_position_and_the_last_value() {
        let issued = parse_issued_pairs(&header_set(&[
            "spchal-auth=first; Max-Age=60",
            "sp_pow=pow-1",
            "spchal-auth=second; Max-Age=300",
        ]));
        assert_eq!(issued.header(), "spchal-auth=second; sp_pow=pow-1");
        assert_eq!(
            issued.lifetimes().collect::<Vec<_>>(),
            vec![PublishedLifetime::MaxAge(300), PublishedLifetime::Unstated]
        );
    }

    #[test]
    fn a_trailing_deletion_does_not_erase_an_earlier_value() {
        let issued = parse_issued_pairs(&header_set(&[
            "spchal-auth=real; Max-Age=300",
            "spchal-auth=gone; Max-Age=0",
        ]));
        assert_eq!(issued.header(), "spchal-auth=real");
        assert_eq!(
            issued.reuse_lifetime(issued_at()),
            PublishedLifetime::MaxAge(300)
        );
    }

    #[test]
    fn malformed_attributes_still_replay_the_pair() {
        let issued = parse_issued_pairs(&headers("spchal-auth=pass-123; Max-Age=abc"));
        assert_eq!(issued.header(), "spchal-auth=pass-123");
        assert_eq!(
            issued.lifetimes().next(),
            Some(PublishedLifetime::Malformed)
        );
        assert_eq!(
            issued.reuse_lifetime(issued_at()),
            PublishedLifetime::Malformed
        );
    }

    #[test]
    fn unstated_outranks_malformed_as_the_reuse_fallback() {
        let mixed = parse_issued_pairs(&header_set(&[
            "spchal-auth=pass-123; Max-Age=abc",
            "sp_pow=pow-1",
        ]));
        assert_eq!(
            mixed.reuse_lifetime(issued_at()),
            PublishedLifetime::Unstated
        );
        assert_eq!(
            IssuedCookies::default().reuse_lifetime(issued_at()),
            PublishedLifetime::Malformed
        );
    }

    #[test]
    fn invalid_pairs_are_skipped() {
        let issued = parse_issued_pairs(&header_set(&[
            "spchal-auth=",
            "spchal-auth= ; Path=/",
            "spchal-auth",
            "sp chal-auth=pass-123",
            "spchal;auth=pass-123",
            "spchal-auth=bad value",
        ]));
        assert!(issued.is_empty());
        assert!(issued.header().is_empty());
        assert_eq!(issued, IssuedCookies::default());
    }

    #[test]
    fn joined_header_is_bounded() {
        let value = "a".repeat(96);
        let values = (0..80)
            .map(|index| format!("c{index:02}={value}"))
            .collect::<Vec<_>>();
        let issued = parse_issued_pairs(&owned_header_set(&values));
        assert_eq!(issued.len(), 40);
        assert_eq!(issued.header(), values[..40].join("; "));
        assert_eq!(issued.header().len(), 4078);
        assert!(issued.header().len() <= MAX_ISSUED_HEADER_BYTES);
        assert_eq!(issued.lifetimes().count(), 40);
    }

    #[test]
    fn debug_redacts_cookie_values() {
        let issued = parse_issued_pairs(&headers("spchal-auth=super-secret; Max-Age=300"));
        let debug = format!("{issued:?}");
        assert!(!debug.contains("super-secret"));
        assert!(debug.contains("spchal-auth"));
        assert!(debug.contains("<redacted>"));
        let pair = format!("{:?}", issued.cookies[0]);
        assert!(!pair.contains("super-secret"));
        assert!(pair.contains("<redacted>"));
    }

    #[test]
    fn legacy_parse_issued_keeps_the_zero_max_age_cookie() {
        let issued = parse_issued(&headers("chllg=altcha-1; Max-Age=0"), "chllg");
        let (value, lifetime) = issued.expect("legacy path keeps the deletion pair");
        assert_eq!(value, "altcha-1");
        assert_eq!(lifetime, PublishedLifetime::MaxAge(0));
        assert_eq!(reuse_ttl(lifetime, issued_at(), CEILING), Duration::ZERO);
        assert!(parse_issued_pairs(&headers("chllg=altcha-1; Max-Age=0")).is_empty());
    }
}
