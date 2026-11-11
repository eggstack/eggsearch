//! Deterministic retrieval-intent derivation.
//!
//! [`derive_intent`] classifies a query with a fixed precedence and
//! reports stable snake-case reason codes for the class that produced
//! the result. The classifier is pure: no clock, no network, no
//! randomness, and the input is bounded to [`MAX_INTENT_QUERY_CHARS`]
//! before any pattern runs.

use std::sync::LazyLock;

use regex::Regex;

use crate::core::query::SearchIntent;
use crate::core::repo_query::RepoQueryHints;

/// Maximum query prefix scanned by [`derive_intent`].
pub(crate) const MAX_INTENT_QUERY_CHARS: usize = 512;

/// Whether production reranking consumes the derived intent when the
/// caller omitted one.
///
/// The M005 rollout gate replayed the tier-1 candidate snapshot corpus
/// with `intent = web` and with the derived intent. The corpus carries
/// four signal-bearing cases (`security-derived-promotion-001`,
/// `releases-derived-promotion-001`, `docs-derived-promotion-001`, and
/// `code-derived-promotion-001`) that each classify as their annotated
/// intent and each promote their intent-appropriate document. No tier-1
/// metric regresses, and recall, MRR, nDCG, and authoritative survival
/// all improve, so reranking consumes the derived intent.
///
/// `tests/retrieval_quality.rs` re-derives this decision from the corpus
/// and fails when this value disagrees with the recorded evidence; flip
/// it only together with fresh snapshot numbers.
pub(crate) const APPLY_DERIVED_INTENT_TO_RERANKING: bool = true;

/// Confidence of a derived intent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DerivedConfidence {
    /// Identifier or structural evidence (compiler code, advisory id,
    /// version constraint, repository hint, code token).
    High,
    /// Matched vocabulary only.
    Medium,
    /// No signal; the fallback intent.
    Low,
}

impl DerivedConfidence {
    /// Stable lowercase string form.
    pub(crate) const fn as_str(&self) -> &'static str {
        match self {
            Self::High => "high",
            Self::Medium => "medium",
            Self::Low => "low",
        }
    }
}

/// The derived intent plus the reason codes that produced it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DerivedIntent {
    /// The classified intent.
    pub(crate) intent: SearchIntent,
    /// Stable snake-case reason codes, in precedence order.
    pub(crate) reasons: Vec<&'static str>,
    /// Confidence implied by the strongest matching reason.
    pub(crate) confidence: DerivedConfidence,
}

static COMPILER_CODE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:E0\d{3}|TS\d{4,5}|CS\d{4})\b").expect("compiler code regex is valid")
});

static VULNERABILITY_ID_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:CVE-\d{4}-\d{4,}|GHSA(?:-[0-9a-z]{4}){2,}|RUSTSEC-\d{4}-\d{4})\b")
        .expect("vulnerability identifier regex is valid")
});

static VERSION_CONSTRAINT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:\b[a-z][a-z0-9._-]*\s*)?(?:==|>=|<=|~=|\^|~)\s*v?\d+\.\d+(?:\.\d+)?\b")
        .expect("version constraint regex is valid")
});

static PACKAGE_AT_VERSION_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b[A-Za-z0-9][A-Za-z0-9_.-]*@\d+\.\d+(?:\.\d+)?\b")
        .expect("package version regex is valid")
});

static NAMED_SEMVER_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b[a-z][a-z0-9._-]{1,}\s+v?\d+\.\d+\.\d+\b").expect("named semver regex is valid")
});

static COMPARISON_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\S+\s+vs\.?\s+\S+").expect("comparison regex is valid"));

static RUST_PATH_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b[A-Za-z_][A-Za-z0-9_]*(?:::[A-Za-z_][A-Za-z0-9_]*)+\b")
        .expect("rust path regex is valid")
});

static SNAKE_CASE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b[a-z][a-z0-9]*_[a-z0-9_]+\b").expect("snake case regex is valid")
});

static CAMEL_CASE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b[a-z][a-z0-9]*(?:[A-Z][a-z0-9]+){2,}\b").expect("camel case regex is valid")
});

static SOURCE_PATH_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b[\w.-]+/[\w.-]+/[\w.-]+\b").expect("source path regex is valid")
});

