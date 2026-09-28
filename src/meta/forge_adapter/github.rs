use super::*;

pub(super) async fn fetch_github_tree(
    client: &Client,
    owner: &str,
    repo: &str,
    req: &RepoMapRequest,
    config: &ForgeTreeConfig,
    timeout: Duration,
) -> Result<ForgeTreeResponse, String> {
    let base = config.base_url.as_deref().unwrap_or(GITHUB_API_BASE);
    let ref_name = req.ref_name.as_deref().unwrap_or("HEAD");
    let max_d = max_depth(req);
    let max_e = max_entries(req);
    let aggregate_limit = config
        .forge_budget_limit
        .unwrap_or(DEFAULT_MAX_RESPONSE_BYTES);
    let mut budget = ForgeReadBudget::new(aggregate_limit);

    let mut identity = ResolvedRepositoryIdentity {
        requested_ref: Some(ref_name.to_string()),
        ..Default::default()
    };

    let (commit_sha, tree_sha) =
        resolve_github_commit(client, owner, repo, ref_name, config, timeout, &mut budget).await;

    identity.resolved_commit_sha = commit_sha.clone();
    identity.tree_sha = tree_sha.clone();

    let default_branch =
        resolve_github_default_branch(client, owner, repo, config, timeout, &mut budget).await;
    identity.default_branch = default_branch.clone();

    let tree_ref = tree_sha.as_deref().unwrap_or(ref_name);

    let tree_url = format!(
        "{base}/repos/{}/{}/git/trees/{}",
        encode_url_component(owner),
        encode_url_component(repo),
        encode_url_component(tree_ref)
    );
    let mut builder = client
        .get(tree_url.as_str())
        .map_err(|e| format!("GitHub API request failed: {e}"))?
        .query("recursive", if max_d > 1 { "1" } else { "0" })
        .timeout(forge_timeout(timeout));

    if let Some(ref key) = config.api_key {
        builder = builder.header("Authorization", &format!("Bearer {key}"));
    }

    let resp = builder
        .send()
        .await
        .map_err(|e| format!("GitHub API request failed: {e}"))?;

    let status = resp.status();
    if status.as_u16() == 404 {
        return Err("repository_not_found".into());
    }
    if status.as_u16() == 401 {
        return Err("authentication_required".into());
    }
    if status.as_u16() == 403 {
        let msg = read_error_body_preview(resp, &mut budget).await;
        if msg.contains("rate limit") || msg.contains("Rate limit") {
            return Err("rate_limited".into());
        }
        return Err("permission_denied".into());
    }
    if !status.is_success() {
        let msg = read_error_body_preview(resp, &mut budget).await;
        return Err(format!("provider_unavailable: {status} - {msg}"));
    }

    let body = read_with_budget(resp, &mut budget, ForgeRequestKind::TreePage)
        .await
        .map_err(|e| e.as_static_str().to_string())?;

    let body_str = std::str::from_utf8(&body)
        .map(|s| s.to_owned())
        .map_err(|_| "invalid_utf8".to_string())?;

    let tree: GitHubTreeResponse =
        serde_json::from_str(&body_str).map_err(|e| format!("malformed response: {e}"))?;

    let truncated_by_provider = tree.truncated.unwrap_or(false);

    let mut entries: Vec<ForgeRawEntry> = tree
        .tree
        .into_iter()
        .map(|item| {
            let kind = match item.type_field.as_str() {
                "blob" => {
                    if item.mode.as_deref() == Some("120000") {
                        EntryKind::Symlink
                    } else {
                        EntryKind::File
                    }
                }
                "tree" => EntryKind::Directory,
                "commit" => EntryKind::Submodule,
                _ => EntryKind::File,
            };
            ForgeRawEntry {
                path: item.path,
                kind,
                size: item.size,
                object_sha: item.sha,
            }
        })
        .collect();

    if truncated_by_provider && !budget.exceeded() {
        if let Ok(fallback) =
            fetch_github_contents_root(client, owner, repo, config, timeout, tree_ref, &mut budget)
                .await
        {
            let existing_paths: std::collections::HashSet<String> =
                entries.iter().map(|e| e.path.clone()).collect();
            for entry in fallback {
                if !existing_paths.contains(&entry.path) {
                    entries.push(entry);
                }
            }
        }
    }

    let mut truncated_by_eggsearch = false;
    if entries.len() > max_e {
        entries.truncate(max_e);
        truncated_by_eggsearch = true;
    }

    let mut warnings = Vec::new();
    if truncated_by_eggsearch {
        warnings.push(SearchWarning::new(
            "github_tree",
            "response_truncated_by_eggsearch: entry limit reached",
        ));
    }
    if truncated_by_provider {
        warnings.push(SearchWarning::new(
            "github_tree",
            "response_truncated_by_provider: GitHub tree response was truncated; \
             results may be incomplete",
        ));
    }
    if commit_sha.is_none() {
        identity.resolved_ref_name = Some(ref_name.to_string());
        warnings.push(SearchWarning::new(
            "github_tree",
            "commit_resolution_unavailable: could not resolve ref to commit SHA; \
             URLs will use mutable ref instead of immutable commit",
        ));
    } else {
        identity.resolved_ref_name = Some(ref_name.to_string());
    }
    if budget.exceeded() {
        warnings.push(SearchWarning::new(
            "github_tree",
            "aggregate_budget_exhausted: aggregate byte budget reached",
        ));
    }

    let telemetry = budget.telemetry();

    Ok(ForgeTreeResponse {
        entries,
        identity,
        truncated_by_provider,
        warnings,
        provider_id: "github_tree".to_string(),
        endpoint_origin: extract_host(base),
        response_bytes_observed: telemetry.aggregate_observed,
        response_cap_applied: telemetry.per_response_cap_hits > 0,
        dns_policy_class: classify_host_from_url(base).await,
        aggregate_byte_cap_reached: budget.exceeded(),
        aggregate_limit: telemetry.aggregate_limit,
        aggregate_remaining: telemetry.remaining,
        request_count: telemetry.request_count,
        exhausted_by: telemetry.exhausted_by,
    })
}

