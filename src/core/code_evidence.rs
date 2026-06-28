//! Exact code evidence metadata for Phase 1.
//!
//! This module defines the `CodeEvidence` data model and pure helper
//! functions for extracting structured evidence about code matches
//! from search results. It bridges `CodeMetadata` (URL-derived) with
//! enriched fields like raw URLs, source roles, and confidence levels.

use serde::{Deserialize, Serialize};

use crate::core::code_metadata::{CodeHost, CodeMetadata};

/// The role a file plays in a repository.
#[derive(
    Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum SourceRole {
    /// Application or library source code.
    #[default]
    Implementation,
    /// Test files.
    Test,
    /// Example code.
    Example,
    /// Benchmarks.
    Benchmark,
    /// Configuration files.
    Configuration,
    /// Build scripts, CI, Dockerfiles.
    Build,
    /// User-facing documentation.
    Documentation,
    /// Project README.
    Readme,
    /// Release changelog.
    Changelog,
    /// Migration guides.
    Migration,
    /// Unrecognized or ambiguous.
    Unknown,
}

/// Kind of symbol matched in a code search result.
#[derive(
    Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum SymbolKind {
    /// A free function.
    #[default]
    Function,
    /// A method on a type.
    Method,
    /// A struct definition.
    Struct,
    /// An enum definition.
    Enum,
    /// A trait definition.
    Trait,
    /// A class (e.g. Python, Java).
    Class,
    /// An interface (e.g. TypeScript, Java).
    Interface,
    /// A module or namespace.
    Module,
    /// A constant or static value.
    Constant,
    /// A type alias.
    TypeAlias,
    /// A macro.
    Macro,
    /// Unrecognized or ambiguous symbol kind.
    Unknown,
}

/// Confidence level for the evidence linkage.
#[derive(
    Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceConfidence {
    /// Exact match (e.g. line anchor + symbol).
    #[default]
    Exact,
    /// Strong match (e.g. URL path + provider text match).
    Strong,
    /// Weak match (e.g. language or repo only).
    Weak,
    /// Unrecoverable or unclassified confidence.
    Unknown,
}

/// Why this evidence was linked to the query.
#[derive(
    Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum CodeEvidenceReason {
    /// URL contains a line anchor (`#L10-L25`).
    #[default]
    UrlLineAnchor,
    /// Provider returned a text match snippet.
    ProviderTextMatch,
    /// Provider returned a path-based match.
    ProviderPathMatch,
    /// Provider returned a symbol-based match.
    ProviderSymbolMatch,
    /// Language inference matched the query context.
    LanguageMatch,
    /// Repository ownership matched the query hints.
    RepoMatch,
    /// Path hint matched the result.
    PathHintMatch,
    /// File hint matched the result.
    FileHintMatch,
    /// Raw content URL was derived from browser URL.
    RawUrlDerived,
    /// Permalink URL was derived.
    PermalinkDerived,
    /// Source role was inferred from the file path.
    SourceRoleInferred,
}

/// Structured evidence about a code match.
///
/// All fields are optional because not every search result yields every
/// piece of evidence. The struct is `Default` (all `None` / empty) so
/// callers can attach it to any `SourceCard` without conditional logic.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CodeEvidence {
    /// The code-hosting platform.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<CodeHost>,
    /// Repository owner (or namespace for GitLab nested groups).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    /// Repository name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repo: Option<String>,
    /// Branch, tag, or commit ref.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ref_name: Option<String>,
    /// Full commit SHA (when available).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit_sha: Option<String>,
    /// File or directory path within the repository.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// Inferred programming language from file extension.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    /// The role this file plays in the repository.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_role: Option<SourceRole>,
    /// Human-readable browser URL for this result.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub browser_url: Option<String>,
    /// Raw content URL (e.g. raw.githubusercontent.com).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_url: Option<String>,
    /// Stable permalink URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permalink_url: Option<String>,
    /// Start line of the matched region.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub match_line_start: Option<u32>,
    /// End line of the matched region.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub match_line_end: Option<u32>,
    /// Start line of the surrounding context window.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_line_start: Option<u32>,
    /// End line of the surrounding context window.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_line_end: Option<u32>,
    /// The symbol name that was matched.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matched_symbol: Option<String>,
    /// Kind of the matched symbol.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symbol_kind: Option<SymbolKind>,
    /// Enclosing type or module of the matched symbol.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enclosing_symbol: Option<String>,
    /// Confidence level for the evidence linkage.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence_confidence: Option<EvidenceConfidence>,
    /// Reasons this evidence was linked to the query.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence_reasons: Vec<CodeEvidenceReason>,
}

