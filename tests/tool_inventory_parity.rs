use std::{collections::BTreeSet, fs, str::FromStr, sync::Arc};

use eggsearch::{
    core::{
        config::{AppConfig, McpToolPreset},
        workflow::{is_stable_tool, sanitize_next_actions, AgentNextAction, STABLE_TOOL_NAMES},
    },
    mcp::{
        advertisement::{self, ALL_PRESETS},
        server::EggsearchServer,
        state::ServerState,
        tool_contract::{self, ALL_CONTRACTS},
    },
    sdk::{EggsearchClient, ToolName},
};

const COMPATIBILITY_ALIASES: &[&str] = &["batch_fetch", "repo_fetch", "web_fetch"];

fn canonical_names() -> Vec<&'static str> {
    ALL_CONTRACTS.iter().map(|contract| contract.name).collect()
}

fn read_repo_file(path: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(path);
    fs::read_to_string(&path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
}

fn section_after<'a>(source: &'a str, heading: &str) -> &'a str {
    let section = source
        .split_once(heading)
        .unwrap_or_else(|| panic!("missing documentation section {heading:?}"))
        .1;
    let end = section.find("\n## ").unwrap_or(section.len());
    &section[..end]
}

fn first_markdown_table(section: &str) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    let mut in_table = false;
    for line in section.lines() {
        let line = line.trim();
        if line.starts_with('|') {
            in_table = true;
            rows.push(
                line.trim_matches('|')
                    .split('|')
                    .map(str::trim)
                    .map(str::to_owned)
                    .collect(),
            );
        } else if in_table {
            break;
        }
    }
    rows
}

fn inline_code_tokens(value: &str) -> Vec<String> {
    value
        .split(char::from(96))
        .skip(1)
        .step_by(2)
        .map(str::to_owned)
        .collect()
}

fn parse_preset(value: &str) -> McpToolPreset {
    match inline_code_tokens(value).first().map(String::as_str) {
        Some("full") => McpToolPreset::Full,
        Some("research") => McpToolPreset::Research,
        Some("code") => McpToolPreset::Code,
        Some("minimal") => McpToolPreset::Minimal,
        other => panic!("unknown documented MCP preset: {other:?}"),
    }
}

fn documented_full_count(value: &str) -> usize {
    let word = value
        .strip_prefix("all ")
        .and_then(|value| value.strip_suffix(" canonical tools"))
        .unwrap_or_else(|| panic!("unexpected full-preset description: {value:?}"));
    word.parse().unwrap_or_else(|_| {
        [
            "zero",
            "one",
            "two",
            "three",
            "four",
            "five",
            "six",
            "seven",
            "eight",
            "nine",
            "ten",
            "eleven",
            "twelve",
            "thirteen",
            "fourteen",
            "fifteen",
            "sixteen",
            "seventeen",
            "eighteen",
            "nineteen",
            "twenty",
        ]
        .iter()
        .position(|candidate| *candidate == word)
        .unwrap_or_else(|| panic!("unrecognized documented tool count: {word:?}"))
    })
}

#[test]
fn canonical_inventory_projections_match_in_both_directions() {
    let contracts = canonical_names();
    let names = tool_contract::tool_names();
    let sdk_names = ToolName::ALL.map(ToolName::as_str).to_vec();
    let stable_names = STABLE_TOOL_NAMES.to_vec();
    let client_names = EggsearchClient::tool_names().collect::<Vec<_>>();
    let preset_union = ALL_PRESETS
        .iter()
        .flat_map(|preset| advertisement::tools_for(*preset).iter().copied())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();

    assert_eq!(contracts, names, "contract names must match tool_names()");
    assert_eq!(
        contracts, sdk_names,
        "contract names must match ToolName::ALL"
    );
    assert_eq!(
        contracts, stable_names,
        "contract names must match STABLE_TOOL_NAMES"
    );
    assert_eq!(
        contracts, client_names,
        "contract names must match SDK discovery"
    );
    assert_eq!(
        contracts, preset_union,
        "contract names must equal the preset union"
    );

    let canonical_set = contracts.iter().copied().collect::<BTreeSet<_>>();
    for preset in ALL_PRESETS {
        let names = advertisement::tools_for(*preset);
        let set = names.iter().copied().collect::<BTreeSet<_>>();
        assert!(
            names.windows(2).all(|pair| pair[0] < pair[1]),
            "{preset:?} preset must be alphabetically ordered: {names:?}"
        );
        assert_eq!(
            set.len(),
            names.len(),
            "{preset:?} preset must be duplicate-free"
        );
        assert!(
            set.is_subset(&canonical_set),
            "{preset:?} preset contains noncanonical names: {names:?}"
        );
    }
}

