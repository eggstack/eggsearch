use std::path::Path;

fn corpus_dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("corpus")
        .join("adversarial")
}

fn load_and_validate(path: &Path) -> serde_json::Value {
    let content = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {}", path.display(), e));
    let value: serde_json::Value = serde_json::from_str(&content)
        .unwrap_or_else(|e| panic!("Invalid JSON in {}: {}", path.display(), e));

    let cases = value.get("cases").unwrap_or_else(|| {
        panic!(
            "Missing 'cases' array in {}",
            path.file_name().unwrap().to_string_lossy()
        )
    });
    let arr = cases.as_array().unwrap_or_else(|| {
        panic!(
            "'cases' is not an array in {}",
            path.file_name().unwrap().to_string_lossy()
        )
    });
    assert!(
        !arr.is_empty(),
        "'cases' is empty in {}",
        path.file_name().unwrap().to_string_lossy()
    );

    value
}

#[test]
fn adversarial_html_malformed_is_valid() {
    let path = corpus_dir().join("html_malformed.json");
    let value = load_and_validate(&path);
    let cases = value["cases"].as_array().unwrap();
    assert!(
        cases.len() >= 15,
        "html_malformed.json has only {} cases, expected >= 15",
        cases.len()
    );
    for (i, case) in cases.iter().enumerate() {
        assert!(
            case.get("input").is_some(),
            "Case {i} in html_malformed.json missing 'input'"
        );
        assert!(
            case.get("description").is_some(),
            "Case {i} in html_malformed.json missing 'description'"
        );
    }
    eprintln!("html_malformed.json: {} cases OK", cases.len());
}

#[test]
fn adversarial_structured_text_is_valid() {
    let path = corpus_dir().join("structured_text.json");
    let value = load_and_validate(&path);
    let cases = value["cases"].as_array().unwrap();
    assert!(
        cases.len() >= 15,
        "structured_text.json has only {} cases, expected >= 15",
        cases.len()
    );
    for (i, case) in cases.iter().enumerate() {
        assert!(
            case.get("input").is_some(),
            "Case {i} in structured_text.json missing 'input'"
        );
        assert!(
            case.get("description").is_some(),
            "Case {i} in structured_text.json missing 'description'"
        );
        assert!(
            case.get("format").is_some(),
            "Case {i} in structured_text.json missing 'format'"
        );
    }
    eprintln!("structured_text.json: {} cases OK", cases.len());
}

#[test]
fn adversarial_url_edge_cases_is_valid() {
    let path = corpus_dir().join("url_edge_cases.json");
    let value = load_and_validate(&path);
    let cases = value["cases"].as_array().unwrap();
    assert!(
        cases.len() >= 20,
        "url_edge_cases.json has only {} cases, expected >= 20",
        cases.len()
    );
    for (i, case) in cases.iter().enumerate() {
        assert!(
            case.get("input").is_some(),
            "Case {i} in url_edge_cases.json missing 'input'"
        );
        assert!(
            case.get("description").is_some(),
            "Case {i} in url_edge_cases.json missing 'description'"
        );
        let expected = case
            .get("expected")
            .and_then(|v| v.as_str())
            .unwrap_or_else(|| panic!("Case {i} in url_edge_cases.json missing 'expected'"));
        assert!(
            expected == "reject" || expected == "allow",
            "Case {i} in url_edge_cases.json has invalid 'expected' value: {expected}"
        );
    }
    eprintln!("url_edge_cases.json: {} cases OK", cases.len());
}

#[test]
fn adversarial_sanitize_edge_cases_is_valid() {
    let path = corpus_dir().join("sanitize_edge_cases.json");
    let value = load_and_validate(&path);
    let cases = value["cases"].as_array().unwrap();
    assert!(
        cases.len() >= 15,
        "sanitize_edge_cases.json has only {} cases, expected >= 15",
        cases.len()
    );
    for (i, case) in cases.iter().enumerate() {
        assert!(
            case.get("input").is_some(),
            "Case {i} in sanitize_edge_cases.json missing 'input'"
        );
        assert!(
            case.get("description").is_some(),
            "Case {i} in sanitize_edge_cases.json missing 'description'"
        );
        assert!(
            case.get("category").is_some(),
            "Case {i} in sanitize_edge_cases.json missing 'category'"
        );
    }
    eprintln!("sanitize_edge_cases.json: {} cases OK", cases.len());
}

