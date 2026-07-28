use std::fs;

#[derive(Debug)]
struct Node {
    key: String,
    value: String,
    block: String,
    children: Vec<Node>,
}

struct SourceLine {
    indent: usize,
    text: String,
}

fn parse_workflow(source: &str) -> Node {
    let lines: Vec<SourceLine> = source
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                None
            } else {
                Some(SourceLine {
                    indent: line.len() - line.trim_start().len(),
                    text: trimmed.to_string(),
                })
            }
        })
        .collect();
    let mut position = 0;
    let children = parse_nodes(&lines, &mut position, 0);
    Node {
        key: String::new(),
        value: String::new(),
        block: String::new(),
        children,
    }
}

fn parse_nodes(lines: &[SourceLine], position: &mut usize, indent: usize) -> Vec<Node> {
    let mut nodes = Vec::new();
    while *position < lines.len() && lines[*position].indent == indent {
        let text = &lines[*position].text;
        let (key, value) = if let Some(item) = text.strip_prefix("- ") {
            split_mapping(item).unwrap_or_else(|| (String::new(), item.to_string()))
        } else {
            split_mapping(text).unwrap_or_else(|| (text.to_string(), String::new()))
        };
        *position += 1;

        let mut node = Node {
            key,
            value,
            block: String::new(),
            children: Vec::new(),
        };
        if node.value == "|" || node.value == ">" {
            let mut block_lines = Vec::new();
            while *position < lines.len() && lines[*position].indent > indent {
                block_lines.push(lines[*position].text.clone());
                *position += 1;
            }
            node.block = block_lines.join("\n");
        } else if *position < lines.len() && lines[*position].indent > indent {
            let child_indent = lines[*position].indent;
            node.children = parse_nodes(lines, position, child_indent);
        }
        nodes.push(node);
    }
    nodes
}

fn split_mapping(text: &str) -> Option<(String, String)> {
    let colon = text.find(':')?;
    let key = text[..colon].trim().trim_matches('"').to_string();
    let value = text[colon + 1..].trim().trim_matches('"').to_string();
    Some((key, value))
}

fn child<'a>(node: &'a Node, key: &str) -> Option<&'a Node> {
    node.children.iter().find(|candidate| candidate.key == key)
}

fn descendants<'a>(node: &'a Node, output: &mut Vec<&'a Node>) {
    for child_node in &node.children {
        output.push(child_node);
        descendants(child_node, output);
    }
}

fn job_run_blocks(job: &Node) -> Vec<&str> {
    let mut nodes = Vec::new();
    descendants(job, &mut nodes);
    nodes
        .into_iter()
        .filter(|node| node.key == "run")
        .map(|node| node.block.as_str())
        .collect()
}

fn step_uses<'a>(job: &'a Node, action: &str) -> Vec<&'a Node> {
    let steps = child(job, "steps").expect("provider job must define steps");
    steps
        .children
        .iter()
        .filter(|step| child(step, "uses").is_some_and(|uses| uses.value == action))
        .collect()
}

fn job_env<'a>(job: &'a Node, key: &str) -> &'a str {
    child(
        child(job, "env").expect("provider job must define env"),
        key,
    )
    .map(|entry| entry.value.as_str())
    .unwrap_or_else(|| panic!("provider job is missing env.{key}"))
}

