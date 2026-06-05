//! URL canonicalization and tracking parameter stripping.

use url::Url;

/// Tracking parameters to strip by default. Matching is case-insensitive
/// and includes the named parameter with any value.
const TRACKING_PARAMS: &[&str] = &[
    "utm_source",
    "utm_medium",
    "utm_campaign",
    "utm_term",
    "utm_content",
    "utm_id",
    "utm_name",
    "utm_brand",
    "utm_social",
    "utm_creative_format",
    "utm_marketing_tactic",
    "gclid",
    "gbraid",
    "wbraid",
    "fbclid",
    "msclkid",
    "dclid",
    "yclid",
    "mc_cid",
    "mc_eid",
    "igshid",
    "ref",
    "ref_src",
    "ref_url",
    "source",
];

/// Canonicalize a URL for dedup and ranking purposes.
///
/// - Lowercases scheme and host.
/// - Removes URL fragments.
/// - Strips common tracking query parameters.
/// - Normalizes trailing slashes for non-root paths.
pub fn canonicalize(input: &str) -> Option<Url> {
    let mut url = Url::parse(input).ok()?;

    // Lowercase scheme + host.
    let scheme = url.scheme().to_lowercase();
    url.set_scheme(&scheme).ok()?;
    if let Some(host) = url.host_str() {
        let _ = url.set_host(Some(&host.to_lowercase()));
    }

    // Strip fragment.
    url.set_fragment(None);

    // Strip tracking params.
    if let Some(query) = url.query().map(|s| s.to_string()) {
        let filtered: Vec<(String, String)> = query
            .split('&')
            .filter(|p| !p.is_empty())
            .filter_map(|p| {
                let (k, v) = p.split_once('=').unwrap_or((p, ""));
                let key_lower = k.to_lowercase();
                if TRACKING_PARAMS.iter().any(|tp| *tp == key_lower) {
                    None
                } else {
                    Some((k.to_string(), v.to_string()))
                }
            })
            .collect();

        if filtered.is_empty() {
            url.set_query(None);
        } else {
            let new_query = filtered
                .into_iter()
                .map(|(k, v)| {
                    if v.is_empty() {
                        k
                    } else {
                        format!("{}={}", k, v)
                    }
                })
                .collect::<Vec<_>>()
                .join("&");
            url.set_query(Some(&new_query));
        }
    }

    // Normalize trailing slash: collapse multiple slashes after path's first char
    // (avoid touching scheme or empty paths).
    if url.path() != "/" {
        let trimmed = url.path().trim_end_matches('/').to_string();
        if trimmed.is_empty() {
            url.set_path("/");
        } else {
            url.set_path(&trimmed);
        }
    }

    Some(url)
}

/// Extract the registrable domain (best-effort). Falls back to the full host.
pub fn domain_of(url: &Url) -> Option<String> {
    url.domain().map(|s| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lowercases_scheme_and_host() {
        let u = canonicalize("HTTPS://Example.COM/Path").unwrap();
        assert_eq!(u.scheme(), "https");
        assert_eq!(u.host_str(), Some("example.com"));
    }

    #[test]
    fn strips_fragment() {
        let u = canonicalize("https://example.com/page#section").unwrap();
        assert!(u.fragment().is_none());
        assert_eq!(u.path(), "/page");
    }

    #[test]
    fn strips_tracking_params() {
        let u = canonicalize("https://example.com/p?a=1&utm_source=x&fbclid=zz&b=2").unwrap();
        let q = u.query().unwrap();
        assert!(q.contains("a=1"));
        assert!(q.contains("b=2"));
        assert!(!q.contains("utm_source"));
        assert!(!q.contains("fbclid"));
    }

    #[test]
    fn normalizes_trailing_slash() {
        let u = canonicalize("https://example.com/page/").unwrap();
        assert_eq!(u.path(), "/page");
        // root should remain "/"
        let root = canonicalize("https://example.com/").unwrap();
        assert_eq!(root.path(), "/");
    }

    #[test]
    fn invalid_url_returns_none() {
        assert!(canonicalize("not a url").is_none());
    }

    #[test]
    fn domain_of_works() {
        let u = Url::parse("https://docs.example.com/x").unwrap();
        assert_eq!(domain_of(&u), Some("docs.example.com".to_string()));
    }
}
