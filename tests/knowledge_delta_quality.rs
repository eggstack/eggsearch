#![cfg(feature = "mock")]

use std::collections::{BTreeMap, BTreeSet};
use std::time::{Duration, Instant};

use chrono::{Duration as ChronoDuration, NaiveDate};
use serde::Deserialize;

use eggsearch::core::config::AppConfig;
use eggsearch::core::identity::knowledge_delta_id;
use eggsearch::core::knowledge_delta::{
    parse_caller_override, CutoffOverrideInput, KnowledgeIndexConfig, RefreshMode,
};
use eggsearch::meta::adapter::MetadataSearchAdapter;
use eggsearch::meta::knowledge_delta::{
    run_knowledge_brief as run_meta_brief, BriefQuery, BriefRun, EvidenceIndex, IndexRecord,
};
use eggsearch::meta::mock::{mock_engines, MockEngine, MockResult};

#[derive(Deserialize)]
struct Corpus {
    #[serde(rename = "_meta")]
    meta: Meta,
    events: Vec<Event>,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Meta {
    #[serde(rename = "caseCount")]
    case_count: usize,
    counts: BTreeMap<String, usize>,
    #[serde(rename = "providerProfile")]
    provider_profile: Vec<String>,
    #[serde(rename = "timestampCapableProviders")]
    timestamp_capable_providers: Vec<String>,
    requirement: Requirement,
}

#[derive(Deserialize)]
struct Requirement {
    #[serde(rename = "datedCases")]
    dated_cases: usize,
    #[serde(rename = "withheldName")]
    withheld_name: usize,
    #[serde(rename = "negativeControls")]
    negative_controls: usize,
    #[serde(rename = "ambiguousOrConflicting")]
    ambiguous_or_conflicting: usize,
}

#[derive(Deserialize, Clone)]
struct Event {
    id: String,
    product: String,
    version: String,
    date: String,
}

#[derive(Deserialize)]
struct Case {
    id: String,
    class: String,
    product: String,
    topic: String,
    cutoff: Cutoff,
    #[serde(rename = "goldEventId")]
    gold_event_id: String,
}

#[derive(Deserialize)]
struct Cutoff {
    date: String,
    precision: String,
}

const CORPUS: &str = include_str!("fixtures/knowledge_delta_quality/corpus.json");
const PROVIDER: &str = "hn_algolia";
const MAX_ITEMS: usize = 8;
const MAX_CHARS: usize = 6_000;

const DISTINCTIVE: &[(&str, &str)] = &[
    ("python", "CPython-interpreter"),
    ("nodejs", "JavaScript-runtime"),
    ("postgresql", "database-upgrade"),
    ("kubernetes", "container-orchestration"),
    ("go", "compiler-toolchain"),
    ("rust", "borrow-checker"),
    ("ruby", "garbage-collector"),
    ("django", "async-view"),
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Variant {
    DirectLive,
    IndexedLexical,
    IndexedExpanded,
}

impl Variant {
    fn label(self) -> &'static str {
        match self {
            Self::DirectLive => "direct_live",
            Self::IndexedLexical => "indexed_lexical",
            Self::IndexedExpanded => "indexed_expanded",
        }
    }
}

fn distinctive(product: &str) -> &'static str {
    DISTINCTIVE
        .iter()
        .find(|(key, _)| *key == product)
        .map(|(_, value)| *value)
        .unwrap_or("release-notes")
}

fn card_url(event: &Event) -> String {
    let version = &event.version;
    match event.product.as_str() {
        "python" => format!(
            "https://www.python.org/downloads/release/python-{}",
            version.replace('.', "")
        ),
        "nodejs" => format!("https://nodejs.org/en/blog/release/v{version}"),
        "postgresql" => format!("https://www.postgresql.org/docs/release/{version}/"),
        "kubernetes" => format!("https://kubernetes.io/releases/{version}/"),
        "go" => format!("https://go.dev/doc/go{version}"),
        "rust" => format!("https://blog.rust-lang.org/releases/{version}/"),
        "ruby" => format!("https://www.ruby-lang.org/en/news/{version}/ruby-{version}-released/"),
        "django" => format!("https://docs.djangoproject.com/en/stable/releases/{version}/"),
        other => format!("https://endoflife.date/{other}/{version}"),
    }
}

fn gold_id(event: &Event) -> String {
    knowledge_delta_id(&card_url(event), "publication", "day", &event.date)
}