static SOURCE_FILE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"(?i)\b[\w-]+\.(?:rs|py|ts|tsx|js|jsx|go|java|kt|rb|php|cs|cpp|cxx|hpp|h|swift",
        r"|scala|ex|exs|hs|toml|lock|json|yaml|gradle|nix)\b"
    ))
    .expect("source file regex is valid")
});

static CALL_SYNTAX_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b[a-z_][a-z0-9_]*\(\s*\)").expect("call syntax regex is valid"));

static FRAME_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"\b[\w./-]*\.(?:rs|py|ts|tsx|js|jsx|go|java|kt|rb|php|cs|cpp|cxx|hpp|h|swift",
        r"|scala|ex|exs|hs):\d+(?::\d+)?\b"
    ))
    .expect("stack frame regex is valid")
});

const VULNERABILITY_WORDS: &[&str] = &[
    "advisories",
    "advisory",
    "cve",
    "exploit",
    "exploited",
    "exploits",
    "ghsa",
    "rustsec",
    "vulnerabilities",
    "vulnerability",
    "vulnerable",
];

const MIGRATION_WORDS: &[&str] = &[
    "migrate",
    "migrated",
    "migrates",
    "migrating",
    "migration",
    "migrations",
    "upgrade",
    "upgraded",
    "upgrades",
    "upgrading",
];

const COMPARISON_PHRASES: &[&str] = &[
    "alternative to",
    "alternatives to",
    "better than",
    "compared to",
    "compared with",
    "difference between",
];

const RELEASE_WORDS: &[&str] = &[
    "changelog",
    "changelogs",
    "release",
    "released",
    "releases",
    "releasing",
];

const RELEASE_PHRASES: &[&str] = &[
    "release note",
    "release notes",
    "version history",
    "what's new",
    "whats new",
];

const ISSUE_WORDS: &[&str] = &["bug", "bugs", "issue", "issues", "pr", "prs"];

const ISSUE_PHRASES: &[&str] = &["pull request", "pull requests", "bug report", "bug reports"];

const DOCS_WORDS: &[&str] = &[
    "doc",
    "docs",
    "documentation",
    "handbook",
    "manual",
    "tutorial",
    "tutorials",
];

const DOCS_PHRASES: &[&str] = &[
    "api docs",
    "api reference",
    "getting started",
    "how to",
    "howto",
];

const NEWS_WORDS: &[&str] = &[
    "announced",
    "announcement",
    "headlines",
    "latest",
    "news",
    "recent",
    "recently",
    "today",
    "trending",
];

const NEWS_PHRASES: &[&str] = &["breaking news", "this month", "this week"];

const PANIC_MARKERS: &[&str] = &[
    "backtrace",
    "core dumped",
    "panic:",
    "panicked at",
    "segmentation fault",
    "stack trace",
    "thread '",
    "traceback (most recent call last)",
];

const REPO_HINT_PREFIXES: &[&str] = &["repo:", "repository:", "project:", "repo=", "org:"];

