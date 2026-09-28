use super::*;

pub(super) struct ForgeTreeParams<'a> {
    pub(super) client: &'a Client,
    pub(super) owner: &'a str,
    pub(super) repo: &'a str,
    pub(super) req: &'a RepoMapRequest,
    pub(super) config: &'a ForgeTreeConfig,
    pub(super) timeout: Duration,
    pub(super) api_base: &'a str,
    pub(super) provider_id: &'a str,
}

pub(super) async fn fetch_forge_tree(
    params: ForgeTreeParams<'_>,
) -> Result<ForgeTreeResponse, String> {
    let ForgeTreeParams {
        client,
        owner,
        repo,
        req,
        config,
        timeout,
        api_base,
        provider_id,
    } = params;
    let ref_name = req.ref_name.as_deref().unwrap_or("HEAD");
    let max_d = max_depth(req);
    let max_e = max_entries(req);
    let per_page = 100.min(max_e);

    let mut identity = ResolvedRepositoryIdentity {
        requested_ref: Some(ref_name.to_string()),
        resolved_ref_name: Some(ref_name.to_string()),
        ..Default::default()
    };

    let mut budget = ForgeReadBudget::new(
        config
            .forge_budget_limit
            .unwrap_or(DEFAULT_MAX_RESPONSE_BYTES),
    );

    let (commit_sha, tree_sha) = resolve_forge_commit(
        client,
        owner,
        repo,
        ref_name,
        config,
        timeout,
        api_base,
        &mut budget,
    )
    .await;
    identity.resolved_commit_sha = commit_sha;
    identity.tree_sha = tree_sha;

    let tree_ref = identity
        .tree_sha
        .as_deref()
        .or(identity.resolved_commit_sha.as_deref())
        .unwrap_or(ref_name);

    let default_branch =
        resolve_forge_default_branch(client, owner, repo, config, timeout, api_base, &mut budget)
            .await;
    identity.default_branch = default_branch;

    let mut all_entries: Vec<ForgeRawEntry> = Vec::new();
    let mut page = 1u32;
    let mut truncated_by_provider = false;
    let mut warnings = Vec::new();
    let max_pages = DEFAULT_MAX_PAGES;

    loop {
        if page > max_pages as u32 {
            warnings.push(SearchWarning::new(
                provider_id,
                "pagination_limit_reached: forge tree pagination hit page limit",
            ));
            break;
        }
        if all_entries.len() >= max_e {
            break;
        }
        if budget.exceeded() {
            warnings.push(SearchWarning::new(
                provider_id,
                "aggregate_budget_exhausted: aggregate byte budget reached",
            ));
            break;
        }

        let tree_url = format!(
            "{api_base}/repos/{}/{}/git/trees/{}",
            encode_url_component(owner),
            encode_url_component(repo),
            encode_url_component(tree_ref)
        );
        let mut builder = client
            .get(tree_url.as_str())
            .map_err(|e| format!("forge API request failed: {e}"))?
            .query("recursive", if max_d > 1 { "1" } else { "0" })
            .query("per_page", &per_page.to_string())
            .query("page", &page.to_string())
            .timeout(forge_timeout(timeout));

        if let Some(ref key) = config.api_key {
            builder = builder.header("Authorization", &format!("token {key}"));
        }

        let resp = builder
            .send()
            .await
            .map_err(|e| format!("forge API request failed: {e}"))?;

        let status = resp.status();
        if status.as_u16() == 404 {
            if all_entries.is_empty() {
                return Err("repository_not_found".into());
            }
            break;
        }
        if status.as_u16() == 401 {
            return Err("authentication_required".into());
        }
        if status.as_u16() == 403 {
            return Err("permission_denied".into());
        }
        if status.as_u16() == 429 {
            if all_entries.is_empty() {
                return Err("rate_limited".into());
            }
            warnings.push(SearchWarning::new(
                provider_id,
                "rate_limited_partial: rate limited mid-pagination; returning partial results",
            ));
            truncated_by_provider = true;
            break;
        }
        if !status.is_success() {
            let msg = read_error_body_preview(resp, &mut budget).await;
            return Err(format!("provider_unavailable: {status} - {msg}"));
        }

        let body = read_with_budget(resp, &mut budget, ForgeRequestKind::TreePage)
            .await
            .map_err(|e| e.as_static_str().to_string())?;

        let body_str = std::str::from_utf8(&body).map_err(|_| "invalid_utf8".to_string())?;

        let tree: ForgeTreeApiResponse =
            serde_json::from_str(body_str).map_err(|e| format!("malformed response: {e}"))?;

        truncated_by_provider |= tree.truncated.unwrap_or(false);

        let page_len = tree.tree.len();
        for item in tree.tree {
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
            all_entries.push(ForgeRawEntry {
                path: item.path,
                kind,
                size: item.size,
                object_sha: item.sha,
            });
        }

        if page_len < per_page {
            break;
        }
        page += 1;
    }

    if all_entries.len() >= max_e {
        warnings.push(SearchWarning::new(
            provider_id,
            "response_truncated_by_eggsearch: entry limit reached",
        ));
        all_entries.truncate(max_e);
    }

    if truncated_by_provider {
        warnings.push(SearchWarning::new(
            provider_id,
            "response_truncated_by_provider: forge tree response was truncated",
        ));
    }

    if identity.resolved_commit_sha.is_none() {
        warnings.push(SearchWarning::new(
            provider_id,
            "commit_resolution_unavailable: could not resolve ref to commit SHA; \
             URLs will use mutable ref instead of immutable commit",
        ));
    }
    if budget.exceeded() {
        warnings.push(SearchWarning::new(
            provider_id,
            "aggregate_budget_exhausted: aggregate byte budget reached",
        ));
    }

    let telemetry = budget.telemetry();

    Ok(ForgeTreeResponse {
        entries: all_entries,
        identity,
        truncated_by_provider,
        warnings,
        provider_id: provider_id.to_string(),
        endpoint_origin: extract_host(api_base),
        response_bytes_observed: telemetry.aggregate_observed,
        response_cap_applied: telemetry.per_response_cap_hits > 0,
        dns_policy_class: classify_host_from_url(api_base).await,
        aggregate_byte_cap_reached: budget.exceeded(),
        aggregate_limit: telemetry.aggregate_limit,
        aggregate_remaining: telemetry.remaining,
        request_count: telemetry.request_count,
        exhausted_by: telemetry.exhausted_by,
    })
}

