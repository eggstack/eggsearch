//! Query planner that builds intent-aware search queries.
//!
//! `build_search_plan` parses repo hints from the raw query, then
//! rewrites `generic_query` with platform-specific suffixes based on
//! the requested [`SearchIntent`]. The `provider_queries` map is
//! populated for future per-provider query overrides (e.g. a future
//! `github_code` provider) when the provider ID is in the selected set.

use std::collections::BTreeMap;

use crate::core::query::{SearchIntent, WebSearchRequest};
use crate::core::repo_query::RepoQueryHints;

/// The output of the query planner.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchPlan {
    /// The raw, unmodified query string from the request.
    pub original_query: String,
    /// The search intent from the request.
    pub intent: SearchIntent,
    /// The freshness hint from the request.
    pub freshness: crate::core::query::Freshness,
    /// Structured hints parsed from the original query.
    pub hints: RepoQueryHints,
    /// The rewritten query string to send to providers that do not
    /// have a provider-specific override.
    pub generic_query: String,
    /// Per-provider query overrides. Keys are provider IDs (e.g.
    /// `"duckduckgo"`, `"brave"`). Providers not listed here receive
    /// `generic_query`.
    pub provider_queries: BTreeMap<String, String>,
}

/// Build a [`SearchPlan`] from a [`WebSearchRequest`].
///
/// `selected_provider_ids` is the list of provider IDs that will be
/// queried. The planner uses this to populate `provider_queries` with
/// provider-specific query overrides for future repo-host providers
/// (e.g. `github_code`, `github_issues`).
///
/// The original query is preserved verbatim; only `generic_query` and
/// `provider_queries` are derived.
pub fn build_search_plan(req: &WebSearchRequest, selected_provider_ids: &[String]) -> SearchPlan {
    let hints = RepoQueryHints::parse(&req.query);

    let generic_query = build_generic_query(&req.query, req.intent, &hints);
    let provider_queries = build_provider_queries(&hints, req.intent, selected_provider_ids);

    SearchPlan {
        original_query: req.query.clone(),
        intent: req.intent,
        freshness: req.freshness,
        hints,
        generic_query,
        provider_queries,
    }
}

