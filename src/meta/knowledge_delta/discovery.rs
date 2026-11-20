use crate::core::identity::{write_entity_prefix, write_str, FnvHasher};

const GENERIC: &[&str] = &[
    "announcement",
    "breaking",
    "change",
    "changes",
    "changelog",
    "com",
    "compatibility",
    "deprecation",
    "docs",
    "for",
    "github",
    "gitlab",
    "http",
    "https",
    "library",
    "migration",
    "new",
    "org",
    "package",
    "python",
    "release",
    "rust",
    "the",
    "update",
    "version",
    "with",
    "www",
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExtractedSubject {
    pub kind: &'static str,
    pub token: String,
    pub span: String,
    pub verified: bool,
}

pub fn edge_id(source_id: &str, from: &str, to: &str, span: &str) -> String {
    let mut hasher = FnvHasher::new();
    write_entity_prefix(&mut hasher, "kedge");
    write_str(&mut hasher, source_id);
    write_str(&mut hasher, from);
    write_str(&mut hasher, to);
    write_str(&mut hasher, span);
    format!("kedge_{:016x}", hasher.finish())
}

pub fn subject_id(kind: &str, token: &str) -> String {
    let mut hasher = FnvHasher::new();
    write_entity_prefix(&mut hasher, "ksubj");
    write_str(&mut hasher, kind);
    write_str(&mut hasher, token);
    format!("ksubj_{:016x}", hasher.finish())
}

pub fn extract_subjects(title: &str, excerpt: &str) -> Vec<ExtractedSubject> {
    let mut found = Vec::new();
    collect_field("title", title, &mut found);
    collect_field("excerpt", excerpt, &mut found);
    found.sort_by(|left, right| {
        left.kind
            .cmp(right.kind)
            .then(left.token.cmp(&right.token))
            .then(left.span.cmp(&right.span))
    });
    found.dedup_by(|left, right| left.kind == right.kind && left.token == right.token);
    found
}

fn collect_field(field: &str, text: &str, out: &mut Vec<ExtractedSubject>) {
    collect_backticks(field, text, out);
    collect_pkg(field, text, out);
    collect_repos(field, text, out);
    collect_versions(field, text, out);
    if field == "title" {
        collect_title_candidates(field, text, out);
    }
}

fn collect_backticks(field: &str, text: &str, out: &mut Vec<ExtractedSubject>) {
    let bytes = text.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'`' {
            index += 1;
            continue;
        }
        let start = index + 1;
        let Some(end) = text[start..].find('`') else {
            break;
        };
        let token = &text[start..start + end];
        push_token(
            "package",
            token,
            format!("{field}:{start}-{}", start + end),
            true,
            out,
        );
        index = start + end + 1;
    }
}

fn collect_pkg(field: &str, text: &str, out: &mut Vec<ExtractedSubject>) {
    let marker = "pkg:";
    let mut rest = text;
    let mut base = 0;
    while let Some(at) = rest.find(marker) {
        let start = base + at + marker.len();
        let tail = &text[start..];
        let len = tail
            .find(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_' || ch == '.' || ch == '-'))
            .unwrap_or(tail.len());
        if len > 0 {
            push_token(
                "package",
                &text[start..start + len],
                format!("{field}:{start}-{}", start + len),
                true,
                out,
            );
        }
        let next = start + len;
        rest = &text[next..];
        base = next;
    }
}

fn collect_repos(field: &str, text: &str, out: &mut Vec<ExtractedSubject>) {
    for host in ["github.com/", "gitlab.com/"] {
        let mut rest = text;
        let mut base = 0;
        while let Some(at) = rest.find(host) {
            let start = base + at;
            let tail = &text[start + host.len()..];
            let Some((owner, repo)) = split_repo(tail) else {
                let next = start + host.len();
                rest = &text[next..];
                base = next;
                continue;
            };
            let token = format!("{host}{owner}/{repo}");
            let end = start + token.len();
            push_token(
                "repository",
                &token,
                format!("{field}:{start}-{end}"),
                true,
                out,
            );
            rest = &text[end..];
            base = end;
        }
    }
}

fn collect_versions(field: &str, text: &str, out: &mut Vec<ExtractedSubject>) {
    let bytes = text.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if !bytes[index].is_ascii_alphanumeric() {
            index += 1;
            continue;
        }
        let start = index;
        while index < bytes.len()
            && (bytes[index].is_ascii_alphanumeric() || matches!(bytes[index], b'.' | b'-' | b'+'))
        {
            index += 1;
        }
        let token = &text[start..index];
        let prefix = text[..start].trim_end();
        let version_marker = prefix
            .rsplit(|ch: char| !ch.is_ascii_alphanumeric())
            .next()
            .is_some_and(|word| word.eq_ignore_ascii_case("version"));
        let version_prefix = token.strip_prefix('v').or_else(|| token.strip_prefix('V'));
        let Some(version) = version_prefix
            .filter(|value| valid_version(value))
            .or_else(|| {
                version_marker
                    .then_some(token)
                    .filter(|value| valid_version(value))
            })
        else {
            continue;
        };
        let start_offset = if version_prefix.is_some() {
            start + 1
        } else {
            start
        };
        out.push(ExtractedSubject {
            kind: "version",
            token: version.to_string(),
            span: format!("{field}:{start_offset}-{index}"),
            verified: true,
        });
    }
}

