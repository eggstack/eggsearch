use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use serde::Deserialize;

#[cfg(feature = "mock")]
use eggsearch::core::query::SearchIntent;
#[cfg(feature = "mock")]
use std::sync::Arc;

const CORPUS: &str = include_str!("fixtures/retrieval_quality/cases.json");
const K: usize = 5;
const PASSAGE_BUDGET_CHARS: usize = 1200;
const EVALUATOR_REVISION: &str = "retrieval-quality-evaluator-v1";
#[cfg(feature = "mock")]
const SNAPSHOT_EVALUATOR_REVISION: &str = "retrieval-quality-candidate-snapshot-v1";
const PROVIDER_SCENARIO: &str =
    include_str!("fixtures/retrieval_quality/provider_contribution.json");
const CANDIDATE_SNAPSHOTS: &str =
    include_str!("fixtures/retrieval_quality/candidate_snapshots.json");
const RECORDED_SNAPSHOTS: &str = include_str!("fixtures/retrieval_quality/recorded_snapshots.json");
const SNAPSHOT_OUTPUT_ROOT: &str = "target/snapshots";
const RECORDER_MAX_RESULTS_PER_PROVIDER: usize = 20;

#[derive(Clone, Debug, Deserialize)]
struct Corpus {
    schema_version: u32,
    corpus_revision: String,
    provenance: String,
    annotation_uncertainty: String,
    #[serde(default)]
    tier: Option<String>,
    #[serde(default)]
    pending: Option<String>,
    cases: Vec<Case>,
}

#[derive(Clone, Debug, Deserialize)]
struct Case {
    id: String,
    category: String,
    partition: String,
    query: String,
    language: String,
    region: String,
    intent: String,
    retrieval_scope: String,
    #[serde(default)]
    provider_restrictions: Vec<String>,
    max_results: usize,
    #[serde(default)]
    temporal_window: Option<TemporalWindow>,
    judgments_complete: bool,
    expected_state: String,
    #[serde(default)]
    actual_state: Option<String>,
    #[serde(default)]
    freshness: Option<String>,
    #[serde(default)]
    observed_at: Option<String>,
    #[serde(default)]
    #[cfg(feature = "mock")]
    provider_lists: Vec<ProviderList>,
    #[serde(default)]
    #[cfg(feature = "mock")]
    routing: Option<CaseRouting>,
    relevant_documents: Vec<Judgment>,
    #[serde(default)]
    results: Vec<ResultEnvelope>,
}

#[cfg(feature = "mock")]
#[derive(Clone, Debug, Deserialize)]
struct CaseRouting {
    enabled_providers: Vec<String>,
    default_providers: Vec<String>,
    intent_mode: String,
    max_extra_providers: usize,
}

#[cfg(feature = "mock")]
#[derive(Clone, Debug, Deserialize)]
struct ProviderList {
    provider_id: String,
    results: Vec<ProviderRank>,
}

#[cfg(feature = "mock")]
#[derive(Clone, Debug, Deserialize)]
struct ProviderRank {
    rank: usize,
    title: String,
    url: String,
}

#[derive(Clone, Debug, Deserialize)]
struct TemporalWindow {
    start: String,
    end: String,
}

#[derive(Clone, Debug, Deserialize)]
struct Judgment {
    id: String,
    urls: Vec<String>,
    grade: u8,
    #[serde(default)]
    published_at: Option<String>,
    passages: Vec<Passage>,
}

#[derive(Clone, Debug, Deserialize)]
struct Passage {
    id: String,
    text: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, serde::Serialize)]
struct ResultEnvelope {
    id: String,
    url: String,
    title: String,
    snippet: String,
    #[serde(default)]
    published_at: Option<String>,
    #[serde(default)]
    passages: Vec<String>,
}

#[derive(Clone, Debug, Default)]
struct Metrics {
    precision_sum: f64,
    recall_sum: f64,
    reciprocal_rank_sum: f64,
    ndcg_sum: f64,
    passage_hits: usize,
    passage_denominator: usize,
    diversity_sum: f64,
    evidence_coverage_sum: f64,
    status_correct: usize,
    cases: usize,
    ranking_cases: usize,
    diversity_cases: usize,
    judged_results: usize,
    precision_denominator: usize,
    relevant_documents: usize,
    top_slots: usize,
    duplicate_slots: usize,
    distinct_domains_sum: f64,
    distinct_domains_cases: usize,
    authoritative_retained: usize,
    authoritative_documents: usize,
}

impl Metrics {
    fn add(&mut self, case: &Case) {
        let top = case.results.iter().take(case.max_results.min(K));
        let top: Vec<&ResultEnvelope> = top.collect();
        let relevant: Vec<&Judgment> = case.relevant_documents.iter().collect();
        let judgments: Vec<(usize, u8)> = top
            .iter()
            .enumerate()
            .filter_map(|(rank, result)| {
                relevant
                    .iter()
                    .find(|judgment| matches_case_document(case, judgment, result))
                    .map(|judgment| (rank, judgment.grade))
                    .or_else(|| case.judgments_complete.then_some((rank, 0)))
            })
            .collect();
        let hits = judgments.iter().filter(|(_, grade)| *grade > 0).count();
        let judged = judgments.len();
        let precision_denominator = if case.judgments_complete {
            case.max_results.min(K)
        } else {
            judged
        };
        let precision = if precision_denominator == 0 {
            0.0
        } else {
            hits as f64 / precision_denominator as f64
        };
        let recall = if relevant.is_empty() {
            1.0
        } else {
            let found: HashSet<&str> = top
                .iter()
                .filter_map(|result| {
                    relevant
                        .iter()
                        .find(|judgment| matches_case_document(case, judgment, result))
                        .map(|judgment| judgment.id.as_str())
                })
                .collect();
            found.len() as f64 / relevant.len() as f64
        };
        let reciprocal_rank = judgments
            .iter()
            .find(|(_, grade)| *grade > 0)
            .map(|(rank, _)| 1.0 / (rank + 1) as f64)
            .unwrap_or(0.0);
        let dcg: f64 = judgments
            .iter()
            .map(|(rank, grade)| (2_f64.powi(i32::from(*grade)) - 1.0) / ((rank + 2) as f64).log2())
            .sum();
        let mut ideal_grades: Vec<u8> = relevant.iter().map(|judgment| judgment.grade).collect();
        ideal_grades.sort_unstable_by(|a, b| b.cmp(a));
        let ideal: f64 = ideal_grades
            .iter()
            .take(K)
            .enumerate()
            .map(|(rank, grade)| (2_f64.powi(i32::from(*grade)) - 1.0) / ((rank + 2) as f64).log2())
            .sum();
        let ndcg = if ideal == 0.0 { 1.0 } else { dcg / ideal };
        let sources: HashSet<String> = top.iter().map(|result| source_host(&result.url)).collect();
        let diversity = if top.is_empty() {
            0.0
        } else {
            sources.len() as f64 / top.len() as f64
        };
        let mut clusters: HashSet<String> = HashSet::new();
        let mut duplicate_slots = 0usize;
        for result in &top {
            if !clusters.insert(normalized_cluster(&result.url)) {
                duplicate_slots += 1;
            }
        }
        let authoritative: Vec<&Judgment> = relevant
            .iter()
            .copied()
            .filter(|judgment| judgment.grade >= 2)
            .collect();
        let authoritative_retained = authoritative
            .iter()
            .filter(|judgment| {
                top.iter()
                    .any(|result| matches_case_document(case, judgment, result))
            })
            .count();
        let passage_hits = relevant
            .iter()
            .filter(|judgment| {
                top.iter().any(|result| {
                    matches_case_document(case, judgment, result)
                        && passage_match(judgment, &top, PASSAGE_BUDGET_CHARS)
                })
            })
            .count();
        let actual_state = case.actual_state.as_deref().unwrap_or({
            if case.results.is_empty() {
                "absence"
            } else {
                "results"
            }
        });
        let status = case.expected_state == actual_state;

        if !relevant.is_empty() && precision_denominator > 0 {
            self.precision_sum += precision;
            self.recall_sum += recall;
            self.reciprocal_rank_sum += reciprocal_rank;
            self.ndcg_sum += ndcg;
            self.evidence_coverage_sum += recall;
            self.ranking_cases += 1;
            self.precision_denominator += precision_denominator;
        }
        self.passage_hits += passage_hits;
        self.passage_denominator += relevant.iter().filter(|j| !j.passages.is_empty()).count();
        if !top.is_empty() {
            self.diversity_sum += diversity;
            self.distinct_domains_sum += sources.len() as f64;
            self.diversity_cases += 1;
            self.distinct_domains_cases += 1;
        }
        self.top_slots += top.len();
        self.duplicate_slots += duplicate_slots;
        self.authoritative_retained += authoritative_retained;
        self.authoritative_documents += authoritative.len();
        self.status_correct += usize::from(status);
        self.cases += 1;
        self.judged_results += judged;
        self.relevant_documents += relevant.len();
    }

    fn report(&self) -> serde_json::Value {
        let ranking_cases = self.ranking_cases.max(1) as f64;
        let diversity_cases = self.diversity_cases.max(1) as f64;
        let passage_interval = wilson(self.passage_hits, self.passage_denominator);
        let status_interval = wilson(self.status_correct, self.cases);
        serde_json::json!({
            "cases": self.cases,
            "ranking_cases": self.ranking_cases,
            "diversity_cases": self.diversity_cases,
            "denominators": {
                "judged_results": self.judged_results,
                "precision_at_5": self.precision_denominator,
                "relevant_documents": self.relevant_documents,
                "passages": self.passage_denominator,
                "top_slots": self.top_slots,
                "authoritative_documents": self.authoritative_documents
            },
            "precision_at_5": self.precision_sum / ranking_cases,
            "recall_at_5": self.recall_sum / ranking_cases,
            "mrr_at_5": self.reciprocal_rank_sum / ranking_cases,
            "ndcg_at_5": self.ndcg_sum / ranking_cases,
            "passage_hit_at_1200_chars": ratio(self.passage_hits, self.passage_denominator),
            "passage_hit_wilson_95": passage_interval,
            "source_diversity_at_5": self.diversity_sum / diversity_cases,
            "distinct_domains_topk": self.distinct_domains_sum
                / self.distinct_domains_cases.max(1) as f64,
            "duplicate_slots": self.duplicate_slots,
            "duplicate_slot_rate": ratio(self.duplicate_slots, self.top_slots),
            "authoritative_survival": ratio(self.authoritative_retained, self.authoritative_documents),
            "evidence_coverage": self.evidence_coverage_sum / ranking_cases,
            "status_accuracy": self.status_correct as f64 / self.cases.max(1) as f64,
            "status_accuracy_wilson_95": status_interval
        })
    }
}

fn wilson(successes: usize, trials: usize) -> serde_json::Value {
    if trials == 0 {
        return serde_json::json!({"lower": null, "upper": null, "trials": 0});
    }
    let z = 1.959_963_984_540_054;
    let n = trials as f64;
    let p = successes as f64 / n;
    let denominator = 1.0 + z * z / n;
    let center = (p + z * z / (2.0 * n)) / denominator;
    let margin = z * ((p * (1.0 - p) / n + z * z / (4.0 * n * n)).sqrt()) / denominator;
    serde_json::json!({
        "lower": (center - margin).max(0.0),
        "upper": (center + margin).min(1.0),
        "trials": trials
    })
}

fn ratio(numerator: usize, denominator: usize) -> f64 {
    if denominator == 0 {
        0.0
    } else {
        numerator as f64 / denominator as f64
    }
}

fn canonical_url(url: &str) -> String {
    eggsearch::core::identity::canonicalize_url(url)
}

fn normalized_cluster(url: &str) -> String {
    eggsearch::meta::engines::normalizer::normalize(url).unwrap_or_else(|| canonical_url(url))
}

fn matches_document(judgment: &Judgment, result: &ResultEnvelope) -> bool {
    judgment.id == result.id
        || judgment
            .urls
            .iter()
            .any(|alias| canonical_url(alias) == canonical_url(&result.url))
}

