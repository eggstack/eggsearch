use super::common::{RepairHint, ToolError, ToolErrorCode};
use crate::core::repo_search::{RepoSearchMode, SearchProfile};
use crate::core::research::ResearchWorkflow;
use crate::core::workflow_coverage::WorkflowKind;

pub const REPO_GOAL_VALUES: &[&str] = &[
    "understand",
    "architecture",
    "debug",
    "migration",
    "security",
    "dependency",
    "performance",
    "compare",
    "pre_change",
    "post_change",
];

pub const REPO_SOURCE_VALUES: &[&str] = &[
    "code",
    "docs",
    "registry",
    "issues",
    "pull_requests",
    "releases",
    "examples",
    "changelog",
    "migration_guides",
    "security",
];

pub const RESEARCH_INCLUDE_VALUES: &[&str] = &[
    "counterpoints",
    "primary_sources",
    "recent_discussion",
    "security_considerations",
];

pub const SECURITY_INCLUDE_VALUES: &[&str] = &[
    "kev",
    "exploit_context",
    "defensive_guidance",
    "vendor_advisories",
];

pub fn parse_repo_goal(s: &str) -> Option<WorkflowKind> {
    match s.to_ascii_lowercase().as_str() {
        "understand" => Some(WorkflowKind::ApiComprehension),
        "architecture" => Some(WorkflowKind::RepositoryArchitecture),
        "debug" => Some(WorkflowKind::ErrorInvestigation),
        "migration" => Some(WorkflowKind::VersionMigration),
        "security" => Some(WorkflowKind::SecurityReview),
        "dependency" => Some(WorkflowKind::DependencyEvaluation),
        "performance" => Some(WorkflowKind::PerformanceInvestigation),
        "compare" => Some(WorkflowKind::ComparativeResearch),
        "pre_change" | "pre-change" | "prechange" => Some(WorkflowKind::PreChangeEvidence),
        "post_change" | "post-change" | "postchange" => Some(WorkflowKind::PostChangeReview),
        _ => None,
    }
}

pub fn parse_research_goal(s: &str) -> Option<ResearchWorkflow> {
    if let Some(w) = ResearchWorkflow::parse(s) {
        return Some(w);
    }
    match s.to_ascii_lowercase().as_str() {
        "understand" => Some(ResearchWorkflow::General),
        "architecture" => Some(ResearchWorkflow::ArchitectureDecision),
        "debug" => Some(ResearchWorkflow::General),
        "migration" => Some(ResearchWorkflow::MigrationPlanning),
        "security" => Some(ResearchWorkflow::SecurityReview),
        "dependency" => Some(ResearchWorkflow::ApiEvaluation),
        "performance" => Some(ResearchWorkflow::PerformanceInvestigation),
        "compare" => Some(ResearchWorkflow::LibraryComparison),
        "pre_change" | "pre-change" | "prechange" => Some(ResearchWorkflow::MigrationPlanning),
        "post_change" | "post-change" | "postchange" => Some(ResearchWorkflow::General),
        _ => None,
    }
}

pub fn parse_security_goal(s: &str) -> Option<WorkflowKind> {
    if let Some(w) = WorkflowKind::parse(s) {
        return Some(w);
    }
    parse_repo_goal(s)
}

fn goal_error(tool: &str, field: &str, raw: &str) -> ToolError {
    let message = format!(
        "invalid {field} '{raw}' for {tool}; accepted values: {}. Repair: omit {field} or use one of the listed values.",
        REPO_GOAL_VALUES.join(", ")
    );
    ToolError::execution_with_repair(
        super::common::ToolErrorCode::InvalidSemanticValue,
        message,
        super::common::RepairHint::new(Some(field), REPO_GOAL_VALUES, None),
    )
}

fn source_error(raw: &str) -> ToolError {
    let message = format!(
        "invalid sources entry '{raw}'; accepted values: {}. Repair: omit sources or use only the listed tokens.",
        REPO_SOURCE_VALUES.join(", ")
    );
    ToolError::execution_with_repair(
        super::common::ToolErrorCode::InvalidSemanticValue,
        message,
        super::common::RepairHint::new(Some("sources"), REPO_SOURCE_VALUES, None),
    )
}

