use url::Url;

const TRACKING_PARAMS: &[&str] = &[
    "utm_source",
    "utm_medium",
    "utm_campaign",
    "utm_term",
    "utm_content",
    "fbclid",
    "gclid",
    "msclkid",
    "yclid",
    "ref",
    "source",
];

const INDEX_FILES: &[&str] = &["index.html", "index.htm", "index.php"];

const LOCALE_SEGMENTS: &[&str] = &[
    "ar", "bg", "ca", "cs", "da", "de", "el", "en", "es", "et", "eu", "fa", "fi", "fr", "he", "hr",
    "hu", "is", "ja", "ko", "lt", "lv", "nb", "nl", "pl", "pt", "ro", "ru", "sk", "sl", "sr", "sv",
    "th", "tr", "uk", "vi", "zh",
];

const LOCALE_REGION_SEGMENTS: &[&str] = &[
    "ar-sa", "bg-bg", "ca-es", "cs-cz", "da-dk", "de-at", "de-ch", "de-de", "el-gr", "en-au",
    "en-ca", "en-gb", "en-ie", "en-in", "en-nz", "en-us", "es-es", "es-mx", "fa-ir", "fi-fi",
    "fr-ca", "fr-fr", "he-il", "hu-hu", "it-it", "ja-jp", "ko-kr", "nb-no", "nl-be", "nl-nl",
    "pl-pl", "pt-br", "pt-pt", "ro-ro", "ru-ru", "sk-sk", "sv-se", "th-th", "tr-tr", "uk-ua",
    "vi-vn", "zh-cn", "zh-hk", "zh-tw",
];

pub fn normalize(raw: &str) -> Option<String> {
    let mut url = Url::parse(raw).ok()?;

    url.set_fragment(None);

    let clean: Vec<(String, String)> = url
        .query_pairs()
        .filter(|(k, _)| !TRACKING_PARAMS.contains(&k.as_ref()))
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();

    if clean.is_empty() {
        url.set_query(None);
    } else {
        let mut sorted = clean;
        sorted.sort_by(|a, b| a.0.cmp(&b.0));
        let qs = sorted
            .iter()
            .map(|(k, v)| format!("{}={}", urlencoding::encode(k), urlencoding::encode(v)))
            .collect::<Vec<_>>()
            .join("&");
        url.set_query(Some(&qs));
    }

    let path = url.path().to_string();
    let path = strip_locale_prefix(&path);
    let path = strip_index_file(path);

    let path = if path.len() > 1 && path.ends_with('/') {
        path.trim_end_matches('/').to_string()
    } else {
        path.to_string()
    };

    url.set_path(&path);

    let mut result = String::with_capacity(url.as_str().len());
    result.push_str(&url.scheme().to_ascii_lowercase());
    result.push_str("://");
    if let Some(host) = url.host_str() {
        result.push_str(&host.to_ascii_lowercase());
    }
    if let Some(port) = url.port() {
        result.push(':');
        result.push_str(&port.to_string());
    }
    result.push_str(url.path());
    if let Some(query) = url.query() {
        result.push('?');
        result.push_str(query);
    }

    Some(result)
}

fn strip_locale_prefix(path: &str) -> &str {
    let rest = match path.strip_prefix('/') {
        Some(r) => r,
        None => return path,
    };

    let (segment, remainder) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => return path,
    };

    if is_locale_segment(segment) {
        remainder
    } else {
        path
    }
}

fn is_locale_segment(s: &str) -> bool {
    let normalized = s.to_ascii_lowercase().replace('_', "-");
    LOCALE_SEGMENTS.contains(&normalized.as_str())
        || LOCALE_REGION_SEGMENTS.contains(&normalized.as_str())
}

fn strip_index_file(path: &str) -> &str {
    for index in INDEX_FILES {
        if let Some(dir) = path.strip_suffix(index) {
            return dir;
        }
    }
    path
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_removes_tracking_params() {
        let n = normalize("https://example.com/page?utm_source=google&q=rust").unwrap();
        assert!(!n.contains("utm_source"));
        assert!(n.contains("q=rust"));
    }

    #[test]
    fn test_removes_fragment() {
        let a = normalize("https://example.com/page#section").unwrap();
        let b = normalize("https://example.com/page").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn test_removes_trailing_slash() {
        let a = normalize("https://example.com/page/").unwrap();
        let b = normalize("https://example.com/page").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn test_root_slash_preserved() {
        let n = normalize("https://example.com/").unwrap();
        assert!(n.ends_with('/') || n == "https://example.com");
    }

    #[test]
    fn test_sorts_query_params() {
        let a = normalize("https://example.com/?z=1&a=2").unwrap();
        let b = normalize("https://example.com/?a=2&z=1").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn only_known_locale_prefixes_are_removed() {
        assert_eq!(
            normalize("https://example.com/de/docs").unwrap(),
            "https://example.com/docs"
        );
        assert_eq!(
            normalize("https://example.com/go/rust").unwrap(),
            "https://example.com/go/rust"
        );
        assert_eq!(
            normalize("https://example.com/id/12345").unwrap(),
            "https://example.com/id/12345"
        );
        assert_eq!(
            normalize("https://example.com/xx-yy/docs").unwrap(),
            "https://example.com/xx-yy/docs"
        );
    }

    #[test]
    fn test_lowercases_scheme_and_host() {
        let a = normalize("HTTPS://Example.COM/page").unwrap();
        let b = normalize("https://example.com/page").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn test_returns_none_for_invalid_url() {
        assert!(normalize("not a url").is_none());
    }

    #[test]
    fn test_strips_locale_language_only() {
        let a = normalize("https://example.com/en/docs").unwrap();
        let b = normalize("https://example.com/docs").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn test_strips_locale_language_region_hyphen() {
        let a = normalize("https://rust-lang.org/en-US/").unwrap();
        let b = normalize("https://rust-lang.org/").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn test_strips_locale_language_region_underscore() {
        let a = normalize("https://example.com/en_US/page").unwrap();
        let b = normalize("https://example.com/page").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn test_does_not_strip_bare_short_segment() {
        let n = normalize("https://example.com/go").unwrap();
        assert!(n.contains("/go"));
    }

    #[test]
    fn test_strips_index_html() {
        let a = normalize("https://example.com/page/index.html").unwrap();
        let b = normalize("https://example.com/page").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn test_strips_index_htm() {
        let a = normalize("https://example.com/page/index.htm").unwrap();
        let b = normalize("https://example.com/page").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn test_strips_index_php() {
        let a = normalize("https://example.com/page/index.php").unwrap();
        let b = normalize("https://example.com/page").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn test_combined_locale_and_index() {
        let a = normalize("https://example.com/en-US/page/index.html").unwrap();
        let b = normalize("https://example.com/page").unwrap();
        assert_eq!(a, b);
    }
}
