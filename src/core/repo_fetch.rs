//! Structured repository fetch request/response types and validation.
//!
//! `repo_fetch` provides an explicit MCP tool for fetching repository
//! objects and source-code spans by structured locator instead of by
//! generic browser URL. This is the preferred path for code agents
//! that need to inspect a known file or line range after discovering
//! it via `repo_search`.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::core::code_evidence::{SourceRole, SymbolKind};
use crate::core::code_metadata::CodeHost;
use crate::core::document::FetchDocument;
use crate::core::sanitize::TrustMarkers;
use crate::fetch::span::SelectedSpan;

/// Discriminator for repository locator kinds.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RepoLocatorKind {
    /// Remote repository file (GitHub, GitLab, etc.).
    #[default]
    Remote,
    /// Local workspace file.
    Workspace,
}

/// Structured locator for a repository file.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct RepoLocator {
    /// Locator kind discriminator.
    pub kind: RepoLocatorKind,
    /// The code host (GitHub, GitLab, etc.). Present for remote
    /// locators, absent for workspace locators.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<CodeHost>,
    /// Repository owner (or namespace for GitLab nested groups).
    /// Present for remote locators, absent for workspace locators.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    /// Repository name. Present for remote locators, absent for
    /// workspace locators.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repo: Option<String>,
    /// Branch, tag, or commit ref. Used for raw URL construction.
    /// Present for remote locators, absent for workspace locators.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ref_name: Option<String>,
    /// Full commit SHA, when known. Used for permalink construction.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit_sha: Option<String>,
    /// File path. For remote locators, relative to repository root.
    /// For workspace locators, relative to the workspace root.
    pub path: String,
    /// Workspace root directory name. Present only for workspace
    /// locators.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_root: Option<String>,
}

/// Request type for the `repo_fetch` tool.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct RepoFetchRequest {
    /// Code host. Optional when owner/repo are unambiguous;
    /// validated against known hosts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<CodeHost>,
    /// Repository owner (or namespace for GitLab nested groups).
    pub owner: String,
    /// Repository name.
    pub repo: String,
    /// Branch, tag, or commit ref. Defaults to `"main"` when omitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ref_name: Option<String>,
    /// Full commit SHA. When provided, used for permalink
    /// construction and preferred over `ref_name` for raw URL
    /// stability. The locator still carries `ref_name` for
    /// operator context.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit_sha: Option<String>,
    /// File path relative to repository root. Must not start with `/`.
    pub path: String,
    /// First line to return (1-indexed). When omitted, starts from
    /// line 1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_start: Option<u32>,
    /// Last line to return (1-indexed, inclusive). When omitted, the
    /// file is returned from `line_start` to the end (or top of file
    /// when `line_start` is also omitted).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_end: Option<u32>,
    /// Extra lines of context before `line_start`. Applied after
    /// the requested range is validated and clamped. Defaults to 0.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_before: Option<u32>,
    /// Extra lines of context after `line_end`. Applied after the
    /// requested range is validated and clamped. Defaults to 0.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_after: Option<u32>,
    /// Maximum characters to return. Defaults to the server's
    /// `max_chars_default` config. Cannot exceed
    /// `max_chars_cap`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_chars: Option<usize>,
    /// Per-request timeout override in milliseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
    /// Symbol name to search for in the file. When provided, the
    /// fetcher scans the file for a matching definition or declaration
    /// and expands to the enclosing block.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    /// Kind of symbol to search for. When omitted, all definition
    /// patterns are tried.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symbol_kind: Option<SymbolKind>,
    /// Text to search for in the file. When provided, the fetcher
    /// finds the first match and expands around it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub match_text: Option<String>,
    /// When true, expand the resolved range (from symbol, match_text,
    /// or explicit line range) to the enclosing block boundary.
    /// Defaults to false.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expand_to_block: Option<bool>,
    /// Maximum lines to include when expanding to a block. Caps the
    /// expanded span to prevent oversized returns. Defaults to 200.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_block_lines: Option<usize>,
    /// When true and a matching local checkout exists, read the file
    /// from the local workspace instead of fetching remotely. Defaults
    /// to false for backward compatibility.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prefer_local: Option<bool>,
}

/// A single line in the fetched result.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct RepoFetchedLine {
    /// 1-indexed line number in the original file.
    pub number: u32,
    /// Line content (without trailing newline).
    pub text: String,
}

