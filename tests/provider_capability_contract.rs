use eggsearch::core::provider::{
    built_in_provider_descriptor, ProviderCapabilities, KNOWN_PROVIDER_IDS,
};
use std::fs;

fn descriptor(id: &str) -> eggsearch::core::provider::ProviderDescriptor {
    built_in_provider_descriptor(id, true, false, true, true, None, None)
        .unwrap_or_else(|| panic!("unknown provider id: {id}"))
}

#[test]
fn capability_matrix_matches_the_pinned_table() {
    const MATRIX: &[(&str, &[&str])] = &[
        ("duckduckgo", &[]),
        ("brave", &[]),
        ("startpage", &[]),
        ("yahoo", &[]),
        ("mojeek", &[]),
        ("searxng", &[]),
        (
            "brave_api",
            &[
                "supports_safe_search",
                "supports_freshness",
                "supports_language",
                "supports_region",
                "supports_news",
                "supports_result_timestamps",
            ],
        ),
        (
            "github_code",
            &[
                "supports_code_search",
                "supports_repo_filter",
                "supports_org_filter",
                "supports_path_filter",
                "supports_language_filter",
                "supports_symbol_hint",
            ],
        ),
        (
            "github_issues",
            &[
                "supports_repo_filter",
                "supports_org_filter",
                "supports_issue_search",
                "supports_result_timestamps",
            ],
        ),
        (
            "github_releases",
            &[
                "supports_repo_filter",
                "supports_release_search",
                "supports_result_timestamps",
            ],
        ),
        (
            "gitlab_code",
            &[
                "supports_code_search",
                "supports_repo_filter",
                "supports_org_filter",
                "supports_path_filter",
            ],
        ),
        (
            "gitlab_issues",
            &[
                "supports_repo_filter",
                "supports_org_filter",
                "supports_issue_search",
                "supports_result_timestamps",
            ],
        ),
        (
            "gitlab_releases",
            &[
                "supports_repo_filter",
                "supports_release_search",
                "supports_result_timestamps",
            ],
        ),
        ("gitea_code", &["supports_code_search"]),
        (
            "gitea_issues",
            &["supports_issue_search", "supports_result_timestamps"],
        ),
        (
            "gitea_releases",
            &["supports_release_search", "supports_result_timestamps"],
        ),
        ("osv", &["supports_security_search"]),
        (
            "github_advisory",
            &[
                "supports_security_search",
                "supports_advisory_lookup_by_id",
                "supports_advisory_lookup_by_package",
            ],
        ),
        (
            "nvd",
            &["supports_security_search", "supports_advisory_lookup_by_id"],
        ),
        (
            "cisa_kev",
            &[
                "supports_advisory_lookup_by_id",
                "supports_exploit_kev_status",
            ],
        ),
        (
            "rustsec",
            &["supports_security_search", "supports_advisory_lookup_by_id"],
        ),
        (
            "crates_io",
            &["supports_package_metadata", "supports_structured_changelog"],
        ),
        (
            "pypi",
            &["supports_package_metadata", "supports_structured_changelog"],
        ),
        (
            "npm_registry",
            &["supports_package_metadata", "supports_structured_changelog"],
        ),
        (
            "go_pkg",
            &["supports_package_metadata", "supports_structured_changelog"],
        ),
        (
            "maven_central",
            &["supports_package_metadata", "supports_structured_changelog"],
        ),
        (
            "nuget",
            &["supports_package_metadata", "supports_structured_changelog"],
        ),
        (
            "rubygems",
            &["supports_package_metadata", "supports_structured_changelog"],
        ),
        (
            "packagist",
            &["supports_package_metadata", "supports_structured_changelog"],
        ),
        (
            "local_workspace",
            &[
                "supports_code_search",
                "supports_path_filter",
                "supports_language_filter",
            ],
        ),
        (
            "openalex",
            &[
                "supports_result_timestamps",
                "supports_scholarly_search",
                "supports_doi_lookup",
            ],
        ),
        (
            "crossref",
            &["supports_scholarly_search", "supports_doi_lookup"],
        ),
        (
            "semantic_scholar",
            &["supports_scholarly_search", "supports_doi_lookup"],
        ),
        (
            "sourcegraph",
            &[
                "supports_code_search",
                "supports_path_filter",
                "supports_language_filter",
                "supports_repo_indexing",
            ],
        ),
        (
            "firecrawl_developer",
            &["supports_repo_filter", "supports_issue_search"],
        ),
        (
            "exa",
            &[
                "supports_freshness",
                "supports_domain_filters",
                "supports_result_timestamps",
            ],
        ),
        (
            "tavily",
            &[
                "supports_safe_search",
                "supports_freshness",
                "supports_language",
                "supports_region",
                "supports_domain_filters",
                "supports_news",
            ],
        ),
        (
            "serpapi",
            &[
                "supports_safe_search",
                "supports_language",
                "supports_region",
            ],
        ),
        (
            "kagi",
            &[
                "supports_safe_search",
                "supports_freshness",
                "supports_region",
                "supports_domain_filters",
                "supports_result_timestamps",
            ],
        ),
        ("wikipedia", &["supports_result_timestamps"]),
        (
            "arxiv",
            &["supports_scholarly_search", "supports_result_timestamps"],
        ),
        (
            "pubmed",
            &["supports_scholarly_search", "supports_result_timestamps"],
        ),
        (
            "hn_algolia",
            &["supports_freshness", "supports_result_timestamps"],
        ),
        ("github_repositories", &["supports_result_timestamps"]),
    ];

    let mut seen = std::collections::HashSet::new();
    for &(id, flags) in MATRIX {
        assert!(seen.insert(id), "duplicate provider id in matrix: {id}");
        assert!(
            KNOWN_PROVIDER_IDS.contains(&id),
            "unknown provider id in matrix: {id}"
        );
        let mut expected = ProviderCapabilities {
            supports_safe_search: false,
            supports_freshness: false,
            supports_language: false,
            supports_region: false,
            supports_domain_filters: false,
            supports_news: false,
            supports_code_search: false,
            supports_repo_filter: false,
            supports_org_filter: false,
            supports_path_filter: false,
            supports_language_filter: false,
            supports_symbol_hint: false,
            supports_issue_search: false,
            supports_release_search: false,
            supports_result_timestamps: false,
            supports_security_search: false,
            supports_package_metadata: false,
            supports_advisory_lookup_by_id: false,
            supports_advisory_lookup_by_package: false,
            supports_exploit_kev_status: false,
            supports_scholarly_search: false,
            supports_doi_lookup: false,
            supports_repo_indexing: false,
            supports_structured_changelog: false,
        };
        for flag in flags {
            match *flag {
                "supports_safe_search" => expected.supports_safe_search = true,
                "supports_freshness" => expected.supports_freshness = true,
                "supports_language" => expected.supports_language = true,
                "supports_region" => expected.supports_region = true,
                "supports_domain_filters" => expected.supports_domain_filters = true,
                "supports_news" => expected.supports_news = true,
                "supports_code_search" => expected.supports_code_search = true,
                "supports_repo_filter" => expected.supports_repo_filter = true,
                "supports_org_filter" => expected.supports_org_filter = true,
                "supports_path_filter" => expected.supports_path_filter = true,
                "supports_language_filter" => expected.supports_language_filter = true,
                "supports_symbol_hint" => expected.supports_symbol_hint = true,
                "supports_issue_search" => expected.supports_issue_search = true,
                "supports_release_search" => expected.supports_release_search = true,
                "supports_result_timestamps" => expected.supports_result_timestamps = true,
                "supports_security_search" => expected.supports_security_search = true,
                "supports_package_metadata" => expected.supports_package_metadata = true,
                "supports_advisory_lookup_by_id" => expected.supports_advisory_lookup_by_id = true,
                "supports_advisory_lookup_by_package" => {
                    expected.supports_advisory_lookup_by_package = true
                }
                "supports_exploit_kev_status" => expected.supports_exploit_kev_status = true,
                "supports_scholarly_search" => expected.supports_scholarly_search = true,
                "supports_doi_lookup" => expected.supports_doi_lookup = true,
                "supports_repo_indexing" => expected.supports_repo_indexing = true,
                "supports_structured_changelog" => expected.supports_structured_changelog = true,
                _ => panic!("unknown capability flag for {id}: {flag}"),
            }
        }
        let actual = built_in_provider_descriptor(id, true, false, true, true, None, None)
            .unwrap_or_else(|| panic!("unknown provider id: {id}"))
            .capabilities;
        assert_eq!(actual, expected, "capability mismatch for {id}");
    }
    assert_eq!(seen.len(), KNOWN_PROVIDER_IDS.len());
    assert!(
        KNOWN_PROVIDER_IDS.iter().all(|id| seen.contains(*id)),
        "matrix must cover every known provider id"
    );
}