#[test]
fn adversarial_identity_edge_cases_is_valid() {
    let path = corpus_dir().join("identity_edge_cases.json");
    let value = load_and_validate(&path);
    let cases = value["cases"].as_array().unwrap();
    assert!(
        cases.len() >= 10,
        "identity_edge_cases.json has only {} cases, expected >= 10",
        cases.len()
    );
    for (i, case) in cases.iter().enumerate() {
        assert!(
            case.get("input").is_some(),
            "Case {i} in identity_edge_cases.json missing 'input'"
        );
        assert!(
            case.get("description").is_some(),
            "Case {i} in identity_edge_cases.json missing 'description'"
        );
        assert!(
            case.get("category").is_some(),
            "Case {i} in identity_edge_cases.json missing 'category'"
        );
    }
    eprintln!("identity_edge_cases.json: {} cases OK", cases.len());
}

#[test]
fn adversarial_html_extended_is_valid() {
    let path = corpus_dir().join("html_extended.json");
    let value = load_and_validate(&path);
    let cases = value["cases"].as_array().unwrap();
    assert!(
        cases.len() >= 15,
        "html_extended.json has only {} cases, expected >= 15",
        cases.len()
    );
    for (i, case) in cases.iter().enumerate() {
        assert!(
            case.get("id").is_some(),
            "Case {i} in html_extended.json missing 'id'"
        );
        assert!(
            case.get("input").is_some(),
            "Case {i} in html_extended.json missing 'input'"
        );
        assert!(
            case.get("description").is_some(),
            "Case {i} in html_extended.json missing 'description'"
        );
    }
    eprintln!("html_extended.json: {} cases OK", cases.len());
}

#[test]
fn adversarial_structured_text_extended_is_valid() {
    let path = corpus_dir().join("structured_text_extended.json");
    let value = load_and_validate(&path);
    let cases = value["cases"].as_array().unwrap();
    assert!(
        cases.len() >= 15,
        "structured_text_extended.json has only {} cases, expected >= 15",
        cases.len()
    );
    for (i, case) in cases.iter().enumerate() {
        assert!(
            case.get("id").is_some(),
            "Case {i} in structured_text_extended.json missing 'id'"
        );
        assert!(
            case.get("input").is_some(),
            "Case {i} in structured_text_extended.json missing 'input'"
        );
        assert!(
            case.get("description").is_some(),
            "Case {i} in structured_text_extended.json missing 'description'"
        );
    }
    eprintln!("structured_text_extended.json: {} cases OK", cases.len());
}

#[test]
fn adversarial_pdf_extended_is_valid() {
    let path = corpus_dir().join("pdf_extended.json");
    let value = load_and_validate(&path);
    let cases = value["cases"].as_array().unwrap();
    assert!(
        cases.len() >= 10,
        "pdf_extended.json has only {} cases, expected >= 10",
        cases.len()
    );
    for (i, case) in cases.iter().enumerate() {
        assert!(
            case.get("id").is_some(),
            "Case {i} in pdf_extended.json missing 'id'"
        );
        assert!(
            case.get("input").is_some(),
            "Case {i} in pdf_extended.json missing 'input'"
        );
        assert!(
            case.get("description").is_some(),
            "Case {i} in pdf_extended.json missing 'description'"
        );
    }
    eprintln!("pdf_extended.json: {} cases OK", cases.len());
}

#[test]
fn adversarial_filesystem_extended_is_valid() {
    let path = corpus_dir().join("filesystem_extended.json");
    let value = load_and_validate(&path);
    let cases = value["cases"].as_array().unwrap();
    assert!(
        cases.len() >= 10,
        "filesystem_extended.json has only {} cases, expected >= 10",
        cases.len()
    );
    for (i, case) in cases.iter().enumerate() {
        assert!(
            case.get("id").is_some(),
            "Case {i} in filesystem_extended.json missing 'id'"
        );
        assert!(
            case.get("input").is_some(),
            "Case {i} in filesystem_extended.json missing 'input'"
        );
        assert!(
            case.get("description").is_some(),
            "Case {i} in filesystem_extended.json missing 'description'"
        );
        assert!(
            case.get("category").is_some(),
            "Case {i} in filesystem_extended.json missing 'category'"
        );
    }
    eprintln!("filesystem_extended.json: {} cases OK", cases.len());
}

