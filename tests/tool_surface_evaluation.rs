//! Deterministic tool-surface evaluation baseline.
//!
//! Layer 1 (deterministic retrieval/discovery) and Layer 2 (synthetic
//! tool-choice mechanics) from the agentic tool-surface evaluation plan.
//! Runs offline in routine CI with no model or network access. Layer 3
//! (live multi-model comparison) is an opt-in ignored suite in
//! `tests/tool_surface_live.rs`.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use eggsearch::core::config::AppConfig;
use eggsearch::core::workflow::MAX_NEXT_ACTIONS;
use eggsearch::mcp::state::ServerState;
use eggsearch::meta::recipe_catalog::{
    repo_search_next_actions, research_search_next_actions, security_search_next_actions,
    web_search_next_actions,
};
use rmcp::ServerHandler;
use serde::Deserialize;

const CASES_JSON: &str = include_str!("fixtures/tool_surface/cases.json");

const KNOWN_TOOLS: &[&str] = &[
    "web_search",
    "web_fetch",
    "batch_fetch",
    "provider_status",
    "repo_search",
    "repo_fetch",
    "repo_map",
    "security_search",
    "research_search",
    "build_evidence_bundle",
];

const MAX_DESCRIPTION_CHARS: usize = 1000;
const MAX_TOTAL_DEFINITION_BYTES: usize = 86000;
const MAX_INSTRUCTIONS_BYTES: usize = 6000;
const MAX_COMPACT_DISCOVERY_BYTES: usize = 512;
const MIN_TOP1_ACCURACY: f64 = 0.90;
const MIN_RECALL_AT_3: f64 = 0.95;
const MIN_MRR: f64 = 0.90;

#[derive(Debug, Deserialize)]
struct Case {
    id: String,
    category: String,
    query: String,
    expected_primary: String,
    acceptable: Vec<String>,
    expected_followups: Vec<String>,
    forbidden_primary: Vec<String>,
    why: String,
}

fn load_cases() -> Vec<Case> {
    serde_json::from_str(CASES_JSON).expect("tool-surface cases.json must parse")
}

fn compact_purpose(tool: &str) -> &'static str {
    match tool {
        "web_search" => "Discover candidate public web sources as source cards",
        "web_fetch" => "Fetch one explicit HTTP(S) URL as bounded text",
        "batch_fetch" => "Fetch several explicit URLs or repo files in one call",
        "provider_status" => "Diagnose provider configuration, health, and recipes",
        "repo_search" => "Discover grouped repository evidence for a codebase",
        "repo_fetch" => "Fetch a known repo file span or symbol by locator",
        "repo_map" => "Show repository structure without file contents",
        "security_search" => "Look up advisories and assess package applicability",
        "research_search" => "Gather multi-source evidence for complex questions",
        "build_evidence_bundle" => "Package already-selected evidence for handoff",
        _ => "unknown tool",
    }
}

