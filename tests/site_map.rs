use std::sync::Arc;

use eggsearch::core::batch_fetch::{BatchFetchItem, SiteQueryNormalization};
use eggsearch::core::config::AppConfig;
use eggsearch::mcp::state::ServerState;
use eggsearch::mcp::tools::{run_batch_fetch, BatchFetchArgs};
use httpmock::prelude::*;
use serde_json::Value;

fn state_with_localhost() -> Arc<ServerState> {
    let mut config = AppConfig::default();
    config.fetch.allow_localhost = true;
    config.fetch.allow_private_network = true;
    Arc::new(ServerState::build(config).expect("state builds"))
}

async fn discover(
    url: String,
    limit: Option<usize>,
    link_discovery: bool,
    normalize_query: Option<SiteQueryNormalization>,
) -> Value {
    run_batch_fetch(
        state_with_localhost(),
        BatchFetchArgs {
            items: vec![BatchFetchItem::Site {
                url,
                limit,
                include_subdomains: Some(false),
                link_discovery: Some(link_discovery),
                normalize_query,
            }],
            ..Default::default()
        },
    )
    .await
    .expect("site discovery request succeeds")
}

fn origin(server: &MockServer) -> String {
    server.url("/").trim_end_matches('/').to_string()
}

fn site_map(response: &Value) -> &Value {
    &response["results"][0]["response"]["site_map"]
}