/// Infer `SourceRole` from a file path.
///
/// Checks filename patterns first (README, CHANGELOG, etc.), then
/// path component patterns (tests/, examples/, etc.), then
/// configuration files. Defaults to `Implementation` for recognized
/// source code extensions unless a more specific role matches.
pub fn infer_source_role(path: &str) -> SourceRole {
    let filename = path.rsplit('/').next().unwrap_or(path);
    let lower_filename = filename.to_lowercase();

    // Filename patterns
    if lower_filename.starts_with("readme") {
        return SourceRole::Readme;
    }
    if lower_filename.starts_with("changelog") || lower_filename.starts_with("changes") {
        return SourceRole::Changelog;
    }
    if lower_filename == "license" || lower_filename.starts_with("license.") {
        return SourceRole::Configuration;
    }

    // Configuration files by exact name
    match lower_filename.as_str() {
        "cargo.toml" | "pyproject.toml" | "package.json" | "setup.py" | "setup.cfg"
        | "requirements.txt" | "go.mod" | "go.sum" | "pom.xml" | "build.gradle"
        | "build.gradle.kts" | "dockerfile" | "docker-compose.yml"
        | "docker-compose.yaml" | "makefile" | "justfile" | ".gitignore"
        | ".gitattributes" | ".editorconfig" | "rustfmt.toml" | ".rustfmt.toml"
        | "clippy.toml" | ".clippy.toml" | "deny.toml" | "release.toml" => {
            return SourceRole::Configuration;
        }
        _ => {}
    }

    // Path component patterns
    let lower_path = path.to_lowercase();
    let components: Vec<&str> = lower_path.split('/').collect();

    for comp in &components {
        match *comp {
            "tests" | "test" | "__tests__" => return SourceRole::Test,
            "examples" | "example" | "demo" | "demos" => return SourceRole::Example,
            "benches" | "bench" | "benchmarks" | "benchmark" => return SourceRole::Benchmark,
            "docs" | "doc" | "documentation" | "wiki" => return SourceRole::Documentation,
            ".github" => {
                return SourceRole::Build;
            }
            _ => {}
        }
    }

    // CI config files anywhere in the path
    if lower_path.contains(".github/workflows/") || lower_path.contains(".circleci/")
        || lower_path.contains(".travis") || lower_path.contains("jenkinsfile")
    {
        return SourceRole::Build;
    }

    // Test files by suffix pattern
    if lower_filename.ends_with("_test.rs") || lower_filename.ends_with("_test.py")
        || lower_filename.ends_with(".test.ts") || lower_filename.ends_with(".test.js")
        || lower_filename.ends_with(".spec.ts") || lower_filename.ends_with(".spec.js")
        || lower_filename.starts_with("test_")
    {
        return SourceRole::Test;
    }

    // Migration files
    if lower_path.contains("migration") || lower_path.contains("migrations") {
        return SourceRole::Migration;
    }

    // Recognized source code extensions default to Implementation
    let ext = filename.rsplit('.').next().unwrap_or("");
    match ext {
        "rs" | "py" | "ts" | "tsx" | "js" | "jsx" | "go" | "java" | "kt" | "c" | "h"
        | "cpp" | "cc" | "hpp" | "rb" | "php" | "swift" | "m" | "mm" => {
            return SourceRole::Implementation;
        }
        _ => {}
    }

    SourceRole::Unknown
}

/// Derive a GitHub raw URL from GitHub metadata.
///
/// Returns the raw content URL. Caller must ensure owner, repo,
/// ref, and path are known (this function does not return `Option`).
pub fn derive_github_raw_url(owner: &str, repo: &str, ref_name: &str, path: &str) -> String {
    format!(
        "https://raw.githubusercontent.com/{owner}/{repo}/{ref_name}/{path}"
    )
}

/// Derive a GitLab raw URL from GitLab metadata.
pub fn derive_gitlab_raw_url(owner: &str, repo: &str, ref_name: &str, path: &str) -> String {
    let namespace = if owner.is_empty() {
        repo.to_string()
    } else {
        format!("{owner}/{repo}")
    };
    format!(
        "https://gitlab.com/{namespace}/-/raw/{ref_name}/{path}"
    )
}

/// Derive a stable browser URL from raw URL metadata (GitHub blob URL).
pub fn derive_browser_url(owner: &str, repo: &str, ref_name: &str, path: &str) -> String {
    format!(
        "https://github.com/{owner}/{repo}/blob/{ref_name}/{path}"
    )
}

