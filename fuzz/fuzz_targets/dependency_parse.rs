#![no_main]

use libfuzzer_sys::fuzz_target;
use eggsearch::core::security_applicability::DependencyParserBudget;
use eggsearch::meta::dependency_parse::parse_dependency_file_report;

const FILENAMES: &[&str] = &[
    "Cargo.lock",
    "Cargo.toml",
    "go.mod",
    "go.sum",
    "requirements.txt",
    "poetry.lock",
    "Pipfile.lock",
    "uv.lock",
    "Gemfile.lock",
    "composer.lock",
    "package-lock.json",
    "yarn.lock",
    "pnpm-lock.yaml",
    "pom.xml",
    "gradle.lockfile",
    "build.gradle",
    "packages.lock.json",
    "app.csproj",
    "Dockerfile",
    "docker-compose.yml",
    ".github/workflows/ci.yml",
    "README.md",
    "proj/vendor/modules.txt",
];

fuzz_target!(|data: &[u8]| {
    if data.len() < 2 {
        return;
    }
    let path = FILENAMES[(data[0] as usize) % FILENAMES.len()];
    let content = String::from_utf8_lossy(&data[1..]);
    let first = parse_dependency_file_report(path, &content);
    assert!(first.findings.len() <= DependencyParserBudget::standard().max_findings_per_file);
    let line_count = content.lines().count().max(1) as u32;
    for finding in &first.findings {
        assert!(!finding.package.is_empty());
        if let Some(line) = finding.source_line {
            assert!(line >= 1 && line <= line_count);
        }
        for field in [
            finding.version.as_deref(),
            finding.resolved_version.as_deref(),
            finding.version_requirement.as_deref(),
            finding.reference_value.as_deref(),
            finding.provenance.as_deref(),
            finding.target_context.as_deref(),
            finding.integrity_hash.as_deref(),
        ] {
            assert!(field.is_none_or(|value| !value.is_empty()));
        }
    }
    let second = parse_dependency_file_report(path, &content);
    assert_eq!(first, second);
});
