use super::*;
use crate::core::fetch::ExtractMode;
use http::header::HeaderMap;

#[test]
fn cache_scope_anonymous() {
    let scope = CacheScope::Anonymous;
    assert_ne!(scope, CacheScope::Profile(ProfileId("test".into())));
}

#[test]
fn cache_scope_profile_partitioning() {
    let s1 = CacheScope::Profile(ProfileId("alice".into()));
    let s2 = CacheScope::Profile(ProfileId("bob".into()));
    assert_ne!(s1, s2);
}

#[test]
fn raw_key_normalizes_url() {
    let k1 = build_raw_cache_key(
        "https://Example.com:443/path",
        &CacheScope::Anonymous,
        crate::fetch::accept::AcceptPolicy::Markdown,
    );
    let k2 = build_raw_cache_key(
        "https://example.com/path",
        &CacheScope::Anonymous,
        crate::fetch::accept::AcceptPolicy::Markdown,
    );
    assert_eq!(k1.url, k2.url);
    assert_eq!(k1, k2);
}

#[test]
fn raw_key_scope_partitioning() {
    let k1 = build_raw_cache_key(
        "https://x.com",
        &CacheScope::Anonymous,
        crate::fetch::accept::AcceptPolicy::Markdown,
    );
    let k2 = build_raw_cache_key(
        "https://x.com",
        &CacheScope::Profile(ProfileId("test".into())),
        crate::fetch::accept::AcceptPolicy::Markdown,
    );
    assert_ne!(k1, k2);
}

#[test]
fn raw_key_accept_policy_partitioning() {
    let markdown = build_raw_cache_key(
        "https://x.com/page",
        &CacheScope::Anonymous,
        crate::fetch::accept::AcceptPolicy::Markdown,
    );
    let html = build_raw_cache_key(
        "https://x.com/page",
        &CacheScope::Anonymous,
        crate::fetch::accept::AcceptPolicy::Html,
    );
    assert_eq!(markdown.url, html.url);
    assert_ne!(
        markdown, html,
        "a markdown body and an html body must never share a raw key"
    );
    assert_eq!(markdown.accept_policy.token(), "md1");
    assert_eq!(html.accept_policy.token(), "html1");
}

#[tokio::test]
async fn cache_raw_entries_never_cross_the_accept_policy_dimension() {
    let cache = FetchCache::new(10, 10, 1024 * 1024, 1024 * 1024);
    let scope = CacheScope::Anonymous;
    let markdown_key = build_raw_cache_key(
        "https://x.com/page",
        &scope,
        crate::fetch::accept::AcceptPolicy::Markdown,
    );
    let html_key = build_raw_cache_key(
        "https://x.com/page",
        &scope,
        crate::fetch::accept::AcceptPolicy::Html,
    );

    let mut markdown_entry = make_raw_entry(0, 4);
    markdown_entry.final_url = "https://x.com/page".to_string();
    markdown_entry.body = Arc::from(b"# md" as &[u8]);
    markdown_entry.content_type = Some("text/markdown".to_string());
    cache.insert_raw(markdown_key.clone(), markdown_entry).await;

    assert!(cache.get_raw(&html_key).await.is_none());
    assert_eq!(
        cache
            .get_raw(&markdown_key)
            .await
            .expect("markdown hit")
            .body
            .as_ref(),
        b"# md"
    );

    let mut html_entry = make_raw_entry(0, 6);
    html_entry.final_url = "https://x.com/page".to_string();
    html_entry.body = Arc::from(b"<p></p>" as &[u8]);
    html_entry.content_type = Some("text/html".to_string());
    cache.insert_raw(html_key.clone(), html_entry).await;

    assert_eq!(
        cache
            .get_raw(&markdown_key)
            .await
            .expect("markdown hit")
            .body
            .as_ref(),
        b"# md",
        "an html body must never be served for the markdown key"
    );
    assert_eq!(
        cache
            .get_raw(&html_key)
            .await
            .expect("html hit")
            .body
            .as_ref(),
        b"<p></p>",
        "a markdown body must never be served for the html key"
    );
}

#[test]
fn freshness_from_headers_max_age() {
    let mut headers = HeaderMap::new();
    headers.insert("cache-control", "max-age=300".parse().unwrap());
    let (freshness, _) = CacheFreshness::from_headers(&headers);
    assert_eq!(freshness.max_age, Some(Duration::from_secs(300)));
    assert!(!freshness.no_store);
}

