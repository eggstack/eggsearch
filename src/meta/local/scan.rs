use std::collections::HashSet;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use crate::process::{run_bounded_command, CommandTermination};

pub(crate) use crate::core::local::{
    GIT_GREP_MAX_PATTERNS, GIT_GREP_STDOUT_CAP, GIT_GREP_TIMEOUT, SECOND_PASS_MAX_BYTES,
    SECOND_PASS_MAX_FILES, SECOND_PASS_TIME_BUDGET,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ScanOutcome {
    Exhausted,
    CutShort,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn should_run_second_pass(
    matches: usize,
    max_results: usize,
    timed_out: bool,
    files_scanned: usize,
    max_indexed_files: usize,
    query_lower: &str,
    path_hint: Option<&str>,
    file_hint: Option<&str>,
    symbol_hint: Option<&str>,
) -> bool {
    matches < max_results
        && !timed_out
        && files_scanned < max_indexed_files
        && (!query_lower.is_empty()
            || path_hint.is_some_and(|hint| !hint.is_empty())
            || file_hint.is_some_and(|hint| !hint.is_empty())
            || symbol_hint.is_some_and(|hint| !hint.is_empty()))
}

pub(crate) fn should_skip_git_candidate(
    relative_path: &str,
    query_tokens: &[&str],
    hint_paths: Option<&HashSet<String>>,
    path_hint: Option<&str>,
    file_hint: Option<&str>,
) -> bool {
    let Some(hint_paths) = hint_paths else {
        return false;
    };
    if hint_paths.contains(relative_path)
        || query_tokens
            .iter()
            .any(|token| relative_path.to_lowercase().contains(token))
        || path_hint.is_some_and(|hint| !hint.is_empty())
        || file_hint.is_some_and(|hint| !hint.is_empty())
    {
        return false;
    }
    true
}

pub(crate) fn git_literal_hint(
    root: &Path,
    query_lower: &str,
    query_tokens: &[&str],
    symbol_hint: Option<&str>,
    respect_gitignore: bool,
    timeout: Duration,
) -> Option<HashSet<String>> {
    let mut patterns = Vec::new();
    if !query_lower.is_empty() {
        patterns.push(query_lower.to_string());
    }
    patterns.extend(query_tokens.iter().map(|token| (*token).to_string()));
    if let Some(symbol) = symbol_hint.filter(|hint| !hint.is_empty()) {
        patterns.push(symbol.to_string());
    }
    if patterns.is_empty() || patterns.len() > GIT_GREP_MAX_PATTERNS {
        return None;
    }

    let mut command = Command::new("git");
    command
        .arg("grep")
        .arg("-l")
        .arg("-z")
        .arg("-i")
        .arg("-F")
        .arg("--untracked");
    if respect_gitignore {
        command.arg("--exclude-standard");
    } else {
        command.arg("--no-exclude-standard");
    }
    for pattern in patterns {
        command.arg("-e").arg(pattern);
    }
    command.arg("--").current_dir(root);

    let result = run_bounded_command(&mut command, timeout, GIT_GREP_STDOUT_CAP, 64 * 1024);
    let exit_code = result
        .status
        .as_ref()
        .and_then(std::process::ExitStatus::code);
    let completed = matches!(exit_code, Some(0 | 1));
    if result.termination != CommandTermination::Exited
        || result.timed_out
        || result.stdout_truncated
        || result.stdout.len() >= GIT_GREP_STDOUT_CAP
        || !completed
    {
        return None;
    }

    Some(
        result
            .stdout
            .split(|byte| *byte == 0)
            .filter(|path| !path.is_empty())
            .map(|path| String::from_utf8_lossy(path).into_owned())
            .collect(),
    )
}
