use crate::core::config::IntentRoutingMode;
use crate::core::provider::{built_in_provider_descriptor, ProviderCapabilities};
use crate::core::query::SearchIntent;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum IntentCapability {
    None,
    Code,
    Issues,
    Releases,
    Security,
    News,
}

impl IntentCapability {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::None => "generic",
            Self::Code => "code search",
            Self::Issues => "issue search",
            Self::Releases => "release search",
            Self::Security => "security advisory search",
            Self::News => "news search",
        }
    }

    pub(crate) fn holds(self, capabilities: &ProviderCapabilities) -> bool {
        match self {
            Self::None => false,
            Self::Code => capabilities.supports_code_search || capabilities.supports_repo_filter,
            Self::Issues => capabilities.supports_issue_search,
            Self::Releases => capabilities.supports_release_search,
            Self::Security => capabilities.supports_security_search,
            Self::News => capabilities.supports_news,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct IntentProviderPreference {
    pub(crate) intent: SearchIntent,
    pub(crate) providers: &'static [&'static str],
    pub(crate) capability: IntentCapability,
}

pub(crate) const INTENT_PROVIDER_PREFERENCES: [IntentProviderPreference; 7] = [
    IntentProviderPreference {
        intent: SearchIntent::Web,
        providers: &[],
        capability: IntentCapability::None,
    },
    IntentProviderPreference {
        intent: SearchIntent::Docs,
        providers: &[],
        capability: IntentCapability::None,
    },
    IntentProviderPreference {
        intent: SearchIntent::Code,
        providers: &["github_code", "gitlab_code", "gitea_code", "sourcegraph"],
        capability: IntentCapability::Code,
    },
    IntentProviderPreference {
        intent: SearchIntent::Issues,
        providers: &[
            "github_issues",
            "gitlab_issues",
            "gitea_issues",
            "firecrawl_developer",
        ],
        capability: IntentCapability::Issues,
    },
    IntentProviderPreference {
        intent: SearchIntent::Releases,
        providers: &["github_releases", "gitlab_releases", "gitea_releases"],
        capability: IntentCapability::Releases,
    },
    IntentProviderPreference {
        intent: SearchIntent::Security,
        providers: &["osv", "github_advisory", "nvd"],
        capability: IntentCapability::Security,
    },
    IntentProviderPreference {
        intent: SearchIntent::News,
        providers: &["tavily", "brave_api"],
        capability: IntentCapability::News,
    },
];

pub(crate) fn preference_for(intent: SearchIntent) -> &'static IntentProviderPreference {
    INTENT_PROVIDER_PREFERENCES
        .iter()
        .find(|preference| preference.intent == intent)
        .expect("intent preference table is total over SearchIntent")
}

