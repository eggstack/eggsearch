use std::sync::Arc;

use eggsearch::core::config::AppConfig;
use eggsearch::mcp::state::ServerState;
#[cfg(not(feature = "documents"))]
use eggsearch::mcp::tools::{run_provider_status, ProviderStatusArgs};
use eggsearch::mcp::tools::{run_web_fetch, WebFetchArgs};
use httpmock::prelude::*;

const DOCX_CONTENT_TYPE: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document";
#[cfg(feature = "documents")]
const EPUB_CONTENT_TYPE: &str = "application/epub+zip";

fn state() -> Arc<ServerState> {
    let mut config = AppConfig::default();
    config.fetch.allow_localhost = true;
    config.fetch.allow_private_network = true;
    Arc::new(ServerState::build(config).expect("state builds"))
}

async fn fetch(url: String) -> serde_json::Value {
    run_web_fetch(
        state(),
        WebFetchArgs {
            url,
            max_chars: Some(12_000),
            ..Default::default()
        },
    )
    .await
    .expect("fetch succeeds")
}

fn error_text(error: &eggsearch::mcp::tools::ToolError) -> String {
    error.error_payload().to_string()
}

#[cfg(not(feature = "documents"))]
#[tokio::test]
async fn docx_without_the_feature_keeps_the_unsupported_outcome() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/report.docx");
        then.status(200)
            .header("content-type", DOCX_CONTENT_TYPE)
            .body(b"PK\x03\x04placeholder".as_slice());
    });
    let error = run_web_fetch(
        state(),
        WebFetchArgs {
            url: server.url("/report.docx"),
            ..Default::default()
        },
    )
    .await
    .expect_err("a container document is unsupported without the feature");
    assert!(error_text(&error).contains("unsupported_content_type"));
}

#[cfg(not(feature = "documents"))]
#[tokio::test]
async fn capability_report_is_honest_without_the_feature() {
    let mut config = AppConfig::default();
    config.fetch.documents_enabled = true;
    let state = Arc::new(ServerState::build(config).expect("state builds"));
    let status = run_provider_status(
        state,
        ProviderStatusArgs {
            probe: false,
            recipe_detail: None,
        },
    )
    .expect("provider status");
    assert_eq!(status["documents_capabilities"]["compiled"], false);
    assert_eq!(status["documents_capabilities"]["configured"], false);
    assert_eq!(status["documents_capabilities"]["usable"], false);
    assert_eq!(status["server_capabilities"]["container_documents"], false);
}

#[cfg(not(feature = "documents"))]
#[tokio::test]
async fn csv_without_the_feature_keeps_the_table_preview() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/table.csv");
        then.status(200)
            .header("content-type", "text/csv")
            .body("name,value\nalpha,1\nbeta,2");
    });
    let response = fetch(server.url("/table.csv")).await;
    assert_eq!(response["document"]["kind"], "csv");
    assert!(response["document"].get("format").is_none());
    assert!(response["document"].get("parts").is_none());
}