#[test]
fn provider_inventory_count_is_stable() {
    assert_eq!(
        KNOWN_PROVIDER_IDS.len(),
        44,
        "KNOWN_PROVIDER_IDS must hold 44 registered provider IDs"
    );
}

#[test]
fn html_scrapers_report_no_native_capabilities() {
    for id in ["duckduckgo", "brave", "startpage", "yahoo", "mojeek"] {
        let caps = descriptor(id).capabilities;
        assert_eq!(
            caps,
            ProviderCapabilities::none(),
            "{id} is an HTML scraper and must report no native capabilities"
        );
    }
}

#[test]
fn brave_api_native_enforcement_matches_documented_contract() {
    let caps = descriptor("brave_api").capabilities;
    assert!(caps.supports_safe_search);
    assert!(caps.supports_freshness);
    assert!(caps.supports_language);
    assert!(caps.supports_region);
    assert!(caps.supports_news);
    assert!(caps.supports_result_timestamps);
    assert!(
        !caps.supports_domain_filters,
        "brave_api must not claim native domain filters"
    );
}

#[test]
fn exa_native_enforcement_matches_documented_contract() {
    let caps = descriptor("exa").capabilities;
    assert!(caps.supports_freshness);
    assert!(caps.supports_domain_filters);
    assert!(caps.supports_result_timestamps);
    assert!(
        !caps.supports_safe_search,
        "exa must not claim native safe-search"
    );
    assert!(
        !caps.supports_language,
        "exa must not claim native language"
    );
    assert!(!caps.supports_region, "exa must not claim native region");
    assert!(!caps.supports_news, "exa must not claim native news");
}

