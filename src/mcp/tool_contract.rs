//! Canonical MCP tool contract registry.
//!
//! One authoritative semantic entry per stable tool. The registry is the
//! source of truth for concise purpose text, routing guidance, MCP
//! annotations, discovery keywords, and related/next-tool relationships.
//! Tool-specific argument documentation stays with each argument type.
//! Disclosure hints are advisory for hosts and never enforce execution
//! policy.

use rmcp::model::ToolAnnotations;

/// Maximum model-facing tool description length in bytes.
pub const MAX_TOOL_DESCRIPTION_LEN: usize = 300;

/// Task area a tool primarily serves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ToolDomain {
    Web,
    Fetch,
    Repository,
    Security,
    Research,
    Evidence,
    Diagnostic,
}

impl ToolDomain {
    /// Stable machine-readable domain label.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Web => "web",
            Self::Fetch => "fetch",
            Self::Repository => "repository",
            Self::Security => "security",
            Self::Research => "research",
            Self::Evidence => "evidence",
            Self::Diagnostic => "diagnostic",
        }
    }
}

/// Advisory host disclosure hint. Descriptive only, never policy-enforcing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ToolDisclosureHint {
    Core,
    Deferred,
    Diagnostic,
}

impl ToolDisclosureHint {
    /// Stable machine-readable disclosure label.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Core => "core",
            Self::Deferred => "deferred",
            Self::Diagnostic => "diagnostic",
        }
    }
}

/// Stable semantic metadata for one MCP tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ToolContract {
    /// Canonical `tools/list` name.
    pub name: &'static str,
    /// Concise selection-oriented model-facing description.
    pub description: &'static str,
    /// What the tool does in one phrase.
    pub purpose: &'static str,
    /// When an agent should select it.
    pub use_when: &'static str,
    /// What the tool is explicitly not for.
    pub not_for: &'static str,
    /// Primary task area.
    pub domain: ToolDomain,
    /// Advisory host disclosure hint.
    pub disclosure: ToolDisclosureHint,
    /// Whether the tool modifies its environment.
    pub read_only: bool,
    /// Whether the tool interacts with the open world.
    pub open_world: bool,
    /// Semantically neighboring tools.
    pub related_tools: &'static [&'static str],
    /// Likely follow-up tools after a successful call.
    pub next_tools: &'static [&'static str],
    /// Discovery keywords for deferred/specialist tools.
    pub keywords: &'static [&'static str],
}

impl ToolContract {
    /// MCP annotations derived from this contract. Static hints only.
    pub fn annotations(self) -> ToolAnnotations {
        ToolAnnotations::from_raw(
            None,
            Some(self.read_only),
            None,
            None,
            Some(self.open_world),
        )
    }
}

