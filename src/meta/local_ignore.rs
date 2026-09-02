//! Minimal `.gitignore` matcher used by the local workspace backend.
//!
//! Implements a deliberately small subset of the gitignore spec
//! sufficient for typical workspace trees:
//!
//! - Comments (`#`) and blank lines
//! - Negation (`!`)
//! - Anchored patterns (leading `/`)
//! - Directory-only patterns (trailing `/`)
//! - `*` and `**` glob wildcards (single-segment and any-segment)
//! - `?` single-character wildcard
//!
//! Patterns are evaluated relative to the directory containing the
//! `.gitignore` file. Nested `.gitignore` files are supported.
//!
//! This is intentionally not a full gitignore implementation. The
//! goal is to honor common ignore patterns (target/, node_modules/,
//! *.log, etc.) without pulling in an external dependency.

use std::path::{Path, PathBuf};

/// One parsed `.gitignore` rule.
#[derive(Debug, Clone)]
pub(crate) struct IgnoreRule {
    /// Original pattern text (without `!` or trailing `/`).
    #[allow(dead_code)]
    pattern: String,
    regex: Option<regex::Regex>,
    /// Whether the rule negates a previous match.
    negate: bool,
    /// Whether the rule only applies to directories.
    dir_only: bool,
    /// Whether the pattern is anchored to the directory containing the
    /// `.gitignore` file (i.e. starts with `/`).
    anchored: bool,
}

impl IgnoreRule {
    fn parse(line: &str) -> Option<Self> {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            return None;
        }
        let (rest, negate) = if let Some(stripped) = trimmed.strip_prefix('!') {
            (stripped, true)
        } else {
            (trimmed, false)
        };
        let (rest, dir_only) = if let Some(stripped) = rest.strip_suffix('/') {
            (stripped, true)
        } else {
            (rest, false)
        };
        let (rest, anchored) = if let Some(stripped) = rest.strip_prefix('/') {
            (stripped, true)
        } else {
            (rest, false)
        };
        if rest.is_empty() {
            return None;
        }
        let regex_src = glob_to_regex(rest);
        let regex = regex::Regex::new(&format!("^(?:{regex_src})$")).ok();
        Some(Self {
            pattern: rest.to_string(),
            regex,
            negate,
            dir_only,
            anchored,
        })
    }

    fn matches(&self, rel_path: &str, is_dir: bool) -> bool {
        if self.dir_only && !is_dir {
            return false;
        }
        let path = rel_path.trim_start_matches('/');
        let regex = match &self.regex {
            Some(r) => r,
            None => return false,
        };
        if self.anchored {
            regex.is_match(path)
        } else {
            if regex.is_match(path) {
                return true;
            }
            for component in path.split('/') {
                if regex.is_match(component) {
                    return true;
                }
            }
            false
        }
    }
}

fn glob_to_regex(glob: &str) -> String {
    let mut out = String::with_capacity(glob.len() * 2);
    let chars: Vec<char> = glob.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            '*' => {
                if i + 1 < chars.len() && chars[i + 1] == '*' {
                    // `**` matches any path component(s); consume a
                    // following `/` if present.
                    out.push_str(".*");
                    i += 2;
                    if i < chars.len() && chars[i] == '/' {
                        i += 1;
                    }
                } else {
                    // `*` matches any chars except `/`.
                    out.push_str("[^/]*");
                    i += 1;
                }
            }
            '?' => {
                out.push_str("[^/]");
                i += 1;
            }
            '.' | '(' | ')' | '+' | '|' | '^' | '$' | '{' | '}' | '\\' => {
                out.push('\\');
                out.push(c);
                i += 1;
            }
            '[' => {
                out.push('[');
                i += 1;
            }
            _ => {
                out.push(c);
                i += 1;
            }
        }
    }
    out
}

/// A `.gitignore` ruleset scoped to a single directory. Rules from
/// parent `.gitignore` files compose on top via [`IgnoreStack`].
#[derive(Debug, Default, Clone)]
pub(crate) struct IgnoreSet {
    rules: Vec<IgnoreRule>,
}

impl IgnoreSet {
    pub fn from_text(text: &str) -> Self {
        let rules = text
            .lines()
            .filter_map(IgnoreRule::parse)
            .collect::<Vec<_>>();
        Self { rules }
    }

