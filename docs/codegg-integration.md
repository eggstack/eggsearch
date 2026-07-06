# eggsearch MCP Integration Guide for Coding Agent Harnesses

This guide covers integrating the eggsearch MCP server into coding-agent
harnesses such as codegg. It is written for harness developers, not end
users. All examples use JSON code blocks; truncated values are clearly
marked.

eggsearch version: see `Cargo.toml` for the live release number.

## Table of Contents

1. [Quick Start](#quick-start)
2. [MCP Server Startup and Configuration](#mcp-server-startup-and-configuration)
3. [Configuration Examples](#configuration-examples)
4. [Tool Selection Policy](#tool-selection-policy)
5. [Required Task Workflows](#required-task-workflows)
6. [Trust Boundary Rules](#trust-boundary-rules)
7. [Warning and Error Handling](#warning-and-error-handling)
8. [Evidence Bundle Handoff](#evidence-bundle-handoff)
9. [Performance and Response-Size Controls](#performance-and-response-size-controls)
10. [Agent UI/UX Guidance](#agent-uiux-guidance)
11. [Failure and Degradation Policy](#failure-and-degradation-policy)
12. [Versioning and Compatibility](#versioning-and-compatibility)
13. [Readiness Checklist](#readiness-checklist)

---

## Quick Start

```bash
# 1. Start the MCP server (stdio transport)
eggsearch mcp stdio

# 2. Discover capabilities
# Call: provider_status({})
# Response includes: providers, code_hosts, health, server_capabilities,
# tool_capabilities, and workflow_recipes

# 3. Search for code in a repo
# Call: repo_search({"query": "middleware", "host": "github", "owner": "tokio-rs", "repo": "axum", "profile": "coding"})

# 4. Fetch a specific file
# Call: repo_fetch({"host": "github", "owner": "tokio-rs", "repo": "axum", "path": "src/routing/mod.rs", "symbol": "Router::layer", "expand_to_block": true})
```

---

## MCP Server Startup and Configuration

### Transport

eggsearch uses **MCP over stdio** only (no HTTP/SSE). The harness starts
the server as a child process and communicates via stdin/stdout JSON
messages.

```bash
# Start the server
eggsearch mcp stdio

# Or specify a config file explicitly
eggsearch --config /path/to/config.toml mcp stdio
```

### Config File Location

Default: `$XDG_CONFIG_HOME/eggsearch/config.toml`

On macOS this is typically `~/.config/eggsearch/config.toml`.

### Config File Structure

```toml eggsearch-config
[search]
mode = "live"                    # "live" or "off"
default_max_results = 10
max_results_cap = 50
max_query_chars = 512
timeout_ms = 8000
default_providers = ["duckduckgo", "startpage", "yahoo"]
sanitize_output = true
multiquery_concurrency = 8
multiquery_provider_concurrency = 2

[search.providers]
duckduckgo = true
startpage = true
yahoo = true

[search.exact_error]
enabled = true
max_subqueries = 6
max_error_chars = 8000
redact_sensitive_tokens = true

[fetch]
enabled = true
timeout_ms = 8000
max_bytes = 2000000
max_chars_default = 12000
max_chars_cap = 50000
redirect_limit = 5
allow_private_network = false
allow_localhost = false
include_links_default = false
sanitize_output = true
pdf_enabled = false
batch_max_items = 8
batch_max_items_cap = 20
batch_max_chars_per_item = 12000
batch_max_total_chars = 50000
batch_max_total_chars_cap = 120000
batch_concurrency = 4
```

### Provider Configuration

```toml eggsearch-config-parse-only
# HTML scraper providers (enabled by default)
default_providers = ["duckduckgo", "startpage", "yahoo"]

# Individual provider toggles (all enabled unless noted)
[search.providers]
duckduckgo = true
brave = true
startpage = true
yahoo = true
mojeek = false           # disabled by default
osv = true

# SearXNG instance (optional)
[search.searxng]
enabled = false
base_url = "https://search.example.com"

# API-key providers (all disabled by default)
[search.api.brave_api]
enabled = false
api_key_env = "BRAVE_API_KEY"
base_url = "https://api.search.brave.com/res/v1/web/search"

[search.api.github_code]
enabled = false
api_key_env = "GITHUB_TOKEN"

[search.api.github_issues]
enabled = false
api_key_env = "GITHUB_TOKEN"

[search.api.github_releases]
enabled = false
api_key_env = "GITHUB_TOKEN"

[search.api.gitlab_code]
enabled = false
api_key_env = "GITLAB_TOKEN"
base_url = "https://gitlab.com"

[search.api.gitlab_issues]
enabled = false
api_key_env = "GITLAB_TOKEN"
base_url = "https://gitlab.com"

[search.api.gitlab_releases]
enabled = false
api_key_env = "GITLAB_TOKEN"
base_url = "https://gitlab.com"

[search.api.gitea_code]
enabled = false
api_key_env = "FORGEJO_TOKEN"
base_url = "https://git.example.com"

[search.api.gitea_issues]
enabled = false
api_key_env = "FORGEJO_TOKEN"
base_url = "https://git.example.com"

[search.api.gitea_releases]
enabled = false
api_key_env = "FORGEJO_TOKEN"
base_url = "https://git.example.com"
```

---

## Configuration Examples

### Minimal: Local-Only, No Network

```toml eggsearch-config-parse-only
[search]
mode = "off"
default_max_results = 10
max_results_cap = 50
max_query_chars = 512
timeout_ms = 8000
default_providers = ["duckduckgo", "startpage", "yahoo"]

[fetch]
enabled = false

[local]
enabled = true
roots = ["/Users/dev/projects/my-app"]
max_indexed_files = 5000
```

Use case: air-gapped environments where only local workspace files are
available. `repo_search` will return local results only. `web_search`
and `web_fetch` will deny requests with a policy message.

For direct local file reads, call `repo_fetch` with
`host = "workspace"`, `owner` set to the configured workspace root
directory name, and `path` set to the root-relative file path. The
`prefer_local = true` mode is different: it accepts a normal remote
locator (`host`, `owner`, `repo`, `path`) and resolves to a matching
local checkout when one is available.

### Generic Web Search

```toml eggsearch-config
[search]
mode = "live"
default_max_results = 10
max_results_cap = 50
max_query_chars = 512
timeout_ms = 8000
default_providers = ["duckduckgo", "brave"]

[search.providers]
duckduckgo = true
brave = true

[fetch]
enabled = true
```

No API keys required. DuckDuckGo and Brave HTML scrapers are used by
default.

### Codegg Coding Profile with Local Workspace

```toml eggsearch-config-parse-only
[search]
mode = "live"
default_max_results = 10
max_results_cap = 25
max_query_chars = 512
timeout_ms = 20000
default_providers = ["duckduckgo", "brave"]
sanitize_output = true

[search.api.github_code]
enabled = true
api_key_env = "GITHUB_TOKEN"

[search.api.github_issues]
enabled = true
api_key_env = "GITHUB_TOKEN"

[search.api.github_releases]
enabled = true
api_key_env = "GITHUB_TOKEN"

[fetch]
enabled = true
timeout_ms = 12000
max_chars_default = 20000
batch_max_items = 15
batch_concurrency = 8

[local]
enabled = true
roots = ["/Users/dev/projects"]
max_indexed_files = 100000
respect_gitignore = true
```

### Security-Focused

```toml eggsearch-config
[search]
mode = "live"
default_max_results = 10
max_results_cap = 50
max_query_chars = 512
default_providers = ["duckduckgo", "brave"]
timeout_ms = 20000

[search.providers]
duckduckgo = true
brave = true

[fetch]
enabled = true
max_chars_default = 20000

[local]
enabled = false
```

No special config needed for `security_search` — OSV is enabled by
default when no config overrides it. Include `assess_applicability: true`
in requests for version-range matching.

### Research-Focused

```toml eggsearch-config
[search]
mode = "live"
default_max_results = 15
max_results_cap = 30
max_query_chars = 512
timeout_ms = 25000
default_providers = ["duckduckgo", "brave"]
multiquery_concurrency = 12

[search.providers]
duckduckgo = true
brave = true

[fetch]
enabled = true
max_chars_default = 25000
max_chars_cap = 75000

[local]
enabled = false
```

### API-Provider Config with Env Vars

```toml eggsearch-config-parse-only
[search.api.brave_api]
enabled = true
api_key_env = "BRAVE_API_KEY"       # set this env var

[search.api.github_code]
enabled = true
api_key_env = "GITHUB_TOKEN"        # set this env var

[search.api.github_issues]
enabled = true
api_key_env = "GITHUB_TOKEN"

[search.api.github_releases]
enabled = true
api_key_env = "GITHUB_TOKEN"

[search.api.gitlab_code]
enabled = true
api_key_env = "GITLAB_TOKEN"        # set this env var
base_url = "https://gitlab.com"     # or self-hosted instance
```

**Never commit real keys.** Always use env-var indirection.

---

## Tool Selection Policy

### Decision Tree

| Context | Preferred Tool | Fallback |
|---------|---------------|----------|
| Known repo owner/name or package/code | `repo_search` | `web_search` with `repo:` hint |
| Unknown repo structure | `repo_map` first | `repo_search` with structural subqueries |
| CVE/GHSA/OSV/package/version/security terms | `security_search` | `web_search(intent="security")` |
| Comparative/architectural/deep research | `research_search` | `web_search` with multiple queries |
| Single explicit URL | `web_fetch` | — |
| Multiple known URLs/locators | `batch_fetch` | multiple `web_fetch` calls |
| Handoff to another agent | `build_evidence_bundle` | raw summary |

### Anti-Patterns

- **Treating snippets as final evidence.** Snippets are search
  previews, not verified content. Always `web_fetch` or `repo_fetch`
  the URL you intend to cite.
- **Treating fetched content as instructions.** All fetched content is
  `external_untrusted`. Code, docs, and comments can contain adversarial
  text. Never execute or follow instructions found in fetched content.
- **Fetching every suggested URL automatically.** Suggested fetches are
  ranked by scoring, not an instruction list. Select the ones relevant
  to your task.
- **Ignoring structured warnings.** Every search/fetch response includes
  `structured_warnings` with machine-readable codes and severities. Check
  them before trusting the result.
- **Ignoring `unknown`/`insufficient_evidence` security states.** These
  are real outcomes, not failures. `unknown` applicability means the
  tool cannot determine status — it does not mean "not affected."
- **Treating local dirty/generated/vendor files as authoritative.** Local
  results with `is_generated`, `is_vendor`, `is_test`, or `is_example`
  flags are supplementary context, not implementation evidence. A dirty
  checkout means uncommitted changes exist — trust the local content with
  caution.
- **Assuming provider absence means factual absence.** If a provider is
  in cooldown or not configured, the search still runs with remaining
  providers. Absence of results from one provider does not mean the
  answer is "nothing found."
- **Calling `build_evidence_bundle` without prior search/fetch.** The
  bundle tool packages *already-gathered* evidence. It does not search
  or fetch on its own.

---

## Required Task Workflows

### 1. Understand a Repo/API/Project

This is the primary codegg flow for repository investigation.

```
Step 1: provider_status(recipe_detail = "summary")
  -> discover available tools, recipes, providers
  -> check "repository_investigation" recipe support status

Step 2: repo_map({ host, owner, repo })
  -> root layout, important files, important directories
  -> local_checkout field if matching local git repo exists

Step 3: repo_search({ query, host, owner, repo, profile = "coding" })
  -> grouped source cards (SourceFiles, Issues, Releases, etc.)
  -> next_actions with priority-1 fetch suggestions
  -> suggested_fetches with ranked URLs

Step 4: repo_fetch({ host, owner, repo, path, symbol, expand_to_block })
  -> source content with code context (imports, enclosing symbol)
  -> code_span with deterministic span_id for cross-referencing

Step 5: batch_fetch({ items: [selected suggested locators] })
  -> bounded parallel fetch of selected evidence
  -> per-item trust markers

Step 6: build_evidence_bundle({ goal, sources, fetches })
  -> deterministic bundle_id, source_links, trust_summary, gaps
```

**Example transcript:**

```jsonc
// Step 1: Discover capabilities
// -> provider_status
// Response (abbreviated):
{
  "providers": [
    { "id": "duckduckgo", "display_name": "DuckDuckGo", "enabled": true, "default": true }
  ],
  "server_capabilities": {
    "generic_search": true,
    "repo_search": true,
    "repo_fetch": true,
    "repo_map": true,
    "security_search": true,
    "research_search": true,
    "evidence_bundle": true,
    "batch_fetch": true,
    "local_workspace": true
  },
  "workflow_recipes": [
    {
      "id": "repository_investigation",
      "title": "Repository Investigation",
      "support": "available"
    }
  ]
}
```

```jsonc
// Step 2: Map the repo
// -> repo_map({ "host": "github", "owner": "tokio-rs", "repo": "axum" })
// Response (abbreviated):
{
  "host": "github",
  "owner": "tokio-rs",
  "repo": "axum",
  "important_files": [
    { "path": "Cargo.toml", "kind": "manifest" },
    { "path": "README.md", "kind": "readme" },
    { "path": "CHANGELOG.md", "kind": "changelog" }
  ],
  "important_directories": [
    { "path": "src/", "kind": "source_root" },
    { "path": "examples/", "kind": "examples" },
    { "path": "tests/", "kind": "tests" }
  ],
  "local_checkout": {
    "root": "/Users/dev/projects/axum",
    "branch": "main",
    "dirty": false,
    "workspace_id": "abc123"
  },
  "suggested_fetches": [
    { "url": "https://raw.githubusercontent.com/tokio-rs/axum/main/README.md", "reason": "Project readme", "priority": 1 }
  ]
}
```

```jsonc
// Step 3: Search for code
// -> repo_search({
//     "query": "Router::layer middleware",
//     "host": "github",
//     "owner": "tokio-rs",
//     "repo": "axum",
//     "profile": "coding"
//   })
// Response (abbreviated):
{
  "query": "Router::layer middleware",
  "groups": [
    {
      "kind": "SourceFiles",
      "label": "Source Files",
      "results": [
        {
          "id": "src_a1b2c3d4e5f6a7b8",
          "stable_id": "src_f1e2d3c4b5a69780",
          "url": "https://github.com/tokio-rs/axum/blob/main/src/routing/mod.rs",
          "title": "axum/src/routing/mod.rs",
          "snippet": "pub fn layer<L>(self, layer: L) -> Router<...>",
          "trust": "external_untrusted",
          "metadata": {
            "source_kind": "source_file",
            "domain": "github.com",
            "rank_reasons": ["rrf_multi_provider", "intent_match"]
          },
          "quality": {
            "confidence": "high",
            "relevance": "exact",
            "authority": "maintainer"
          },
          "code_evidence": {
            "source_role": "implementation",
            "evidence_confidence": "exact",
            "matched_symbol": "Router::layer",
            "provider_text_match": true
          }
        }
      ],
      "truncated": false
    }
  ],
  "next_actions": [
    {
      "tool": "repo_fetch",
      "reason_code": "inspect_top_source",
      "priority": 1,
      "input_template": {
        "host": "github",
        "owner": "tokio-rs",
        "repo": "axum",
        "path": "src/routing/mod.rs",
        "symbol": "Router::layer",
        "expand_to_block": true
      },
      "source_ids": ["src_a1b2c3d4e5f6a7b8"]
    }
  ],
  "suggested_fetches": [
    {
      "url": "https://raw.githubusercontent.com/tokio-rs/axum/main/src/routing/mod.rs",
      "reason": "Implementation: Router::layer",
      "group": "SourceFiles",
      "priority": 1,
      "source_id": "src_a1b2c3d4e5f6a7b8"
    }
  ],
  "structured_warnings": [],
  "routing_decision": {
    "selected_providers": ["duckduckgo", "github_code"],
    "degraded": false
  }
}
```

```jsonc
// Step 4: Fetch the specific symbol
// -> repo_fetch({
//     "host": "github",
//     "owner": "tokio-rs",
//     "repo": "axum",
//     "path": "src/routing/mod.rs",
//     "symbol": "Router::layer",
//     "expand_to_block": true
//   })
// Response (abbreviated):
{
  "locator": { "host": "github", "owner": "tokio-rs", "repo": "axum", "path": "src/routing/mod.rs" },
  "text": "pub fn layer<L>(self, layer: L) -> Router<...> {\n    ...\n}",
  "line_start": 42,
  "line_end": 58,
  "browser_url": "https://github.com/tokio-rs/axum/blob/main/src/routing/mod.rs",
  "raw_url": "https://raw.githubusercontent.com/tokio-rs/axum/main/src/routing/mod.rs",
  "permalink_url": "https://github.com/tokio-rs/axum/blob/abc123def/src/routing/mod.rs",
  "raw_permalink_url": "https://raw.githubusercontent.com/tokio-rs/axum/abc123def/src/routing/mod.rs",
  "trust": "external_untrusted",
  "trust_markers": {
    "text_sanitized": true,
    "control_chars_removed": 0,
    "injection_hits": 0
  },
  "selected_span": {
    "line_start": 42,
    "line_end": 58,
    "selection_kind": "SymbolDefinition",
    "confidence": "Exact"
  },
  "code_context": {
    "language": "rust",
    "imports": ["use tower::ServiceBuilder;", "use tower_layer::Layer;"],
    "enclosing_symbol": "impl<S> Router<S>",
    "enclosing_symbol_kind": "impl"
  },
  "code_span": {
    "span_id": "span_a1b2c3d4e5f6a7b8",
    "language": "rust",
    "line_start": 42,
    "line_end": 58,
    "symbol_name": "Router::layer",
    "symbol_kind": "function",
    "source_id": "src_a1b2c3d4e5f6a7b8",
    "fetch_id": "fetch_f1e2d3c4b5a69780"
  }
}
```

### 2. Debug Exact Error

```jsonc
// Step 1: Search with exact error mode
// -> repo_search({
//     "query": "error[E0308]: mismatched types - expected `String`, found `i32`",
//     "host": "github",
//     "owner": "tokio-rs",
//     "repo": "axum",
//     "mode": "exact_error",
//     "profile": "coding"
//   })
//
// The planner generates error-aware subqueries preserving the exact
// phrase, extracting the error code (E0308), and targeting docs/issues.
// Sensitive tokens (local paths, API keys) are redacted.

// Response includes:
// - groups: ErrorDocs, Issues, Releases, SourceFiles
// - error_context: { parsed_parts: { error_code: "E0308", phrase: "..." }, redactions_applied: [...] }
// - structured_warnings: [ { code: "ExactErrorPhraseMatch", severity: "Info" } ]
// - next_actions: [ { tool: "web_fetch", reason_code: "fetch_official_docs", priority: 1 } ]

// Step 2: Fetch official error docs or relevant issue
// Select the highest-priority suggested fetch from the response.

// Step 3: Bundle evidence for handoff
// -> build_evidence_bundle({ goal: "debug E0308 mismatched types", sources: [...], fetches: [...] })
```

### 3. Security Triage

```jsonc
// Step 1: Security search with applicability
// -> security_search({
//     "query": "axum",
//     "ecosystem": "crates.io",
//     "package": "axum",
//     "version": "0.7.0",
//     "include_kev": true,
//     "include_defensive_guidance": true,
//     "assess_applicability": true,
//     "dependency_files": ["Cargo.lock"]
//   })

// Response includes:
// - vulnerabilities: [ { id, severity, affected_ranges, patched_ranges, ... } ]
// - applicability: [ { status: "not_affected", confidence: "high", advisory_ids: [...] } ]
// - remediation_actions: [ { category: "Upgrade", description: "..." } ]
// - security_evidence_summary: { total_vulnerabilities, highest_severity, has_authoritative_source }
// - groups: AuthoritativeAdvisories, VendorAdvisories, PackageAdvisories, ...
// - structured_warnings: [ { code: "kev_match", ... } ]

// Step 2: Fetch primary advisory
// web_fetch the advisory URL from suggested_fetches[0]

// Step 3: Bundle
// -> build_evidence_bundle({ goal: "security triage for axum 0.7.0", sources: [...], fetches: [...] })
```

**Key fields to inspect:**

- `applicability[].status`: `affected` / `not_affected` / `unknown` / `insufficient_evidence`
- `applicability[].confidence`: `high` / `medium` / `low`
- `vulnerability_metadata.severity`: CVSS severity
- `vulnerability_metadata.kev`: whether in CISA KEV catalog
- `source_quality.has_authoritative_source`: Tier 1 source present

### 4. Architecture / Deep Research

```jsonc
// Step 1: Research with workflow scaffolding
// -> research_search({
//     "query": "axum vs actix-web for high-performance REST API",
//     "research_domain": "software_architecture",
//     "workflow": "library_comparison",
//     "depth": "standard",
//     "compare_targets": ["axum", "actix-web"],
//     "include_counterpoints": true,
//     "include_primary_sources": true,
//     "desired_source_types": ["benchmarks", "official_docs"]
//   })

// Response includes:
// - claims: [ { id, text, claim_type, confidence, supporting_source_ids, conflicting_source_ids } ]
// - conflicts: [ { id, topic, side_a_source_ids, side_b_source_ids } ]
// - evidence_gaps: [ { kind: "no_benchmark_source", message: "...", recommended_actions: [...] } ]
// - source_quality: [ { source_id, source_class, quality_signals, is_stale } ]
// - groups: OfficialDocs, Benchmarks, DesignDiscussions, Counterpoints, ...
// - workflow_context: { dimensions, coverage, gaps }
// - next_actions: [ { tool: "web_fetch", reason_code: "fetch_counterpoint", priority: 1 } ]

// Step 2: Fetch counterpoint or benchmark source
// web_fetch the URL from next_actions[0].input_template.url

// Step 3: Fetch primary docs
// web_fetch the URL from the OfficialDocs group suggested_fetches

// Step 4: Bundle
// -> build_evidence_bundle({ goal: "axum vs actix-web comparison", sources: [...], fetches: [...] })
```

**Key fields to inspect:**

- `claims[].confidence`: `high` / `medium` / `low` / `unknown`
- `conflicts[].topic`: what the disagreement is about
- `evidence_gaps[].kind`: `no_primary_source`, `no_benchmark_source`, etc.
- `source_quality[].quality_signals`: `primary_source`, `reproducible_benchmark`, etc.
- `workflow_context.gaps`: missing coverage dimensions

### 5. Local Workspace Investigation

Prefer local results only when a matching checkout exists, is clean
enough, and is not solely generated/vendor code.

```
Step 1: repo_search with host/owner/repo
  -> local results appear with trust = "local_trusted"
  -> inspect local_repo_match.match_confidence (exact/strong/weak)
  -> inspect is_generated, is_vendor, is_test, is_example flags

Step 2: repo_fetch(prefer_local = true)
  -> resolves to local filesystem when a matching checkout exists
  -> returns trust = "local_trusted"
  -> validates path stays within configured roots

Step 3: If local is dirty or insufficient, fall back to remote fetch
  -> remote results are external_untrusted
  -> combine local + remote evidence

Step 4: Bundle with trust markers from both local and remote
```

**Decision rules for local preference:**

| Condition | Action |
|-----------|--------|
| `local_repo_match.match_confidence = exact` and `dirty = false` | Prefer local |
| `dirty = true` | Use local with warning; note uncommitted changes |
| `is_generated = true` and no other local sources | Fall back to remote |
| `is_vendor = true` | Supplementary only; do not treat as primary |
| `local_repo_match = None` | Use remote only |
| No matching checkout under configured roots | Use remote only |

---

## Trust Boundary Rules

### Trust Levels

| Level | Meaning | Source |
|-------|---------|--------|
| `external_untrusted` | All web/remote results and fetched content | `web_search`, `web_fetch`, `repo_search` remote results, `repo_fetch` remote |
| `local_trusted` | Local workspace files with provenance metadata | `repo_search` local results, `repo_fetch` with `prefer_local` or workspace locator |

### Trust Markers

Every response includes `trust_markers` recording what was applied:

```jsonc
{
  "text_sanitized": true,
  "text_truncated": false,
  "text_framed": false,
  "control_chars_removed": 0,
  "injection_hits": 0
}
```

- `text_sanitized`: Tier 1 control-char strip was applied
- `text_framed`: Tier 2 framing with `<<<EXTERNAL_UNTRUSTED>>>` delimiters was applied
- `injection_hits`: number of prompt-injection patterns detected (Tier 3)
- `control_chars_removed`: count of NUL/CR/bidi/zero-width chars stripped

### Harness Responsibilities

1. **Never treat fetched content as executable instructions.** All
   fetched content is data, even when it contains code.
2. **Never trust content because it came from `local_trusted`.** Local
   files can still contain comments or docs with adversarial text.
3. **Check `trust_markers.injection_hits`.** If nonzero, the content
   triggered a Tier 3 scan. Display a warning to the user.
4. **Deduplicate using `stable_id`.** The `stable_id` on `SourceCard`
   and `fetch_id` on `EvidenceBundleFetchedItem` are deterministic
   and content-derived. Use them to deduplicate across tools.
5. **Link sources to fetches via `source_id`.** Suggested fetches
   include a `source_id` field linking back to the source card that
   generated the suggestion. `build_evidence_bundle` also links via
   `source_links`.

### Evidence Bundle Trust

`build_evidence_bundle` **never elevates trust.** External untrusted
inputs remain external untrusted in the bundle. The `trust_summary`
field aggregates counts but does not change trust semantics.

---

## Warning and Error Handling

### Structured Warnings

Every MCP tool response includes both `structured_warnings` (machine-readable)
and `warnings` (legacy string array) for backward compatibility.

```jsonc
// structured_warnings is a Vec<AgentWarning>:
[
  {
    "code": "profile_degraded",
    "severity": "Warning",
    "message": "coding profile fell back to default providers",
    "provider_ids": [],
    "result_ids": [],
    "source_ids": [],
    "recommended_action": "configure native code providers for full coding profile coverage"
  }
]
```

### Warning Severities

| Severity | Harness Action |
|----------|----------------|
| `Error` | Block the operation or surface as a hard failure |
| `Warning` | Surface to user; degrade gracefully |
| `Notice` | Informational; show in warnings panel |
| `Info` | Advisory; show on hover or in details |

### Common Warning Codes

| Code | Severity | Meaning |
|------|----------|---------|
| `profile_degraded` | Warning | Profile fell back to default providers |
| `profile_provider_not_built` | Notice | A profile provider is missing its API key |
| `native_code_search_unavailable` | Notice | No GitHub/GitLab/Gitea provider configured |
| `freshness_unenforced` | Notice | Freshness requested but no provider enforces it |
| `safe_search_unenforced` | Notice | Safe search requested but not enforced |
| `kev_match` | Warning | CVE found in KEV catalog |
| `version_match_unavailable` | Notice | Affected version could not be determined |
| `applicability_not_exploitability` | Notice | Applicability is metadata-only, not runtime analysis |
| `source_quality_low` | Notice | Only low-tier sources found |

### Error Handling

MCP tools return `Result<serde_json::Value, String>`. Errors are
mapped to MCP error responses. The harness should:

1. Display the error message to the user.
2. Check if partial results are still available (e.g. `repo_search`
   returns groups even when some providers fail).
3. Surface `providers_failed` and `routing_decision` in diagnostics.
4. Never silently swallow errors.

---

## Evidence Bundle Handoff

### When to Bundle

Bundle evidence before handing off to:
- A manager/reviewer agent
- A security review agent
- A documentation agent
- Any agent that needs to inspect gathered evidence without re-searching

### Bundle Workflow

```jsonc
// 1. Gather evidence via search tools
// 2. Fetch selected URLs via fetch tools
// 3. Bundle:
{
  "goal": "understand axum router middleware architecture",
  "sources": [
    {
      "id": "src_a1b2c3d4e5f6a7b8",
      "url": "https://docs.rs/axum/latest/axum/struct.Router.html",
      "title": "Router - axum",
      "providers": ["duckduckgo"],
      "trust": "external_untrusted",
      "metadata": { "source_kind": "official_docs", "domain": "docs.rs" }
    }
  ],
  "fetches": [
    {
      "source_id": "src_a1b2c3d4e5f6a7b8",
      "url": "https://docs.rs/axum/latest/axum/struct.Router.html",
      "text": "pub struct Router<S = ()> { ... }",
      "truncated": false,
      "trust": "external_untrusted"
    }
  ]
}

// Response (abbreviated):
{
  "bundle_id": "bundle_9f8e7d6c5b4a3210",
  "goal": "understand axum router middleware architecture",
  "sources": [
    {
      "source_id": "src_a1b2c3d4e5f6a7b8",
      "url": "https://docs.rs/axum/latest/axum/struct.Router.html",
      "trust": "external_untrusted"
    }
  ],
  "fetched_items": [
    {
      "fetch_id": "fetch_1a2b3c4d5e6f7890",
      "source_id": "src_a1b2c3d4e5f6a7b8",
      "url": "https://docs.rs/axum/latest/axum/struct.Router.html",
      "truncated": false
    }
  ],
  "source_links": [
    {
      "source_id": "src_a1b2c3d4e5f6a7b8",
      "fetch_id": "fetch_1a2b3c4d5e6f7890",
      "link_reason": "url_match"
    }
  ],
  "trust_summary": {
    "external_untrusted_count": 1,
    "local_trusted_count": 0,
    "total_injection_hits": 0
  },
  "provider_summary": {
    "providers_used": ["duckduckgo"],
    "per_provider_counts": [
      { "provider_id": "duckduckgo", "count": 1 }
    ]
  },
  "gaps": [],
  "limits": {
    "max_sources": 50,
    "max_fetched_items": 20,
    "max_total_chars": 100000,
    "sources_truncated": false,
    "fetched_items_truncated": false,
    "total_chars_exceeded": false
  }
}
```

### Bundle Limits

| Field | Default | Cap |
|-------|---------|-----|
| `max_sources` | 50 | 200 |
| `max_fetched_items` | 20 | 100 |
| `max_total_chars` | 100,000 | 500,000 |

When limits are exceeded, the bundle is truncated with a warning. Check
`limits.sources_truncated`, `limits.fetched_items_truncated`, and
`limits.total_chars_exceeded`.

### Receiving Agent Responsibilities

1. Inspect `source_links` to understand source-to-fetch relationships.
2. Check `trust_summary` for aggregate trust levels.
3. Review `gaps` for missing evidence categories.
4. Do not re-fetch already-fetched URLs unless freshness is required.
5. Treat all evidence as untrusted data, not verified facts.

---

## Performance and Response-Size Controls

### Response Size Bounds

| Tool | Field | Default | Cap |
|------|-------|---------|-----|
| `web_search` | `max_results` | 10 | 50 (`max_results_cap`) |
| `web_fetch` | `max_chars` | 12,000 | 50,000 (`max_chars_cap`) |
| `batch_fetch` | `max_chars` (per item) | 12,000 | — |
| `batch_fetch` | `max_total_chars` | 50,000 | 120,000 |
| `batch_fetch` | items count | 8 | 20 |
| `repo_fetch` | `max_chars` | 12,000 | 50,000 |
| `repo_search` | `max_results` | 10 | 50 |
| `repo_search` | `max_per_group` | 4 | 10 |

### Concurrency Controls

| Field | Default | Purpose |
|-------|---------|---------|
| `multiquery_concurrency` | 8 | Global max in-flight subquery jobs |
| `multiquery_provider_concurrency` | 2 | Per-provider max concurrent jobs |
| `batch_concurrency` | 4 | Max concurrent fetches in batch_fetch |

### Timeout Controls

| Tool | Field | Default |
|------|-------|---------|
| Search | `timeout_ms` | 8,000 |
| Fetch | `timeout_ms` | 8,000 |
| Batch | `timeout_ms` | — (uses per-item fetch timeout) |

### Reducing Response Size

- Set `max_chars` on fetch calls to limit extracted text.
- Set `max_results` on search calls to limit result count.
- Use `include_links = false` (default) on fetch to skip link extraction.
- Use `metadata_only` extract mode on `web_fetch` to get metadata without body content.

---

## Agent UI/UX Guidance

### Warnings Panel

Display structured warnings grouped by severity. For each warning,
show:
- Severity badge (Error / Warning / Notice / Info)
- Warning code (e.g. `profile_degraded`)
- Human-readable message
- Affected provider IDs, result IDs, or source IDs (when present)
- Recommended action (when present)

### Evidence Cards

Group source cards by `source_kind` and `trust` level. Display:
- Title and domain
- Trust badge (`external_untrusted` / `local_trusted`)
- Quality confidence (if present): high / medium / low / unknown
- Source kind label (official_docs, source_repository, etc.)
- `code_evidence` source_role and matched_symbol (when present)

### Next Actions

Display `next_actions` as **selectable actions**, not automatic
executions. Each action card should show:
- Target tool name
- Reason code as a label (e.g. "Inspect top source")
- Priority indicator (1 = most productive)
- Source IDs it relates to

### Security Verdicts

For `security_search` applicability results, display verdict chips:
- **Affected** (red) — status = `affected`
- **Not affected** (green) — status = `not_affected`
- **Unknown** (yellow) — status = `unknown`
- **Insufficient evidence** (gray) — status = `insufficient_evidence`

Include confidence level and advisory IDs.

### Research Gaps

Display `evidence_gaps` as a checklist:
- Gap kind as a label (e.g. "No primary source found")
- Recommended action as a follow-up step
- Affected claim IDs for traceability

### Local Flags

For local results, display flags in the evidence panel:
- `is_generated` — generated file indicator
- `is_vendor` — vendor/third-party indicator
- `is_test` — test file indicator
- `is_example` — example file indicator
- `is_config` — configuration file indicator
- `local_repo_match.match_confidence` — exact / strong / weak
- Dirty state badge when `dirty = true`

### Code Span Links

When `code_span` is present on a fetch response:
- Display `span_id` for cross-referencing
- Link to `permalink_url` for source view
- Show `symbol_name` and `symbol_kind` as labels
- Show language badge

### Bundle Export/Import

For evidence bundle handoff:
- Export: serialize `EvidenceBundle` as JSON for subagent input
- Import: deserialize and display `sources`, `fetched_items`,
  `trust_summary`, and `gaps`
- Show bundle ID for traceability
- Link source IDs to their original search results

---

## Failure and Degradation Policy

Each failure case should map to a **user-visible status**, not a
silent failure.

| Failure | Response | Harness Behavior |
|---------|----------|-----------------|
| Provider unavailable | `providers_failed` in response, partial results | Show degraded badge; results from remaining providers are still valid |
| API key missing | `routing_decision.skipped_providers` with `reason_code: "not_built"` | Show "API key not configured" notice; suggest configuration |
| Live mode disabled | `web_search` denies with policy message | Show "Offline mode" status; suggest enabling `[search].mode = "live"` |
| Safe search unenforced | `structured_warnings` with `safe_search_unenforced` | Show notice; results may not be filtered |
| Freshness unenforced | `structured_warnings` with `freshness_unenforced` | Show notice; results may be stale |
| Request deadline exceeded | `telemetry.deadline_exceeded = true`, `subqueries_interrupted`/`subqueries_skipped` | Show timeout warning; partial results may still be valid |
| Fetch truncated | `text_truncated = true` on fetch response | Show "Content truncated" notice; offer to re-fetch with higher `max_chars` |
| PDF unsupported | Error: `pdf_not_enabled` | Show "PDF extraction not enabled"; suggest enabling `pdf_enabled` in config |
| Local workspace mismatch | No `local_repo_match` on local results | Fall back to remote; inform user no local checkout was found |
| Local workspace dirty | `local_repo_match` with dirty state | Show warning; local results may contain uncommitted changes |
| Research evidence gaps | `evidence_gaps` in response | Display gap checklist; offer to fetch missing sources |
| Security applicability unknown | `applicability.status = "unknown"` or `insufficient_evidence` | Show "Cannot determine applicability" status; suggest fetching advisory directly |
| All providers fail | Empty results with error warnings | Show "Search failed" with error details; suggest retrying or checking network |

### Degradation Signals

- `routing_decision.degraded = true`: profile fell back to defaults
- `routing_decision.partial = true`: some profile providers skipped
- `telemetry.deadline_exceeded = true`: request timed out
- `providers_failed`: list of provider IDs that failed entirely

Never treat partial results as total failure. If groups or source cards
are present, they are valid evidence from the providers that succeeded.

---

## Versioning and Compatibility

### Breaking Changes (require major version bump)

These changes break existing harnesses:

- **Removing fields** from any response type
- **Renaming serialized enum variants** (e.g. changing `"source_file"` to
  `"source_file_v2"` in `SourceKind`)
- **Changing ID algorithms** without a version bump (e.g. changing the
  FNV-1a hash of `SourceKey`)
- **Changing default trust semantics** (e.g. marking local results as
  `external_untrusted` by default)
- **Changing warning severities** for existing `WarningCode` values
- **Changing recipe IDs** or removing recipes
- **Changing reason codes** on `AgentNextAction` or `FetchRankReason`
- **Changing `provider_status` default detail behavior** (e.g. requiring
  `recipe_detail` where it was previously optional)

### Additive-Compatible Changes

These changes do not break existing harnesses:

- **Adding optional fields** to response types (serde `skip_serializing_if`)
- **Adding new `WarningCode` variants** (harnesses should handle unknown codes gracefully)
- **Adding new `FetchRankReason` variants** (existing fallback behavior preserved)
- **Adding new recipes** (harnesses ignore unknown recipe IDs)
- **Adding new `source_kind` values** (harnesses should have a default handling for unknown kinds)
- **Adding new `AgentNextAction.reason_code` values** (harnesses treat unknown reasons as informational)
- **Adding new `SecurityRemediationCategory` or `SecuritySourceClass` variants**

### Harness Best Practices

1. **Treat unknown enum variants as `"unknown"` or skip them.** Never
   hard-fail on an unrecognized string value.
2. **Treat missing optional fields as absent.** All optional fields
   use `skip_serializing_if = "Option::is_none"`.
3. **Use `stable_id` for cross-tool deduplication, not `id`.** The
   `id` is a random UUID per response. The `stable_id` is deterministic
   and content-derived.
4. **Check `WarningCode` values programmatically when possible.** The
   58 stable snake_case variants are the contract. The `message` field
   is human-readable and may change.
5. **Do not depend on field ordering.** JSON fields are unordered.
6. **Handle partial responses gracefully.** A search may return some
   groups empty and others populated.

---

## Readiness Checklist

Before shipping codegg integration, verify:

- [ ] **Documentation complete**: the deep docs set exists and covers
  configuration, safety, tool routing, recipes, and the response contract
- [ ] **Examples are schema-valid or clearly abbreviated**: every JSON
  example either matches the current MCP schema or is marked
  `(abbreviated)` / `(truncated)`
- [ ] **CI covers offline verification**: `cargo test --features mock`
  passes with all integration and corpus tests
- [ ] **Local workspace policy is explicit**: decision rules for local
  preference are documented (see "Local Workspace Investigation")
- [ ] **Security output is defensive-only**: `security_search` output
  provides evidence and guidance, not exploit instructions
- [ ] **Recipes/next-actions usable without hardcoded prompt prose**:
  codegg can discover recipes via `provider_status` and follow
  `next_actions` from search responses
- [ ] **Evidence bundles preserve enough state for subagents**:
  `build_evidence_bundle` output includes `source_links`,
  `trust_summary`, and `gaps` sufficient for a receiving agent to
  continue work without re-searching
- [ ] **Generic non-codegg MCP clients remain supported**: all tools,
  types, and responses are generic. No codegg-specific fields or
  behavior exist in the server
- [ ] **Trust semantics documented**: `external_untrusted` vs
  `local_trusted` are defined and the harness handles both
- [ ] **Warning codes documented**: the 58 stable `WarningCode` variants
  are the contract for programmatic handling
- [ ] **Configuration examples provided**: minimal, generic, coding,
  security, research, and API-provider configs are documented
- [ ] **Failure modes mapped**: every known failure case has a
  user-visible status, not a silent failure

---

## Appendix: Stable ID Format Reference

| Entity | Prefix | Example |
|--------|--------|---------|
| Source card | `src_<16hex>` | `src_f1e2d3c4b5a69780` |
| Fetch result | `fetch_<16hex>` | `fetch_1a2b3c4d5e6f7890` |
| Suggested fetch | `suggested_<16hex>` | `suggested_9f8e7d6c5b4a3210` |
| Batch item | `batch_<16hex>` | `batch_a1b2c3d4e5f6a7b8` |
| Repo locator | `loc_<16hex>` | `loc_f1e2d3c4b5a69780` |
| Document | `doc_<16hex>` | `doc_1a2b3c4d5e6f7890` |
| Document chunk | `chunk_<16hex>` | `chunk_9f8e7d6c5b4a3210` |
| Code span | `span_<16hex>` | `span_a1b2c3d4e5f6a7b8` |
| Evidence bundle | `bundle_<16hex>` | `bundle_9f8e7d6c5b4a3210` |

IDs are FNV-1a 64-bit hashes with a versioned input prefix
(`eggsearch-id-v1\0`). URL canonicalization normalizes scheme, strips
`www.`, removes default ports, strips fragments, and normalizes
percent-encoding before hashing.

---

## Appendix: Tool Quick Reference

| Tool | Primary Use | Trust |
|------|-------------|-------|
| `web_search` | General metasearch with intent/freshness hints | external_untrusted |
| `web_fetch` | Bounded extraction of one HTTP(S) URL | external_untrusted |
| `batch_fetch` | Bounded batch fetch over explicit URLs/locators | external_untrusted |
| `provider_status` | Capability discovery, recipe catalog, health | local_trusted |
| `repo_search` | Structured repo evidence with grouped bundles | external_untrusted + local_trusted |
| `repo_fetch` | Structured repo file fetch by locator | external_untrusted + local_trusted |
| `repo_map` | Repository structure discovery | external_untrusted + local_trusted |
| `security_search` | Security retrieval with normalized advisory metadata | external_untrusted |
| `research_search` | Research evidence with claims/conflicts/gaps | external_untrusted |
| `build_evidence_bundle` | Package evidence for multi-agent handoff | preserves input trust |

---

## Appendix: Recipe Catalog

| Recipe ID | Purpose | Key Tools |
|-----------|---------|-----------|
| `generic_web_lookup` | General web search and fetch | `web_search`, `web_fetch` |
| `documentation_api_lookup` | Authoritative docs and API references | `web_search(intent="docs")`, `web_fetch` |
| `repository_investigation` | Code, issues, releases in a repo | `repo_map`, `repo_search(profile="coding")`, `repo_fetch`, `batch_fetch` |
| `exact_error_investigation` | Debug compiler/runtime errors | `repo_search(mode="exact_error")`, `web_fetch` |
| `security_package_triage` | Vulnerability lookup and applicability | `security_search`, `web_fetch` |
| `dependency_upgrade_research` | Changelogs, migration guides, breaking changes | `repo_search`, `research_search` |
| `architecture_deep_research` | Multi-source comparison and decisions | `research_search(workflow=...)`, `web_fetch` |
| `local_workspace_investigation` | Local source file investigation | `repo_search(include_local=true)`, `repo_fetch(prefer_local=true)` |

Each recipe's `support` status (`available`, `partial`, `unavailable`)
is evaluated against the current provider configuration at runtime.
Call `provider_status(recipe_detail = "summary")` to check.