#[test]
fn tavily_native_enforcement_matches_documented_contract() {
    let caps = descriptor("tavily").capabilities;
    assert!(caps.supports_safe_search);
    assert!(caps.supports_freshness);
    assert!(caps.supports_language);
    assert!(caps.supports_region);
    assert!(caps.supports_domain_filters);
    assert!(caps.supports_news);
    assert!(
        !caps.supports_result_timestamps,
        "tavily must not claim result timestamps"
    );
}

#[test]
fn firecrawl_developer_capability_contract() {
    let desc = descriptor("firecrawl_developer");
    assert_eq!(desc.kind, eggsearch::core::provider::ProviderKind::JsonApi);
    assert!(desc.capabilities.supports_issue_search);
    assert!(desc.capabilities.supports_repo_filter);
    assert!(!desc.capabilities.supports_freshness);
    assert!(!desc.capabilities.supports_domain_filters);
    assert!(
        eggsearch::core::provider::OPTIONAL_API_PROVIDER_IDS.contains(&"firecrawl_developer"),
        "firecrawl_developer must remain keyless-optional"
    );
}

#[test]
fn serpapi_native_enforcement_matches_documented_contract() {
    let caps = descriptor("serpapi").capabilities;
    assert!(caps.supports_safe_search);
    assert!(caps.supports_language);
    assert!(caps.supports_region);
    assert!(
        !caps.supports_freshness,
        "tbs value syntax is undocumented upstream, so freshness is not natively enforced"
    );
    assert!(
        !caps.supports_domain_filters,
        "the Google engine exposes no site-restriction parameter"
    );
    assert!(
        !caps.supports_news,
        "news would require the separately billed `tbm=nws` vertical, which eggsearch never requests"
    );
    assert!(
        !caps.supports_result_timestamps,
        "organic results carry no documented per-result timestamp"
    );
}