#[test]
fn apply_304_headers_updates_supplied_fields() {
    let mut freshness = CacheFreshness {
        max_age: Some(Duration::from_secs(3600)),
        ..Default::default()
    };
    let mut validators = CacheValidators {
        etag: Some("\"old\"".to_string()),
        last_modified: Some("Mon, 01 Jan 2024 00:00:00 GMT".to_string()),
    };
    let h304: HashMap<String, String> = [
        (
            "cache-control".to_string(),
            "max-age=0, no-cache".to_string(),
        ),
        ("etag".to_string(), "\"new\"".to_string()),
    ]
    .into_iter()
    .collect();

    apply_304_headers(&mut freshness, &mut validators, &h304);

    assert_eq!(freshness.max_age, Some(Duration::from_secs(0)));
    assert!(freshness.no_cache);
    assert!(!freshness.no_store);
    assert_eq!(validators.etag.as_deref(), Some("\"new\""));
    assert_eq!(
        validators.last_modified.as_deref(),
        Some("Mon, 01 Jan 2024 00:00:00 GMT")
    );
}

#[test]
fn apply_304_headers_keeps_absent_fields() {
    let mut freshness = CacheFreshness {
        max_age: Some(Duration::from_secs(60)),
        expires: Some(SystemTime::now() + Duration::from_secs(120)),
        ..Default::default()
    };
    let mut validators = CacheValidators {
        etag: Some("\"keep\"".to_string()),
        last_modified: None,
    };

    apply_304_headers(&mut freshness, &mut validators, &HashMap::new());

    assert_eq!(freshness.max_age, Some(Duration::from_secs(60)));
    assert!(freshness.expires.is_some());
    assert_eq!(validators.etag.as_deref(), Some("\"keep\""));
}

#[test]
fn apply_304_headers_only_expires_preserves_max_age() {
    let stored_max_age = Duration::from_secs(60);
    let mut freshness = CacheFreshness {
        max_age: Some(stored_max_age),
        ..Default::default()
    };
    let mut validators = CacheValidators {
        etag: Some("\"keep\"".to_string()),
        last_modified: None,
    };
    let headers: HashMap<String, String> = [(
        "expires".to_string(),
        "Wed, 01 Jan 2025 00:00:00 GMT".to_string(),
    )]
    .into_iter()
    .collect();

    apply_304_headers(&mut freshness, &mut validators, &headers);

    assert_eq!(freshness.max_age, Some(stored_max_age));
    assert!(freshness.expires.is_some());
}

#[test]
fn apply_304_headers_updates_vary_and_expires() {
    let mut freshness = CacheFreshness {
        vary: Some("Accept-Encoding".to_string()),
        ..Default::default()
    };
    let mut validators = CacheValidators {
        etag: None,
        last_modified: None,
    };
    let h304: HashMap<String, String> = [
        ("vary".to_string(), "User-Agent".to_string()),
        (
            "expires".to_string(),
            "Wed, 21 Oct 2099 07:28:00 GMT".to_string(),
        ),
    ]
    .into_iter()
    .collect();

    apply_304_headers(&mut freshness, &mut validators, &h304);

    assert_eq!(freshness.vary.as_deref(), Some("User-Agent"));
    assert!(freshness.expires.is_some());
}

#[test]
fn freshness_from_headers_matches_cache_control_tokens() {
    let mut headers = HeaderMap::new();
    headers.insert(
        "cache-control",
        "no-cacheable, custom-no-store-extension, not-private"
            .parse()
            .unwrap(),
    );
    let (freshness, _) = CacheFreshness::from_headers(&headers);
    assert!(!freshness.no_store);
    assert!(!freshness.no_cache);
    assert!(!freshness.private);
}

#[test]
fn freshness_from_headers_prefers_s_maxage() {
    let mut headers = HeaderMap::new();
    headers.insert("cache-control", "max-age=300, s-maxage=60".parse().unwrap());
    let (freshness, _) = CacheFreshness::from_headers(&headers);
    assert_eq!(freshness.max_age, Some(Duration::from_secs(60)));
}

#[test]
fn freshness_from_headers_no_store() {
    let mut headers = HeaderMap::new();
    headers.insert("cache-control", "no-store".parse().unwrap());
    let (freshness, _) = CacheFreshness::from_headers(&headers);
    assert!(freshness.no_store);
}

#[test]
fn freshness_from_headers_etag() {
    let mut headers = HeaderMap::new();
    headers.insert("etag", "\"abc123\"".parse().unwrap());
    let (_, validators) = CacheFreshness::from_headers(&headers);
    assert_eq!(validators.etag.as_deref(), Some("\"abc123\""));
}