/// Derive the retrieval intent for `query`.
///
/// Precedence is fixed and evaluated once; the first matching class
/// wins:
///
/// 1. error shapes (compiler codes, runtime exceptions, panic and
///    stack-frame markers) → `code`
/// 2. vulnerability identifiers and vocabulary → `security`
/// 3. dependency and version syntax → `code`
/// 4. migration and upgrade vocabulary → `releases`
/// 5. comparison vocabulary → `web`
/// 6. release and changelog vocabulary → `releases`
/// 7. issue and pull-request vocabulary → `issues`
/// 8. code-shaped tokens and repository hints → `code`
/// 9. documentation vocabulary → `docs`
/// 10. news and recency vocabulary → `news`
/// 11. fallback → `web` (`no_signal`)
pub(crate) fn derive_intent(query: &str, hints: &RepoQueryHints) -> DerivedIntent {
    let bounded = bound_query(query);
    let lower = bounded.to_lowercase();
    let tokens: Vec<&str> = lower
        .split(|c: char| !c.is_ascii_alphanumeric() && c != '\'')
        .filter(|token| !token.is_empty())
        .collect();

    let classes: [(SearchIntent, Vec<&'static str>); 10] = [
        (SearchIntent::Code, error_reasons(&bounded, &lower)),
        (SearchIntent::Security, security_reasons(&lower, &tokens)),
        (SearchIntent::Code, dependency_reasons(&bounded)),
        (
            SearchIntent::Releases,
            vocabulary(
                &lower,
                &tokens,
                MIGRATION_WORDS,
                &[],
                "migration_vocabulary",
            ),
        ),
        (SearchIntent::Web, comparison_reasons(&lower)),
        (
            SearchIntent::Releases,
            vocabulary(
                &lower,
                &tokens,
                RELEASE_WORDS,
                RELEASE_PHRASES,
                "release_vocabulary",
            ),
        ),
        (
            SearchIntent::Issues,
            vocabulary(
                &lower,
                &tokens,
                ISSUE_WORDS,
                ISSUE_PHRASES,
                "issue_vocabulary",
            ),
        ),
        (SearchIntent::Code, code_reasons(&bounded, &lower, hints)),
        (
            SearchIntent::Docs,
            vocabulary(&lower, &tokens, DOCS_WORDS, DOCS_PHRASES, "docs_vocabulary"),
        ),
        (
            SearchIntent::News,
            vocabulary(&lower, &tokens, NEWS_WORDS, NEWS_PHRASES, "news_vocabulary"),
        ),
    ];

    for (intent, reasons) in classes {
        if !reasons.is_empty() {
            let confidence = if reasons
                .iter()
                .any(|reason| HIGH_CONFIDENCE_REASONS.contains(reason))
            {
                DerivedConfidence::High
            } else {
                DerivedConfidence::Medium
            };
            return DerivedIntent {
                intent,
                reasons,
                confidence,
            };
        }
    }

    DerivedIntent {
        intent: SearchIntent::Web,
        reasons: vec!["no_signal"],
        confidence: DerivedConfidence::Low,
    }
}

const HIGH_CONFIDENCE_REASONS: &[&str] = &[
    "compiler_error_code",
    "stack_frame",
    "vulnerability_identifier",
    "dependency_version",
    "repo_hint",
    "symbol_hint",
    "path_hint",
    "file_hint",
    "language_hint",
    "host_hint",
    "code_token",
];

fn bound_query(query: &str) -> String {
    query.chars().take(MAX_INTENT_QUERY_CHARS).collect()
}

fn error_reasons(query: &str, lower: &str) -> Vec<&'static str> {
    let parts = crate::core::error_query::parse_error_query(query);
    let mut reasons = Vec::new();
    if COMPILER_CODE_RE.is_match(query)
        || parts
            .error_codes
            .iter()
            .any(|code| matches!(code.tool.as_str(), "rustc" | "typescript"))
    {
        reasons.push("compiler_error_code");
    }
    if !parts.error_codes.is_empty() {
        reasons.push("error_vocabulary");
    }
    if !parts.stack_frames.is_empty() || FRAME_RE.is_match(query) {
        reasons.push("stack_frame");
    }
    if PANIC_MARKERS.iter().any(|marker| lower.contains(marker)) {
        reasons.push("panic_marker");
    }
    reasons
}

fn security_reasons(lower: &str, tokens: &[&str]) -> Vec<&'static str> {
    let mut reasons = Vec::new();
    if VULNERABILITY_ID_RE.is_match(lower) {
        reasons.push("vulnerability_identifier");
    }
    if tokens
        .iter()
        .any(|token| VULNERABILITY_WORDS.contains(token))
    {
        reasons.push("vulnerability_vocabulary");
    }
    reasons
}

fn dependency_reasons(query: &str) -> Vec<&'static str> {
    if VERSION_CONSTRAINT_RE.is_match(query)
        || PACKAGE_AT_VERSION_RE.is_match(query)
        || NAMED_SEMVER_RE.is_match(query)
    {
        vec!["dependency_version"]
    } else {
        Vec::new()
    }
}

fn comparison_reasons(lower: &str) -> Vec<&'static str> {
    if COMPARISON_RE.is_match(lower)
        || lower.contains(" versus ")
        || lower.starts_with("versus ")
        || COMPARISON_PHRASES
            .iter()
            .any(|phrase| lower.contains(phrase))
    {
        vec!["comparison_vocabulary"]
    } else {
        Vec::new()
    }
}

fn vocabulary(
    lower: &str,
    tokens: &[&str],
    words: &[&str],
    phrases: &[&str],
    reason: &'static str,
) -> Vec<&'static str> {
    let matched = tokens.iter().any(|token| words.contains(token))
        || phrases.iter().any(|phrase| lower.contains(phrase));
    if matched {
        vec![reason]
    } else {
        Vec::new()
    }
}