fn conflict_error(message: String, field: Option<&str>) -> ToolError {
    ToolError::execution_with_repair(
        ToolErrorCode::ConflictingArguments,
        message,
        RepairHint::new(field, &[], None),
    )
}

#[derive(Debug, Clone, Default)]
pub struct ResolvedRepoSemantics {
    pub profile: Option<SearchProfile>,
    pub mode: Option<RepoSearchMode>,
    pub workflow: Option<WorkflowKind>,
}

pub fn resolve_repo_semantics(
    goal_raw: Option<&str>,
    profile_raw: Option<&str>,
    mode_raw: Option<&str>,
    workflow_raw: Option<&str>,
) -> Result<ResolvedRepoSemantics, ToolError> {
    let goal_kind = match goal_raw {
        Some(g) => {
            let trimmed = g.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(parse_repo_goal(trimmed).ok_or_else(|| goal_error("repo_search", "goal", g))?)
            }
        }
        None => None,
    };

    let legacy_workflow = match workflow_raw {
        Some(w) => {
            let trimmed = w.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(crate::core::workflow_coverage::WorkflowKind::parse(trimmed).ok_or_else(
                    || {
                        let message = format!(
                            "invalid workflow '{w}'; accepted values: api_comprehension, repository_architecture, error_investigation, version_migration, security_review, dependency_evaluation, performance_investigation, comparative_research, pre_change_evidence, post_change_review. Repair: use goal instead ({}), or omit workflow.",
                            REPO_GOAL_VALUES.join(", ")
                        );
                        ToolError::execution_with_repair(
                            ToolErrorCode::InvalidSemanticValue,
                            message,
                            RepairHint::new(Some("workflow"), REPO_GOAL_VALUES, None),
                        )
                    },
                )?)
            }
        }
        None => None,
    };

    if let (Some(g), Some(w)) = (goal_kind, legacy_workflow) {
        if g != w {
            return Err(conflict_error(format!(
                "conflicting goal '{}' ({}) and workflow '{}' ({}); they must agree. Repair: omit workflow and keep goal, or omit goal and keep workflow.",
                goal_raw.unwrap_or(""),
                g.as_str(),
                workflow_raw.unwrap_or(""),
                w.as_str()
            ), Some("workflow")));
        }
    }

    let workflow = goal_kind.or(legacy_workflow);

    let profile = match profile_raw {
        Some(p) => {
            let trimmed = p.trim();
            if trimmed.is_empty() {
                None
            } else {
                let parsed = SearchProfile::parse(trimmed).ok_or_else(|| {
                    let message = format!(
                        "invalid profile '{p}'; accepted values: generic, coding, security, research. Repair: omit profile and use goal instead ({}).",
                        REPO_GOAL_VALUES.join(", ")
                    );
                    ToolError::execution_with_repair(
                        ToolErrorCode::InvalidSemanticValue,
                        message,
                        RepairHint::new(
                            Some("profile"),
                            &["generic", "coding", "security", "research"],
                            None,
                        ),
                    )
                })?;
                if let Some(g) = goal_kind {
                    if parsed == SearchProfile::Security && g != WorkflowKind::SecurityReview {
                        return Err(conflict_error(format!(
                            "conflicting goal '{}' and profile 'security'; profile 'security' implies goal 'security'. Repair: use goal 'security' with profile 'security', or omit profile.",
                            goal_raw.unwrap_or("")
                        ), Some("profile")));
                    }
                }
                Some(parsed)
            }
        }
        None => None,
    };

    let mode = match mode_raw {
        Some(m) => {
            let trimmed = m.trim();
            if trimmed.is_empty() {
                None
            } else {
                let parsed = RepoSearchMode::parse(trimmed).ok_or_else(|| {
                    ToolError::execution_with_repair(
                        ToolErrorCode::InvalidSemanticValue,
                        "invalid mode 'invalid'; accepted values: default, exact_error. Repair: omit mode and use goal 'debug' for error investigation.".to_string().replace("invalid", m),
                        RepairHint::new(Some("mode"), &["default", "exact_error"], Some("exact_error")),
                    )
                })?;
                if let Some(g) = goal_kind {
                    let expects_exact = g == WorkflowKind::ErrorInvestigation;
                    let is_exact = parsed == RepoSearchMode::ExactError;
                    if expects_exact && !is_exact {
                        return Err(conflict_error(format!(
                            "conflicting goal 'debug' and mode '{m}'; goal 'debug' requires exact-error behavior. Repair: omit mode or use mode 'exact_error'."
                        ), Some("mode")));
                    }
                    if !expects_exact && is_exact {
                        return Err(conflict_error(format!(
                            "conflicting goal '{}' and mode 'exact_error'; exact-error mode implies goal 'debug'. Repair: use goal 'debug' or omit mode.",
                            goal_raw.unwrap_or("")
                        ), Some("mode")));
                    }
                }
                Some(parsed)
            }
        }
        None => {
            if goal_kind == Some(WorkflowKind::ErrorInvestigation) {
                Some(RepoSearchMode::ExactError)
            } else {
                None
            }
        }
    };

    Ok(ResolvedRepoSemantics {
        profile,
        mode,
        workflow,
    })
}