#[test]
fn freshness_from_headers_last_modified() {
    let mut headers = HeaderMap::new();
    headers.insert(
        "last-modified",
        "Wed, 01 Jan 2025 00:00:00 GMT".parse().unwrap(),
    );
    let (_, validators) = CacheFreshness::from_headers(&headers);
    assert!(validators.last_modified.is_some());
}

#[test]
fn freshness_from_headers_expires() {
    let mut headers = HeaderMap::new();
    headers.insert("expires", "Wed, 01 Jan 2025 00:00:00 GMT".parse().unwrap());
    let (freshness, _) = CacheFreshness::from_headers(&headers);
    assert!(freshness.expires.is_some());
}

#[test]
fn freshness_from_headers_parses_legacy_http_date_zones() {
    for zone in ["UT", "Z", "PST"] {
        let mut headers = HeaderMap::new();
        headers.insert(
            "expires",
            format!("Wed, 01 Jan 2025 00:00:00 {zone}").parse().unwrap(),
        );
        let (freshness, _) = CacheFreshness::from_headers(&headers);
        assert!(freshness.expires.is_some(), "failed to parse {zone}");
    }
}

#[test]
fn freshness_no_cache_not_fresh() {
    let freshness = CacheFreshness {
        max_age: Some(Duration::from_secs(3600)),
        no_cache: true,
        ..CacheFreshness::default()
    };
    assert!(!freshness.is_fresh());
}

#[test]
fn should_cache_rejects_no_store() {
    let freshness = CacheFreshness {
        no_store: true,
        ..CacheFreshness::default()
    };
    assert!(!should_cache_response(
        200,
        Some("text/html"),
        &freshness,
        &CacheScope::Anonymous
    ));
}

#[test]
fn should_cache_rejects_error_status() {
    let freshness = CacheFreshness {
        max_age: Some(Duration::from_secs(300)),
        ..CacheFreshness::default()
    };
    assert!(!should_cache_response(
        404,
        Some("text/html"),
        &freshness,
        &CacheScope::Anonymous
    ));
    assert!(!should_cache_response(
        500,
        Some("text/html"),
        &freshness,
        &CacheScope::Anonymous
    ));
}

#[test]
fn should_cache_rejects_images() {
    let freshness = CacheFreshness {
        max_age: Some(Duration::from_secs(300)),
        ..CacheFreshness::default()
    };
    assert!(!should_cache_response(
        200,
        Some("image/png"),
        &freshness,
        &CacheScope::Anonymous
    ));
}

#[test]
fn should_cache_allows_text_html() {
    let freshness = CacheFreshness {
        max_age: Some(Duration::from_secs(300)),
        ..CacheFreshness::default()
    };
    assert!(should_cache_response(
        200,
        Some("text/html"),
        &freshness,
        &CacheScope::Anonymous
    ));
}

#[test]
fn should_cache_rejects_private_in_anonymous_scope() {
    let freshness = CacheFreshness {
        max_age: Some(Duration::from_secs(300)),
        private: true,
        ..CacheFreshness::default()
    };
    assert!(!should_cache_response(
        200,
        Some("text/html"),
        &freshness,
        &CacheScope::Anonymous
    ));
}

#[test]
fn should_cache_allows_private_in_profile_scope() {
    let freshness = CacheFreshness {
        max_age: Some(Duration::from_secs(300)),
        private: true,
        ..CacheFreshness::default()
    };
    assert!(should_cache_response(
        200,
        Some("text/html"),
        &freshness,
        &CacheScope::Profile(ProfileId("alice".into()))
    ));
}

#[test]
fn should_cache_rejects_unsupported_vary() {
    let freshness = CacheFreshness {
        max_age: Some(Duration::from_secs(300)),
        vary: Some("Authorization".into()),
        ..CacheFreshness::default()
    };
    assert!(!should_cache_response(
        200,
        Some("text/html"),
        &freshness,
        &CacheScope::Anonymous
    ));
}

#[test]
fn should_cache_allows_vary_accept_encoding_only() {
    let freshness = CacheFreshness {
        max_age: Some(Duration::from_secs(300)),
        vary: Some("Accept-Encoding".into()),
        ..CacheFreshness::default()
    };
    assert!(should_cache_response(
        200,
        Some("text/html"),
        &freshness,
        &CacheScope::Anonymous
    ));
}

