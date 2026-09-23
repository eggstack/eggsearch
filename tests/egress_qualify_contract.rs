use std::collections::BTreeSet;
use std::fs;
fn release_targets() -> Vec<String> {
    let text = fs::read_to_string("packaging/release-targets.txt").expect("read release targets");
    let mut out = Vec::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        let first = line.split('|').next().expect("pipe-delimited row");
        assert!(!first.is_empty(), "empty target in release-targets.txt");
        out.push(first.to_string());
    }
    out.sort();
    out.dedup();
    out
}
fn workflow_text() -> String {
    fs::read_to_string(".github/workflows/egress-feature-qualify.yml")
        .expect("read egress workflow")
}
fn looks_like_target(token: &str) -> bool {
    let token = token.trim().trim_matches(['"', '\'']);
    if token.contains('$') || token.contains("${{") {
        return false;
    }
    if token.matches('-').count() < 2 {
        return false;
    }
    token.contains("unknown") || token.contains("apple") || token.contains("pc-windows")
}
fn matrix_targets(text: &str) -> Vec<String> {
    let mut set = BTreeSet::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("target:") {
            let token = rest.trim().trim_matches(['"', '\'']);
            let token = token.split_whitespace().next().unwrap_or("");
            if looks_like_target(token) {
                set.insert(token.to_string());
            }
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("targets:") {
            for piece in rest.split([' ', ',']) {
                let token = piece.trim().trim_matches(['"', '\'']);
                if looks_like_target(token) {
                    set.insert(token.to_string());
                }
            }
            continue;
        }
        if line.contains("--target") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            for (i, part) in parts.iter().enumerate() {
                if *part == "--target" {
                    if let Some(next) = parts.get(i + 1) {
                        let token = next.trim().trim_matches(['"', '\'']);
                        let token = token.trim_end_matches(['"', '\'']);
                        if looks_like_target(token) {
                            set.insert(token.to_string());
                        }
                    }
                }
            }
        }
    }
    set.into_iter().collect()
}
fn exact_diff(release: &[String], matrix: &[String]) -> (Vec<String>, Vec<String>) {
    let release_set: BTreeSet<&str> = release.iter().map(|s| s.as_str()).collect();
    let matrix_set: BTreeSet<&str> = matrix.iter().map(|s| s.as_str()).collect();
    let missing: Vec<String> = release_set
        .difference(&matrix_set)
        .map(|s| (*s).to_string())
        .collect();
    let extra: Vec<String> = matrix_set
        .difference(&release_set)
        .map(|s| (*s).to_string())
        .collect();
    (missing, extra)
}
#[test]
fn egress_matrix_equals_release_targets() {
    let release = release_targets();
    assert_eq!(
        release.len(),
        7,
        "release contract must hold seven targets: {release:?}"
    );
    let workflow = workflow_text();
    let matrix = matrix_targets(&workflow);
    let (missing, extra) = exact_diff(&release, &matrix);
    assert!(
        missing.is_empty() && extra.is_empty(),
        "egress matrix must exactly equal release-targets.txt; missing={missing:?} extra={extra:?} matrix={matrix:?} release={release:?}"
    );
}
#[test]
fn egress_matrix_missing_target_fails() {
    let release = release_targets();
    let mut matrix = release.clone();
    matrix.pop();
    let (missing, extra) = exact_diff(&release, &matrix);
    assert_eq!(
        missing.len(),
        1,
        "omitting one target must report one missing: {missing:?}"
    );
    assert!(
        extra.is_empty(),
        "omitted-target fixture must not report extras: {extra:?}"
    );
}
#[test]
fn egress_matrix_extra_target_fails() {
    let release = release_targets();
    let mut matrix = release.clone();
    matrix.push("riscv64-unknown-linux-gnu".to_string());
    let (missing, extra) = exact_diff(&release, &matrix);
    assert!(
        missing.is_empty(),
        "extra-target fixture must not report missing: {missing:?}"
    );
    assert_eq!(
        extra,
        vec!["riscv64-unknown-linux-gnu".to_string()],
        "added target must be reported as extra: {extra:?}"
    );
}
#[test]
fn egress_workflow_is_non_publishing_and_checks_each_target() {
    let workflow = workflow_text();
    let release = release_targets();
    assert!(
        workflow.contains("cargo check --locked --features egress"),
        "workflow must run cargo check --locked --features egress"
    );
    for target in &release {
        assert!(
            workflow.contains(target),
            "workflow must reference maintained target: {target}"
        );
    }
    for forbidden in [
        "upload-artifact",
        "upload-release",
        "cargo publish",
        "softprops/",
        "svenstaro/",
        "gh release",
    ] {
        assert!(
            !workflow.contains(forbidden),
            "workflow must remain non-publishing; found: {forbidden}"
        );
    }
}
#[test]
fn egress_workflow_retains_msrv_all_features() {
    let workflow = workflow_text();
    assert!(
        workflow.contains("msrv-all-features"),
        "workflow must retain the MSRV all-features job"
    );
    assert!(
        workflow.contains("cargo +1.89.0 check --locked --all-features"),
        "workflow must run the Rust 1.89 all-features check"
    );
}
#[test]
fn egress_workflow_triggers_cover_route_seam() {
    let workflow = workflow_text();
    let required = [
        "src/fetch/egress.rs",
        "src/core/config.rs",
        "src/meta/engines/mod.rs",
        "src/meta/adapter/builders.rs",
        "src/meta/adapter/mod.rs",
        "src/mcp/state.rs",
        "tests/egress_routing.rs",
        "tests/static_guards.rs",
        "tests/egress_qualify_contract.rs",
        "Cargo.toml",
        "Cargo.lock",
        "packaging/release-targets.txt",
        "packaging/check-egress-qualify-contract.sh",
        ".github/workflows/egress-feature-qualify.yml",
    ];
    let mut uncovered = Vec::new();
    for path in required {
        if !workflow.contains(path) {
            uncovered.push(path);
        }
    }
    assert!(
        uncovered.is_empty(),
        "workflow path filters miss route-seam owners: {uncovered:?}"
    );
}
#[test]
fn egress_route_symbols_are_covered() {
    let workflow = workflow_text();
    let owners = [
        ("build_http_client_with_egress", "src/meta/engines/mod.rs"),
        (
            "build_default_engines_with_egress",
            "src/meta/adapter/builders.rs",
        ),
        ("new_with_egress", "src/meta/adapter/mod.rs"),
        ("config.egress", "src/mcp/state.rs"),
        ("EgressSection", "src/core/config.rs"),
        ("apply_route", "src/fetch/egress.rs"),
    ];
    for (symbol, rel) in owners {
        let source = fs::read_to_string(rel).expect("read route owner");
        assert!(source.contains(symbol), "expected {symbol} in {rel}");
        assert!(
            workflow.contains(rel),
            "route owner {rel} must be in workflow path filters"
        );
    }
}