async fn resolve_forge_default_branch(
    client: &Client,
    owner: &str,
    repo: &str,
    config: &ForgeTreeConfig,
    timeout: Duration,
    api_base: &str,
    budget: &mut ForgeReadBudget,
) -> Option<String> {
    let repo_url = format!(
        "{api_base}/repos/{}/{}",
        encode_url_component(owner),
        encode_url_component(repo)
    );
    let mut builder = match client.get(repo_url.as_str()) {
        Ok(builder) => builder.timeout(forge_timeout(timeout)),
        Err(e) => {
            tracing::warn!(
                forge = "gitea-like",
                owner = owner,
                repo = repo,
                error = %e,
                "default-branch resolve request failed; identity.default_branch will be None"
            );
            return None;
        }
    };
    if let Some(ref key) = config.api_key {
        builder = builder.header("Authorization", &format!("token {key}"));
    }
    let resp = match builder.send().await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(
                forge = "gitea-like",
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
            forge = "gitea-like",
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
                forge = "gitea-like",
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
                forge = "gitea-like",
                owner = owner,
                repo = repo,
                error = %e,
                "default-branch resolve body was not UTF-8; identity.default_branch will be None"
            );
            return None;
        }
    };
    let info: ForgeRepoInfo = match serde_json::from_str(body_str) {
        Ok(info) => info,
        Err(e) => {
            tracing::warn!(
                forge = "gitea-like",
                owner = owner,
                repo = repo,
                error = %e,
                "default-branch resolve body parse failed; identity.default_branch will be None"
            );
            return None;
        }
    };
    Some(info.default_branch)
}

/// Resolve a forge ref (Gitea/Forgejo/Codeberg) to a commit SHA.
///
/// Uses `GET /repos/{owner}/{repo}/commits/{ref}` to obtain the commit
/// SHA. The tree SHA is not directly available from this endpoint for
/// all providers, so it may be `None`. Returns `(commit_sha, tree_sha)`
/// where either may be `None` if resolution fails.
#[allow(clippy::too_many_arguments)]
async fn resolve_forge_commit(
    client: &Client,
    owner: &str,
    repo: &str,
    ref_name: &str,
    config: &ForgeTreeConfig,
    timeout: Duration,
    api_base: &str,
    budget: &mut ForgeReadBudget,
) -> (Option<String>, Option<String>) {
    let commit_url = format!(
        "{api_base}/repos/{}/{}/commits/{}",
        encode_url_component(owner),
        encode_url_component(repo),
        encode_url_component(ref_name)
    );
    let mut builder = match client.get(commit_url.as_str()) {
        Ok(builder) => builder.timeout(forge_timeout(timeout)),
        Err(e) => {
            tracing::warn!(
                forge = "gitea-like",
                owner = owner,
                repo = repo,
                error = %e,
                "commit resolve request failed"
            );
            return (None, None);
        }
    };
    if let Some(ref key) = config.api_key {
        builder = builder.header("Authorization", &format!("token {key}"));
    }
    let resp = match builder.send().await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(
                forge = "gitea-like",
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
            forge = "gitea-like",
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
                forge = "gitea-like",
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
                forge = "gitea-like",
                owner = owner,
                repo = repo,
                error = %e,
                "commit resolve body was not UTF-8"
            );
            return (None, None);
        }
    };
    let commit: ForgeCommitInfo = match serde_json::from_str(body_str) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(
                forge = "gitea-like",
                owner = owner,
                repo = repo,
                error = %e,
                "commit resolve body parse failed"
            );
            return (None, None);
        }
    };
    let commit_sha = Some(commit.sha);
    (commit_sha, None)
}

#[derive(Deserialize)]
struct ForgeCommitInfo {
    sha: String,
}

#[derive(Deserialize)]
struct ForgeRepoInfo {
    default_branch: String,
}

#[derive(Deserialize)]
struct ForgeTreeApiResponse {
    truncated: Option<bool>,
    tree: Vec<ForgeTreeApiEntry>,
}

#[derive(Deserialize)]
struct ForgeTreeApiEntry {
    path: String,
    mode: Option<String>,
    #[serde(rename = "type")]
    type_field: String,
    size: Option<u64>,
    sha: Option<String>,
}