#[derive(Debug, Clone, Default)]
pub struct ResolvedRepoSources {
    pub include_docs: Option<bool>,
    pub include_registry: Option<bool>,
    pub include_issues: Option<bool>,
    pub include_releases: Option<bool>,
    pub include_examples: Option<bool>,
    pub include_pull_requests: Option<bool>,
    pub include_changelog: Option<bool>,
    pub include_migration_guides: Option<bool>,
    pub include_security_context: Option<bool>,
}

#[allow(clippy::too_many_arguments)]
pub fn resolve_repo_sources(
    sources: &[String],
    legacy_docs: Option<bool>,
    legacy_registry: Option<bool>,
    legacy_issues: Option<bool>,
    legacy_releases: Option<bool>,
    legacy_examples: Option<bool>,
    legacy_prs: Option<bool>,
    legacy_changelog: Option<bool>,
    legacy_migration: Option<bool>,
    legacy_security: Option<bool>,
) -> Result<ResolvedRepoSources, ToolError> {
    if sources.is_empty() {
        return Ok(ResolvedRepoSources {
            include_docs: legacy_docs,
            include_registry: legacy_registry,
            include_issues: legacy_issues,
            include_releases: legacy_releases,
            include_examples: legacy_examples,
            include_pull_requests: legacy_prs,
            include_changelog: legacy_changelog,
            include_migration_guides: legacy_migration,
            include_security_context: legacy_security,
        });
    }
    let mut seen = std::collections::HashSet::new();
    for s in sources {
        let key = s.trim().to_ascii_lowercase();
        if key.is_empty() {
            return Err(source_error(s));
        }
        if !REPO_SOURCE_VALUES.contains(&key.as_str()) {
            return Err(source_error(s));
        }
        seen.insert(key);
    }
    let implied = |token: &str| seen.contains(token);
    let check = |name: &str,
                 legacy: Option<bool>,
                 implied_val: bool|
     -> Result<Option<bool>, ToolError> {
        if let Some(v) = legacy {
            if v != implied_val {
                return Err(ToolError::Validation(format!(
                    "conflicting sources list and legacy '{name}={v}'; sources implies '{name}={implied_val}'. Repair: omit '{name}' when 'sources' is set, or make them agree."
                )));
            }
            Ok(Some(v))
        } else {
            Ok(Some(implied_val))
        }
    };
    let has_code_only = seen.len() == 1 && seen.contains("code");
    if has_code_only
        && [
            legacy_docs,
            legacy_registry,
            legacy_issues,
            legacy_releases,
            legacy_examples,
            legacy_prs,
            legacy_changelog,
            legacy_migration,
            legacy_security,
        ]
        .iter()
        .all(|v| v.is_none())
    {
        return Ok(ResolvedRepoSources {
            include_docs: Some(false),
            include_registry: Some(false),
            include_issues: Some(false),
            include_releases: Some(false),
            include_examples: Some(false),
            include_pull_requests: Some(false),
            include_changelog: Some(false),
            include_migration_guides: Some(false),
            include_security_context: Some(false),
        });
    }
    Ok(ResolvedRepoSources {
        include_docs: check("include_docs", legacy_docs, implied("docs"))?,
        include_registry: check("include_registry", legacy_registry, implied("registry"))?,
        include_issues: check("include_issues", legacy_issues, implied("issues"))?,
        include_releases: check("include_releases", legacy_releases, implied("releases"))?,
        include_examples: check("include_examples", legacy_examples, implied("examples"))?,
        include_pull_requests: check(
            "include_pull_requests",
            legacy_prs,
            implied("pull_requests"),
        )?,
        include_changelog: check("include_changelog", legacy_changelog, implied("changelog"))?,
        include_migration_guides: check(
            "include_migration_guides",
            legacy_migration,
            implied("migration_guides"),
        )?,
        include_security_context: check(
            "include_security_context",
            legacy_security,
            implied("security"),
        )?,
    })
}