async fn resolve_github_default_branch(
    client: &Client,
    owner: &str,
    repo: &str,
    config: &ForgeTreeConfig,
    timeout: Duration,
    budget: &mut ForgeReadBudget,
) -> Option<String> {
    let base = config.base_url.as_deref().unwrap_or(GITHUB_API_BASE);
    let repo_url = format!(
        "{base}/repos/{}/{}",
        encode_url_component(owner),
        encode_url_component(repo)
    );
    let mut builder = match client.get(repo_url.as_str()) {
        Ok(builder) => builder.timeout(forge_timeout(timeout)),
        Err(e) => {
            tracing::warn!(
                forge = "github",
                owner = owner,
                repo = repo,
                error = %e,
                "default-branch resolve request failed; identity.default_branch will be None"
            );
            return None;
        }
    };
    if let Some(ref key) = config.api_key {
        builder = builder.header("Authorization", &format!("Bearer {key}"));
    }
    let resp = match builder.send().await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(
                forge = "github",
                owner = owner,
                repo = repo,
                error = %e,
                "default-branch resolve request failed; identity.default_branch will be None"
            );
            return None;
        }
    };
    if !resp.status().is_success() {
        tracing::warn!(
            forge = "github",
            owner = owner,
            repo = repo,
            status = resp.status().as_u16(),
            "default-branch resolve returned non-success; identity.default_branch will be None"
        );
        return None;
    }
    let body = match read_with_budget(resp, budget, ForgeRequestKind::RepositoryMetadata).await {
        Ok(b) => b,
        Err(e) => {
            tracing::warn!(
                forge = "github",
                owner = owner,
                repo = repo,
                error = %e,
                "default-branch resolve body read failed; identity.default_branch will be None"
            );
            return None;
        }
    };
    let body_str = match std::str::from_utf8(&body) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(
                forge = "github",
                owner = owner,
                repo = repo,
                error = %e,
                "default-branch resolve body was not UTF-8; identity.default_branch will be None"
            );
            return None;
        }
    };
    let repo_info: GitHubRepoInfo = match serde_json::from_str(body_str) {
        Ok(info) => info,
        Err(e) => {
            tracing::warn!(
                forge = "github",
                owner = owner,
                repo = repo,
                error = %e,
                "default-branch resolve body parse failed; identity.default_branch will be None"
            );
            return None;
        }
    };
    Some(repo_info.default_branch)
}