pub(crate) fn provider_supports(id: &str, capability: IntentCapability) -> bool {
    built_in_provider_descriptor(id, true, false, true, true, None, None)
        .is_some_and(|descriptor| capability.holds(&descriptor.capabilities))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RoutingDisposition {
    Added,
    AlreadySelected,
    NotEnabled,
    NotBuilt,
    CoolingDown,
    CapabilityMissing,
    CapExceeded,
}

impl RoutingDisposition {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Added => "added",
            Self::AlreadySelected => "already_selected",
            Self::NotEnabled => "not_enabled",
            Self::NotBuilt => "not_built",
            Self::CoolingDown => "cooling_down",
            Self::CapabilityMissing => "capability_missing",
            Self::CapExceeded => "cap_exceeded",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RoutingAttempt {
    pub(crate) provider_id: &'static str,
    pub(crate) disposition: RoutingDisposition,
}

pub(crate) fn select_intent_additions(
    intent: SearchIntent,
    base: &[String],
    enabled: &[String],
    built: &[String],
    cooling: &[String],
    max_extra: usize,
) -> Vec<RoutingAttempt> {
    select_additions(
        preference_for(intent),
        base,
        enabled,
        built,
        cooling,
        max_extra,
    )
}

fn select_additions(
    preference: &IntentProviderPreference,
    base: &[String],
    enabled: &[String],
    built: &[String],
    cooling: &[String],
    max_extra: usize,
) -> Vec<RoutingAttempt> {
    let contains =
        |haystack: &[String], needle: &str| haystack.iter().any(|candidate| candidate == needle);

    let mut seen: Vec<&'static str> = Vec::new();
    let mut added = 0usize;
    let mut attempts = Vec::with_capacity(preference.providers.len());
    for id in preference.providers {
        if seen.contains(id) {
            continue;
        }
        seen.push(id);
        let disposition = if contains(base, id) {
            RoutingDisposition::AlreadySelected
        } else if !contains(enabled, id) {
            RoutingDisposition::NotEnabled
        } else if !contains(built, id) {
            RoutingDisposition::NotBuilt
        } else if contains(cooling, id) {
            RoutingDisposition::CoolingDown
        } else if !provider_supports(id, preference.capability) {
            RoutingDisposition::CapabilityMissing
        } else if added >= max_extra {
            RoutingDisposition::CapExceeded
        } else {
            added += 1;
            RoutingDisposition::Added
        };
        attempts.push(RoutingAttempt {
            provider_id: id,
            disposition,
        });
    }
    attempts
}

#[cfg(feature = "mock")]
pub(crate) fn select_intent_additions_with_order(
    intent: SearchIntent,
    providers: &'static [&'static str],
    base: &[String],
    enabled: &[String],
    built: &[String],
    cooling: &[String],
    max_extra: usize,
) -> Vec<RoutingAttempt> {
    let preference = IntentProviderPreference {
        intent,
        providers,
        capability: preference_for(intent).capability,
    };
    select_additions(&preference, base, enabled, built, cooling, max_extra)
}

pub(crate) fn added_ids(attempts: &[RoutingAttempt]) -> Vec<String> {
    attempts
        .iter()
        .filter(|attempt| attempt.disposition == RoutingDisposition::Added)
        .map(|attempt| attempt.provider_id.to_string())
        .collect()
}

pub(crate) fn withheld_ids(attempts: &[RoutingAttempt]) -> Vec<String> {
    attempts
        .iter()
        .filter(|attempt| attempt.disposition == RoutingDisposition::CapExceeded)
        .map(|attempt| attempt.provider_id.to_string())
        .collect()
}

pub(crate) fn capable_built_providers(intent: SearchIntent, built: &[String]) -> Vec<String> {
    let preference = preference_for(intent);
    let mut ordered: Vec<String> = Vec::new();
    for id in preference.providers {
        if built.iter().any(|candidate| candidate == id)
            && provider_supports(id, preference.capability)
        {
            ordered.push((*id).to_string());
        }
    }
    let mut rest: Vec<String> = built
        .iter()
        .filter(|id| !ordered.contains(id))
        .filter(|id| provider_supports(id, preference.capability))
        .cloned()
        .collect();
    rest.sort();
    rest.dedup();
    ordered.extend(rest);
    ordered
}

pub(crate) fn capability_exclusion_warning(
    intent: SearchIntent,
    excluded: &[String],
) -> Option<String> {
    if excluded.is_empty() {
        return None;
    }
    let preference = preference_for(intent);
    if preference.capability == IntentCapability::None {
        return None;
    }
    Some(format!(
        "intent_routing_capability_excluded: intent={} requested; enabled native {} provider(s) {} were not routed, so results are from generic/contextual search",
        intent.as_str(),
        preference.capability.label(),
        excluded.join(", ")
    ))
}

pub(crate) fn disclosure(
    intent_mode: IntentRoutingMode,
    max_extra_providers: usize,
) -> serde_json::Value {
    let preferences: serde_json::Map<String, serde_json::Value> = INTENT_PROVIDER_PREFERENCES
        .iter()
        .map(|preference| {
            (
                preference.intent.as_str().to_string(),
                serde_json::json!(preference.providers),
            )
        })
        .collect();
    serde_json::json!({
        "intent_mode": intent_mode.as_str(),
        "max_extra_providers": max_extra_providers,
        "preferences": preferences,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::provider::KNOWN_PROVIDER_IDS;

    fn strings(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    const ALL_INTENTS: [SearchIntent; 7] = [
        SearchIntent::Web,
        SearchIntent::Docs,
        SearchIntent::Code,
        SearchIntent::Issues,
        SearchIntent::Releases,
        SearchIntent::Security,
        SearchIntent::News,
    ];

    #[test]
    fn preference_table_is_total_over_search_intent() {
        assert_eq!(INTENT_PROVIDER_PREFERENCES.len(), ALL_INTENTS.len());
        for intent in ALL_INTENTS {
            let matches: Vec<_> = INTENT_PROVIDER_PREFERENCES
                .iter()
                .filter(|preference| preference.intent == intent)
                .collect();
            assert_eq!(matches.len(), 1, "intent {} is not total", intent.as_str());
        }
        for preference in &INTENT_PROVIDER_PREFERENCES {
            match preference.intent {
                SearchIntent::Web | SearchIntent::Docs => {
                    assert!(
                        preference.providers.is_empty(),
                        "intent {} deliberately has no preference list",
                        preference.intent.as_str()
                    );
                    assert_eq!(preference.capability, IntentCapability::None);
                }
                SearchIntent::Code
                | SearchIntent::Issues
                | SearchIntent::Releases
                | SearchIntent::Security
                | SearchIntent::News => {
                    assert!(
                        !preference.providers.is_empty(),
                        "intent {} must list at least one provider",
                        preference.intent.as_str()
                    );
                    assert_ne!(preference.capability, IntentCapability::None);
                }
            }
        }
    }

    #[test]
    fn preference_table_matches_the_declared_order() {
        let expected: [(SearchIntent, &[&str]); 7] = [
            (SearchIntent::Web, &[]),
            (SearchIntent::Docs, &[]),
            (
                SearchIntent::Code,
                &["github_code", "gitlab_code", "gitea_code", "sourcegraph"],
            ),
            (
                SearchIntent::Issues,
                &[
                    "github_issues",
                    "gitlab_issues",
                    "gitea_issues",
                    "firecrawl_developer",
                ],
            ),
            (
                SearchIntent::Releases,
                &["github_releases", "gitlab_releases", "gitea_releases"],
            ),
            (SearchIntent::Security, &["osv", "github_advisory", "nvd"]),
            (SearchIntent::News, &["tavily", "brave_api"]),
        ];
        for (intent, providers) in expected {
            assert_eq!(
                preference_for(intent).providers,
                providers,
                "preference order for {} drifted",
                intent.as_str()
            );
        }
    }

    #[test]
    fn every_preference_id_is_a_known_provider() {
        for preference in &INTENT_PROVIDER_PREFERENCES {
            for id in preference.providers {
                assert!(
                    KNOWN_PROVIDER_IDS.contains(id),
                    "{} is not in KNOWN_PROVIDER_IDS",
                    id
                );
            }
        }
    }

    #[test]
    fn every_preference_entry_satisfies_its_capability_predicate() {
        for preference in &INTENT_PROVIDER_PREFERENCES {
            for id in preference.providers {
                assert!(
                    provider_supports(id, preference.capability),
                    "{} does not satisfy the {} predicate its warning uses",
                    id,
                    preference.capability.label()
                );
            }
        }
    }

    #[test]
    fn every_intent_routes_only_predicate_holding_providers() {
        for intent in ALL_INTENTS {
            let preference = preference_for(intent);
            let built = strings(&["duckduckgo", "osv", "github_code", "github_issues"]);
            let attempts = select_intent_additions(intent, &[], &built, &built, &[], 4);
            for attempt in &attempts {
                if attempt.disposition == RoutingDisposition::Added {
                    assert!(
                        provider_supports(attempt.provider_id, preference.capability),
                        "{} was added for {} without the capability",
                        attempt.provider_id,
                        intent.as_str()
                    );
                }
            }
        }
    }

    #[test]
    fn disabled_providers_are_never_added() {
        let enabled = strings(&["osv"]);
        let built = strings(&["duckduckgo", "osv", "github_advisory"]);
        let attempts =
            select_intent_additions(SearchIntent::Security, &[], &enabled, &built, &[], 4);
        assert_eq!(added_ids(&attempts), vec!["osv".to_string()]);
        let github_advisory = attempts
            .iter()
            .find(|attempt| attempt.provider_id == "github_advisory")
            .expect("github_advisory is considered");
        assert_eq!(github_advisory.disposition, RoutingDisposition::NotEnabled);
    }

    #[test]
    fn providers_absent_from_the_built_list_are_never_added() {
        let enabled = strings(&["osv", "github_advisory", "nvd"]);
        let built = strings(&["duckduckgo", "osv"]);
        let attempts =
            select_intent_additions(SearchIntent::Security, &[], &enabled, &built, &[], 4);
        assert_eq!(added_ids(&attempts), vec!["osv".to_string()]);
        let github_advisory = attempts
            .iter()
            .find(|attempt| attempt.provider_id == "github_advisory")
            .expect("considered");
        assert_eq!(github_advisory.disposition, RoutingDisposition::NotBuilt);
        let nvd = attempts
            .iter()
            .find(|attempt| attempt.provider_id == "nvd")
            .expect("considered");
        assert_eq!(nvd.disposition, RoutingDisposition::NotBuilt);
    }

    #[test]
    fn cooling_providers_are_never_added() {
        let enabled = strings(&["osv", "github_advisory", "rustsec", "nvd"]);
        let built = enabled.clone();
        let cooling = strings(&["osv"]);
        let attempts =
            select_intent_additions(SearchIntent::Security, &[], &enabled, &built, &cooling, 4);
        assert_eq!(added_ids(&attempts), strings(&["github_advisory", "nvd"]));
        assert_eq!(attempts[0].disposition, RoutingDisposition::CoolingDown);
    }

    #[test]
    fn base_providers_are_never_reordered_or_duplicated() {
        let base = strings(&["osv", "duckduckgo"]);
        let enabled = strings(&["duckduckgo", "osv", "github_advisory", "rustsec", "nvd"]);
        let built = enabled.clone();
        let attempts =
            select_intent_additions(SearchIntent::Security, &base, &enabled, &built, &[], 2);
        assert_eq!(attempts[0].provider_id, "osv");
        assert_eq!(attempts[0].disposition, RoutingDisposition::AlreadySelected);
        assert_eq!(
            added_ids(&attempts),
            strings(&["github_advisory", "nvd"]),
            "additions must follow table order and skip a selected provider"
        );
        assert_eq!(base, strings(&["osv", "duckduckgo"]));
    }

    #[test]
    fn max_extra_zero_reports_withheld_providers_and_adds_nothing() {
        let enabled = strings(&["osv", "github_advisory", "rustsec", "nvd"]);
        let built = enabled.clone();
        let attempts =
            select_intent_additions(SearchIntent::Security, &[], &enabled, &built, &[], 0);
        assert!(added_ids(&attempts).is_empty());
        assert_eq!(
            withheld_ids(&attempts),
            strings(&["osv", "github_advisory", "nvd"])
        );
        for attempt in &attempts {
            assert_eq!(attempt.disposition, RoutingDisposition::CapExceeded);
        }
    }

    #[test]
    fn max_extra_two_adds_exactly_the_first_two() {
        let enabled = strings(&["osv", "github_advisory", "rustsec", "nvd"]);
        let built = enabled.clone();
        let attempts =
            select_intent_additions(SearchIntent::Security, &[], &enabled, &built, &[], 2);
        assert_eq!(added_ids(&attempts), strings(&["osv", "github_advisory"]));
        assert_eq!(withheld_ids(&attempts), strings(&["nvd"]));
    }

    #[test]
    fn max_extra_at_the_cap_adds_every_eligible_provider() {
        let enabled = strings(&["osv", "github_advisory", "rustsec", "nvd"]);
        let built = enabled.clone();
        let attempts = select_intent_additions(
            SearchIntent::Security,
            &[],
            &enabled,
            &built,
            &[],
            crate::core::config::INTENT_ROUTING_MAX_EXTRA_PROVIDERS,
        );
        assert_eq!(
            added_ids(&attempts),
            strings(&["osv", "github_advisory", "nvd"])
        );
        assert!(withheld_ids(&attempts).is_empty());
    }

    #[test]
    fn every_intent_adds_its_own_specialists_only() {
        let enabled = strings(&[
            "github_code",
            "gitlab_code",
            "gitea_code",
            "sourcegraph",
            "github_issues",
            "gitlab_issues",
            "gitea_issues",
            "firecrawl_developer",
            "github_releases",
            "gitlab_releases",
            "gitea_releases",
            "osv",
            "github_advisory",
            "rustsec",
            "nvd",
            "tavily",
            "brave_api",
            "duckduckgo",
        ]);
        let built = enabled.clone();
        let mut routed = Vec::new();
        for intent in ALL_INTENTS {
            let attempts = select_intent_additions(intent, &[], &enabled, &built, &[], 4);
            let added = added_ids(&attempts);
            assert_eq!(
                added,
                preference_for(intent)
                    .providers
                    .iter()
                    .map(|id| id.to_string())
                    .collect::<Vec<_>>()
                    .into_iter()
                    .take(4)
                    .collect::<Vec<_>>(),
                "intent {} routed the wrong providers",
                intent.as_str()
            );
            routed.push((intent, added));
        }
        let code = &routed[2].1;
        assert!(code.contains(&"github_code".to_string()));
        assert!(!code.contains(&"osv".to_string()));
        let docs = &routed[1].1;
        assert!(docs.is_empty(), "docs has no capability-justified provider");
        assert!(routed[0].1.is_empty(), "web has no preference list");
    }

    #[test]
    fn capability_missing_is_reported_instead_of_adding_a_wrong_provider() {
        let preference = IntentProviderPreference {
            intent: SearchIntent::Security,
            providers: &["duckduckgo", "osv"],
            capability: IntentCapability::Security,
        };
        let enabled = strings(&["duckduckgo", "osv"]);
        let built = enabled.clone();
        let attempts = select_additions(&preference, &[], &enabled, &built, &[], 2);
        assert_eq!(attempts[0].provider_id, "duckduckgo");
        assert_eq!(
            attempts[0].disposition,
            RoutingDisposition::CapabilityMissing
        );
        assert_eq!(added_ids(&attempts), strings(&["osv"]));
    }

    #[test]
    fn selection_is_deterministic_and_built_order_independent() {
        let enabled = strings(&["nvd", "rustsec", "osv", "github_advisory"]);
        let forward = strings(&["duckduckgo", "osv", "rustsec"]);
        let reversed = strings(&["rustsec", "osv", "duckduckgo"]);
        let first =
            select_intent_additions(SearchIntent::Security, &[], &enabled, &forward, &[], 2);
        let second =
            select_intent_additions(SearchIntent::Security, &[], &enabled, &forward, &[], 2);
        assert_eq!(first, second);
        assert_eq!(
            added_ids(&first),
            added_ids(&select_intent_additions(
                SearchIntent::Security,
                &[],
                &enabled,
                &reversed,
                &[],
                2
            ))
        );
    }

    #[test]
    fn capable_built_providers_follow_table_order_then_alphabetical() {
        let built = strings(&["nvd", "duckduckgo", "osv", "github_advisory"]);
        assert_eq!(
            capable_built_providers(SearchIntent::Security, &built),
            strings(&["osv", "github_advisory", "nvd"])
        );
        assert!(capable_built_providers(SearchIntent::Web, &built).is_empty());
        assert!(capable_built_providers(SearchIntent::Docs, &built).is_empty());
    }

    #[test]
    fn exclusion_warning_uses_the_declared_template() {
        let message =
            capability_exclusion_warning(SearchIntent::Security, &strings(&["osv", "rustsec"]))
                .expect("a capable-but-unrouted provider produces a warning");
        assert_eq!(
            message,
            "intent_routing_capability_excluded: intent=security requested; enabled native security advisory search provider(s) osv, rustsec were not routed, so results are from generic/contextual search"
        );
        assert!(
            capability_exclusion_warning(SearchIntent::Web, &strings(&["osv"])).is_none(),
            "web has no capability predicate to exclude from"
        );
    }

    #[test]
    fn disclosure_matches_the_constant_table() {
        let defaults = crate::core::config::RoutingSection::default();
        let disclosure = disclosure(defaults.intent_mode, defaults.max_extra_providers);
        assert_eq!(disclosure["intent_mode"], serde_json::json!("off"));
        assert_eq!(
            disclosure["max_extra_providers"],
            serde_json::json!(defaults.max_extra_providers)
        );
        let preferences = disclosure["preferences"].as_object().expect("object");
        assert_eq!(preferences.len(), INTENT_PROVIDER_PREFERENCES.len());
        for preference in &INTENT_PROVIDER_PREFERENCES {
            assert_eq!(
                preferences[preference.intent.as_str()],
                serde_json::json!(preference.providers)
            );
        }
    }

    proptest::proptest! {
        #[test]
        fn additions_never_touch_base_and_stay_bounded(
            base in proptest::collection::vec(
                proptest::sample::select(vec!["duckduckgo", "osv", "github_advisory", "rustsec", "nvd"]),
                0..4,
            ),
            max_extra in 0usize..6usize,
        ) {
            let enabled = strings(&["duckduckgo", "osv", "github_advisory", "rustsec", "nvd"]);
            let built = enabled.clone();
            let base = base.iter().map(|id| (*id).to_string()).collect::<Vec<_>>();
            let attempts = select_intent_additions(SearchIntent::Security, &base, &enabled, &built, &[], max_extra);
            let added = added_ids(&attempts);
            proptest::prop_assert!(added.len() <= max_extra);
            for id in &added {
                proptest::prop_assert!(!base.contains(id));
            }
            let table: Vec<&str> = preference_for(SearchIntent::Security).providers.to_vec();
            let positions: Vec<usize> = added.iter().map(|id| table.iter().position(|entry| entry == id).expect("table id")).collect();
            proptest::prop_assert!(positions.windows(2).all(|pair| pair[0] < pair[1]));
            let repeat = select_intent_additions(SearchIntent::Security, &base, &enabled, &built, &[], max_extra);
            proptest::prop_assert_eq!(attempts, repeat);
        }
    }
}