#[test]
fn should_cache_allows_vary_accept() {
    for vary in [
        "Accept",
        "accept",
        "Accept, Accept-Encoding",
        "Accept-Encoding, Accept",
    ] {
        let freshness = CacheFreshness {
            max_age: Some(Duration::from_secs(300)),
            vary: Some(vary.into()),
            ..CacheFreshness::default()
        };
        assert!(
            should_cache_response(
                200,
                Some("text/markdown"),
                &freshness,
                &CacheScope::Anonymous
            ),
            "Vary: {vary:?} is fixed server policy and must stay cacheable"
        );
    }
}

#[test]
fn should_cache_rejects_vary_accept_mixed_with_unsupported() {
    for vary in [
        "Accept, Authorization",
        "Accept, Cookie",
        "Accept, User-Agent",
    ] {
        let freshness = CacheFreshness {
            max_age: Some(Duration::from_secs(300)),
            vary: Some(vary.into()),
            ..CacheFreshness::default()
        };
        assert!(
            !should_cache_response(200, Some("text/html"), &freshness, &CacheScope::Anonymous),
            "Vary: {vary:?} carries caller input and must stay rejected"
        );
    }
}

#[test]
fn should_cache_rejects_mixed_vary_with_unsupported() {
    let freshness = CacheFreshness {
        max_age: Some(Duration::from_secs(300)),
        vary: Some("Accept-Encoding, Cookie".into()),
        ..CacheFreshness::default()
    };
    assert!(!should_cache_response(
        200,
        Some("text/html"),
        &freshness,
        &CacheScope::Anonymous
    ));
}

#[test]
fn vary_semantics_survive_304_refresh() {
    let empty_validators = CacheValidators {
        etag: None,
        last_modified: None,
    };
    let vary_values = [
        "Accept-Encoding",
        "accept-encoding",
        "ACCEPT-ENCODING",
        "Accept-Encoding, Accept-Encoding",
        "Accept",
        "Accept, Accept-Encoding",
    ];
    for v in vary_values {
        let mut freshness = CacheFreshness::default();
        let mut validators = empty_validators.clone();
        let mut map = HashMap::new();
        map.insert("vary".to_string(), v.to_string());
        map.insert("cache-control".to_string(), "max-age=300".to_string());
        apply_304_headers(&mut freshness, &mut validators, &map);
        assert!(
            should_cache_response(200, Some("text/html"), &freshness, &CacheScope::Anonymous),
            "expected cacheable after 304 refresh for Vary: {v:?}, got vary={:?}",
            freshness.vary
        );
    }

    let reject_values = [
        "Authorization",
        "User-Agent",
        "Accept-Encoding, User-Agent",
        "Cookie",
        "Accept, Cookie",
    ];
    for v in reject_values {
        let mut freshness = CacheFreshness::default();
        let mut validators = empty_validators.clone();
        let mut map = HashMap::new();
        map.insert("vary".to_string(), v.to_string());
        map.insert("cache-control".to_string(), "max-age=300".to_string());
        apply_304_headers(&mut freshness, &mut validators, &map);
        assert!(
            !should_cache_response(200, Some("text/html"), &freshness, &CacheScope::Anonymous),
            "expected rejected after 304 refresh for Vary: {v:?}, got vary={:?}",
            freshness.vary
        );
    }
}

#[test]
fn build_conditional_headers_with_etag() {
    let validators = CacheValidators {
        etag: Some("\"abc\"".into()),
        last_modified: None,
    };
    let headers = build_request_conditional_headers(&validators);
    assert_eq!(headers.len(), 1);
    assert_eq!(headers[0].0, "If-None-Match");
    assert_eq!(headers[0].1, "\"abc\"");
}

#[test]
fn build_conditional_headers_with_last_modified() {
    let validators = CacheValidators {
        etag: None,
        last_modified: Some("Wed, 01 Jan 2025 00:00:00 GMT".into()),
    };
    let headers = build_request_conditional_headers(&validators);
    assert_eq!(headers.len(), 1);
    assert_eq!(headers[0].0, "If-Modified-Since");
}

#[test]
fn build_conditional_headers_etag_takes_precedence() {
    let validators = CacheValidators {
        etag: Some("\"abc\"".into()),
        last_modified: Some("Wed, 01 Jan 2025 00:00:00 GMT".into()),
    };
    let headers = build_request_conditional_headers(&validators);
    assert_eq!(headers.len(), 1);
    assert_eq!(headers[0].0, "If-None-Match");
}