#[test]
fn compatibility_aliases_remain_routed_but_undiscoverable() {
    let canonical = canonical_names().into_iter().collect::<BTreeSet<_>>();
    let sdk_names = ToolName::ALL.map(ToolName::as_str).to_vec();
    let expected_aliases = COMPATIBILITY_ALIASES
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let state = Arc::new(ServerState::build(AppConfig::default()).expect("build server state"));
    let server = EggsearchServer::new(state);
    let routed = server
        .routed_tool_names()
        .into_iter()
        .collect::<BTreeSet<_>>();
    let routed_aliases = routed
        .iter()
        .filter(|name| !canonical.contains(name.as_str()))
        .map(String::as_str)
        .collect::<BTreeSet<_>>();

    assert_eq!(
        routed_aliases, expected_aliases,
        "routed compatibility alias names changed"
    );
    for alias in COMPATIBILITY_ALIASES {
        assert!(!canonical.contains(alias), "{alias} entered ALL_CONTRACTS");
        assert!(!sdk_names.contains(alias), "{alias} entered ToolName::ALL");
        assert!(
            !STABLE_TOOL_NAMES.contains(alias),
            "{alias} entered STABLE_TOOL_NAMES"
        );
        assert!(routed.contains(*alias), "{alias} must remain routed");
        assert_eq!(ToolName::from_str(alias).unwrap().as_str(), *alias);
        for preset in ALL_PRESETS {
            assert!(
                !advertisement::tools_for(*preset).contains(alias),
                "{alias} entered {preset:?} preset"
            );
        }
    }

    let alias_actions = COMPATIBILITY_ALIASES
        .iter()
        .map(|alias| AgentNextAction {
            tool: (*alias).to_owned(),
            reason_code: "compatibility_alias".to_owned(),
            priority: 1,
            input_template: serde_json::json!({}),
            source_ids: Vec::new(),
            evidence_role: None,
            evidence_gap: None,
            rationale: None,
        })
        .collect();
    assert!(
        sanitize_next_actions(alias_actions).is_empty(),
        "compatibility aliases must stay out of next_actions"
    );

    let mut candidates = canonical.iter().copied().collect::<BTreeSet<_>>();
    candidates.extend(COMPATIBILITY_ALIASES.iter().copied());
    candidates.insert("not_a_canonical_tool");
    for name in candidates {
        let expected = canonical.contains(name);
        assert_eq!(
            tool_contract::is_known_tool(name),
            expected,
            "is_known_tool({name:?})"
        );
        assert_eq!(is_stable_tool(name), expected, "is_stable_tool({name:?})");
    }
}

#[test]
fn documented_preset_table_matches_advertisement() {
    let document = read_repo_file("docs/tool-matrix.md");
    let section = section_after(&document, "## Advertisement presets");
    let rows = first_markdown_table(section);
    let data_rows = rows
        .iter()
        .skip(2)
        .filter(|row| !row.is_empty())
        .collect::<Vec<_>>();
    assert_eq!(
        data_rows.len(),
        ALL_PRESETS.len(),
        "documented preset row count changed"
    );

    let canonical = canonical_names()
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    for preset in ALL_PRESETS {
        let row = data_rows
            .iter()
            .find(|row| parse_preset(&row[0]) == *preset)
            .unwrap_or_else(|| panic!("missing documented {preset:?} preset row"));
        let (documented_names, advertised_count) = if *preset == McpToolPreset::Full {
            let count = documented_full_count(&row[1]);
            (canonical.clone(), count)
        } else {
            let names = inline_code_tokens(&row[1]);
            let count = names.len();
            (names, count)
        };
        let actual = advertisement::tools_for(*preset);
        let mut actual_names = actual
            .iter()
            .map(|name| (*name).to_owned())
            .collect::<Vec<_>>();
        assert_eq!(
            advertised_count,
            actual.len(),
            "documented {preset:?} advertised count differs from tools_for"
        );
        let mut documented_names = documented_names;
        documented_names.sort();
        actual_names.sort();
        assert_eq!(
            documented_names, actual_names,
            "documented {preset:?} tools differ from tools_for"
        );
    }
}

#[test]
fn documented_codegg_canonical_list_matches_contract_names() {
    let document = read_repo_file("architecture/codegg-contract.md");
    let section = section_after(&document, "## 1. Stable Tool Surface");
    let rows = first_markdown_table(section);
    let documented_names = rows
        .iter()
        .skip(2)
        .filter_map(|row| row.first())
        .map(|cell| {
            inline_code_tokens(cell)
                .into_iter()
                .next()
                .expect("tool name cell")
        })
        .collect::<Vec<_>>();
    let canonical = tool_contract::tool_names()
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    assert_eq!(
        documented_names, canonical,
        "CodeGG canonical tool list differs from tool_names()"
    );
}