#[test]
fn kagi_native_enforcement_matches_documented_contract() {
    let caps = descriptor("kagi").capabilities;
    assert!(caps.supports_safe_search);
    assert!(caps.supports_freshness);
    assert!(caps.supports_region);
    assert!(
        caps.supports_domain_filters,
        "kagi maps include/exclude domains onto the documented inline lens"
    );
    assert!(caps.supports_result_timestamps);
    assert!(
        !caps.supports_language,
        "kagi exposes region but no language field"
    );
    assert!(
        !caps.supports_news,
        "the news workflow is a separate result collection eggsearch never requests"
    );
}

#[test]
fn nvd_native_enforcement_matches_documented_contract() {
    let caps = descriptor("nvd").capabilities;
    assert!(caps.supports_security_search);
    assert!(caps.supports_advisory_lookup_by_id);
    assert!(
        !caps.supports_freshness,
        "the keywordSearch request carries no date parameter, so freshness is not natively enforced"
    );
    assert!(!caps.supports_advisory_lookup_by_package);
}

#[test]
fn credentialed_providers_require_operator_credentials() {
    for id in ["serpapi", "kagi"] {
        assert!(
            eggsearch::core::provider::is_api_provider(id),
            "{id} is a required-credential API provider"
        );
        assert_eq!(
            eggsearch::core::provider::credential_requirement(id),
            eggsearch::core::provider::CredentialRequirement::Required
        );
        let desc = descriptor(id);
        assert!(desc.requires_api_key, "{id} must declare a required key");
        assert_eq!(desc.kind, eggsearch::core::provider::ProviderKind::ApiKey);
        assert!(
            !eggsearch::core::provider::is_optional_api_provider(id),
            "{id} must not be keyless"
        );
    }
}

#[test]
fn credentialed_providers_stay_out_of_default_fan_out() {
    use eggsearch::core::config::{ApiProviderConfig, AppConfig};

    let env = "EGGSEARCH_TEST_M002_API_KEY";
    std::env::set_var(env, "test-key-value");
    for id in ["serpapi", "kagi"] {
        let mut cfg = AppConfig::default();
        assert!(
            !cfg.search.default_providers.iter().any(|p| p == id),
            "{id} must never appear in default_providers"
        );
        assert!(
            !cfg.effective_provider_ids().iter().any(|p| p == id),
            "{id} must not be routable without explicit opt-in"
        );
        assert!(
            !cfg.provider_is_available(id),
            "{id} must be unavailable without [search.api.{id}]"
        );
        cfg.search.api.insert(
            id.to_string(),
            ApiProviderConfig {
                enabled: true,
                api_key_env: Some(env.to_string()),
                base_url: None,
            },
        );
        assert!(cfg.provider_is_available(id));
        assert!(cfg.effective_provider_ids().iter().any(|p| p == id));
        assert!(
            !cfg.resolve_providers(&[])
                .expect("defaults")
                .contains(&id.to_string()),
            "{id} must stay out of default fan-out even once configured"
        );
        assert_eq!(
            cfg.resolve_providers(&[id.to_string()]).expect("explicit"),
            vec![id.to_string()],
            "{id} must be reachable through explicit provider selection"
        );
    }
    std::env::remove_var(env);
}