/// Response type for the `repo_fetch` tool.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct RepoFetchResponse {
    /// The resolved locator for the fetched file.
    pub locator: RepoLocator,
    /// Whether content was successfully fetched.
    pub fetched: bool,
    /// HTTP status code, if a network request was made.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<u16>,
    /// Detected content type (e.g. `"text/plain"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    /// Detected programming language (from file extension or
    /// content-type).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    /// Inferred source role for this file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_role: Option<SourceRole>,
    /// Browser-viewable URL for this file.
    pub browser_url: String,
    /// Raw content URL fetched.
    pub raw_url: String,
    /// Stable permalink URL (human-viewable, when commit SHA is known).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permalink_url: Option<String>,
    /// Raw content URL at the commit SHA (when commit SHA is known).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_permalink_url: Option<String>,
    /// The actual URL used for the network fetch. Differs from
    /// `raw_url` when `commit_sha` is provided (fetches from
    /// `raw_permalink_url`) or when `test_fetch_url` overrides the URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fetched_url: Option<String>,
    /// The ref that was actually used for fetching (may differ from
    /// the requested ref if a redirect was followed).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ref_resolved: Option<String>,
    /// Requested start line (1-indexed). `None` when no line range
    /// was requested.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_start: Option<u32>,
    /// Requested end line (1-indexed, inclusive).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_end: Option<u32>,
    /// Actual start line returned (1-indexed), after clamping to
    /// file boundaries and applying context.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub returned_line_start: Option<u32>,
    /// Actual end line returned (1-indexed, inclusive).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub returned_line_end: Option<u32>,
    /// Total number of lines in the full file (when known).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_lines: Option<u32>,
    /// Extracted text content. `None` when `extract_mode` is
    /// `metadata_only` or on failure.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Individual fetched lines with line numbers. Always populated
    /// when content is successfully fetched, even when a line range
    /// is specified.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lines: Vec<RepoFetchedLine>,
    /// Structured document representation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub document: Option<FetchDocument>,
    /// Whether the output was truncated.
    pub truncated: bool,
    /// Warning messages.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
    /// Trust label for the fetched content.
    pub trust: FetchTrust,
    /// Trust markers describing what sanitization was applied.
    pub trust_markers: TrustMarkers,
    /// Metadata describing how the final line span was selected.
    /// Present when symbol, match_text, or expand_to_block was used.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_span: Option<SelectedSpan>,
}

/// Trust label for fetched repository content.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FetchTrust {
    /// External untrusted content.
    #[default]
    ExternalUntrusted,
    /// Local workspace content with operator-configured provenance.
    LocalTrusted,
}

impl RepoFetchRequest {
    /// Validate the request, returning an error if invalid.
    ///
    /// `max_chars_cap` is the server-configured upper bound for
    /// `max_chars`. The caller should pass
    /// `config.fetch.max_chars_cap`.
    pub fn validate(&self, max_chars_cap: usize) -> Result<(), String> {
        if self.owner.trim().is_empty() {
            return Err("owner must not be empty".to_string());
        }
        if self.repo.trim().is_empty() {
            return Err("repo must not be empty".to_string());
        }
        if self.path.trim().is_empty() {
            return Err("path must not be empty".to_string());
        }

        // Path must be relative, not absolute.
        if self.path.starts_with('/') {
            return Err("path must be relative, not absolute (do not start with '/')".to_string());
        }

        // Reject path traversal.
        if self.path.contains("..") {
            return Err("path must not contain '..' (path traversal)".to_string());
        }

        // Validate host if provided.
        if let Some(host) = self.host {
            match host {
                CodeHost::Github | CodeHost::Gitlab => {}
                other => {
                    return Err(format!(
                        "host '{other:?}' is not supported for repo_fetch; \
                         use github or gitlab"
                    ));
                }
            }
        }

        // Validate line range.
        if let (Some(start), Some(end)) = (self.line_start, self.line_end) {
            if start == 0 {
                return Err("line_start must be >= 1 (1-indexed)".to_string());
            }
            if end == 0 {
                return Err("line_end must be >= 1 (1-indexed)".to_string());
            }
            if start > end {
                return Err(format!("line_start ({start}) must be <= line_end ({end})"));
            }
        } else if let Some(start) = self.line_start {
            if start == 0 {
                return Err("line_start must be >= 1 (1-indexed)".to_string());
            }
        } else if let Some(end) = self.line_end {
            if end == 0 {
                return Err("line_end must be >= 1 (1-indexed)".to_string());
            }
        }

        // Validate context values.
        if let Some(ctx) = self.context_before {
            if ctx > 500 {
                return Err("context_before must be <= 500".to_string());
            }
        }
        if let Some(ctx) = self.context_after {
            if ctx > 500 {
                return Err("context_after must be <= 500".to_string());
            }
        }

        // Validate max_chars.
        if let Some(max) = self.max_chars {
            if max == 0 {
                return Err("max_chars must be > 0".to_string());
            }
            if max > max_chars_cap {
                return Err(format!(
                    "max_chars ({max}) exceeds server cap ({max_chars_cap})"
                ));
            }
        }

        Ok(())
    }
}