fn dated_card(event: &Event) -> MockResult {
    let mut result = MockResult::new(
        format!("{} {} release notes", event.product, event.version),
        card_url(event),
        PROVIDER,
    );
    result.snippet = Some(format!(
        "{} {} published release notes",
        event.product, event.version
    ));
    result.published_at = Some(event.date.clone());
    result
}

fn undated_card(product: &str) -> MockResult {
    MockResult::new(
        format!("{product} ecosystem overview"),
        format!("https://endoflife.date/{product}/undated-overview"),
        PROVIDER,
    )
    .with_snippet(format!(
        "{product} compatibility overview without a declared date"
    ))
}

fn index_config(path: &std::path::Path) -> KnowledgeIndexConfig {
    KnowledgeIndexConfig {
        enabled: true,
        path: Some(path.to_string_lossy().to_string()),
        max_bytes: 4 * 1024 * 1024,
        max_documents: 64,
        max_excerpt_chars: 400,
        retention_days: 365,
    }
}

fn private_index_path(root: &std::path::Path, name: &str) -> std::path::PathBuf {
    let parent = root.join(name);
    std::fs::create_dir(&parent).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    parent.join("index.sqlite")
}

fn record(url: &str, title: &str, excerpt: &str, date: &str, source_id: &str) -> IndexRecord {
    IndexRecord {
        partition: "anonymous".into(),
        canonical_url: url.into(),
        source_id: source_id.into(),
        title: title.into(),
        excerpt: excerpt.into(),
        trust: "external_untrusted".into(),
        basis: "publication".into(),
        precision: "day".into(),
        normalized_date: Some(date.into()),
        kdelta_id: knowledge_delta_id(url, "publication", "day", date),
        conflicted: false,
        temporal_relationship: "after_cutoff".into(),
    }
}

fn cutoff_of(case: &Case) -> eggsearch::core::knowledge_delta::ResolvedCutoff {
    parse_caller_override(&CutoffOverrideInput {
        date: case.cutoff.date.clone(),
        precision: case.cutoff.precision.clone(),
        source: "frozen corpus".into(),
        reason: "M007 offline qualification".into(),
    })
    .expect("corpus cutoff is valid")
}

fn query_for(case: &Case, refresh: RefreshMode, related: bool) -> BriefQuery {
    BriefQuery {
        topic: case.topic.clone(),
        model_id: None,
        caller_cutoff: Some(cutoff_of(case)),
        max_items: MAX_ITEMS,
        max_chars: MAX_CHARS,
        refresh,
        retain: false,
        related_topics: related,
        language: None,
        partition: "anonymous".into(),
    }
}

#[derive(Default)]
struct Totals {
    cases: usize,
    expectation_met: usize,
    gold_hits: usize,
    gold_false_emissions: usize,
    duplicate_ids: usize,
    payload_chars: usize,
    max_payload_chars: usize,
    emitted: usize,
    with_spans: usize,
    withheld_cases: usize,
    withheld_gold_hits: usize,
    expanded: usize,
    expanded_with_spans: usize,
    wall_ms: u128,
    capability_gap: usize,
}

impl Totals {
    fn record(
        &mut self,
        met: bool,
        hit: bool,
        false_emission: bool,
        deltas: &[serde_json::Value],
        wall: Duration,
    ) {
        self.cases += 1;
        self.expectation_met += usize::from(met);
        self.gold_hits += usize::from(hit);
        self.gold_false_emissions += usize::from(false_emission);
        let ids: Vec<&str> = deltas
            .iter()
            .filter_map(|delta| delta["id"].as_str())
            .collect();
        let unique: BTreeSet<&str> = ids.iter().copied().collect();
        self.duplicate_ids += ids.len().saturating_sub(unique.len());
        let chars: usize = deltas
            .iter()
            .map(|delta| delta["payload_chars"].as_u64().unwrap_or_default() as usize)
            .sum();
        self.payload_chars += chars;
        self.max_payload_chars = self.max_payload_chars.max(chars);
        self.emitted += deltas.len();
        self.with_spans += deltas
            .iter()
            .filter(|delta| {
                delta["spans"]
                    .as_array()
                    .is_some_and(|spans| !spans.is_empty())
            })
            .count();
        for delta in deltas.iter().filter(|delta| delta["expanded"] == true) {
            self.expanded += 1;
            if delta["spans"]
                .as_array()
                .is_some_and(|spans| !spans.is_empty())
            {
                self.expanded_with_spans += 1;
            }
        }
        self.wall_ms += wall.as_millis();
    }

    fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "cases": self.cases,
            "expectation_met": self.expectation_met,
            "expectation_rate": ratio_precise(self.expectation_met, self.cases),
            "gold_hits": self.gold_hits,
            "gold_false_emissions": self.gold_false_emissions,
            "duplicate_ids": self.duplicate_ids,
            "mean_payload_chars": ratio_precise(self.payload_chars, self.cases),
            "max_payload_chars": self.max_payload_chars,
            "mean_emitted": ratio_precise(self.emitted, self.cases),
            "span_coverage": ratio_precise(self.with_spans, self.emitted),
            "withheld_cases": self.withheld_cases,
            "withheld_gold_hits": self.withheld_gold_hits,
            "withheld_recall": ratio_precise(self.withheld_gold_hits, self.withheld_cases),
            "expanded_deltas": self.expanded,
            "expanded_span_coverage": ratio_precise(self.expanded_with_spans, self.expanded),
            "mean_wall_ms": ratio_precise(self.wall_ms as usize, self.cases),
            "capability_gap": self.capability_gap,
        })
    }
}

fn ratio_precise(numerator: usize, denominator: usize) -> f64 {
    if denominator == 0 {
        0.0
    } else {
        numerator as f64 / denominator as f64
    }
}

fn expected_emit(class: &str, variant: Variant) -> Option<bool> {
    match (class, variant) {
        ("direct", _) | ("conflict", _) => Some(true),
        ("withheld", Variant::IndexedExpanded) => Some(true),
        ("withheld", _) => Some(false),
        ("negative_control" | "negative_past" | "ambiguous_month", _) => Some(false),
        _ => None,
    }
}

fn seed_index(path: &std::path::Path, case: &Case, event: &Event) {
    let index = EvidenceIndex::open(path, &index_config(path)).expect("index opens");
    if case.class == "withheld" {
        let subject = distinctive(&case.product);
        let gold_subject = format!("{}-{}", event.product, event.version);
        let bridge_url = format!("https://endoflife.date/{}/{}", case.product, subject);
        index
            .retain(&[
                record(
                    &bridge_url,
                    &format!("{subject} release notes"),
                    &format!("pkg:{subject} references pkg:{gold_subject}"),
                    &event.date,
                    "src_bridge",
                ),
                record(
                    &card_url(event),
                    &gold_subject,
                    &format!("pkg:{gold_subject} retained release evidence"),
                    &event.date,
                    "src_gold",
                ),
            ])
            .expect("seed withheld index");
    } else {
        index
            .retain(&[record(
                &card_url(event),
                &format!("{} {} release notes", event.product, event.version),
                &format!("{} {} release notes", event.product, event.version),
                &event.date,
                "src_gold",
            )])
            .expect("seed index");
    }
}

fn live_cards(case: &Case, event: &Event, events: &[Event]) -> Vec<MockResult> {
    if case.class == "withheld" {
        return vec![undated_card(&case.product)];
    }
    let mut cards = vec![dated_card(event)];
    let mut peers: Vec<(i64, &Event)> = events
        .iter()
        .filter(|other| other.product != case.product)
        .map(|other| (date_distance(&other.date, &event.date), other))
        .collect();
    peers.sort_by(|left, right| left.0.cmp(&right.0).then(left.1.id.cmp(&right.1.id)));
    for (_, peer) in peers.into_iter().take(5) {
        cards.push(dated_card(peer));
    }
    if case.class == "conflict" {
        let mut duplicate = dated_card(event);
        duplicate.published_at = Some(shift_days(&event.date, 10));
        cards.push(duplicate);
    }
    cards.push(undated_card(&case.product));
    cards
}

fn parse_day(value: &str) -> NaiveDate {
    NaiveDate::parse_from_str(value, "%Y-%m-%d").expect("corpus date")
}

fn date_distance(left: &str, right: &str) -> i64 {
    parse_day(left)
        .signed_duration_since(parse_day(right))
        .num_days()
        .abs()
}

fn shift_days(date: &str, days: i64) -> String {
    (parse_day(date) + ChronoDuration::days(days))
        .format("%Y-%m-%d")
        .to_string()
}