fn matches_case_document(case: &Case, judgment: &Judgment, result: &ResultEnvelope) -> bool {
    if !matches_document(judgment, result) {
        return false;
    }
    case.temporal_window.as_ref().is_none_or(|window| {
        result
            .published_at
            .as_ref()
            .is_some_and(|date| date >= &window.start && date <= &window.end)
    })
}

fn aliases_are_unambiguous(case: &Case) -> bool {
    let mut aliases = HashMap::<String, &str>::new();
    for judgment in &case.relevant_documents {
        for url in &judgment.urls {
            if aliases
                .insert(canonical_url(url), judgment.id.as_str())
                .is_some_and(|existing| existing != judgment.id.as_str())
            {
                return false;
            }
        }
    }
    true
}

fn passage_match(judgment: &Judgment, results: &[&ResultEnvelope], budget: usize) -> bool {
    let mut used = 0usize;
    let mut evidence = String::new();
    for result in results {
        for text in result.passages.iter().chain([&result.snippet]) {
            if used >= budget {
                break;
            }
            let bounded: String = text.chars().take(budget - used).collect();
            used += bounded.chars().count();
            evidence.push_str(&bounded);
        }
    }
    judgment
        .passages
        .iter()
        .any(|gold| evidence.contains(&gold.text))
}

fn source_host(url: &str) -> String {
    url.split_once("//")
        .map(|(_, rest)| {
            rest.split('/')
                .next()
                .unwrap_or_default()
                .trim_start_matches("www.")
                .to_string()
        })
        .unwrap_or_default()
}

fn live_intent(intent: &str) -> eggsearch::core::query::SearchIntent {
    use eggsearch::core::query::SearchIntent;

    match intent {
        "docs" => SearchIntent::Docs,
        "code" | "exact_error" => SearchIntent::Code,
        "issues" => SearchIntent::Issues,
        "releases" => SearchIntent::Releases,
        "security" => SearchIntent::Security,
        "news" => SearchIntent::News,
        "web" | "package" | "research" => SearchIntent::Web,
        other => panic!("unsupported task intent {other}"),
    }
}

fn fingerprint(bytes: &[u8]) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("fnv1a64:{hash:016x}")
}

fn evaluate(
    corpus: &Corpus,
    partition: Option<&str>,
) -> (serde_json::Value, BTreeMap<String, Metrics>) {
    let mut all = Metrics::default();
    let mut categories = BTreeMap::<String, Metrics>::new();
    for case in corpus
        .cases
        .iter()
        .filter(|case| partition.is_none_or(|p| case.partition == p))
    {
        all.add(case);
        categories
            .entry(case.category.clone())
            .or_default()
            .add(case);
    }
    (all.report(), categories)
}

fn offline_report(
    corpus: &Corpus,
    aggregate: serde_json::Value,
    categories: &BTreeMap<String, Metrics>,
    train: serde_json::Value,
    held_out: serde_json::Value,
) -> serde_json::Value {
    let build_features = [
        ("pdf", cfg!(feature = "pdf")),
        ("browser", cfg!(feature = "browser")),
        ("egress", cfg!(feature = "egress")),
        ("mock", cfg!(feature = "mock")),
    ]
    .into_iter()
    .filter_map(|(name, enabled)| enabled.then_some(name))
    .collect::<Vec<_>>();
    let per_category = categories
        .iter()
        .map(|(name, metrics)| (name.clone(), metrics.report()))
        .collect::<BTreeMap<_, _>>();
    let result_hashes = corpus
        .cases
        .iter()
        .map(|case| {
            (
                case.id.clone(),
                fingerprint(&serde_json::to_vec(&case.results).unwrap()),
            )
        })
        .collect::<BTreeMap<_, _>>();
    serde_json::json!({
        "schema_version": 1,
        "source_sha": std::env::var("EGGSEARCH_BENCH_SOURCE_SHA").ok(),
        "corpus_revision": corpus.corpus_revision,
        "corpus_fingerprint": fingerprint(CORPUS.as_bytes()),
        "algorithm_revision": EVALUATOR_REVISION,
        "build_features": build_features,
        "configuration_fingerprint": fingerprint(b"synthetic-corpus-replay-v1"),
        "cache_state": "synthetic_fixture_no_cache",
        "tie_break_seed": null,
        "aggregate": aggregate,
        "per_partition": {"train": train, "held_out": held_out},
        "per_category": per_category,
        "result_hashes": result_hashes
    })
}

#[test]
fn retrieval_quality_corpus_and_metrics_are_reproducible() {
    let corpus: Corpus = serde_json::from_str(CORPUS).expect("corpus parses");
    assert_eq!(corpus.schema_version, 1);
    assert_eq!(corpus.corpus_revision, "retrieval-quality-v1");
    assert!(!corpus.provenance.is_empty());
    assert!(!corpus.annotation_uncertainty.is_empty());
    assert!(corpus.cases.len() >= 12);
    let ids: BTreeSet<&str> = corpus.cases.iter().map(|case| case.id.as_str()).collect();
    assert_eq!(ids.len(), corpus.cases.len());
    for case in &corpus.cases {
        assert!(
            aliases_are_unambiguous(case),
            "gold URL aliases must identify one judgment"
        );
        assert!(!case.query.is_empty());
        assert!(!case.language.is_empty() && !case.region.is_empty());
        assert!(!case.intent.is_empty() && !case.retrieval_scope.is_empty());
        assert!(case.provider_restrictions.iter().all(|provider| {
            eggsearch::core::provider::KNOWN_PROVIDER_IDS.contains(&provider.as_str())
        }));
        assert!(case.max_results > 0);
        assert!(matches!(
            case.expected_state.as_str(),
            "results" | "absence" | "failure"
        ));
        for judgment in &case.relevant_documents {
            assert!((1..=3).contains(&judgment.grade));
            assert!(!judgment.urls.is_empty());
            for passage in &judgment.passages {
                assert!(!passage.id.is_empty() && !passage.text.is_empty());
            }
            if let (Some(window), Some(date)) = (&case.temporal_window, &judgment.published_at) {
                assert!(date >= &window.start && date <= &window.end);
            }
        }
        for result in &case.results {
            assert!(!result.title.is_empty());
            if let Some(window) = &case.temporal_window {
                if let Some(date) = &result.published_at {
                    assert!(date >= &window.start && date <= &window.end);
                }
            }
        }
    }
    let (first, categories) = evaluate(&corpus, None);
    let (second, _) = evaluate(&corpus, None);
    let (train, _) = evaluate(&corpus, Some("train"));
    let (held_out, _) = evaluate(&corpus, Some("held_out"));
    assert_eq!(first, second);
    assert_eq!(
        categories.len(),
        corpus
            .cases
            .iter()
            .map(|case| &case.category)
            .collect::<BTreeSet<_>>()
            .len()
    );
    assert_eq!(first["cases"], corpus.cases.len());
    assert_eq!(first["status_accuracy"], 1.0);
    assert!(first["recall_at_5"].as_f64().unwrap() < 1.0);
    assert!(corpus.cases.iter().any(|case| case.partition == "held_out"));
    assert!(corpus.cases.iter().any(|case| !case.judgments_complete));
    let _metadata = corpus
        .cases
        .iter()
        .map(|case| (case.id.as_str(), case.partition.as_str()))
        .collect::<Vec<_>>();
    let corpus_hash = fingerprint(CORPUS.as_bytes());
    assert!(corpus_hash.starts_with("fnv1a64:"));
    println!(
        "retrieval-quality-offline-report {}",
        serde_json::to_string(&offline_report(
            &corpus,
            first,
            &categories,
            train,
            held_out
        ))
        .unwrap()
    );
}

#[derive(Debug, Deserialize)]
struct ProviderScenario {
    schema_version: u32,
    provenance: String,
    task: ProviderTask,
    providers: Vec<ProviderEnvelope>,
}

#[derive(Debug, Deserialize)]
struct ProviderTask {
    id: String,
    category: String,
    query: String,
    relevant_documents: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ProviderEnvelope {
    id: String,
    outcome: String,
    documents: Vec<String>,
}

#[test]
fn synthetic_provider_contribution_and_overlap_are_reproducible() {
    let scenario: ProviderScenario = serde_json::from_str(PROVIDER_SCENARIO).unwrap();
    assert_eq!(scenario.schema_version, 1);
    assert!(scenario.provenance.contains("synthetic"));
    assert_eq!(scenario.task.id, "generic-web-provider-overlap-001");
    assert_eq!(scenario.task.category, "generic_web");
    assert!(!scenario.task.query.is_empty());
    let relevant: HashSet<&str> = scenario
        .task
        .relevant_documents
        .iter()
        .map(String::as_str)
        .collect();
    let mut individual_hits = BTreeMap::new();
    let mut combined = HashSet::new();
    for provider in scenario.providers.iter().filter(|p| p.outcome == "success") {
        let hits: HashSet<&str> = provider
            .documents
            .iter()
            .map(String::as_str)
            .filter(|id| relevant.contains(id))
            .collect();
        individual_hits.insert(provider.id.as_str(), hits.len());
        combined.extend(hits);
    }
    assert_eq!(individual_hits.get("duckduckgo"), Some(&1));
    assert_eq!(individual_hits.get("startpage"), Some(&2));
    assert_eq!(individual_hits.get("yahoo"), Some(&2));
    assert_eq!(combined.len(), 3);
    assert_eq!(combined.len() as f64 / relevant.len() as f64, 1.0);
    let provider_hits: Vec<HashSet<&str>> = scenario
        .providers
        .iter()
        .filter(|p| p.outcome == "success")
        .map(|provider| {
            provider
                .documents
                .iter()
                .map(String::as_str)
                .filter(|id| relevant.contains(id))
                .collect()
        })
        .collect();
    let overlap: HashSet<&str> = provider_hits
        .iter()
        .flat_map(|hits| hits.iter().copied())
        .filter(|id| {
            provider_hits
                .iter()
                .filter(|hits| hits.contains(id))
                .count()
                > 1
        })
        .collect();
    assert_eq!(overlap.len(), 2);
    let selected: Vec<&str> = scenario
        .providers
        .iter()
        .filter(|p| p.outcome == "success")
        .map(|p| p.id.as_str())
        .collect();
    assert_eq!(selected, ["duckduckgo", "startpage", "yahoo"]);
    println!(
        "provider-contribution-fixture task={} category={} providers={:?} individual_relevant_hits={:?} combined_recall={}/{} overlap_relevant_documents={} upstream_observation=false",
        scenario.task.id,
        scenario.task.category,
        selected,
        individual_hits,
        combined.len(),
        relevant.len(),
        overlap.len()
    );
}

#[test]
fn deliberately_degraded_ranking_fails_the_known_quality_gate() {
    let mut corpus: Corpus = serde_json::from_str(CORPUS).expect("corpus parses");
    let case = corpus
        .cases
        .iter_mut()
        .find(|case| case.id == "code-error-001")
        .unwrap();
    case.results.remove(1);
    let mut degraded = Metrics::default();
    degraded.add(case);
    let score = degraded.report();
    assert_eq!(score["mrr_at_5"], 0.0);
    assert_eq!(score["ndcg_at_5"], 0.0);
}

#[test]
fn incomplete_judgments_exclude_unjudged_items_from_precision() {
    let corpus: Corpus = serde_json::from_str(CORPUS).expect("corpus parses");
    let case = corpus
        .cases
        .iter()
        .find(|case| case.id == "incomplete-judgments-001")
        .unwrap();
    let mut metrics = Metrics::default();
    metrics.add(case);
    let score = metrics.report();
    assert_eq!(score["precision_at_5"], 1.0);
    assert_eq!(score["denominators"]["judged_results"], 1);
}

#[test]
fn canonical_url_aliases_match_after_url_normalization() {
    let judgment = Judgment {
        id: "gold-document".into(),
        urls: vec!["https://example.test/path".into()],
        grade: 3,
        published_at: None,
        passages: Vec::new(),
    };
    let result = ResultEnvelope {
        id: "provider-specific-id".into(),
        url: "HTTPS://WWW.EXAMPLE.TEST/path/#section".into(),
        title: "Gold document".into(),
        snippet: String::new(),
        published_at: None,
        passages: Vec::new(),
    };
    assert!(matches_document(&judgment, &result));
    let mut distinct_query = result;
    distinct_query.url = "https://www.example.test/path/?revision=2".into();
    assert!(!matches_document(&judgment, &distinct_query));
}

#[test]
fn ambiguous_gold_url_aliases_are_rejected() {
    let mut corpus: Corpus = serde_json::from_str(CORPUS).expect("corpus parses");
    let case = corpus
        .cases
        .iter_mut()
        .find(|case| case.id == "security-applicability-001")
        .unwrap();
    case.relevant_documents[1].urls = case.relevant_documents[0].urls.clone();
    assert!(!aliases_are_unambiguous(case));
}

#[test]
fn results_outside_the_task_date_window_do_not_match_gold() {
    let corpus: Corpus = serde_json::from_str(CORPUS).expect("corpus parses");
    let mut case = corpus
        .cases
        .iter()
        .find(|case| case.id == "freshness-date-001")
        .unwrap()
        .clone();
    case.results[0].published_at = Some("2025-12-31".into());
    let mut metrics = Metrics::default();
    metrics.add(&case);
    assert_eq!(metrics.report()["recall_at_5"], 0.0);
}

#[cfg(feature = "mock")]
#[tokio::test]
async fn synthetic_reference_envelope_replays_through_search_adapter_and_tool_handler() {
    use std::sync::Arc;
    use std::time::Duration;

    use eggsearch::core::config::AppConfig;
    use eggsearch::mcp::state::ServerState;
    use eggsearch::mcp::tools::{run_web_search, WebSearchArgs};
    use eggsearch::meta::mock::{mock_engines, MockEngine, MockResult};
    use eggsearch::meta::MetadataSearchAdapter;

    let corpus: Corpus = serde_json::from_str(CORPUS).expect("corpus parses");
    let case = corpus
        .cases
        .iter()
        .find(|case| case.id == "code-error-001")
        .unwrap();
    let gold = &case.relevant_documents[0];
    let passage = &gold.passages[0].text;
    let engine = MockEngine::success(
        "mock_a",
        vec![MockResult::new("Error E0277", &gold.urls[0], "mock_a").with_snippet(passage)],
    );
    let adapter =
        MetadataSearchAdapter::from_engines(mock_engines(vec![engine]), Duration::from_secs(2));
    let mut config = AppConfig::default();
    config.search.providers.insert("mock_a".into(), true);
    let state = Arc::new(ServerState::with_adapter(config, Arc::new(adapter)));
    let payload = run_web_search(
        state,
        WebSearchArgs {
            query: case.query.clone(),
            max_results: Some(case.max_results),
            providers: vec!["mock_a".into()],
            safe_search: None,
            timeout_ms: None,
            intent: None,
            freshness: None,
            date_range: None,
            include_domains: Vec::new(),
            exclude_domains: Vec::new(),
            language: Some(case.language.clone()),
            region: Some(case.region.clone()),
            excerpt_count: None,
            response_detail: None,
        },
    )
    .await
    .expect("mock search succeeds");
    let results: Vec<ResultEnvelope> = payload["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| ResultEnvelope {
            id: item["stable_id"].as_str().unwrap_or_default().to_owned(),
            url: item["url"].as_str().unwrap_or_default().to_owned(),
            title: item["title"].as_str().unwrap_or_default().to_owned(),
            snippet: item["snippet"].as_str().unwrap_or_default().to_owned(),
            published_at: None,
            passages: Vec::new(),
        })
        .collect();
    let mut replay = case.clone();
    replay.results = results;
    let mut metrics = Metrics::default();
    metrics.add(&replay);
    assert_eq!(metrics.report()["recall_at_5"], 1.0);
}

