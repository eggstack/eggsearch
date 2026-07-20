use eggsearch::core::identity::{chunk_id, code_span_id, locator_id};
use proptest::prelude::*;
use proptest::test_runner::TestRunner;

fn url_strategy() -> impl Strategy<Value = String> {
    "https?://[a-z][a-z0-9-]*\\.[a-z]{2,}(:[0-9]{2,5})?(/[a-zA-Z0-9/_.~-]*)?"
}

#[test]
fn chunk_id_deterministic() {
    let mut runner = TestRunner::default();
    let strat = (
        "doc_[a-f0-9]{16}",
        0usize..100usize,
        "[a-zA-Z0-9/_.-]{1,50}",
    );
    runner
        .run(&strat, |(did, index, path)| {
            let a = chunk_id(&did, index, &path);
            let b = chunk_id(&did, index, &path);
            prop_assert_eq!(a, b);
            Ok(())
        })
        .unwrap();
}

#[test]
fn chunk_id_starts_with_prefix() {
    let id = chunk_id("doc_0123456789abcdef", 0, "intro");
    assert!(
        id.starts_with("chunk_"),
        "id doesn't start with chunk_: {id}"
    );
}

#[test]
fn chunk_id_length() {
    let id = chunk_id("doc_0123456789abcdef", 0, "intro");
    assert_eq!(id.len(), 22);
}

#[test]
fn code_span_id_deterministic() {
    let mut runner = TestRunner::default();
    let strat = (url_strategy(), 1u32..1000u32, 1u32..1000u32, "[a-zA-Z_]+");
    runner
        .run(&strat, |(locator, line_start, line_end, symbol)| {
            let a = code_span_id(&locator, Some(line_start), Some(line_end), Some(&symbol));
            let b = code_span_id(&locator, Some(line_start), Some(line_end), Some(&symbol));
            prop_assert_eq!(a, b);
            Ok(())
        })
        .unwrap();
}

#[test]
fn code_span_id_starts_with_prefix() {
    let id = code_span_id("https://example.com/f.rs", None, None, None);
    assert!(id.starts_with("span_"), "id doesn't start with span_: {id}");
}

#[test]
fn code_span_id_length() {
    let id = code_span_id("https://example.com/f.rs", None, None, None);
    assert_eq!(id.len(), 21);
}

#[test]
fn locator_id_deterministic() {
    let mut runner = TestRunner::default();
    let strat = ("[a-z]{1,10}", "[a-z]{1,10}", "[a-z]{1,10}", "[a-z]{1,10}");
    runner
        .run(&strat, |(owner, repo, ref_name, path)| {
            let loc = eggsearch::core::repo_fetch::RepoLocator {
                kind: eggsearch::core::repo_fetch::RepoLocatorKind::Remote,
                host: Some(eggsearch::core::code_metadata::CodeHost::Github),
                owner: Some(owner),
                repo: Some(repo),
                ref_name: Some(ref_name),
                commit_sha: None,
                path,
                workspace_root: None,
            };
            let a = locator_id(&loc);
            let b = locator_id(&loc);
            prop_assert_eq!(a, b);
            Ok(())
        })
        .unwrap();
}

#[test]
fn locator_id_starts_with_prefix() {
    let loc = eggsearch::core::repo_fetch::RepoLocator {
        kind: eggsearch::core::repo_fetch::RepoLocatorKind::Remote,
        host: Some(eggsearch::core::code_metadata::CodeHost::Github),
        owner: Some("a".into()),
        repo: Some("r".into()),
        ref_name: None,
        commit_sha: None,
        path: "f.rs".into(),
        workspace_root: None,
    };
    let id = locator_id(&loc);
    assert!(id.starts_with("loc_"), "id doesn't start with loc_: {id}");
}

#[test]
fn locator_id_length() {
    let loc = eggsearch::core::repo_fetch::RepoLocator {
        kind: eggsearch::core::repo_fetch::RepoLocatorKind::Remote,
        host: Some(eggsearch::core::code_metadata::CodeHost::Github),
        owner: Some("a".into()),
        repo: Some("r".into()),
        ref_name: None,
        commit_sha: None,
        path: "f.rs".into(),
        workspace_root: None,
    };
    let id = locator_id(&loc);
    assert_eq!(id.len(), 20);
}
