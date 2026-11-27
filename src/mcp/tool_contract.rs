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
    /// Whether the tool may make destructive changes. Only meaningful when
    /// `read_only` is false.
    pub destructive: bool,
    /// Whether repeating an identical call has no additional effect.
    pub idempotent: bool,
    /// Semantically neighboring tools.
    pub related_tools: &'static [&'static str],
    /// Likely follow-up tools after a successful call.
    pub next_tools: &'static [&'static str],
    /// Discovery keywords for deferred/specialist tools.
    pub keywords: &'static [&'static str],
    /// Discovery-only aliases (never wire names) for host catalog indexing.
    pub aliases: &'static [&'static str],
}

impl ToolContract {
    /// MCP annotations derived from this contract. Static hints only.
    pub fn annotations(self) -> ToolAnnotations {
        ToolAnnotations::from_raw(
            None,
            Some(self.read_only),
            Some(self.destructive),
            Some(self.idempotent),
            Some(self.open_world),
        )
    }

    /// Compact discovery text for host catalog indexing (BM25/keyword).
    ///
    /// Concatenates domain, purpose, use-when/not-for, keywords, aliases,
    /// and related/next graphs. Hosts should index this text without
    /// returning full input schemas in ordinary discovery results.
    pub fn discovery_text(self) -> String {
        let mut parts = vec![
            self.name,
            self.domain.as_str(),
            self.disclosure.as_str(),
            self.purpose,
            self.use_when,
            self.not_for,
        ];
        parts.extend(self.keywords.iter().copied());
        parts.extend(self.aliases.iter().copied());
        parts.extend(self.related_tools.iter().copied());
        parts.extend(self.next_tools.iter().copied());
        parts.join(" ")
    }
}

/// Whether a name is one of the nine stable canonical tool names.
///
/// Harnesses must ignore `next_actions` entries whose `tool` fails this
/// check; unknown or malicious names never widen execution authority.
pub fn is_known_tool(name: &str) -> bool {
    ALL_CONTRACTS.iter().any(|c| c.name == name)
}