#[cfg(feature = "mock")]
fn replay_case(
    case: &Case,
    knobs: eggsearch::selection_test::SelectionKnobs,
) -> Vec<ResultEnvelope> {
    replay_case_with_intent(case, knobs, live_intent(&case.intent))
}

#[cfg(feature = "mock")]
fn case_provider_lists(
    case: &Case,
    only: Option<&[String]>,
) -> Vec<(String, Vec<eggsearch::meta::engines::models::SearchResult>)> {
    use eggsearch::meta::engines::models::{ResultMetadata, SearchResult};

    assert!(
        case.results.is_empty(),
        "snapshot case {} must score selection output, not a recorded result list",
        case.id
    );
    assert!(
        !case.provider_lists.is_empty(),
        "snapshot case {} has no provider lists",
        case.id
    );
    assert_eq!(
        case.freshness.as_deref(),
        Some("any"),
        "snapshot replay cannot exercise freshness: recorded rows carry no timestamps"
    );
    let mut lists: Vec<(String, Vec<SearchResult>)> = Vec::new();
    for list in &case.provider_lists {
        if only.is_some_and(|ids| !ids.iter().any(|id| id == &list.provider_id)) {
            continue;
        }
        assert!(
            eggsearch::core::provider::KNOWN_PROVIDER_IDS.contains(&list.provider_id.as_str()),
            "snapshot case {} uses unknown provider {}",
            case.id,
            list.provider_id
        );
        let mut ranked: Vec<&ProviderRank> = list.results.iter().collect();
        ranked.sort_by_key(|entry| entry.rank);
        let ranks: Vec<usize> = ranked.iter().map(|entry| entry.rank).collect();
        let expected: Vec<usize> = (1..=ranked.len()).collect();
        assert_eq!(
            ranks, expected,
            "provider ranks must be contiguous from 1 in {}",
            list.provider_id
        );
        let results: Vec<SearchResult> = ranked
            .into_iter()
            .map(|entry| SearchResult {
                title: entry.title.clone(),
                url: entry.url.clone(),
                snippet: None,
                source_engine: list.provider_id.clone(),
                metadata: ResultMetadata::None,
                excerpts: Vec::new(),
                published_at: None,
            })
            .collect();
        lists.push((list.provider_id.clone(), results));
    }
    if let Some(ids) = only {
        for id in ids {
            assert!(
                case.provider_lists
                    .iter()
                    .any(|list| &list.provider_id == id),
                "snapshot case {} routes {id} but records no candidate list for it",
                case.id
            );
        }
    }
    lists
}

#[cfg(feature = "mock")]
fn replay_case_with_intent(
    case: &Case,
    knobs: eggsearch::selection_test::SelectionKnobs,
    intent: eggsearch::core::query::SearchIntent,
) -> Vec<ResultEnvelope> {
    replay_case_with_subset(case, knobs, intent, None)
}

#[cfg(feature = "mock")]
fn replay_case_with_subset(
    case: &Case,
    knobs: eggsearch::selection_test::SelectionKnobs,
    intent: eggsearch::core::query::SearchIntent,
    only: Option<&[String]>,
) -> Vec<ResultEnvelope> {
    let lists = case_provider_lists(case, only);
    let rows = if knobs == eggsearch::selection_test::SelectionKnobs::production() {
        eggsearch::selection_test::select(lists, case.max_results, intent)
    } else {
        eggsearch::selection_test::select_with_knobs(lists, case.max_results, intent, knobs)
    };
    rows.into_iter()
        .map(|row| ResultEnvelope {
            id: row.stable_id,
            url: row.url,
            title: row.title,
            snippet: String::new(),
            published_at: None,
            passages: Vec::new(),
        })
        .collect()
}

#[cfg(feature = "mock")]
fn replay_corpus(corpus: &Corpus, knobs: eggsearch::selection_test::SelectionKnobs) -> Corpus {
    let mut replayed = corpus.clone();
    for case in &mut replayed.cases {
        if !case.provider_lists.is_empty() {
            case.results = replay_case(case, knobs);
        }
    }
    replayed
}

#[cfg(feature = "mock")]
fn replay_corpus_with_intent<F>(
    corpus: &Corpus,
    knobs: eggsearch::selection_test::SelectionKnobs,
    intent_for: F,
) -> Corpus
where
    F: Fn(&Case) -> eggsearch::core::query::SearchIntent,
{
    let mut replayed = corpus.clone();
    for case in &mut replayed.cases {
        if !case.provider_lists.is_empty() {
            let intent = intent_for(case);
            case.results = replay_case_with_intent(case, knobs, intent);
        }
    }
    replayed
}

#[cfg(feature = "mock")]
fn case_metrics(corpus: &Corpus, id: &str) -> serde_json::Value {
    let case = corpus
        .cases
        .iter()
        .find(|case| case.id == id)
        .unwrap_or_else(|| panic!("missing case {id}"));
    let mut metrics = Metrics::default();
    metrics.add(case);
    metrics.report()
}

#[cfg(feature = "mock")]
fn metric(value: &serde_json::Value, key: &str) -> f64 {
    value[key]
        .as_f64()
        .unwrap_or_else(|| panic!("metric {key} is not a number: {value}"))
}

#[cfg(feature = "mock")]
fn snapshot_report(corpus: &Corpus, aggregate: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "schema_version": 1,
        "corpus_revision": corpus.corpus_revision,
        "tier": corpus.tier,
        "corpus_fingerprint": fingerprint(CANDIDATE_SNAPSHOTS.as_bytes()),
        "algorithm_revision": SNAPSHOT_EVALUATOR_REVISION,
        "build_features": ["mock"],
        "tie_break_seed": null,
        "aggregate": aggregate,
        "per_case": corpus
            .cases
            .iter()
            .map(|case| (case.id.clone(), case_metrics(corpus, &case.id)))
            .collect::<BTreeMap<_, _>>(),
    })
}

#[cfg(feature = "mock")]
struct SignalCase {
    case_id: &'static str,
    annotated_intent: &'static str,
    intent_document_url: &'static str,
    competitor_url: &'static str,
    promotes_into_window: bool,
}

#[cfg(feature = "mock")]
const SIGNAL_CASES: [SignalCase; 4] = [
    SignalCase {
        case_id: "security-derived-promotion-001",
        annotated_intent: "security",
        intent_document_url: "https://osv.dev/vulnerability/CVE-2024-1234",
        competitor_url: "https://blog.example.test/openssl-cert-rotation",
        promotes_into_window: true,
    },
    SignalCase {
        case_id: "releases-derived-promotion-001",
        annotated_intent: "releases",
        intent_document_url: "https://example.test/projects/axum/CHANGELOG.md",
        competitor_url: "https://blog.example.test/axum-request-lifecycle",
        promotes_into_window: true,
    },
    SignalCase {
        case_id: "docs-derived-promotion-001",
        annotated_intent: "docs",
        intent_document_url: "https://docs.rs/tower-http/latest/tower_http/trace/index.html",
        competitor_url: "https://blog.example.test/middleware-composition",
        promotes_into_window: false,
    },
    SignalCase {
        case_id: "code-derived-promotion-001",
        annotated_intent: "code",
        intent_document_url: "https://github.com/example/router/blob/main/src/layer.rs",
        competitor_url: "https://blog.example.test/router-layer-explained",
        promotes_into_window: true,
    },
];

#[cfg(feature = "mock")]
fn row_rank(rows: &[ResultEnvelope], url: &str) -> Option<usize> {
    let target = canonical_url(url);
    rows.iter()
        .position(|row| canonical_url(&row.url) == target)
}

