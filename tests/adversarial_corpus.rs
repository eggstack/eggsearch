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
            "Case {} in html_malformed.json missing 'input'",
            i
        );
        assert!(
            case.get("description").is_some(),
            "Case {} in html_malformed.json missing 'description'",
            i
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
            "Case {} in structured_text.json missing 'input'",
            i
        );
        assert!(
            case.get("description").is_some(),
            "Case {} in structured_text.json missing 'description'",
            i
        );
        assert!(
            case.get("format").is_some(),
            "Case {} in structured_text.json missing 'format'",
            i
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
            "Case {} in url_edge_cases.json missing 'input'",
            i
        );
        assert!(
            case.get("description").is_some(),
            "Case {} in url_edge_cases.json missing 'description'",
            i
        );
        let expected = case
            .get("expected")
            .and_then(|v| v.as_str())
            .unwrap_or_else(|| panic!("Case {} in url_edge_cases.json missing 'expected'", i));
        assert!(
            expected == "reject" || expected == "allow",
            "Case {} in url_edge_cases.json has invalid 'expected' value: {}",
            i,
            expected
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
            "Case {} in sanitize_edge_cases.json missing 'input'",
            i
        );
        assert!(
            case.get("description").is_some(),
            "Case {} in sanitize_edge_cases.json missing 'description'",
            i
        );
        assert!(
            case.get("category").is_some(),
            "Case {} in sanitize_edge_cases.json missing 'category'",
            i
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
            "Case {} in identity_edge_cases.json missing 'input'",
            i
        );
        assert!(
            case.get("description").is_some(),
            "Case {} in identity_edge_cases.json missing 'description'",
            i
        );
        assert!(
            case.get("category").is_some(),
            "Case {} in identity_edge_cases.json missing 'category'",
            i
        );
    }
    eprintln!("identity_edge_cases.json: {} cases OK", cases.len());
}