pub fn resolve_research_workflow(
    goal_raw: Option<&str>,
    workflow_raw: Option<&str>,
) -> Result<Option<ResearchWorkflow>, ToolError> {
    let goal_kind = match goal_raw {
        Some(g) => {
            let trimmed = g.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(
                    parse_research_goal(trimmed)
                        .ok_or_else(|| goal_error("research_search", "goal", g))?,
                )
            }
        }
        None => None,
    };
    let legacy = match workflow_raw {
        Some(w) => {
            let trimmed = w.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(ResearchWorkflow::parse(trimmed).ok_or_else(|| {
                    ToolError::Validation(format!(
                        "invalid workflow '{w}'; accepted values: general, api_evaluation, library_comparison, migration_planning, security_review, performance_investigation, ecosystem_survey, architecture_decision. Repair: use goal instead ({}), or omit workflow.",
                        REPO_GOAL_VALUES.join(", ")
                    ))
                })?)
            }
        }
        None => None,
    };
    if let (Some(g), Some(w)) = (goal_kind, legacy) {
        if g != w {
            return Err(ToolError::Validation(format!(
                "conflicting goal '{}' and workflow '{}'; they must agree. Repair: omit workflow and keep goal, or omit goal and keep workflow.",
                goal_raw.unwrap_or(""),
                workflow_raw.unwrap_or("")
            )));
        }
    }
    Ok(goal_kind.or(legacy))
}

#[derive(Debug, Clone, Default)]
pub struct ResolvedResearchIncludes {
    pub include_counterpoints: Option<bool>,
    pub include_primary_sources: Option<bool>,
    pub include_recent_discussion: Option<bool>,
    pub include_security_considerations: Option<bool>,
}