#[cfg(feature = "mock")]
#[test]
fn candidate_snapshot_corpus_is_structured_and_replays_deterministically() {
    let corpus: Corpus = serde_json::from_str(CANDIDATE_SNAPSHOTS).expect("snapshot corpus parses");
    assert_eq!(corpus.schema_version, 2);
    assert_eq!(corpus.corpus_revision, "candidate-snapshots-v1");
    assert_eq!(corpus.tier.as_deref(), Some("structured_synthetic"));
    assert!(corpus
        .provenance
        .contains("do not reproduce any provider response"));
    assert!(corpus.provenance.contains("page bodies"));
    assert!(!corpus.annotation_uncertainty.is_empty());
    assert!(corpus.cases.len() >= 7);
    let ids: BTreeSet<&str> = corpus.cases.iter().map(|case| case.id.as_str()).collect();
    assert_eq!(ids.len(), corpus.cases.len());
    let partitions: BTreeSet<&str> = corpus
        .cases
        .iter()
        .map(|case| case.partition.as_str())
        .collect();
    assert_eq!(partitions.len(), 2);
    for case in &corpus.cases {
        assert!(aliases_are_unambiguous(case));
        assert!(!case.query.is_empty());
        assert!(case.max_results > 0);
        assert!(matches!(
            case.expected_state.as_str(),
            "results" | "absence"
        ));
        assert_eq!(case.freshness.as_deref(), Some("any"));
        assert!(!case.provider_lists.is_empty());
        assert!(case.provider_lists.len() >= 2);
        for judgment in &case.relevant_documents {
            assert!((1..=3).contains(&judgment.grade));
            assert!(!judgment.urls.is_empty());
        }
        assert!(case
            .relevant_documents
            .iter()
            .any(|judgment| judgment.grade == 3));
        for list in &case.provider_lists {
            assert!(
                case.provider_lists
                    .iter()
                    .filter(|other| other.provider_id == list.provider_id)
                    .count()
                    == 1
            );
            assert!(!list.results.is_empty());
            for entry in &list.results {
                assert!(!entry.title.is_empty());
                assert!(!entry.url.is_empty());
            }
        }
    }

    let mut signal_rows = Vec::new();
    for signal in &SIGNAL_CASES {
        let case = corpus
            .cases
            .iter()
            .find(|case| case.id == signal.case_id)
            .unwrap_or_else(|| panic!("signal case {} is missing", signal.case_id));
        assert_eq!(
            case.intent, signal.annotated_intent,
            "signal case {} must be annotated with the intent it exercises",
            case.id
        );
        let derived = eggsearch::selection_test::derive_intent(&case.query);
        assert_eq!(
            derived.intent.as_str(),
            signal.annotated_intent,
            "signal case {} must derive its annotated intent; classifier said {:?} with reasons {:?}",
            case.id,
            derived.intent,
            derived.reasons
        );
        assert!(
            !derived.reasons.is_empty() && derived.reasons != vec!["no_signal"],
            "signal case {} must carry lexical intent evidence, got {:?}",
            case.id,
            derived.reasons
        );

        let window = case.max_results.min(K);
        let web_rows = replay_case_with_intent(
            case,
            eggsearch::selection_test::SelectionKnobs::production(),
            SearchIntent::Web,
        );
        let derived_rows = replay_case_with_intent(
            case,
            eggsearch::selection_test::SelectionKnobs::production(),
            derived.intent,
        );
        let web_rank = row_rank(&web_rows, signal.intent_document_url);
        let derived_rank = row_rank(&derived_rows, signal.intent_document_url)
            .unwrap_or_else(|| panic!("{} must return its intent document", case.id));
        let web_competitor_rank = row_rank(&web_rows, signal.competitor_url)
            .unwrap_or_else(|| panic!("{} must return its competitor document", case.id));
        let derived_competitor_rank = row_rank(&derived_rows, signal.competitor_url)
            .unwrap_or_else(|| panic!("{} must return its competitor document", case.id));

        assert!(
            derived_rank < derived_competitor_rank,
            "{} must place its intent document above the generic competitor once the intent boosts it: intent={derived_rank} competitor={derived_competitor_rank}",
            case.id
        );
        assert!(
            derived_rank < window,
            "{} must promote its intent document into the final window: derived={derived_rank} window={window}",
            case.id
        );
        if signal.promotes_into_window {
            assert!(
                web_rank.is_none() && web_competitor_rank < window,
                "{} must keep its intent document outside the returned window under the neutral prior: intent={web_rank:?} competitor={web_competitor_rank} window={window}",
                case.id
            );
        } else {
            let web_rank = web_rank.unwrap_or_else(|| {
                panic!(
                    "{} must return its intent document under the neutral prior",
                    case.id
                )
            });
            assert!(
                web_competitor_rank < web_rank,
                "{} must place its intent document below the generic competitor under the neutral prior: competitor={web_competitor_rank} intent={web_rank}",
                case.id
            );
            assert!(
                derived_rank < web_rank,
                "{} must promote its intent document: web={web_rank} derived={derived_rank}",
                case.id
            );
        }
        signal_rows.push(serde_json::json!({
            "case": case.id,
            "annotated_intent": case.intent,
            "derived_intent": derived.intent.as_str(),
            "derived_reasons": derived.reasons,
            "derived_confidence": derived.confidence,
            "window": window,
            "intent_document": {
                "url": signal.intent_document_url,
                "intent_web_rank": web_rank,
                "derived_rank": derived_rank
            },
            "competitor": {
                "url": signal.competitor_url,
                "intent_web_rank": web_competitor_rank,
                "derived_rank": derived_competitor_rank
            }
        }));
    }
    println!(
        "retrieval-quality-signal-promotion {}",
        serde_json::to_string(&serde_json::json!({
            "corpus_fingerprint": fingerprint(CANDIDATE_SNAPSHOTS.as_bytes()),
            "cases": signal_rows
        }))
        .unwrap()
    );

    let first = replay_corpus(
        &corpus,
        eggsearch::selection_test::SelectionKnobs::production(),
    );
    let second = replay_corpus(
        &corpus,
        eggsearch::selection_test::SelectionKnobs::production(),
    );
    for (left, right) in first.cases.iter().zip(&second.cases) {
        assert_eq!(left.results, right.results, "replay order must be stable");
    }
    let (aggregate, _) = evaluate(&first, None);
    let (repeated, _) = evaluate(&second, None);
    assert_eq!(aggregate, repeated);

    let inversion = case_metrics(&first, "rank-inversion-001");
    assert_eq!(metric(&inversion, "recall_at_5"), 1.0);
    assert_eq!(metric(&inversion, "mrr_at_5"), 1.0);
    assert!(
        metric(&inversion, "ndcg_at_5") < 0.9,
        "the swapped authoritative document must cost graded ranking: {inversion}"
    );

    let duplicates = case_metrics(&first, "duplicate-cluster-001");
    assert_eq!(metric(&duplicates, "recall_at_5"), 1.0);
    assert_eq!(metric(&duplicates, "duplicate_slots"), 0.0);
    assert_eq!(metric(&duplicates, "distinct_domains_topk"), 4.0);

    let concentration = case_metrics(&first, "domain-concentration-001");
    assert_eq!(metric(&concentration, "recall_at_5"), 0.5);
    assert_eq!(metric(&concentration, "distinct_domains_topk"), 2.0);
    assert_eq!(metric(&concentration, "authoritative_survival"), 0.5);

    let agreement = case_metrics(&first, "provider-agreement-001");
    assert_eq!(metric(&agreement, "recall_at_5"), 1.0);
    assert_eq!(metric(&agreement, "mrr_at_5"), 1.0);
    assert_eq!(metric(&agreement, "distinct_domains_topk"), 5.0);

    let disagreement = case_metrics(&first, "provider-disagreement-001");
    assert!(
        metric(&disagreement, "recall_at_5") < 1.0,
        "provider-disagreement case must remain a consensus-bias stress: {disagreement}"
    );

    for id in ["authoritative-promotion-001", "code-intent-promotion-001"] {
        let promotion = case_metrics(&first, id);
        assert_eq!(
            metric(&promotion, "authoritative_survival"),
            1.0,
            "{id} must be promoted into the final window: {promotion}"
        );
        assert_eq!(metric(&promotion, "recall_at_5"), 1.0, "{id}");
    }

    for case in &first.cases {
        let metrics = case_metrics(&first, &case.id);
        assert!(
            metric(&metrics, "ndcg_at_5") <= 1.0,
            "nDCG above 1 means a document matched more than once: {} {metrics}",
            case.id
        );
    }

    assert_eq!(metric(&aggregate, "duplicate_slots"), 0.0);
    assert!(metric(&aggregate, "authoritative_survival") > 0.5);
    println!(
        "retrieval-quality-candidate-snapshot-report {}",
        serde_json::to_string(&snapshot_report(&first, aggregate)).unwrap()
    );
}

#[test]
fn recorded_snapshot_tier_is_explicit_about_its_provenance() {
    let corpus: Corpus = serde_json::from_str(RECORDED_SNAPSHOTS).expect("recorded corpus parses");
    assert_eq!(corpus.schema_version, 1);
    assert_eq!(corpus.corpus_revision, "candidate-snapshots-recorded-v1");
    assert_eq!(corpus.tier.as_deref(), Some("recorded_real"));
    assert!(corpus.provenance.contains("page bodies"));
    if corpus.cases.is_empty() {
        assert!(
            corpus
                .pending
                .as_deref()
                .is_some_and(|pending| pending.contains("recorder")),
            "an empty recorded tier must name the recorder that populates it"
        );
        println!(
            "retrieval-quality-recorded-snapshots state=empty reason={}",
            corpus.pending.unwrap_or_default()
        );
        return;
    }
    for case in &corpus.cases {
        let observed_at = case
            .observed_at
            .as_deref()
            .unwrap_or_else(|| panic!("recorded case {} must record a UTC date", case.id));
        assert!(
            observed_at.len() == 10
                && observed_at.as_bytes()[4] == b'-'
                && observed_at.as_bytes()[7] == b'-',
            "recorded case {} must record a YYYY-MM-DD UTC date, got {observed_at}",
            case.id
        );
        assert_eq!(case.freshness.as_deref(), Some("any"));
    }
    #[cfg(feature = "mock")]
    {
        for case in &corpus.cases {
            assert!(
                !case.provider_lists.is_empty(),
                "recorded case {} must record provider lists",
                case.id
            );
        }
        let replayed = replay_corpus(
            &corpus,
            eggsearch::selection_test::SelectionKnobs::production(),
        );
        let (aggregate, _) = evaluate(&replayed, None);
        assert_eq!(aggregate["cases"], corpus.cases.len());
        println!(
            "retrieval-quality-recorded-snapshot-report {}",
            serde_json::to_string(&snapshot_report(&replayed, aggregate)).unwrap()
        );
    }
}