fn valid_version(value: &str) -> bool {
    let core = value.split(['-', '+']).next().unwrap_or_default();
    let components: Vec<_> = core.split('.').collect();
    components.len() >= 3
        && components
            .iter()
            .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b".-+".contains(&byte))
}

fn collect_title_candidates(field: &str, text: &str, out: &mut Vec<ExtractedSubject>) {
    let bytes = text.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if !(bytes[index].is_ascii_alphanumeric() || matches!(bytes[index], b'_' | b'-' | b'.')) {
            index += 1;
            continue;
        }
        let start = index;
        while index < bytes.len()
            && (bytes[index].is_ascii_alphanumeric() || matches!(bytes[index], b'_' | b'-' | b'.'))
        {
            index += 1;
        }
        let token = &text[start..index];
        if token.bytes().any(|byte| matches!(byte, b'_' | b'-' | b'.'))
            && !token
                .strip_prefix('v')
                .or_else(|| token.strip_prefix('V'))
                .is_some_and(valid_version)
        {
            push_token(
                "package",
                token,
                format!("{field}:{start}-{index}"),
                false,
                out,
            );
        }
    }
}

fn split_repo(tail: &str) -> Option<(&str, &str)> {
    let slash = tail.find('/')?;
    let owner = &tail[..slash];
    if !is_repo_part(owner) {
        return None;
    }
    let after = &tail[slash + 1..];
    let len = after
        .find(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_' || ch == '.' || ch == '-'))
        .unwrap_or(after.len());
    let repo = &after[..len];
    if !is_repo_part(repo) {
        return None;
    }
    Some((owner, repo))
}

fn is_repo_part(value: &str) -> bool {
    let mut chars = value.chars();
    matches!(chars.next(), Some(ch) if ch.is_ascii_alphanumeric())
        && value.len() <= 64
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '.' || ch == '-')
}

fn push_token(
    kind: &'static str,
    token: &str,
    span: String,
    verified: bool,
    out: &mut Vec<ExtractedSubject>,
) {
    let token = token.trim();
    if token.len() < 2 || token.len() > 80 || is_generic(token) || token.contains(' ') {
        return;
    }
    if !token
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_alphabetic())
    {
        return;
    }
    out.push(ExtractedSubject {
        kind,
        token: token.to_string(),
        span,
        verified,
    });
}

fn is_generic(token: &str) -> bool {
    GENERIC
        .binary_search(&token.to_ascii_lowercase().as_str())
        .is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generic_cooccurrence_is_not_an_edge_subject() {
        let found = extract_subjects("release compatibility update", "the package version");
        assert!(found.is_empty());
    }

    #[test]
    fn backtick_package_keeps_a_span() {
        let tick = '`';
        let excerpt = format!("see {tick}left-pad{tick} wheels");
        let found = extract_subjects("widget-runtime notes", &excerpt);
        let package = found
            .iter()
            .find(|subject| subject.kind == "package" && subject.token == "left-pad")
            .expect("backtick package should be extracted");
        assert!(package.verified);
        assert!(package.span.starts_with("excerpt:"));
    }
}
