//! Repo bundle planner: generates bounded subqueries from resolved repo hints.

use crate::core::code_metadata::CodeHost;
use crate::core::repo_query::RepoQueryHints;
use crate::core::repo_search::RepoSearchRequest;

/// A single subquery for the repo bundle search.
#[derive(Clone, Debug)]
pub struct RepoSubquery {
    /// Label for this subquery (used in debugging and optional metadata).
    pub label: &'static str,
    /// The query text to send to providers.
    pub query: String,
    /// Which groups this subquery targets (used for filtering/classification).
    pub target_groups: Vec<&'static str>,
}

/// Complete plan for a repo bundle search.
#[derive(Clone, Debug)]
pub struct RepoSearchPlan {
    /// Resolved hints (merged from explicit fields and query tokens).
    pub hints: RepoQueryHints,
    /// The generated subqueries (max 6).
    pub subqueries: Vec<RepoSubquery>,
}

/// Build a repo search plan from a request.
pub fn build_repo_search_plan(req: &RepoSearchRequest) -> RepoSearchPlan {
    let hints = req.resolved_hints();

    let residual = hints.residual_query.clone();
    let owner_repo = match (&hints.owner, &hints.repo) {
        (Some(o), Some(r)) => Some(format!("{o}/{r}")),
        _ => None,
    };

    let mut subqueries: Vec<RepoSubquery> = Vec::new();

    // docs subquery
    if req.include_docs_enabled() {
        let q = build_docs_query(&residual, &owner_repo);
        if let Some(query) = q {
            subqueries.push(RepoSubquery {
                label: "docs",
                query,
                target_groups: vec!["official_docs"],
            });
        }
    }

    // registry subquery
    if req.include_registry_enabled() {
        let q = build_registry_query(&residual, &owner_repo, hints.language.as_deref());
        if let Some(query) = q {
            subqueries.push(RepoSubquery {
                label: "registry",
                query,
                target_groups: vec!["package_registry"],
            });
        }
    }

    // source subquery (also covers README, Examples, Tests, SourceFiles)
    let has_source_context = owner_repo.is_some()
        || hints.path.is_some()
        || hints.file.is_some()
        || hints.symbol.is_some();
    if has_source_context || !residual.is_empty() {
        let q = build_source_query(&residual, &owner_repo, &hints);
        if let Some(query) = q {
            subqueries.push(RepoSubquery {
                label: "source",
                query,
                target_groups: vec!["repository", "readme", "examples", "tests", "source_files"],
            });
        }
    }

    // examples subquery
    if req.include_examples_enabled() {
        let q = build_examples_query(&residual, &owner_repo);
        if let Some(query) = q {
            subqueries.push(RepoSubquery {
                label: "examples",
                query,
                target_groups: vec!["examples"],
            });
        }
    }

    // issues subquery
    if req.include_issues_enabled() {
        let q = build_issues_query(&residual, &owner_repo, hints.host);
        if let Some(query) = q {
            subqueries.push(RepoSubquery {
                label: "issues",
                query,
                target_groups: vec!["issues", "pull_requests"],
            });
        }
    }

    // releases subquery
    if req.include_releases_enabled() {
        let q = build_releases_query(&residual, &owner_repo, hints.host);
        if let Some(query) = q {
            subqueries.push(RepoSubquery {
                label: "releases",
                query,
                target_groups: vec!["releases", "migration_notes", "changelog"],
            });
        }
    }

    // Cap at 6 subqueries
    subqueries.truncate(6);

    RepoSearchPlan { hints, subqueries }
}

fn build_docs_query(residual: &str, owner_repo: &Option<String>) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();
    if !residual.is_empty() {
        parts.push(residual.to_string());
    }
    if let Some(or) = owner_repo {
        parts.push(or.clone());
    }
    if parts.is_empty() {
        return None;
    }
    parts.push("docs".to_string());
    parts.push("documentation".to_string());
    parts.push("api reference".to_string());
    Some(parts.join(" "))
}

fn build_registry_query(
    residual: &str,
    owner_repo: &Option<String>,
    language: Option<&str>,
) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();
    if !residual.is_empty() {
        parts.push(residual.to_string());
    }
    if let Some(or) = owner_repo {
        parts.push(or.clone());
    }
    if let Some(lang) = language {
        parts.push(lang.to_string());
    }
    if parts.is_empty() {
        return None;
    }
    parts.push("crates.io".to_string());
    parts.push("docs.rs".to_string());
    parts.push("pypi".to_string());
    parts.push("npm".to_string());
    Some(parts.join(" "))
}

