//! Repo bundle planner: generates bounded subqueries from resolved repo hints.

use crate::core::code_metadata::CodeHost;
use crate::core::package::PackageResolution;
use crate::core::repo_query::RepoQueryHints;
use crate::core::repo_search::RepoSearchRequest;

/// A single subquery for the repo bundle search.
#[derive(Clone, Debug)]
pub struct RepoSubquery {
    /// Label for this subquery (used in debugging and optional metadata).
    pub label: String,
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
    /// The generated subqueries (max 8).
    pub subqueries: Vec<RepoSubquery>,
}

/// Build a repo search plan from a request, with optional package resolution.
pub fn build_repo_search_plan(req: &RepoSearchRequest) -> RepoSearchPlan {
    build_repo_search_plan_with_package(req, None)
}

/// Build a repo search plan from a request with optional package resolution.
///
/// When `package_resolution` is provided and successful, generates
/// additional subqueries scoped to the resolved package context.
pub fn build_repo_search_plan_with_package(
    req: &RepoSearchRequest,
    package_resolution: Option<&PackageResolution>,
) -> RepoSearchPlan {
    let mut hints = req.resolved_hints();

    // Merge package-derived hints when no explicit repo is supplied
    if let Some(pr) = package_resolution {
        if pr.verified && hints.owner.is_none() && hints.repo.is_none() {
            if let Some(repo_url) = &pr.source_repository_url {
                if let Some((owner, repo)) = parse_github_owner_repo(repo_url) {
                    hints.owner = Some(owner);
                    hints.repo = Some(repo);
                    if hints.host.is_none() {
                        hints.host = Some(CodeHost::Github);
                    }
                }
            }
        }
    }

    let residual = hints.residual_query.clone();
    let owner_repo = match (&hints.owner, &hints.repo) {
        (Some(o), Some(r)) => Some(format!("{o}/{r}")),
        _ => None,
    };

    let pkg_name =
        package_resolution.and_then(|pr| pr.verified.then_some(pr.coordinate.name.as_str()));

    let mut subqueries: Vec<RepoSubquery> = Vec::new();

    // docs subquery
    if req.include_docs_enabled() {
        let q = build_docs_query(&residual, &owner_repo, pkg_name);
        if let Some(query) = q {
            subqueries.push(RepoSubquery {
                label: "docs".to_string(),
                query,
                target_groups: vec!["official_docs"],
            });
        }
    }

    // registry subquery
    if req.include_registry_enabled() {
        let q = build_registry_query(&residual, &owner_repo, hints.language.as_deref(), pkg_name);
        if let Some(query) = q {
            subqueries.push(RepoSubquery {
                label: "registry".to_string(),
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
                label: "source".to_string(),
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
                label: "examples".to_string(),
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
                label: "issues".to_string(),
                query,
                target_groups: vec!["issues", "pull_requests"],
            });
        }
    }

    // releases subquery
    if req.include_releases_enabled() {
        let q = build_releases_query(&residual, &owner_repo, hints.host, package_resolution);
        if let Some(query) = q {
            subqueries.push(RepoSubquery {
                label: "releases".to_string(),
                query,
                target_groups: vec!["releases", "migration_notes", "changelog"],
            });
        }
    }

    // changelog subquery (when package has compare_version or changelog requested)
    if req.include_changelog_enabled() && req.compare_version.is_some() {
        let q = build_changelog_query(&owner_repo, package_resolution, &req.compare_version);
        if let Some(query) = q {
            subqueries.push(RepoSubquery {
                label: "changelog".to_string(),
                query,
                target_groups: vec!["changelog", "migration_notes"],
            });
        }
    }

    // Cap at 8 subqueries
    subqueries.truncate(8);

    RepoSearchPlan { hints, subqueries }
}

