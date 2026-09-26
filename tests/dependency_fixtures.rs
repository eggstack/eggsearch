use eggsearch::core::package::PackageEcosystem;
use eggsearch::core::security_applicability::{DependencyRelation, ParseStatus};
use eggsearch::meta::dependency_parse::parse_dependency_file_report;

fn parse(
    logical_path: &str,
    fixture: &str,
) -> Vec<eggsearch::core::security_applicability::DependencyFinding> {
    let report = parse_dependency_file_report(logical_path, fixture);
    assert_eq!(report.status, ParseStatus::Complete, "{logical_path}");
    report.findings
}

#[test]
fn structured_syntax_errors_never_look_like_empty_success() {
    let cases = [
        ("Cargo.toml", "[dependencies"),
        ("Cargo.lock", "[[package]"),
        ("composer.lock", "{"),
        ("package-lock.json", "{"),
        ("npm-shrinkwrap.json", "{"),
        ("packages.lock.json", "{"),
        ("poetry.lock", "[[package]"),
        ("uv.lock", "[[package]"),
        ("Pipfile.lock", "{"),
    ];
    for (path, invalid) in cases {
        let report = parse_dependency_file_report(path, invalid);
        assert_eq!(report.status, ParseStatus::Malformed, "{path}");
        assert!(report.findings.is_empty(), "{path}");
    }
}