#[test]
fn derived_keys_do_not_share_max_chars_buckets() {
    let low = build_derived_key(
        &CacheScope::Anonymous,
        1,
        ExtractMode::Text,
        1000,
        false,
        None,
        None,
        false,
        false,
    );
    let high = build_derived_key(
        &CacheScope::Anonymous,
        1,
        ExtractMode::Text,
        4096,
        false,
        None,
        None,
        false,
        false,
    );
    assert_ne!(low, high);
}

#[tokio::test]
async fn cache_insert_and_get_raw() {
    let cache = FetchCache::new(10, 10, 1024 * 1024, 1024 * 1024);
    let key = RawCacheKey {
        url: "https://x.com".into(),
        scope: CacheScope::Anonymous,
        accept_policy: crate::fetch::accept::AcceptPolicy::Markdown,
    };
    let entry = RawFetchCacheEntry {
        final_url: "https://x.com".into(),
        status: 200,
        headers: HashMap::new(),
        body: Arc::from(b"hello" as &[u8]),
        fetched_at: SystemTime::now(),
        freshness: CacheFreshness {
            max_age: Some(Duration::from_secs(300)),
            ..CacheFreshness::default()
        },
        validators: CacheValidators {
            etag: None,
            last_modified: None,
        },
        scope: CacheScope::Anonymous,
        content_type: Some("text/html".into()),
        content_length_header: Some(5),
        redirect_count: 0,
        representation: RawRepresentation::Http,
        truncated: false,
        browser_escalated: false,
    };

    assert!(cache.get_raw(&key).await.is_none());
    cache.insert_raw(key.clone(), entry).await;
    assert!(cache.get_raw(&key).await.is_some());
}

#[tokio::test]
async fn cache_evicts_oldest_on_byte_pressure() {
    let cache = FetchCache::new(10, 10, 20, 20);
    for i in 0..5 {
        let key = RawCacheKey {
            url: format!("https://x.com/{i}"),
            scope: CacheScope::Anonymous,
            accept_policy: crate::fetch::accept::AcceptPolicy::Markdown,
        };
        let entry = RawFetchCacheEntry {
            final_url: format!("https://x.com/{i}"),
            status: 200,
            headers: HashMap::new(),
            body: Arc::from(vec![0u8; 10]),
            fetched_at: SystemTime::now(),
            freshness: CacheFreshness {
                max_age: Some(Duration::from_secs(300)),
                ..CacheFreshness::default()
            },
            validators: CacheValidators {
                etag: None,
                last_modified: None,
            },
            scope: CacheScope::Anonymous,
            content_type: Some("text/html".into()),
            content_length_header: Some(10),
            redirect_count: 0,
            representation: RawRepresentation::Http,
            truncated: false,
            browser_escalated: false,
        };
        cache.insert_raw(key, entry).await;
    }
    let stats = cache.stats().await;
    assert!(stats.raw_bytes <= 20);
}

fn make_raw_entry(index: usize, body_len: usize) -> RawFetchCacheEntry {
    RawFetchCacheEntry {
        final_url: format!("https://x.com/{index}"),
        status: 200,
        headers: HashMap::new(),
        body: Arc::from(vec![0u8; body_len]),
        fetched_at: SystemTime::now(),
        freshness: CacheFreshness {
            max_age: Some(Duration::from_secs(300)),
            ..CacheFreshness::default()
        },
        validators: CacheValidators {
            etag: None,
            last_modified: None,
        },
        scope: CacheScope::Anonymous,
        content_type: Some("text/html".into()),
        content_length_header: Some(body_len),
        redirect_count: 0,
        representation: RawRepresentation::Http,
        truncated: false,
        browser_escalated: false,
    }
}

#[tokio::test]
async fn cache_raw_capacity_eviction_keeps_byte_accounting_exact() {
    let cache = FetchCache::new(2, 10, 10_000, 10_000);
    for i in 0..3 {
        cache
            .insert_raw(
                RawCacheKey {
                    url: format!("https://x.com/{i}"),
                    scope: CacheScope::Anonymous,
                    accept_policy: crate::fetch::accept::AcceptPolicy::Markdown,
                },
                make_raw_entry(i, 10),
            )
            .await;
    }

    let stats = cache.stats().await;
    assert_eq!(stats.raw_entries, 2);
    assert_eq!(stats.raw_bytes, 20);
    assert!(cache
        .get_raw(&RawCacheKey {
            url: "https://x.com/0".into(),
            scope: CacheScope::Anonymous,
            accept_policy: crate::fetch::accept::AcceptPolicy::Markdown,
        })
        .await
        .is_none());
}