/// Resolve a GitHub ref to a commit SHA and tree SHA.
///
/// Uses `GET /repos/{owner}/{repo}/commits/{ref}` to obtain the commit
/// SHA and the root tree SHA for the given ref. Returns
/// `(commit_sha, tree_sha)` where either may be `None` if resolution
/// fails.
async fn resolve_github_commit(
    client: &Client,
    owner: &str,
    repo: &str,
    ref_name: &str,
    config: &ForgeTreeConfig,
    timeout: Duration,
    budget: &mut ForgeReadBudget,
) -> (Option<String>, Option<String>) {
    let base = config.base_url.as_deref().unwrap_or(GITHUB_API_BASE);
    let commit_url = format!(
        "{base}/repos/{}/{}/commits/{}",
        encode_url_component(owner),
        encode_url_component(repo),
        encode_url_component(ref_name)
    );
    let mut builder = match client.get(commit_url.as_str()) {
        Ok(builder) => builder.timeout(forge_timeout(timeout)),
        Err(e) => {
            tracing::warn!(
                forge = "github",
                owner = owner,
                repo = repo,
                error = %e,
                "commit resolve request failed"
            );
            return (None, None);
        }
    };
    if let Some(ref key) = config.api_key {
        builder = builder.header("Authorization", &format!("Bearer {key}"));
    }
    let resp = match builder.send().await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(
                forge = "github",
                owner = owner,
                repo = repo,
                error = %e,
                "commit resolve request failed"
            );
            return (None, None);
        }
    };
    if !resp.status().is_success() {
        tracing::warn!(
            forge = "github",
            owner = owner,
            repo = repo,
            status = resp.status().as_u16(),
            "commit resolve returned non-success"
        );
        return (None, None);
    }
    let body = match read_with_budget(resp, budget, ForgeRequestKind::CommitResolution).await {
        Ok(b) => b,
        Err(e) => {
            tracing::warn!(
                forge = "github",
                owner = owner,
                repo = repo,
                error = %e,
                "commit resolve body read failed"
            );
            return (None, None);
        }
    };
    let body_str = match std::str::from_utf8(&body) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(
                forge = "github",
                owner = owner,
                repo = repo,
                error = %e,
                "commit resolve body was not UTF-8"
            );
            return (None, None);
        }
    };
    let commit: GitHubCommitInfo = match serde_json::from_str(body_str) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(
                forge = "github",
                owner = owner,
                repo = repo,
                error = %e,
                "commit resolve body parse failed"
            );
            return (None, None);
        }
    };
    let commit_sha = Some(commit.sha);
    let tree_sha = Some(commit.commit_info.tree.sha);
    (commit_sha, tree_sha)
}

async fn fetch_github_contents_root(
    client: &Client,
    owner: &str,
    repo: &str,
    config: &ForgeTreeConfig,
    timeout: Duration,
    tree_ref: &str,
    budget: &mut ForgeReadBudget,
) -> Result<Vec<ForgeRawEntry>, String> {
    let base = config.base_url.as_deref().unwrap_or(GITHUB_API_BASE);
    let contents_url = format!(
        "{base}/repos/{}/{}/contents/",
        encode_url_component(owner),
        encode_url_component(repo)
    );
    let mut builder = client
        .get(contents_url.as_str())
        .map_err(|e| format!("GitHub Contents API request failed: {e}"))?
        .query("ref", tree_ref)
        .timeout(forge_timeout(timeout));
    if let Some(ref key) = config.api_key {
        builder = builder.header("Authorization", &format!("Bearer {key}"));
    }
    let resp = builder
        .send()
        .await
        .map_err(|e| format!("GitHub Contents API request failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("contents_api_failed: {}", resp.status()));
    }
    let body = read_with_budget(resp, budget, ForgeRequestKind::ContentsFallback)
        .await
        .map_err(|e| e.as_static_str().to_string())?;
    let body_str = std::str::from_utf8(&body).map_err(|_| "invalid_utf8".to_string())?;
    let items: Vec<GitHubContentsEntry> =
        serde_json::from_str(body_str).map_err(|e| format!("malformed Contents response: {e}"))?;
    let entries = items
        .into_iter()
        .map(|item| {
            let kind = match item.type_field.as_str() {
                "dir" => EntryKind::Directory,
                "file" => EntryKind::File,
                "symlink" => EntryKind::Symlink,
                "submodule" => EntryKind::Submodule,
                _ => EntryKind::File,
            };
            ForgeRawEntry {
                path: item.name,
                kind,
                size: item.size,
                object_sha: item.sha,
            }
        })
        .collect();
    Ok(entries)
}

#[derive(Deserialize)]
struct GitHubContentsEntry {
    name: String,
    #[serde(rename = "type")]
    type_field: String,
    size: Option<u64>,
    sha: Option<String>,
}

#[derive(Deserialize)]
struct GitHubRepoInfo {
    default_branch: String,
}

#[derive(Deserialize)]
struct GitHubCommitInfo {
    sha: String,
    #[serde(rename = "commit")]
    commit_info: GitHubCommitObject,
}

#[derive(Deserialize)]
struct GitHubCommitObject {
    tree: GitHubTreeRef,
}

#[derive(Deserialize)]
struct GitHubTreeRef {
    sha: String,
}

#[derive(Deserialize)]
struct GitHubTreeResponse {
    truncated: Option<bool>,
    tree: Vec<GitHubTreeEntry>,
}

#[derive(Deserialize)]
struct GitHubTreeEntry {
    path: String,
    mode: Option<String>,
    #[serde(rename = "type")]
    type_field: String,
    size: Option<u64>,
    sha: Option<String>,
}