#[test]
fn all_corpus_total_case_count() {
    let dir = corpus_dir();
    let entries: Vec<_> = std::fs::read_dir(&dir)
        .expect("Failed to read adversarial corpus directory")
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map(|ext| ext == "json")
                .unwrap_or(false)
        })
        .collect();

    let mut total = 0;
    for entry in &entries {
        let content = std::fs::read_to_string(entry.path())
            .unwrap_or_else(|e| panic!("Failed to read {}: {}", entry.path().display(), e));
        let value: serde_json::Value = serde_json::from_str(&content)
            .unwrap_or_else(|e| panic!("Invalid JSON in {}: {}", entry.path().display(), e));
        if let Some(cases) = value.get("cases").and_then(|v| v.as_array()) {
            total += cases.len();
        }
    }
    eprintln!(
        "Total adversarial corpus cases across {} files: {}",
        entries.len(),
        total
    );
    assert!(
        total >= 100,
        "Total corpus cases should be >= 100, got {total}"
    );
}

fn load_corpus_cases(filename: &str) -> Vec<serde_json::Value> {
    let path = corpus_dir().join(filename);
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {}", path.display(), e));
    let value: serde_json::Value = serde_json::from_str(&content)
        .unwrap_or_else(|e| panic!("Invalid JSON in {}: {}", path.display(), e));
    value["cases"].as_array().cloned().unwrap_or_default()
}

#[test]
fn html_corpus_exercises_extract_content() {
    let cases = load_corpus_cases("html_malformed.json");
    let cases_ext = load_corpus_cases("html_extended.json");
    let all: Vec<_> = cases.into_iter().chain(cases_ext).collect();
    assert!(!all.is_empty(), "need at least one HTML corpus case");
    for case in &all {
        let input = case["input"].as_str().expect("case must have 'input'");
        let result =
            eggsearch::fetch::extract_content(input.as_bytes(), "http://example.com", 10000, false);
        let _ = result;
    }
}

#[test]
fn structured_text_corpus_exercises_render_code() {
    let cases = load_corpus_cases("structured_text.json");
    let cases_ext = load_corpus_cases("structured_text_extended.json");
    let all: Vec<_> = cases.into_iter().chain(cases_ext).collect();
    assert!(
        !all.is_empty(),
        "need at least one structured text corpus case"
    );
    for case in &all {
        let input = case["input"].as_str().expect("case must have 'input'");
        let format = case["format"].as_str().unwrap_or("text");
        let result = match format {
            "diff" | "patch" => eggsearch::fetch::render::render_diff(input, 10000),
            "csv" => eggsearch::fetch::render::render_csv(input, 10000),
            _ => eggsearch::fetch::render::render_code(input, None, 10000),
        };
        assert!(
            !result.blocks.is_empty() || input.trim().is_empty(),
            "rendered blocks should not be empty for non-empty input (format={format})"
        );
    }
}

#[cfg(feature = "pdf")]
#[test]
fn pdf_corpus_exercises_extract_pdf_text() {
    let cases = load_corpus_cases("pdf_extended.json");
    assert!(!cases.is_empty(), "need at least one PDF corpus case");
    let limits = eggsearch::fetch::pdf::PdfLimits {
        max_pages: 25,
        max_chars_per_page: 12000,
        max_total_chars: 50000,
        max_decompressed_bytes: 67_108_864,
    };
    for case in &cases {
        let input = case["input"].as_str().expect("case must have 'input'");
        let _ = eggsearch::fetch::pdf::extract_pdf_text(input.as_bytes(), 10000, &limits, None);
    }
}