#[cfg(feature = "mock")]
#[test]
fn seeded_selector_regressions_are_detected_on_candidate_snapshots() {
    use eggsearch::selection_test::SelectionKnobs;

    let corpus: Corpus = serde_json::from_str(CANDIDATE_SNAPSHOTS).expect("snapshot corpus parses");
    let production_knobs = SelectionKnobs::production();
    let production_corpus = replay_corpus(&corpus, production_knobs);
    let (production, _) = evaluate(&production_corpus, None);

    let degenerate_corpus = replay_corpus(
        &corpus,
        SelectionKnobs {
            rrf_k: f64::INFINITY,
            ..production_knobs
        },
    );
    let (degenerate, _) = evaluate(&degenerate_corpus, None);

    let dedup_off_corpus = replay_corpus(
        &corpus,
        SelectionKnobs {
            dedup: false,
            ..production_knobs
        },
    );
    let (dedup_off, _) = evaluate(&dedup_off_corpus, None);

    let truncated_corpus = replay_corpus(
        &corpus,
        SelectionKnobs {
            truncate_before_rerank: true,
            ..production_knobs
        },
    );
    let (truncated, _) = evaluate(&truncated_corpus, None);

    let production_survival = metric(&production, "authoritative_survival");
    let production_mrr = metric(&production, "mrr_at_5");
    let production_duplicates = metric(&production, "duplicate_slots");

    assert_eq!(production_duplicates, 0.0);
    assert!(
        metric(&dedup_off, "duplicate_slots") >= production_duplicates + 4.0,
        "dedup disabled must occupy duplicate slots: {dedup_off}"
    );
    assert_eq!(
        metric(
            &case_metrics(&dedup_off_corpus, "duplicate-cluster-001"),
            "duplicate_slots"
        ),
        2.0
    );

    assert!(
        metric(&degenerate, "authoritative_survival") < production_survival - 0.2,
        "degenerate RRF constant must collapse authoritative survival: prod={production_survival} degenerate={}",
        metric(&degenerate, "authoritative_survival")
    );
    assert!(
        metric(&degenerate, "mrr_at_5") < production_mrr - 0.2,
        "degenerate RRF constant must collapse rank ordering: prod={production_mrr} degenerate={}",
        metric(&degenerate, "mrr_at_5")
    );
    let production_concentration = case_metrics(&production_corpus, "domain-concentration-001");
    let degenerate_concentration = case_metrics(&degenerate_corpus, "domain-concentration-001");
    assert!(
        metric(&degenerate_concentration, "distinct_domains_topk")
            < metric(&production_concentration, "distinct_domains_topk"),
        "degenerate RRF constant must collapse domain diversity: prod={production_concentration} degenerate={degenerate_concentration}"
    );

    for id in ["authoritative-promotion-001", "code-intent-promotion-001"] {
        assert_eq!(
            metric(
                &case_metrics(&production_corpus, id),
                "authoritative_survival"
            ),
            1.0
        );
        assert_eq!(
            metric(
                &case_metrics(&truncated_corpus, id),
                "authoritative_survival"
            ),
            0.0,
            "{id} must lose its authoritative document when the pool is cut before reranking"
        );
    }
    assert!(
        metric(&truncated, "authoritative_survival") < production_survival - 0.25,
        "truncation before reranking must lower authoritative survival: prod={production_survival} truncated={}",
        metric(&truncated, "authoritative_survival")
    );

    println!(
        "retrieval-quality-negative-controls {}",
        serde_json::json!({
            "corpus_fingerprint": fingerprint(CANDIDATE_SNAPSHOTS.as_bytes()),
            "production": {
                "duplicate_slots": production_duplicates,
                "distinct_domains_topk": metric(&production, "distinct_domains_topk"),
                "authoritative_survival": production_survival,
                "mrr_at_5": production_mrr,
                "ndcg_at_5": metric(&production, "ndcg_at_5")
            },
            "case_probes": {
                "domain_concentration_distinct_domains": {
                    "production": metric(&production_concentration, "distinct_domains_topk"),
                    "degenerate_rrf_constant": metric(
                        &degenerate_concentration,
                        "distinct_domains_topk"
                    )
                },
                "duplicate_cluster_duplicate_slots": {
                    "production": metric(
                        &case_metrics(&production_corpus, "duplicate-cluster-001"),
                        "duplicate_slots"
                    ),
                    "dedup_disabled": metric(
                        &case_metrics(&dedup_off_corpus, "duplicate-cluster-001"),
                        "duplicate_slots"
                    )
                },
                "authoritative_promotion_survival": {
                    "production": metric(
                        &case_metrics(&production_corpus, "authoritative-promotion-001"),
                        "authoritative_survival"
                    ),
                    "truncate_before_rerank": metric(
                        &case_metrics(&truncated_corpus, "authoritative-promotion-001"),
                        "authoritative_survival"
                    )
                },
                "code_intent_promotion_survival": {
                    "production": metric(
                        &case_metrics(&production_corpus, "code-intent-promotion-001"),
                        "authoritative_survival"
                    ),
                    "truncate_before_rerank": metric(
                        &case_metrics(&truncated_corpus, "code-intent-promotion-001"),
                        "authoritative_survival"
                    )
                }
            },
            "degenerate_rrf_constant": {
                "rrf_k": "infinity",
                "duplicate_slots": metric(&degenerate, "duplicate_slots"),
                "distinct_domains_topk": metric(&degenerate, "distinct_domains_topk"),
                "authoritative_survival": metric(&degenerate, "authoritative_survival"),
                "mrr_at_5": metric(&degenerate, "mrr_at_5"),
                "ndcg_at_5": metric(&degenerate, "ndcg_at_5")
            },
            "dedup_disabled": {
                "duplicate_slots": metric(&dedup_off, "duplicate_slots"),
                "distinct_domains_topk": metric(&dedup_off, "distinct_domains_topk"),
                "authoritative_survival": metric(&dedup_off, "authoritative_survival"),
                "mrr_at_5": metric(&dedup_off, "mrr_at_5"),
                "ndcg_at_5": metric(&dedup_off, "ndcg_at_5")
            },
            "truncate_before_rerank": {
                "duplicate_slots": metric(&truncated, "duplicate_slots"),
                "distinct_domains_topk": metric(&truncated, "distinct_domains_topk"),
                "authoritative_survival": metric(&truncated, "authoritative_survival"),
                "mrr_at_5": metric(&truncated, "mrr_at_5"),
                "ndcg_at_5": metric(&truncated, "ndcg_at_5")
            }
        })
    );
}

#[cfg(feature = "mock")]
struct QueryRecordingEngine {
    results: Vec<eggsearch::meta::engines::models::SearchResult>,
    queries: Arc<std::sync::Mutex<Vec<String>>>,
}

#[cfg(feature = "mock")]
impl eggsearch::meta::engines::SearchEngine for QueryRecordingEngine {
    fn name(&self) -> &'static str {
        "mock_a"
    }

    fn search<'a>(
        &'a self,
        request: &'a eggsearch::meta::engines::EngineSearchRequest,
    ) -> eggsearch::meta::engines::BoxFuture<
        'a,
        Result<
            Vec<eggsearch::meta::engines::models::SearchResult>,
            eggsearch::meta::engines::error::EngineError,
        >,
    > {
        if let Ok(mut seen) = self.queries.lock() {
            seen.push(request.query.clone());
        }
        let results = self.results.clone();
        Box::pin(async move { Ok(results) })
    }
}

#[cfg(feature = "mock")]
async fn recorded_provider_queries(
    query: &str,
    caller_intent: Option<SearchIntent>,
) -> Vec<String> {
    use eggsearch::core::WebSearchRequest;
    use eggsearch::meta::MetadataSearchAdapter;

    let queries = Arc::new(std::sync::Mutex::new(Vec::new()));
    let engine = QueryRecordingEngine {
        results: vec![eggsearch::meta::engines::models::SearchResult {
            title: "Example".to_string(),
            url: "https://example.test/page".to_string(),
            snippet: None,
            source_engine: "mock_a".to_string(),
            metadata: Default::default(),
            excerpts: Vec::new(),
            published_at: None,
        }],
        queries: Arc::clone(&queries),
    };
    let adapter = MetadataSearchAdapter::from_engines(
        vec![Arc::new(engine) as Arc<dyn eggsearch::meta::engines::SearchEngine>],
        std::time::Duration::from_secs(2),
    );
    let mut req = WebSearchRequest::new(query);
    req.providers = vec!["mock_a".to_string()];
    adapter
        .web_search_with_intent(
            &req,
            5,
            50,
            caller_intent,
            eggsearch::core::config::IntentRoutingMode::Off,
        )
        .await;
    let seen = queries.lock().map(|seen| seen.clone()).unwrap_or_default();
    seen
}

#[cfg(feature = "mock")]
#[tokio::test]
async fn derived_intent_keeps_planner_output_and_provider_query_text() {
    use eggsearch::core::query::SearchIntent;
    use eggsearch::core::WebSearchRequest;
    use eggsearch::meta::build_search_plan;
    use eggsearch::selection_test::derive_intent;

    let snapshot: Corpus =
        serde_json::from_str(CANDIDATE_SNAPSHOTS).expect("snapshot corpus parses");
    let mut queries: Vec<String> = snapshot
        .cases
        .iter()
        .map(|case| case.query.clone())
        .collect();
    queries.extend(
        [
            "Router::layer middleware ordering",
            "rust E0277 trait bound",
            "CVE-2024-1234 openssl",
            "serde == 1.0.200 features",
            "tokio release notes",
            "axum websocket issue",
            "tokio documentation runtime",
            "latest database news",
        ]
        .into_iter()
        .map(str::to_string),
    );

    let mut derived_cases = 0usize;
    for query in &queries {
        let derived = derive_intent(query);
        if derived.intent != SearchIntent::Web {
            derived_cases += 1;
        }
        let mut req = WebSearchRequest::new(query.clone());
        req.providers = vec!["mock_a".to_string()];
        let plan = build_search_plan(&req, &[]);
        assert_eq!(
            plan.generic_query,
            query.trim(),
            "derived {:?} must not rewrite the planned query",
            derived.reasons
        );
        assert!(plan.provider_queries.is_empty());

        let implicit = recorded_provider_queries(query, None).await;
        let explicit = recorded_provider_queries(query, Some(SearchIntent::Web)).await;
        assert_eq!(
            implicit, explicit,
            "omitted intent must dispatch the same provider query text for {query:?}"
        );
        assert_eq!(implicit, vec![query.trim().to_string()]);
    }
    assert!(
        derived_cases >= 6,
        "the invariance corpus must include derived non-web intents, got {derived_cases}"
    );
}

#[cfg(feature = "mock")]
struct RolloutMetric {
    key: &'static str,
    higher_is_better: bool,
}

#[cfg(feature = "mock")]
const ROLLOUT_METRICS: [RolloutMetric; 6] = [
    RolloutMetric {
        key: "recall_at_5",
        higher_is_better: true,
    },
    RolloutMetric {
        key: "mrr_at_5",
        higher_is_better: true,
    },
    RolloutMetric {
        key: "ndcg_at_5",
        higher_is_better: true,
    },
    RolloutMetric {
        key: "authoritative_survival",
        higher_is_better: true,
    },
    RolloutMetric {
        key: "duplicate_slots",
        higher_is_better: false,
    },
    RolloutMetric {
        key: "distinct_domains_topk",
        higher_is_better: true,
    },
];

#[cfg(feature = "mock")]
#[test]
fn derived_intent_rollout_gate_matches_the_recorded_decision() {
    use eggsearch::core::query::SearchIntent;
    use eggsearch::selection_test::{derive_intent, reranking_uses_derived_intent, SelectionKnobs};

    let corpus: Corpus = serde_json::from_str(CANDIDATE_SNAPSHOTS).expect("snapshot corpus parses");
    let web_corpus = replay_corpus_with_intent(&corpus, SelectionKnobs::production(), |_case| {
        SearchIntent::Web
    });
    let derived_corpus = replay_corpus_with_intent(&corpus, SelectionKnobs::production(), |case| {
        derive_intent(&case.query).intent
    });
    let (web, _) = evaluate(&web_corpus, None);
    let (derived, _) = evaluate(&derived_corpus, None);

    let mut aggregate_improved = false;
    let mut aggregate_worsened: Vec<&'static str> = Vec::new();
    let mut aggregate_rows = Vec::new();
    for spec in &ROLLOUT_METRICS {
        let base = metric(&web, spec.key);
        let candidate = metric(&derived, spec.key);
        let delta = candidate - base;
        let improvement = if spec.higher_is_better { delta } else { -delta };
        if improvement > 1e-9 {
            aggregate_improved = true;
        }
        if improvement < -1e-9 {
            aggregate_worsened.push(spec.key);
        }
        aggregate_rows.push(serde_json::json!({
            "metric": spec.key,
            "higher_is_better": spec.higher_is_better,
            "intent_web": base,
            "derived": candidate,
            "delta": delta
        }));
    }

    let mut case_rows = Vec::new();
    let mut signal_improved: Vec<String> = Vec::new();
    let mut case_worsened: Vec<String> = Vec::new();
    for case in corpus
        .cases
        .iter()
        .filter(|case| !case.provider_lists.is_empty())
    {
        let derived_intent = derive_intent(&case.query);
        let web_case = case_metrics(&web_corpus, &case.id);
        let derived_case = case_metrics(&derived_corpus, &case.id);
        let signal = SIGNAL_CASES.iter().find(|signal| signal.case_id == case.id);
        let ranks = signal.map(|signal| {
            let web_rows = &web_corpus
                .cases
                .iter()
                .find(|replayed| replayed.id == case.id)
                .expect("replayed case")
                .results;
            let derived_rows = &derived_corpus
                .cases
                .iter()
                .find(|replayed| replayed.id == case.id)
                .expect("replayed case")
                .results;
            serde_json::json!({
                "intent_document_url": signal.intent_document_url,
                "intent_document_rank": {
                    "intent_web": row_rank(web_rows, signal.intent_document_url),
                    "derived": row_rank(derived_rows, signal.intent_document_url)
                },
                "competitor_url": signal.competitor_url,
                "competitor_rank": {
                    "intent_web": row_rank(web_rows, signal.competitor_url),
                    "derived": row_rank(derived_rows, signal.competitor_url)
                }
            })
        });
        for spec in &ROLLOUT_METRICS {
            let base = metric(&web_case, spec.key);
            let candidate = metric(&derived_case, spec.key);
            let delta = candidate - base;
            let improvement = if spec.higher_is_better { delta } else { -delta };
            if improvement > 1e-9 && signal.is_some() {
                signal_improved.push(format!("{}:{}", case.id, spec.key));
            }
            if improvement < -1e-9 {
                case_worsened.push(format!("{}:{}", case.id, spec.key));
            }
            case_rows.push(serde_json::json!({
                "case": case.id,
                "annotated_intent": case.intent,
                "derived_intent": derived_intent.intent.as_str(),
                "derived_reasons": derived_intent.reasons,
                "derived_confidence": derived_intent.confidence,
                "metric": spec.key,
                "intent_web": base,
                "derived": candidate,
                "delta": delta,
                "signal_case": signal.is_some(),
                "ranks": ranks,
            }));
        }
    }

    let should_apply = aggregate_improved
        && aggregate_worsened.is_empty()
        && case_worsened.is_empty()
        && !signal_improved.is_empty();
    assert_eq!(
        reranking_uses_derived_intent(),
        should_apply,
        "the applied/observe-only switch must follow the snapshot evidence: \
         aggregate_worsened={aggregate_worsened:?} case_worsened={case_worsened:?} \
         signal_improved={signal_improved:?} aggregate_improved={aggregate_improved} \
         aggregate={aggregate_rows:?}"
    );
    if should_apply {
        assert!(
            signal_improved.len() >= SIGNAL_CASES.len(),
            "every signal case must improve at least one ranking metric, improved={signal_improved:?}"
        );
    }

    println!(
        "retrieval-quality-derived-intent-rollout {}",
        serde_json::to_string(&serde_json::json!({
            "corpus_fingerprint": fingerprint(CANDIDATE_SNAPSHOTS.as_bytes()),
            "decision": if should_apply { "applied" } else { "observe_only" },
            "improved_any_metric": aggregate_improved,
            "worsened_metrics": aggregate_worsened,
            "case_worsened_metrics": case_worsened,
            "signal_improved_metrics": signal_improved,
            "aggregate": aggregate_rows,
            "per_case": case_rows
        }))
        .unwrap()
    );
}