#[test]
fn valid_empty_structured_documents_remain_complete() {
    let cases = [
        ("Cargo.toml", "[package]\nname='empty'\nversion='0.1.0'"),
        ("Cargo.lock", "version = 3\n\n[[package]]\n"),
        ("composer.lock", r#"{"packages":[],"packages-dev":[]}"#),
        ("poetry.lock", "[[package]]\n"),
        ("uv.lock", "[[package]]\n"),
        ("Pipfile.lock", r#"{"default":{},"develop":{}}"#),
        (
            "package-lock.json",
            r#"{"lockfileVersion":3,"packages":{}}"#,
        ),
        (
            "npm-shrinkwrap.json",
            r#"{"lockfileVersion":3,"packages":{}}"#,
        ),
        ("packages.lock.json", r#"{"version":1,"dependencies":{}}"#),
        ("pom.xml", "<project></project>"),
        ("Demo.csproj", "<Project></Project>"),
    ];
    for (path, empty) in cases {
        let report = parse_dependency_file_report(path, empty);
        assert_eq!(report.status, ParseStatus::Complete, "{path}: {report:?}");
        assert!(report.findings.is_empty(), "{path}");
    }
    for (path, invalid) in [("pom.xml", "<project>"), ("Demo.csproj", "<Project>")] {
        assert_eq!(
            parse_dependency_file_report(path, invalid).status,
            ParseStatus::Malformed,
            "{path}"
        );
    }
}

#[test]
fn valid_unknown_structured_shapes_remain_unsupported() {
    for (path, content) in [
        ("Cargo.toml", "[unrecognized]\nvalue = true"),
        ("Pipfile.lock", "{}"),
        ("composer.lock", r#"{"libraries":[]}"#),
    ] {
        assert_eq!(
            parse_dependency_file_report(path, content).status,
            ParseStatus::Unsupported,
            "{path}"
        );
    }
}

#[test]
fn cargo_lock_fixture() {
    let findings = parse(
        "Cargo.lock",
        include_str!("fixtures/dependency_evidence/Cargo.lock.fixture"),
    );
    assert_eq!(findings.len(), 3);
    let git = findings
        .iter()
        .find(|f| f.package == "internal-tool")
        .unwrap();
    assert_eq!(git.exact_version(), Some("0.1.0"));
    assert!(git
        .provenance
        .as_deref()
        .unwrap_or("")
        .contains("git+https"));
    assert!(findings
        .iter()
        .all(|f| f.ecosystem == PackageEcosystem::CratesIo));
}

#[test]
fn cargo_toml_fixture() {
    let findings = parse(
        "Cargo.toml",
        include_str!("fixtures/dependency_evidence/Cargo.toml.fixture"),
    );
    assert_eq!(findings.len(), 8);
    assert!(findings.iter().all(|f| f.exact_version().is_none()));
    let renamed = findings.iter().find(|f| f.package == "real-crate").unwrap();
    assert!(renamed
        .provenance
        .as_deref()
        .unwrap_or("")
        .contains("alias"));
    let target = findings.iter().find(|f| f.package == "winapi").unwrap();
    assert!(target.target_context.is_some());
}

#[test]
fn go_mod_fixture() {
    let findings = parse(
        "go.mod",
        include_str!("fixtures/dependency_evidence/go.mod.fixture"),
    );
    assert_eq!(findings.len(), 3);
    let indirect = findings
        .iter()
        .find(|f| f.package == "github.com/stretchr/testify")
        .unwrap();
    assert_eq!(indirect.relation, Some(DependencyRelation::Transitive));
    let replaced = findings
        .iter()
        .find(|f| f.package == "github.com/gin-gonic/gin")
        .unwrap();
    assert!(replaced
        .provenance
        .as_deref()
        .unwrap_or("")
        .contains("fork"));
    assert!(findings.iter().all(|f| f.exact_version().is_none()));
}

#[test]
fn go_sum_fixture_is_integrity_only() {
    let findings = parse(
        "go.sum",
        include_str!("fixtures/dependency_evidence/go.sum.fixture"),
    );
    assert_eq!(findings.len(), 2);
    assert!(findings.iter().all(|f| !f.has_resolved_evidence()));
    assert!(findings.iter().all(|f| f.integrity_hash.is_some()));
}

#[test]
fn vendor_modules_fixture() {
    let findings = parse(
        "proj/vendor/modules.txt",
        include_str!("fixtures/dependency_evidence/vendor-modules.txt.fixture"),
    );
    assert_eq!(findings.len(), 2);
    let direct = findings
        .iter()
        .find(|f| f.package == "github.com/gin-gonic/gin")
        .unwrap();
    assert_eq!(direct.exact_version(), Some("1.9.1"));
    assert_eq!(direct.relation, Some(DependencyRelation::Direct));
}

#[test]
fn requirements_fixture() {
    let findings = parse(
        "requirements.txt",
        include_str!("fixtures/dependency_evidence/requirements.txt.fixture"),
    );
    assert_eq!(findings.len(), 8);
    assert!(findings.iter().all(|f| f.exact_version().is_none()));
    let direct = findings.iter().find(|f| f.package == "direct").unwrap();
    assert!(direct.reference_value.is_some());
}

#[test]
fn poetry_lock_fixture() {
    let findings = parse(
        "poetry.lock",
        include_str!("fixtures/dependency_evidence/poetry.lock.fixture"),
    );
    assert_eq!(findings.len(), 2);
    let git = findings
        .iter()
        .find(|f| f.package == "private-pkg")
        .unwrap();
    assert_eq!(git.exact_version(), Some("0.9.0"));
    assert!(git
        .provenance
        .as_deref()
        .unwrap_or("")
        .contains("example.com"));
}

#[test]
fn pipfile_lock_fixture() {
    let findings = parse(
        "Pipfile.lock",
        include_str!("fixtures/dependency_evidence/Pipfile.lock.fixture"),
    );
    assert_eq!(findings.len(), 3);
    let dev = findings.iter().find(|f| f.package == "pytest").unwrap();
    assert_eq!(dev.target_context.as_deref(), Some("develop"));
}

#[test]
fn uv_lock_fixture() {
    let findings = parse(
        "uv.lock",
        include_str!("fixtures/dependency_evidence/uv.lock.fixture"),
    );
    assert_eq!(findings.len(), 2);
    assert!(findings.iter().all(|f| f.has_resolved_evidence()));
}

#[test]
fn nuget_lock_fixture() {
    let findings = parse(
        "packages.lock.json",
        include_str!("fixtures/dependency_evidence/packages.lock.json.fixture"),
    );
    assert_eq!(findings.len(), 4);
    let project = findings.iter().find(|f| f.package == "Demo.App").unwrap();
    assert_eq!(project.exact_version(), None);
}

#[test]
fn csproj_fixture() {
    let findings = parse(
        "Demo.csproj",
        include_str!("fixtures/dependency_evidence/app.csproj.fixture"),
    );
    assert_eq!(findings.len(), 3);
    assert!(findings.iter().all(|f| f.exact_version().is_none()));
}

#[test]
fn pom_fixture() {
    let findings = parse(
        "pom.xml",
        include_str!("fixtures/dependency_evidence/pom.xml.fixture"),
    );
    assert_eq!(findings.len(), 3);
    assert!(findings.iter().all(|f| !f.package.contains("managed")));
    assert!(findings.iter().all(|f| !f.package.contains("hidden")));
}

#[test]
fn gradle_lockfile_fixture() {
    let findings = parse(
        "gradle.lockfile",
        include_str!("fixtures/dependency_evidence/gradle.lockfile.fixture"),
    );
    assert_eq!(findings.len(), 2);
    assert!(findings.iter().all(|f| f.has_resolved_evidence()));
}

#[test]
fn build_gradle_fixture() {
    let findings = parse(
        "build.gradle",
        include_str!("fixtures/dependency_evidence/build.gradle.fixture"),
    );
    assert_eq!(findings.len(), 5);
    assert!(findings.iter().all(|f| f.exact_version().is_none()));
}

#[test]
fn gemfile_lock_fixture() {
    let findings = parse(
        "Gemfile.lock",
        include_str!("fixtures/dependency_evidence/Gemfile.lock.fixture"),
    );
    assert_eq!(findings.len(), 4);
    assert!(findings.iter().all(|f| f.has_resolved_evidence()));
}

#[test]
fn composer_lock_fixture() {
    let findings = parse(
        "composer.lock",
        include_str!("fixtures/dependency_evidence/composer.lock.fixture"),
    );
    assert_eq!(findings.len(), 3);
    assert!(findings.iter().all(|f| f.exact_version().is_some()));
}

#[test]
fn npm_v3_lock_fixture() {
    let findings = parse(
        "package-lock.json",
        include_str!("fixtures/dependency_evidence/package-lock.json.fixture"),
    );
    assert_eq!(findings.len(), 4);
    assert!(findings.iter().all(|f| f.package != "demo"));
}

#[test]
fn npm_v1_lock_fixture() {
    let findings = parse(
        "package-lock.json",
        include_str!("fixtures/dependency_evidence/package-lock-v1.json.fixture"),
    );
    assert_eq!(findings.len(), 2);
}

#[test]
fn yarn_berry_fixture() {
    let findings = parse(
        "yarn.lock",
        include_str!("fixtures/dependency_evidence/yarn.lock.fixture"),
    );
    assert_eq!(findings.len(), 2);
    assert!(findings.iter().any(|f| f.exact_version().is_none()));
}

#[test]
fn pnpm_v9_fixture() {
    let findings = parse(
        "pnpm-lock.yaml",
        include_str!("fixtures/dependency_evidence/pnpm-lock.yaml.fixture"),
    );
    assert_eq!(findings.len(), 2);
    assert!(findings.iter().all(|f| !f.package.contains('(')));
}

#[test]
fn workflow_fixture() {
    let findings = parse(
        ".github/workflows/ci.yml",
        include_str!("fixtures/dependency_evidence/workflow.yml.fixture"),
    );
    assert_eq!(findings.len(), 3);
    assert!(findings.iter().all(|f| f.exact_version().is_none()));
}

#[test]
fn dockerfile_fixture() {
    let findings = parse(
        "Dockerfile",
        include_str!("fixtures/dependency_evidence/Dockerfile.fixture"),
    );
    assert_eq!(findings.len(), 2);
    assert!(findings.iter().all(|f| f.package != "builder"));
}

#[test]
fn compose_fixture() {
    let findings = parse(
        "docker-compose.yml",
        include_str!("fixtures/dependency_evidence/docker-compose.yml.fixture"),
    );
    assert_eq!(findings.len(), 2);
}

#[test]
fn fixtures_are_deterministic() {
    let cases = [
        (
            "Cargo.lock",
            include_str!("fixtures/dependency_evidence/Cargo.lock.fixture"),
        ),
        (
            "pom.xml",
            include_str!("fixtures/dependency_evidence/pom.xml.fixture"),
        ),
        (
            "pnpm-lock.yaml",
            include_str!("fixtures/dependency_evidence/pnpm-lock.yaml.fixture"),
        ),
    ];
    for (path, content) in cases {
        let first = parse_dependency_file_report(path, content);
        let second = parse_dependency_file_report(path, content);
        assert_eq!(first, second, "{path}");
    }
}
