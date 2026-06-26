//! Query planner that builds intent-aware search queries.
//!
//! `build_search_plan` parses repo hints from the raw query, then
//! rewrites `generic_query` with platform-specific suffixes based on
//! the requested [`SearchIntent`]. The `provider_queries` map is
//! reserved for future per-provider query overrides (e.g. a future
//! `github_code` provider).

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
/// The original query is preserved verbatim; only `generic_query` and
/// `provider_queries` are derived.
pub fn build_search_plan(req: &WebSearchRequest) -> SearchPlan {
    let hints = RepoQueryHints::parse(&req.query);

    let generic_query = build_generic_query(&req.query, req.intent, &hints);
    let provider_queries = BTreeMap::new();

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
        SearchIntent::Code => {
            build_repo_query(query, hints, "github gitlab codeberg source repository")
        }
        SearchIntent::Issues => build_repo_query(
            query,
            hints,
            "issues discussions pull request github gitlab",
        ),
        SearchIntent::Releases => build_repo_query(
            query,
            hints,
            "releases changelog migration tag github gitlab",
        ),
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

/// Build an intent-aware query string.
///
/// When repo hints are present the residual terms and `owner/repo`
/// fragment are preserved, then the platform suffix is appended. When
/// no repo hints are present the full original query is used.
fn build_repo_query(query: &str, hints: &RepoQueryHints, suffix: &str) -> String {
    let mut parts = Vec::new();

    let residual = hints.residual_query.trim();
    if !residual.is_empty() {
        parts.push(residual.to_string());
    }

    if let (Some(owner), Some(repo)) = (&hints.owner, &hints.repo) {
        parts.push(format!("{owner}/{repo}"));
    }

    // If neither residual nor owner/repo contributed anything, fall
    // back to the original query so the result is never empty.
    if parts.is_empty() {
        parts.push(query.trim().to_string());
    }

    let mut result = parts.join(" ");
    result.push(' ');
    result.push_str(suffix);
    result
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
        let plan = build_search_plan(&req("rust axum middleware", SearchIntent::Web));
        assert_eq!(plan.generic_query, "rust axum middleware");
    }

    #[test]
    fn web_intent_trims_whitespace() {
        let plan = build_search_plan(&req("  rust axum  ", SearchIntent::Web));
        assert_eq!(plan.generic_query, "rust axum");
    }

    #[test]
    fn web_intent_preserves_original() {
        let plan = build_search_plan(&req("  rust axum  ", SearchIntent::Web));
        assert_eq!(plan.original_query, "  rust axum  ");
    }

    // --- Docs intent ---

    #[test]
    fn docs_intent_passthrough() {
        let plan = build_search_plan(&req("axum middleware docs", SearchIntent::Docs));
        assert_eq!(plan.generic_query, "axum middleware docs");
    }

    // --- Code intent ---

    #[test]
    fn code_intent_with_repo_hints() {
        let plan = build_search_plan(&req("repo:tokio-rs/axum Router::layer", SearchIntent::Code));
        assert!(plan.generic_query.contains("tokio-rs/axum"));
        assert!(plan.generic_query.contains("Router::layer"));
        assert!(plan
            .generic_query
            .contains("github gitlab codeberg source repository"));
    }

    #[test]
    fn code_intent_without_repo_hints() {
        let plan = build_search_plan(&req("rust axum middleware", SearchIntent::Code));
        assert_eq!(
            plan.generic_query,
            "rust axum middleware github gitlab codeberg source repository"
        );
    }

    #[test]
    fn code_intent_bare_owner_repo() {
        let plan = build_search_plan(&req("tokio-rs/axum Router::layer", SearchIntent::Code));
        assert!(plan.generic_query.contains("tokio-rs/axum"));
        assert!(plan.generic_query.contains("Router::layer"));
        assert!(plan
            .generic_query
            .contains("github gitlab codeberg source repository"));
    }

    #[test]
    fn code_intent_residual_only() {
        let plan = build_search_plan(&req("repo:tokio-rs/axum", SearchIntent::Code));
        assert!(plan.generic_query.contains("tokio-rs/axum"));
        assert!(plan
            .generic_query
            .contains("github gitlab codeberg source repository"));
    }

    #[test]
    fn code_intent_file_hint() {
        let plan = build_search_plan(&req(
            "repo:tokio-rs/axum file:Cargo.toml",
            SearchIntent::Code,
        ));
        assert!(plan.generic_query.contains("tokio-rs/axum"));
        assert!(plan
            .generic_query
            .contains("github gitlab codeberg source repository"));
    }

    // --- Issues intent ---

    #[test]
    fn issues_intent_with_repo_hints() {
        let plan = build_search_plan(&req(
            "repo:tokio-rs/axum Router::layer",
            SearchIntent::Issues,
        ));
        assert!(plan.generic_query.contains("tokio-rs/axum"));
        assert!(plan.generic_query.contains("Router::layer"));
        assert!(plan
            .generic_query
            .contains("issues discussions pull request github gitlab"));
    }

    #[test]
    fn issues_intent_without_repo_hints() {
        let plan = build_search_plan(&req("rust axum middleware", SearchIntent::Issues));
        assert_eq!(
            plan.generic_query,
            "rust axum middleware issues discussions pull request github gitlab"
        );
    }

    // --- Releases intent ---

    #[test]
    fn releases_intent_with_repo_hints() {
        let plan = build_search_plan(&req("repo:tokio-rs/axum", SearchIntent::Releases));
        assert!(plan.generic_query.contains("tokio-rs/axum"));
        assert!(plan
            .generic_query
            .contains("releases changelog migration tag github gitlab"));
    }

    #[test]
    fn releases_intent_without_repo_hints() {
        let plan = build_search_plan(&req("rust axum middleware", SearchIntent::Releases));
        assert_eq!(
            plan.generic_query,
            "rust axum middleware releases changelog migration tag github gitlab"
        );
    }

    // --- Security intent ---

    #[test]
    fn security_intent_passthrough() {
        let plan = build_search_plan(&req("axum CVE", SearchIntent::Security));
        assert_eq!(plan.generic_query, "axum CVE");
    }

    // --- News intent ---

    #[test]
    fn news_intent_passthrough() {
        let plan = build_search_plan(&req("rust axum release", SearchIntent::News));
        assert_eq!(plan.generic_query, "rust axum release");
    }

    // --- Plan struct fields ---

    #[test]
    fn plan_preserves_intent_and_freshness() {
        let mut r = WebSearchRequest::new("test");
        r.intent = SearchIntent::Code;
        r.freshness = crate::core::query::Freshness::Week;
        let plan = build_search_plan(&r);
        assert_eq!(plan.intent, SearchIntent::Code);
        assert_eq!(plan.freshness, crate::core::query::Freshness::Week);
    }

    #[test]
    fn plan_hints_populated() {
        let plan = build_search_plan(&req("repo:tokio-rs/axum test", SearchIntent::Code));
        assert!(plan.hints.has_any());
        assert_eq!(plan.hints.owner.as_deref(), Some("tokio-rs"));
        assert_eq!(plan.hints.repo.as_deref(), Some("axum"));
    }

    #[test]
    fn plan_provider_queries_empty_by_default() {
        let plan = build_search_plan(&req("test", SearchIntent::Code));
        assert!(plan.provider_queries.is_empty());
    }

    // --- Edge cases ---

    #[test]
    fn code_intent_with_language_hint() {
        let plan = build_search_plan(&req("lang:rust repo:tokio-rs/axum", SearchIntent::Code));
        assert!(plan.generic_query.contains("tokio-rs/axum"));
        assert!(plan
            .generic_query
            .contains("github gitlab codeberg source repository"));
    }

    #[test]
    fn code_intent_with_symbol_hint() {
        let plan = build_search_plan(&req(
            "symbol:Router::layer repo:tokio-rs/axum",
            SearchIntent::Code,
        ));
        assert!(plan.generic_query.contains("tokio-rs/axum"));
        assert!(plan
            .generic_query
            .contains("github gitlab codeberg source repository"));
    }

    #[test]
    fn code_intent_with_host_hint() {
        let plan = build_search_plan(&req("host:gitlab repo:tokio-rs/axum", SearchIntent::Code));
        assert!(plan.generic_query.contains("tokio-rs/axum"));
        assert!(plan
            .generic_query
            .contains("github gitlab codeberg source repository"));
    }

    #[test]
    fn issues_intent_with_org_hint() {
        let plan = build_search_plan(&req("org:rust-lang", SearchIntent::Issues));
        // org:hint doesn't produce owner/repo, so it falls back to
        // the full original query.
        assert!(plan.generic_query.contains("org:rust-lang"));
        assert!(plan
            .generic_query
            .contains("issues discussions pull request github gitlab"));
    }

    #[test]
    fn releases_intent_residual_with_file() {
        let plan = build_search_plan(&req(
            "repo:tokio-rs/axum file:CHANGELOG.md",
            SearchIntent::Releases,
        ));
        assert!(plan.generic_query.contains("tokio-rs/axum"));
        assert!(plan.hints.file.as_deref() == Some("CHANGELOG.md"));
        assert!(plan
            .generic_query
            .contains("releases changelog migration tag github gitlab"));
    }

    #[test]
    fn empty_residual_and_no_repo_uses_original() {
        let plan = build_search_plan(&req("repo:invalid", SearchIntent::Code));
        // "repo:invalid" has no owner/repo, so residual is "repo:invalid"
        assert!(plan.generic_query.contains("repo:invalid"));
        assert!(plan
            .generic_query
            .contains("github gitlab codeberg source repository"));
    }
}