/// Extract owner/repo from a GitHub URL.
fn parse_github_owner_repo(url: &str) -> Option<(String, String)> {
    let url = url.trim_end_matches('/');
    // https://github.com/owner/repo
    let path = url.strip_prefix("https://github.com/")?;
    let parts: Vec<&str> = path.split('/').collect();
    if parts.len() >= 2 && !parts[0].is_empty() && !parts[1].is_empty() {
        Some((parts[0].to_string(), parts[1].to_string()))
    } else {
        None
    }
}

fn build_docs_query(
    residual: &str,
    owner_repo: &Option<String>,
    pkg_name: Option<&str>,
) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();
    if !residual.is_empty() {
        parts.push(residual.to_string());
    }
    if let Some(or) = owner_repo {
        parts.push(or.clone());
    }
    if let Some(name) = pkg_name {
        parts.push(name.to_string());
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
    pkg_name: Option<&str>,
) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();
    if !residual.is_empty() {
        parts.push(residual.to_string());
    }
    if let Some(or) = owner_repo {
        parts.push(or.clone());
    }
    if let Some(name) = pkg_name {
        parts.push(name.to_string());
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
    package_resolution: Option<&PackageResolution>,
) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();
    if !residual.is_empty() {
        parts.push(residual.to_string());
    }
    if let Some(or) = owner_repo {
        parts.push(or.clone());
    }
    // Add package name for scoped release search
    if let Some(pr) = package_resolution {
        if pr.verified {
            parts.push(pr.coordinate.name.clone());
        }
    }
    if parts.is_empty() {
        return None;
    }
    parts.push("releases".to_string());
    parts.push("changelog".to_string());
    parts.push("migration".to_string());
    // Add version context when available
    if let Some(pr) = package_resolution {
        if let Some(ver) = &pr.resolved_version {
            parts.push(ver.clone());
        }
    }
    match host {
        Some(CodeHost::Github) => parts.push("github".to_string()),
        Some(CodeHost::Gitlab) => parts.push("gitlab".to_string()),
        _ => {}
    }
    Some(parts.join(" "))
}