#[tokio::test]
async fn cache_derived_insert_and_get() {
    let cache = FetchCache::new(10, 10, 1024 * 1024, 1024 * 1024);
    let key = DerivedCacheKey {
        scope: CacheScope::Anonymous,
        raw_content_hash: 12345,
        extraction_key: ExtractionCacheKey {
            extract_mode: ExtractMode::Text,
            max_chars_class: 12000,
            include_links: false,
            pdf_pages: None,
            pdf_ocr: None,
            include_media: false,
            renderer_version: 4,
            sanitize_output: true,
        },
    };
    let entry = DerivedDocumentCacheEntry {
        raw_content_hash: 12345,
        extraction_key: key.extraction_key.clone(),
        response: CachedExtractedDocument {
            title: Some("test".into()),
            description: None,
            text: Some("hello".into()),
            raw_text: Some("hello".into()),
            raw_text_truncated: false,
            raw_text_cap: None,
            links: Vec::new(),
            links_seen: None,
            links_truncated: false,
            truncated: false,
            document: None,
            trust_markers: crate::core::sanitize::TrustMarkers::default(),
            transport: Some("http".into()),
            browser_escalated: false,
            representation: None,
        },
        created_at: SystemTime::now(),
    };

    assert!(cache.get_derived(&key).await.is_none());
    cache.insert_derived(key.clone(), entry).await;
    assert!(cache.get_derived(&key).await.is_some());
}

fn make_derived_entry(raw_content_hash: u64, text_len: usize) -> DerivedDocumentCacheEntry {
    DerivedDocumentCacheEntry {
        raw_content_hash,
        extraction_key: ExtractionCacheKey {
            extract_mode: ExtractMode::Text,
            max_chars_class: 12000,
            include_links: false,
            pdf_pages: None,
            pdf_ocr: None,
            include_media: false,
            renderer_version: 4,
            sanitize_output: true,
        },
        response: CachedExtractedDocument {
            title: None,
            description: None,
            text: Some("x".repeat(text_len)),
            raw_text: Some("y".repeat(text_len)),
            raw_text_truncated: false,
            raw_text_cap: None,
            links: Vec::new(),
            links_seen: None,
            links_truncated: false,
            truncated: false,
            document: None,
            trust_markers: crate::core::sanitize::TrustMarkers::default(),
            transport: Some("http".into()),
            browser_escalated: false,
            representation: None,
        },
        created_at: SystemTime::now(),
    }
}

fn make_derived_key(raw_content_hash: u64) -> DerivedCacheKey {
    DerivedCacheKey {
        scope: CacheScope::Anonymous,
        raw_content_hash,
        extraction_key: ExtractionCacheKey {
            extract_mode: ExtractMode::Text,
            max_chars_class: 12000,
            include_links: false,
            pdf_pages: None,
            pdf_ocr: None,
            include_media: false,
            renderer_version: 4,
            sanitize_output: true,
        },
    }
}

#[tokio::test]
async fn cache_derived_evicts_oldest_on_byte_pressure() {
    let cache = FetchCache::new(10, 10, 500, 800);
    for i in 0..5u64 {
        cache
            .insert_derived(make_derived_key(i), make_derived_entry(i, 100))
            .await;
    }

    let oldest = make_derived_key(0);
    let kept_old = make_derived_key(3);
    let newest = make_derived_key(4);
    assert!(
        cache.get_derived(&oldest).await.is_none(),
        "oldest derived entry should be evicted under byte pressure"
    );
    assert!(cache.get_derived(&kept_old).await.is_some());
    assert!(cache.get_derived(&newest).await.is_some());

    let stats = cache.stats().await;
    assert_eq!(stats.derived_entries, 2);
}

#[tokio::test]
async fn cache_derived_capacity_eviction_keeps_byte_accounting_exact() {
    let cache = FetchCache::new(10, 2, 10_000, 10_000);
    let entry_len = derived_entry_bytes(&make_derived_entry(0, 100));
    for i in 0..3u64 {
        cache
            .insert_derived(make_derived_key(i), make_derived_entry(i, 100))
            .await;
    }

    let stats = cache.stats().await;
    assert_eq!(stats.derived_entries, 2);
    assert_eq!(stats.derived_bytes, entry_len * 2);
    assert!(cache.get_derived(&make_derived_key(0)).await.is_none());
}

#[tokio::test]
async fn cache_derived_oversized_entry_not_stored() {
    let cache = FetchCache::new(10, 10, 64, 64);
    let key = make_derived_key(1);
    cache
        .insert_derived(key.clone(), make_derived_entry(1, 4096))
        .await;
    assert!(cache.get_derived(&key).await.is_none());
    let stats = cache.stats().await;
    assert_eq!(stats.derived_entries, 0);
}