/// Canonical contracts in deterministic alphabetical name order.
pub static ALL_CONTRACTS: &[ToolContract] = &[
    ToolContract {
        name: "batch_fetch",
        description: "Fetch several known URLs or repo locators in one bounded call. Use for fan-out over selected `items`; not a crawler. Each item reports its own result.",
        purpose: "bounded multi-target fetch over explicit targets",
        use_when: "several search results or suggested fetches are already selected",
        not_for: "search, crawling, or following links",
        domain: ToolDomain::Fetch,
        disclosure: ToolDisclosureHint::Deferred,
        read_only: true,
        open_world: true,
        related_tools: &["repo_fetch", "repo_search", "web_fetch", "web_search"],
        next_tools: &["build_evidence_bundle"],
        keywords: &["batch", "fan-out", "multi-fetch", "suggested fetches"],
    },
    ToolContract {
        name: "build_evidence_bundle",
        description: "Package already-selected search and fetch evidence for handoff. Use with gathered `sources` and `fetches`; does not search, fetch, or summarize.",
        purpose: "deterministic evidence packaging for handoff",
        use_when: "evidence is already gathered and must be handed off without summarization",
        not_for: "search, fetch, synthesis, or summarization",
        domain: ToolDomain::Evidence,
        disclosure: ToolDisclosureHint::Deferred,
        read_only: true,
        open_world: false,
        related_tools: &[
            "batch_fetch",
            "repo_fetch",
            "repo_search",
            "research_search",
            "security_search",
            "web_fetch",
            "web_search",
        ],
        next_tools: &[],
        keywords: &["bundle", "handoff", "evidence", "package"],
    },
    ToolContract {
        name: "provider_status",
        description: "Report configured providers, capabilities, and recipes for diagnostics. Use only when availability is in question; not a normal first research step.",
        purpose: "diagnostic provider and capability report",
        use_when: "provider availability itself is relevant or troubleshooting is required",
        not_for: "ordinary research discovery or as a normal first step",
        domain: ToolDomain::Diagnostic,
        disclosure: ToolDisclosureHint::Diagnostic,
        read_only: true,
        open_world: false,
        related_tools: &[],
        next_tools: &[],
        keywords: &["providers", "capabilities", "diagnostic", "health", "recipes"],
    },
    ToolContract {
        name: "repo_fetch",
        description: "Fetch a known repository file or span by structured locator. Use after `repo_search` with `owner`, `repo`, `path`; for arbitrary URLs use `web_fetch`.",
        purpose: "bounded repository file or span inspection",
        use_when: "a concrete repository file or line span is already known",
        not_for: "repository discovery or arbitrary URL fetching",
        domain: ToolDomain::Repository,
        disclosure: ToolDisclosureHint::Deferred,
        read_only: true,
        open_world: true,
        related_tools: &["batch_fetch", "repo_map", "repo_search", "web_fetch"],
        next_tools: &["build_evidence_bundle"],
        keywords: &["file", "span", "symbol", "locator", "source"],
    },
    ToolContract {
        name: "repo_map",
        description: "Describe a repository layout without file contents. Use to orient before `repo_search` with `owner`, `repo`; not a substitute for search or fetch.",
        purpose: "repository structure discovery without contents",
        use_when: "repository layout is unknown and must be understood before searching",
        not_for: "file contents, code search, or advisory lookup",
        domain: ToolDomain::Repository,
        disclosure: ToolDisclosureHint::Deferred,
        read_only: true,
        open_world: true,
        related_tools: &["repo_fetch", "repo_search"],
        next_tools: &["repo_search"],
        keywords: &["structure", "layout", "tree", "modules", "packages"],
    },
    ToolContract {
        name: "repo_search",
        description: "Discover evidence for a repository across source, docs, issues, releases, and package metadata. Use for codebase investigation; use `repo_fetch` only after a concrete file/span is known.",
        purpose: "structured repository evidence discovery",
        use_when: "investigating a specific codebase, package, or repository",
        not_for: "fetching a known file span or general web lookup",
        domain: ToolDomain::Repository,
        disclosure: ToolDisclosureHint::Core,
        read_only: true,
        open_world: true,
        related_tools: &["repo_fetch", "repo_map", "web_search"],
        next_tools: &["batch_fetch", "repo_fetch", "repo_map"],
        keywords: &["code", "repository", "issues", "releases", "docs"],
    },
    ToolContract {
        name: "research_search",
        description: "Gather multi-source evidence for complex architectural questions. Use when flat search is insufficient with `query`; does not synthesize answers or fetch pages.",
        purpose: "multi-source research evidence discovery",
        use_when: "a complex architectural or comparative question needs grouped evidence",
        not_for: "simple lookups, known-file fetch, or answer synthesis",
        domain: ToolDomain::Research,
        disclosure: ToolDisclosureHint::Deferred,
        read_only: true,
        open_world: true,
        related_tools: &["batch_fetch", "repo_search", "web_fetch", "web_search"],
        next_tools: &["batch_fetch", "build_evidence_bundle", "web_fetch"],
        keywords: &["architecture", "comparison", "multi-source", "workflow", "depth"],
    },
    ToolContract {
        name: "security_search",
        description: "Find vulnerability and advisory evidence with applicability context. Use for CVE/GHSA/advisory questions with `query`; for general background use `web_search`.",
        purpose: "vulnerability and advisory evidence discovery",
        use_when: "assessing CVEs, advisories, or package security posture",
        not_for: "general web lookup or runtime exploitability analysis",
        domain: ToolDomain::Security,
        disclosure: ToolDisclosureHint::Deferred,
        read_only: true,
        open_world: true,
        related_tools: &["batch_fetch", "web_fetch", "web_search"],
        next_tools: &["batch_fetch", "build_evidence_bundle", "web_fetch"],
        keywords: &["cve", "ghsa", "advisory", "vulnerability", "applicability"],
    },
    ToolContract {
        name: "web_fetch",
        description: "Fetch one known HTTP(S) URL with bounded extracted text. Use after search with `url`; for several targets use `batch_fetch`. Do not use for search or crawling.",
        purpose: "bounded single-URL inspection",
        use_when: "one explicit URL from search results or user input must be read",
        not_for: "search, crawling, or following links",
        domain: ToolDomain::Web,
        disclosure: ToolDisclosureHint::Core,
        read_only: true,
        open_world: true,
        related_tools: &["batch_fetch", "web_search"],
        next_tools: &["build_evidence_bundle"],
        keywords: &["fetch", "url", "page", "extract"],
    },
    ToolContract {
        name: "web_search",
        description: "Find candidate public web sources as source cards. Use for general research with `query`; inspect content with `web_fetch`. Returns cards only, never page text.",
        purpose: "general web source discovery",
        use_when: "candidate sources must be discovered before any fetch",
        not_for: "fetching page content or inspecting a known URL",
        domain: ToolDomain::Web,
        disclosure: ToolDisclosureHint::Core,
        read_only: true,
        open_world: true,
        related_tools: &[
            "batch_fetch",
            "repo_search",
            "research_search",
            "security_search",
            "web_fetch",
        ],
        next_tools: &["batch_fetch", "web_fetch"],
        keywords: &["search", "web", "discovery", "sources"],
    },
];

/// Look up a canonical contract by tool name.
pub fn lookup(name: &str) -> Option<&'static ToolContract> {
    ALL_CONTRACTS.iter().find(|c| c.name == name)
}

/// Canonical tool names in deterministic alphabetical order.
pub fn tool_names() -> Vec<&'static str> {
    ALL_CONTRACTS.iter().map(|c| c.name).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_is_sorted_unique_and_complete() {
        let mut names: Vec<&str> = ALL_CONTRACTS.iter().map(|c| c.name).collect();
        let mut sorted = names.clone();
        sorted.sort_unstable();
        assert_eq!(names, sorted, "contracts must stay in alphabetical order");
        let mut dedup = sorted.clone();
        dedup.dedup();
        assert_eq!(dedup.len(), 10, "exactly ten contracts expected");
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), 10);
    }

    #[test]
    fn descriptions_respect_size_budget() {
        for contract in ALL_CONTRACTS {
            assert!(
                contract.description.len() <= MAX_TOOL_DESCRIPTION_LEN,
                "{} description is {} bytes, over budget {}",
                contract.name,
                contract.description.len(),
                MAX_TOOL_DESCRIPTION_LEN
            );
        }
    }
}