#[cfg(feature = "mock")]
const SECURITY_PREFERENCE_ORDER: [&str; 4] = ["osv", "github_advisory", "rustsec", "nvd"];

#[cfg(feature = "mock")]
const SECURITY_DISTRACTOR_ORDER: [&str; 4] = ["nvd", "osv", "github_advisory", "rustsec"];

#[cfg(feature = "mock")]
const INTENT_ROUTING_RECORDED_DECISION: &str = "observe_only";

#[cfg(feature = "mock")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RoutingReplay {
    Baseline,
    Routed,
    Ordered(&'static [&'static str], usize),
}

#[cfg(feature = "mock")]
fn built_provider_ids(case: &Case) -> Vec<String> {
    case.provider_lists
        .iter()
        .map(|list| list.provider_id.clone())
        .collect()
}

#[cfg(feature = "mock")]
fn routing_base(routing: &CaseRouting) -> Vec<String> {
    routing
        .default_providers
        .iter()
        .filter(|id| {
            routing
                .enabled_providers
                .iter()
                .any(|enabled| enabled == *id)
        })
        .cloned()
        .collect()
}

#[cfg(feature = "mock")]
fn routed_provider_ids(
    case: &Case,
    routing: &CaseRouting,
    order: Option<&'static [&'static str]>,
    max_extra: usize,
) -> Vec<String> {
    let built = built_provider_ids(case);
    let base = routing_base(routing);
    let mut routed = base.clone();
    if routing.intent_mode != "apply" {
        return routed;
    }
    let intent = live_intent(&case.intent);
    let additions: Vec<String> = match order {
        Some(order) => eggsearch::selection_test::route_with_order(
            intent,
            order,
            &base,
            &routing.enabled_providers,
            &built,
            &[],
            max_extra,
        )
        .into_iter()
        .filter(|outcome| outcome.disposition == "added")
        .map(|outcome| outcome.provider_id)
        .collect(),
        None => eggsearch::selection_test::intent_additions(
            intent,
            &base,
            &routing.enabled_providers,
            &built,
            &[],
            max_extra,
        ),
    };
    for id in additions {
        if !routed.contains(&id) {
            routed.push(id);
        }
    }
    routed
}

#[cfg(feature = "mock")]
fn replay_corpus_with_routing(corpus: &Corpus, replay: RoutingReplay) -> Corpus {
    let mut replayed = corpus.clone();
    for case in &mut replayed.cases {
        if case.provider_lists.is_empty() {
            continue;
        }
        let only = match (&case.routing, replay) {
            (None, _) => None,
            (Some(routing), RoutingReplay::Baseline) => Some(routing_base(routing)),
            (Some(routing), RoutingReplay::Routed) => Some(routed_provider_ids(
                case,
                routing,
                None,
                routing.max_extra_providers,
            )),
            (Some(routing), RoutingReplay::Ordered(order, max_extra)) => {
                Some(routed_provider_ids(case, routing, Some(order), max_extra))
            }
        };
        case.results = replay_case_with_subset(
            case,
            eggsearch::selection_test::SelectionKnobs::production(),
            live_intent(&case.intent),
            only.as_deref(),
        );
    }
    replayed
}

#[cfg(feature = "mock")]
#[test]
fn candidate_snapshot_routing_cases_route_only_native_specialists() {
    let corpus: Corpus = serde_json::from_str(CANDIDATE_SNAPSHOTS).expect("snapshot corpus parses");
    let baseline = replay_corpus_with_routing(&corpus, RoutingReplay::Baseline);
    let routed = replay_corpus_with_routing(&corpus, RoutingReplay::Routed);
    let mut rows = Vec::new();
    for case in corpus.cases.iter().filter(|case| case.routing.is_some()) {
        let routing = case.routing.as_ref().expect("routing block");
        assert!(
            matches!(routing.intent_mode.as_str(), "off" | "apply"),
            "{} has unsupported intent_mode {}",
            case.id,
            routing.intent_mode
        );
        for id in &routing.default_providers {
            assert!(
                routing.enabled_providers.contains(id),
                "{} routes an unenabled default provider {id}",
                case.id
            );
        }
        let derived = eggsearch::selection_test::derive_intent(&case.query);
        assert_eq!(
            derived.intent.as_str(),
            case.intent,
            "routing case {} must derive its annotated intent; classifier said {:?}",
            case.id,
            derived
        );
        let built = built_provider_ids(case);
        let base = routing_base(routing);
        let routed_ids = routed_provider_ids(case, routing, None, routing.max_extra_providers);
        let additions: Vec<String> = routed_ids
            .iter()
            .filter(|id| !base.contains(id))
            .cloned()
            .collect();
        assert!(
            additions.len() <= routing.max_extra_providers,
            "{} exceeded max_extra_providers",
            case.id
        );
        let capable = eggsearch::selection_test::capable_providers(derived.intent, &built);
        let unrouted: Vec<String> = capable
            .iter()
            .filter(|id| !routed_ids.contains(id))
            .cloned()
            .collect();
        let base_metrics = case_metrics(&baseline, &case.id);
        let routed_metrics = case_metrics(&routed, &case.id);
        let mut deltas = Vec::new();
        for spec in &ROLLOUT_METRICS {
            let base_value = metric(&base_metrics, spec.key);
            let routed_value = metric(&routed_metrics, spec.key);
            let delta = routed_value - base_value;
            let regression = if spec.higher_is_better {
                delta < -1e-9
            } else {
                delta > 1e-9
            };
            assert!(
                !regression,
                "routing case {} regresses {}: baseline={base_value} routed={routed_value}",
                case.id, spec.key
            );
            deltas.push(serde_json::json!({
                "metric": spec.key,
                "baseline": base_value,
                "routed": routed_value,
                "delta": delta
            }));
        }
        rows.push(serde_json::json!({
            "case": case.id,
            "intent": case.intent,
            "derived_intent": derived.intent.as_str(),
            "intent_mode": routing.intent_mode,
            "max_extra_providers": routing.max_extra_providers,
            "base_providers": base,
            "routed_providers": routed_ids,
            "added_providers": additions,
            "capable_enabled_providers": capable,
            "unrouted_capable_providers": unrouted,
            "exclusion_warning_would_fire": !unrouted.is_empty(),
            "baseline": base_metrics,
            "routed": routed_metrics,
            "deltas": deltas
        }));
    }
    assert!(
        rows.len() >= 4,
        "the routing lane needs at least four cases, got {}",
        rows.len()
    );
    println!(
        "retrieval-quality-routing-report {}",
        serde_json::to_string(&serde_json::json!({
            "corpus_fingerprint": fingerprint(CANDIDATE_SNAPSHOTS.as_bytes()),
            "intent_mode": eggsearch::core::config::AppConfig::default().search.routing.intent_mode.as_str(),
            "cases": rows
        }))
        .unwrap()
    );
}

#[cfg(feature = "mock")]
#[test]
fn seeded_wrong_routing_order_is_detected_on_candidate_snapshots() {
    let corpus: Corpus = serde_json::from_str(CANDIDATE_SNAPSHOTS).expect("snapshot corpus parses");
    let correct = replay_corpus_with_routing(
        &corpus,
        RoutingReplay::Ordered(&SECURITY_PREFERENCE_ORDER, 1),
    );
    let distractor = replay_corpus_with_routing(
        &corpus,
        RoutingReplay::Ordered(&SECURITY_DISTRACTOR_ORDER, 1),
    );
    let repeated = replay_corpus_with_routing(
        &corpus,
        RoutingReplay::Ordered(&SECURITY_DISTRACTOR_ORDER, 1),
    );
    for (left, right) in distractor.cases.iter().zip(&repeated.cases) {
        assert_eq!(
            left.results, right.results,
            "distractor replay must be reproducible"
        );
    }
    let correct_metrics = case_metrics(&correct, "security-osv-routing-001");
    let distractor_metrics = case_metrics(&distractor, "security-osv-routing-001");
    let drop = metric(&correct_metrics, "recall_at_5") - metric(&distractor_metrics, "recall_at_5");
    assert!(
        drop >= 0.25,
        "the nvd-before-osv distractor must cost at least 0.25 recall_at_5 on the security routing case: correct={correct_metrics} distractor={distractor_metrics}"
    );
    println!(
        "retrieval-quality-routing-negative-control {}",
        serde_json::to_string(&serde_json::json!({
            "case": "security-osv-routing-001",
            "control": "security-nvd-first",
            "correct_order": SECURITY_PREFERENCE_ORDER,
            "distractor_order": SECURITY_DISTRACTOR_ORDER,
            "recall_drop": drop,
            "correct": correct_metrics,
            "distractor": distractor_metrics
        }))
        .unwrap()
    );
}