#[tokio::test]
async fn cache_derived_budget_is_independent_of_raw_budget() {
    let cache = FetchCache::new(10, 10, 8, 4096);
    for i in 0..4u64 {
        cache
            .insert_derived(make_derived_key(i), make_derived_entry(i, 100))
            .await;
    }
    for i in 0..4u64 {
        assert!(
            cache.get_derived(&make_derived_key(i)).await.is_some(),
            "derived entry {i} should survive: derived budget is independent of raw budget"
        );
    }
    let stats = cache.stats().await;
    assert_eq!(stats.derived_entries, 4);
}

#[tokio::test]
async fn cache_raw_oversized_entry_not_stored_with_separate_derived_budget() {
    let cache = FetchCache::new(10, 10, 16, 4096);
    let key = RawCacheKey {
        url: "https://x.com".into(),
        scope: CacheScope::Anonymous,
        accept_policy: crate::fetch::accept::AcceptPolicy::Markdown,
    };
    let entry = RawFetchCacheEntry {
        final_url: "https://x.com".into(),
        status: 200,
        headers: HashMap::new(),
        body: Arc::from(vec![0u8; 128]),
        fetched_at: SystemTime::now(),
        freshness: CacheFreshness::default(),
        validators: CacheValidators {
            etag: None,
            last_modified: None,
        },
        scope: CacheScope::Anonymous,
        content_type: Some("text/html".into()),
        content_length_header: Some(128),
        redirect_count: 0,
        representation: RawRepresentation::Http,
        truncated: false,
        browser_escalated: false,
    };
    assert!(!cache.insert_raw(key.clone(), entry).await);
    assert!(cache.get_raw(&key).await.is_none());

    let dkey = make_derived_key(1);
    cache
        .insert_derived(dkey.clone(), make_derived_entry(1, 100))
        .await;
    assert!(cache.get_derived(&dkey).await.is_some());
}

#[test]
fn build_derived_key_distinguishes_modes() {
    let scope = CacheScope::Anonymous;
    let k1 = build_derived_key(
        &scope,
        100,
        ExtractMode::Text,
        12000,
        false,
        None,
        None,
        false,
        true,
    );
    let k2 = build_derived_key(
        &scope,
        100,
        ExtractMode::Markdown,
        12000,
        false,
        None,
        None,
        false,
        true,
    );
    assert_ne!(k1, k2);
}

#[test]
fn derived_key_renderer_version_isolates_pre_encryption_refusal_entries() {
    let current = build_derived_key(
        &CacheScope::Anonymous,
        100,
        ExtractMode::Text,
        12000,
        false,
        None,
        None,
        false,
        true,
    );
    assert_eq!(current.extraction_key.renderer_version, 4);
    let mut legacy = current.clone();
    legacy.extraction_key.renderer_version = 3;
    assert_ne!(
        current, legacy,
        "an entry derived before the encryption refusal changed must not be reused"
    );
}

#[tokio::test]
async fn cache_derived_does_not_serve_pre_encryption_refusal_entries() {
    let cache = FetchCache::new(10, 10, 1_000_000, 1_000_000);
    let mut legacy = make_derived_key(7);
    legacy.extraction_key.renderer_version = 3;
    let mut legacy_entry = make_derived_entry(7, 32);
    legacy_entry.extraction_key = legacy.extraction_key.clone();
    cache.insert_derived(legacy.clone(), legacy_entry).await;
    assert!(
        cache.get_derived(&legacy).await.is_some(),
        "the fixture entry must be retrievable under its own key"
    );

    let current = build_derived_key(
        &CacheScope::Anonymous,
        7,
        ExtractMode::Text,
        12000,
        false,
        None,
        None,
        false,
        true,
    );
    assert_eq!(current.extraction_key.renderer_version, 4);
    assert!(
        cache.get_derived(&current).await.is_none(),
        "a derived entry written before the encryption-refusal bump must not be served"
    );
}

#[test]
fn build_derived_key_distinguishes_pdf_pages() {
    let scope = CacheScope::Anonymous;
    let k1 = build_derived_key(
        &scope,
        100,
        ExtractMode::Text,
        12000,
        false,
        Some("1-3"),
        None,
        false,
        true,
    );
    let k2 = build_derived_key(
        &scope,
        100,
        ExtractMode::Text,
        12000,
        false,
        Some("1-5"),
        None,
        false,
        true,
    );
    assert_ne!(k1, k2);
}