const KEYWORDS: &[(&str, &str, i32)] = &[
    ("web_search", "latest release", 6),
    ("web_search", "release version", 6),
    ("web_search", "official docs", 5),
    ("web_search", "documentation", 4),
    ("web_search", "latest news", 8),
    ("web_search", "news this week", 8),
    ("web_search", "news", 3),
    ("web_search", "restricted to", 5),
    ("web_search", "how do i", 4),
    ("web_search", "how to", 3),
    ("web_search", "tutorial", 4),
    ("web_search", "secure rust code", 6),
    ("web_search", "sql injection", 5),
    ("web_search", "avoid path traversal", 6),
    ("web_search", "file handling code", 4),
    ("web_search", "secure-coding", 4),
    ("web_search", "secure code", 4),
    ("web_search", "writing rust", 3),
    ("web_search", "latest", 2),
    ("web_search", "docs", 2),
    ("web_search", "guide", 2),
    ("repo_search", "where is", 3),
    ("repo_search", "defined in", 6),
    ("repo_search", "definition", 4),
    ("repo_search", "symbol", 5),
    ("repo_search", "repository architecture", 8),
    ("repo_search", "architecture of", 3),
    ("repo_search", "implementation and tests", 7),
    ("repo_search", "implementation", 3),
    ("repo_search", "tests for", 3),
    ("repo_search", "issue", 3),
    ("repo_search", "release changes", 5),
    ("repo_search", "release notes", 5),
    ("repo_search", "migration notes", 6),
    ("repo_search", "changelog", 6),
    ("repo_search", "migrate", 4),
    ("repo_search", "migration", 4),
    ("repo_search", "what changed", 6),
    ("repo_search", "error[", 10),
    ("repo_search", "error ts", 8),
    ("repo_search", "error e", 8),
    ("repo_search", "traceback", 10),
    ("repo_search", "valueerror", 8),
    ("repo_search", "mismatched", 6),
    ("repo_search", "not assignable", 6),
    ("repo_search", "cargo build", 7),
    ("repo_search", "exact message", 6),
    ("repo_search", "failed to resolve", 5),
    ("repo_search", "compiler error", 6),
    ("repo_search", "exception", 5),
    ("repo_search", "toolchain", 5),
    ("repo_search", "repository", 4),
    ("repo_search", "codebase", 5),
    ("repo_search", "middleware", 3),
    ("repo_search", "compare versions", 8),
    ("repo_search", "versions", 4),
    ("repo_search", "performance regression", 5),
    ("repo_search", "release", 2),
    ("repo_map", "repository structure", 10),
    ("repo_map", "structure and layout", 10),
    ("repo_map", "layout of", 6),
    ("repo_map", "important files", 8),
    ("repo_map", "directories", 4),
    ("repo_map", "structure", 4),
    ("web_fetch", "fetch", 4),
    ("repo_fetch", "fetch", 3),
    ("repo_fetch", "fetch file", 10),
    ("repo_fetch", "fetch symbol", 10),
    ("repo_fetch", "main branch", 5),
    ("repo_fetch", "line range", 6),
    ("repo_fetch", "lines", 3),
    ("repo_fetch", "symbol", 5),
    ("batch_fetch", "these three urls", 12),
    ("batch_fetch", "three urls", 10),
    ("batch_fetch", "multiple", 6),
    ("batch_fetch", "several", 6),
    ("batch_fetch", "fetch these", 8),
    ("batch_fetch", "sources you found", 10),
    ("batch_fetch", "fetch the sources", 10),
    ("batch_fetch", "fetch", 3),
    ("provider_status", "provider", 6),
    ("provider_status", "unavailable", 6),
    ("provider_status", "outage", 10),
    ("provider_status", "troubleshooting", 6),
    ("provider_status", "configuration check", 6),
    ("provider_status", "missing api key", 8),
    ("provider_status", "health status", 8),
    ("provider_status", "api key", 5),
    ("provider_status", "health", 6),
    ("provider_status", "diagnos", 5),
    ("provider_status", "troubleshoot", 6),
    ("provider_status", "status", 3),
    ("provider_status", "configuration", 3),
    ("security_search", "cve-", 12),
    ("security_search", "cve", 8),
    ("security_search", "cve details", 8),
    ("security_search", "ghsa-", 12),
    ("security_search", "ghsa", 8),
    ("security_search", "advisory", 6),
    ("security_search", "severity", 5),
    ("security_search", "affected", 5),
    ("security_search", "which versions are affected", 8),
    ("security_search", "applicability", 10),
    ("security_search", "assess applicability", 10),
    ("security_search", "cargo.lock", 10),
    ("security_search", "lockfile", 8),
    ("security_search", "known vulnerabilities", 6),
    ("security_search", "vulnerab", 2),
    ("security_search", "package", 3),
    ("security_search", "ecosystem", 4),
    ("security_search", "crate", 4),
    ("security_search", "security changes", 6),
    ("security_search", "osv", 4),
    ("security_search", "nvd", 4),
    ("security_search", "kev", 4),
    ("security_search", "rustsec", 6),
    ("security_search", "exploit", 4),
    ("research_search", "compare", 7),
    ("research_search", " vs ", 6),
    ("research_search", "versus", 6),
    ("research_search", "tradeoffs", 7),
    ("research_search", "trade-off", 6),
    ("research_search", "architecture decision", 10),
    ("research_search", "ecosystem", 6),
    ("research_search", "survey", 7),
    ("research_search", "benchmarks", 4),
    ("research_search", "profiling", 4),
    ("research_search", "performance", 3),
    ("research_search", "investigate", 4),
    ("research_search", "plan migration", 8),
    ("research_search", "migration planning", 8),
    ("research_search", "library comparison", 8),
    ("research_search", "comparison", 5),
    ("research_search", "across api routes", 6),
    ("research_search", "plan", 2),
    ("research_search", "research", 1),
    ("build_evidence_bundle", "package the selected", 12),
    ("build_evidence_bundle", "for handoff", 10),
    ("build_evidence_bundle", "subagent", 10),
    ("build_evidence_bundle", "deterministic evidence bundle", 12),
    ("build_evidence_bundle", "evidence bundle", 10),
    ("build_evidence_bundle", "without new retrieval", 10),
    ("build_evidence_bundle", "source cards and fetch", 8),
    ("build_evidence_bundle", "bundle", 4),
];