#[cfg(feature = "documents")]
fn archive(entries: &[(&str, &str)]) -> Vec<u8> {
    use std::io::{Cursor, Write};
    use zip::write::SimpleFileOptions;
    use zip::{CompressionMethod, ZipWriter};

    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, text) in entries {
        let options = if *name == "mimetype" {
            SimpleFileOptions::default().compression_method(CompressionMethod::Stored)
        } else {
            SimpleFileOptions::default().compression_method(CompressionMethod::Deflated)
        };
        writer.start_file(*name, options).unwrap();
        writer.write_all(text.as_bytes()).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

#[cfg(feature = "documents")]
fn docx_with_sections() -> Vec<u8> {
    archive(&[(
        "word/document.xml",
        r#"<w:document xmlns:w="urn:w"><w:body><w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Overview</w:t></w:r></w:p><w:p><w:r><w:t>Intro text</w:t></w:r></w:p><w:p><w:pPr><w:pStyle w:val="Heading2"/></w:pPr><w:r><w:t>Details</w:t></w:r></w:p><w:p><w:r><w:t>More text</w:t></w:r></w:p></w:body></w:document>"#,
    )])
}

#[cfg(feature = "documents")]
fn epub_with_two_chapters() -> Vec<u8> {
    archive(&[
        ("mimetype", "application/epub+zip"),
        (
            "META-INF/container.xml",
            r#"<container><rootfiles><rootfile full-path="OPS/book.opf"/></rootfiles></container>"#,
        ),
        (
            "OPS/book.opf",
            r#"<package><manifest><item id="one" href="one.xhtml"/><item id="two" href="two.xhtml"/></manifest><spine><itemref idref="two"/><itemref idref="one"/></spine></package>"#,
        ),
        (
            "OPS/one.xhtml",
            r#"<html><body><h1>First chapter</h1><p>One text</p></body></html>"#,
        ),
        (
            "OPS/two.xhtml",
            r#"<html><body><h1>Second chapter</h1><p>Two text</p></body></html>"#,
        ),
    ])
}

#[cfg(feature = "documents")]
#[tokio::test]
async fn docx_url_returns_ordered_parts_deterministically() {
    let server = MockServer::start();
    let body = docx_with_sections();
    server.mock(|when, then| {
        when.method(GET).path("/report.docx");
        then.status(200)
            .header("content-type", DOCX_CONTENT_TYPE)
            .body(body.as_slice());
    });
    let url = server.url("/report.docx");
    let first = fetch(url.clone()).await;
    let second = fetch(url).await;

    assert_eq!(first["document"]["format"], "docx");
    assert_eq!(first["document"]["parts_total"], 2);
    let parts = first["document"]["parts"].as_array().unwrap();
    assert_eq!(parts.len(), 2);
    assert_eq!(parts[0]["index"], 1);
    assert_eq!(parts[0]["kind"], "section");
    assert!(parts[0]["title"].as_str().unwrap().contains("Overview"));
    assert!(parts[0]["text"].as_str().unwrap().contains("Intro text"));
    assert_eq!(parts[1]["index"], 2);
    assert!(parts[1]["title"].as_str().unwrap().contains("Details"));
    assert!(parts[1]["text"].as_str().unwrap().contains("More text"));
    assert_eq!(first["document"]["parts"], second["document"]["parts"]);
}

#[cfg(feature = "documents")]
#[tokio::test]
async fn epub_url_returns_parts_in_spine_order() {
    let server = MockServer::start();
    let body = epub_with_two_chapters();
    server.mock(|when, then| {
        when.method(GET).path("/book.epub");
        then.status(200)
            .header("content-type", EPUB_CONTENT_TYPE)
            .body(body.as_slice());
    });
    let response = fetch(server.url("/book.epub")).await;
    assert_eq!(response["document"]["format"], "epub");
    assert_eq!(response["document"]["parts_total"], 2);
    let parts = response["document"]["parts"].as_array().unwrap();
    assert_eq!(parts[0]["kind"], "spine_item");
    assert!(parts[0]["title"]
        .as_str()
        .unwrap()
        .contains("Second chapter"));
    assert!(parts[0]["text"].as_str().unwrap().contains("Two text"));
    assert!(parts[1]["title"]
        .as_str()
        .unwrap()
        .contains("First chapter"));
    assert!(parts[1]["text"].as_str().unwrap().contains("One text"));
}

#[cfg(feature = "documents")]
#[tokio::test]
async fn csv_url_returns_fixed_row_blocks() {
    let server = MockServer::start();
    let mut text = String::from("name,value");
    for row in 1..=150 {
        text.push('\n');
        text.push_str(&format!("name{row},value{row}"));
    }
    server.mock(|when, then| {
        when.method(GET).path("/table.csv");
        then.status(200)
            .header("content-type", "text/csv")
            .body(text.as_str());
    });
    let response = fetch(server.url("/table.csv")).await;
    assert_eq!(response["document"]["format"], "csv");
    assert_eq!(response["document"]["parts_total"], 2);
    let parts = response["document"]["parts"].as_array().unwrap();
    assert_eq!(parts[0]["kind"], "table_block");
    assert!(parts[0]["title"].as_str().unwrap().contains("rows 1-100"));
    let first_block = parts[0]["text"].as_str().unwrap();
    assert!(first_block.contains("name1"));
    assert!(first_block.contains("value1"));
    assert!(parts[1]["title"].as_str().unwrap().contains("rows 101-151"));
}

#[cfg(feature = "documents")]
#[tokio::test]
async fn disabled_setting_refuses_container_extraction() {
    let server = MockServer::start();
    let body = docx_with_sections();
    server.mock(|when, then| {
        when.method(GET).path("/report.docx");
        then.status(200)
            .header("content-type", DOCX_CONTENT_TYPE)
            .body(body.as_slice());
    });
    let mut config = AppConfig::default();
    config.fetch.allow_localhost = true;
    config.fetch.allow_private_network = true;
    config.fetch.documents_enabled = false;
    let state = Arc::new(ServerState::build(config).expect("state builds"));
    let error = run_web_fetch(
        state,
        WebFetchArgs {
            url: server.url("/report.docx"),
            ..Default::default()
        },
    )
    .await
    .expect_err("a disabled setting must refuse container extraction");
    assert!(error_text(&error).contains("document_disabled"));
}

#[cfg(feature = "documents")]
#[tokio::test]
async fn extensionless_paths_are_not_legacy_document_formats() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/doc");
        then.status(200)
            .header("content-type", "text/html; charset=utf-8")
            .body("<html><body><p>Ordinary page</p></body></html>");
    });
    let response = fetch(server.url("/doc")).await;
    assert!(response["text"].as_str().unwrap().contains("Ordinary page"));
    assert!(response["document"].get("format").is_none());
}

#[cfg(feature = "documents")]
#[tokio::test]
async fn legacy_binary_office_is_unsupported() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/old.doc");
        then.status(200)
            .header("content-type", "application/msword")
            .body(b"legacy body".as_slice());
    });
    let error = run_web_fetch(
        state(),
        WebFetchArgs {
            url: server.url("/old.doc"),
            ..Default::default()
        },
    )
    .await
    .expect_err("legacy binary Office is unsupported");
    assert!(error_text(&error).contains("document_unsupported"));
}

#[cfg(feature = "documents")]
#[tokio::test]
async fn encrypted_container_is_distinguishable() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/secret.docx");
        then.status(200)
            .header("content-type", DOCX_CONTENT_TYPE)
            .body(b"\xd0\xcf\x11\xe0\xa1\xb1\x1a\xe1padding".as_slice());
    });
    let error = run_web_fetch(
        state(),
        WebFetchArgs {
            url: server.url("/secret.docx"),
            ..Default::default()
        },
    )
    .await
    .expect_err("an encrypted container is refused");
    assert!(error_text(&error).contains("document_encrypted"));
}

#[cfg(feature = "documents")]
#[tokio::test]
async fn malformed_container_is_distinguishable() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/broken.docx");
        then.status(200)
            .header("content-type", DOCX_CONTENT_TYPE)
            .body(b"PK\x03\x04not-a-real-archive".as_slice());
    });
    let error = run_web_fetch(
        state(),
        WebFetchArgs {
            url: server.url("/broken.docx"),
            ..Default::default()
        },
    )
    .await
    .expect_err("a malformed container is refused");
    assert!(error_text(&error).contains("document_malformed"));
}