#[test]
fn build_derived_key_distinguishes_max_chars() {
    let scope = CacheScope::Anonymous;
    let k1 = build_derived_key(
        &scope,
        100,
        ExtractMode::Text,
        8000,
        false,
        None,
        None,
        false,
        true,
    );
    let k2 = build_derived_key(
        &scope,
        100,
        ExtractMode::Text,
        10000,
        false,
        None,
        None,
        false,
        true,
    );
    assert_ne!(k1, k2);
}

#[test]
fn stale_entry_with_etag_gets_conditional_headers() {
    let validators = CacheValidators {
        etag: Some("\"abc123\"".into()),
        last_modified: None,
    };
    let headers = build_request_conditional_headers(&validators);
    assert_eq!(headers.len(), 1);
    assert_eq!(headers[0].0, "If-None-Match");
    assert_eq!(headers[0].1, "\"abc123\"");
}

#[test]
fn stale_entry_with_last_modified_gets_conditional_headers() {
    let validators = CacheValidators {
        etag: None,
        last_modified: Some("Wed, 01 Jan 2025 00:00:00 GMT".into()),
    };
    let headers = build_request_conditional_headers(&validators);
    assert_eq!(headers.len(), 1);
    assert_eq!(headers[0].0, "If-Modified-Since");
    assert_eq!(headers[0].1, "Wed, 01 Jan 2025 00:00:00 GMT");
}

#[test]
fn no_validators_means_no_conditional_headers() {
    let validators = CacheValidators {
        etag: None,
        last_modified: None,
    };
    let headers = build_request_conditional_headers(&validators);
    assert!(headers.is_empty());
}

#[tokio::test]
async fn invalidate_scope_evicts_raw_and_derived_entries() {
    let cache = FetchCache::new(10, 10, 1024 * 1024, 1024 * 1024);

    let raw_key = RawCacheKey {
        url: "https://x.com".into(),
        scope: CacheScope::Profile(ProfileId("prof_abc".into())),
        accept_policy: crate::fetch::accept::AcceptPolicy::Markdown,
    };
    let raw_entry = RawFetchCacheEntry {
        final_url: "https://x.com".into(),
        status: 200,
        headers: HashMap::new(),
        body: Arc::from(b"hello" as &[u8]),
        fetched_at: SystemTime::now(),
        freshness: CacheFreshness {
            max_age: Some(Duration::from_secs(300)),
            ..CacheFreshness::default()
        },
        validators: CacheValidators {
            etag: None,
            last_modified: None,
        },
        scope: CacheScope::Profile(ProfileId("prof_abc".into())),
        content_type: Some("text/html".into()),
        content_length_header: Some(5),
        redirect_count: 0,
        representation: RawRepresentation::Http,
        truncated: false,
        browser_escalated: false,
    };
    cache.insert_raw(raw_key.clone(), raw_entry).await;
    cache
        .insert_derived(make_derived_key(1), make_derived_entry(1, 100))
        .await;

    let before = cache.stats().await;
    assert_eq!(before.raw_entries, 1);
    assert_eq!(before.derived_entries, 1);

    cache
        .invalidate_scope(&CacheScope::Profile(ProfileId("prof_abc".into())))
        .await;

    let after = cache.stats().await;
    assert_eq!(after.raw_entries, 0);
    assert_eq!(after.raw_bytes, 0);
    // Derived entries use a different scope and must survive.
    assert_eq!(after.derived_entries, 1);
    assert!(
        cache.get_raw(&raw_key).await.is_none(),
        "scoped raw entries must be evicted"
    );
}

#[test]
fn stale_freshness_is_not_fresh() {
    let freshness = CacheFreshness {
        max_age: Some(std::time::Duration::from_secs(300)),
        fetched_at: Some(std::time::SystemTime::now() - std::time::Duration::from_secs(600)),
        ..CacheFreshness::default()
    };
    assert!(!freshness.is_fresh());
}

#[test]
fn fresh_freshness_is_fresh() {
    let freshness = CacheFreshness {
        max_age: Some(std::time::Duration::from_secs(300)),
        fetched_at: Some(std::time::SystemTime::now()),
        ..CacheFreshness::default()
    };
    assert!(freshness.is_fresh());
}

#[test]
fn no_store_never_fresh() {
    let freshness = CacheFreshness {
        no_store: true,
        max_age: Some(std::time::Duration::from_secs(3600)),
        fetched_at: Some(std::time::SystemTime::now()),
        ..CacheFreshness::default()
    };
    assert!(!freshness.is_fresh());
}