#[cfg(feature = "mock")]
#[test]
fn intent_routing_rollout_gate_matches_the_recorded_decision() {
    use eggsearch::core::config::{AppConfig, IntentRoutingMode};

    let corpus: Corpus = serde_json::from_str(CANDIDATE_SNAPSHOTS).expect("snapshot corpus parses");
    let plain = replay_corpus(
        &corpus,
        eggsearch::selection_test::SelectionKnobs::production(),
    );
    let baseline = replay_corpus_with_routing(&corpus, RoutingReplay::Baseline);
    let routed = replay_corpus_with_routing(&corpus, RoutingReplay::Routed);

    let mut pre_existing_changed: Vec<String> = Vec::new();
    for case in corpus.cases.iter().filter(|case| case.routing.is_none()) {
        let plain_case = plain
            .cases
            .iter()
            .find(|replayed| replayed.id == case.id)
            .expect("plain replay");
        let baseline_case = baseline
            .cases
            .iter()
            .find(|replayed| replayed.id == case.id)
            .expect("baseline replay");
        let routed_case = routed
            .cases
            .iter()
            .find(|replayed| replayed.id == case.id)
            .expect("routed replay");
        if plain_case.results != baseline_case.results || plain_case.results != routed_case.results
        {
            pre_existing_changed.push(case.id.clone());
        }
    }

    let routing_cases: Vec<&Case> = corpus
        .cases
        .iter()
        .filter(|case| case.routing.is_some())
        .collect();
    let mut improved: Vec<String> = Vec::new();
    let mut worsened: Vec<String> = Vec::new();
    for case in &routing_cases {
        let base_metrics = case_metrics(&baseline, &case.id);
        let routed_metrics = case_metrics(&routed, &case.id);
        for spec in &ROLLOUT_METRICS {
            let delta = metric(&routed_metrics, spec.key) - metric(&base_metrics, spec.key);
            let improvement = if spec.higher_is_better { delta } else { -delta };
            if improvement > 1e-9 {
                improved.push(format!("{}:{}", case.id, spec.key));
            }
            if improvement < -1e-9 {
                worsened.push(format!("{}:{}", case.id, spec.key));
            }
        }
    }

    let security_control = replay_corpus_with_routing(
        &corpus,
        RoutingReplay::Ordered(&SECURITY_PREFERENCE_ORDER, 1),
    );
    let security_distractor = replay_corpus_with_routing(
        &corpus,
        RoutingReplay::Ordered(&SECURITY_DISTRACTOR_ORDER, 1),
    );
    let negative_control_recall_drop = metric(
        &case_metrics(&security_control, "security-osv-routing-001"),
        "recall_at_5",
    ) - metric(
        &case_metrics(&security_distractor, "security-osv-routing-001"),
        "recall_at_5",
    );
    let negative_control_detected = negative_control_recall_drop >= 0.25;

    let shipped_defaults = AppConfig::default().search.default_providers;
    let lane_measures_shipped_default_path = !routing_cases.is_empty()
        && routing_cases.iter().all(|case| {
            case.routing
                .as_ref()
                .is_some_and(|routing| routing.default_providers == shipped_defaults)
        });

    let should_apply = pre_existing_changed.is_empty()
        && worsened.is_empty()
        && !improved.is_empty()
        && negative_control_detected
        && lane_measures_shipped_default_path;
    let decision = if should_apply {
        "applied"
    } else {
        "observe_only"
    };
    assert_eq!(
        decision, INTENT_ROUTING_RECORDED_DECISION,
        "the recorded routing decision must follow the snapshot evidence: \
         pre_existing_changed={pre_existing_changed:?} worsened={worsened:?} \
         improved={improved:?} negative_control_recall_drop={negative_control_recall_drop} \
         lane_measures_shipped_default_path={lane_measures_shipped_default_path}"
    );
    assert_eq!(
        AppConfig::default().search.routing.intent_mode == IntentRoutingMode::Apply,
        should_apply,
        "the shipped [search.routing].intent_mode must equal the recorded decision: \
         pre_existing_changed={pre_existing_changed:?} worsened={worsened:?} \
         improved={improved:?} negative_control_recall_drop={negative_control_recall_drop} \
         lane_measures_shipped_default_path={lane_measures_shipped_default_path}"
    );
    println!(
        "retrieval-quality-routing-rollout {}",
        serde_json::to_string(&serde_json::json!({
            "corpus_fingerprint": fingerprint(CANDIDATE_SNAPSHOTS.as_bytes()),
            "decision": decision,
            "intent_mode": AppConfig::default().search.routing.intent_mode.as_str(),
            "pre_existing_cases_changed": pre_existing_changed,
            "worsened_metrics": worsened,
            "improved_metrics": improved,
            "negative_control": {
                "case": "security-osv-routing-001",
                "distractor": "security-nvd-first",
                "recall_drop": negative_control_recall_drop,
                "detected": negative_control_detected
            },
            "lane_measures_shipped_default_path": lane_measures_shipped_default_path,
            "shipped_default_providers": shipped_defaults
        }))
        .unwrap()
    );
}