const STOPWORDS: &[&str] = &[
    "the", "a", "an", "and", "or", "of", "to", "in", "on", "for", "with", "is", "are", "was",
    "were", "be", "been", "being", "by", "at", "as", "it", "this", "that", "these", "those",
    "from", "into", "over", "my", "our", "your", "their", "his", "her", "its", "you", "we", "they",
    "he", "she", "me", "him", "us", "them", "do", "does", "did", "how", "what", "why", "when",
    "where", "which", "who", "whom", "show", "list", "give", "get", "use", "using", "used",
    "about", "more", "most", "some", "any", "all", "can", "could", "should", "would", "will",
    "shall", "may", "might", "must", "not", "no", "yes", "if", "then", "than", "so", "such", "too",
    "very", "just", "only", "also", "well", "much", "many", "have", "has", "had", "having", "i",
    "s", "t", "re", "ve", "ll", "d", "m",
];

fn tokenize(text: &str) -> HashSet<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| t.len() >= 2 && !STOPWORDS.contains(t))
        .map(str::to_string)
        .collect()
}

fn count_urls(lower: &str) -> usize {
    lower.matches("https://").count() + lower.matches("http://").count()
}

fn has_owner_repo_pattern(lower: &str) -> bool {
    lower.split_whitespace().any(|raw| {
        let word = raw.trim_matches(|c: char| {
            !(c.is_alphanumeric() || c == '/' || c == '-' || c == '_' || c == '.')
        });
        if word.len() < 7 || word.contains(':') {
            return false;
        }
        match word.split_once('/') {
            Some((a, b)) => {
                a.len() >= 2
                    && b.len() >= 2
                    && b.chars().next().is_some_and(|c| c.is_alphanumeric())
                    && a.chars()
                        .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
            }
            None => false,
        }
    })
}

fn score_tool(
    query_lower: &str,
    query_tokens: &HashSet<String>,
    tool: &str,
    desc_tokens: &HashSet<String>,
) -> i32 {
    let mut score = 0;
    for part in tool.split('_') {
        if query_tokens.contains(part) {
            score += 6;
        }
    }
    score += query_tokens
        .iter()
        .filter(|t| desc_tokens.contains(*t))
        .count()
        .min(8) as i32;
    for (name, phrase, weight) in KEYWORDS {
        if *name == tool && query_lower.contains(phrase) {
            score += *weight;
        }
    }
    match count_urls(query_lower) {
        1 if tool == "web_fetch" => score += 12,
        n if n >= 2 && tool == "batch_fetch" => score += 15,
        n if n >= 2 && tool == "web_fetch" => score += 6,
        _ => {}
    }
    if has_owner_repo_pattern(query_lower) {
        match tool {
            "repo_search" => score += 6,
            "repo_fetch" => score += 4,
            "repo_map" => score += 4,
            _ => {}
        }
    }
    if query_lower.contains("src/") {
        match tool {
            "repo_fetch" => score += 5,
            "repo_search" => score += 3,
            _ => {}
        }
    }
    score
}

