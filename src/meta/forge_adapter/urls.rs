use super::*;

/// Compute browser and raw URLs for a tree entry based on the host.
///
/// For GitHub, immutable permalinks use `commit_sha` (the resolved commit
/// SHA), not the entry's `object_sha` (blob/tree SHA). For Gitea/Forgejo,
/// `gitea_base_url` should be the instance root URL
/// (e.g. `https://gitea.example.com`), not the API base.
///
/// Directory entries do not receive raw-file URLs.
#[allow(clippy::too_many_arguments)]
pub(super) fn build_entry_urls(
    host: CodeHost,
    owner: &str,
    repo: &str,
    ref_name: &str,
    commit_sha: Option<&str>,
    object_sha: Option<&str>,
    path: &str,
    kind: EntryKind,
    gitea_base_url: Option<&str>,
) -> (Option<String>, Option<String>) {
    if kind == EntryKind::Directory {
        let browser = match host {
            CodeHost::Github => {
                let r = commit_sha.unwrap_or(ref_name);
                github_browser_url(owner, repo, r, path)
            }
            CodeHost::Gitlab => {
                let r = commit_sha.unwrap_or(ref_name);
                gitlab_browser_url(owner, repo, r, path)
            }
            CodeHost::Codeberg => {
                let r = commit_sha.unwrap_or(ref_name);
                codeberg_browser_url(owner, repo, r, path)
            }
            CodeHost::Gitea | CodeHost::Forgejo => {
                let r = commit_sha.unwrap_or(ref_name);
                if let Some(base) = gitea_base_url {
                    gitea_browser_url(base, owner, repo, r, path)
                } else {
                    String::new()
                }
            }
            CodeHost::Unknown => String::new(),
        };
        let browser_opt = if browser.is_empty() {
            None
        } else {
            Some(browser)
        };
        return (browser_opt, None);
    }

    let (browser, raw) = match host {
        CodeHost::Github => {
            if let Some(sha) = commit_sha {
                (
                    github_permalink_url(owner, repo, sha, path),
                    github_raw_permalink_url(owner, repo, sha, path),
                )
            } else {
                (
                    github_browser_url(owner, repo, ref_name, path),
                    github_raw_url(owner, repo, ref_name, path),
                )
            }
        }
        CodeHost::Gitlab => {
            if let Some(sha) = commit_sha {
                (
                    gitlab_browser_url(owner, repo, sha, path),
                    gitlab_raw_url(owner, repo, sha, path),
                )
            } else {
                (
                    gitlab_browser_url(owner, repo, ref_name, path),
                    gitlab_raw_url(owner, repo, ref_name, path),
                )
            }
        }
        CodeHost::Codeberg => {
            if let Some(sha) = commit_sha {
                (
                    codeberg_browser_url(owner, repo, sha, path),
                    codeberg_raw_url(owner, repo, sha, path),
                )
            } else {
                (
                    codeberg_browser_url(owner, repo, ref_name, path),
                    codeberg_raw_url(owner, repo, ref_name, path),
                )
            }
        }
        CodeHost::Gitea | CodeHost::Forgejo => {
            let ref_or_commit = commit_sha.unwrap_or(ref_name);
            if let Some(base) = gitea_base_url {
                (
                    gitea_browser_url(base, owner, repo, ref_or_commit, path),
                    gitea_raw_url(base, owner, repo, ref_or_commit, path),
                )
            } else {
                (String::new(), String::new())
            }
        }
        CodeHost::Unknown => (String::new(), String::new()),
    };
    let _ = object_sha;
    let browser_opt = if browser.is_empty() {
        None
    } else {
        Some(browser)
    };
    let raw_opt = if raw.is_empty() { None } else { Some(raw) };
    (browser_opt, raw_opt)
}

/// Derive the Gitea/Forgejo instance root URL from an API base URL.
///
/// E.g. `https://gitea.example.com/api/v1` → `https://gitea.example.com`.
pub fn derive_gitea_instance_root(api_base: &str) -> String {
    let base = api_base.trim_end_matches('/');
    if let Some(pos) = base.rfind("/api") {
        let root = &base[..pos];
        if crate::meta::engines::is_http_url(root) {
            return root.to_string();
        }
    }
    base.to_string()
}
