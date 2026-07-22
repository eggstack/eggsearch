use std::fs;

fn read_source(path: &str) -> String {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    fs::read_to_string(format!("{manifest_dir}/{path}"))
        .unwrap_or_else(|e| panic!("failed to read {path}: {e}"))
}

fn strip_test_code(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let mut in_test = false;
    for line in source.lines() {
        if line.trim_start().starts_with("#[cfg(test)]") {
            in_test = true;
        }
        if !in_test {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

#[test]
fn no_unbounded_forge_body_reads() {
    let source = read_source("src/meta/forge_adapter.rs");
    let non_test = strip_test_code(&source);

    let forbidden = [".text().await", ".bytes().await", ".json().await"];
    for pattern in &forbidden {
        assert!(
            !non_test.contains(pattern),
            "forge_adapter.rs (non-test) contains forbidden unbounded read pattern: {pattern}"
        );
    }
}

#[test]
fn no_unbounded_git_output() {
    let source = read_source("src/meta/local_inventory_cache.rs");
    let non_test = strip_test_code(&source);

    assert!(
        !non_test.contains(".output()"),
        "local_inventory_cache.rs (non-test) contains unbounded .output() call; use run_bounded_command() instead"
    );
}

#[test]
fn no_path_based_reads_in_safe_open() {
    let source = read_source("src/meta/safe_open.rs");
    let non_test = strip_test_code(&source);

    let forbidden = ["std::fs::read(", "std::fs::read_to_string("];
    for pattern in &forbidden {
        assert!(
            !non_test.contains(pattern),
            "safe_open.rs (non-test) contains forbidden path-based read: {pattern}"
        );
    }
}

#[test]
fn no_object_sha_in_commit_urls() {
    let source = read_source("src/meta/forge_adapter.rs");

    // Find the build_entry_urls function body
    let fn_start = source
        .find("fn build_entry_urls(")
        .expect("build_entry_urls not found");
    let fn_body = &source[fn_start..];

    // Verify that object_sha is explicitly suppressed (let _ = object_sha)
    assert!(
        fn_body.contains("let _ = object_sha;"),
        "build_entry_urls must explicitly suppress object_sha (let _ = object_sha;)"
    );

    // Verify that object_sha is NOT passed to any URL builder function
    // (github_permalink_url, gitlab_browser_url, etc. should use commit_sha, not object_sha)
    let lines: Vec<&str> = fn_body.lines().collect();
    let mut fn_end = lines.len();
    for (i, line) in lines.iter().enumerate() {
        if i > 0 && line.trim().starts_with("fn ") {
            fn_end = i;
            break;
        }
    }
    let fn_only = &fn_body.lines().take(fn_end).collect::<Vec<_>>().join("\n");

    let url_fns = [
        "github_permalink_url",
        "github_raw_permalink_url",
        "github_browser_url",
        "github_raw_url",
        "gitlab_browser_url",
        "gitlab_raw_url",
        "codeberg_browser_url",
        "codeberg_raw_url",
        "gitea_browser_url",
        "gitea_raw_url",
    ];
    for url_fn in &url_fns {
        for line in fn_only.lines() {
            if line.contains(url_fn) && line.contains("object_sha") {
                panic!(
                    "build_entry_urls passes object_sha to {url_fn}; \
                     must use commit_sha for immutable permalinks"
                );
            }
        }
    }
}

#[test]
fn postprocess_called_with_workflow_model_for_non_web_tools() {
    // Check adapter.rs: repo_search and research_search must pass Some(model)
    let adapter_source = read_source("src/meta/adapter.rs");
    let adapter_non_test = strip_test_code(&adapter_source);

    // repo_search postprocess call
    let repo_idx = adapter_non_test
        .find("fn repo_search(")
        .expect("repo_search not found in adapter.rs");
    let repo_section = &adapter_non_test[repo_idx..];
    let repo_postprocess_start = repo_section
        .find("evidence_postprocess::postprocess(")
        .expect("postprocess call not found in repo_search");
    let repo_postprocess_end =
        repo_section[repo_postprocess_start..].find(')').unwrap() + repo_postprocess_start;
    let repo_call = &repo_section[repo_postprocess_start..=repo_postprocess_end];
    assert!(
        repo_call.contains("workflow_model.as_ref()"),
        "repo_search postprocess must pass workflow_model.as_ref(), got: {repo_call}"
    );

    // research_search postprocess call
    let research_idx = adapter_non_test
        .find("fn research_search(")
        .expect("research_search not found in adapter.rs");
    let research_section = &adapter_non_test[research_idx..];
    let research_postprocess_start = research_section
        .find("evidence_postprocess::postprocess(")
        .expect("postprocess call not found in research_search");
    let research_postprocess_end = research_section[research_postprocess_start..]
        .find(')')
        .unwrap()
        + research_postprocess_start;
    let research_call = &research_section[research_postprocess_start..=research_postprocess_end];
    assert!(
        research_call.contains("workflow_model.as_ref()"),
        "research_search postprocess must pass workflow_model.as_ref(), got: {research_call}"
    );

    // Check security_search.rs: must pass Some(model)
    let security_source = read_source("src/meta/security_search.rs");
    let security_non_test = strip_test_code(&security_source);
    let sec_postprocess_start = security_non_test
        .find("evidence_postprocess::postprocess(")
        .expect("postprocess call not found in security_search.rs");
    let sec_postprocess_end = security_non_test[sec_postprocess_start..]
        .find(')')
        .unwrap()
        + sec_postprocess_start;
    let sec_call = &security_non_test[sec_postprocess_start..=sec_postprocess_end];
    assert!(
        sec_call.contains("workflow_model.as_ref()"),
        "security_search postprocess must pass workflow_model.as_ref(), got: {sec_call}"
    );
}

#[test]
fn git_runner_drains_stdout_before_stderr_concurrently() {
    let source = read_source("src/meta/local_inventory_cache.rs");
    let non_test = strip_test_code(&source);

    let has_stdout_thread = non_test.contains("std::thread::spawn")
        && non_test
            .lines()
            .any(|l| l.contains("stdout") && l.contains("thread"));
    assert!(
        has_stdout_thread,
        "run_bounded_command must drain stdout and stderr concurrently using threads, \
         not sequentially. Currently reads stdout to completion before stderr."
    );
}

#[test]
fn forge_has_aggregate_byte_budget_type() {
    let source = read_source("src/meta/forge_adapter.rs");
    let non_test = strip_test_code(&source);

    assert!(
        non_test.contains("struct ForgeReadBudget"),
        "forge_adapter.rs must define ForgeReadBudget for aggregate byte enforcement. \
         Currently uses bare total_bytes: &mut usize without formal budget."
    );
}

#[test]
fn all_forge_response_paths_bounded() {
    let source = read_source("src/meta/forge_adapter.rs");
    let non_test = strip_test_code(&source);

    // Every response from a forge API must go through read_bounded_response
    // or read_bounded_body. Verify no direct .bytes_stream() usage outside
    // of these two functions and read_error_body_preview.
    let bounded_fns = [
        "fn read_bounded_body",
        "fn read_bounded_response",
        "fn read_error_body_preview",
    ];

    // Find all bytes_stream() usages and verify the nearest preceding fn
    // declaration is one of the bounded read functions.
    let lines: Vec<&str> = non_test.lines().collect();
    for (line_num, line) in lines.iter().enumerate() {
        if line.contains("bytes_stream()") {
            let nearest_fn = lines.iter().take(line_num).rev().find_map(|l| {
                let t = l.trim();
                if t.starts_with("fn ")
                    || t.starts_with("pub async fn ")
                    || t.starts_with("async fn ")
                {
                    Some(*l)
                } else {
                    None
                }
            });
            let in_bounded =
                nearest_fn.is_some_and(|fn_line| bounded_fns.iter().any(|f| fn_line.contains(f)));
            assert!(
                in_bounded,
                "bytes_stream() at line {} is outside bounded read functions; \
                 all forge responses must use read_bounded_body/read_bounded_response",
                line_num + 1
            );
        }
    }
}