/// Build a changelog/migration subquery for version comparison.
fn build_changelog_query(
    owner_repo: &Option<String>,
    package_resolution: Option<&PackageResolution>,
    compare_version: &Option<String>,
) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();
    if let Some(or) = owner_repo {
        parts.push(or.clone());
    }
    if let Some(pr) = package_resolution {
        if pr.verified {
            parts.push(pr.coordinate.name.clone());
            if let Some(ver) = &pr.resolved_version {
                parts.push(ver.clone());
            }
        }
    }
    if let Some(cv) = compare_version {
        parts.push(cv.clone());
    }
    if parts.is_empty() {
        return None;
    }
    parts.push("changelog".to_string());
    parts.push("migration".to_string());
    parts.push("breaking changes".to_string());
    parts.push("upgrade guide".to_string());
    Some(parts.join(" "))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::code_metadata::CodeHost;
    use crate::core::package::{PackageCoordinate, PackageEcosystem, PackageResolution};

    fn full_request() -> RepoSearchRequest {
        RepoSearchRequest {
            query: "router middleware".to_string(),
            host: Some(CodeHost::Github),
            owner: Some("tokio-rs".to_string()),
            repo: Some("axum".to_string()),
            ..Default::default()
        }
    }

    fn package_resolution() -> PackageResolution {
        PackageResolution {
            coordinate: PackageCoordinate {
                ecosystem: PackageEcosystem::CratesIo,
                name: "axum".to_string(),
                version: Some("0.7.0".to_string()),
                version_requirement: None,
            },
            registry_url: Some("https://crates.io/crates/axum".to_string()),
            source_repository_url: Some("https://github.com/tokio-rs/axum".to_string()),
            verified: true,
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
        let labels: Vec<&str> = plan.subqueries.iter().map(|s| s.label.as_str()).collect();
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
        let labels: Vec<&str> = plan.subqueries.iter().map(|s| s.label.as_str()).collect();
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
        let labels: Vec<&str> = plan.subqueries.iter().map(|s| s.label.as_str()).collect();
        assert!(!labels.contains(&"docs"));
        assert!(!labels.contains(&"registry"));
    }

    #[test]
    fn repo_only_generates_structural_subqueries() {
        let req = RepoSearchRequest {
            query: String::new(),
            owner: Some("tokio-rs".to_string()),
            repo: Some("axum".to_string()),
            ..Default::default()
        };
        let plan = build_repo_search_plan(&req);
        assert!(plan.hints.owner.is_some());
        assert!(plan.hints.repo.is_some());
        let labels: Vec<&str> = plan.subqueries.iter().map(|s| s.label.as_str()).collect();
        assert!(labels.contains(&"docs"));
        assert!(labels.contains(&"registry"));
        assert!(labels.contains(&"source"));
        assert!(labels.contains(&"issues"));
        assert!(labels.contains(&"releases"));
    }

    #[test]
    fn identity_forms_produce_equivalent_planner_output() {
        // 1. Explicit owner+repo
        let explicit = RepoSearchRequest {
            query: String::new(),
            owner: Some("tokio-rs".to_string()),
            repo: Some("axum".to_string()),
            ..Default::default()
        };
        // 2. Slash-form repo (no owner)
        let slash_form = RepoSearchRequest {
            query: String::new(),
            repo: Some("tokio-rs/axum".to_string()),
            ..Default::default()
        };
        // 3. Query-hint repo
        let query_hint = RepoSearchRequest {
            query: "repo:tokio-rs/axum".to_string(),
            ..Default::default()
        };

        let plan_explicit = build_repo_search_plan(&explicit);
        let plan_slash = build_repo_search_plan(&slash_form);
        let plan_hint = build_repo_search_plan(&query_hint);

        // All three should resolve to the same owner/repo
        assert_eq!(plan_explicit.hints.owner.as_deref(), Some("tokio-rs"));
        assert_eq!(plan_slash.hints.owner.as_deref(), Some("tokio-rs"));
        assert_eq!(plan_hint.hints.owner.as_deref(), Some("tokio-rs"));

        assert_eq!(plan_explicit.hints.repo.as_deref(), Some("axum"));
        assert_eq!(plan_slash.hints.repo.as_deref(), Some("axum"));
        assert_eq!(plan_hint.hints.repo.as_deref(), Some("axum"));

        // All three should produce the same subquery labels
        let labels_explicit: Vec<&str> = plan_explicit
            .subqueries
            .iter()
            .map(|s| s.label.as_str())
            .collect();
        let labels_slash: Vec<&str> = plan_slash
            .subqueries
            .iter()
            .map(|s| s.label.as_str())
            .collect();
        let labels_hint: Vec<&str> = plan_hint
            .subqueries
            .iter()
            .map(|s| s.label.as_str())
            .collect();

        assert_eq!(
            labels_explicit, labels_slash,
            "explicit and slash-form should produce same subquery labels"
        );
        assert_eq!(
            labels_explicit, labels_hint,
            "explicit and query-hint should produce same subquery labels"
        );
    }

    #[test]
    fn max_eight_subqueries_cap() {
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
            compare_version: Some("0.6.0".to_string()),
            ..Default::default()
        };
        let plan = build_repo_search_plan(&req);
        assert!(plan.subqueries.len() <= 8);
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
        let labels: Vec<&str> = plan.subqueries.iter().map(|s| s.label.as_str()).collect();
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

    #[test]
    fn package_resolution_derives_repo_hints() {
        let req = RepoSearchRequest {
            query: "Router::layer middleware".to_string(),
            ..Default::default()
        };
        let pr = package_resolution();
        let plan = build_repo_search_plan_with_package(&req, Some(&pr));
        // Should derive owner/repo from package resolution
        assert_eq!(plan.hints.owner.as_deref(), Some("tokio-rs"));
        assert_eq!(plan.hints.repo.as_deref(), Some("axum"));
        assert_eq!(plan.hints.host, Some(CodeHost::Github));
    }

    #[test]
    fn explicit_repo_overrides_package_derived() {
        let req = RepoSearchRequest {
            query: "Router::layer middleware".to_string(),
            owner: Some("other-owner".to_string()),
            repo: Some("other-repo".to_string()),
            ..Default::default()
        };
        let pr = package_resolution();
        let plan = build_repo_search_plan_with_package(&req, Some(&pr));
        // Explicit repo should win over package-derived
        assert_eq!(plan.hints.owner.as_deref(), Some("other-owner"));
        assert_eq!(plan.hints.repo.as_deref(), Some("other-repo"));
    }

    #[test]
    fn package_name_appears_in_docs_query() {
        let req = RepoSearchRequest {
            query: "middleware".to_string(),
            ..Default::default()
        };
        let pr = package_resolution();
        let plan = build_repo_search_plan_with_package(&req, Some(&pr));
        let docs = plan.subqueries.iter().find(|s| s.label == "docs").unwrap();
        assert!(docs.query.contains("axum"));
    }

    #[test]
    fn package_name_appears_in_registry_query() {
        let req = RepoSearchRequest {
            query: "middleware".to_string(),
            ..Default::default()
        };
        let pr = package_resolution();
        let plan = build_repo_search_plan_with_package(&req, Some(&pr));
        let reg = plan
            .subqueries
            .iter()
            .find(|s| s.label == "registry")
            .unwrap();
        assert!(reg.query.contains("axum"));
    }

    #[test]
    fn compare_version_generates_changelog_subquery() {
        let req = RepoSearchRequest {
            query: "axum".to_string(),
            owner: Some("tokio-rs".to_string()),
            repo: Some("axum".to_string()),
            compare_version: Some("0.6.0".to_string()),
            include_changelog: Some(true),
            ..Default::default()
        };
        let plan = build_repo_search_plan(&req);
        let labels: Vec<&str> = plan.subqueries.iter().map(|s| s.label.as_str()).collect();
        assert!(labels.contains(&"changelog"));
        let changelog = plan
            .subqueries
            .iter()
            .find(|s| s.label == "changelog")
            .unwrap();
        assert!(changelog.query.contains("0.6.0"));
        assert!(changelog.query.contains("breaking changes"));
    }

    #[test]
    fn changelog_subquery_omitted_without_compare_version() {
        let req = RepoSearchRequest {
            query: "axum".to_string(),
            include_changelog: Some(true),
            ..Default::default()
        };
        let plan = build_repo_search_plan(&req);
        let labels: Vec<&str> = plan.subqueries.iter().map(|s| s.label.as_str()).collect();
        assert!(!labels.contains(&"changelog"));
    }

    #[test]
    fn unverified_package_does_not_derive_repo_hints() {
        let req = RepoSearchRequest {
            query: "middleware".to_string(),
            ..Default::default()
        };
        let pr = PackageResolution {
            coordinate: PackageCoordinate {
                ecosystem: PackageEcosystem::CratesIo,
                name: "axum".to_string(),
                version: Some("0.7.0".to_string()),
                version_requirement: None,
            },
            verified: false,
            ..Default::default()
        };
        let plan = build_repo_search_plan_with_package(&req, Some(&pr));
        // Unverified resolution should not derive hints
        assert!(plan.hints.owner.is_none());
        assert!(plan.hints.repo.is_none());
    }

    #[test]
    fn parse_github_owner_repo_basic() {
        assert_eq!(
            parse_github_owner_repo("https://github.com/tokio-rs/axum"),
            Some(("tokio-rs".to_string(), "axum".to_string()))
        );
    }

    #[test]
    fn parse_github_owner_repo_trailing_slash() {
        assert_eq!(
            parse_github_owner_repo("https://github.com/tokio-rs/axum/"),
            Some(("tokio-rs".to_string(), "axum".to_string()))
        );
    }

    #[test]
    fn parse_github_owner_repo_non_github() {
        assert_eq!(
            parse_github_owner_repo("https://gitlab.com/user/repo"),
            None
        );
    }

    #[test]
    fn parse_github_owner_repo_too_few_segments() {
        assert_eq!(parse_github_owner_repo("https://github.com/tokio-rs"), None);
    }
}