fn assert_provider_job(job: &Node, provider: &str, credential: &str) {
    let token_value = job_env(job, credential);
    assert!(
        token_value.contains("secrets.") && token_value.contains(credential),
        "{provider} must map {credential} from secrets"
    );
    if provider == "github" {
        assert_eq!(
            job_env(job, "GITHUB_SLASH_REF"),
            "${{ vars.NATIVE_SMOKE_GITHUB_SLASH_REF }}"
        );
    }

    let checkout_steps = step_uses(job, "actions/checkout@v4");
    assert!(
        checkout_steps.iter().any(|step| {
            child(step, "if").is_some_and(|condition| condition.value.contains("workflow_dispatch"))
                && child(
                    child(step, "with").expect("checkout must define with"),
                    "ref",
                )
                .is_some_and(|reference| reference.value == "${{ inputs.release_subject }}")
        }),
        "{provider} must checkout inputs.release_subject for manual release evidence"
    );
    assert!(
        checkout_steps.iter().any(|step| {
            child(step, "if").is_some_and(|condition| condition.value.contains("schedule"))
                && child(
                    child(step, "with").expect("checkout must define with"),
                    "ref",
                )
                .is_some_and(|reference| reference.value == "${{ github.sha }}")
        }),
        "{provider} must keep scheduled checkout separate from release checkout"
    );

    let run_blocks = job_run_blocks(job);
    assert!(
        run_blocks.iter().any(|block| {
            block.contains("git rev-parse HEAD")
                && block.contains("EGGSEARCH_RELEASE_SUBJECT")
                && block.contains("40")
        }),
        "{provider} must verify the checked out full release SHA"
    );
    assert!(
        run_blocks.iter().any(|block| {
            block.contains("cargo test")
                && block.contains("--test native_forge_smoke")
                && block.contains(provider)
        }),
        "{provider} must run its named native smoke filter"
    );
    assert!(
        run_blocks.iter().any(|block| {
            block.contains("find")
                && block.contains("jq -e")
                && block.contains(".mode == \"native\"")
                && block.contains(".result == \"pass\"")
                && block.contains("EGGSEARCH_NATIVE_SMOKE_EVIDENCE_DIR")
        }),
        "{provider} must validate structured native evidence"
    );
    assert!(
        step_uses(job, "actions/upload-artifact@v4")
            .iter()
            .any(|step| {
                child(
                    child(step, "with").expect("artifact upload must define with"),
                    "path",
                )
                .is_some_and(|entry| {
                    (entry.value.contains("native-smoke") && entry.value.contains("json"))
                        || (entry.value == "|"
                            && entry.block.contains("native-smoke")
                            && entry.block.contains("json"))
                })
            }),
        "{provider} must upload structured evidence"
    );
}

#[test]
fn native_forge_workflow_is_fail_closed() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let source = fs::read_to_string(format!(
        "{manifest_dir}/.github/workflows/native-forge-smoke.yml"
    ))
    .expect("native forge workflow exists");
    let workflow = parse_workflow(&source);

    let trigger = child(&workflow, "on").expect("workflow trigger exists");
    let dispatch = child(trigger, "workflow_dispatch").expect("manual trigger exists");
    let release_subject = child(
        child(
            child(dispatch, "inputs").expect("dispatch inputs exist"),
            "release_subject",
        )
        .expect("release_subject input exists"),
        "required",
    )
    .expect("release_subject required setting exists");
    assert_eq!(release_subject.value, "true");
    let input = child(child(dispatch, "inputs").unwrap(), "release_subject").unwrap();
    assert_eq!(
        child(input, "type").expect("input type exists").value,
        "string"
    );
    assert!(
        child(trigger, "schedule").is_some(),
        "diagnostic schedule exists"
    );

    let jobs = child(&workflow, "jobs").expect("workflow jobs exist");
    for (provider, credential) in [
        ("github", "GITHUB_TOKEN"),
        ("gitlab", "GITLAB_TOKEN"),
        ("codeberg", "CODEBERG_TOKEN"),
        ("gitea", "GITEA_TOKEN"),
    ] {
        assert_provider_job(
            child(jobs, provider).unwrap_or_else(|| panic!("missing {provider} job")),
            provider,
            credential,
        );
    }

    let summary = child(jobs, "summary").expect("summary job exists");
    let summary_runs = job_run_blocks(summary);
    assert!(
        summary_runs.iter().any(|block| {
            (block.contains("needs.github.result")
                && block.contains("needs.github.outputs.result")
                && block.contains("== pass"))
                || (block.contains("adapters")
                    && block.contains("ADAPTER_LIST")
                    && block.contains("result_var"))
        }),
        "release summary must require exact pass for selected providers"
    );
    assert!(
        summary_runs
            .iter()
            .any(|block| block.contains("sha256sum") && block.contains("jq -n")),
        "release summary must build a hashed evidence manifest"
    );
    assert!(
        step_uses(summary, "actions/download-artifact@v4")
            .iter()
            .any(|step| {
                child(
                    child(step, "with").expect("artifact download must define with"),
                    "pattern",
                )
                .is_some_and(|pattern| pattern.value == "native-smoke-*")
            }),
        "release summary must download provider evidence"
    );
    assert!(
        step_uses(summary, "actions/upload-artifact@v4")
            .iter()
            .any(|step| {
                child(
                    child(step, "with").expect("manifest upload must define with"),
                    "name",
                )
                .is_some_and(|name| name.value == "native-smoke-release-manifest")
            }),
        "release summary must upload the combined manifest"
    );

    assert!(!source.contains("smoke/slash-ref"));
    assert!(!source.contains("GITHUB_SLASH_REF ||"));
    assert!(!source.contains("GITEA_INSTANCE_URL ||"));
}
