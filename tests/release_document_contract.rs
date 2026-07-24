use std::fs;
use std::path::Path;

fn read(path: &str) -> String {
    fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(path))
        .unwrap_or_else(|error| panic!("read {path}: {error}"))
}

#[test]
fn release_record_does_not_promote_pending_evidence() {
    let document = read("docs/release-verification.md");
    let lower = document.to_lowercase();
    assert!(lower.contains("provisional"));
    assert!(lower.contains("native forge"));
    assert!(lower.contains("evidence commit `e`"));
    assert!(!lower.contains("no unbounded memory growth"));
    assert!(!lower.contains("all 9 live-smoke tests pass"));

    let evidence_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/release-evidence");
    if !evidence_root.exists() {
        assert!(lower.contains("pending"));
        return;
    }

    for entry in fs::read_dir(&evidence_root).expect("read release evidence directory") {
        let path = entry.expect("release evidence entry").path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("json") {
            continue;
        }
        let value: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).expect("read release manifest"))
                .expect("parse release manifest");
        let subject = value
            .get("release_subject")
            .and_then(serde_json::Value::as_str)
            .expect("release manifest subject");
        assert!(
            subject.len() == 40 && subject.bytes().all(|byte| byte.is_ascii_hexdigit()),
            "release manifest subject must be a full SHA"
        );
        assert!(
            document.contains(subject),
            "release document must record manifest subject {subject}"
        );
    }
}
