use super::*;

pub(super) async fn fetch_gitlab_tree(
    client: &Client,
    owner: &str,
    repo: &str,
    req: &RepoMapRequest,
    config: &ForgeTreeConfig,
    timeout: Duration,
) -> Result<ForgeTreeResponse, String> {
    let base = config.base_url.as_deref().unwrap_or(GITLAB_API_BASE);
    let project_path_raw = format!("{owner}/{repo}");
    let project_path = urlencoding::encode(&project_path_raw);
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

    let (commit_sha, tree_sha) =
        resolve_gitlab_commit(client, owner, repo, ref_name, config, timeout, &mut budget).await;
    identity.resolved_commit_sha = commit_sha;
    identity.tree_sha = tree_sha;

    let default_branch =
        resolve_gitlab_default_branch(client, owner, repo, config, timeout, &mut budget).await;
    identity.default_branch = default_branch;

    let mut all_entries: Vec<ForgeRawEntry> = Vec::new();
    let mut page = 1u32;
    let mut truncated_by_provider = false;
    let mut warnings = Vec::new();
    let max_pages = DEFAULT_MAX_PAGES;

    loop {
        if page > max_pages as u32 {
            warnings.push(SearchWarning::new(
                "gitlab_tree",
                "pagination_limit_reached: GitLab tree pagination hit page limit",
            ));
            break;
        }
        if all_entries.len() >= max_e {
            break;
        }
        if budget.exceeded() {
            warnings.push(SearchWarning::new(
                "gitlab_tree",
                "aggregate_budget_exhausted: aggregate byte budget reached",
            ));
            break;
        }

        let tree_url = format!("{base}/projects/{project_path}/repository/tree");
        let mut builder = client
            .get(tree_url.as_str())
            .map_err(|e| format!("GitLab API request failed: {e}"))?
            .query("ref", ref_name)
            .query("recursive", if max_d > 1 { "true" } else { "false" })
            .query("per_page", &per_page.to_string())
            .query("page", &page.to_string())
            .timeout(forge_timeout(timeout));

        if let Some(ref key) = config.api_key {
            builder = builder.header("PRIVATE-TOKEN", key.as_str());
        }

        let resp = builder
            .send()
            .await
            .map_err(|e| format!("GitLab API request failed: {e}"))?;

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
                "gitlab_tree",
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

        let items: Vec<GitLabTreeEntry> =
            serde_json::from_str(body_str).map_err(|e| format!("malformed response: {e}"))?;

        let page_len = items.len();
        for item in items {
            let kind = match item.type_field.as_str() {
                "blob" => EntryKind::File,
                "tree" => EntryKind::Directory,
                "commit" => EntryKind::Submodule,
                _ => EntryKind::File,
            };
            all_entries.push(ForgeRawEntry {
                path: item.path,
                kind,
                size: item.size,
                object_sha: item.id,
            });
        }

        if page_len < per_page {
            break;
        }
        page += 1;
    }

    if all_entries.len() >= max_e {
        warnings.push(SearchWarning::new(
            "gitlab_tree",
            "response_truncated_by_eggsearch: entry limit reached",
        ));
        all_entries.truncate(max_e);
    }

    let telemetry = budget.telemetry();

    Ok(ForgeTreeResponse {
        entries: all_entries,
        identity,
        truncated_by_provider,
        warnings,
        provider_id: "gitlab_tree".to_string(),
        endpoint_origin: extract_host(config.base_url.as_deref().unwrap_or(GITLAB_API_BASE)),
        response_bytes_observed: telemetry.aggregate_observed,
        response_cap_applied: telemetry.per_response_cap_hits > 0,
        dns_policy_class: classify_host_from_url(
            config.base_url.as_deref().unwrap_or(GITLAB_API_BASE),
        )
        .await,
        aggregate_byte_cap_reached: budget.exceeded(),
        aggregate_limit: telemetry.aggregate_limit,
        aggregate_remaining: telemetry.remaining,
        request_count: telemetry.request_count,
        exhausted_by: telemetry.exhausted_by,
    })
}

