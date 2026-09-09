use eggsearch::core::provider::{
    built_in_provider_descriptor, ProviderCapabilities, KNOWN_PROVIDER_IDS,
};
use std::fs;

fn descriptor(id: &str) -> eggsearch::core::provider::ProviderDescriptor {
    built_in_provider_descriptor(id, true, false, true, true, None, None)
        .unwrap_or_else(|| panic!("unknown provider id: {id}"))
}

#[test]
fn provider_inventory_count_is_stable() {
    assert_eq!(
        KNOWN_PROVIDER_IDS.len(),
        37,
        "KNOWN_PROVIDER_IDS must hold 37 registered provider IDs"
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
fn domain_filters_are_native_only_for_exa_and_tavily() {
    let mut native = Vec::new();
    for id in KNOWN_PROVIDER_IDS {
        let caps = descriptor(id).capabilities;
        if caps.supports_domain_filters {
            native.push(*id);
        }
    }
    native.sort_unstable();
    assert_eq!(native, vec!["exa", "tavily"]);
}

#[test]
fn documented_capability_prose_matches_descriptors() {
    let agents = fs::read_to_string("AGENTS.md").expect("read AGENTS.md");
    for claim in [
        "supports_domain_filters",
        "currently `exa`, `tavily`",
        "`brave_api` natively enforces safe-search",
        "`exa` natively enforces freshness/date-range, domain filters",
        "`tavily` natively enforces safe-search, freshness/date-range",
    ] {
        assert!(
            agents.contains(claim),
            "AGENTS.md must document capability claim: {claim}"
        );
    }
}