pub fn resolve_research_includes(
    include: &[String],
    legacy_counterpoints: Option<bool>,
    legacy_primary: Option<bool>,
    legacy_recent: Option<bool>,
    legacy_security: Option<bool>,
) -> Result<ResolvedResearchIncludes, ToolError> {
    if include.is_empty() {
        return Ok(ResolvedResearchIncludes {
            include_counterpoints: legacy_counterpoints,
            include_primary_sources: legacy_primary,
            include_recent_discussion: legacy_recent,
            include_security_considerations: legacy_security,
        });
    }
    let mut seen = std::collections::HashSet::new();
    for s in include {
        let key = s.trim().to_ascii_lowercase();
        if !RESEARCH_INCLUDE_VALUES.contains(&key.as_str()) {
            return Err(ToolError::Validation(format!(
                "invalid include entry '{s}'; accepted values: {}. Repair: omit include or use only the listed tokens.",
                RESEARCH_INCLUDE_VALUES.join(", ")
            )));
        }
        seen.insert(key);
    }
    let check = |name: &str,
                 legacy: Option<bool>,
                 implied: bool|
     -> Result<Option<bool>, ToolError> {
        if let Some(v) = legacy {
            if v != implied {
                return Err(ToolError::Validation(format!(
                    "conflicting include list and legacy '{name}={v}'; include list implies '{name}={implied}'. Repair: omit '{name}' when 'include' is set, or make them agree."
                )));
            }
            Ok(Some(v))
        } else {
            Ok(Some(implied))
        }
    };
    Ok(ResolvedResearchIncludes {
        include_counterpoints: check(
            "include_counterpoints",
            legacy_counterpoints,
            seen.contains("counterpoints"),
        )?,
        include_primary_sources: check(
            "include_primary_sources",
            legacy_primary,
            seen.contains("primary_sources"),
        )?,
        include_recent_discussion: check(
            "include_recent_discussion",
            legacy_recent,
            seen.contains("recent_discussion"),
        )?,
        include_security_considerations: check(
            "include_security_considerations",
            legacy_security,
            seen.contains("security_considerations"),
        )?,
    })
}

pub fn resolve_security_workflow(
    goal_raw: Option<&str>,
    workflow_raw: Option<&str>,
) -> Result<Option<WorkflowKind>, ToolError> {
    let goal_kind = match goal_raw {
        Some(g) => {
            let trimmed = g.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(
                    parse_security_goal(trimmed)
                        .ok_or_else(|| goal_error("security_search", "goal", g))?,
                )
            }
        }
        None => None,
    };
    let legacy = match workflow_raw {
        Some(w) => {
            let trimmed = w.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(WorkflowKind::parse(trimmed).ok_or_else(|| {
                    ToolError::Validation(format!(
                        "invalid workflow '{w}'; accepted values: api_comprehension, repository_architecture, error_investigation, version_migration, security_review, dependency_evaluation, performance_investigation, comparative_research, pre_change_evidence, post_change_review. Repair: omit workflow (security_search defaults to security_review) or use goal ({}).",
                        REPO_GOAL_VALUES.join(", ")
                    ))
                })?)
            }
        }
        None => None,
    };
    if let (Some(g), Some(w)) = (goal_kind, legacy) {
        if g != w {
            return Err(ToolError::Validation(format!(
                "conflicting goal '{}' and workflow '{}'; they must agree. Repair: omit workflow and keep goal, or omit goal and keep workflow.",
                goal_raw.unwrap_or(""),
                workflow_raw.unwrap_or("")
            )));
        }
    }
    Ok(goal_kind.or(legacy))
}

#[derive(Debug, Clone, Default)]
pub struct ResolvedSecurityIncludes {
    pub include_kev: Option<bool>,
    pub include_exploit_context: Option<bool>,
    pub include_defensive_guidance: Option<bool>,
    pub include_vendor_advisories: Option<bool>,
}