fn rank_tools(
    query: &str,
    tools: &[rmcp::model::Tool],
    desc_index: &HashMap<String, HashSet<String>>,
) -> Vec<(String, i32)> {
    let lower = query.to_lowercase();
    let tokens = tokenize(query);
    let mut ranked: Vec<(String, i32)> = tools
        .iter()
        .map(|t| {
            let name = t.name.to_string();
            let empty = HashSet::new();
            let desc = desc_index.get(&name).unwrap_or(&empty);
            (name.clone(), score_tool(&lower, &tokens, &name, desc))
        })
        .collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    ranked
}

fn description_index(tools: &[rmcp::model::Tool]) -> HashMap<String, HashSet<String>> {
    tools
        .iter()
        .map(|t| {
            (
                t.name.to_string(),
                tokenize(t.description.as_deref().unwrap_or("")),
            )
        })
        .collect()
}

fn estimate_tokens(bytes: usize) -> usize {
    bytes.div_ceil(4)
}

fn tool_surface_fingerprint(tools: &[rmcp::model::Tool], total_bytes: usize) -> String {
    let mut parts: Vec<String> = tools
        .iter()
        .map(|t| {
            format!(
                "{}:{}",
                t.name,
                t.description.as_deref().unwrap_or("").len()
            )
        })
        .collect();
    parts.sort();
    format!(
        "eggsearch-{}|tools={}|bytes={total_bytes}",
        env!("CARGO_PKG_VERSION"),
        parts.join(",")
    )
}

fn live_server() -> eggsearch::mcp::EggsearchServer {
    let state = Arc::new(ServerState::build(AppConfig::default()).expect("default state builds"));
    eggsearch::mcp::EggsearchServer::new(state)
}

#[test]
fn tool_surface_corpus_is_well_formed() {
    let cases = load_cases();
    assert!(!cases.is_empty(), "tool-surface corpus must not be empty");
    let known: HashSet<&str> = KNOWN_TOOLS.iter().copied().collect();
    let mut ids = HashSet::new();
    let mut primary_coverage: HashSet<&str> = HashSet::new();
    for case in &cases {
        assert!(
            ids.insert(case.id.clone()),
            "duplicate tool-surface case id: {}",
            case.id
        );
        assert!(
            !case.query.trim().is_empty(),
            "case {} must have a query",
            case.id
        );
        assert!(
            known.contains(case.expected_primary.as_str()),
            "case {} has unknown expected_primary {}",
            case.id,
            case.expected_primary
        );
        assert!(
            case.acceptable.contains(&case.expected_primary),
            "case {} acceptable must include expected_primary",
            case.id
        );
        assert!(
            !case.why.trim().is_empty(),
            "case {} must document why the primary is preferred",
            case.id
        );
        for tool in case
            .acceptable
            .iter()
            .chain(case.expected_followups.iter())
            .chain(case.forbidden_primary.iter())
        {
            assert!(
                known.contains(tool.as_str()),
                "case {} references unknown tool {tool}",
                case.id
            );
        }
        for forbidden in &case.forbidden_primary {
            assert!(
                !case.acceptable.contains(forbidden),
                "case {} lists {forbidden} as both acceptable and forbidden",
                case.id
            );
        }
        primary_coverage.insert(case.expected_primary.as_str());
    }
    for tool in KNOWN_TOOLS {
        assert!(
            primary_coverage.contains(tool),
            "corpus must cover {tool} as an expected primary (discoverability)"
        );
    }
}

