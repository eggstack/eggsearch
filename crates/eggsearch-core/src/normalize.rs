//! URL canonicalization and tracking parameter stripping.
//!
//! The canonicalization logic is currently unused in production code.
//! The upstream `metadata-search-engine-rs` crate provides its own
//! URL normalization in `normalizer::normalize`. This module is
//! retained for potential future use in result deduplication.

use url::Url;

const TRACKING_PARAMS: &[&str] = &[
    "utm_source", "utm_medium", "utm_campaign", "utm_term", "utm_content",
    "utm_id", "utm_name", "utm_brand", "utm_social", "utm_creative_format",
    "utm_marketing_tactic", "gclid", "gbraid", "wbraid", "fbclid", "msclkid",
    "dclid", "yclid", "mc_cid", "mc_eid", "igshid", "ref", "ref_src",
    "ref_url", "source",
];

pub fn canonicalize(input: &str) -> Option<Url> {
    let mut url = Url::parse(input).ok()?;
    let scheme = url.scheme().to_lowercase();
    url.set_scheme(&scheme).ok()?;
    if let Some(host) = url.host_str() {
        let _ = url.set_host(Some(&host.to_lowercase()));
    }
    url.set_fragment(None);
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
                .map(|(k, v)| if v.is_empty() { k } else { format!("{}={}", k, v) })
                .collect::<Vec<_>>()
                .join("&");
            url.set_query(Some(&new_query));
        }
    }
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
        let root = canonicalize("https://example.com/").unwrap();
        assert_eq!(root.path(), "/");
    }

    #[test]
    fn invalid_url_returns_none() {
        assert!(canonicalize("not a url").is_none());
    }

    #[test]
    fn preserves_encoded_query_values() {
        let u = canonicalize("https://example.com/search?q=hello+world&lang=en").unwrap();
        let q = u.query().unwrap();
        assert!(q.contains("q=hello+world"));
        assert!(q.contains("lang=en"));
    }

    #[test]
    fn drops_all_tracking_params_leaves_clean_url() {
        let u = canonicalize(
            "https://example.com/page?utm_source=x&utm_medium=y&fbclid=z&page=1",
        )
        .unwrap();
        assert_eq!(u.query(), Some("page=1"));
    }

    #[test]
    fn empty_query_after_filtering_removes_question_mark() {
        let u = canonicalize("https://example.com/page?utm_source=x").unwrap();
        assert!(u.query().is_none());
        assert!(!u.as_str().contains('?'));
    }
}