/// Canonical contracts in deterministic alphabetical name order.
pub static ALL_CONTRACTS: &[ToolContract] = &[
    ToolContract {
        name: "build_evidence_bundle",
        description: "Package selected evidence for handoff; pass prior output in `sources` or `fetches` (at least one required). Does not search, fetch, or summarize.",
        purpose: "deterministic evidence packaging for handoff",
        use_when: "evidence is already gathered and must be handed off without summarization",
        not_for: "search, fetch, synthesis, or summarization",
        domain: ToolDomain::Evidence,
        disclosure: ToolDisclosureHint::Deferred,
        read_only: true,
        open_world: false,
        destructive: false,
        idempotent: true,
        related_tools: &[
            "fetch",
            "repo_search",
            "research_search",
            "security_search",
            "web_search",
        ],
        next_tools: &[],
        keywords: &["bundle", "handoff", "evidence", "package"],
        aliases: &["bundle", "evidence", "handoff"],
    },
    ToolContract {
        name: "fetch",
        description: "Fetch explicitly selected web URLs and repository targets, or return a bounded site URL inventory without fetching discovered targets. Repo items take a structured locator with optional symbol or line span. Site discovery does not traverse discovered URLs.",
        purpose: "bounded web and repository fetching plus site URL inventory discovery",
        use_when: "explicit web or repo targets should be read, or a bounded site URL inventory is needed before selecting targets",
        not_for: "search, recursive crawling, or fetching discovered URLs",
        domain: ToolDomain::Fetch,
        disclosure: ToolDisclosureHint::Core,
        read_only: true,
        open_world: true,
        destructive: false,
        idempotent: true,
        related_tools: &["repo_search", "web_search"],
        next_tools: &["build_evidence_bundle"],
        keywords: &[
            "url", "page", "extract", "read", "inspect", "repository", "file", "multiple",
            "offset", "limit", "locator", "symbol", "span", "lines", "site",
            "sitemap", "inventory", "discovery",
        ],
        aliases: &["web", "repository", "page", "read"],
    },
    ToolContract {
        name: "knowledge_brief",
        description: "Rank candidate developments whose dated evidence is after a model cutoff. Use with `topic` and optional `model_id`; does not synthesize answers or claim a model lacks a fact.",
        purpose: "cutoff-aware candidate development evidence",
        use_when: "a topic needs dated evidence compared with a model cutoff",
        not_for: "answer synthesis, claiming a model lacks a fact, or a lookup that does not need a cutoff",
        domain: ToolDomain::Research,
        disclosure: ToolDisclosureHint::Deferred,
        read_only: true,
        open_world: true,
        destructive: false,
        idempotent: true,
        related_tools: &["fetch", "research_search", "web_search"],
        next_tools: &["fetch", "research_search"],
        keywords: &[
            "cutoff",
            "knowledge",
            "brief",
            "model",
            "temporal",
            "delta",
            "developments",
        ],
        aliases: &["cutoff_brief", "knowledge"],
    },
    ToolContract {
        name: "provider_status",
        description: "Report routable providers by default; request diagnostic detail for the full capability matrix. Use only when availability is in question; it is not a normal first research step.",
        purpose: "diagnostic provider and capability report",
        use_when: "provider availability itself is relevant or troubleshooting is required",
        not_for: "ordinary research discovery or as a normal first step",
        domain: ToolDomain::Diagnostic,
        disclosure: ToolDisclosureHint::Diagnostic,
        read_only: true,
        open_world: false,
        destructive: false,
        idempotent: true,
        related_tools: &[],
        next_tools: &[],
        keywords: &["providers", "capabilities", "diagnostic", "health", "recipes"],
        aliases: &["providers", "health", "diagnostics"],
    },
    ToolContract {
        name: "repo_map",
        description: "Describe a repository layout without file contents. Requires forge identity via `owner` and `repo`; a matching local checkout may enrich the result.",
        purpose: "repository structure discovery without contents",
        use_when: "repository layout is unknown and must be understood before searching",
        not_for: "file contents, code search, or advisory lookup",
        domain: ToolDomain::Repository,
        disclosure: ToolDisclosureHint::Deferred,
        read_only: true,
        open_world: true,
        destructive: false,
        idempotent: true,
        related_tools: &["repo_search"],
        next_tools: &["repo_search"],
        keywords: &[
            "structure",
            "layout",
            "tree",
            "modules",
            "packages",
            "map",
            "orient",
        ],
        aliases: &["map", "structure", "layout"],
    },
    ToolContract {
        name: "repo_search",
        description: "Discover repository evidence across source, docs, issues, releases, and packages. A `symbol` requires a non-empty `query` or a repository locator (`owner` and `repo`).",
        purpose: "structured repository evidence discovery",
        use_when: "investigating a specific codebase, package, or repository",
        not_for: "fetching a known file span or general web lookup",
        domain: ToolDomain::Repository,
        disclosure: ToolDisclosureHint::Core,
        read_only: true,
        open_world: true,
        destructive: false,
        idempotent: true,
        related_tools: &["fetch", "repo_map", "web_search"],
        next_tools: &["fetch", "repo_map"],
        keywords: &[
            "code",
            "repository",
            "issues",
            "releases",
            "docs",
            "find",
            "definition",
            "symbol",
        ],
        aliases: &["code_search", "repo", "codebase"],
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
        destructive: false,
        idempotent: true,
        related_tools: &["fetch", "repo_search", "fetch", "web_search"],
        next_tools: &["fetch", "build_evidence_bundle"],
        keywords: &[
            "architecture",
            "comparison",
            "multi-source",
            "workflow",
            "depth",
            "decision",
            "survey",
        ],
        aliases: &["deep_research", "compare", "research"],
    },
    ToolContract {
        name: "security_search",
        description: "Find vulnerability and advisory evidence. Requires at least one of `query`, `package`, `cve_id`, `ghsa_id`, `osv_id`, or `rustsec_id`; use `web_search` for background.",
        purpose: "vulnerability and advisory evidence discovery",
        use_when: "assessing CVEs, advisories, or package security posture",
        not_for: "general web lookup or runtime exploitability analysis",
        domain: ToolDomain::Security,
        disclosure: ToolDisclosureHint::Deferred,
        read_only: true,
        open_world: true,
        destructive: false,
        idempotent: true,
        related_tools: &["fetch", "fetch", "web_search"],
        next_tools: &["fetch", "build_evidence_bundle", "fetch"],
        keywords: &[
            "cve",
            "ghsa",
            "advisory",
            "vulnerability",
            "applicability",
            "package",
            "version",
            "kev",
            "osv",
        ],
        aliases: &["cve", "vuln", "advisory", "security"],
    },
    ToolContract {
        name: "web_search",
        description: "Find candidate public web sources as source cards. Use for general research with `query`; inspect content with `fetch`. Returns source cards only, never page text or a synthesized answer.",
        purpose: "general web source discovery",
        use_when: "candidate sources must be discovered before any fetch",
        not_for: "fetching page content or inspecting a known URL",
        domain: ToolDomain::Web,
        disclosure: ToolDisclosureHint::Core,
        read_only: true,
        open_world: true,
        destructive: false,
        idempotent: true,
        related_tools: &[
            "fetch",
            "repo_search",
            "research_search",
            "security_search",
            "fetch",
        ],
        next_tools: &["fetch", "fetch"],
        keywords: &["search", "web", "discovery", "sources", "lookup", "general"],
        aliases: &["websearch", "search", "lookup"],
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
        assert_eq!(dedup.len(), 9, "exactly nine contracts expected");
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), 9);
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