#[tokio::test]
async fn site_discovery_is_deterministic_and_never_fetches_discovered_urls() {
    let server = MockServer::start();
    let base = origin(&server);
    let robots_body = include_str!("fixtures/site_map/robots_sitemap.txt")
        .replace("{{SITEMAP_URL}}", &format!("{base}/sitemap.xml"));
    let sitemap_body = include_str!("fixtures/site_map/plain.xml").replace("{{ORIGIN}}", &base);
    let robots = server.mock(|when, then| {
        when.method(GET).path("/robots.txt");
        then.status(200)
            .header("content-type", "text/plain")
            .body(robots_body);
    });
    let sitemap = server.mock(|when, then| {
        when.method(GET).path("/sitemap.xml");
        then.status(200)
            .header("content-type", "application/xml")
            .body(sitemap_body);
    });
    let page = server.mock(|when, then| {
        when.method(GET).path("/docs/");
        then.status(200)
            .header("content-type", "text/html")
            .body("<html><body><a href='/from-map/a'>A</a><a href='/from-page/c?z=1#top'>C</a><a href='/from-map/z?b=2&amp;a=1'>duplicate</a></body></html>");
    });
    let discovered_a = server.mock(|when, then| {
        when.method(GET).path("/from-map/a");
        then.status(200).body("must not be fetched");
    });
    let discovered_c = server.mock(|when, then| {
        when.method(GET).path("/from-page/c");
        then.status(200).body("must not be fetched");
    });
    let discovered_z = server.mock(|when, then| {
        when.method(GET).path("/from-map/z");
        then.status(200).body("must not be fetched");
    });

    let first = discover(
        format!("{base}/docs/"),
        None,
        true,
        Some(SiteQueryNormalization::Keep),
    )
    .await;
    let second = discover(
        format!("{base}/docs/"),
        None,
        true,
        Some(SiteQueryNormalization::Keep),
    )
    .await;
    let map = site_map(&first);
    assert_eq!(map, site_map(&second));
    let urls = map["urls"].as_array().expect("URL inventory");
    let url_values = urls
        .iter()
        .map(|entry| entry["url"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert!(url_values
        .iter()
        .any(|url| url.ends_with("/private/sitemap-only")));
    assert!(url_values
        .iter()
        .any(|url| url.ends_with("/from-map/z?b=2&a=1")));
    assert!(url_values
        .iter()
        .any(|url| url.ends_with("/from-page/c?z=1")));
    assert_eq!(map["counts"]["duplicates"], 1, "site map: {map}");
    assert_eq!(map["counts"]["link"], 2);
    assert_eq!(map["counts"]["sitemap"], 2);
    assert!(!map["truncated"].as_bool().unwrap());
    assert_eq!(robots.calls(), 2);
    assert_eq!(sitemap.calls(), 2);
    assert_eq!(page.calls(), 2);
    assert_eq!(discovered_a.calls(), 0);
    assert_eq!(discovered_c.calls(), 0);
    assert_eq!(discovered_z.calls(), 0);
}

#[tokio::test]
async fn site_discovery_follows_one_sitemap_index_level_and_fallbacks() {
    let server = MockServer::start();
    let base = origin(&server);
    let robots = server.mock(|when, then| {
        when.method(GET).path("/robots.txt");
        then.status(404);
    });
    let index_body = include_str!("fixtures/site_map/index.xml").replace("{{ORIGIN}}", &base);
    let index = server.mock(|when, then| {
        when.method(GET).path("/sitemap.xml");
        then.status(200)
            .header("content-type", "application/xml")
            .body(index_body);
    });
    let alternate = server.mock(|when, then| {
        when.method(GET).path("/sitemap_index.xml");
        then.status(404);
    });
    let child_body = include_str!("fixtures/site_map/child.xml").replace("{{ORIGIN}}", &base);
    let child = server.mock(|when, then| {
        when.method(GET).path("/child.xml");
        then.status(200)
            .header("content-type", "application/xml")
            .body(child_body);
    });
    let target = server.mock(|when, then| {
        when.method(GET).path("/child-target");
        then.status(200).body("must not be fetched");
    });

    let response = discover(format!("{base}/entry"), None, false, None).await;
    let map = site_map(&response);
    assert_eq!(map["sitemap_index_used"], true);
    assert_eq!(map["robots_sitemaps"], 0);
    assert_eq!(map["urls"][0]["url"], format!("{base}/child-target"));
    assert_eq!(robots.calls(), 1);
    assert_eq!(index.calls(), 1);
    assert_eq!(alternate.calls(), 1);
    assert_eq!(child.calls(), 1);
    assert_eq!(target.calls(), 0);
}

#[tokio::test]
async fn site_discovery_does_not_recurse_into_a_child_index() {
    let server = MockServer::start();
    let base = origin(&server);
    let robots = server.mock(|when, then| {
        when.method(GET).path("/robots.txt");
        then.status(404);
    });
    let index_body = include_str!("fixtures/site_map/index.xml")
        .replace("/child.xml", "/child-index.xml")
        .replace("{{ORIGIN}}", &base);
    let index = server.mock(|when, then| {
        when.method(GET).path("/sitemap.xml");
        then.status(200)
            .header("content-type", "application/xml")
            .body(index_body);
    });
    let alternate = server.mock(|when, then| {
        when.method(GET).path("/sitemap_index.xml");
        then.status(404);
    });
    let child_index_body =
        include_str!("fixtures/site_map/child_index.xml").replace("{{ORIGIN}}", &base);
    let child_index = server.mock(|when, then| {
        when.method(GET).path("/child-index.xml");
        then.status(200)
            .header("content-type", "application/xml")
            .body(child_index_body);
    });
    let grandchild = server.mock(|when, then| {
        when.method(GET).path("/grandchild.xml");
        then.status(200).body("must not be fetched");
    });

    let response = discover(format!("{base}/entry"), None, false, None).await;
    let map = site_map(&response);
    assert_eq!(map["sitemap_index_used"], true);
    assert_eq!(map["truncated"], true);
    assert_eq!(robots.calls(), 1);
    assert_eq!(index.calls(), 1);
    assert_eq!(alternate.calls(), 1);
    assert_eq!(child_index.calls(), 1);
    assert_eq!(grandchild.calls(), 0);
}

#[tokio::test]
async fn site_discovery_rejects_cross_origin_targets_and_malformed_xml() {
    let server = MockServer::start();
    let other = MockServer::start();
    let base = origin(&server);
    let other_base = origin(&other);
    let robots_body = include_str!("fixtures/site_map/robots_two_sitemaps.txt")
        .replace("{{SITEMAP_URL}}", &format!("{base}/sitemap.xml"))
        .replace(
            "{{CROSS_SITEMAP_URL}}",
            &format!("{other_base}/external.xml"),
        );
    let robots = server.mock(|when, then| {
        when.method(GET).path("/robots.txt");
        then.status(200)
            .header("content-type", "text/plain")
            .body(robots_body);
    });
    let cross_body =
        include_str!("fixtures/site_map/cross_origin.xml").replace("{{OTHER_ORIGIN}}", &other_base);
    let sitemap = server.mock(|when, then| {
        when.method(GET).path("/sitemap.xml");
        then.status(200)
            .header("content-type", "application/xml")
            .body(cross_body);
    });
    let external_sitemap = other.mock(|when, then| {
        when.method(GET).path("/external.xml");
        then.status(200).body("must not be fetched");
    });
    let external_page = other.mock(|when, then| {
        when.method(GET).path("/not-contacted");
        then.status(200).body("must not be fetched");
    });
    let entry = server.mock(|when, then| {
        when.method(GET).path("/entry");
        then.status(200)
            .body("must not be fetched when link discovery is disabled");
    });

    let response = discover(format!("{base}/entry"), None, false, None).await;
    let map = site_map(&response);
    assert_eq!(map["counts"]["cross_origin_skipped"], 2);
    assert_eq!(map["counts"]["total"], 0);
    assert_eq!(robots.calls(), 1);
    assert_eq!(sitemap.calls(), 1);
    assert_eq!(external_sitemap.calls(), 0);
    assert_eq!(external_page.calls(), 0);
    assert_eq!(entry.calls(), 0);

    let malformed_server = MockServer::start();
    let malformed_base = origin(&malformed_server);
    let malformed_robots_body = include_str!("fixtures/site_map/robots_sitemap.txt")
        .replace("{{SITEMAP_URL}}", &format!("{malformed_base}/sitemap.xml"));
    malformed_server.mock(|when, then| {
        when.method(GET).path("/robots.txt");
        then.status(200).body(malformed_robots_body);
    });
    let malformed_body =
        include_str!("fixtures/site_map/malformed.xml").replace("{{ORIGIN}}", &malformed_base);
    malformed_server.mock(|when, then| {
        when.method(GET).path("/sitemap.xml");
        then.status(200)
            .header("content-type", "application/xml")
            .body(malformed_body);
    });
    let malformed_response = discover(format!("{malformed_base}/entry"), None, false, None).await;
    assert_eq!(malformed_response["results"][0]["ok"], false);
    assert!(malformed_response["results"][0]["error"]
        .as_str()
        .unwrap()
        .contains("site_map_malformed"));
}

#[tokio::test]
async fn site_discovery_refuses_gzip_and_reports_url_limit_and_xml_depth_truncation() {
    let gzip_server = MockServer::start();
    let gzip_base = origin(&gzip_server);
    let gzip_robots_body = include_str!("fixtures/site_map/robots_gzip.txt")
        .replace("{{SITEMAP_URL}}", &format!("{gzip_base}/sitemap.xml.gz"));
    gzip_server.mock(|when, then| {
        when.method(GET).path("/robots.txt");
        then.status(200).body(gzip_robots_body);
    });
    let gzip = gzip_server.mock(|when, then| {
        when.method(GET).path("/sitemap.xml.gz");
        then.status(200).body("must not be fetched");
    });
    let gzip_response = discover(format!("{gzip_base}/entry"), None, false, None).await;
    assert!(gzip_response["results"][0]["error"]
        .as_str()
        .unwrap()
        .contains("site_map_gzip_unsupported"));
    assert_eq!(gzip.calls(), 0);

    let limit_server = MockServer::start();
    let limit_base = origin(&limit_server);
    let limit_robots_body = include_str!("fixtures/site_map/robots_sitemap.txt")
        .replace("{{SITEMAP_URL}}", &format!("{limit_base}/sitemap.xml"));
    limit_server.mock(|when, then| {
        when.method(GET).path("/robots.txt");
        then.status(200).body(limit_robots_body);
    });
    let limit_body =
        include_str!("fixtures/site_map/over_limit.xml").replace("{{ORIGIN}}", &limit_base);
    limit_server.mock(|when, then| {
        when.method(GET).path("/sitemap.xml");
        then.status(200)
            .header("content-type", "application/xml")
            .body(limit_body);
    });
    let limit_response = discover(format!("{limit_base}/entry"), Some(2), false, None).await;
    let limited = site_map(&limit_response);
    assert_eq!(limited["counts"]["total"], 2);
    assert_eq!(limited["truncated"], true);

    let depth_server = MockServer::start();
    let depth_base = origin(&depth_server);
    let depth_robots_body = include_str!("fixtures/site_map/robots_sitemap.txt")
        .replace("{{SITEMAP_URL}}", &format!("{depth_base}/sitemap.xml"));
    depth_server.mock(|when, then| {
        when.method(GET).path("/robots.txt");
        then.status(200).body(depth_robots_body);
    });
    let mut deep_xml = String::from("<urlset>");
    for _ in 0..32 {
        deep_xml.push_str("<node>");
    }
    for _ in 0..32 {
        deep_xml.push_str("</node>");
    }
    deep_xml.push_str("</urlset>");
    depth_server.mock(|when, then| {
        when.method(GET).path("/sitemap.xml");
        then.status(200)
            .header("content-type", "application/xml")
            .body(deep_xml);
    });
    let depth_response = discover(format!("{depth_base}/entry"), None, false, None).await;
    assert_eq!(site_map(&depth_response)["truncated"], true);
}

#[tokio::test]
async fn site_discovery_query_drop_collapses_duplicate_urls() {
    let server = MockServer::start();
    let base = origin(&server);
    let robots_body = include_str!("fixtures/site_map/robots_sitemap.txt")
        .replace("{{SITEMAP_URL}}", &format!("{base}/sitemap.xml"));
    server.mock(|when, then| {
        when.method(GET).path("/robots.txt");
        then.status(200).body(robots_body);
    });
    let duplicate_body =
        include_str!("fixtures/site_map/duplicates.xml").replace("{{ORIGIN}}", &base);
    server.mock(|when, then| {
        when.method(GET).path("/sitemap.xml");
        then.status(200)
            .header("content-type", "application/xml")
            .body(duplicate_body);
    });
    let target = server.mock(|when, then| {
        when.method(GET).path("/duplicate");
        then.status(200).body("must not be fetched");
    });

    let response = discover(
        format!("{base}/entry"),
        None,
        false,
        Some(SiteQueryNormalization::Drop),
    )
    .await;
    let map = site_map(&response);
    assert_eq!(map["urls"].as_array().unwrap().len(), 1);
    assert_eq!(map["urls"][0]["url"], format!("{base}/duplicate"));
    assert_eq!(map["counts"]["duplicates"], 1);
    assert_eq!(target.calls(), 0);
}

#[tokio::test]
async fn site_discovery_refuses_a_cross_origin_sitemap_redirect() {
    let server = MockServer::start();
    let base = origin(&server);
    let elsewhere = MockServer::start();
    let robots_body = include_str!("fixtures/site_map/robots_sitemap.txt")
        .replace("{{SITEMAP_URL}}", &format!("{base}/sitemap.xml"));
    let robots = server.mock(|when, then| {
        when.method(GET).path("/robots.txt");
        then.status(200)
            .header("content-type", "text/plain")
            .body(robots_body);
    });
    let redirect = server.mock(|when, then| {
        when.method(GET).path("/sitemap.xml");
        then.status(302)
            .header("location", elsewhere.url("/sitemap.xml"));
    });
    let crossed = elsewhere.mock(|when, then| {
        when.method(GET).path("/sitemap.xml");
        then.status(200)
            .header("content-type", "application/xml")
            .body("<urlset></urlset>");
    });
    let response = discover(format!("{base}/entry"), None, false, None).await;
    assert_eq!(response["results"][0]["ok"], false);
    assert!(response["results"][0]["error"]
        .as_str()
        .unwrap()
        .contains("site_map_cross_origin_redirect"));
    assert_eq!(robots.calls(), 1);
    assert_eq!(redirect.calls(), 1);
    assert_eq!(crossed.calls(), 0);
}