#[test]
fn tool_surface_byte_budgets() {
    let server = live_server();
    let tools = server.tool_definitions();
    assert_eq!(
        tools.len(),
        KNOWN_TOOLS.len(),
        "must advertise exactly ten stable tools"
    );
    let mut total_bytes = 0usize;
    let mut max_desc = 0usize;
    for tool in &tools {
        let desc = tool.description.as_deref().unwrap_or("");
        assert!(
            !desc.is_empty(),
            "tool {} must have a description",
            tool.name
        );
        assert!(
            desc.len() <= MAX_DESCRIPTION_CHARS,
            "tool {} description is {} chars, max is {MAX_DESCRIPTION_CHARS}",
            tool.name,
            desc.len()
        );
        max_desc = max_desc.max(desc.len());
        let value = serde_json::to_value(tool).expect("tool definition serializes");
        let bytes = serde_json::to_string(&value)
            .expect("tool definition stringifies")
            .len();
        total_bytes += bytes;
        println!(
            "tool-surface-bytes tool={} desc_chars={} json_bytes={} est_tokens={}",
            tool.name,
            desc.len(),
            bytes,
            estimate_tokens(bytes)
        );
    }
    assert!(
        total_bytes <= MAX_TOTAL_DEFINITION_BYTES,
        "total advertised definition bytes {total_bytes} exceeds {MAX_TOTAL_DEFINITION_BYTES}"
    );
    let instructions = server.get_info().instructions.unwrap_or_default();
    assert!(
        instructions.len() <= MAX_INSTRUCTIONS_BYTES,
        "server instructions are {} bytes, max is {MAX_INSTRUCTIONS_BYTES}",
        instructions.len()
    );
    let desc_index = description_index(&tools);
    let cases = load_cases();
    let mut max_compact = 0usize;
    for case in &cases {
        let ranked = rank_tools(&case.query, &tools, &desc_index);
        let compact: Vec<serde_json::Value> = ranked
            .iter()
            .take(3)
            .map(|(name, _)| serde_json::json!({"name": name, "purpose": compact_purpose(name)}))
            .collect();
        let bytes = serde_json::to_string(&compact)
            .expect("compact stringifies")
            .len();
        max_compact = max_compact.max(bytes);
    }
    assert!(
        max_compact <= MAX_COMPACT_DISCOVERY_BYTES,
        "compact discovery result is {max_compact} bytes, max is {MAX_COMPACT_DISCOVERY_BYTES}"
    );
    let fingerprint = tool_surface_fingerprint(&tools, total_bytes);
    assert_eq!(
        fingerprint,
        tool_surface_fingerprint(&tools, total_bytes),
        "tool-surface fingerprint must be deterministic"
    );
    println!(
        "tool-surface-summary total_bytes={total_bytes} est_tokens={} max_desc={max_desc} instructions_bytes={} max_compact={max_compact} fingerprint={fingerprint}",
        estimate_tokens(total_bytes),
        instructions.len(),
    );
}

