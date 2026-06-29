//! Exact-error planner: generates targeted subqueries for compiler/runtime error messages.
//!
//! When `RepoSearchMode::ExactError` is active, this planner replaces the standard
//! repo-search subquery generation with error-aware subqueries that preserve exact
//! phrases, extract error codes, and target docs/issues/changelogs.

use crate::core::error_query::{
    generate_error_subqueries, parse_error_query, redact_error_query, ErrorQueryParts,
    ErrorSubquery, ExactErrorConfig,
};

/// Result of the error planner: parsed parts plus generated subqueries.
pub struct ErrorPlan {
    /// Parsed error query parts.
    pub parts: ErrorQueryParts,
    /// Generated subqueries (before redaction).
    pub subqueries: Vec<ErrorSubquery>,
    /// Warnings generated during planning.
    pub warnings: Vec<String>,
}

/// Build an error plan from a query string and config.
///
/// Parses the error text, optionally redacts sensitive tokens, and
/// generates bounded subqueries for docs, issues, and releases.
pub fn build_error_plan(query: &str, config: &ExactErrorConfig) -> ErrorPlan {
    let mut parts = parse_error_query(query);
    let mut warnings = Vec::new();

    // Redact sensitive tokens if enabled
    if config.redact_sensitive_tokens {
        parts = redact_error_query(&parts);
        if !parts.redactions_applied.is_empty() {
            warnings.push(format!(
                "redacted_sensitive_tokens: {} local/sensitive token(s) were removed from provider queries",
                parts.redactions_applied.len()
            ));
        }
    }

    // Detect stack traces and warn about truncation
    if parts.stack_frames.len() > 5 {
        warnings.push(
            "stack_trace_truncated: query looked like a stack trace; \
             only primary frames were used for subquery generation"
                .to_string(),
        );
    }

    // Generate subqueries from the parsed parts
    let subqueries = generate_error_subqueries(&parts, config.max_subqueries);

    // Warn if no exact phrase matches are possible
    if parts.quoted_exact.is_empty() {
        warnings.push(
            "no_exact_phrase: could not extract a primary error line; \
             results may be fuzzy"
                .to_string(),
        );
    }

    // Warn if only community results are likely
    if parts.error_codes.is_empty() && parts.package_names.is_empty() {
        warnings.push(
            "no_structured_signals: no error codes or package names detected; \
             results are from generic web search"
                .to_string(),
        );
    }

    ErrorPlan {
        parts,
        subqueries,
        warnings,
    }
}

/// Convert `ErrorSubquery` values into the format expected by the adapter's
/// subquery dispatch (label, query, target_groups).
pub fn to_repo_subqueries(
    error_subqueries: &[ErrorSubquery],
) -> Vec<crate::meta::repo_planner::RepoSubquery> {
    error_subqueries
        .iter()
        .map(|es| crate::meta::repo_planner::RepoSubquery {
            label: match es.label.as_str() {
                "exact_phrase" => "error_exact".to_string(),
                "error_code" => "error_code".to_string(),
                "package_error" => "error_package".to_string(),
                "docs" => "error_docs".to_string(),
                "issues" => "error_issues".to_string(),
                "releases" => "error_releases".to_string(),
                other => other.to_string(),
            },
            query: es.query.clone(),
            target_groups: vec![match es.target_group.as_str() {
                "official_docs" => "official_docs",
                "issues" => "issues",
                "releases" => "releases",
                _ => "other",
            }],
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_error_plan_rust() {
        let config = ExactErrorConfig::default();
        let plan = build_error_plan(
            "error[E0277]: the trait bound `Foo: Bar` is not satisfied",
            &config,
        );
        assert_eq!(plan.parts.error_codes.len(), 1);
        assert_eq!(plan.parts.error_codes[0].code, "E0277");
        assert!(plan.subqueries.len() >= 2);
        // Should have exact_phrase, error_code, docs, issues
        let labels: Vec<&str> = plan.subqueries.iter().map(|s| s.label.as_str()).collect();
        assert!(labels.contains(&"exact_phrase"));
        assert!(labels.contains(&"error_code"));
    }

    #[test]
    fn build_error_plan_redacts_paths() {
        let config = ExactErrorConfig::default();
        let plan = build_error_plan("error in /Users/john/project/src/main.rs", &config);
        assert!(!plan.parts.redactions_applied.is_empty());
        assert!(plan.warnings.iter().any(|w| w.contains("redacted")));
    }

    #[test]
    fn build_error_plan_no_redaction_when_disabled() {
        let mut config = ExactErrorConfig::default();
        config.redact_sensitive_tokens = false;
        let plan = build_error_plan("error in /Users/john/project/src/main.rs", &config);
        assert!(plan.parts.redactions_applied.is_empty());
    }

    #[test]
    fn build_error_plan_respects_max_subqueries() {
        let mut config = ExactErrorConfig::default();
        config.max_subqueries = 2;
        let plan = build_error_plan(
            "error[E0277]: the trait bound is not satisfied\npackage `tokio` not found",
            &config,
        );
        assert!(plan.subqueries.len() <= 2);
    }

    #[test]
    fn build_error_plan_warns_on_empty_primary() {
        let config = ExactErrorConfig::default();
        let plan = build_error_plan("", &config);
        assert!(plan.warnings.iter().any(|w| w.contains("no_exact_phrase")));
    }

    #[test]
    fn build_error_plan_warns_on_no_signals() {
        let config = ExactErrorConfig::default();
        let plan = build_error_plan("something went wrong", &config);
        assert!(plan
            .warnings
            .iter()
            .any(|w| w.contains("no_structured_signals")));
    }

    #[test]
    fn to_repo_subqueries_mapping() {
        let error_subs = vec![ErrorSubquery {
            label: "exact_phrase".to_string(),
            query: "\"error[E0277]\"".to_string(),
            target_group: "official_docs".to_string(),
        }];
        let repo_subs = to_repo_subqueries(&error_subs);
        assert_eq!(repo_subs.len(), 1);
        assert_eq!(repo_subs[0].label, "error_exact");
        assert_eq!(repo_subs[0].target_groups, vec!["official_docs"]);
    }
}