fn code_reasons(query: &str, lower: &str, hints: &RepoQueryHints) -> Vec<&'static str> {
    let mut reasons = Vec::new();
    if (hints.owner.is_some() && hints.repo.is_some() || hints.org.is_some())
        && REPO_HINT_PREFIXES
            .iter()
            .any(|prefix| lower.contains(prefix))
    {
        reasons.push("repo_hint");
    }
    if hints.symbol.is_some() {
        reasons.push("symbol_hint");
    }
    if hints.path.is_some() {
        reasons.push("path_hint");
    }
    if hints.file.is_some() {
        reasons.push("file_hint");
    }
    if hints.language.is_some() {
        reasons.push("language_hint");
    }
    if hints.host.is_some() {
        reasons.push("host_hint");
    }
    if RUST_PATH_RE.is_match(query)
        || SNAKE_CASE_RE.is_match(query)
        || CAMEL_CASE_RE.is_match(query)
        || SOURCE_PATH_RE.is_match(query)
        || SOURCE_FILE_RE.is_match(query)
        || CALL_SYNTAX_RE.is_match(query)
    {
        reasons.push("code_token");
    }
    reasons
}

#[cfg(test)]
mod tests {
    use super::*;

    fn derive(query: &str) -> DerivedIntent {
        derive_intent(query, &RepoQueryHints::parse(query))
    }

    fn assert_class(query: &str, intent: SearchIntent, reason: &str) {
        let derived = derive(query);
        assert_eq!(
            derived.intent, intent,
            "query {query:?} derived {:?} with {:?}",
            derived.intent, derived.reasons
        );
        assert!(
            derived.reasons.contains(&reason),
            "query {query:?} must report {reason}, got {:?}",
            derived.reasons
        );
    }

    #[test]
    fn compiler_error_codes_are_code() {
        assert_class(
            "rust E0277 trait bound not satisfied",
            SearchIntent::Code,
            "compiler_error_code",
        );
        assert_class(
            "TS2345 argument of type string",
            SearchIntent::Code,
            "compiler_error_code",
        );
        assert_class(
            "CS0246 type or namespace not found",
            SearchIntent::Code,
            "compiler_error_code",
        );
        assert_class(
            "error[E0502] cannot borrow as mutable",
            SearchIntent::Code,
            "compiler_error_code",
        );
    }

    #[test]
    fn runtime_exceptions_are_error_vocabulary() {
        assert_class(
            "Python TypeError NoneType is not subscriptable",
            SearchIntent::Code,
            "error_vocabulary",
        );
        assert_class(
            "npm ENOENT no such file",
            SearchIntent::Code,
            "error_vocabulary",
        );
    }

    #[test]
    fn panic_and_stack_frames_are_code() {
        assert_class(
            "thread 'main' panicked at src/main.rs:12:5",
            SearchIntent::Code,
            "panic_marker",
        );
        assert_class(
            "Traceback (most recent call last)",
            SearchIntent::Code,
            "panic_marker",
        );
        assert_class("crash at main.rs:42", SearchIntent::Code, "stack_frame");
        assert_class(
            "segmentation fault in libfoo",
            SearchIntent::Code,
            "panic_marker",
        );
    }

    #[test]
    fn security_identifiers_are_security() {
        assert_class(
            "CVE-2024-1234 openssl",
            SearchIntent::Security,
            "vulnerability_identifier",
        );
        assert_class(
            "GHSA-8fr3-jv9q-4j6g tokio",
            SearchIntent::Security,
            "vulnerability_identifier",
        );
        assert_class(
            "RUSTSEC-2024-0001 memory safety",
            SearchIntent::Security,
            "vulnerability_identifier",
        );
    }

    #[test]
    fn security_vocabulary_is_security() {
        assert_class(
            "openssl vulnerability advisory",
            SearchIntent::Security,
            "vulnerability_vocabulary",
        );
        assert_class(
            "is this crate vulnerable",
            SearchIntent::Security,
            "vulnerability_vocabulary",
        );
        assert_class(
            "exploit technique for log4j",
            SearchIntent::Security,
            "vulnerability_vocabulary",
        );
    }