fn build_generic_query(query: &str, intent: SearchIntent, hints: &RepoQueryHints) -> String {
    match intent {
        SearchIntent::Web => {
            // Keep query as-is.
            query.trim().to_string()
        }
        SearchIntent::Docs => {
            // No major changes.
            query.trim().to_string()
        }
        SearchIntent::Code => build_code_generic_query(query, hints),
        SearchIntent::Issues => build_issues_generic_query(query, hints),
        SearchIntent::Releases => build_releases_generic_query(query, hints),
        SearchIntent::Security => {
            // No major repo behavior.
            query.trim().to_string()
        }
        SearchIntent::News => {
            // No repo behavior.
            query.trim().to_string()
        }
    }
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Push the residual query and owner/repo terms into `parts`.
fn push_residual_and_repo_terms(parts: &mut Vec<String>, _query: &str, hints: &RepoQueryHints) {
    let residual = hints.residual_query.trim();
    if !residual.is_empty() {
        parts.push(residual.to_string());
    }

    push_repo_scope(parts, hints);
}

/// Push owner/repo if present.
fn push_repo_scope(parts: &mut Vec<String>, hints: &RepoQueryHints) {
    if let (Some(owner), Some(repo)) = (&hints.owner, &hints.repo) {
        parts.push(format!("{owner}/{repo}"));
    }
}

/// Push org if present and no owner/repo was pushed.
fn push_org_scope(parts: &mut Vec<String>, hints: &RepoQueryHints) {
    if hints.owner.is_none() && hints.repo.is_none() {
        if let Some(org) = &hints.org {
            parts.push(org.clone());
        }
    }
}

/// Push a host keyword (e.g. "github", "gitlab") when a concrete
/// host is parsed. Only emits well-known single-word keywords.
fn push_host_term(parts: &mut Vec<String>, hints: &RepoQueryHints) {
    use crate::core::code_metadata::CodeHost;
    let keyword = match hints.host {
        Some(CodeHost::Github) => "github",
        Some(CodeHost::Gitlab) => "gitlab",
        Some(CodeHost::Codeberg) => "codeberg",
        _ => return,
    };
    // Avoid duplicating a host term that already appears in residual.
    let residual = hints.residual_query.to_lowercase();
    if !residual.contains(keyword) {
        parts.push(keyword.to_string());
    }
}

/// Push file hint if present.
fn push_file_hint(parts: &mut Vec<String>, hints: &RepoQueryHints) {
    if let Some(file) = &hints.file {
        // Avoid duplicating if already in residual.
        let residual = &hints.residual_query;
        if !residual.contains(file.as_str()) {
            parts.push(file.clone());
        }
    }
}

/// Push path hint if present.
fn push_path_hint(parts: &mut Vec<String>, hints: &RepoQueryHints) {
    if let Some(path) = &hints.path {
        let residual = &hints.residual_query;
        if !residual.contains(path.as_str()) {
            parts.push(path.clone());
        }
    }
}

/// Push language hint if present.
fn push_language_hint(parts: &mut Vec<String>, hints: &RepoQueryHints) {
    if let Some(lang) = &hints.language {
        let residual = &hints.residual_query.to_lowercase();
        if !residual.contains(lang.as_str()) {
            parts.push(lang.clone());
        }
    }
}

/// Push symbol hint if present.
fn push_symbol_hint(parts: &mut Vec<String>, hints: &RepoQueryHints) {
    if let Some(sym) = &hints.symbol {
        let residual = &hints.residual_query;
        if !residual.contains(sym.as_str()) {
            parts.push(sym.clone());
        }
    }
}

/// Remove empty strings and deduplicate terms across the whole query,
/// preserving order.
fn dedupe_terms(parts: Vec<String>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for part in parts {
        let trimmed = part.trim().to_string();
        if trimmed.is_empty() {
            continue;
        }
        if seen.insert(trimmed.clone()) {
            out.push(trimmed);
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Intent-specific generic query builders
// ---------------------------------------------------------------------------

fn build_code_generic_query(query: &str, hints: &RepoQueryHints) -> String {
    let mut parts = Vec::new();
    push_residual_and_repo_terms(&mut parts, query, hints);
    push_path_hint(&mut parts, hints);
    push_file_hint(&mut parts, hints);
    push_language_hint(&mut parts, hints);
    push_symbol_hint(&mut parts, hints);
    push_host_term(&mut parts, hints);

    if parts.is_empty() {
        parts.push(query.trim().to_string());
    }

    let mut result = dedupe_terms(parts).join(" ");
    result.push_str(" github gitlab codeberg source repository");
    result
}

fn build_issues_generic_query(query: &str, hints: &RepoQueryHints) -> String {
    let mut parts = Vec::new();
    push_residual_and_repo_terms(&mut parts, query, hints);
    push_org_scope(&mut parts, hints);
    push_symbol_hint(&mut parts, hints);
    push_host_term(&mut parts, hints);

    if parts.is_empty() {
        parts.push(query.trim().to_string());
    }

    let mut result = dedupe_terms(parts).join(" ");
    result.push_str(" issues discussions pull request github gitlab");
    result
}

fn build_releases_generic_query(query: &str, hints: &RepoQueryHints) -> String {
    let mut parts = Vec::new();
    push_residual_and_repo_terms(&mut parts, query, hints);
    push_org_scope(&mut parts, hints);
    push_file_hint(&mut parts, hints);
    push_path_hint(&mut parts, hints);
    push_host_term(&mut parts, hints);

    if parts.is_empty() {
        parts.push(query.trim().to_string());
    }

    let mut result = dedupe_terms(parts).join(" ");
    result.push_str(" releases changelog migration tag github gitlab");
    result
}

// ---------------------------------------------------------------------------
// Provider-specific query generation
// ---------------------------------------------------------------------------

/// Known future repo-host provider IDs that may receive provider-specific
/// query overrides. Providers not in this list receive `generic_query`.
const FUTURE_REPO_PROVIDER_IDS: &[&str] = &[
    "github_code",
    "github_issues",
    "github_releases",
    "gitlab_code",
    "gitlab_issues",
    "gitlab_releases",
    "codeberg_code",
    "gitea_code",
    "gitea_issues",
    "gitea_releases",
];

/// Build per-provider query overrides for future repo-host providers.
///
/// Only providers that appear in `selected_provider_ids` AND are in
/// `FUTURE_REPO_PROVIDER_IDS` get an entry. All other providers
/// receive `generic_query` (the caller handles the fallback).
fn build_provider_queries(
    hints: &RepoQueryHints,
    intent: SearchIntent,
    selected_provider_ids: &[String],
) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();

    for id in selected_provider_ids {
        if !FUTURE_REPO_PROVIDER_IDS.contains(&id.as_str()) {
            continue;
        }
        if let Some(query) = build_provider_specific_query(hints, intent, id) {
            map.insert(id.clone(), query);
        }
    }

    map
}

/// Build a provider-specific query for a known future repo provider.
///
/// Returns `None` when the provider ID is not recognized or when
/// the intent does not match the provider's domain.
fn build_provider_specific_query(
    hints: &RepoQueryHints,
    intent: SearchIntent,
    provider_id: &str,
) -> Option<String> {
    match provider_id {
        "github_code" if intent == SearchIntent::Code => Some(build_github_code_query(hints)),
        "github_issues" if intent == SearchIntent::Issues => Some(build_github_issues_query(hints)),
        "github_releases" if intent == SearchIntent::Releases => {
            Some(build_github_releases_query(hints))
        }
        "gitlab_code" if intent == SearchIntent::Code => Some(build_gitlab_code_query(hints)),
        "gitlab_issues" if intent == SearchIntent::Issues => Some(build_gitlab_issues_query(hints)),
        "gitlab_releases" if intent == SearchIntent::Releases => {
            Some(build_gitlab_releases_query(hints))
        }
        "codeberg_code" if intent == SearchIntent::Code => Some(build_codeberg_code_query(hints)),
        "gitea_code" if intent == SearchIntent::Code => Some(build_gitea_code_query(hints)),
        "gitea_issues" if intent == SearchIntent::Issues => Some(build_gitea_issues_query(hints)),
        "gitea_releases" if intent == SearchIntent::Releases => {
            Some(build_gitea_releases_query(hints))
        }
        _ => None,
    }
}

// --- GitHub provider queries ---

fn build_github_code_query(hints: &RepoQueryHints) -> String {
    let mut parts = Vec::new();
    if let Some(sym) = &hints.symbol {
        parts.push(sym.clone());
    }
    if let Some(path) = &hints.path {
        parts.push(format!("path:{path}"));
    }
    if let Some(file) = &hints.file {
        parts.push(format!("filename:{file}"));
    }
    if let Some(lang) = &hints.language {
        parts.push(format!("language:{lang}"));
    }
    if let (Some(owner), Some(repo)) = (&hints.owner, &hints.repo) {
        parts.push(format!("repo:{owner}/{repo}"));
    } else if let Some(org) = &hints.org {
        parts.push(format!("org:{org}"));
    }
    let residual = hints.residual_query.trim();
    if !residual.is_empty() {
        parts.push(residual.to_string());
    }
    if parts.is_empty() {
        return String::new();
    }
    dedupe_terms(parts).join(" ")
}

fn build_github_issues_query(hints: &RepoQueryHints) -> String {
    let mut parts = Vec::new();
    let residual = hints.residual_query.trim();
    if !residual.is_empty() {
        parts.push(residual.to_string());
    }
    if let Some(sym) = &hints.symbol {
        parts.push(sym.clone());
    }
    if let (Some(owner), Some(repo)) = (&hints.owner, &hints.repo) {
        parts.push(format!("repo:{owner}/{repo}"));
    } else if let Some(org) = &hints.org {
        parts.push(format!("org:{org}"));
    }
    parts.push("is:issue".to_string());
    if parts.is_empty() {
        return String::new();
    }
    dedupe_terms(parts).join(" ")
}

fn build_github_releases_query(hints: &RepoQueryHints) -> String {
    let mut parts = Vec::new();
    let residual = hints.residual_query.trim();
    if !residual.is_empty() {
        parts.push(residual.to_string());
    }
    if let (Some(owner), Some(repo)) = (&hints.owner, &hints.repo) {
        parts.push(format!("repo:{owner}/{repo}"));
    } else if let Some(org) = &hints.org {
        parts.push(format!("org:{org}"));
    }
    if let Some(file) = &hints.file {
        parts.push(file.clone());
    }
    if let Some(path) = &hints.path {
        parts.push(path.clone());
    }
    parts.push("release".to_string());
    parts.push("changelog".to_string());
    if parts.is_empty() {
        return String::new();
    }
    dedupe_terms(parts).join(" ")
}

// --- GitLab provider queries (provisional syntax) ---

fn build_gitlab_code_query(hints: &RepoQueryHints) -> String {
    let mut parts = Vec::new();
    if let Some(sym) = &hints.symbol {
        parts.push(sym.clone());
    }
    if let Some(path) = &hints.path {
        parts.push(path.clone());
    }
    if let Some(file) = &hints.file {
        parts.push(file.clone());
    }
    if let Some(lang) = &hints.language {
        parts.push(lang.clone());
    }
    if let (Some(owner), Some(repo)) = (&hints.owner, &hints.repo) {
        // GitLab does not use repo: operator; include as visible text.
        parts.push(format!("{owner}/{repo}"));
    } else if let Some(org) = &hints.org {
        parts.push(org.clone());
    }
    let residual = hints.residual_query.trim();
    if !residual.is_empty() {
        parts.push(residual.to_string());
    }
    if parts.is_empty() {
        return String::new();
    }
    dedupe_terms(parts).join(" ")
}

fn build_gitlab_issues_query(hints: &RepoQueryHints) -> String {
    let mut parts = Vec::new();
    let residual = hints.residual_query.trim();
    if !residual.is_empty() {
        parts.push(residual.to_string());
    }
    if let Some(sym) = &hints.symbol {
        parts.push(sym.clone());
    }
    if let (Some(owner), Some(repo)) = (&hints.owner, &hints.repo) {
        parts.push(format!("{owner}/{repo}"));
    } else if let Some(org) = &hints.org {
        parts.push(org.clone());
    }
    parts.push("issues".to_string());
    if parts.is_empty() {
        return String::new();
    }
    dedupe_terms(parts).join(" ")
}

fn build_gitlab_releases_query(hints: &RepoQueryHints) -> String {
    let mut parts = Vec::new();
    let residual = hints.residual_query.trim();
    if !residual.is_empty() {
        parts.push(residual.to_string());
    }
    if let (Some(owner), Some(repo)) = (&hints.owner, &hints.repo) {
        parts.push(format!("{owner}/{repo}"));
    } else if let Some(org) = &hints.org {
        parts.push(org.clone());
    }
    if let Some(file) = &hints.file {
        parts.push(file.clone());
    }
    parts.push("releases".to_string());
    if parts.is_empty() {
        return String::new();
    }
    dedupe_terms(parts).join(" ")
}

// --- Codeberg provider queries ---

fn build_codeberg_code_query(hints: &RepoQueryHints) -> String {
    let mut parts = Vec::new();
    if let Some(sym) = &hints.symbol {
        parts.push(sym.clone());
    }
    if let Some(path) = &hints.path {
        parts.push(path.clone());
    }
    if let Some(file) = &hints.file {
        parts.push(file.clone());
    }
    if let Some(lang) = &hints.language {
        parts.push(lang.clone());
    }
    if let (Some(owner), Some(repo)) = (&hints.owner, &hints.repo) {
        parts.push(format!("{owner}/{repo}"));
    } else if let Some(org) = &hints.org {
        parts.push(org.clone());
    }
    let residual = hints.residual_query.trim();
    if !residual.is_empty() {
        parts.push(residual.to_string());
    }
    if parts.is_empty() {
        return String::new();
    }
    dedupe_terms(parts).join(" ")
}

// --- Gitea/Forgejo provider queries ---

fn build_gitea_code_query(hints: &RepoQueryHints) -> String {
    let mut parts = Vec::new();
    if let Some(sym) = &hints.symbol {
        parts.push(sym.clone());
    }
    if let Some(path) = &hints.path {
        parts.push(path.clone());
    }
    if let Some(file) = &hints.file {
        parts.push(file.clone());
    }
    if let Some(lang) = &hints.language {
        parts.push(lang.clone());
    }
    if let (Some(owner), Some(repo)) = (&hints.owner, &hints.repo) {
        parts.push(format!("{owner}/{repo}"));
    } else if let Some(org) = &hints.org {
        parts.push(org.clone());
    }
    let residual = hints.residual_query.trim();
    if !residual.is_empty() {
        parts.push(residual.to_string());
    }
    if parts.is_empty() {
        return String::new();
    }
    dedupe_terms(parts).join(" ")
}

fn build_gitea_issues_query(hints: &RepoQueryHints) -> String {
    let mut parts = Vec::new();
    let residual = hints.residual_query.trim();
    if !residual.is_empty() {
        parts.push(residual.to_string());
    }
    if let Some(sym) = &hints.symbol {
        parts.push(sym.clone());
    }
    if let (Some(owner), Some(repo)) = (&hints.owner, &hints.repo) {
        parts.push(format!("{owner}/{repo}"));
    } else if let Some(org) = &hints.org {
        parts.push(org.clone());
    }
    parts.push("issues".to_string());
    if parts.is_empty() {
        return String::new();
    }
    dedupe_terms(parts).join(" ")
}

fn build_gitea_releases_query(hints: &RepoQueryHints) -> String {
    let mut parts = Vec::new();
    let residual = hints.residual_query.trim();
    if !residual.is_empty() {
        parts.push(residual.to_string());
    }
    if let (Some(owner), Some(repo)) = (&hints.owner, &hints.repo) {
        parts.push(format!("{owner}/{repo}"));
    } else if let Some(org) = &hints.org {
        parts.push(org.clone());
    }
    if let Some(file) = &hints.file {
        parts.push(file.clone());
    }
    parts.push("releases".to_string());
    if parts.is_empty() {
        return String::new();
    }
    dedupe_terms(parts).join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(query: &str, intent: SearchIntent) -> WebSearchRequest {
        let mut r = WebSearchRequest::new(query);
        r.intent = intent;
        r
    }

    // --- Web intent ---

    #[test]
    fn web_intent_passthrough() {
        let plan = build_search_plan(&req("rust axum middleware", SearchIntent::Web), &[]);
        assert_eq!(plan.generic_query, "rust axum middleware");
    }

    #[test]
    fn web_intent_trims_whitespace() {
        let plan = build_search_plan(&req("  rust axum  ", SearchIntent::Web), &[]);
        assert_eq!(plan.generic_query, "rust axum");
    }

    #[test]
    fn web_intent_preserves_original() {
        let plan = build_search_plan(&req("  rust axum  ", SearchIntent::Web), &[]);
        assert_eq!(plan.original_query, "  rust axum  ");
    }

    // --- Docs intent ---

    #[test]
    fn docs_intent_passthrough() {
        let plan = build_search_plan(&req("axum middleware docs", SearchIntent::Docs), &[]);
        assert_eq!(plan.generic_query, "axum middleware docs");
    }

    // --- Code intent ---

    #[test]
    fn code_intent_with_repo_hints() {
        let plan = build_search_plan(
            &req("repo:tokio-rs/axum Router::layer", SearchIntent::Code),
            &[],
        );
        assert!(plan.generic_query.contains("tokio-rs/axum"));
        assert!(plan.generic_query.contains("Router::layer"));
        assert!(plan
            .generic_query
            .contains("github gitlab codeberg source repository"));
    }

    #[test]
    fn code_intent_without_repo_hints() {
        let plan = build_search_plan(&req("rust axum middleware", SearchIntent::Code), &[]);
        assert_eq!(
            plan.generic_query,
            "rust axum middleware github gitlab codeberg source repository"
        );
    }

    #[test]
    fn code_intent_bare_owner_repo() {
        let plan = build_search_plan(&req("tokio-rs/axum Router::layer", SearchIntent::Code), &[]);
        assert!(plan.generic_query.contains("tokio-rs/axum"));
        assert!(plan.generic_query.contains("Router::layer"));
        assert!(plan
            .generic_query
            .contains("github gitlab codeberg source repository"));
    }

    #[test]
    fn code_intent_residual_only() {
        let plan = build_search_plan(&req("repo:tokio-rs/axum", SearchIntent::Code), &[]);
        assert!(plan.generic_query.contains("tokio-rs/axum"));
        assert!(plan
            .generic_query
            .contains("github gitlab codeberg source repository"));
    }

    #[test]
    fn code_intent_file_hint() {
        let plan = build_search_plan(
            &req("repo:tokio-rs/axum file:Cargo.toml", SearchIntent::Code),
            &[],
        );
        assert!(plan.generic_query.contains("tokio-rs/axum"));
        assert!(plan.generic_query.contains("Cargo.toml"));
        assert!(plan
            .generic_query
            .contains("github gitlab codeberg source repository"));
    }

    // --- Issues intent ---

    #[test]
    fn issues_intent_with_repo_hints() {
        let plan = build_search_plan(
            &req("repo:tokio-rs/axum Router::layer", SearchIntent::Issues),
            &[],
        );
        assert!(plan.generic_query.contains("tokio-rs/axum"));
        assert!(plan.generic_query.contains("Router::layer"));
        assert!(plan
            .generic_query
            .contains("issues discussions pull request github gitlab"));
    }

    #[test]
    fn issues_intent_without_repo_hints() {
        let plan = build_search_plan(&req("rust axum middleware", SearchIntent::Issues), &[]);
        assert_eq!(
            plan.generic_query,
            "rust axum middleware issues discussions pull request github gitlab"
        );
    }

    // --- Releases intent ---

    #[test]
    fn releases_intent_with_repo_hints() {
        let plan = build_search_plan(&req("repo:tokio-rs/axum", SearchIntent::Releases), &[]);
        assert!(plan.generic_query.contains("tokio-rs/axum"));
        assert!(plan
            .generic_query
            .contains("releases changelog migration tag github gitlab"));
    }

    #[test]
    fn releases_intent_without_repo_hints() {
        let plan = build_search_plan(&req("rust axum middleware", SearchIntent::Releases), &[]);
        assert_eq!(
            plan.generic_query,
            "rust axum middleware releases changelog migration tag github gitlab"
        );
    }

    // --- Security intent ---

    #[test]
    fn security_intent_passthrough() {
        let plan = build_search_plan(&req("axum CVE", SearchIntent::Security), &[]);
        assert_eq!(plan.generic_query, "axum CVE");
    }

    // --- News intent ---

    #[test]
    fn news_intent_passthrough() {
        let plan = build_search_plan(&req("rust axum release", SearchIntent::News), &[]);
        assert_eq!(plan.generic_query, "rust axum release");
    }

    // --- Plan struct fields ---

    #[test]
    fn plan_preserves_intent_and_freshness() {
        let mut r = WebSearchRequest::new("test");
        r.intent = SearchIntent::Code;
        r.freshness = crate::core::query::Freshness::Week;
        let plan = build_search_plan(&r, &[]);
        assert_eq!(plan.intent, SearchIntent::Code);
        assert_eq!(plan.freshness, crate::core::query::Freshness::Week);
    }

    #[test]
    fn plan_hints_populated() {
        let plan = build_search_plan(&req("repo:tokio-rs/axum test", SearchIntent::Code), &[]);
        assert!(plan.hints.has_any());
        assert_eq!(plan.hints.owner.as_deref(), Some("tokio-rs"));
        assert_eq!(plan.hints.repo.as_deref(), Some("axum"));
    }

    #[test]
    fn plan_provider_queries_empty_by_default() {
        let plan = build_search_plan(&req("test", SearchIntent::Code), &[]);
        assert!(plan.provider_queries.is_empty());
    }

    // --- Edge cases ---

    #[test]
    fn code_intent_with_language_hint() {
        let plan = build_search_plan(
            &req("lang:rust repo:tokio-rs/axum", SearchIntent::Code),
            &[],
        );
        assert!(plan.generic_query.contains("tokio-rs/axum"));
        assert!(plan.generic_query.contains("rust"));
        assert!(plan
            .generic_query
            .contains("github gitlab codeberg source repository"));
    }

    #[test]
    fn code_intent_with_symbol_hint() {
        let plan = build_search_plan(
            &req(
                "symbol:Router::layer repo:tokio-rs/axum",
                SearchIntent::Code,
            ),
            &[],
        );
        assert!(plan.generic_query.contains("tokio-rs/axum"));
        assert!(plan.generic_query.contains("Router::layer"));
        assert!(plan
            .generic_query
            .contains("github gitlab codeberg source repository"));
    }

    #[test]
    fn code_intent_with_host_hint() {
        let plan = build_search_plan(
            &req("host:gitlab repo:tokio-rs/axum", SearchIntent::Code),
            &[],
        );
        assert!(plan.generic_query.contains("tokio-rs/axum"));
        assert!(plan.generic_query.contains("gitlab"));
        assert!(plan
            .generic_query
            .contains("github gitlab codeberg source repository"));
    }

    #[test]
    fn issues_intent_with_org_hint() {
        let plan = build_search_plan(&req("org:rust-lang", SearchIntent::Issues), &[]);
        assert!(plan.generic_query.contains("rust-lang"));
        assert!(plan
            .generic_query
            .contains("issues discussions pull request github gitlab"));
    }

    #[test]
    fn releases_intent_residual_with_file() {
        let plan = build_search_plan(
            &req(
                "repo:tokio-rs/axum file:CHANGELOG.md",
                SearchIntent::Releases,
            ),
            &[],
        );
        assert!(plan.generic_query.contains("tokio-rs/axum"));
        assert!(plan.generic_query.contains("CHANGELOG.md"));
        assert!(plan.hints.file.as_deref() == Some("CHANGELOG.md"));
        assert!(plan
            .generic_query
            .contains("releases changelog migration tag github gitlab"));
    }

    #[test]
    fn empty_residual_and_no_repo_uses_original() {
        let plan = build_search_plan(&req("repo:invalid", SearchIntent::Code), &[]);
        assert!(plan.generic_query.contains("repo:invalid"));
        assert!(plan
            .generic_query
            .contains("github gitlab codeberg source repository"));
    }

    // --- Issue 2: Hint inclusion in generic queries ---

    #[test]
    fn code_intent_file_hint_included_in_generic_query() {
        let plan = build_search_plan(
            &req("repo:tokio-rs/axum file:Cargo.toml", SearchIntent::Code),
            &[],
        );
        assert!(plan.generic_query.contains("tokio-rs/axum"));
        assert!(plan.generic_query.contains("Cargo.toml"));
        let count = plan.generic_query.matches("Cargo.toml").count();
        assert_eq!(count, 1, "Cargo.toml should appear exactly once");
    }

    #[test]
    fn code_intent_path_symbol_language_included_in_generic_query() {
        let plan = build_search_plan(
            &req(
                "repo:tokio-rs/axum path:axum/src/routing/mod.rs symbol:Router::layer lang:rust",
                SearchIntent::Code,
            ),
            &[],
        );
        assert!(plan.generic_query.contains("tokio-rs/axum"));
        assert!(plan.generic_query.contains("axum/src/routing/mod.rs"));
        assert!(plan.generic_query.contains("Router::layer"));
        assert!(plan.generic_query.contains("rust"));
    }

    #[test]
    fn code_intent_host_hint_included_in_generic_query() {
        let plan = build_search_plan(
            &req("host:github repo:tokio-rs/axum", SearchIntent::Code),
            &[],
        );
        assert!(plan.generic_query.contains("github"));
    }

    #[test]
    fn issues_intent_org_hint_included_without_raw_org_prefix() {
        let plan = build_search_plan(
            &req("org:rust-lang borrow checker", SearchIntent::Issues),
            &[],
        );
        assert!(plan.generic_query.contains("rust-lang"));
        assert!(!plan.generic_query.contains("org:rust-lang"));
        assert!(plan.generic_query.contains("borrow"));
        assert!(plan.generic_query.contains("checker"));
    }

    #[test]
    fn releases_intent_changelog_file_included() {
        let plan = build_search_plan(
            &req(
                "repo:tokio-rs/axum file:CHANGELOG.md",
                SearchIntent::Releases,
            ),
            &[],
        );
        assert!(plan.generic_query.contains("tokio-rs/axum"));
        assert!(plan.generic_query.contains("CHANGELOG.md"));
    }

    #[test]
    fn generic_query_dedupes_terms() {
        let plan = build_search_plan(
            &req("github tokio-rs/axum host:github", SearchIntent::Code),
            &[],
        );
        let count = plan.generic_query.matches("github").count();
        assert!(
            count <= 3,
            "github should not be duplicated by host hint: {}",
            plan.generic_query
        );
    }

    #[test]
    fn generic_query_never_empty_when_all_hints() {
        let plan = build_search_plan(
            &req(
                "repo:tokio-rs/axum file:Cargo.toml lang:rust",
                SearchIntent::Code,
            ),
            &[],
        );
        assert!(!plan.generic_query.is_empty());
        assert!(plan.generic_query.contains("tokio-rs/axum"));
    }

    // --- Issue 3 & 4: Provider-specific queries ---

    #[test]
    fn github_code_provider_query_generated_when_selected() {
        let plan = build_search_plan(
            &req("repo:tokio-rs/axum file:Cargo.toml", SearchIntent::Code),
            &["github_code".to_string()],
        );
        let q = plan
            .provider_queries
            .get("github_code")
            .expect("github_code query");
        assert!(q.contains("repo:tokio-rs/axum"));
        assert!(q.contains("filename:Cargo.toml"));
    }

    #[test]
    fn github_code_provider_query_omitted_when_not_selected() {
        let plan = build_search_plan(
            &req("repo:tokio-rs/axum", SearchIntent::Code),
            &["duckduckgo".to_string()],
        );
        assert!(plan.provider_queries.is_empty());
    }

    #[test]
    fn github_issues_provider_query_generated_for_issues_intent() {
        let plan = build_search_plan(
            &req("repo:tokio-rs/axum panic", SearchIntent::Issues),
            &["github_issues".to_string()],
        );
        let q = plan
            .provider_queries
            .get("github_issues")
            .expect("github_issues query");
        assert!(q.contains("repo:tokio-rs/axum"));
        assert!(q.contains("is:issue"));
        assert!(q.contains("panic"));
    }

    #[test]
    fn github_releases_provider_query_generated_for_releases_intent() {
        let plan = build_search_plan(
            &req("repo:tokio-rs/axum", SearchIntent::Releases),
            &["github_releases".to_string()],
        );
        let q = plan
            .provider_queries
            .get("github_releases")
            .expect("github_releases query");
        assert!(q.contains("repo:tokio-rs/axum"));
        assert!(q.contains("release"));
        assert!(q.contains("changelog"));
    }

    #[test]
    fn unknown_provider_uses_generic_query() {
        let plan = build_search_plan(
            &req("repo:tokio-rs/axum", SearchIntent::Code),
            &["unknown_provider".to_string()],
        );
        assert!(plan.provider_queries.is_empty());
    }

    #[test]
    fn github_code_provider_query_with_symbol_and_language() {
        let plan = build_search_plan(
            &req(
                "repo:tokio-rs/axum symbol:Router::layer lang:rust",
                SearchIntent::Code,
            ),
            &["github_code".to_string()],
        );
        let q = plan
            .provider_queries
            .get("github_code")
            .expect("github_code query");
        assert!(q.contains("repo:tokio-rs/axum"));
        assert!(q.contains("Router::layer"));
        assert!(q.contains("language:rust"));
    }

    #[test]
    fn gitlab_code_provider_uses_visible_text_not_operators() {
        let plan = build_search_plan(
            &req("repo:tokio-rs/axum lang:rust", SearchIntent::Code),
            &["gitlab_code".to_string()],
        );
        let q = plan
            .provider_queries
            .get("gitlab_code")
            .expect("gitlab_code query");
        assert!(!q.contains("repo:"));
        assert!(q.contains("tokio-rs/axum"));
        assert!(q.contains("rust"));
    }

    #[test]
    fn codeberg_code_provider_query_generated() {
        let plan = build_search_plan(
            &req("repo:tokio-rs/axum", SearchIntent::Code),
            &["codeberg_code".to_string()],
        );
        let q = plan
            .provider_queries
            .get("codeberg_code")
            .expect("codeberg_code query");
        assert!(q.contains("tokio-rs/axum"));
    }

    #[test]
    fn provider_query_empty_for_mismatched_intent() {
        let plan = build_search_plan(
            &req("repo:tokio-rs/axum", SearchIntent::Code),
            &["github_issues".to_string()],
        );
        assert!(plan.provider_queries.is_empty());
    }

    #[test]
    fn provider_query_with_org_and_no_repo() {
        let plan = build_search_plan(
            &req("org:rust-lang borrow checker", SearchIntent::Issues),
            &["github_issues".to_string()],
        );
        let q = plan
            .provider_queries
            .get("github_issues")
            .expect("github_issues query");
        assert!(q.contains("org:rust-lang"));
        assert!(q.contains("borrow"));
        assert!(q.contains("checker"));
    }

    // --- Phase 3: github_code query syntax ---

    #[test]
    fn github_code_file_hint_uses_filename_syntax() {
        let plan = build_search_plan(
            &req("repo:tokio-rs/axum file:Cargo.toml", SearchIntent::Code),
            &["github_code".to_string()],
        );
        let q = plan
            .provider_queries
            .get("github_code")
            .expect("github_code query");
        assert!(q.contains("filename:Cargo.toml"));
        assert!(
            !q.contains("path:Cargo.toml"),
            "file hint should use filename: not path:"
        );
    }

    #[test]
    fn github_code_path_hint_uses_path_syntax() {
        let plan = build_search_plan(
            &req("repo:tokio-rs/axum path:src/routing", SearchIntent::Code),
            &["github_code".to_string()],
        );
        let q = plan
            .provider_queries
            .get("github_code")
            .expect("github_code query");
        assert!(q.contains("path:src/routing"));
    }

    #[test]
    fn github_code_symbol_remains_free_text() {
        let plan = build_search_plan(
            &req(
                "repo:tokio-rs/axum symbol:Router::layer",
                SearchIntent::Code,
            ),
            &["github_code".to_string()],
        );
        let q = plan
            .provider_queries
            .get("github_code")
            .expect("github_code query");
        assert!(q.contains("Router::layer"));
        assert!(!q.contains("symbol:"));
    }

    #[test]
    fn dedupe_terms_removes_nonconsecutive_duplicates() {
        let parts = vec![
            "github".to_string(),
            "tokio-rs/axum".to_string(),
            "github".to_string(),
            "source".to_string(),
            "tokio-rs/axum".to_string(),
        ];
        let result = dedupe_terms(parts);
        assert_eq!(result, vec!["github", "tokio-rs/axum", "source"]);
    }
}