async fn run_case(
    case: &Case,
    event: &Event,
    events: &[Event],
    variant: Variant,
) -> (serde_json::Value, Duration) {
    let dir = tempfile::tempdir().expect("temp dir");
    let mut cfg = AppConfig::default();
    cfg.fetch.sanitize_output = false;
    let refresh = match variant {
        Variant::DirectLive => RefreshMode::Live,
        Variant::IndexedLexical | Variant::IndexedExpanded => RefreshMode::Index,
    };
    let related = variant == Variant::IndexedExpanded;
    if variant != Variant::DirectLive {
        let path = private_index_path(dir.path(), "index");
        seed_index(&path, case, event);
        cfg.knowledge_delta.index = index_config(&path);
    }
    let engines = mock_engines(vec![MockEngine::success(
        PROVIDER,
        live_cards(case, event, events),
    )]);
    let adapter = MetadataSearchAdapter::from_engines(engines, Duration::from_secs(5));
    let run = BriefRun {
        adapter: &adapter,
        config: &cfg,
        max_results_cap: cfg.search.max_results_cap,
        config_dir: dir.path().to_path_buf(),
        evidence_fetcher: None,
    };
    let started = Instant::now();
    let value = run_meta_brief(&run, &query_for(case, refresh, related))
        .await
        .expect("brief run");
    (value, started.elapsed())
}

fn assert_run_invariants(case: &Case, variant: Variant, value: &serde_json::Value) {
    let deltas = value["deltas"].as_array().expect("deltas array");
    assert!(
        deltas.len() <= MAX_ITEMS,
        "{} items for {}",
        deltas.len(),
        case.id
    );
    let ids: Vec<&str> = deltas
        .iter()
        .filter_map(|delta| delta["id"].as_str())
        .collect();
    let unique: BTreeSet<&str> = ids.iter().copied().collect();
    assert_eq!(ids.len(), unique.len(), "duplicate ids in {}", case.id);
    let mut total = 0usize;
    for delta in deltas {
        assert_eq!(delta["trust"], "external_untrusted");
        assert!(delta["id"]
            .as_str()
            .is_some_and(|id| id.starts_with("kdelta_")));
        assert!(delta["source_ids"]
            .as_array()
            .is_some_and(|ids| !ids.is_empty()));
        assert!(delta["urls"].as_array().is_some_and(|urls| {
            urls.iter().all(|url| {
                url.as_str()
                    .is_some_and(|value| value.starts_with("https://"))
            })
        }));
        total += delta["payload_chars"].as_u64().unwrap_or_default() as usize;
    }
    assert!(total <= MAX_CHARS, "{} chars in {}", total, case.id);
    if variant == Variant::DirectLive {
        assert_eq!(value["coverage"]["index_status"], "not_opened");
        let capable = value["coverage"]["timestamp_capable_providers"]
            .as_array()
            .expect("capability list");
        assert!(
            !capable.is_empty(),
            "{} ran without a timestamp-capable provider",
            case.id
        );
        assert_eq!(value["coverage"]["selected_providers"][0], PROVIDER);
    } else {
        assert_eq!(
            value["coverage"]["index_status"], "open",
            "{} did not open the seeded index",
            case.id
        );
    }
}

#[tokio::test]
async fn corpus_meets_the_declared_composition() {
    let corpus: Corpus = serde_json::from_str(CORPUS).expect("corpus parses");
    assert_eq!(corpus.meta.case_count, corpus.cases.len());
    assert!(corpus.meta.case_count >= corpus.meta.requirement.dated_cases);
    let class_count = |name: &str| corpus.meta.counts.get(name).copied().unwrap_or_default();
    assert!(class_count("withheld") >= corpus.meta.requirement.withheld_name);
    assert!(
        class_count("negative_control") + class_count("negative_past")
            >= corpus.meta.requirement.negative_controls
    );
    assert!(
        class_count("ambiguous_month") + class_count("conflict")
            >= corpus.meta.requirement.ambiguous_or_conflicting
    );
    assert!(!corpus.meta.provider_profile.is_empty());
    assert!(corpus
        .meta
        .provider_profile
        .iter()
        .all(|provider| corpus.meta.timestamp_capable_providers.contains(provider)));
    let ids: BTreeSet<&str> = corpus.cases.iter().map(|case| case.id.as_str()).collect();
    assert_eq!(ids.len(), corpus.cases.len());
    for case in &corpus.cases {
        assert!(
            corpus
                .events
                .iter()
                .any(|event| event.id == case.gold_event_id),
            "{} has no gold event",
            case.id
        );
        assert!(matches!(
            case.cutoff.precision.as_str(),
            "day" | "month" | "year" | "unknown"
        ));
    }
    let event_ids: BTreeSet<&str> = corpus
        .events
        .iter()
        .map(|event| event.id.as_str())
        .collect();
    assert_eq!(event_ids.len(), corpus.events.len());
}