#[test]
fn sanitize_corpus_exercises_pipeline() {
    let cases = load_corpus_cases("sanitize_edge_cases.json");
    assert!(!cases.is_empty(), "need at least one sanitize corpus case");
    for case in &cases {
        let input = case["input"].as_str().expect("case must have 'input'");
        let (cleaned, _) = eggsearch::core::sanitize::strip_control_chars(input);
        let (bounded, _) = eggsearch::core::sanitize::bound_text(&cleaned, 5000);
        let _ = eggsearch::core::sanitize::scan_injection_markers(&bounded);
    }
}

#[test]
fn identity_corpus_exercises_canonicalize() {
    let cases = load_corpus_cases("identity_edge_cases.json");
    assert!(!cases.is_empty(), "need at least one identity corpus case");
    for case in &cases {
        let input = case["input"].as_str().expect("case must have 'input'");
        let _ = eggsearch::core::identity::source_id(None, Some(input), None, None);
    }
}

#[test]
fn url_corpus_exercises_validate_url() {
    use eggsearch::fetch::limits::{validate_url, FetchLimits};
    let cases = load_corpus_cases("url_edge_cases.json");
    assert!(!cases.is_empty(), "need at least one URL corpus case");
    let limits = FetchLimits::default();
    for case in &cases {
        let input = case["input"].as_str().expect("case must have 'input'");
        let _ = validate_url(input, &limits);
    }
}

#[cfg(feature = "pdf")]
mod pdf_decompression_bounds {
    use eggsearch::core::fetch::RedactedString;
    use eggsearch::fetch::pdf::extract_pdf_text;
    use eggsearch::fetch::pdf::PdfExtractOptions;
    use eggsearch::fetch::pdf::PdfLimits;
    use eggsearch::fetch::pdf_stream::scan_stream_budget;
    use eggsearch::fetch::FetchError;
    use std::io::Write;

    const BUDGET: usize = 65_536;
    const CONTENT: &str = "BT /F1 12 Tf 72 720 Td (Bounded extraction fixture text) Tj ET";

