use eggsearch::core::package::PackageEcosystem;
use eggsearch::core::security_applicability::{
    canonical_package_name, packages_match, DependencyParserBudget,
};
use eggsearch::meta::dependency_parse::parse_dependency_file_report;
use proptest::prelude::*;

fn filename_strategy() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("Cargo.lock".to_string()),
        Just("Cargo.toml".to_string()),
        Just("go.mod".to_string()),
        Just("go.sum".to_string()),
        Just("requirements.txt".to_string()),
        Just("poetry.lock".to_string()),
        Just("Gemfile.lock".to_string()),
        Just("composer.lock".to_string()),
        Just("package-lock.json".to_string()),
        Just("yarn.lock".to_string()),
        Just("pnpm-lock.yaml".to_string()),
        Just("pom.xml".to_string()),
        Just("packages.lock.json".to_string()),
        Just("Dockerfile".to_string()),
        Just("C:\\proj\\Cargo.lock".to_string()),
        Just("proj/vendor/modules.txt".to_string()),
        Just(".github/workflows/ci.yml".to_string()),
        "[a-z]{1,12}\\.(txt|md|json|lock|yml|yaml|toml|xml)",
    ]
}

fn ecosystem_strategy() -> impl Strategy<Value = PackageEcosystem> {
    prop_oneof![
        Just(PackageEcosystem::CratesIo),
        Just(PackageEcosystem::Pypi),
        Just(PackageEcosystem::Npm),
        Just(PackageEcosystem::Go),
        Just(PackageEcosystem::Maven),
        Just(PackageEcosystem::Nuget),
        Just(PackageEcosystem::Rubygems),
        Just(PackageEcosystem::Packagist),
        Just(PackageEcosystem::Oci),
        Just(PackageEcosystem::GithubActions),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn parse_report_is_deterministic(path in filename_strategy(), content in any::<String>()) {
        let first = parse_dependency_file_report(&path, &content);
        let second = parse_dependency_file_report(&path, &content);
        prop_assert_eq!(first, second);
    }

    #[test]
    fn findings_respect_per_file_budget(path in filename_strategy(), content in any::<String>()) {
        let report = parse_dependency_file_report(&path, &content);
        prop_assert!(report.findings.len() <= DependencyParserBudget::standard().max_findings_per_file);
    }

    #[test]
    fn no_empty_package_identity(path in filename_strategy(), content in any::<String>()) {
        let report = parse_dependency_file_report(&path, &content);
        for finding in &report.findings {
            prop_assert!(!finding.package.is_empty(), "empty package in {path}");
        }
    }

    #[test]
    fn optional_text_fields_never_empty_strings(path in filename_strategy(), content in any::<String>()) {
        let report = parse_dependency_file_report(&path, &content);
        for finding in &report.findings {
            for field in [
                finding.version.as_deref(),
                finding.resolved_version.as_deref(),
                finding.version_requirement.as_deref(),
                finding.reference_value.as_deref(),
                finding.provenance.as_deref(),
                finding.target_context.as_deref(),
                finding.integrity_hash.as_deref(),
            ] {
                prop_assert!(field.is_none_or(|v| !v.is_empty()), "empty string field in {path}");
            }
        }
    }

    #[test]
    fn source_lines_within_input(path in filename_strategy(), content in any::<String>()) {
        let line_count = content.lines().count().max(1) as u32;
        let report = parse_dependency_file_report(&path, &content);
        for finding in &report.findings {
            if let Some(line) = finding.source_line {
                prop_assert!(line >= 1 && line <= line_count, "line {line} out of {line_count} in {path}");
            }
        }
    }

    #[test]
    fn manifest_findings_never_carry_resolved_versions(path in filename_strategy(), content in any::<String>()) {
        use eggsearch::core::security_applicability::DependencySource;
        let report = parse_dependency_file_report(&path, &content);
        for finding in &report.findings {
            if finding.source_kind == DependencySource::Manifest {
                prop_assert!(
                    finding.exact_version().is_none(),
                    "manifest finding with resolved version in {path}"
                );
            }
        }
    }

    #[test]
    fn canonicalization_idempotent(ecosystem in ecosystem_strategy(), name in "[A-Za-z0-9._-]{1,40}") {
        let once = canonical_package_name(&ecosystem, &name);
        let twice = canonical_package_name(&ecosystem, &once);
        prop_assert_eq!(once, twice);
    }

    #[test]
    fn package_match_agrees_with_canonical(ecosystem in ecosystem_strategy(), name in "[A-Za-z0-9._-]{1,40}") {
        prop_assert!(packages_match(&ecosystem, &name, &name));
        let canonical = canonical_package_name(&ecosystem, &name);
        prop_assert!(packages_match(&ecosystem, &name, &canonical));
    }
}