    #[test]
    fn dependency_versions_are_code() {
        assert_class(
            "serde == 1.0.200 features",
            SearchIntent::Code,
            "dependency_version",
        );
        assert_class(
            "react@18.2.0 hydration",
            SearchIntent::Code,
            "dependency_version",
        );
        assert_class(
            "tokio >=1.35.0 breaking",
            SearchIntent::Code,
            "dependency_version",
        );
        assert_class(
            "axum ^0.7 routing",
            SearchIntent::Code,
            "dependency_version",
        );
        assert_class(
            "numpy 1.26.4 wheels",
            SearchIntent::Code,
            "dependency_version",
        );
    }

    #[test]
    fn migration_vocabulary_is_releases() {
        assert_class(
            "migrate from diesel 1 to 2",
            SearchIntent::Releases,
            "migration_vocabulary",
        );
        assert_class(
            "upgrading react 17 to 18",
            SearchIntent::Releases,
            "migration_vocabulary",
        );
        assert_class(
            "breaking change migration guide",
            SearchIntent::Releases,
            "migration_vocabulary",
        );
    }

    #[test]
    fn comparison_vocabulary_is_web() {
        assert_class("vim vs emacs", SearchIntent::Web, "comparison_vocabulary");
        assert_class(
            "rust versus go for services",
            SearchIntent::Web,
            "comparison_vocabulary",
        );
        assert_class(
            "difference between threads and processes",
            SearchIntent::Web,
            "comparison_vocabulary",
        );
        assert_class(
            "alternative to kubernetes",
            SearchIntent::Web,
            "comparison_vocabulary",
        );
    }

    #[test]
    fn release_vocabulary_is_releases() {
        assert_class(
            "tokio release notes",
            SearchIntent::Releases,
            "release_vocabulary",
        );
        assert_class(
            "axum changelog",
            SearchIntent::Releases,
            "release_vocabulary",
        );
        assert_class(
            "what's new in rust 1.80",
            SearchIntent::Releases,
            "release_vocabulary",
        );
    }

    #[test]
    fn issue_vocabulary_is_issues() {
        assert_class(
            "axum websocket issue",
            SearchIntent::Issues,
            "issue_vocabulary",
        );
        assert_class(
            "pull request review checklist",
            SearchIntent::Issues,
            "issue_vocabulary",
        );
        assert_class(
            "serde bug reports",
            SearchIntent::Issues,
            "issue_vocabulary",
        );
    }

    #[test]
    fn code_tokens_and_hints_are_code() {
        assert_class("Router::layer ordering", SearchIntent::Code, "code_token");
        assert_class(
            "getElementById returns null",
            SearchIntent::Code,
            "code_token",
        );
        assert_class(
            "parse_error_query signature",
            SearchIntent::Code,
            "code_token",
        );
        assert_class("src/meta/adapter layout", SearchIntent::Code, "code_token");
        assert_class("src/lib.rs module map", SearchIntent::Code, "code_token");
        assert_class("unwrap() on None", SearchIntent::Code, "code_token");
        assert_class(
            "repo:tokio-rs/axum middleware",
            SearchIntent::Code,
            "repo_hint",
        );
        assert_class("org:tokio-rs axum", SearchIntent::Code, "repo_hint");
        assert_class(
            "symbol:Router::layer axum",
            SearchIntent::Code,
            "symbol_hint",
        );
        assert_class(
            "lang:rust error handling",
            SearchIntent::Code,
            "language_hint",
        );
        assert_class("path:src/lib.rs exports", SearchIntent::Code, "path_hint");
        assert_class("file:Cargo.toml workspace", SearchIntent::Code, "file_hint");
        assert_class(
            "host:gitlab merge pipeline",
            SearchIntent::Code,
            "host_hint",
        );
    }

    #[test]
    fn docs_vocabulary_is_docs() {
        assert_class(
            "tokio documentation runtime",
            SearchIntent::Docs,
            "docs_vocabulary",
        );
        assert_class("axum api reference", SearchIntent::Docs, "docs_vocabulary");
        assert_class(
            "getting started with sqlx",
            SearchIntent::Docs,
            "docs_vocabulary",
        );
        assert_class(
            "how to configure tracing",
            SearchIntent::Docs,
            "docs_vocabulary",
        );
    }