    fn zlib(data: &[u8]) -> Vec<u8> {
        let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::best());
        encoder.write_all(data).expect("deflate writes");
        encoder.finish().expect("deflate finishes")
    }

    fn ascii85(data: &[u8]) -> Vec<u8> {
        let mut encoded = Vec::new();
        for chunk in data.chunks(4) {
            let mut buffer = [0u8; 4];
            buffer[..chunk.len()].copy_from_slice(chunk);
            let mut value = u32::from_be_bytes(buffer);
            let mut digits = [0u8; 5];
            for digit in digits.iter_mut().rev() {
                *digit = b'!' + (value % 85) as u8;
                value /= 85;
            }
            let keep = if chunk.len() == 4 { 5 } else { chunk.len() + 1 };
            encoded.extend_from_slice(&digits[..keep]);
        }
        encoded.extend_from_slice(b"~>");
        encoded
    }

    fn stream_object(extra_dict: &str, content: &[u8]) -> Vec<u8> {
        let mut body =
            format!("<< /Length {} {extra_dict}>>\nstream\n", content.len()).into_bytes();
        body.extend_from_slice(content);
        body.extend_from_slice(b"\nendstream");
        body
    }

    fn build_pdf(objects: &[(u32, Vec<u8>)], trailer_extra: &str) -> Vec<u8> {
        let mut out = b"%PDF-1.5\n".to_vec();
        let mut offsets = std::collections::BTreeMap::new();
        for (id, body) in objects {
            offsets.insert(*id, out.len());
            out.extend_from_slice(format!("{id} 0 obj\n").as_bytes());
            out.extend_from_slice(body);
            out.extend_from_slice(b"\nendobj\n");
        }
        let last_id = objects.iter().map(|(id, _)| *id).max().unwrap_or(0);
        let xref_offset = out.len();
        let size = last_id + 1;
        out.extend_from_slice(format!("xref\n0 {size}\n").as_bytes());
        out.extend_from_slice(b"0000000000 65535 f \n");
        for id in 1..=last_id {
            match offsets.get(&id) {
                Some(offset) => {
                    out.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes())
                }
                None => out.extend_from_slice(b"0000000000 65535 f \n"),
            }
        }
        out.extend_from_slice(
            format!(
                "trailer\n<< /Size {size} /Root 1 0 R {trailer_extra}>>\nstartxref\n{xref_offset}\n%%EOF\n"
            )
            .as_bytes(),
        );
        out
    }

    fn page_objects(content: Vec<u8>) -> Vec<(u32, Vec<u8>)> {
        vec![
            (1, b"<< /Type /Catalog /Pages 2 0 R >>".to_vec()),
            (
                2,
                b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
            ),
            (
                3,
                b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>".to_vec(),
            ),
            (4, content),
            (
                5,
                b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_vec(),
            ),
        ]
    }

    fn pdf_with_extra_object(extra: Vec<u8>) -> Vec<u8> {
        let mut objects = page_objects(stream_object("", CONTENT.as_bytes()));
        objects.push((6, extra));
        build_pdf(&objects, "")
    }

    fn limits() -> PdfLimits {
        PdfLimits {
            max_pages: 25,
            max_chars_per_page: 12000,
            max_total_chars: 50000,
            max_decompressed_bytes: BUDGET,
        }
    }

    fn assert_breach(bytes: &[u8], expected_budget: usize) {
        match extract_pdf_text(bytes, 12000, &limits(), None) {
            Err(FetchError::PdfDecompressionBoundExceeded { measured, budget }) => {
                assert_eq!(budget, expected_budget);
                assert!(
                    measured > budget,
                    "a refusal must report at least the budget: {measured} vs {budget}"
                );
                assert!(measured <= budget + 65_536);
            }
            other => panic!("expected a decompression-bound refusal, got {other:?}"),
        }
    }

    #[test]
    fn pdf_content_stream_bomb_is_refused() {
        let bomb = zlib(&vec![b'a'; BUDGET * 8]);
        let bytes = build_pdf(
            &page_objects(stream_object("/Filter /FlateDecode ", &bomb)),
            "",
        );
        assert_breach(&bytes, BUDGET);
    }

    #[test]
    fn pdf_object_stream_bomb_is_refused() {
        let payload = zlib(&vec![b'o'; BUDGET * 8]);
        let body = format!(
            "<< /Type /ObjStm /N 1 /First 4 /Filter /FlateDecode /Length {} >>\nstream\n",
            payload.len()
        )
        .into_bytes();
        let mut extra = body;
        extra.extend_from_slice(&payload);
        extra.extend_from_slice(b"\nendstream");
        let bytes = pdf_with_extra_object(extra);
        assert_breach(&bytes, BUDGET);
    }

    #[test]
    fn pdf_xref_stream_bomb_is_refused() {
        let payload = zlib(&vec![b'x'; BUDGET * 8]);
        let body = format!("<< /Type /XRef /W [1 2 2] /Size 6 /Root 1 0 R /Filter /FlateDecode /Length {} >>\nstream\n", payload.len()).into_bytes();
        let mut extra = body;
        extra.extend_from_slice(&payload);
        extra.extend_from_slice(b"\nendstream");
        let bytes = pdf_with_extra_object(extra);
        assert_breach(&bytes, BUDGET);
    }

    #[test]
    fn pdf_chained_filter_bomb_is_refused() {
        let chained = ascii85(&zlib(&vec![b'c'; BUDGET * 8]));
        let bytes = build_pdf(
            &page_objects(stream_object(
                "/Filter [/ASCII85Decode /FlateDecode] ",
                &chained,
            )),
            "",
        );
        assert_breach(&bytes, BUDGET);
    }

    #[test]
    fn pdf_flate_then_unknown_filter_bomb_is_refused() {
        let bomb = zlib(&vec![b'a'; BUDGET * 8]);
        let bytes = build_pdf(
            &page_objects(stream_object("/Filter [/FlateDecode /DCTDecode] ", &bomb)),
            "",
        );
        assert_breach(&bytes, BUDGET);
    }

    #[test]
    fn pdf_xref_width_geometry_bomb_is_refused() {
        let body = b"<< /Type /XRef /W [1 2000000000 2] /Size 6 /Root 1 0 R /Length 0 >>\nstream\n\nendstream".to_vec();
        let bytes = pdf_with_extra_object(body);
        assert_breach(&bytes, BUDGET);
    }

    #[test]
    fn pdf_lzw_stream_is_refused() {
        let bytes = build_pdf(
            &page_objects(stream_object(
                "/Filter /LZWDecode ",
                b"unmeasured lzw bytes",
            )),
            "",
        );
        assert_breach(&bytes, BUDGET);
    }

    #[test]
    fn pdf_bound_reports_the_configured_budget() {
        let bomb = zlib(&vec![b'a'; BUDGET * 8]);
        let bytes = build_pdf(
            &page_objects(stream_object("/Filter /FlateDecode ", &bomb)),
            "",
        );
        let mut tightened = limits();
        tightened.max_decompressed_bytes = BUDGET * 2;
        match extract_pdf_text(&bytes, 12000, &tightened, None) {
            Err(FetchError::PdfDecompressionBoundExceeded { budget, .. }) => {
                assert_eq!(budget, BUDGET * 2)
            }
            other => panic!("expected a bound refusal, got {other:?}"),
        }
    }

    #[test]
    fn pdf_encrypted_document_is_refused_without_and_with_a_password() {
        let mut objects = page_objects(stream_object("", CONTENT.as_bytes()));
        objects.push((
            6,
            b"<< /Filter /Standard /V 1 /R 2 /O <0000000000000000000000000000000000000000000000000000000000000000> /U <1111111111111111111111111111111111111111111111111111111111111111> /P -4 >>".to_vec(),
        ));
        let bytes = build_pdf(&objects, "/Encrypt 6 0 R ");

        let without = PdfExtractOptions {
            selected_pages: None,
            password: None,
            include_media: false,
            ocr_policy: eggsearch::fetch::pdf::PdfOcrPolicy::Never,
        };
        let with = PdfExtractOptions {
            selected_pages: None,
            password: Some(RedactedString::new("secret")),
            include_media: false,
            ocr_policy: eggsearch::fetch::pdf::PdfOcrPolicy::Never,
        };

        for options in [Some(&without), Some(&with)] {
            match extract_pdf_text(&bytes, 12000, &limits(), options) {
                Err(error) => {
                    assert_eq!(error.error_code(), "pdf_encrypted");
                    assert!(matches!(error, FetchError::PdfEncrypted));
                }
                Ok(result) => panic!("encrypted documents must not extract: {result:?}"),
            }
        }
    }

    #[test]
    fn pdf_compressed_document_still_extracts() {
        let payload = zlib(CONTENT.as_bytes());
        let bytes = build_pdf(
            &page_objects(stream_object("/Filter /FlateDecode ", &payload)),
            "",
        );
        let measured = scan_stream_budget(&bytes, BUDGET).expect("compressed fixture scans");
        assert_eq!(measured, CONTENT.len());
        let result = extract_pdf_text(&bytes, 12000, &limits(), None)
            .expect("a compressed document under the budget extracts");
        assert!(result.text.contains("Bounded extraction fixture text"));
    }

    #[test]
    fn pdf_plain_document_is_unaffected_by_the_password_argument() {
        let bytes = build_pdf(&page_objects(stream_object("", CONTENT.as_bytes())), "");
        let options = PdfExtractOptions {
            selected_pages: None,
            password: Some(RedactedString::new("irrelevant")),
            include_media: false,
            ocr_policy: eggsearch::fetch::pdf::PdfOcrPolicy::Never,
        };
        let result = extract_pdf_text(&bytes, 12000, &limits(), Some(&options))
            .expect("an unencrypted document extracts regardless of the password argument");
        assert!(result.text.contains("Bounded extraction fixture text"));
    }

    #[test]
    fn pdf_scan_total_stays_within_the_configured_budget() {
        let bytes = build_pdf(&page_objects(stream_object("", CONTENT.as_bytes())), "");
        let measured = scan_stream_budget(&bytes, BUDGET).expect("plain fixture scans");
        assert_eq!(measured, 0, "uncompressed streams contribute no expansion");
        let bomb = zlib(&vec![b'a'; BUDGET * 8]);
        let bomb_bytes = build_pdf(
            &page_objects(stream_object("/Filter /FlateDecode ", &bomb)),
            "",
        );
        assert!(scan_stream_budget(&bomb_bytes, BUDGET * 1024).is_ok());
        assert!(scan_stream_budget(&bomb_bytes, BUDGET).is_err());
    }
}
