#![cfg(feature = "pdf")]

use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use eggsearch::core::document::PdfPageQualityKind;
use eggsearch::fetch::pdf::{
    extract_pdf_text, parse_pdf_page_spec, PdfExtractOptions, PdfExtractionResult, PdfLimits,
    PdfOcrPolicy,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};

const CORPUS: &str = include_str!("fixtures/pdf/corpus.json");
const TOTAL_BYTE_BUDGET: u64 = 3 * 1024 * 1024;
const CASE_BYTE_BUDGET: u64 = 1024 * 1024;
const MAX_PAGES: usize = 25;
const MAX_CHARS_PER_PAGE: usize = 100_000;
const MAX_TOTAL_CHARS: usize = 50_000;
const MAX_CHARS: usize = 200_000;
const DOCUMENT_CLASSES: [&str; 8] = [
    "two_column",
    "standards_table",
    "clean_text",
    "image_only",
    "encrypted",
    "malformed",
    "cid_corrupt",
    "outline_heavy",
];

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Corpus {
    schema_version: u32,
    corpus_revision: String,
    fixture_policy: String,
    byte_budget: ByteBudget,
    cases: Vec<Case>,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ByteBudget {
    total: u64,
    per_case: u64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    id: String,
    document_class: String,
    source_type: String,
    license: String,
    provenance: Provenance,
    construction: String,
    file: String,
    bytes: u64,
    expected_outcome: String,
    #[serde(default)]
    expected_error: Option<String>,
    #[serde(default)]
    include_media: Option<bool>,
    #[serde(default)]
    page_selection: Option<String>,
    #[serde(default)]
    required_words: Vec<String>,
    #[serde(default)]
    required_table_rows: Vec<TableRow>,
    #[serde(default)]
    opening_passage: Vec<String>,
    #[serde(default)]
    min_body_lines: Option<usize>,
    #[serde(default)]
    max_body_line_chars: Option<usize>,
    #[serde(default)]
    running_heads: Vec<RunningHeadExpectation>,
    #[serde(default)]
    expected_pages: Vec<PageExpectation>,
    #[serde(default)]
    expected_outline_entries: Option<usize>,
    #[serde(default)]
    expected_outline_max_level: Option<usize>,
    #[serde(default)]
    fidelity_targets: Option<FidelityTargets>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Provenance {
    #[serde(default)]
    url: Option<String>,
    retrieved: String,
    sha256: String,
    pages: usize,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct TableRow {
    label: String,
    value: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct RunningHeadExpectation {
    text: String,
    expected_pages: usize,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct PageExpectation {
    page: usize,
    quality_kind: String,
    #[serde(default)]
    image_count: Option<usize>,
}

#[derive(Clone, Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct FidelityTargets {
    #[serde(default)]
    line_shape: Option<TargetLineShape>,
    #[serde(default)]
    required_words: Option<TargetWords>,
    #[serde(default)]
    table_rows: Option<TargetRows>,
    #[serde(default)]
    opening_passage: Option<TargetPassage>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TargetLineShape {
    min_body_lines: usize,
    max_body_line_chars: usize,
    status: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TargetWords {
    words: Vec<String>,
    status: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TargetRows {
    rows: Vec<TableRow>,
    status: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TargetPassage {
    prefixes: Vec<String>,
    status: String,
}

#[derive(Clone, Copy, Debug, Default)]
struct ShapeAnnotations {
    min_body_lines: Option<usize>,
    max_body_line_chars: Option<usize>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct LineShape {
    body_lines: usize,
    max_line_chars: usize,
    violations: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct WordPresence {
    checked: usize,
    present: usize,
    missing: Vec<String>,
    violations: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct TableAdjacency {
    checked: usize,
    adjacent: usize,
    unmet: Vec<String>,
    violations: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct OrderedPassage {
    checked: usize,
    found: usize,
    unmatched: Vec<String>,
    violations: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct PageClassification {
    observed_pages: usize,
    expected_pages: usize,
    mismatches: Vec<String>,
    violations: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RunningHeadReport {
    text: String,
    expected_pages: usize,
    observed_pages: usize,
    violations: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PageText {
    page: usize,
    text: String,
    quality_kind: &'static str,
    image_count: Option<usize>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct ExtractionView {
    text: String,
    pages: Vec<PageText>,
}

#[derive(Clone, Debug)]
struct Measurement {
    shape: LineShape,
    words: WordPresence,
    table: TableAdjacency,
    passage: OrderedPassage,
    pages: PageClassification,
    heads: Vec<RunningHeadReport>,
}

fn line_shape_distribution(text: &str, annotations: ShapeAnnotations) -> LineShape {
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    let max_line_chars = lines
        .iter()
        .map(|line| line.chars().count())
        .max()
        .unwrap_or(0);
    let mut violations = Vec::new();
    if let Some(minimum) = annotations.min_body_lines {
        if lines.len() < minimum {
            violations.push(format!(
                "line_shape.min_body_lines: observed {} below {}",
                lines.len(),
                minimum
            ));
        }
    }
    if let Some(maximum) = annotations.max_body_line_chars {
        if max_line_chars > maximum {
            violations.push(format!(
                "line_shape.max_body_line_chars: observed {max_line_chars} above {maximum}"
            ));
        }
    }
    LineShape {
        body_lines: lines.len(),
        max_line_chars,
        violations,
    }
}

fn required_word_presence(text: &str, words: &[String]) -> WordPresence {
    let tokens: BTreeSet<&str> = text
        .split(|character: char| !character.is_alphanumeric())
        .filter(|token| !token.is_empty())
        .collect();
    let mut missing = Vec::new();
    for word in words {
        if !tokens.contains(word.as_str()) {
            missing.push(word.clone());
        }
    }
    let violations = missing
        .iter()
        .map(|word| format!("required_words.{word}: missing as an intact token"))
        .collect();
    WordPresence {
        checked: words.len(),
        present: words.len() - missing.len(),
        missing,
        violations,
    }
}

fn table_row_adjacency(text: &str, rows: &[TableRow]) -> TableAdjacency {
    let lines: Vec<&str> = text.lines().collect();
    let mut unmet = Vec::new();
    for row in rows {
        if !lines.iter().any(|line| row_is_adjacent(line, row)) {
            unmet.push(format!("{} / {}", row.label, row.value));
        }
    }
    let violations = unmet
        .iter()
        .map(|row| format!("table_row_adjacency.{row}: label and value are not on one line"))
        .collect();
    TableAdjacency {
        checked: rows.len(),
        adjacent: rows.len() - unmet.len(),
        unmet,
        violations,
    }
}

fn row_is_adjacent(line: &str, row: &TableRow) -> bool {
    let Some(index) = line.find(&row.label) else {
        return false;
    };
    let rest = &line[index + row.label.len()..];
    let Some(rest) = rest.strip_prefix(' ').or_else(|| rest.strip_prefix('\t')) else {
        return false;
    };
    rest.trim_start().starts_with(&row.value)
}

fn ordered_passage_presence(text: &str, prefixes: &[String]) -> OrderedPassage {
    let mut cursor = 0usize;
    let mut unmatched = Vec::new();
    for (index, prefix) in prefixes.iter().enumerate() {
        match text[cursor..].find(prefix.as_str()) {
            Some(offset) => cursor += offset + prefix.len(),
            None => unmatched.push(format!("[{index}] {prefix}")),
        }
    }
    let violations = unmatched
        .iter()
        .map(|prefix| format!("opening_passage.{prefix}: not found in annotated order"))
        .collect();
    OrderedPassage {
        checked: prefixes.len(),
        found: prefixes.len() - unmatched.len(),
        unmatched,
        violations,
    }
}

fn page_classification(view: &ExtractionView, expected: &[PageExpectation]) -> PageClassification {
    let mut mismatches = Vec::new();
    if view.pages.len() != expected.len() {
        mismatches.push(format!(
            "page_count: observed {} expected {}",
            view.pages.len(),
            expected.len()
        ));
    }
    for (index, expectation) in expected.iter().enumerate() {
        let Some(observed) = view.pages.get(index) else {
            continue;
        };
        if observed.page != expectation.page {
            mismatches.push(format!(
                "page[{}].page: observed {} expected {}",
                index, observed.page, expectation.page
            ));
        }
        if observed.quality_kind != expectation.quality_kind {
            mismatches.push(format!(
                "page[{}].quality_kind: observed {} expected {}",
                index, observed.quality_kind, expectation.quality_kind
            ));
        }
        if let Some(image_count) = expectation.image_count {
            if observed.image_count != Some(image_count) {
                mismatches.push(format!(
                    "page[{}].image_count: observed {:?} expected {}",
                    index, observed.image_count, image_count
                ));
            }
        }
    }
    let violations = mismatches
        .iter()
        .map(|mismatch| format!("page_classification.{mismatch}"))
        .collect();
    PageClassification {
        observed_pages: view.pages.len(),
        expected_pages: expected.len(),
        mismatches,
        violations,
    }
}

fn running_head_repetition(
    view: &ExtractionView,
    heads: &[RunningHeadExpectation],
) -> Vec<RunningHeadReport> {
    heads
        .iter()
        .map(|head| {
            let observed_pages = view
                .pages
                .iter()
                .filter(|page| page.text.lines().any(|line| line.trim() == head.text))
                .count();
            let violations = if observed_pages == head.expected_pages {
                Vec::new()
            } else {
                vec![format!(
                    "running_heads.{}: observed {} pages expected {}",
                    head.text, observed_pages, head.expected_pages
                )]
            };
            RunningHeadReport {
                text: head.text.clone(),
                expected_pages: head.expected_pages,
                observed_pages,
                violations,
            }
        })
        .collect()
}

fn quality_name(kind: PdfPageQualityKind) -> &'static str {
    match kind {
        PdfPageQualityKind::CleanText => "clean_text",
        PdfPageQualityKind::SparseText => "sparse_text",
        PdfPageQualityKind::CidCorrupt => "cid_corrupt",
        PdfPageQualityKind::ScannedOrImageOnly => "scanned_or_image_only",
        PdfPageQualityKind::Blank => "blank",
        PdfPageQualityKind::ExtractionFailed => "extraction_failed",
    }
}

fn extraction_view(result: &PdfExtractionResult) -> ExtractionView {
    let pages: Vec<PageText> = result
        .page_metadata
        .iter()
        .map(|metadata| PageText {
            page: metadata.page,
            text: result
                .document
                .blocks
                .iter()
                .filter(|block| block.page == Some(metadata.page))
                .map(|block| block.text.clone())
                .collect::<Vec<_>>()
                .join("\n"),
            quality_kind: quality_name(metadata.quality_kind),
            image_count: metadata.image_count,
        })
        .collect();
    let text = pages
        .iter()
        .map(|page| page.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    ExtractionView { text, pages }
}

fn measure(case: &Case, view: &ExtractionView) -> Measurement {
    let shape = line_shape_distribution(
        &view.text,
        ShapeAnnotations {
            min_body_lines: case.min_body_lines,
            max_body_line_chars: case.max_body_line_chars,
        },
    );
    Measurement {
        shape,
        words: required_word_presence(&view.text, &case.required_words),
        table: table_row_adjacency(&view.text, &case.required_table_rows),
        passage: ordered_passage_presence(&view.text, &case.opening_passage),
        pages: page_classification(view, &case.expected_pages),
        heads: running_head_repetition(view, &case.running_heads),
    }
}

fn baseline_violations(case: &Case, measurement: &Measurement) -> Vec<String> {
    measurement
        .shape
        .violations
        .iter()
        .chain(measurement.words.violations.iter())
        .chain(measurement.table.violations.iter())
        .chain(measurement.passage.violations.iter())
        .chain(measurement.pages.violations.iter())
        .chain(
            measurement
                .heads
                .iter()
                .flat_map(|head| head.violations.iter()),
        )
        .map(|violation| format!("{} {violation}", case.id))
        .collect()
}

fn target_failures(case: &Case, view: &ExtractionView) -> Vec<String> {
    let Some(targets) = &case.fidelity_targets else {
        return Vec::new();
    };
    let mut failures = Vec::new();
    if let Some(target) = &targets.line_shape {
        let measured = line_shape_distribution(
            &view.text,
            ShapeAnnotations {
                min_body_lines: Some(target.min_body_lines),
                max_body_line_chars: Some(target.max_body_line_chars),
            },
        );
        check_target_status(
            case,
            "fidelity_targets.line_shape",
            &target.status,
            &measured.violations,
            &mut failures,
        );
    }
    if let Some(target) = &targets.required_words {
        let measured = required_word_presence(&view.text, &target.words);
        check_target_status(
            case,
            "fidelity_targets.required_words",
            &target.status,
            &measured.violations,
            &mut failures,
        );
    }
    if let Some(target) = &targets.table_rows {
        let measured = table_row_adjacency(&view.text, &target.rows);
        check_target_status(
            case,
            "fidelity_targets.table_rows",
            &target.status,
            &measured.violations,
            &mut failures,
        );
    }
    if let Some(target) = &targets.opening_passage {
        let measured = ordered_passage_presence(&view.text, &target.prefixes);
        check_target_status(
            case,
            "fidelity_targets.opening_passage",
            &target.status,
            &measured.violations,
            &mut failures,
        );
    }
    failures
}

fn check_target_status(
    case: &Case,
    annotation: &str,
    recorded: &str,
    violations: &[String],
    failures: &mut Vec<String>,
) {
    let measured_met = violations.is_empty();
    if measured_met != (recorded == "met") {
        failures.push(format!(
            "{} {annotation}: recorded status {recorded}, measured violations {violations:?}; update the corpus annotation",
            case.id
        ));
    }
}

fn target_report(case: &Case, view: &ExtractionView) -> String {
    let Some(targets) = &case.fidelity_targets else {
        return "none".to_string();
    };
    let mut fields = Vec::new();
    if let Some(target) = &targets.line_shape {
        let measured = line_shape_distribution(
            &view.text,
            ShapeAnnotations {
                min_body_lines: Some(target.min_body_lines),
                max_body_line_chars: Some(target.max_body_line_chars),
            },
        );
        fields.push(format!("line_shape={}", status_word(&measured.violations)));
    }
    if let Some(target) = &targets.required_words {
        let measured = required_word_presence(&view.text, &target.words);
        fields.push(format!(
            "required_words={}",
            status_word(&measured.violations)
        ));
    }
    if let Some(target) = &targets.table_rows {
        let measured = table_row_adjacency(&view.text, &target.rows);
        fields.push(format!("table_rows={}", status_word(&measured.violations)));
    }
    if let Some(target) = &targets.opening_passage {
        let measured = ordered_passage_presence(&view.text, &target.prefixes);
        fields.push(format!(
            "opening_passage={}",
            status_word(&measured.violations)
        ));
    }
    fields.join(",")
}

fn status_word(violations: &[String]) -> &'static str {
    if violations.is_empty() {
        "met"
    } else {
        "unmet"
    }
}

fn report_line(case: &Case, view: &ExtractionView, measurement: &Measurement) -> String {
    let kinds = view
        .pages
        .iter()
        .map(|page| format!("{}:{}", page.page, page.quality_kind))
        .collect::<Vec<_>>()
        .join(",");
    let heads = measurement
        .heads
        .iter()
        .map(|head| {
            format!(
                "{}={}/{}",
                head.text, head.observed_pages, head.expected_pages
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "pdf-fidelity case={} class={} bytes={} pages={} lines={} max_line={} words={}/{} table={}/{} passage={}/{} kinds={} heads={}",
        case.id,
        case.document_class,
        case.bytes,
        view.pages.len(),
        measurement.shape.body_lines,
        measurement.shape.max_line_chars,
        measurement.words.present,
        measurement.words.checked,
        measurement.table.adjacent,
        measurement.table.checked,
        measurement.passage.found,
        measurement.passage.checked,
        kinds,
        if heads.is_empty() { "none".to_string() } else { heads }
    )
}

fn degrade(text: &str) -> String {
    let joined: String = text.lines().collect();
    joined
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect()
}

fn degraded_view(view: &ExtractionView) -> ExtractionView {
    let text = degrade(&view.text);
    ExtractionView {
        text: text.clone(),
        pages: vec![PageText {
            page: 1,
            text,
            quality_kind: "clean_text",
            image_count: None,
        }],
    }
}

fn declared_metric_violations(
    case: &Case,
    view: &ExtractionView,
) -> Vec<(&'static str, Vec<String>)> {
    let mut groups = Vec::new();
    if case.min_body_lines.is_some() || case.max_body_line_chars.is_some() {
        let measured = line_shape_distribution(
            &view.text,
            ShapeAnnotations {
                min_body_lines: case.min_body_lines,
                max_body_line_chars: case.max_body_line_chars,
            },
        );
        groups.push(("line_shape", measured.violations));
    }
    if !case.required_words.is_empty() {
        groups.push((
            "required_words",
            required_word_presence(&view.text, &case.required_words).violations,
        ));
    }
    if !case.required_table_rows.is_empty() {
        groups.push((
            "table_row_adjacency",
            table_row_adjacency(&view.text, &case.required_table_rows).violations,
        ));
    }
    if !case.opening_passage.is_empty() {
        groups.push((
            "opening_passage",
            ordered_passage_presence(&view.text, &case.opening_passage).violations,
        ));
    }
    if !case.expected_pages.is_empty() {
        groups.push((
            "page_classification",
            page_classification(view, &case.expected_pages).violations,
        ));
    }
    if !case.running_heads.is_empty() {
        let violations = running_head_repetition(view, &case.running_heads)
            .into_iter()
            .flat_map(|head| head.violations)
            .collect();
        groups.push(("running_heads", violations));
    }
    groups
}

fn load_corpus() -> Corpus {
    serde_json::from_str(CORPUS).expect("pdf fidelity corpus manifest parses")
}

fn corpus_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/pdf")
}

fn extraction_limits() -> PdfLimits {
    PdfLimits {
        max_pages: MAX_PAGES,
        max_chars_per_page: MAX_CHARS_PER_PAGE,
        max_total_chars: MAX_TOTAL_CHARS,
        max_decompressed_bytes: 67_108_864,
    }
}

fn extract_case(case: &Case) -> Result<PdfExtractionResult, eggsearch::fetch::FetchError> {
    let bytes = std::fs::read(corpus_root().join(&case.file))
        .unwrap_or_else(|error| panic!("fixture {} is readable: {error}", case.file));
    let selected_pages = case
        .page_selection
        .as_ref()
        .map(|spec| parse_pdf_page_spec(spec, MAX_PAGES).expect("page selection parses"));
    let options = PdfExtractOptions {
        selected_pages,
        password: None,
        include_media: case.include_media.unwrap_or(false),
        ocr_policy: PdfOcrPolicy::Never,
    };
    extract_pdf_text(&bytes, MAX_CHARS, &extraction_limits(), Some(&options))
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[test]
fn pdf_fidelity_corpus_manifest_is_valid() {
    let corpus = load_corpus();
    assert_eq!(corpus.schema_version, 1);
    assert_eq!(corpus.corpus_revision, "pdf-fidelity-v1");
    assert!(corpus.fixture_policy.contains("synthetic"));
    assert!(corpus.fixture_policy.contains("third-party"));
    assert!(corpus.byte_budget.total <= TOTAL_BYTE_BUDGET);
    assert!(corpus.byte_budget.per_case <= CASE_BYTE_BUDGET);
    assert!(corpus.cases.len() >= DOCUMENT_CLASSES.len());

    let classes: BTreeSet<&str> = corpus
        .cases
        .iter()
        .map(|case| case.document_class.as_str())
        .collect();
    assert_eq!(
        classes,
        DOCUMENT_CLASSES.into_iter().collect::<BTreeSet<_>>(),
        "every document class needs a committed case"
    );
    let ids: BTreeSet<&str> = corpus.cases.iter().map(|case| case.id.as_str()).collect();
    assert_eq!(ids.len(), corpus.cases.len(), "case IDs are unique");

    let mut total_bytes = 0u64;
    for case in &corpus.cases {
        assert!(!case.id.is_empty());
        assert_eq!(case.source_type, "synthetic");
        assert!(!case.license.is_empty(), "{} records a licence", case.id);
        assert!(!case.provenance.retrieved.is_empty());
        assert!(case.provenance.pages > 0);
        assert_eq!(case.provenance.sha256.len(), 64);
        assert!(case
            .provenance
            .sha256
            .chars()
            .all(|c| c.is_ascii_hexdigit()));
        assert!(
            case.construction.len() >= 80,
            "{} records a rebuildable construction recipe",
            case.id
        );
        if case.source_type == "synthetic" {
            assert!(
                case.provenance.url.is_none(),
                "{} declares no third-party source URL",
                case.id
            );
        }

        let relative = Path::new(&case.file);
        assert!(!relative.is_absolute());
        assert!(relative
            .components()
            .all(|component| matches!(component, Component::Normal(_))));
        let bytes = std::fs::read(corpus_root().join(relative))
            .unwrap_or_else(|error| panic!("{} fixture exists: {error}", case.id));
        assert_eq!(bytes.len() as u64, case.bytes, "{} byte count", case.id);
        assert!(case.bytes <= corpus.byte_budget.per_case);
        assert_eq!(
            sha256_hex(&bytes),
            case.provenance.sha256,
            "{} sha256",
            case.id
        );
        total_bytes += case.bytes;

        assert!(matches!(
            case.expected_outcome.as_str(),
            "extracted" | "empty" | "structural_break"
        ));
        match case.expected_outcome.as_str() {
            "extracted" => {
                assert!(case.expected_error.is_none());
                assert!(!case.expected_pages.is_empty());
                assert!(case.min_body_lines.is_some());
                assert!(case.max_body_line_chars.is_some());
                assert!(
                    !case.required_words.is_empty()
                        || !case.required_table_rows.is_empty()
                        || !case.opening_passage.is_empty(),
                    "{} carries at least one text annotation",
                    case.id
                );
            }
            "empty" => {
                assert!(!case.expected_pages.is_empty());
            }
            "structural_break" => {
                let expected = case.expected_error.as_deref().unwrap_or_default();
                assert!(
                    expected.starts_with("pdf_"),
                    "{} names a PDF error",
                    case.id
                );
            }
            _ => {}
        }

        let mut previous_page = 0usize;
        for expectation in &case.expected_pages {
            assert!(expectation.page > previous_page, "{} page order", case.id);
            previous_page = expectation.page;
            assert!(!expectation.quality_kind.is_empty());
        }

        if let Some(spec) = &case.page_selection {
            let pages = parse_pdf_page_spec(spec, MAX_PAGES).expect("page selection parses");
            assert!(!pages.is_empty());
            assert!(
                pages
                    .iter()
                    .all(|page| *page as usize <= case.provenance.pages),
                "{} page selection stays inside the document",
                case.id
            );
        }

        if let Some(targets) = &case.fidelity_targets {
            let statuses = [
                targets
                    .line_shape
                    .as_ref()
                    .map(|target| target.status.as_str()),
                targets
                    .required_words
                    .as_ref()
                    .map(|target| target.status.as_str()),
                targets
                    .table_rows
                    .as_ref()
                    .map(|target| target.status.as_str()),
                targets
                    .opening_passage
                    .as_ref()
                    .map(|target| target.status.as_str()),
            ];
            for status in statuses.into_iter().flatten() {
                assert!(
                    matches!(status, "met" | "unmet"),
                    "{} target status",
                    case.id
                );
            }
        }

        if let Some(entries) = case.expected_outline_entries {
            assert!(entries > 0);
        }
        if let Some(level) = case.expected_outline_max_level {
            assert!((1..=6).contains(&level));
        }
    }
    assert!(
        total_bytes <= corpus.byte_budget.total,
        "committed corpus is {total_bytes} bytes, over the declared budget"
    );
    assert!(total_bytes <= TOTAL_BYTE_BUDGET);
}

#[test]
fn pdf_fidelity_corpus_matches_annotations() {
    let corpus = load_corpus();
    let mut failures = Vec::new();
    let mut report = Vec::new();
    for case in &corpus.cases {
        match extract_case(case) {
            Ok(result) => {
                if case.expected_outcome == "structural_break" {
                    failures.push(format!(
                        "{} expected structural_break but extraction succeeded",
                        case.id
                    ));
                    continue;
                }
                let view = extraction_view(&result);
                if case.expected_outcome == "empty" {
                    if !view.text.trim().is_empty() {
                        failures.push(format!(
                            "{} expected empty text, got {:?}",
                            case.id, view.text
                        ));
                    }
                    if result.content_ok {
                        failures.push(format!("{} expected content_ok=false", case.id));
                    }
                } else if view.text.trim().is_empty() {
                    failures.push(format!("{} expected extracted text, got empty", case.id));
                }
                if case.page_selection.is_none() && view.pages.len() != case.provenance.pages {
                    failures.push(format!(
                        "{} extracted {} pages, provenance declares {}",
                        case.id,
                        view.pages.len(),
                        case.provenance.pages
                    ));
                }
                let measurement = measure(case, &view);
                failures.extend(baseline_violations(case, &measurement));
                failures.extend(target_failures(case, &view));
                if let Some(expected) = case.expected_outline_entries {
                    if result.document.outline.len() != expected {
                        failures.push(format!(
                            "{} outline entries observed {} expected {}",
                            case.id,
                            result.document.outline.len(),
                            expected
                        ));
                    }
                }
                if let Some(expected) = case.expected_outline_max_level {
                    let observed = result
                        .document
                        .outline
                        .iter()
                        .map(|entry| entry.level)
                        .max()
                        .unwrap_or(0);
                    if observed != expected {
                        failures.push(format!(
                            "{} outline max level observed {} expected {}",
                            case.id, observed, expected
                        ));
                    }
                }
                report.push(format!(
                    "{} targets={}",
                    report_line(case, &view, &measurement),
                    target_report(case, &view)
                ));
            }
            Err(error) => {
                if case.expected_outcome == "structural_break" {
                    let expected = case.expected_error.as_deref().unwrap_or_default();
                    if error.error_code() != expected {
                        failures.push(format!(
                            "{} expected error {expected}, observed {}",
                            case.id,
                            error.error_code()
                        ));
                    }
                    report.push(format!(
                        "pdf-fidelity case={} class={} bytes={} outcome=structural_break error={}",
                        case.id,
                        case.document_class,
                        case.bytes,
                        error.error_code()
                    ));
                } else {
                    failures.push(format!(
                        "{} expected {}, extraction failed with {}",
                        case.id,
                        case.expected_outcome,
                        error.error_code()
                    ));
                }
            }
        }
    }
    for line in &report {
        println!("{line}");
    }
    assert!(
        failures.is_empty(),
        "pdf fidelity corpus failures:\n{}",
        failures.join("\n")
    );
}

#[test]
fn pdf_fidelity_metrics_reject_degraded_extraction() {
    let corpus = load_corpus();
    let mut failures = Vec::new();
    let mut reports = Vec::new();
    for case in corpus
        .cases
        .iter()
        .filter(|case| case.expected_outcome == "extracted")
    {
        let result = extract_case(case).expect("fixture extracts");
        let view = extraction_view(&result);
        let intact = declared_metric_violations(case, &view);
        for (metric, violations) in &intact {
            if !violations.is_empty() {
                failures.push(format!(
                    "{} intact extraction already violates {metric}: {violations:?}",
                    case.id
                ));
            }
        }
        let degraded = degraded_view(&view);
        let groups = declared_metric_violations(case, &degraded);
        if groups.is_empty() {
            failures.push(format!("{} declares no metric to control", case.id));
        }
        for (metric, violations) in &groups {
            if violations.is_empty() {
                failures.push(format!(
                    "{} degraded extraction was accepted by {metric}",
                    case.id
                ));
            }
            if violations
                .iter()
                .any(|violation| !violation.starts_with(metric))
            {
                failures.push(format!(
                    "{} {metric} violation does not name the annotation: {violations:?}",
                    case.id
                ));
            }
            reports.push(format!(
                "pdf-fidelity negative-control case={} transform=join-lines+strip-spaces metric={} violations={violations:?}",
                case.id, metric
            ));
        }
    }
    for line in &reports {
        println!("{line}");
    }
    let controlled = reports
        .iter()
        .filter(|line| line.contains("case=pdf-two-column-synthetic"))
        .count();
    assert_eq!(
        controlled, 6,
        "the two-column case controls every metric family"
    );
    assert!(
        failures.is_empty(),
        "pdf fidelity negative control failures:\n{}",
        failures.join("\n")
    );
}

#[test]
fn line_shape_distribution_accepts_lines_within_bounds() {
    let measured = line_shape_distribution(
        "one\ntwo\nthree",
        ShapeAnnotations {
            min_body_lines: Some(3),
            max_body_line_chars: Some(10),
        },
    );
    assert_eq!(measured.body_lines, 3);
    assert_eq!(measured.max_line_chars, 5);
    assert!(measured.violations.is_empty());
}

#[test]
fn line_shape_distribution_rejects_a_collapsed_line() {
    let measured = line_shape_distribution(
        "one two three four",
        ShapeAnnotations {
            min_body_lines: Some(3),
            max_body_line_chars: Some(10),
        },
    );
    assert_eq!(measured.body_lines, 1);
    assert_eq!(measured.max_line_chars, 18);
    assert!(measured
        .violations
        .iter()
        .any(|violation| violation.starts_with("line_shape.min_body_lines")));
    assert!(measured
        .violations
        .iter()
        .any(|violation| violation.starts_with("line_shape.max_body_line_chars")));
}

#[test]
fn required_word_presence_accepts_intact_words() {
    let measured = required_word_presence(
        "the definition of an identifier is stable",
        &["definition".to_string(), "identifier".to_string()],
    );
    assert_eq!(measured.present, 2);
    assert!(measured.violations.is_empty());
}

#[test]
fn required_word_presence_rejects_space_split_ligature_words() {
    let measured = required_word_presence(
        "the de ﬁ nition of an identi ﬁ er is stable",
        &["definition".to_string(), "identifier".to_string()],
    );
    assert_eq!(measured.missing.len(), 2);
    assert!(measured
        .violations
        .iter()
        .any(|violation| violation.starts_with("required_words.definition")));
    assert!(measured
        .violations
        .iter()
        .any(|violation| violation.starts_with("required_words.identifier")));
}

#[test]
fn table_row_adjacency_accepts_same_line_rows() {
    let measured = table_row_adjacency(
        "Limit: 100\nTimeout: 30",
        &[
            TableRow {
                label: "Limit:".to_string(),
                value: "100".to_string(),
            },
            TableRow {
                label: "Timeout:".to_string(),
                value: "30".to_string(),
            },
        ],
    );
    assert_eq!(measured.adjacent, 2);
    assert!(measured.violations.is_empty());
}

#[test]
fn table_row_adjacency_rejects_split_rows() {
    let measured = table_row_adjacency(
        "Limit:\n100\nTimeout: 30",
        &[
            TableRow {
                label: "Limit:".to_string(),
                value: "100".to_string(),
            },
            TableRow {
                label: "Timeout:".to_string(),
                value: "30".to_string(),
            },
        ],
    );
    assert_eq!(measured.adjacent, 1);
    assert!(measured
        .violations
        .iter()
        .any(|violation| violation.starts_with("table_row_adjacency.Limit:")));
}

#[test]
fn ordered_passage_presence_accepts_annotated_order() {
    let measured = ordered_passage_presence(
        "alpha one beta two gamma three",
        &["alpha".to_string(), "beta".to_string(), "gamma".to_string()],
    );
    assert_eq!(measured.found, 3);
    assert!(measured.violations.is_empty());
}

#[test]
fn ordered_passage_presence_rejects_inverted_order() {
    let measured = ordered_passage_presence(
        "alpha one gamma three beta two",
        &["alpha".to_string(), "beta".to_string(), "gamma".to_string()],
    );
    assert_eq!(measured.found, 2);
    assert!(measured
        .violations
        .iter()
        .any(|violation| violation.contains("opening_passage.[2] gamma")));
}

#[test]
fn page_classification_accepts_expected_kinds() {
    let view = ExtractionView {
        text: "first\nsecond".to_string(),
        pages: vec![
            PageText {
                page: 1,
                text: "first".to_string(),
                quality_kind: "clean_text",
                image_count: None,
            },
            PageText {
                page: 2,
                text: "second".to_string(),
                quality_kind: "clean_text",
                image_count: None,
            },
        ],
    };
    let measured = page_classification(
        &view,
        &[
            PageExpectation {
                page: 1,
                quality_kind: "clean_text".to_string(),
                image_count: None,
            },
            PageExpectation {
                page: 2,
                quality_kind: "clean_text".to_string(),
                image_count: None,
            },
        ],
    );
    assert_eq!(measured.observed_pages, 2);
    assert!(measured.violations.is_empty());
}

#[test]
fn page_classification_rejects_a_wrong_page_count() {
    let view = ExtractionView {
        text: "first".to_string(),
        pages: vec![PageText {
            page: 1,
            text: "first".to_string(),
            quality_kind: "clean_text",
            image_count: None,
        }],
    };
    let measured = page_classification(
        &view,
        &[
            PageExpectation {
                page: 1,
                quality_kind: "clean_text".to_string(),
                image_count: None,
            },
            PageExpectation {
                page: 2,
                quality_kind: "clean_text".to_string(),
                image_count: None,
            },
        ],
    );
    assert!(measured
        .violations
        .iter()
        .any(|violation| violation.starts_with("page_classification.page_count")));
}

#[test]
fn running_head_repetition_accepts_repeated_head() {
    let view = ExtractionView {
        text: "head\nbody\nhead\nbody".to_string(),
        pages: vec![
            PageText {
                page: 1,
                text: "head\nbody".to_string(),
                quality_kind: "clean_text",
                image_count: None,
            },
            PageText {
                page: 2,
                text: "head\nbody".to_string(),
                quality_kind: "clean_text",
                image_count: None,
            },
        ],
    };
    let measured = running_head_repetition(
        &view,
        &[RunningHeadExpectation {
            text: "head".to_string(),
            expected_pages: 2,
        }],
    );
    assert_eq!(measured[0].observed_pages, 2);
    assert!(measured[0].violations.is_empty());
}

#[test]
fn running_head_repetition_rejects_missing_head() {
    let view = ExtractionView {
        text: "body".to_string(),
        pages: vec![PageText {
            page: 1,
            text: "body".to_string(),
            quality_kind: "clean_text",
            image_count: None,
        }],
    };
    let measured = running_head_repetition(
        &view,
        &[RunningHeadExpectation {
            text: "head".to_string(),
            expected_pages: 2,
        }],
    );
    assert_eq!(measured[0].observed_pages, 0);
    assert!(measured[0].violations[0].starts_with("running_heads.head"));
}

#[tokio::test]
#[ignore = "opt-in live recorder; downloads the two workstream source documents into target/pdf-corpus/"]
async fn pdf_fidelity_recorder_downloads_and_measures_reference_documents() {
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let output = repository.join("target/pdf-corpus");
    assert!(output.starts_with(repository.join("target")));
    std::fs::create_dir_all(&output).expect("recorder output directory is creatable");

    let corpus = load_corpus();
    let timeout = Duration::from_secs(120);
    let client = eggfetch_core::Client::builder()
        .user_agent("eggsearch-pdf-fidelity-recorder/1.0")
        .timeout(eggfetch_core::Timeout {
            pool: Some(timeout),
            connect: Some(timeout),
            write: Some(timeout),
            read: Some(timeout),
            total: Some(timeout),
        })
        .follow_redirects(true)
        .max_redirects(10)
        .build();

    let sources = [
        (
            "arXiv 1706.03762",
            "https://arxiv.org/pdf/1706.03762",
            "two_column",
        ),
        (
            "RFC 9110",
            "https://www.rfc-editor.org/rfc/rfc9110.pdf",
            "standards_table",
        ),
    ];

    for (label, url, document_class) in sources {
        let case = corpus
            .cases
            .iter()
            .find(|case| case.document_class == document_class)
            .expect("reference class has a corpus case");
        let response = client
            .get(url)
            .expect("request builds")
            .send()
            .await
            .expect("reference document is reachable");
        assert!(response.status().is_success());
        let bytes = eggsearch::meta::engines::read_bounded_body(
            response,
            "pdf_fidelity_recorder",
            16 * 1048576,
        )
        .await
        .expect("reference document body is bounded");
        let path = output.join(format!("{document_class}.pdf"));
        std::fs::write(&path, &bytes).expect("reference document is writable under target/");

        let options = PdfExtractOptions {
            selected_pages: None,
            password: None,
            include_media: true,
            ocr_policy: PdfOcrPolicy::Never,
        };
        match extract_pdf_text(&bytes, MAX_CHARS, &extraction_limits(), Some(&options)) {
            Ok(result) => {
                let view = extraction_view(&result);
                let measurement = measure(case, &view);
                println!(
                    "pdf-fidelity recorder source={label} url={url} path={} bytes={} pages={} lines={} max_line={} kinds={}",
                    path.display(),
                    bytes.len(),
                    view.pages.len(),
                    measurement.shape.body_lines,
                    measurement.shape.max_line_chars,
                    view.pages
                        .iter()
                        .map(|page| format!("{}:{}", page.page, page.quality_kind))
                        .collect::<Vec<_>>()
                        .join(",")
                );
                println!(
                    "pdf-fidelity recorder annotations source={label} targets={} violations={:?}",
                    target_report(case, &view),
                    baseline_violations(case, &measurement)
                );
            }
            Err(error) => println!(
                "pdf-fidelity recorder source={label} url={url} error={}",
                error.error_code()
            ),
        }
    }
}
