use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

const SKIPPED_DIRECTORIES: [&str; 3] = ["bindings", ".git", "target"];

#[test]
fn in_repository_markdown_fragments_resolve() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    collect_markdown(&root, &mut files);
    files.sort();

    let mut failures = Vec::new();
    for file in &files {
        let markdown = fs::read_to_string(file)
            .unwrap_or_else(|error| panic!("read {}: {error}", file.display()));
        let parent = file.parent().unwrap_or(&root);
        for raw in link_targets(&markdown) {
            let Some((relative, fragment)) = relative_fragment(&raw) else {
                continue;
            };
            let target = if relative.as_os_str().is_empty() {
                file.clone()
            } else {
                parent.join(relative)
            };
            let label = format!(
                "{} -> {raw}",
                file.strip_prefix(&root).unwrap_or(file).display()
            );
            if !target.exists() {
                failures.push(format!("{label}: missing file"));
                continue;
            }
            if target.extension().and_then(|extension| extension.to_str()) != Some("md") {
                continue;
            }
            let target_markdown = fs::read_to_string(&target)
                .unwrap_or_else(|error| panic!("read {}: {error}", target.display()));
            if !heading_slugs(&target_markdown).contains(&fragment) {
                failures.push(format!("{label}: missing fragment"));
            }
        }
    }

    assert!(
        failures.is_empty(),
        "unresolved in-repository Markdown fragments:\n{}",
        failures.join("\n")
    );
}

fn collect_markdown(directory: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            let name = entry.file_name();
            if name
                .to_str()
                .is_some_and(|name| SKIPPED_DIRECTORIES.contains(&name))
            {
                continue;
            }
            collect_markdown(&entry.path(), files);
        } else if entry
            .path()
            .extension()
            .and_then(|extension| extension.to_str())
            == Some("md")
        {
            files.push(entry.path());
        }
    }
}

fn relative_fragment(raw: &str) -> Option<(PathBuf, String)> {
    let target = raw.split_whitespace().next()?;
    if target.starts_with("//") || has_scheme(target) {
        return None;
    }
    let (path, fragment) = target.split_once('#')?;
    if fragment.is_empty() {
        return None;
    }
    Some((PathBuf::from(path), fragment.to_string()))
}

fn has_scheme(target: &str) -> bool {
    target.split_once(':').is_some_and(|(scheme, _)| {
        !scheme.is_empty()
            && scheme.chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '+' | '-' | '.')
            })
    })
}

fn link_targets(markdown: &str) -> Vec<String> {
    let mut targets = Vec::new();
    for line in non_fenced_lines(markdown) {
        let line = strip_inline_code(line);
        targets.extend(targets_in_line(&line));
    }
    targets
}

fn non_fenced_lines(markdown: &str) -> Vec<&str> {
    let mut lines = Vec::new();
    let mut fence: Option<(char, usize)> = None;
    for line in markdown.lines() {
        if let Some((marker, length, bare)) = fence_marker(line) {
            match fence {
                None => {
                    fence = Some((marker, length));
                    continue;
                }
                Some((open_marker, open_length))
                    if marker == open_marker && length >= open_length && bare =>
                {
                    fence = None;
                    continue;
                }
                _ => {}
            }
        }
        if fence.is_none() {
            lines.push(line);
        }
    }
    lines
}

fn fence_marker(line: &str) -> Option<(char, usize, bool)> {
    let trimmed = line.trim_start_matches(' ');
    if line.len() - trimmed.len() > 3 {
        return None;
    }
    let marker = trimmed.chars().next()?;
    if marker != '`' && marker != '~' {
        return None;
    }
    let length = trimmed
        .chars()
        .take_while(|character| *character == marker)
        .count();
    if length < 3 {
        return None;
    }
    Some((marker, length, trimmed[length..].trim().is_empty()))
}

fn strip_inline_code(line: &str) -> String {
    let mut stripped = String::new();
    let mut in_code = false;
    for character in line.chars() {
        if character == '`' {
            in_code = !in_code;
            continue;
        }
        if !in_code {
            stripped.push(character);
        }
    }
    stripped
}

fn targets_in_line(line: &str) -> Vec<String> {
    let mut targets = Vec::new();
    let mut rest = line;
    while let Some(start) = rest.find("](") {
        let after = &rest[start + 2..];
        let Some(end) = after.find(')') else {
            break;
        };
        targets.push(after[..end].trim().to_string());
        rest = &after[end + 1..];
    }
    targets
}

fn heading_slugs(markdown: &str) -> HashSet<String> {
    let mut slugs = HashSet::new();
    let mut counts: HashMap<String, usize> = HashMap::new();
    for line in non_fenced_lines(markdown) {
        let Some(heading) = heading_text(line) else {
            continue;
        };
        let base = slugify(heading);
        let count = counts.entry(base.clone()).or_insert(0);
        let slug = if *count == 0 {
            base.clone()
        } else {
            format!("{base}-{count}")
        };
        *count += 1;
        slugs.insert(slug);
    }
    slugs
}

fn heading_text(line: &str) -> Option<&str> {
    let trimmed = line.trim_start_matches(' ');
    if line.len() - trimmed.len() > 3 {
        return None;
    }
    let level = trimmed
        .chars()
        .take_while(|character| *character == '#')
        .count();
    if level == 0 || level > 6 {
        return None;
    }
    let rest = &trimmed[level..];
    if !rest.starts_with(' ') && !rest.starts_with('\t') {
        return None;
    }
    Some(rest.trim())
}

fn slugify(heading: &str) -> String {
    let text = heading.trim().trim_start_matches('#').trim();
    let text = text.trim_end_matches('#').trim_end();
    let mut slug = String::new();
    for character in text.to_lowercase().chars() {
        if character.is_alphanumeric() || character == ' ' || character == '-' || character == '_' {
            slug.push(character);
        }
    }
    slug.replace(' ', "-")
}

fn unresolved_same_file(markdown: &str) -> Vec<String> {
    let slugs = heading_slugs(markdown);
    link_targets(markdown)
        .iter()
        .filter_map(|raw| relative_fragment(raw))
        .filter(|(path, _)| path.as_os_str().is_empty())
        .map(|(_, fragment)| fragment)
        .filter(|fragment| !slugs.contains(fragment))
        .collect()
}

#[test]
fn synthetic_broken_fragment_is_reported() {
    assert_eq!(
        unresolved_same_file("# Present\n\n[broken](#missing)\n"),
        vec!["missing".to_string()]
    );
}

#[test]
fn fenced_code_fragment_is_ignored() {
    assert!(unresolved_same_file("# Present\n\n```\n[broken](#missing)\n```\n").is_empty());
}

#[test]
fn inline_code_fragment_is_ignored() {
    assert!(unresolved_same_file("# Present\n\n`[broken](#missing)`\n").is_empty());
}

#[test]
fn matching_fragment_resolves() {
    assert!(unresolved_same_file("# Present\n\n[ok](#present)\n").is_empty());
}

#[test]
fn duplicate_headings_receive_github_style_suffixes() {
    let slugs = heading_slugs("# Same\n\n## Same\n");
    assert!(slugs.contains("same"));
    assert!(slugs.contains("same-1"));
}

#[test]
fn punctuation_is_dropped_from_headings() {
    assert_eq!(
        slugify("Legacy call compatibility and web field aliases"),
        "legacy-call-compatibility-and-web-field-aliases"
    );
    assert_eq!(
        slugify("Keyless-tier degradation runbook"),
        "keyless-tier-degradation-runbook"
    );
}