/// Build a browser-viewable URL for a GitHub source file.
pub fn github_browser_url(owner: &str, repo: &str, ref_name: &str, path: &str) -> String {
    format!("https://github.com/{owner}/{repo}/blob/{ref_name}/{path}")
}

/// Build a raw content URL for a GitHub source file.
pub fn github_raw_url(owner: &str, repo: &str, ref_name: &str, path: &str) -> String {
    format!("https://raw.githubusercontent.com/{owner}/{repo}/{ref_name}/{path}")
}

/// Build a browser-viewable permalink URL for a GitHub source file at a specific commit SHA.
pub fn github_permalink_url(owner: &str, repo: &str, commit_sha: &str, path: &str) -> String {
    format!("https://github.com/{owner}/{repo}/blob/{commit_sha}/{path}")
}

/// Build a raw content permalink URL for a GitHub source file at a specific commit SHA.
pub fn github_raw_permalink_url(owner: &str, repo: &str, commit_sha: &str, path: &str) -> String {
    format!("https://raw.githubusercontent.com/{owner}/{repo}/{commit_sha}/{path}")
}

/// Build a browser-viewable URL for a GitLab source file.
pub fn gitlab_browser_url(owner: &str, repo: &str, ref_name: &str, path: &str) -> String {
    let namespace = if owner.is_empty() {
        repo.to_string()
    } else {
        format!("{owner}/{repo}")
    };
    format!("https://gitlab.com/{namespace}/-/blob/{ref_name}/{path}")
}

/// Build a raw content URL for a GitLab source file.
pub fn gitlab_raw_url(owner: &str, repo: &str, ref_name: &str, path: &str) -> String {
    let namespace = if owner.is_empty() {
        repo.to_string()
    } else {
        format!("{owner}/{repo}")
    };
    format!("https://gitlab.com/{namespace}/-/raw/{ref_name}/{path}")
}

/// Apply line range and context to a list of lines, returning the
/// sliced lines, the actual returned range, whether the range was
/// truncated, and an optional warning message.
///
/// `lines` is 1-indexed (line 1 is at index 0).
pub fn apply_line_range(
    lines: &[String],
    line_start: Option<u32>,
    line_end: Option<u32>,
    context_before: u32,
    context_after: u32,
) -> (
    Vec<RepoFetchedLine>,
    Option<u32>,
    Option<u32>,
    bool,
    Option<String>,
) {
    if lines.is_empty() {
        return (vec![], None, None, false, None);
    }

    let total = lines.len() as u32;
    let start = line_start.unwrap_or(1).max(1);
    let end = line_end.unwrap_or(total).min(total);

    let mut warnings = Vec::new();

    // Warn when the requested range exceeds the file and was clamped.
    if let Some(req_end) = line_end {
        if req_end > total {
            warnings.push(format!(
                "line_end ({req_end}) exceeds file length ({total}); \
                 clamped to {total}"
            ));
        }
    }
    if let Some(req_start) = line_start {
        if req_start > total {
            warnings.push(format!(
                "line_start ({req_start}) exceeds file length ({total}); \
                 clamped to {total}"
            ));
        }
    }

    // Apply context, clamped to file boundaries.
    let ctx_start = start.saturating_sub(context_before).max(1);
    let ctx_end = end.saturating_add(context_after).min(total);

    let truncated = line_end.is_some_and(|e| e > total) || line_start.is_some_and(|s| s > total);

    let sliced: Vec<RepoFetchedLine> = (ctx_start..=ctx_end)
        .filter_map(|n| {
            let idx = (n - 1) as usize;
            lines.get(idx).map(|text| RepoFetchedLine {
                number: n,
                text: text.clone(),
            })
        })
        .collect();

    let warning = if warnings.is_empty() {
        None
    } else {
        Some(warnings.join("; "))
    };

    (sliced, Some(ctx_start), Some(ctx_end), truncated, warning)
}