    #[test]
    fn news_vocabulary_is_news() {
        assert_class("latest ai news", SearchIntent::News, "news_vocabulary");
        assert_class("headlines this week", SearchIntent::News, "news_vocabulary");
        assert_class(
            "recently announced chips",
            SearchIntent::News,
            "news_vocabulary",
        );
    }

    #[test]
    fn fallback_is_web_with_no_signal() {
        let derived = derive("best sourdough bread recipe");
        assert_eq!(derived.intent, SearchIntent::Web);
        assert_eq!(derived.reasons, vec!["no_signal"]);
        assert_eq!(derived.confidence, DerivedConfidence::Low);
        assert_eq!(derive("").reasons, vec!["no_signal"]);
        assert_eq!(derive("   ").reasons, vec!["no_signal"]);
    }

    #[test]
    fn near_miss_negatives_do_not_match() {
        let cases = [
            ("error", "error handling best practices"),
            ("security", "security camera installation"),
            ("dependency", "python 3.11 typing features"),
            ("migration", "migratory bird routes"),
            ("comparison", "vs code extension marketplace"),
            ("release", "relief road closures"),
            ("issue", "the problem with modern diets"),
            ("code", "javascript frameworks overview"),
            ("code", "github actions workflow syntax"),
            ("code", "read/write throughput benchmarks"),
            ("docs", "travel guide to japan"),
            ("news", "new restaurant openings"),
            ("fallback", "memory usage of containers"),
        ];
        for (near, query) in cases {
            let derived = derive(query);
            assert_eq!(
                derived.intent,
                SearchIntent::Web,
                "{near} near miss {query:?} derived {:?} {:?}",
                derived.intent,
                derived.reasons
            );
        }
    }

    #[test]
    fn precedence_is_fixed_first_class_wins() {
        assert_eq!(
            derive("CVE-2024-1234 release notes").intent,
            SearchIntent::Security
        );
        assert_eq!(
            derive("E0277 compiler issue discussion").intent,
            SearchIntent::Code
        );
        assert_eq!(
            derive("serde == 1.0.200 changelog").intent,
            SearchIntent::Code
        );
        assert_eq!(
            derive("migrate to 2.0 vs staying on 1.x").intent,
            SearchIntent::Releases
        );
        assert_eq!(
            derive("Router::layer vs middleware").intent,
            SearchIntent::Web,
            "comparison outranks code tokens"
        );
    }

    #[test]
    fn derivation_is_deterministic() {
        for query in [
            "rust E0277",
            "repo:tokio-rs/axum Router::layer",
            "latest news about sqlite",
            "best sourdough bread recipe",
        ] {
            let first = derive(query);
            for _ in 0..8 {
                assert_eq!(derive(query), first);
            }
        }
    }

    #[test]
    fn input_is_bounded_to_the_query_limit() {
        let long_query = format!("{} CVE-2024-1234", "a".repeat(MAX_INTENT_QUERY_CHARS));
        assert_eq!(derive(&long_query).reasons, vec!["no_signal"]);

        let tail_token = format!("{} Router::layer", "b".repeat(MAX_INTENT_QUERY_CHARS - 32));
        assert_eq!(derive(&tail_token).reasons, vec!["code_token"]);
    }

    #[test]
    fn unicode_input_is_classified_without_panicking() {
        let derived = derive("错误的 E0277 编译错误 — なぜ");
        assert_eq!(derived.intent, SearchIntent::Code);
        assert!(derived.reasons.contains(&"compiler_error_code"));

        let long_unicode = "界".repeat(MAX_INTENT_QUERY_CHARS + 40);
        assert_eq!(derive(&long_unicode).reasons, vec!["no_signal"]);
    }

    #[test]
    fn reasons_are_lowercase_snake_case() {
        let queries = [
            "rust E0277",
            "CVE-2024-1234",
            "serde == 1.0.200",
            "migrate to 2.0",
            "vim vs emacs",
            "axum changelog",
            "axum issue",
            "Router::layer",
            "axum documentation",
            "latest news",
            "sourdough recipe",
        ];
        for query in queries {
            let derived = derive(query);
            assert!(!derived.reasons.is_empty());
            for reason in derived.reasons {
                assert!(
                    !reason.is_empty()
                        && reason.chars().all(|c| c.is_ascii_lowercase() || c == '_'),
                    "reason {reason} must be lowercase snake case"
                );
            }
        }
    }
}
