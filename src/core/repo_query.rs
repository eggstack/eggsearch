use crate::core::code_metadata::CodeHost;

/// Structured hints parsed from a repo-oriented search query string.
///
/// Supports `repo:`, `org:`, `path:`, `file:`, `lang:`/`language:`,
/// `symbol:`, and `host:` hint tokens, plus bare `owner/repo` extraction
/// and `repo=` alternative syntax. Unknown `key:value` tokens are
/// preserved in `residual_query`.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RepoQueryHints {
    /// Code-host platform (e.g. GitHub, GitLab, Codeberg).
    pub host: Option<CodeHost>,
    /// Repository owner extracted from `repo:owner/name`.
    pub owner: Option<String>,
    /// Repository name extracted from `repo:owner/name`.
    pub repo: Option<String>,
    /// Organization extracted from `org:` or `owner:`.
    pub org: Option<String>,
    /// File path hint from `path:`.
    pub path: Option<String>,
    /// Filename hint from `file:`.
    pub file: Option<String>,
    /// Programming language hint from `lang:` or `language:` (lowercased).
    pub language: Option<String>,
    /// Symbol hint from `symbol:`.
    pub symbol: Option<String>,
    /// Free text remaining after all recognized hints are removed.
    pub residual_query: String,
}

impl RepoQueryHints {
    /// Returns `true` if any hint field is populated.
    pub fn has_any(&self) -> bool {
        self.host.is_some()
            || self.owner.is_some()
            || self.repo.is_some()
            || self.org.is_some()
            || self.path.is_some()
            || self.file.is_some()
            || self.language.is_some()
            || self.symbol.is_some()
    }

    /// Parse structured hints from a query string.
    pub fn parse(query: &str) -> Self {
        let mut hints = RepoQueryHints::default();
        let mut tokens: Vec<&str> = Vec::new();
        for token in query.split_whitespace() {
            tokens.push(token);
        }

        let mut i = 0;
        while i < tokens.len() {
            let token = tokens[i];
            if let Some(parsed) = parse_hint_token(token) {
                match parsed.kind {
                    HintKind::Repo => {
                        if let Some((o, r)) = parsed.value.split_once('/') {
                            if !o.is_empty() && !r.is_empty() {
                                hints.owner = Some(o.to_string());
                                hints.repo = Some(r.to_string());
                            }
                        }
                    }
                    HintKind::Org => {
                        if !parsed.value.is_empty() {
                            hints.org = Some(parsed.value.to_string());
                        }
                    }
                    HintKind::Path => {
                        if !parsed.value.is_empty() {
                            hints.path = Some(parsed.value.to_string());
                        }
                    }
                    HintKind::File => {
                        if !parsed.value.is_empty() {
                            hints.file = Some(parsed.value.to_string());
                        }
                    }
                    HintKind::Language => {
                        if !parsed.value.is_empty() {
                            hints.language = Some(parsed.value.to_lowercase());
                        }
                    }
                    HintKind::Symbol => {
                        if !parsed.value.is_empty() {
                            hints.symbol = Some(parsed.value.to_string());
                        }
                    }
                    HintKind::Host => {
                        hints.host = Some(parse_host(&parsed.value));
                    }
                }
                i += 1;
                continue;
            }

            if parsed_value_eq(token, "repo=") {
                if let Some(val) = token.split_once('=') {
                    let v = val.1.trim_matches('"');
                    if let Some((o, r)) = v.split_once('/') {
                        if !o.is_empty() && !r.is_empty() {
                            hints.owner = Some(o.to_string());
                            hints.repo = Some(r.to_string());
                        }
                    }
                }
                i += 1;
                continue;
            }

            if looks_like_owner_repo(token) && hints.owner.is_none() && hints.repo.is_none() {
                if let Some((o, r)) = token.split_once('/') {
                    if !o.is_empty() && !r.is_empty() {
                        hints.owner = Some(o.to_string());
                        hints.repo = Some(r.to_string());
                        i += 1;
                        continue;
                    }
                }
            }

            i += 1;
        }

        let residual: Vec<&str> = query
            .split_whitespace()
            .filter(|t| !is_consumed_hint(t, &hints))
            .collect();
        hints.residual_query = residual.join(" ");

        hints
    }
}

struct ParsedHint {
    kind: HintKind,
    value: String,
}

enum HintKind {
    Repo,
    Org,
    Path,
    File,
    Language,
    Symbol,
    Host,
}