fn build_source_query(
    residual: &str,
    owner_repo: &Option<String>,
    hints: &RepoQueryHints,
) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();
    if !residual.is_empty() {
        parts.push(residual.to_string());
    }
    if let Some(or) = owner_repo {
        parts.push(or.clone());
    }
    if let Some(p) = &hints.path {
        parts.push(p.clone());
    }
    if let Some(f) = &hints.file {
        parts.push(f.clone());
    }
    if let Some(l) = &hints.language {
        parts.push(l.clone());
    }
    if let Some(s) = &hints.symbol {
        parts.push(s.clone());
    }
    if parts.is_empty() {
        return None;
    }
    parts.push("github".to_string());
    parts.push("gitlab".to_string());
    parts.push("source".to_string());
    Some(parts.join(" "))
}

fn build_examples_query(residual: &str, owner_repo: &Option<String>) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();
    if let Some(or) = owner_repo {
        parts.push(or.clone());
    }
    if !residual.is_empty() {
        parts.push(residual.to_string());
    }
    if parts.is_empty() {
        return None;
    }
    parts.push("examples".to_string());
    parts.push("sample".to_string());
    parts.push("usage".to_string());
    parts.push("demo".to_string());
    Some(parts.join(" "))
}

fn build_issues_query(
    residual: &str,
    owner_repo: &Option<String>,
    host: Option<CodeHost>,
) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();
    if !residual.is_empty() {
        parts.push(residual.to_string());
    }
    if let Some(or) = owner_repo {
        parts.push(or.clone());
    }
    if parts.is_empty() {
        return None;
    }
    parts.push("issues".to_string());
    match host {
        Some(CodeHost::Github) => parts.push("github".to_string()),
        Some(CodeHost::Gitlab) => parts.push("gitlab".to_string()),
        _ => {
            parts.push("discussions".to_string());
        }
    }
    Some(parts.join(" "))
}