/// Clamp a set of line-numbered slices to a character budget.
///
/// Builds text by joining line text with newlines, then truncates
/// from the end when the result exceeds `max_chars`. Partial lines
/// at the truncation boundary are omitted to preserve clean line
/// semantics. If no full line fits within the budget, a bounded
/// prefix of the first line is returned.
///
/// Returns `(clamped_lines, clamped_text, truncated)`.
pub fn clamp_lines_to_max_chars(
    lines: &[RepoFetchedLine],
    max_chars: Option<usize>,
) -> (Vec<RepoFetchedLine>, Option<String>, bool) {
    let Some(max) = max_chars else {
        // No budget — return everything.
        let text = if lines.is_empty() {
            None
        } else {
            Some(
                lines
                    .iter()
                    .map(|l| l.text.as_str())
                    .collect::<Vec<_>>()
                    .join("\n"),
            )
        };
        return (lines.to_vec(), text, false);
    };

    if lines.is_empty() {
        return (vec![], None, false);
    }

    // Build text and check if it fits.
    let full_text: String = lines
        .iter()
        .map(|l| l.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");

    if full_text.len() <= max {
        return (lines.to_vec(), Some(full_text), false);
    }

    // Truncate line by line from the end.
    let mut used = 0usize;
    let mut kept = Vec::new();
    for line in lines {
        let line_len = line.text.len();
        // Account for newline separator (except before first line).
        let cost = if kept.is_empty() {
            line_len
        } else {
            line_len + 1 // +1 for '\n'
        };

        if used + cost <= max {
            used += cost;
            kept.push(line.clone());
        } else {
            // This line would exceed the budget — omit it (and all
            // subsequent lines) to keep line semantics clean.
            break;
        }
    }

    if kept.is_empty() {
        // No full line fits — return a bounded prefix of the first line.
        let first = &lines[0];
        let prefix: String = first.text.chars().take(max).collect();
        let truncated_line = RepoFetchedLine {
            number: first.number,
            text: prefix,
        };
        let text = Some(truncated_line.text.clone());
        return (vec![truncated_line], text, true);
    }

    let text = Some(
        kept.iter()
            .map(|l| l.text.as_str())
            .collect::<Vec<_>>()
            .join("\n"),
    );
    let truncated = kept.len() < lines.len();
    (kept, text, truncated)
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Validation tests ---

    #[test]
    fn valid_github_request() {
        let req = RepoFetchRequest {
            host: Some(CodeHost::Github),
            owner: "tokio-rs".to_string(),
            repo: "axum".to_string(),
            ref_name: Some("main".to_string()),
            commit_sha: None,
            path: "src/lib.rs".to_string(),
            line_start: None,
            line_end: None,
            context_before: None,
            context_after: None,
            max_chars: None,
            timeout_ms: None,
            symbol: None,
            symbol_kind: None,
            match_text: None,
            expand_to_block: None,
            max_block_lines: None,
            prefer_local: None,
        };
        req.validate(50000).unwrap();
    }

    #[test]
    fn empty_owner_rejected() {
        let req = RepoFetchRequest {
            host: Some(CodeHost::Github),
            owner: "".to_string(),
            repo: "axum".to_string(),
            ref_name: Some("main".to_string()),
            commit_sha: None,
            path: "src/lib.rs".to_string(),
            line_start: None,
            line_end: None,
            context_before: None,
            context_after: None,
            max_chars: None,
            timeout_ms: None,
            symbol: None,
            symbol_kind: None,
            match_text: None,
            expand_to_block: None,
            max_block_lines: None,
            prefer_local: None,
        };
        let err = req.validate(50000).unwrap_err();
        assert!(err.contains("owner"));
    }

    #[test]
    fn empty_repo_rejected() {
        let req = RepoFetchRequest {
            host: Some(CodeHost::Github),
            owner: "tokio-rs".to_string(),
            repo: "".to_string(),
            ref_name: Some("main".to_string()),
            commit_sha: None,
            path: "src/lib.rs".to_string(),
            line_start: None,
            line_end: None,
            context_before: None,
            context_after: None,
            max_chars: None,
            timeout_ms: None,
            symbol: None,
            symbol_kind: None,
            match_text: None,
            expand_to_block: None,
            max_block_lines: None,
            prefer_local: None,
        };
        let err = req.validate(50000).unwrap_err();
        assert!(err.contains("repo"));
    }

    #[test]
    fn empty_path_rejected() {
        let req = RepoFetchRequest {
            host: Some(CodeHost::Github),
            owner: "tokio-rs".to_string(),
            repo: "axum".to_string(),
            ref_name: Some("main".to_string()),
            commit_sha: None,
            path: "".to_string(),
            line_start: None,
            line_end: None,
            context_before: None,
            context_after: None,
            max_chars: None,
            timeout_ms: None,
            symbol: None,
            symbol_kind: None,
            match_text: None,
            expand_to_block: None,
            max_block_lines: None,
            prefer_local: None,
        };
        let err = req.validate(50000).unwrap_err();
        assert!(err.contains("path"));
    }

    #[test]
    fn absolute_path_rejected() {
        let req = RepoFetchRequest {
            host: Some(CodeHost::Github),
            owner: "tokio-rs".to_string(),
            repo: "axum".to_string(),
            ref_name: Some("main".to_string()),
            commit_sha: None,
            path: "/src/lib.rs".to_string(),
            line_start: None,
            line_end: None,
            context_before: None,
            context_after: None,
            max_chars: None,
            timeout_ms: None,
            symbol: None,
            symbol_kind: None,
            match_text: None,
            expand_to_block: None,
            max_block_lines: None,
            prefer_local: None,
        };
        let err = req.validate(50000).unwrap_err();
        assert!(err.contains("relative"));
    }

    #[test]
    fn path_traversal_rejected() {
        let req = RepoFetchRequest {
            host: Some(CodeHost::Github),
            owner: "tokio-rs".to_string(),
            repo: "axum".to_string(),
            ref_name: Some("main".to_string()),
            commit_sha: None,
            path: "../etc/passwd".to_string(),
            line_start: None,
            line_end: None,
            context_before: None,
            context_after: None,
            max_chars: None,
            timeout_ms: None,
            symbol: None,
            symbol_kind: None,
            match_text: None,
            expand_to_block: None,
            max_block_lines: None,
            prefer_local: None,
        };
        let err = req.validate(50000).unwrap_err();
        assert!(err.contains("traversal"));
    }

    #[test]
    fn inverted_line_range_rejected() {
        let req = RepoFetchRequest {
            host: Some(CodeHost::Github),
            owner: "tokio-rs".to_string(),
            repo: "axum".to_string(),
            ref_name: Some("main".to_string()),
            commit_sha: None,
            path: "src/lib.rs".to_string(),
            line_start: Some(50),
            line_end: Some(10),
            context_before: None,
            context_after: None,
            max_chars: None,
            timeout_ms: None,
            symbol: None,
            symbol_kind: None,
            match_text: None,
            expand_to_block: None,
            max_block_lines: None,
            prefer_local: None,
        };
        let err = req.validate(50000).unwrap_err();
        assert!(err.contains("line_start"));
    }

    #[test]
    fn zero_line_start_rejected() {
        let req = RepoFetchRequest {
            host: Some(CodeHost::Github),
            owner: "tokio-rs".to_string(),
            repo: "axum".to_string(),
            ref_name: Some("main".to_string()),
            commit_sha: None,
            path: "src/lib.rs".to_string(),
            line_start: Some(0),
            line_end: None,
            context_before: None,
            context_after: None,
            max_chars: None,
            timeout_ms: None,
            symbol: None,
            symbol_kind: None,
            match_text: None,
            expand_to_block: None,
            max_block_lines: None,
            prefer_local: None,
        };
        let err = req.validate(50000).unwrap_err();
        assert!(err.contains(">= 1"));
    }

    #[test]
    fn zero_line_end_rejected() {
        let req = RepoFetchRequest {
            host: Some(CodeHost::Github),
            owner: "tokio-rs".to_string(),
            repo: "axum".to_string(),
            ref_name: Some("main".to_string()),
            commit_sha: None,
            path: "src/lib.rs".to_string(),
            line_start: None,
            line_end: Some(0),
            context_before: None,
            context_after: None,
            max_chars: None,
            timeout_ms: None,
            symbol: None,
            symbol_kind: None,
            match_text: None,
            expand_to_block: None,
            max_block_lines: None,
            prefer_local: None,
        };
        let err = req.validate(50000).unwrap_err();
        assert!(err.contains(">= 1"));
    }

    #[test]
    fn excessive_context_rejected() {
        let req = RepoFetchRequest {
            host: Some(CodeHost::Github),
            owner: "tokio-rs".to_string(),
            repo: "axum".to_string(),
            ref_name: Some("main".to_string()),
            commit_sha: None,
            path: "src/lib.rs".to_string(),
            line_start: None,
            line_end: None,
            context_before: Some(501),
            context_after: None,
            max_chars: None,
            timeout_ms: None,
            symbol: None,
            symbol_kind: None,
            match_text: None,
            expand_to_block: None,
            max_block_lines: None,
            prefer_local: None,
        };
        let err = req.validate(50000).unwrap_err();
        assert!(err.contains("context_before"));
    }

    #[test]
    fn max_chars_above_cap_rejected() {
        let req = RepoFetchRequest {
            host: Some(CodeHost::Github),
            owner: "tokio-rs".to_string(),
            repo: "axum".to_string(),
            ref_name: Some("main".to_string()),
            commit_sha: None,
            path: "src/lib.rs".to_string(),
            line_start: None,
            line_end: None,
            context_before: None,
            context_after: None,
            max_chars: Some(60000),
            timeout_ms: None,
            symbol: None,
            symbol_kind: None,
            match_text: None,
            expand_to_block: None,
            max_block_lines: None,
            prefer_local: None,
        };
        let err = req.validate(50000).unwrap_err();
        assert!(err.contains("exceeds server cap"));
    }

    #[test]
    fn zero_max_chars_rejected() {
        let req = RepoFetchRequest {
            host: Some(CodeHost::Github),
            owner: "tokio-rs".to_string(),
            repo: "axum".to_string(),
            ref_name: Some("main".to_string()),
            commit_sha: None,
            path: "src/lib.rs".to_string(),
            line_start: None,
            line_end: None,
            context_before: None,
            context_after: None,
            max_chars: Some(0),
            timeout_ms: None,
            symbol: None,
            symbol_kind: None,
            match_text: None,
            expand_to_block: None,
            max_block_lines: None,
            prefer_local: None,
        };
        let err = req.validate(50000).unwrap_err();
        assert!(err.contains("> 0"));
    }

    #[test]
    fn unsupported_host_rejected() {
        let req = RepoFetchRequest {
            host: Some(CodeHost::Codeberg),
            owner: "owner".to_string(),
            repo: "repo".to_string(),
            ref_name: Some("main".to_string()),
            commit_sha: None,
            path: "src/lib.rs".to_string(),
            line_start: None,
            line_end: None,
            context_before: None,
            context_after: None,
            max_chars: None,
            timeout_ms: None,
            symbol: None,
            symbol_kind: None,
            match_text: None,
            expand_to_block: None,
            max_block_lines: None,
            prefer_local: None,
        };
        let err = req.validate(50000).unwrap_err();
        assert!(err.contains("not supported"));
    }

    // --- URL construction tests ---

    #[test]
    fn github_browser_url_basic() {
        let url = github_browser_url("tokio-rs", "axum", "main", "src/lib.rs");
        assert_eq!(url, "https://github.com/tokio-rs/axum/blob/main/src/lib.rs");
    }

    #[test]
    fn github_raw_url_basic() {
        let url = github_raw_url("tokio-rs", "axum", "main", "src/lib.rs");
        assert_eq!(
            url,
            "https://raw.githubusercontent.com/tokio-rs/axum/main/src/lib.rs"
        );
    }

    #[test]
    fn github_permalink_url_with_sha() {
        let url = github_permalink_url("tokio-rs", "axum", "abc123def", "src/lib.rs");
        assert_eq!(
            url,
            "https://github.com/tokio-rs/axum/blob/abc123def/src/lib.rs"
        );
    }

    #[test]
    fn github_raw_permalink_url_with_sha() {
        let url = github_raw_permalink_url("tokio-rs", "axum", "abc123def", "src/lib.rs");
        assert_eq!(
            url,
            "https://raw.githubusercontent.com/tokio-rs/axum/abc123def/src/lib.rs"
        );
    }

    #[test]
    fn gitlab_browser_url_basic() {
        let url = gitlab_browser_url("group", "project", "main", "src/lib.rs");
        assert_eq!(
            url,
            "https://gitlab.com/group/project/-/blob/main/src/lib.rs"
        );
    }

    #[test]
    fn gitlab_raw_url_basic() {
        let url = gitlab_raw_url("group", "project", "main", "src/lib.rs");
        assert_eq!(
            url,
            "https://gitlab.com/group/project/-/raw/main/src/lib.rs"
        );
    }

    #[test]
    fn gitlab_browser_url_nested_namespace() {
        let url = gitlab_browser_url("group/sub", "project", "main", "src/lib.rs");
        assert_eq!(
            url,
            "https://gitlab.com/group/sub/project/-/blob/main/src/lib.rs"
        );
    }

    #[test]
    fn gitlab_browser_url_with_sha_is_permalink() {
        let sha = "abc123def456";
        let url = gitlab_browser_url("group", "project", sha, "src/lib.rs");
        assert_eq!(
            url,
            "https://gitlab.com/group/project/-/blob/abc123def456/src/lib.rs"
        );
        assert!(url.contains("blob"));
    }

    #[test]
    fn gitlab_raw_url_with_sha_is_raw_permalink() {
        let sha = "abc123def456";
        let url = gitlab_raw_url("group", "project", sha, "src/lib.rs");
        assert_eq!(
            url,
            "https://gitlab.com/group/project/-/raw/abc123def456/src/lib.rs"
        );
        assert!(url.contains("raw"));
    }

    #[test]
    fn gitlab_permalink_urls_are_distinct() {
        let sha = "abc123def456";
        let browser = gitlab_browser_url("g", "p", sha, "src/lib.rs");
        let raw = gitlab_raw_url("g", "p", sha, "src/lib.rs");
        assert_ne!(browser, raw);
        assert!(browser.contains("/blob/"));
        assert!(raw.contains("/raw/"));
    }

    // --- Line range tests ---

    #[test]
    fn apply_line_range_no_range_returns_all() {
        let lines: Vec<String> = (1..=5).map(|n| format!("line {n}")).collect();
        let (sliced, start, end, _truncated, _warn) = apply_line_range(&lines, None, None, 0, 0);
        assert_eq!(sliced.len(), 5);
        assert_eq!(start, Some(1));
        assert_eq!(end, Some(5));
    }

    #[test]
    fn apply_line_range_specific_range() {
        let lines: Vec<String> = (1..=10).map(|n| format!("line {n}")).collect();
        let (sliced, start, end, _truncated, _warn) =
            apply_line_range(&lines, Some(3), Some(5), 0, 0);
        assert_eq!(sliced.len(), 3);
        assert_eq!(start, Some(3));
        assert_eq!(end, Some(5));
        assert_eq!(sliced[0].number, 3);
        assert_eq!(sliced[2].number, 5);
    }

    #[test]
    fn apply_line_range_with_context() {
        let lines: Vec<String> = (1..=10).map(|n| format!("line {n}")).collect();
        let (sliced, start, end, _truncated, _warn) =
            apply_line_range(&lines, Some(3), Some(5), 1, 1);
        assert_eq!(sliced.len(), 5);
        assert_eq!(start, Some(2));
        assert_eq!(end, Some(6));
    }

    #[test]
    fn apply_line_range_context_clamped() {
        let lines: Vec<String> = (1..=5).map(|n| format!("line {n}")).collect();
        let (sliced, start, end, _truncated, _warn) =
            apply_line_range(&lines, Some(1), Some(2), 10, 10);
        assert_eq!(start, Some(1));
        assert_eq!(end, Some(5));
        assert_eq!(sliced.len(), 5);
    }

    #[test]
    fn apply_line_range_empty_lines() {
        let lines: Vec<String> = vec![];
        let (sliced, start, end, _truncated, _warn) = apply_line_range(&lines, None, None, 0, 0);
        assert!(sliced.is_empty());
        assert_eq!(start, None);
        assert_eq!(end, None);
    }

    #[test]
    fn apply_line_range_line_end_clamped() {
        let lines: Vec<String> = (1..=5).map(|n| format!("line {n}")).collect();
        let (sliced, _start, end, truncated, warn) =
            apply_line_range(&lines, Some(3), Some(100), 0, 0);
        assert_eq!(end, Some(5));
        assert_eq!(sliced.len(), 3);
        assert_eq!(sliced[0].number, 3);
        assert_eq!(sliced[2].number, 5);
        assert!(truncated);
        assert!(warn.is_some());
        assert!(warn.unwrap().contains("line_end (100)"));
    }

    #[test]
    fn apply_line_range_line_start_exceeds_file() {
        let lines: Vec<String> = (1..=5).map(|n| format!("line {n}")).collect();
        let (_sliced, _start, _end, truncated, warn) =
            apply_line_range(&lines, Some(10), Some(15), 0, 0);
        assert!(truncated);
        let w = warn.unwrap();
        assert!(w.contains("line_start (10)"));
        assert!(w.contains("line_end (15)"));
    }

    // --- clamp_lines_to_max_chars tests ---

    fn make_fetched_lines(texts: &[&str]) -> Vec<RepoFetchedLine> {
        texts
            .iter()
            .enumerate()
            .map(|(i, t)| RepoFetchedLine {
                number: (i + 1) as u32,
                text: t.to_string(),
            })
            .collect()
    }

    #[test]
    fn clamp_no_budget_returns_all() {
        let lines = make_fetched_lines(&["line 1", "line 2", "line 3"]);
        let (kept, text, truncated) = clamp_lines_to_max_chars(&lines, None);
        assert_eq!(kept.len(), 3);
        assert_eq!(text.as_deref(), Some("line 1\nline 2\nline 3"));
        assert!(!truncated);
    }

    #[test]
    fn clamp_within_budget() {
        let lines = make_fetched_lines(&["hi", "ok"]);
        let (kept, text, truncated) = clamp_lines_to_max_chars(&lines, Some(100));
        assert_eq!(kept.len(), 2);
        assert!(!truncated);
        assert_eq!(text.as_deref(), Some("hi\nok"));
    }

    #[test]
    fn clamp_truncates_from_end() {
        let lines = make_fetched_lines(&["aaa", "bbb", "ccc", "ddd"]);
        // "aaa\nbbb\nccc\nddd" = 3+1+3+1+3+1+3 = 15 chars
        // Budget 7: "aaa\nbbb" = 7 chars (2 lines)
        let (kept, text, truncated) = clamp_lines_to_max_chars(&lines, Some(7));
        assert_eq!(kept.len(), 2);
        assert!(truncated);
        assert_eq!(kept[0].number, 1);
        assert_eq!(kept[1].number, 2);
        assert_eq!(text.as_deref(), Some("aaa\nbbb"));
    }

    #[test]
    fn clamp_empty_lines() {
        let (kept, text, truncated) = clamp_lines_to_max_chars(&[], Some(100));
        assert!(kept.is_empty());
        assert_eq!(text, None);
        assert!(!truncated);
    }

    #[test]
    fn clamp_no_line_fits_returns_prefix() {
        let lines = make_fetched_lines(&["this is a very long line"]);
        let (kept, text, truncated) = clamp_lines_to_max_chars(&lines, Some(5));
        assert_eq!(kept.len(), 1);
        assert!(truncated);
        assert_eq!(kept[0].text, "this ");
        assert_eq!(text.as_deref(), Some("this "));
    }

    #[test]
    fn clamp_preserves_line_numbers() {
        let lines = make_fetched_lines(&["a", "b", "c", "d", "e"]);
        let (kept, _, truncated) = clamp_lines_to_max_chars(&lines, Some(5));
        // "a\nb\nc" = 5 chars (3 lines)
        assert_eq!(kept.len(), 3);
        assert_eq!(kept[0].number, 1);
        assert_eq!(kept[2].number, 3);
        assert!(truncated);
    }

    #[test]
    fn clamp_single_line_within_budget() {
        let lines = make_fetched_lines(&["hello"]);
        let (kept, text, truncated) = clamp_lines_to_max_chars(&lines, Some(100));
        assert_eq!(kept.len(), 1);
        assert!(!truncated);
        assert_eq!(text.as_deref(), Some("hello"));
    }
}