#[tokio::test]
async fn offline_variant_qualification() {
    let corpus: Corpus = serde_json::from_str(CORPUS).expect("corpus parses");
    let events: BTreeMap<&str, &Event> = corpus
        .events
        .iter()
        .map(|event| (event.id.as_str(), event))
        .collect();
    let variants = [
        Variant::DirectLive,
        Variant::IndexedLexical,
        Variant::IndexedExpanded,
    ];
    let mut totals: BTreeMap<&'static str, Totals> = variants
        .iter()
        .map(|variant| (variant.label(), Totals::default()))
        .collect();
    let mut per_class: BTreeMap<String, usize> = BTreeMap::new();
    let mut negative_failures = Vec::new();
    for case in &corpus.cases {
        let event = events.get(case.gold_event_id.as_str()).expect("gold event");
        let expected = gold_id(event);
        for variant in variants {
            let (value, wall) = run_case(case, event, &corpus.events, variant).await;
            assert_run_invariants(case, variant, &value);
            let deltas = value["deltas"].as_array().expect("deltas");
            let hit = deltas
                .iter()
                .any(|delta| delta["id"].as_str() == Some(expected.as_str()));
            let wanted = expected_emit(&case.class, variant).unwrap_or(false);
            let met = if case.class == "conflict" {
                let occurrences = deltas
                    .iter()
                    .filter(|delta| {
                        delta["urls"].as_array().is_some_and(|urls| {
                            urls.iter()
                                .any(|url| url.as_str() == Some(card_url(event).as_str()))
                        })
                    })
                    .count();
                occurrences <= 1 && hit
            } else {
                hit == wanted
            };
            if !met
                && matches!(
                    case.class.as_str(),
                    "negative_control" | "negative_past" | "ambiguous_month"
                )
            {
                negative_failures.push(format!("{}:{}:{}", case.id, variant.label(), hit));
            }
            *per_class
                .entry(format!("{}:{}", case.class, variant.label()))
                .or_default() += usize::from(met);
            let entry = totals
                .get_mut(variant.label())
                .expect("variant accumulator");
            let false_emission = hit
                && matches!(
                    case.class.as_str(),
                    "negative_control" | "negative_past" | "ambiguous_month"
                );
            entry.record(met, hit, false_emission, deltas, wall);
            if case.class == "withheld" {
                entry.withheld_cases += 1;
                entry.withheld_gold_hits += usize::from(hit);
            }
            if variant == Variant::DirectLive
                && value["coverage"]["timestamp_capable_providers"]
                    .as_array()
                    .map(Vec::len)
                    .unwrap_or_default()
                    == 0
            {
                entry.capability_gap += 1;
            }
        }
    }
    assert!(
        negative_failures.is_empty(),
        "false temporal claims: {negative_failures:?}"
    );
    let report = serde_json::json!({
        "corpus": {
            "cases": corpus.meta.case_count,
            "classes": corpus.meta.counts,
            "provider_profile": corpus.meta.provider_profile,
            "timestamp_capable_providers": corpus.meta.timestamp_capable_providers,
        },
        "variants": totals
            .iter()
            .map(|(name, total)| ((*name).to_string(), total.to_json()))
            .collect::<BTreeMap<_, _>>(),
        "expectations_by_class": per_class,
    });
    println!("{}", serde_json::to_string_pretty(&report).expect("report"));
    for (name, total) in &totals {
        assert_eq!(total.duplicate_ids, 0, "{name} duplicated deltas");
        assert_eq!(total.gold_false_emissions, 0, "{name} false emissions");
        assert_eq!(total.capability_gap, 0, "{name} capability gap");
        assert_eq!(total.cases, corpus.cases.len());
    }
}

#[tokio::test]
async fn repeated_runs_are_identical() {
    let corpus: Corpus = serde_json::from_str(CORPUS).expect("corpus parses");
    let events: BTreeMap<&str, &Event> = corpus
        .events
        .iter()
        .map(|event| (event.id.as_str(), event))
        .collect();
    let sample: Vec<&Case> = corpus
        .cases
        .iter()
        .filter(|case| case.class == "direct" || case.class == "conflict")
        .take(6)
        .collect();
    for case in sample {
        let event = events.get(case.gold_event_id.as_str()).expect("gold event");
        for variant in [Variant::DirectLive, Variant::IndexedExpanded] {
            let (first, _) = run_case(case, event, &corpus.events, variant).await;
            let (second, _) = run_case(case, event, &corpus.events, variant).await;
            assert_eq!(
                stable_view(&first),
                stable_view(&second),
                "{} repeated on {}",
                case.id,
                variant.label()
            );
        }
    }
}

fn stable_view(value: &serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "deltas": value["deltas"],
        "coverage": value["coverage"],
        "empty_state_reason": value["empty_state_reason"],
    })
}