#[test]
fn credentialed_providers_build_only_with_a_resolvable_key() {
    use eggsearch::core::config::ApiProviderConfig;
    use eggsearch::meta::adapter::build_default_engines;

    let api = std::collections::BTreeMap::from([
        (
            "serpapi".to_string(),
            ApiProviderConfig {
                enabled: true,
                api_key_env: Some("EGGSEARCH_TEST_M002_SERPAPI".to_string()),
                base_url: None,
            },
        ),
        (
            "kagi".to_string(),
            ApiProviderConfig {
                enabled: true,
                api_key_env: Some("EGGSEARCH_TEST_M002_KAGI".to_string()),
                base_url: None,
            },
        ),
    ]);
    let requested: Vec<String> = ["serpapi", "kagi"]
        .iter()
        .map(|id| (*id).to_string())
        .collect();
    let (engines, skipped) =
        build_default_engines(&requested, None, None, &api).expect("engines build");
    for id in ["serpapi", "kagi"] {
        assert!(
            !engines.iter().any(|e| e.name() == id),
            "{id} must not be constructed without a resolvable credential"
        );
        let skip = skipped
            .iter()
            .find(|s| s.id == id)
            .unwrap_or_else(|| panic!("{id} must be reported as skipped"));
        assert!(
            skip.reason.contains("missing_api_key"),
            "{id} must report a typed missing-key skip, got: {}",
            skip.reason
        );
    }

    std::env::set_var("EGGSEARCH_TEST_M002_SERPAPI", "serp-key");
    std::env::set_var("EGGSEARCH_TEST_M002_KAGI", "kagi-key");
    let (engines, skipped) =
        build_default_engines(&requested, None, None, &api).expect("engines build");
    for id in ["serpapi", "kagi"] {
        assert!(
            engines.iter().any(|e| e.name() == id),
            "{id} must be constructed once its credential resolves"
        );
        assert!(!skipped.iter().any(|s| s.id == id));
    }
    std::env::remove_var("EGGSEARCH_TEST_M002_SERPAPI");
    std::env::remove_var("EGGSEARCH_TEST_M002_KAGI");
}

#[test]
fn domain_filters_are_native_only_for_exa_kagi_and_tavily() {
    let mut native = Vec::new();
    for id in KNOWN_PROVIDER_IDS {
        let caps = descriptor(id).capabilities;
        if caps.supports_domain_filters {
            native.push(*id);
        }
    }
    native.sort_unstable();
    assert_eq!(native, vec!["exa", "kagi", "tavily"]);
}

#[test]
fn keyless_source_providers_are_inventory_and_keyless() {
    for id in [
        "wikipedia",
        "arxiv",
        "pubmed",
        "hn_algolia",
        "github_repositories",
    ] {
        assert!(
            KNOWN_PROVIDER_IDS.contains(&id),
            "{id} must be part of the canonical provider inventory"
        );
        let desc = descriptor(id);
        assert!(
            !desc.requires_api_key,
            "{id} must remain keyless; availability may not depend on credentials"
        );
        assert_eq!(desc.id, id, "descriptor id must match the inventory id");
        assert!(
            !desc.capabilities.supports_safe_search
                && !desc.capabilities.supports_language
                && !desc.capabilities.supports_region
                && !desc.capabilities.supports_domain_filters
                && !desc.capabilities.supports_news,
            "{id} has no upstream parameter for safe-search, language, region, domains, or news"
        );
    }
}

#[test]
fn keyless_source_providers_stay_out_of_default_fan_out() {
    for id in [
        "wikipedia",
        "arxiv",
        "pubmed",
        "hn_algolia",
        "github_repositories",
    ] {
        let mut cfg = eggsearch::core::config::AppConfig::default();
        assert_eq!(
            cfg.search.providers.get(id),
            Some(&false),
            "{id} must default to disabled so it never joins default fan-out implicitly"
        );
        assert!(
            !cfg.search.default_providers.iter().any(|p| p == id),
            "{id} must never appear in default_providers"
        );
        assert!(
            !cfg.effective_provider_ids().iter().any(|p| p == id),
            "{id} must not be routable without explicit operator opt-in"
        );
        cfg.search.providers.insert(id.to_string(), true);
        assert!(cfg.provider_is_available(id));
        assert!(
            !cfg.resolve_providers(&[])
                .expect("defaults")
                .contains(&id.to_string()),
            "{id} must stay out of default fan-out even once enabled"
        );
        assert_eq!(
            cfg.resolve_providers(&[id.to_string()]).expect("explicit"),
            vec![id.to_string()],
            "{id} must be reachable through explicit provider selection"
        );
    }
}