    pub fn from_file(path: &Path) -> Self {
        match std::fs::read_to_string(path) {
            Ok(text) => Self::from_text(&text),
            Err(_) => Self::default(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    /// Returns true if the given relative path is ignored by this set.
    pub fn is_ignored(&self, rel_path: &str, is_dir: bool) -> bool {
        let mut ignored = false;
        for rule in &self.rules {
            if rule.matches(rel_path, is_dir) {
                ignored = !rule.negate;
            }
        }
        ignored
    }

    /// Returns true if the set has any rule whose pattern matches
    /// the given path, regardless of negation. Used by
    /// [`IgnoreStack`] to decide whether this set has a verdict that
    /// overrides parent `.gitignore` files.
    pub fn has_matching_rule(&self, rel_path: &str, is_dir: bool) -> bool {
        self.rules.iter().any(|r| r.matches(rel_path, is_dir))
    }
}

/// Stack of `.gitignore` rules discovered while walking from a
/// workspace root to a given directory.
#[derive(Debug, Default, Clone)]
pub(crate) struct IgnoreStack {
    entries: Vec<(PathBuf, IgnoreSet)>,
}

impl IgnoreStack {
    /// Build an empty stack.
    pub fn new() -> Self {
        Self::default()
    }

    /// Walk up to `dir` from `root`, collecting any `.gitignore`
    /// files encountered. The nearest `.gitignore` rules take
    /// precedence.
    pub fn build(root: &Path, dir: &Path) -> Self {
        let mut entries: Vec<(PathBuf, IgnoreSet)> = Vec::new();
        let mut current: Option<PathBuf> = Some(dir.to_path_buf());
        while let Some(path) = current {
            let candidate = path.join(".gitignore");
            if candidate.is_file() {
                let set = IgnoreSet::from_file(&candidate);
                if !set.is_empty() {
                    entries.push((path.clone(), set));
                }
            }
            // Stop when we go above the root.
            if path == root || !path.starts_with(root) {
                break;
            }
            current = path.parent().map(Path::to_path_buf);
        }
        Self { entries }
    }

    /// Returns true if the given absolute path is ignored. `abs_path`
    /// must be inside `root`.
    pub fn is_ignored(&self, root: &Path, abs_path: &Path, is_dir: bool) -> bool {
        let rel = match abs_path.strip_prefix(root) {
            Ok(p) => p.to_string_lossy().replace('\\', "/"),
            Err(_) => return false,
        };
        // Walk closest-to-furthest. The closest `.gitignore` with any
        // matching rule decides the verdict, including any explicit
        // negation.
        for (_, set) in self.entries.iter() {
            if set.has_matching_rule(&rel, is_dir) {
                return set.is_ignored(&rel, is_dir);
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_skips_blank_and_comment() {
        let set = IgnoreSet::from_text("\n# comment\n\n");
        assert!(set.is_empty());
    }

    #[test]
    fn parse_skips_empty_after_strip() {
        let set = IgnoreSet::from_text("!\n");
        assert!(set.is_empty());
    }

    #[test]
    fn literal_match_anchored_dir() {
        let set = IgnoreSet::from_text("/target");
        assert!(set.is_ignored("target", true));
        assert!(!set.is_ignored("src/target", true));
    }

    #[test]
    fn unanchored_literal_matches_any_segment() {
        let set = IgnoreSet::from_text("node_modules");
        assert!(set.is_ignored("node_modules", true));
        assert!(set.is_ignored("packages/node_modules", true));
        assert!(!set.is_ignored("node_modules_alt", true));
    }

    #[test]
    fn trailing_slash_marks_dir_only() {
        let set = IgnoreSet::from_text("build/");
        assert!(set.is_ignored("build", true));
        assert!(!set.is_ignored("build", false));
        assert!(!set.is_ignored("build/main.rs", false));
    }

    #[test]
    fn star_wildcard_matches_extension() {
        let set = IgnoreSet::from_text("*.log");
        assert!(set.is_ignored("debug.log", false));
        assert!(!set.is_ignored("debug.txt", false));
    }

    #[test]
    fn double_star_matches_nested() {
        let set = IgnoreSet::from_text("**/generated/**");
        assert!(set.is_ignored("generated/x.rs", false));
        assert!(set.is_ignored("src/generated/x.rs", false));
    }

    #[test]
    fn negation_unignores() {
        let set = IgnoreSet::from_text("*.log\n!important.log");
        assert!(set.is_ignored("debug.log", false));
        assert!(!set.is_ignored("important.log", false));
    }

    #[test]
    fn dir_only_does_not_match_file() {
        let set = IgnoreSet::from_text("dist/");
        assert!(set.is_ignored("dist", true));
        assert!(!set.is_ignored("dist", false));
    }

    #[test]
    fn stack_nearest_rules_win() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        std::fs::create_dir(root.join("src")).unwrap();
        std::fs::write(root.join(".gitignore"), "*.tmp\n").unwrap();
        std::fs::write(root.join("src/.gitignore"), "!keep.tmp\n").unwrap();
        std::fs::write(root.join("src/keep.tmp"), "kept").unwrap();
        std::fs::write(root.join("src/other.tmp"), "dropped").unwrap();

        let stack = IgnoreStack::build(root, &root.join("src"));
        assert!(!stack.is_ignored(root, &root.join("src/keep.tmp"), false));
        assert!(stack.is_ignored(root, &root.join("src/other.tmp"), false));
    }
}