fn build_releases_query(
    residual: &str,
    owner_repo: &Option<String>,
    host: Option<CodeHost>,
) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();
    if !residual.is_empty() {
        parts.push(residual.to_string());
    }
    if let Some(or) = owner_repo {
        parts.push(or.clone());
    }
    if parts.is_empty() {
        return None;
    }
    parts.push("releases".to_string());
    parts.push("changelog".to_string());
    parts.push("migration".to_string());
    match host {
        Some(CodeHost::Github) => parts.push("github".to_string()),
        Some(CodeHost::Gitlab) => parts.push("gitlab".to_string()),
        _ => {}
    }
    Some(parts.join(" "))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::code_metadata::CodeHost;

    fn full_request() -> RepoSearchRequest {
        RepoSearchRequest {
            query: "router middleware".to_string(),
            host: Some(CodeHost::Github),
            owner: Some("tokio-rs".to_string()),
            repo: Some("axum".to_string()),
            ..Default::default()
        }
    }

    #[test]
    fn full_request_generates_all_subqueries() {
        let plan = build_repo_search_plan(&full_request());
        assert!(plan.hints.owner.is_some());
        assert!(plan.hints.repo.is_some());
        // Should have docs, registry, source, examples, issues, releases
        assert!(plan.subqueries.len() >= 5);
        let labels: Vec<&str> = plan.subqueries.iter().map(|s| s.label).collect();
        assert!(labels.contains(&"docs"));
        assert!(labels.contains(&"registry"));
        assert!(labels.contains(&"source"));
        assert!(labels.contains(&"issues"));
        assert!(labels.contains(&"releases"));
    }

    #[test]
    fn include_flags_suppress_subqueries() {
        let req = RepoSearchRequest {
            query: "tokio-rs/axum middleware".to_string(),
            include_docs: Some(false),
            include_registry: Some(false),
            include_issues: Some(false),
            include_releases: Some(false),
            include_examples: Some(false),
            ..Default::default()
        };
        let plan = build_repo_search_plan(&req);
        let labels: Vec<&str> = plan.subqueries.iter().map(|s| s.label).collect();
        assert!(!labels.contains(&"docs"));
        assert!(!labels.contains(&"registry"));
        assert!(!labels.contains(&"issues"));
        assert!(!labels.contains(&"releases"));
        assert!(!labels.contains(&"examples"));
        // source should still be present (it has owner/repo)
        assert!(labels.contains(&"source"));
    }

    #[test]
    fn missing_hints_dont_generate_empty_subqueries() {
        let req = RepoSearchRequest {
            query: "   ".to_string(),
            include_docs: Some(true),
            include_registry: Some(true),
            ..Default::default()
        };
        // validate would reject empty, but build_repo_search_plan doesn't check
        // When query is empty, hints are all None and residual is empty
        let plan = build_repo_search_plan(&req);
        // No owner/repo, no residual -> docs and registry should be skipped
        let labels: Vec<&str> = plan.subqueries.iter().map(|s| s.label).collect();
        assert!(!labels.contains(&"docs"));
        assert!(!labels.contains(&"registry"));
    }

    #[test]
    fn max_six_subqueries_cap() {
        let req = RepoSearchRequest {
            query: "tokio-rs/axum middleware router".to_string(),
            host: Some(CodeHost::Github),
            owner: Some("tokio-rs".to_string()),
            repo: Some("axum".to_string()),
            include_docs: Some(true),
            include_registry: Some(true),
            include_issues: Some(true),
            include_releases: Some(true),
            include_examples: Some(true),
            ..Default::default()
        };
        let plan = build_repo_search_plan(&req);
        assert!(plan.subqueries.len() <= 6);
    }

    #[test]
    fn docs_query_includes_residual() {
        let req = RepoSearchRequest {
            query: "serde deserialize".to_string(),
            owner: Some("serde-rs".to_string()),
            repo: Some("serde".to_string()),
            include_docs: Some(true),
            ..Default::default()
        };
        let plan = build_repo_search_plan(&req);
        let docs = plan.subqueries.iter().find(|s| s.label == "docs").unwrap();
        assert!(docs.query.contains("serde deserialize"));
        assert!(docs.query.contains("serde-rs/serde"));
        assert!(docs.query.contains("docs"));
    }

    #[test]
    fn registry_query_includes_language() {
        let req = RepoSearchRequest {
            query: "requests".to_string(),
            owner: Some("psf".to_string()),
            repo: Some("requests".to_string()),
            language: Some("python".to_string()),
            include_registry: Some(true),
            ..Default::default()
        };
        let plan = build_repo_search_plan(&req);
        let reg = plan
            .subqueries
            .iter()
            .find(|s| s.label == "registry")
            .unwrap();
        assert!(reg.query.contains("python"));
        assert!(reg.query.contains("crates.io"));
    }

    #[test]
    fn issues_query_uses_github_host() {
        let req = RepoSearchRequest {
            query: "tokio-rs/axum".to_string(),
            host: Some(CodeHost::Github),
            owner: Some("tokio-rs".to_string()),
            repo: Some("axum".to_string()),
            include_issues: Some(true),
            ..Default::default()
        };
        let plan = build_repo_search_plan(&req);
        let issues = plan
            .subqueries
            .iter()
            .find(|s| s.label == "issues")
            .unwrap();
        assert!(issues.query.contains("github"));
        assert!(issues.query.contains("issues"));
    }

    #[test]
    fn issues_query_fallback_without_host() {
        let req = RepoSearchRequest {
            query: "tokio-rs/axum".to_string(),
            owner: Some("tokio-rs".to_string()),
            repo: Some("axum".to_string()),
            include_issues: Some(true),
            ..Default::default()
        };
        let plan = build_repo_search_plan(&req);
        let issues = plan
            .subqueries
            .iter()
            .find(|s| s.label == "issues")
            .unwrap();
        assert!(issues.query.contains("discussions"));
    }

    #[test]
    fn releases_query_includes_changelog_migration() {
        let req = RepoSearchRequest {
            query: "tokio-rs/axum".to_string(),
            host: Some(CodeHost::Github),
            owner: Some("tokio-rs".to_string()),
            repo: Some("axum".to_string()),
            include_releases: Some(true),
            ..Default::default()
        };
        let plan = build_repo_search_plan(&req);
        let rel = plan
            .subqueries
            .iter()
            .find(|s| s.label == "releases")
            .unwrap();
        assert!(rel.query.contains("releases"));
        assert!(rel.query.contains("changelog"));
        assert!(rel.query.contains("migration"));
        assert!(rel.query.contains("github"));
    }

    #[test]
    fn residual_only_generates_source_and_docs() {
        let req = RepoSearchRequest {
            query: "axum middleware".to_string(),
            include_docs: Some(true),
            ..Default::default()
        };
        let plan = build_repo_search_plan(&req);
        let labels: Vec<&str> = plan.subqueries.iter().map(|s| s.label).collect();
        assert!(labels.contains(&"docs"));
        assert!(labels.contains(&"source"));
    }

    #[test]
    fn subqueries_contain_target_groups() {
        let plan = build_repo_search_plan(&full_request());
        for sq in &plan.subqueries {
            assert!(!sq.target_groups.is_empty());
        }
    }
}
