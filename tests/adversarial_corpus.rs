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