async fn resolve_gitlab_default_branch(
    client: &Client,
    owner: &str,
    repo: &str,
    config: &ForgeTreeConfig,
    timeout: Duration,
    budget: &mut ForgeReadBudget,
) -> Option<String> {
    let base = config.base_url.as_deref().unwrap_or(GITLAB_API_BASE);
    let project_path_raw = format!("{owner}/{repo}");
    let project_path = urlencoding::encode(&project_path_raw);
    let project_url = format!("{base}/projects/{project_path}");
    let mut builder = match client.get(project_url.as_str()) {
        Ok(builder) => builder.timeout(forge_timeout(timeout)),
        Err(e) => {
            tracing::warn!(
                forge = "gitlab",
                owner = owner,
                repo = repo,
                error = %e,
                "default-branch resolve request failed; identity.default_branch will be None"
            );
            return None;
        }
    };
    if let Some(ref key) = config.api_key {
        builder = builder.header("PRIVATE-TOKEN", key.as_str());
    }
    let resp = match builder.send().await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(
                forge = "gitlab",
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
            forge = "gitlab",
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
                forge = "gitlab",
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
                forge = "gitlab",
                owner = owner,
                repo = repo,
                error = %e,
                "default-branch resolve body was not UTF-8; identity.default_branch will be None"
            );
            return None;
        }
    };
    let info: GitLabProjectInfo = match serde_json::from_str(body_str) {
        Ok(info) => info,
        Err(e) => {
            tracing::warn!(
                forge = "gitlab",
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

#[derive(Deserialize)]
struct GitLabProjectInfo {
    default_branch: String,
}

/// Resolve a GitLab ref to a commit SHA and tree SHA.
///
/// Uses `GET /projects/:id/repository/commits/:sha` to obtain the commit
/// SHA and root tree SHA for the given ref. Returns
/// `(commit_sha, tree_sha)` where either may be `None` if resolution
/// fails.
async fn resolve_gitlab_commit(
    client: &Client,
    owner: &str,
    repo: &str,
    ref_name: &str,
    config: &ForgeTreeConfig,
    timeout: Duration,
    budget: &mut ForgeReadBudget,
) -> (Option<String>, Option<String>) {
    let base = config.base_url.as_deref().unwrap_or(GITLAB_API_BASE);
    let project_path_raw = format!("{owner}/{repo}");
    let project_path = urlencoding::encode(&project_path_raw);
    let encoded_ref = encode_url_component(ref_name);
    let commit_url = format!("{base}/projects/{project_path}/repository/commits/{encoded_ref}");
    let mut builder = match client.get(commit_url.as_str()) {
        Ok(builder) => builder.timeout(forge_timeout(timeout)),
        Err(e) => {
            tracing::warn!(
                forge = "gitlab",
                owner = owner,
                repo = repo,
                error = %e,
                "commit resolve request failed"
            );
            return (None, None);
        }
    };
    if let Some(ref key) = config.api_key {
        builder = builder.header("PRIVATE-TOKEN", key.as_str());
    }
    let resp = match builder.send().await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(
                forge = "gitlab",
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
            forge = "gitlab",
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
                forge = "gitlab",
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
                forge = "gitlab",
                owner = owner,
                repo = repo,
                error = %e,
                "commit resolve body was not UTF-8"
            );
            return (None, None);
        }
    };
    let commit: GitLabCommitInfo = match serde_json::from_str(body_str) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(
                forge = "gitlab",
                owner = owner,
                repo = repo,
                error = %e,
                "commit resolve body parse failed"
            );
            return (None, None);
        }
    };
    let commit_sha = Some(commit.id);
    let tree_sha = commit.tree_id;
    (commit_sha, tree_sha)
}

#[derive(Deserialize)]
struct GitLabCommitInfo {
    id: String,
    tree_id: Option<String>,
}

#[derive(Deserialize)]
struct GitLabTreeEntry {
    id: Option<String>,
    path: String,
    #[serde(rename = "type")]
    type_field: String,
    size: Option<u64>,
}