#[test]
fn scholarly_providers_advertise_scholarly_search() {
    for id in ["openalex", "crossref", "arxiv", "pubmed"] {
        assert!(
            descriptor(id).capabilities.supports_scholarly_search,
            "{id} is a scholarly source"
        );
    }
    assert!(
        !descriptor("wikipedia")
            .capabilities
            .supports_scholarly_search,
        "wikipedia is reference text, not a scholarly index"
    );
}

#[test]
fn keyless_source_providers_preserve_result_timestamps() {
    for id in [
        "wikipedia",
        "arxiv",
        "pubmed",
        "hn_algolia",
        "github_repositories",
    ] {
        assert!(
            descriptor(id).capabilities.supports_result_timestamps,
            "{id} preserves a per-result timestamp in SearchResult::published_at"
        );
    }
}

#[test]
fn hn_algolia_freshness_is_native_but_others_are_not() {
    assert!(
        descriptor("hn_algolia").capabilities.supports_freshness,
        "HN Algolia maps freshness/date-range onto numericFilters"
    );
    for id in ["wikipedia", "arxiv", "pubmed", "github_repositories"] {
        assert!(
            !descriptor(id).capabilities.supports_freshness,
            "{id} has no upstream freshness parameter; claiming it would be an approximation"
        );
    }
}

#[test]
fn github_repositories_is_discovery_not_code_or_indexing() {
    let caps = descriptor("github_repositories").capabilities;
    assert!(!caps.supports_code_search);
    assert!(!caps.supports_repo_indexing);
    assert!(!caps.supports_issue_search);
    assert!(!caps.supports_release_search);
    assert!(caps.summary().contains("result_timestamps"));
}

#[test]
fn arxiv_is_the_only_structured_api_provider_kind() {
    let structured: Vec<&str> = KNOWN_PROVIDER_IDS
        .iter()
        .copied()
        .filter(|id| descriptor(id).kind == eggsearch::core::provider::ProviderKind::StructuredApi)
        .collect();
    assert_eq!(structured, vec!["arxiv"]);
}

#[test]
fn optional_keys_never_gate_keyless_routing_for_new_providers() {
    for id in ["pubmed", "github_repositories"] {
        assert!(
            eggsearch::core::provider::is_optional_api_provider(id),
            "{id} accepts an optional key"
        );
        assert_eq!(
            eggsearch::core::provider::credential_requirement(id),
            eggsearch::core::provider::CredentialRequirement::Optional
        );
        assert!(
            eggsearch::core::provider::provider_configured_state(id, false, false, false),
            "{id} is configured and routable keyless"
        );
    }
    for id in ["wikipedia", "arxiv", "hn_algolia"] {
        assert!(
            !eggsearch::core::provider::is_optional_api_provider(id),
            "{id} has no operator credential and must not advertise one"
        );
    }
}

#[test]
fn keyless_source_providers_build_and_route_keyless() {
    use eggsearch::meta::adapter::build_default_engines;

    let ids: Vec<String> = [
        "wikipedia",
        "arxiv",
        "pubmed",
        "hn_algolia",
        "github_repositories",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect();
    let (engines, skipped) =
        build_default_engines(&ids, None, None, &std::collections::BTreeMap::new())
            .expect("engines build keyless");
    for id in &ids {
        assert!(
            engines.iter().any(|e| e.name() == id),
            "{id} must be reachable through the normal engine builder without credentials"
        );
        assert!(
            !skipped.iter().any(|s| &s.id == id),
            "{id} must never be skipped when explicitly enabled"
        );
    }
}

#[test]
fn documented_capability_prose_matches_descriptors() {
    let agents = fs::read_to_string("AGENTS.md").expect("read AGENTS.md");
    for claim in [
        "supports_domain_filters",
        "currently `exa`, `kagi`, `tavily`",
        "`brave_api` natively enforces safe-search",
        "`exa` natively enforces freshness/date-range, domain filters",
        "`tavily` natively enforces safe-search, freshness/date-range",
        "`serpapi` natively enforces safe-search, language, region",
        "`kagi` natively enforces safe-search, freshness/date-range, region, domain filters",
    ] {
        assert!(
            agents.contains(claim),
            "AGENTS.md must document capability claim: {claim}"
        );
    }
}