fn parse_hint_token(token: &str) -> Option<ParsedHint> {
    let (key, raw_value) = token.split_once(':')?;
    let key_lower = key.to_lowercase();
    let value = raw_value.trim_matches('"');

    match key_lower.as_str() {
        "repo" | "repository" | "project" => Some(ParsedHint {
            kind: HintKind::Repo,
            value: value.to_string(),
        }),
        "org" | "owner" => Some(ParsedHint {
            kind: HintKind::Org,
            value: value.to_string(),
        }),
        "path" => Some(ParsedHint {
            kind: HintKind::Path,
            value: value.to_string(),
        }),
        "file" => Some(ParsedHint {
            kind: HintKind::File,
            value: value.to_string(),
        }),
        "lang" | "language" => Some(ParsedHint {
            kind: HintKind::Language,
            value: value.to_string(),
        }),
        "symbol" => Some(ParsedHint {
            kind: HintKind::Symbol,
            value: value.to_string(),
        }),
        "host" => Some(ParsedHint {
            kind: HintKind::Host,
            value: value.to_string(),
        }),
        _ => None,
    }
}

fn parsed_value_eq(token: &str, prefix: &str) -> bool {
    token.len() > prefix.len() && token.starts_with(prefix)
}

fn parse_host(value: &str) -> CodeHost {
    match value.to_lowercase().as_str() {
        "github" | "gh" => CodeHost::Github,
        "gitlab" | "gl" => CodeHost::Gitlab,
        "codeberg" | "cb" => CodeHost::Codeberg,
        _ => CodeHost::Unknown,
    }
}

fn looks_like_owner_repo(token: &str) -> bool {
    if token.starts_with(':') || token.starts_with('=') {
        return false;
    }
    if let Some((left, right)) = token.split_once('/') {
        if left.is_empty() || right.is_empty() {
            return false;
        }
        if right.contains('/') {
            return false;
        }
        return left
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
            && right
                .chars()
                .all(|c| c.is_alphanumeric() || c == '-' || c == '_');
    }
    false
}