#[tokio::test]
#[ignore = "opt-in live observation; configure providers and an explicit task allowlist"]
async fn retrieval_quality_live_observation() {
    use std::sync::Arc;
    use std::time::Instant;

    use eggsearch::core::config::AppConfig;
    use eggsearch::mcp::state::ServerState;
    use eggsearch::mcp::tools::{run_web_search, WebSearchArgs};

    let enabled = std::env::var("EGGSEARCH_RETRIEVAL_LIVE").unwrap_or_default();
    assert_eq!(
        enabled, "1",
        "set EGGSEARCH_RETRIEVAL_LIVE=1 to authorize live queries"
    );
    let providers = std::env::var("EGGSEARCH_RETRIEVAL_PROVIDERS")
        .expect("set an explicit comma-separated provider allowlist");
    let provider_ids: Vec<String> = providers
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect();
    assert!(!provider_ids.is_empty());
    assert_eq!(
        provider_ids.iter().collect::<HashSet<_>>().len(),
        provider_ids.len(),
        "provider IDs must not be duplicated"
    );
    assert!(
        provider_ids.len() <= 8,
        "live provider ceiling is 8 providers"
    );
    assert!(
        provider_ids
            .iter()
            .all(|id| eggsearch::core::provider::KNOWN_PROVIDER_IDS.contains(&id.as_str())),
        "provider IDs must be registered providers"
    );
    let paid = std::env::var("EGGSEARCH_RETRIEVAL_PAID_PROVIDERS")
        .expect("set paid provider IDs explicitly, or set this to an empty string");
    let paid_ids: HashSet<String> = paid
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect();
    assert!(
        paid_ids.iter().all(|id| provider_ids.contains(id)),
        "paid provider IDs must be included in the provider allowlist"
    );
    if !paid_ids.is_empty() {
        assert_eq!(
            std::env::var("EGGSEARCH_RETRIEVAL_ALLOW_BILLING").as_deref(),
            Ok("1"),
            "set EGGSEARCH_RETRIEVAL_ALLOW_BILLING=1 to explicitly authorize billable references"
        );
    }
    let source_sha = std::env::var("EGGSEARCH_BENCH_SOURCE_SHA").expect("set the exact source SHA");
    assert!(
        source_sha.len() == 40 && source_sha.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "source SHA must be a 40-character hexadecimal commit ID"
    );
    let locale =
        std::env::var("EGGSEARCH_RETRIEVAL_LOCALE").expect("set the locale for this live cohort");
    let case_ids =
        std::env::var("EGGSEARCH_RETRIEVAL_CASES").expect("set explicit comma-separated case IDs");
    let allowed: HashSet<String> = case_ids
        .split(',')
        .map(str::trim)
        .map(str::to_owned)
        .collect();
    assert!(!allowed.is_empty(), "select at least one task case");
    let corpus: Corpus = serde_json::from_str(CORPUS).expect("corpus parses");
    let cases: Vec<&Case> = corpus
        .cases
        .iter()
        .filter(|case| allowed.contains(&case.id))
        .collect();
    assert_eq!(
        cases.len(),
        allowed.len(),
        "all selected case IDs must exist"
    );
    for case in &cases {
        assert!(
            aliases_are_unambiguous(case),
            "gold URL aliases must identify one judgment"
        );
        assert!(
            case.provider_restrictions.is_empty()
                || provider_ids
                    .iter()
                    .all(|provider| case.provider_restrictions.contains(provider)),
            "selected providers violate a task's provider restriction"
        );
    }
    assert!(cases.len() <= 20, "live task ceiling is 20 cases per run");
    let mut config = AppConfig::default();
    config.search.providers = provider_ids.iter().map(|id| (id.clone(), true)).collect();
    let state = Arc::new(ServerState::build(config).expect("configured state builds"));
    let mut records = Vec::new();
    let mut scored = Metrics::default();
    let mut live_categories = BTreeMap::<String, Metrics>::new();
    let mut live_partitions = BTreeMap::<String, Metrics>::new();
    let mut latencies = Vec::new();
    let mut response_bytes = 0usize;
    for case in cases {
        let started = Instant::now();
        let response = run_web_search(
            state.clone(),
            WebSearchArgs {
                query: case.query.clone(),
                max_results: Some(case.max_results.min(K)),
                providers: provider_ids.clone(),
                safe_search: None,
                timeout_ms: Some(10_000),
                intent: Some(live_intent(&case.intent)),
                freshness: None,
                date_range: case.temporal_window.as_ref().map(|window| {
                    eggsearch::core::query::SearchDateRange::new(
                        window.start.clone(),
                        window.end.clone(),
                    )
                }),
                include_domains: Vec::new(),
                exclude_domains: Vec::new(),
                language: Some(case.language.clone()),
                region: Some(case.region.clone()),
                excerpt_count: Some(1),
                response_detail: None,
            },
        )
        .await;
        let elapsed_ms = started.elapsed().as_millis();
        latencies.push(elapsed_ms);
        let tool_error_code = response.as_ref().err().map(|error| error.code().as_str());
        let failed = tool_error_code.is_some();
        let payload =
            response.unwrap_or_else(|error| serde_json::json!({"error": error.to_string()}));
        let bytes = serde_json::to_vec(&payload).expect("response serializes");
        response_bytes = response_bytes.saturating_add(bytes.len());
        let mut observed = case.clone();
        observed.actual_state = Some(if failed {
            "failure".to_owned()
        } else if payload["results"].as_array().is_some_and(Vec::is_empty) {
            "absence".to_owned()
        } else {
            "results".to_owned()
        });
        observed.results = payload
            .get("results")
            .and_then(serde_json::Value::as_array)
            .map(|items| items.iter().filter_map(|item| serde_json::from_value::<ResultEnvelope>(serde_json::json!({
                "id": item.get("stable_id").or_else(|| item.get("id")).and_then(serde_json::Value::as_str).unwrap_or_default(),
                "url": item.get("url").and_then(serde_json::Value::as_str).unwrap_or_default(),
                "title": item.get("title").and_then(serde_json::Value::as_str).unwrap_or_default(),
                "snippet": item.get("snippet").and_then(serde_json::Value::as_str).unwrap_or_default(),
                "published_at": item.get("metadata").and_then(|metadata| metadata.get("published_at")).and_then(serde_json::Value::as_str),
                "passages": item.get("excerpts").and_then(serde_json::Value::as_array).map(|xs| xs.iter().filter_map(|x| x.get("text").and_then(serde_json::Value::as_str).map(str::to_owned)).collect::<Vec<_>>()).unwrap_or_default()
            })).ok()).collect())
            .unwrap_or_default();
        scored.add(&observed);
        live_categories
            .entry(case.category.clone())
            .or_default()
            .add(&observed);
        live_partitions
            .entry(case.partition.clone())
            .or_default()
            .add(&observed);
        let attempt_count = payload["retrieval_summary"]["attempted_job_count"].as_u64();
        let provider_failure_count = payload["providers_failed"].as_array().map_or(0, Vec::len);
        let result_items = payload["results"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or_default();
        let result_count = result_items.len();
        let source_hashes = result_items
            .iter()
            .filter_map(|item| item["url"].as_str())
            .map(canonical_url)
            .map(|url| fingerprint(url.as_bytes()))
            .collect::<Vec<_>>();
        let source_contributions = result_items
            .iter()
            .filter_map(|item| {
                let url = item["url"].as_str()?;
                Some(serde_json::json!({
                    "source_hash": fingerprint(canonical_url(url).as_bytes()),
                    "providers": item["providers"]
                }))
            })
            .collect::<Vec<_>>();
        let unique_sources = result_items
            .iter()
            .filter_map(|item| item["url"].as_str())
            .map(canonical_url)
            .collect::<HashSet<_>>()
            .len();
        let useful_evidence = observed.results.iter().any(|result| {
            observed
                .relevant_documents
                .iter()
                .any(|gold| matches_case_document(case, gold, result))
        });
        let matched_relevant_documents = observed
            .results
            .iter()
            .flat_map(|result| {
                observed
                    .relevant_documents
                    .iter()
                    .filter(move |gold| matches_case_document(case, gold, result))
                    .map(|gold| gold.id.clone())
            })
            .collect::<BTreeSet<_>>();
        let provider_failures = payload["providers_failed"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|failure| {
                serde_json::json!({
                    "id": failure["id"],
                    "error_class": failure["error_class"]
                })
            })
            .collect::<Vec<_>>();
        records.push(serde_json::json!({
            "case_id": case.id,
            "providers": provider_ids,
            "providers_queried": payload["providers_queried"],
            "tool_error_code": tool_error_code,
            "elapsed_ms": elapsed_ms,
            "attempt_count": attempt_count,
            "retry_count": null,
            "provider_failure_count": provider_failure_count,
            "provider_failures": provider_failures,
            "timeout_count": payload["retrieval_summary"]["timed_out_count"],
            "deadline_interrupted_count": payload["retrieval_summary"]["deadline_interrupted_count"],
            "result_count": result_count,
            "unique_source_count": unique_sources,
            "canonical_source_hashes": source_hashes,
            "source_contributions": source_contributions,
            "matched_relevant_document_ids": matched_relevant_documents,
            "first_useful_evidence_latency_ms": if useful_evidence {Some(elapsed_ms)} else {None::<u128>},
            "response_bytes": bytes.len(),
            "estimated_tokens_from_response_bytes": bytes.len().div_ceil(4),
            "response_hash": fingerprint(&bytes),
            "outcome": if payload.get("error").is_some() {"failure"} else {"observed"}
        }));
    }
    latencies.sort_unstable();
    let percentile =
        |p: f64| -> u128 { latencies[((latencies.len() - 1) as f64 * p).ceil() as usize] };
    let features = [
        ("pdf", cfg!(feature = "pdf")),
        ("browser", cfg!(feature = "browser")),
        ("egress", cfg!(feature = "egress")),
        ("mock", cfg!(feature = "mock")),
    ]
    .into_iter()
    .filter_map(|(name, enabled)| enabled.then_some(name))
    .collect::<Vec<_>>();
    let report = serde_json::json!({
        "schema_version": 1,
        "source_sha": source_sha,
        "observed_at_unix_seconds": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
        "corpus_revision": corpus.corpus_revision,
        "corpus_fingerprint": fingerprint(CORPUS.as_bytes()),
        "eggsearch_version": env!("CARGO_PKG_VERSION"),
        "features": features,
        "configuration_fingerprint": fingerprint(format!("{}|{locale}", provider_ids.join(",")).as_bytes()),
        "locale": locale,
        "machine": {"os": std::env::consts::OS, "arch": std::env::consts::ARCH},
        "providers": provider_ids,
        "paid_reference_providers": paid_ids,
        "cache_state": "not_exposed_by_current_search_contract",
        "tie_break_seed": null,
        "trial_count": records.len(),
        "request_ceiling": 160,
        "request_count_upper_bound": records.len() * provider_ids.len(),
        "billable_request_upper_bound": records.len() * paid_ids.len(),
        "response_bytes": response_bytes,
        "estimated_response_tokens": response_bytes.div_ceil(4),
        "latency_ms": {"p50": percentile(0.50), "p95": percentile(0.95)},
        "aggregate": scored.report(),
        "per_category": live_categories.iter().map(|(name, metrics)| (name.clone(), metrics.report())).collect::<BTreeMap<_, _>>(),
        "per_partition": live_partitions.iter().map(|(name, metrics)| (name.clone(), metrics.report())).collect::<BTreeMap<_, _>>(),
        "records": records
    });
    println!(
        "retrieval-quality-live-report {}",
        serde_json::to_string(&report).unwrap()
    );
}

fn snapshot_output_path() -> std::path::PathBuf {
    use std::path::{Component, PathBuf};

    let raw = std::env::var("EGGSEARCH_RETRIEVAL_SNAPSHOT_OUT")
        .expect("set EGGSEARCH_RETRIEVAL_SNAPSHOT_OUT to a .json path under target/snapshots/");
    let path = PathBuf::from(&raw);
    assert!(
        !path.is_absolute(),
        "snapshot output must be relative to the repository, not absolute: {raw}"
    );
    assert!(
        path.starts_with(SNAPSHOT_OUTPUT_ROOT),
        "snapshot output must live under {SNAPSHOT_OUTPUT_ROOT}/: {raw}"
    );
    assert!(
        path.extension()
            .is_some_and(|extension| extension == "json"),
        "snapshot output must be a .json file: {raw}"
    );
    assert!(
        path.components()
            .all(|component| matches!(component, Component::Normal(_))),
        "snapshot output must not contain . or .. components: {raw}"
    );
    path
}

#[tokio::test]
#[ignore = "opt-in live recorder; writes an untracked candidate snapshot under target/snapshots/"]
async fn retrieval_quality_candidate_snapshot_recorder() {
    use std::sync::Arc;

    use eggsearch::core::config::AppConfig;
    use eggsearch::core::query::{Freshness, SearchIntent};
    use eggsearch::mcp::state::ServerState;
    use eggsearch::mcp::tools::{run_web_search, WebSearchArgs};

    let output = snapshot_output_path();
    assert_eq!(
        std::env::var("EGGSEARCH_RETRIEVAL_LIVE").unwrap_or_default(),
        "1",
        "set EGGSEARCH_RETRIEVAL_LIVE=1 to authorize live queries"
    );
    let providers = std::env::var("EGGSEARCH_RETRIEVAL_PROVIDERS")
        .expect("set an explicit comma-separated provider allowlist");
    let provider_ids: Vec<String> = providers
        .split(',')
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(str::to_owned)
        .collect();
    assert!(!provider_ids.is_empty());
    assert_eq!(
        provider_ids.iter().collect::<HashSet<_>>().len(),
        provider_ids.len(),
        "provider IDs must not be duplicated"
    );
    assert!(
        provider_ids.len() <= 8,
        "live provider ceiling is 8 providers"
    );
    assert!(
        provider_ids
            .iter()
            .all(|id| eggsearch::core::provider::KNOWN_PROVIDER_IDS.contains(&id.as_str())),
        "provider IDs must be registered providers"
    );
    let paid = std::env::var("EGGSEARCH_RETRIEVAL_PAID_PROVIDERS")
        .expect("set paid provider IDs explicitly, or set this to an empty string");
    let paid_ids: HashSet<String> = paid
        .split(',')
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(str::to_owned)
        .collect();
    assert!(
        paid_ids.iter().all(|id| provider_ids.contains(id)),
        "paid provider IDs must be included in the provider allowlist"
    );
    if !paid_ids.is_empty() {
        assert_eq!(
            std::env::var("EGGSEARCH_RETRIEVAL_ALLOW_BILLING").as_deref(),
            Ok("1"),
            "set EGGSEARCH_RETRIEVAL_ALLOW_BILLING=1 to explicitly authorize billable references"
        );
    }
    let source_sha = std::env::var("EGGSEARCH_BENCH_SOURCE_SHA").expect("set the exact source SHA");
    assert!(
        source_sha.len() == 40 && source_sha.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "source SHA must be a 40-character hexadecimal commit ID"
    );
    let locale =
        std::env::var("EGGSEARCH_RETRIEVAL_LOCALE").expect("set the locale for this live cohort");

    let corpus: Corpus = serde_json::from_str(CANDIDATE_SNAPSHOTS).expect("snapshot corpus parses");
    let requested = std::env::var("EGGSEARCH_RETRIEVAL_CASES").expect("set case IDs to record");
    let allowed: HashSet<String> = requested
        .split(',')
        .map(str::trim)
        .map(str::to_owned)
        .collect();
    let cases: Vec<&Case> = corpus
        .cases
        .iter()
        .filter(|case| allowed.contains(&case.id))
        .collect();
    assert_eq!(
        cases.len(),
        allowed.len(),
        "all recorded case IDs must exist"
    );
    assert!(!cases.is_empty(), "select at least one case to record");

    let mut config = AppConfig::default();
    config.search.providers = provider_ids.iter().map(|id| (id.clone(), true)).collect();
    config.search.sanitize_output = false;
    let state = Arc::new(ServerState::build(config).expect("configured state builds"));

    let observed_at = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let mut recorded = Vec::new();
    let mut failures: Vec<serde_json::Value> = Vec::new();
    let mut candidate_results = 0usize;
    let mut provider_lists = 0usize;
    for case in &cases {
        let mut lists = Vec::new();
        for provider in &provider_ids {
            let response = run_web_search(
                state.clone(),
                WebSearchArgs {
                    query: case.query.clone(),
                    max_results: Some(RECORDER_MAX_RESULTS_PER_PROVIDER),
                    providers: vec![provider.clone()],
                    safe_search: None,
                    timeout_ms: Some(10_000),
                    intent: Some(SearchIntent::Web),
                    freshness: Some(Freshness::Any),
                    date_range: None,
                    include_domains: Vec::new(),
                    exclude_domains: Vec::new(),
                    language: Some(case.language.clone()),
                    region: Some(case.region.clone()),
                    excerpt_count: None,
                    response_detail: None,
                },
            )
            .await;
            let response = match response {
                Ok(response) => response,
                Err(error) => {
                    failures.push(serde_json::json!({
                        "case_id": case.id,
                        "provider_id": provider,
                        "error": error.to_string()
                    }));
                    continue;
                }
            };
            let ranked: Vec<serde_json::Value> = response
                .get("results")
                .and_then(serde_json::Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .enumerate()
                        .filter_map(|(index, item)| {
                            let url = item.get("url").and_then(serde_json::Value::as_str)?;
                            let normalized = eggsearch::meta::engines::normalizer::normalize(url)
                                .unwrap_or_else(|| url.to_owned());
                            let title = item
                                .get("title")
                                .and_then(serde_json::Value::as_str)
                                .unwrap_or_default();
                            Some(serde_json::json!({
                                "rank": index + 1,
                                "title": title,
                                "url": normalized
                            }))
                        })
                        .collect()
                })
                .unwrap_or_default();
            if ranked.is_empty() {
                continue;
            }
            candidate_results += ranked.len();
            provider_lists += 1;
            lists.push(serde_json::json!({
                "provider_id": provider,
                "results": ranked
            }));
        }
        recorded.push(serde_json::json!({
            "id": case.id,
            "category": case.category,
            "partition": "recorded",
            "query": case.query,
            "language": case.language,
            "region": case.region,
            "intent": case.intent,
            "freshness": "any",
            "retrieval_scope": case.retrieval_scope,
            "max_results": case.max_results,
            "judgments_complete": false,
            "expected_state": case.expected_state,
            "actual_state": if lists.is_empty() { Some("absence") } else { None },
            "observed_at": observed_at,
            "relevant_documents": [],
            "provider_lists": lists
        }));
    }
    assert!(
        candidate_results > 0,
        "live dispatch produced no candidate rows for any provider; refusing to write an empty recorded snapshot: {}",
        serde_json::to_string(&failures).unwrap_or_default()
    );

    let document = serde_json::json!({
        "schema_version": 1,
        "corpus_revision": "candidate-snapshots-recorded-v1",
        "tier": "recorded_real",
        "provenance": "Recorded-real candidate snapshots from an opt-in live run. Only provider id, query, UTC observation date, rank, title, and normalized URL are stored; page bodies are never written.",
        "annotation_uncertainty": "Recorded provider order is not relevance truth. Cases stay unjudged until a hand-written gold set is added without reading the recorded order.",
        "source_sha": source_sha,
        "observed_at": observed_at,
        "locale": locale,
        "eggsearch_version": env!("CARGO_PKG_VERSION"),
        "providers": provider_ids,
        "paid_reference_providers": paid_ids,
        "recorded_case_count": recorded.len(),
        "provider_list_count": provider_lists,
        "candidate_row_count": candidate_results,
        "provider_failures": failures,
        "cases": recorded
    });
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent).expect("snapshot output directory is creatable");
    }
    std::fs::write(
        &output,
        serde_json::to_vec_pretty(&document).expect("snapshot serializes"),
    )
    .expect("snapshot output is writable");
    println!(
        "retrieval-quality-candidate-snapshot-recorder {}",
        serde_json::json!({
            "output": output.display().to_string(),
            "cases": recorded.len(),
            "provider_lists": provider_lists,
            "candidate_rows": candidate_results
        })
    );
}