/// Build `CodeEvidence` from existing `CodeMetadata`.
///
/// This is the main integration point. Attaches raw_url, browser_url,
/// source_role, and evidence_reasons based on what information is
/// available. Returns `None` when the metadata is too sparse to
/// produce meaningful evidence (no owner/repo/path).
pub fn build_code_evidence(code: &CodeMetadata, browser_url: Option<&str>) -> Option<CodeEvidence> {
    let owner = code.owner.as_deref()?;
    let repo = code.repo.as_deref()?;
    let ref_name = code.ref_name.as_deref().unwrap_or("main");
    let path = code.path.as_deref()?;

    let host = code.host.unwrap_or(CodeHost::Unknown);
    let language = code
        .language
        .clone()
        .or_else(|| crate::core::code_metadata::language_from_extension(path).map(String::from));

    let source_role = infer_source_role(path);

    // Build URLs based on host
    let (raw_url, browser_url_value, permalink_url) = match host {
        CodeHost::Github => {
            let raw = derive_github_raw_url(owner, repo, ref_name, path);
            let browser = browser_url
                .map(String::from)
                .unwrap_or_else(|| derive_browser_url(owner, repo, ref_name, path));
            let permalink = derive_github_raw_url(owner, repo, ref_name, path);
            (Some(raw), Some(browser), Some(permalink))
        }
        CodeHost::Gitlab => {
            let raw = derive_gitlab_raw_url(owner, repo, ref_name, path);
            let browser = browser_url.map(String::from);
            let permalink = Some(raw.clone());
            (Some(raw), browser, permalink)
        }
        _ => (None, browser_url.map(String::from), None),
    };

    let mut evidence_reasons = Vec::new();
    if raw_url.is_some() {
        evidence_reasons.push(CodeEvidenceReason::RawUrlDerived);
    }
    if source_role != SourceRole::Unknown {
        evidence_reasons.push(CodeEvidenceReason::SourceRoleInferred);
    }
    if language.is_some() {
        evidence_reasons.push(CodeEvidenceReason::LanguageMatch);
    }

    Some(CodeEvidence {
        host: Some(host),
        owner: Some(owner.to_string()),
        repo: Some(repo.to_string()),
        ref_name: Some(ref_name.to_string()),
        commit_sha: None,
        path: Some(path.to_string()),
        language,
        source_role: Some(source_role),
        browser_url: browser_url_value,
        raw_url,
        permalink_url,
        match_line_start: code.line_start,
        match_line_end: code.line_end,
        context_line_start: None,
        context_line_end: None,
        matched_symbol: code.symbol_hint.clone(),
        symbol_kind: None,
        enclosing_symbol: None,
        evidence_confidence: Some(EvidenceConfidence::Strong),
        evidence_reasons,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Source role inference tests ---

    #[test]
    fn infer_source_role_implementation() {
        assert_eq!(infer_source_role("src/lib.rs"), SourceRole::Implementation);
    }

    #[test]
    fn infer_source_role_test_by_dir() {
        assert_eq!(
            infer_source_role("tests/integration.rs"),
            SourceRole::Test
        );
    }

    #[test]
    fn infer_source_role_test_by_suffix() {
        assert_eq!(infer_source_role("src/foo_test.rs"), SourceRole::Test);
    }

    #[test]
    fn infer_source_role_example() {
        assert_eq!(
            infer_source_role("examples/server.rs"),
            SourceRole::Example
        );
    }

    #[test]
    fn infer_source_role_configuration() {
        assert_eq!(
            infer_source_role("Cargo.toml"),
            SourceRole::Configuration
        );
    }

    #[test]
    fn infer_source_role_build_ci() {
        assert_eq!(
            infer_source_role(".github/workflows/ci.yml"),
            SourceRole::Build
        );
    }

    #[test]
    fn infer_source_role_readme() {
        assert_eq!(infer_source_role("README.md"), SourceRole::Readme);
    }

    #[test]
    fn infer_source_role_changelog() {
        assert_eq!(infer_source_role("CHANGELOG.md"), SourceRole::Changelog);
    }

    #[test]
    fn infer_source_role_benchmark() {
        assert_eq!(
            infer_source_role("benches/foo.rs"),
            SourceRole::Benchmark
        );
    }

    #[test]
    fn infer_source_role_documentation() {
        assert_eq!(
            infer_source_role("docs/guide.md"),
            SourceRole::Documentation
        );
    }

    #[test]
    fn infer_source_role_unknown_extension() {
        assert_eq!(infer_source_role("data.xyz"), SourceRole::Unknown);
    }

    // --- URL derivation tests ---

    #[test]
    fn derive_github_raw_url_basic() {
        let url = derive_github_raw_url("tokio-rs", "axum", "main", "src/lib.rs");
        assert_eq!(
            url,
            "https://raw.githubusercontent.com/tokio-rs/axum/main/src/lib.rs"
        );
    }

    #[test]
    fn derive_gitlab_raw_url_basic() {
        let url = derive_gitlab_raw_url("group", "project", "main", "src/lib.rs");
        assert_eq!(
            url,
            "https://gitlab.com/group/project/-/raw/main/src/lib.rs"
        );
    }

    #[test]
    fn derive_browser_url_basic() {
        let url = derive_browser_url("tokio-rs", "axum", "main", "src/lib.rs");
        assert_eq!(
            url,
            "https://github.com/tokio-rs/axum/blob/main/src/lib.rs"
        );
    }

    // --- build_code_evidence tests ---

    #[test]
    fn build_code_evidence_full_metadata() {
        let code = CodeMetadata {
            host: Some(CodeHost::Github),
            owner: Some("tokio-rs".to_string()),
            repo: Some("axum".to_string()),
            ref_name: Some("main".to_string()),
            path: Some("src/lib.rs".to_string()),
            language: Some("rust".to_string()),
            line_start: Some(10),
            line_end: Some(25),
            ..Default::default()
        };

        let evidence = build_code_evidence(&code, None).unwrap();
        assert_eq!(evidence.host, Some(CodeHost::Github));
        assert_eq!(evidence.owner.as_deref(), Some("tokio-rs"));
        assert_eq!(evidence.repo.as_deref(), Some("axum"));
        assert_eq!(evidence.ref_name.as_deref(), Some("main"));
        assert_eq!(evidence.path.as_deref(), Some("src/lib.rs"));
        assert_eq!(evidence.language.as_deref(), Some("rust"));
        assert_eq!(evidence.source_role, Some(SourceRole::Implementation));
        assert_eq!(evidence.match_line_start, Some(10));
        assert_eq!(evidence.match_line_end, Some(25));
        assert_eq!(
            evidence.raw_url.as_deref(),
            Some("https://raw.githubusercontent.com/tokio-rs/axum/main/src/lib.rs")
        );
        assert!(evidence.browser_url.is_some());
        assert!(
            evidence
                .evidence_reasons
                .contains(&CodeEvidenceReason::RawUrlDerived)
        );
        assert!(
            evidence
                .evidence_reasons
                .contains(&CodeEvidenceReason::LanguageMatch)
        );
    }

    #[test]
    fn build_code_evidence_sparse_returns_none() {
        let code = CodeMetadata {
            host: Some(CodeHost::Github),
            owner: Some("tokio-rs".to_string()),
            repo: Some("axum".to_string()),
            ..Default::default()
        };

        // No path => returns None
        let result = build_code_evidence(&code, None);
        assert!(result.is_none());
    }

    #[test]
    fn build_code_evidence_no_owner_returns_none() {
        let code = CodeMetadata {
            host: Some(CodeHost::Github),
            repo: Some("axum".to_string()),
            path: Some("src/lib.rs".to_string()),
            ..Default::default()
        };

        let result = build_code_evidence(&code, None);
        assert!(result.is_none());
    }

    // --- Serialization roundtrip tests ---

    #[test]
    fn code_evidence_default_serializes_no_nulls() {
        let evidence = CodeEvidence::default();
        let json = serde_json::to_value(&evidence).unwrap();
        let obj = json.as_object().unwrap();
        // Default: all optional fields absent, evidence_reasons is empty vec
        assert!(!obj.contains_key("host"));
        assert!(!obj.contains_key("owner"));
        assert!(!obj.contains_key("evidence_reasons"));
    }

    #[test]
    fn code_evidence_populated_serializes_snake_case() {
        let evidence = CodeEvidence {
            host: Some(CodeHost::Github),
            source_role: Some(SourceRole::Implementation),
            evidence_confidence: Some(EvidenceConfidence::Strong),
            evidence_reasons: vec![CodeEvidenceReason::RawUrlDerived],
            ..Default::default()
        };
        let json = serde_json::to_value(&evidence).unwrap();
        assert_eq!(json["host"], "github");
        assert_eq!(json["source_role"], "implementation");
        assert_eq!(json["evidence_confidence"], "strong");
        assert_eq!(json["evidence_reasons"][0], "raw_url_derived");
    }

    #[test]
    fn code_evidence_roundtrip() {
        let evidence = CodeEvidence {
            host: Some(CodeHost::Gitlab),
            owner: Some("group".to_string()),
            repo: Some("project".to_string()),
            ref_name: Some("main".to_string()),
            path: Some("src/main.rs".to_string()),
            language: Some("rust".to_string()),
            source_role: Some(SourceRole::Implementation),
            evidence_confidence: Some(EvidenceConfidence::Exact),
            evidence_reasons: vec![
                CodeEvidenceReason::RawUrlDerived,
                CodeEvidenceReason::LanguageMatch,
            ],
            ..Default::default()
        };
        let json_str = serde_json::to_string(&evidence).unwrap();
        let deserialized: CodeEvidence = serde_json::from_str(&json_str).unwrap();
        assert_eq!(evidence, deserialized);
    }
}