fn is_consumed_hint(token: &str, hints: &RepoQueryHints) -> bool {
    if let Some((key, raw_value)) = token.split_once(':') {
        let key_lower = key.to_lowercase();
        let value = raw_value.trim_matches('"');
        match key_lower.as_str() {
            "repo" | "repository" | "project" => {
                if let Some((o, r)) = value.split_once('/') {
                    hints.owner.as_deref() == Some(o) && hints.repo.as_deref() == Some(r)
                } else {
                    false
                }
            }
            "org" | "owner" => hints.org.as_deref() == Some(value),
            "path" => hints.path.as_deref() == Some(value),
            "file" => hints.file.as_deref() == Some(value),
            "lang" | "language" => hints.language.as_deref() == Some(&value.to_lowercase()),
            "symbol" => hints.symbol.as_deref() == Some(value),
            "host" => hints.host == Some(parse_host(value)),
            _ => false,
        }
    } else if token.contains('=') && token.starts_with("repo=") {
        if let Some(val) = token.split_once('=') {
            let v = val.1.trim_matches('"');
            if let Some((o, r)) = v.split_once('/') {
                return hints.owner.as_deref() == Some(o) && hints.repo.as_deref() == Some(r);
            }
        }
        false
    } else if looks_like_owner_repo(token) && hints.owner.is_some() && hints.repo.is_some() {
        if let Some((o, r)) = token.split_once('/') {
            hints.owner.as_deref() == Some(o) && hints.repo.as_deref() == Some(r)
        } else {
            false
        }
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repo_hint_axum() {
        let h = RepoQueryHints::parse("repo:tokio-rs/axum Router::layer");
        assert_eq!(h.owner.as_deref(), Some("tokio-rs"));
        assert_eq!(h.repo.as_deref(), Some("axum"));
        assert_eq!(h.residual_query, "Router::layer");
        assert!(h.has_any());
    }

    #[test]
    fn repository_alias() {
        let h = RepoQueryHints::parse("repository:tokio-rs/axum Router::layer");
        assert_eq!(h.owner.as_deref(), Some("tokio-rs"));
        assert_eq!(h.repo.as_deref(), Some("axum"));
        assert_eq!(h.residual_query, "Router::layer");
    }

    #[test]
    fn project_alias() {
        let h = RepoQueryHints::parse("project:tokio-rs/axum Router::layer");
        assert_eq!(h.owner.as_deref(), Some("tokio-rs"));
        assert_eq!(h.repo.as_deref(), Some("axum"));
        assert_eq!(h.residual_query, "Router::layer");
    }

    #[test]
    fn org_hint() {
        let h = RepoQueryHints::parse("org:rust-lang MIR borrow checker");
        assert_eq!(h.org.as_deref(), Some("rust-lang"));
        assert_eq!(h.residual_query, "MIR borrow checker");
    }

    #[test]
    fn owner_alias_for_org() {
        let h = RepoQueryHints::parse("owner:rust-lang MIR borrow checker");
        assert_eq!(h.org.as_deref(), Some("rust-lang"));
        assert_eq!(h.residual_query, "MIR borrow checker");
    }

    #[test]
    fn combined_hints() {
        let h = RepoQueryHints::parse(
            "host:github repo:rust-lang/rust path:compiler/rustc_borrowck lang:rust",
        );
        assert_eq!(h.host, Some(CodeHost::Github));
        assert_eq!(h.owner.as_deref(), Some("rust-lang"));
        assert_eq!(h.repo.as_deref(), Some("rust"));
        assert_eq!(h.path.as_deref(), Some("compiler/rustc_borrowck"));
        assert_eq!(h.language.as_deref(), Some("rust"));
        assert_eq!(h.residual_query, "");
        assert!(h.residual_query.is_empty());
    }

    #[test]
    fn language_and_repo() {
        let h = RepoQueryHints::parse("language:python repo:psf/requests Session.send");
        assert_eq!(h.language.as_deref(), Some("python"));
        assert_eq!(h.owner.as_deref(), Some("psf"));
        assert_eq!(h.repo.as_deref(), Some("requests"));
        assert_eq!(h.residual_query, "Session.send");
    }

    #[test]
    fn symbol_and_repo() {
        let h = RepoQueryHints::parse("symbol:Router::layer repo:tokio-rs/axum");
        assert_eq!(h.symbol.as_deref(), Some("Router::layer"));
        assert_eq!(h.owner.as_deref(), Some("tokio-rs"));
        assert_eq!(h.repo.as_deref(), Some("axum"));
        assert_eq!(h.residual_query, "");
    }

    #[test]
    fn bare_owner_repo() {
        let h = RepoQueryHints::parse("tokio-rs/axum Router::layer");
        assert_eq!(h.owner.as_deref(), Some("tokio-rs"));
        assert_eq!(h.repo.as_deref(), Some("axum"));
        assert_eq!(h.residual_query, "Router::layer");
    }

    #[test]
    fn malformed_repo_does_not_panic() {
        let h = RepoQueryHints::parse("repo:tokio-rs");
        assert!(h.owner.is_none());
        assert!(h.repo.is_none());
        assert_eq!(h.residual_query, "repo:tokio-rs");
    }

    #[test]
    fn empty_path_does_not_panic() {
        let h = RepoQueryHints::parse("path:");
        assert!(h.path.is_none());
        assert_eq!(h.residual_query, "path:");
    }

    #[test]
    fn unknown_hint_preserved_in_residual() {
        let h = RepoQueryHints::parse("foo:bar");
        assert_eq!(h.residual_query, "foo:bar");
        assert!(!h.has_any());
    }

    #[test]
    fn repo_equals_syntax() {
        let h = RepoQueryHints::parse("repo=tokio-rs/axum Router::layer");
        assert_eq!(h.owner.as_deref(), Some("tokio-rs"));
        assert_eq!(h.repo.as_deref(), Some("axum"));
        assert_eq!(h.residual_query, "Router::layer");
    }

    #[test]
    fn trim_quotes_path() {
        let h = RepoQueryHints::parse("path:\"src/lib.rs\"");
        assert_eq!(h.path.as_deref(), Some("src/lib.rs"));
    }

    #[test]
    fn trim_quotes_path_no_quotes() {
        let h = RepoQueryHints::parse("path:src/lib.rs");
        assert_eq!(h.path.as_deref(), Some("src/lib.rs"));
    }

    #[test]
    fn host_alias_gh() {
        let h = RepoQueryHints::parse("host:gh");
        assert_eq!(h.host, Some(CodeHost::Github));
    }

    #[test]
    fn host_alias_gl() {
        let h = RepoQueryHints::parse("host:gl");
        assert_eq!(h.host, Some(CodeHost::Gitlab));
    }

    #[test]
    fn host_alias_cb() {
        let h = RepoQueryHints::parse("host:cb");
        assert_eq!(h.host, Some(CodeHost::Codeberg));
    }

    #[test]
    fn host_github_full() {
        let h = RepoQueryHints::parse("host:github");
        assert_eq!(h.host, Some(CodeHost::Github));
    }

    #[test]
    fn host_gitlab_full() {
        let h = RepoQueryHints::parse("host:gitlab");
        assert_eq!(h.host, Some(CodeHost::Gitlab));
    }

    #[test]
    fn host_codeberg_full() {
        let h = RepoQueryHints::parse("host:codeberg");
        assert_eq!(h.host, Some(CodeHost::Codeberg));
    }

    #[test]
    fn host_unknown() {
        let h = RepoQueryHints::parse("host:bitbucket");
        assert_eq!(h.host, Some(CodeHost::Unknown));
    }

    #[test]
    fn language_normalized_lowercase() {
        let h = RepoQueryHints::parse("lang:Python");
        assert_eq!(h.language.as_deref(), Some("python"));
    }

    #[test]
    fn empty_query() {
        let h = RepoQueryHints::parse("");
        assert_eq!(h, RepoQueryHints::default());
        assert!(!h.has_any());
    }

    #[test]
    fn only_whitespace() {
        let h = RepoQueryHints::parse("   ");
        assert_eq!(h, RepoQueryHints::default());
    }

    #[test]
    fn empty_residual_when_all_hints() {
        let h = RepoQueryHints::parse("repo:tokio-rs/axum");
        assert_eq!(h.residual_query, "");
    }

    #[test]
    fn empty_residual_path_symbol_and_repo() {
        let h = RepoQueryHints::parse("repo:tokio-rs/axum path:src/lib.rs symbol:Router::layer");
        assert_eq!(h.residual_query, "");
    }

    #[test]
    fn malformed_repo_with_slash_no_value() {
        let h = RepoQueryHints::parse("repo:/axum");
        assert!(h.owner.is_none());
        assert!(h.repo.is_none());
    }

    #[test]
    fn owner_slash_only() {
        let h = RepoQueryHints::parse("repo:tokio-rs/");
        assert!(h.owner.is_none());
        assert!(h.repo.is_none());
    }

    #[test]
    fn file_hint() {
        let h = RepoQueryHints::parse("file:Cargo.toml tokio");
        assert_eq!(h.file.as_deref(), Some("Cargo.toml"));
        assert_eq!(h.residual_query, "tokio");
    }

    #[test]
    fn multiple_tokens_with_mixed_hints_and_free_text() {
        let h = RepoQueryHints::parse("repo:tokio-rs/axum middleware layer ordering");
        assert_eq!(h.owner.as_deref(), Some("tokio-rs"));
        assert_eq!(h.repo.as_deref(), Some("axum"));
        assert_eq!(h.residual_query, "middleware layer ordering");
    }

    #[test]
    fn has_any_true() {
        let mut h = RepoQueryHints::default();
        assert!(!h.has_any());
        h.language = Some("rust".to_string());
        assert!(h.has_any());
    }

    #[test]
    fn bare_owner_repo_not_parsed_when_explicit_repo_exists() {
        let h = RepoQueryHints::parse("repo:rust-lang/rust tokio-rs/axum");
        assert_eq!(h.owner.as_deref(), Some("rust-lang"));
        assert_eq!(h.repo.as_deref(), Some("rust"));
        assert_eq!(h.residual_query, "tokio-rs/axum");
    }

    #[test]
    fn repo_equals_with_quotes() {
        let h = RepoQueryHints::parse("repo=\"tokio-rs/axum\" Router::layer");
        assert_eq!(h.owner.as_deref(), Some("tokio-rs"));
        assert_eq!(h.repo.as_deref(), Some("axum"));
        assert_eq!(h.residual_query, "Router::layer");
    }

    #[test]
    fn unknown_key_value_preserved() {
        let h = RepoQueryHints::parse("repo:tokio-rs/axum unknown:thing Router::layer");
        assert_eq!(h.owner.as_deref(), Some("tokio-rs"));
        assert_eq!(h.repo.as_deref(), Some("axum"));
        assert!(h.residual_query.contains("unknown:thing"));
        assert!(h.residual_query.contains("Router::layer"));
    }

    #[test]
    fn repo_alias_case_insensitive() {
        let h = RepoQueryHints::parse("REPO:tokio-rs/axum");
        assert_eq!(h.owner.as_deref(), Some("tokio-rs"));
        assert_eq!(h.repo.as_deref(), Some("axum"));
    }

    #[test]
    fn host_case_insensitive() {
        let h = RepoQueryHints::parse("HOST:GitHub");
        assert_eq!(h.host, Some(CodeHost::Github));
    }
}