#[test]
fn tool_surface_discovery_accuracy() {
    let server = live_server();
    let tools = server.tool_definitions();
    let desc_index = description_index(&tools);
    let cases = load_cases();
    let mut top1_hits = 0usize;
    let mut recall3_hits = 0usize;
    let mut reciprocal_sum = 0.0;
    let mut per_category: HashMap<String, (usize, usize)> = HashMap::new();
    let mut failures: Vec<String> = Vec::new();
    for case in &cases {
        let ranked = rank_tools(&case.query, &tools, &desc_index);
        assert_eq!(
            ranked.len(),
            KNOWN_TOOLS.len(),
            "ranking must cover every tool"
        );
        assert!(
            !case.forbidden_primary.contains(&ranked[0].0),
            "case {} ranked forbidden tool {} first: {}",
            case.id,
            ranked[0].0,
            case.query
        );
        let acceptable: HashSet<&str> = case.acceptable.iter().map(String::as_str).collect();
        let first_hit = ranked
            .iter()
            .position(|(name, _)| acceptable.contains(name.as_str()));
        match first_hit {
            Some(0) => top1_hits += 1,
            Some(pos) => {
                failures.push(format!(
                    "{}: primary #{} ({})",
                    case.id,
                    pos + 1,
                    ranked[0].0
                ));
            }
            None => {
                failures.push(format!("{}: primary missing from top-3", case.id));
            }
        }
        if first_hit.is_some_and(|pos| pos < 3) {
            recall3_hits += 1;
        }
        if let Some(pos) = first_hit {
            reciprocal_sum += 1.0 / (pos as f64 + 1.0);
        }
        let entry = per_category.entry(case.category.clone()).or_insert((0, 0));
        entry.1 += 1;
        if first_hit == Some(0) {
            entry.0 += 1;
        }
    }
    let total = cases.len() as f64;
    let top1 = top1_hits as f64 / total;
    let recall3 = recall3_hits as f64 / total;
    let mrr = reciprocal_sum / total;
    let mut categories: Vec<(&String, &(usize, usize))> = per_category.iter().collect();
    categories.sort_by(|a, b| a.0.cmp(b.0));
    for (category, (hits, count)) in &categories {
        println!(
            "tool-surface-category category={category} top1={hits}/{count} acc={:.3}",
            *hits as f64 / *count as f64
        );
    }
    for failure in &failures {
        println!("tool-surface-miss {failure}");
    }
    println!(
        "tool-surface-report cases={} top1={top1_hits}/{n} acc={top1:.3} recall3={recall3_hits}/{n} r3={recall3:.3} mrr={mrr:.3}",
        cases.len(),
        n = cases.len(),
    );
    assert!(
        top1 >= MIN_TOP1_ACCURACY,
        "top-1 accuracy {top1:.3} below {MIN_TOP1_ACCURACY}"
    );
    assert!(
        recall3 >= MIN_RECALL_AT_3,
        "recall@3 {recall3:.3} below {MIN_RECALL_AT_3}"
    );
    assert!(mrr >= MIN_MRR, "MRR {mrr:.3} below {MIN_MRR}");
}

#[test]
fn tool_surface_layer2_synthetic_mechanics() {
    let source_ids = vec!["src_1".to_string(), "src_2".to_string()];
    let bundles = [
        web_search_next_actions(&source_ids, true),
        repo_search_next_actions(&source_ids, true),
        security_search_next_actions(&source_ids, true),
        research_search_next_actions(&source_ids, true),
    ];
    let known: HashSet<&str> = KNOWN_TOOLS.iter().copied().collect();
    let mut hydrated: HashSet<String> = HashSet::new();
    for actions in &bundles {
        assert!(
            actions.len() <= MAX_NEXT_ACTIONS,
            "next-action hydration set exceeds policy cap"
        );
        for action in actions {
            assert!(
                known.contains(action.tool.as_str()),
                "next action hydrates unknown tool {}",
                action.tool
            );
            assert!(
                (1..=5).contains(&action.priority),
                "next action priority out of range for {}",
                action.tool
            );
            hydrated.insert(action.tool.clone());
        }
    }
    assert!(
        hydrated.contains("web_fetch") || hydrated.contains("batch_fetch"),
        "synthetic follow-up hydration must reach a fetch tool: {hydrated:?}"
    );
    assert!(
        hydrated.contains("build_evidence_bundle"),
        "synthetic follow-up hydration must reach evidence handoff: {hydrated:?}"
    );
    let server = live_server();
    let tools = server.tool_definitions();
    let full: usize = tools
        .iter()
        .map(|t| {
            serde_json::to_string(&serde_json::to_value(t).expect("json"))
                .expect("str")
                .len()
        })
        .sum();
    let compact = KNOWN_TOOLS
        .iter()
        .map(|name| {
            serde_json::json!({"name": name, "purpose": compact_purpose(name)})
                .to_string()
                .len()
        })
        .sum::<usize>();
    assert!(
        compact < full,
        "compact discovery ({compact}) must stay below full definitions ({full})"
    );
    println!("tool-surface-layer2 full_bytes={full} compact_bytes={compact} hydrated={hydrated:?}");
}