pub fn resolve_security_includes(
    include: &[String],
    legacy_kev: Option<bool>,
    legacy_exploit: Option<bool>,
    legacy_defensive: Option<bool>,
    legacy_vendor: Option<bool>,
) -> Result<ResolvedSecurityIncludes, ToolError> {
    if include.is_empty() {
        return Ok(ResolvedSecurityIncludes {
            include_kev: legacy_kev,
            include_exploit_context: legacy_exploit,
            include_defensive_guidance: legacy_defensive,
            include_vendor_advisories: legacy_vendor,
        });
    }
    let mut seen = std::collections::HashSet::new();
    for s in include {
        let key = s.trim().to_ascii_lowercase();
        if !SECURITY_INCLUDE_VALUES.contains(&key.as_str()) {
            return Err(ToolError::Validation(format!(
                "invalid include entry '{s}'; accepted values: {}. Repair: omit include or use only the listed tokens.",
                SECURITY_INCLUDE_VALUES.join(", ")
            )));
        }
        seen.insert(key);
    }
    let check = |name: &str,
                 legacy: Option<bool>,
                 implied: bool|
     -> Result<Option<bool>, ToolError> {
        if let Some(v) = legacy {
            if v != implied {
                return Err(ToolError::Validation(format!(
                    "conflicting include list and legacy '{name}={v}'; include list implies '{name}={implied}'. Repair: omit '{name}' when 'include' is set, or make them agree."
                )));
            }
            Ok(Some(v))
        } else {
            Ok(Some(implied))
        }
    };
    Ok(ResolvedSecurityIncludes {
        include_kev: check("include_kev", legacy_kev, seen.contains("kev"))?,
        include_exploit_context: check(
            "include_exploit_context",
            legacy_exploit,
            seen.contains("exploit_context"),
        )?,
        include_defensive_guidance: check(
            "include_defensive_guidance",
            legacy_defensive,
            seen.contains("defensive_guidance"),
        )?,
        include_vendor_advisories: check(
            "include_vendor_advisories",
            legacy_vendor,
            seen.contains("vendor_advisories"),
        )?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repo_goal_parses_canonical_values() {
        assert_eq!(
            parse_repo_goal("understand"),
            Some(WorkflowKind::ApiComprehension)
        );
        assert_eq!(
            parse_repo_goal("architecture"),
            Some(WorkflowKind::RepositoryArchitecture)
        );
        assert_eq!(
            parse_repo_goal("debug"),
            Some(WorkflowKind::ErrorInvestigation)
        );
        assert_eq!(
            parse_repo_goal("pre_change"),
            Some(WorkflowKind::PreChangeEvidence)
        );
        assert_eq!(parse_repo_goal("bogus"), None);
    }

    #[test]
    fn repo_semantics_goal_debug_implies_exact_error() {
        let r = resolve_repo_semantics(Some("debug"), None, None, None).unwrap();
        assert_eq!(r.workflow, Some(WorkflowKind::ErrorInvestigation));
        assert_eq!(r.mode, Some(RepoSearchMode::ExactError));
    }

    #[test]
    fn repo_semantics_conflicting_goal_and_workflow_errors() {
        let e =
            resolve_repo_semantics(Some("debug"), None, None, Some("security_review")).unwrap_err();
        assert!(e.to_string().contains("conflicting goal"));
    }

    #[test]
    fn repo_semantics_equivalent_goal_and_workflow_accepted() {
        let r =
            resolve_repo_semantics(Some("security"), None, None, Some("security_review")).unwrap();
        assert_eq!(r.workflow, Some(WorkflowKind::SecurityReview));
    }

    #[test]
    fn repo_semantics_profile_security_conflict_errors() {
        let e = resolve_repo_semantics(Some("debug"), Some("security"), None, None).unwrap_err();
        assert!(e.to_string().contains("conflicting goal"));
    }

    #[test]
    fn repo_sources_translate_to_booleans() {
        let r = resolve_repo_sources(
            &["docs".to_string(), "issues".to_string()],
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
        assert_eq!(r.include_docs, Some(true));
        assert_eq!(r.include_issues, Some(true));
        assert_eq!(r.include_registry, Some(false));
    }

    #[test]
    fn repo_sources_conflict_errors_with_repair() {
        let e = resolve_repo_sources(
            &["docs".to_string()],
            Some(false),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap_err();
        assert!(e.to_string().contains("Repair"));
    }

    #[test]
    fn research_goal_maps_canonical_values() {
        assert_eq!(
            parse_research_goal("architecture"),
            Some(ResearchWorkflow::ArchitectureDecision)
        );
        assert_eq!(
            parse_research_goal("compare"),
            Some(ResearchWorkflow::LibraryComparison)
        );
    }

    #[test]
    fn security_includes_translate() {
        let r = resolve_security_includes(&["kev".to_string()], None, None, None, None).unwrap();
        assert_eq!(r.include_kev, Some(true));
        assert_eq!(r.include_exploit_context, Some(false));
    }
}
