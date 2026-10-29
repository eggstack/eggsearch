use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use serde::Deserialize;

const CORPUS: &str = include_str!("fixtures/retrieval_quality/cases.json");
const K: usize = 5;
const PASSAGE_BUDGET_CHARS: usize = 1200;
const EVALUATOR_REVISION: &str = "retrieval-quality-evaluator-v1";
const PROVIDER_SCENARIO: &str =
    include_str!("fixtures/retrieval_quality/provider_contribution.json");

#[derive(Debug, Deserialize)]
struct Corpus {
    schema_version: u32,
    corpus_revision: String,
    provenance: String,
    annotation_uncertainty: String,
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
    relevant_documents: Vec<Judgment>,
    results: Vec<ResultEnvelope>,
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

#[derive(Clone, Debug, Deserialize, serde::Serialize)]
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
            self.diversity_cases += 1;
        }
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
                "passages": self.passage_denominator
            },
            "precision_at_5": self.precision_sum / ranking_cases,
            "recall_at_5": self.recall_sum / ranking_cases,
            "mrr_at_5": self.reciprocal_rank_sum / ranking_cases,
            "ndcg_at_5": self.ndcg_sum / ranking_cases,
            "passage_hit_at_1200_chars": ratio(self.passage_hits, self.passage_denominator),
            "passage_hit_wilson_95": passage_interval,
            "source_diversity_at_5": self.diversity_sum / diversity_cases,
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
        let failed = response.is_err();
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
        records.push(serde_json::json!({
            "case_id": case.id,
            "providers": provider_ids,
            "elapsed_ms": elapsed_ms,
            "attempt_count": attempt_count,
            "retry_count": null,
            "provider_failure_count": provider_failure_count,
            "timeout_count": payload["retrieval_summary"]["timed_out_count"],
            "deadline_interrupted_count": payload["retrieval_summary"]["deadline_interrupted_count"],
            "result_count": result_count,
            "unique_source_count": unique_sources,
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
