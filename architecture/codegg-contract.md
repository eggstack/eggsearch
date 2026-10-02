# eggsearch MCP Response Handling Contract

**Audience:** Coding-agent harness developers (codegg and similar).
**Status:** Stable, versioned contract.
**Scope:** MCP tool responses from the ten stable tools in
`src/mcp/tool_contract.rs` (`ALL_CONTRACTS`).
**Sources:** `src/mcp/tool_contract.rs`, `src/mcp/projection.rs`,
`src/mcp/output_schema.rs`, `src/mcp/server.rs`
(`apply_contract_metadata`, `contract_fingerprint`),
`src/mcp/tools/common.rs` (`map_tool_result`, `ToolError`,
`RepairHint`), `tests/docs_tool_names.rs`, `tests/evidence_contract.rs`.

This document defines the machine-readable contract harnesses must
implement to consume, deduplicate, triage, and route eggsearch MCP
output. All types, codes, and semantics here are **stable** — the
stable contract is the ten MCP tools plus the CLI, and breaking
changes follow additive-only evolution (see §8).

---

## 1. Stable Tool Surface

Exactly ten tools, canonical names in deterministic alphabetical
order (`tool_names()`). `tests/docs_tool_names.rs` fails the build on
any other count and on phantom tool-like names in docs.

| Tool | Domain | Disclosure | Purpose |
|------|--------|-----------|---------|
| `batch_fetch` | fetch | deferred | bounded multi-target fetch over explicit targets |
| `build_evidence_bundle` | evidence | deferred | deterministic evidence packaging for handoff |
| `provider_status` | diagnostic | diagnostic | diagnostic provider and capability report |
| `repo_fetch` | repository | deferred | bounded repository file or span inspection |
| `repo_map` | repository | deferred | repository structure discovery without contents |
| `repo_search` | repository | core | structured repository evidence discovery |
| `research_search` | research | deferred | multi-source research evidence discovery |
| `security_search` | security | deferred | vulnerability and advisory evidence discovery |
| `web_fetch` | web | core | bounded single-URL inspection |
| `web_search` | web | core | general web source discovery |

Each `ToolContract` carries `name`, `description` (selection-oriented,
budgeted), `purpose`, `use_when`, `not_for`, `domain`
(`ToolDomain::as_str`: `web`, `fetch`, `repository`, `security`,
`research`, `evidence`, `diagnostic`), `disclosure`
(`ToolDisclosureHint::as_str`: `core`, `deferred`, `diagnostic` —
advisory for hosts, never policy-enforcing), `read_only` (true for
all ten), `open_world` (true except `build_evidence_bundle` and
`provider_status`), `related_tools`, `next_tools`, `keywords`, and
`aliases` (discovery-only, never wire names). `annotations()` derives
static MCP hints from `read_only`/`open_world`; annotations never vary
by arguments — `provider_status` reports `open_world_hint = false`
even though `probe: true` performs bounded live liveness checks.
`discovery_text()` concatenates domain, disclosure, purpose,
use-when/not-for, keywords, aliases, and the related/next graphs for
host catalog indexing without returning full input schemas.

`is_known_tool(name)` gates the ten canonical names. Harnesses must
ignore `next_actions` entries whose `tool` fails this check, and
`sanitize_next_actions()` drops unknown tools and empty reason codes,
clamps priority to 1–5, and truncates to `MAX_NEXT_ACTIONS`
preserving order. Unknown or malicious names never widen execution
authority.

---

## 2. Deterministic Identity System

All ids are content-derived FNV-1a hex (mostly `src/core/identity.rs`;
`bundle_` in `src/core/evidence_bundle.rs`, `conflict_` in
`src/core/conflict.rs`, `fp_` in `src/core/retrieval_status.rs`).
Never random UUIDs; never change id semantics (breaks corpus
regression and cross-tool dedup).

| Entity | Prefix | Key fields |
|--------|--------|------------|
| Source card | `src_` | provider + url + title + source kind |
| Suggested fetch | `suggested_` | url + group + priority |
| Fetch result | `fetch_` | url (or locator) + line range + text prefix |
| Code span | `span_` | locator + line_start + line_end + symbol |
| Batch fetch item | `batch_` | label + index |
| Evidence bundle | `bundle_` | goal + source ids + fetch ids |
| Locator | `loc_` | host + owner + repo + ref + path |
| Document | `doc_` | url + title + kind |
| Document chunk | `chunk_` | chunk-scoped derivation |
| Conflict | `conflict_` | source ids + field |
| Query fingerprint | `fp_` | non-recoverable query hash |

Linking rules harnesses must verify when chaining tools:
`SourceCard.stable_id` ↔ `EvidenceBundleSource.source_id` ↔
`SuggestedFetch.source_id`; fetch linkage resolves by explicit
`source_id`, then URL, then structured locator
(`SourceIdMatch` → `UrlMatch` → `LocatorMatch`). Only bundle source
ids previously received from search responses; the bundle builder
preserves whatever identity it is given, so linkage hygiene is the
caller's job.

---

## 3. Trust Markers

Every source and fetched item carries `trust` plus `trust_markers`
with five fields: `text_sanitized`, `text_truncated`, `text_framed`,
`control_chars_removed`, `injection_hits`. `TrustLevel` is
`external_untrusted` (default — all web/remote content is data, never
instructions), `local_trusted` (workspace provenance-trusted, still
not instruction-trusted — comments can be adversarial), or `unknown`
(treat as `external_untrusted`). Tier 1 sanitization (control-char
stripping + length bounding) is always on; `[search].sanitize_output`
and `[fetch].sanitize_output` (both default `true`) gate Tier 2
framing and Tier 3 marker scans. When `injection_hits > 0`, content
is framed with `<<<EXTERNAL_UNTRUSTED>>>` delimiters and must still
be treated with caution — flag for review, require a full fetch
before final use of snippet-only sources.

---

## 4. Structured Warnings

Every tool response includes `warnings: Vec<String>` (legacy,
human-readable) and `structured_warnings: Vec<AgentWarning>`
(machine-readable, deduplicated). Harnesses **must** read
`structured_warnings` and never parse `warnings` for programmatic
decisions. `AgentWarning` carries stable `code` (`WarningCode`),
`severity`, `message`, scoping arrays (`provider_ids`, `result_ids`,
`source_ids` — empty means global to the response), and
`recommended_action`. Warning codes include trust signals
(`untrusted_external_content`,
`untrusted_local_workspace_content`,
`prompt_injection_marker_detected`), capability enforcement
(`safe_search_unenforced`, `freshness_unenforced`), native-provider
availability (`native_code_search_unavailable`,
`native_issue_search_unavailable`,
`native_release_search_unavailable`,
`native_advisory_search_unavailable`,
`symbol_hint_no_native_provider`,
`repo_hints_not_enforced_natively`,
`issue_search_no_native_provider`,
`release_search_no_native_provider`), provider status
(`unknown_provider`, `disabled_provider`, `missing_api_key`,
`provider_failed`, `provider_timeout`, `provider_rate_limited`,
`provider_cooldown`), and routing (`profile_degraded`,
`profile_partial`, `profile_provider_not_built`,
`profile_provider_unknown`, `profile_provider_unavailable`); the full
set lives in `src/core/warning.rs` and only grows additively.

---

## 5. `map_tool_result` Seam Codes

`map_tool_result()` (`src/mcp/tools/common.rs`) is the single
MCP error/result conversion seam:

- `Ok(value)` → native structured success (`structuredContent` plus
  a text JSON fallback for older clients).
- `ToolError::InvalidRequest` → JSON-RPC `invalid_params`. Only
  uninterpretable invocation shapes go here.
- `ToolError::Validation` (legacy) and `ToolError::Execution` →
  recoverable `isError: true` results with a stable `code` and
  bounded `repair { field, accepted, suggested_value }`.
- `ToolError::Internal` → JSON-RPC `internal_error`, never leaking
  stack traces.

Stable semantic `ToolErrorCode` strings (`as_str()`):
`invalid_semantic_value`, `conflicting_arguments`,
`capability_unavailable`, `provider_unavailable`, `policy_denied`,
`budget_invalid`, `locator_invalid`, `manual_interaction_required`,
`upstream_failed` — plus the protocol-level `invalid_request` and
`internal`, which are transport codes, not repairable semantics.
`RepairHint` is bounded: `accepted` capped at 20 entries, field
strings at 128 chars, `suggested_value` at 256 chars. Distinguish
tool-level `isError` from transport failure; match on `code` to
decide whether a repaired retry is worthwhile.

---

## 6. `ResponseDetail` Modes

All search/fetch tools accept `response_detail`
(`compact`/`standard`/`diagnostic`, default `diagnostic` via
`from_opt()`); `parse_raw()` accepts all modes case-insensitively.
Canonical responses are captured before projection; `project()`
trims only model-visible JSON at the MCP boundary.

- `compact` preserves query identity, cards/groups, stable ids,
  locators, trust + injection markers, evidence roles, 1 excerpt per
  card, essential warnings, explicit failure/absence state (minimal
  `retrieval_status`: `has_failures`, `has_absences`,
  `has_truncation`, attempted/completed/failed job counts),
  `conflict_indicator` (`has_conflicts`, `conflict_count`),
  `routing_summary` (selected/skipped counts, `degraded`, `partial`),
  `capability_summary` (enforced/approximated/not-enforced counts
  where applicable), and `next_actions`/`suggested_fetches`. Full
  routing, telemetry, document/link detail, and per-dimension
  retrieval detail are omitted. Output is stamped `response_detail`
  plus `projection_excerpts_trimmed`.
- `standard` keeps the full `retrieval_summary`,
  `conflict_metadata`, `workflow_coverage`, capability summaries,
  and fetch/cache metadata, adding `routing_summary` and the
  `response_detail` stamp.
- `diagnostic` is the full passthrough payload.

`build_evidence_bundle` accepts `response_detail` but returns
identical canonical content in all modes; `provider_status` likewise
passes through. Harnesses must store full `structuredContent`
internally and inject only the selected projection into
model-visible context. Compact output must never be read as evidence
absence when `retrieval_status.has_failures` or non-empty
`providers_failed` is present.

---

## 7. `tools/list` Fingerprint

`apply_contract_metadata()` overwrites each tool's description and
annotations from the canonical registry, attaches the permissive
output schema, and sorts by name so `tools/list` is deterministic
and cacheable across transports and protocol eras.
`contract_fingerprint()` hashes canonical names, descriptions,
annotations, serialized input/output schemas, and discovery metadata
(domain, disclosure, purpose, use-when/not-for, keywords, aliases,
related/next) with FNV-1a; `tool_fingerprint()` exposes it.
Harnesses must cache `tools/list` by this fingerprint plus the
server version — never by tool count — and apply hydration state
after retrieving a cached base surface.

---

## 8. Additive-Only Evolution and Schema Budgets

Breaking changes (require a major version bump): removing or
renaming an enum variant, struct field, `WarningCode`, or
`FetchRankReason` variant; changing a serialized enum string or a
deterministic id for the same input; changing a recipe id or step
tool reference. Non-breaking additions: appended enum variants, new
optional (`skip_serializing_if`) fields, new warning/reason codes,
new tool capabilities, new `server_capabilities` flags. Harnesses
must treat unknown enum variants and optional fields as opaque —
skip them, never crash.

Budgets enforced by tests: `MAX_TOOL_DESCRIPTION_LEN = 300` bytes
per description (`descriptions_respect_size_budget`); every output
schema is a permissive object (`additionalProperties: true`) of at
most 1200 bytes (`output_schemas_stay_compact`); all ten tools have
an output schema (`all_stable_tools_have_output_schemas`). Typed
envelopes generate schemas where they exist; open-ended envelopes
(`web_search`, `web_fetch`, `provider_status`) use stable permissive
schemas by design.

---

## 9. Retrieval State Interpretation

Harnesses must interpret `retrieval_summary` through the
authoritative `state` field (`RetrievalDimensionState`: `satisfied`,
`completed_no_match`, `failed`, `skipped_by_policy`,
`capability_unavailable`, `interrupted`, `partial`,
`not_applicable`), `attempt_outcome` for exact operation outcomes
(10 `RetrievalAttemptOutcome` variants), and
`truncation_evidence` for completeness — never through
`absence_kind` alone. Priority order for the same role: `satisfied`
→ `partial` → `failed` → `interrupted` → `completed_no_match` →
`skipped_by_policy` → `capability_unavailable` →
`not_applicable`. State-aware helpers (`is_absence_only()`,
`is_failure_only()`, `has_indeterminate()`, `absent_roles()`,
`failed_providers()`) encode these rules; required roles with
capability or policy skips stay indeterminate in workflow coverage.
`not_applicable` counts exist at both levels
(`not_applicable_count`, `not_applicable_job_count`) and fold into
completed counts. `limit_reached_unknown` is possible, unconfirmed
truncation — never treat it as confirmed.

---

## 10. Keyless-Core Invariant

A clean install with no config file and no credential env vars
starts successfully and serves keyless search/fetch. Harnesses must
not prompt for API keys, gate tool calls on credentials, or treat
missing credentials as global failure; inspect `provider_status`
only when availability itself is relevant. Prefer native adapters
when routable; continue with keyless providers otherwise; never
label generic web results as native forge evidence. Credential
suggestions are contextual, optional, and paired with a keyless
fallback.

### 10.1 Do Not Require Credentialed Providers

Baseline search, fetch, security, and research operations must work
without API keys. Harnesses must not prompt for keys to perform
baseline search, require credentials before attempting a tool call,
or treat missing credentials as a global server failure.

### 10.2 Do Not Prompt for Keys on Baseline Operations

Credential-related prompts are appropriate only when the user
explicitly requests a capability that requires native adapter
access (for example private repository access). When a workflow
would benefit from native adapter access, suggest the optional
credential contextually, label it an enhancement rather than a
requirement, and offer the keyless fallback.

---

## 11. Local Workspace Metadata

Local workspace results carry `workspace_id`, checkout state, and
`dirty_state`; trust is `local_trusted` for provenance only, never
instruction-trusted.

### 11.1 Dirty State

| State | Meaning | Harness Action |
|-------|---------|----------------|
| `clean` | No uncommitted changes | Proceed normally |
| `dirty` | Uncommitted changes exist | Warn user; content may be stale relative to HEAD |
| `unknown` | Could not determine dirty state | Treat as dirty (conservative) |
| `not_git` | Not a git repository | Ignore dirty state |

Check `dirty_state` before treating a checkout as committed state.

### 11.2 File Classification Flags

Local source cards carry flat classification flags on result
metadata (`is_generated`, `is_vendor`, `is_test`, `is_example`,
`is_config`, `is_lockfile`): filter by file type, distrust
generated or vendored content that may be stale, and apply
language-specific tooling where appropriate.

### 11.3 Workspace ID

Local workspace results include a `workspace_id` string
(`ws_` plus 16 lowercase hex chars) derived deterministically
from the canonical workspace root, remote URLs, and HEAD commit.
Use it to track checkout state across tool invocations,
deduplicate results from the same workspace, and correlate
`repo_search`, `repo_fetch`, and `repo_map` calls. It changes only
when the root, remotes, or commit change.

---

## 12. Downstream Retirement Contract

**Qualifying version: `eggsearch` >= 0.4.0.** The provider-parity surface
described here and in `docs/codegg-integration.md` first ships in 0.4.0. Pin
the minor line, not a moving branch, and confirm with `eggsearch --version`
(prints `eggsearch 0.4.0`).

This is the upstream half of a two-part handoff. eggsearch does not edit the
consumer; it fixes the contract the consumer's separate retirement workstream
may rely on. A harness must not begin deleting its in-tree search backend until
`provider_status` reports the required ids as `routable` on the pinned version.

### 12.1 Provider migration and disposition matrix

Frozen and derived from implemented ids. The full table with per-provider
constraint notes lives in
`docs/codegg-integration.md` § Legacy External-Search Provider Migration; the
disposition classes are what this contract fixes:

| Class | Legacy hint -> eggsearch id | Meaning for the harness |
|-------|-----------------------------|--------------------------|
| `routing` | `auto` -> *(omit `providers`)* | Not a provider. Automatic routing is expressed by sending no `providers` field. |
| `exact` | `duckduckgo`, `mojeek`, `openalex`, `brave`, `brave_api`, `exa`, `tavily`, `wikipedia`, `arxiv`, `pubmed`, `hn_algolia`, `serpapi` | Same upstream source, same id. Interchangeable results. |
| `renamed` | `github` (repository discovery) -> `github_repositories` | Same upstream source; renamed to avoid collision with `github_code` / `github_issues` / `github_releases`. |
| `equivalent` | `kagi` -> `kagi` | Same provider, but the **current** v1 contract (`POST /api/v1/search`, `Authorization: Bearer`, `workflow: "search"`, `data.search` only) replaces the deprecated v0 endpoint. Constraint coverage differs: no language field, no news workflow. Do not assume v0 field parity. |
| `retired` | `google_news` (RSS) -> none | No provider stands behind it. Delete the hint. The news *capability* is preserved as `capability` via `intent: "news"`, optionally with an explicit provider from `brave_api` / `tavily`, the only ids advertising native news support. |

No row claims an unrelated general provider is an exact replacement for a
removed source. `google_news` is the only non-exact retirement and it has no
stand-in provider id.

### 12.2 Explicit-provider failure semantics

A non-empty `providers` array is a hard contract. eggsearch never converts an
explicit request into automatic routing, and never narrows it to the routable
remainder. Each requested id resolves to exactly one typed outcome, surfaced as
a tool-level `isError` with stable code `provider_unavailable`:

| Condition | `ProviderSkipCode` | Harness action |
|-----------|--------------------|----------------|
| Unknown id | `unknown_provider` | Fix the caller. Do not substitute a provider. |
| Known, not enabled or not configured | `disabled_by_user` | Enable it in config. |
| Known and enabled, credential unset or empty | `missing_api_key` | Populate the environment variable **named in the error message**. |
| Known, enabled, configured, no engine in this build | `not_built` | Report as unsupported; do not substitute. |

The missing-credential case names the exact environment variable, so the
`ProviderSkipCode` vocabulary and the message text agree. When all explicitly
selected providers are queried and all fail, the error carries `upstream_failed`
and preserves each provider's reason; `internal_error` is reserved for genuine
server faults. Advisory health and cooldown never drop an explicitly requested
provider, and a capability a provider cannot serve is recorded as a
capability-skip attempt in the retrieval ledger rather than a silent omission.

### 12.3 Stable ten-tool statement

This contract is unchanged by provider parity. The surface remains exactly ten
tools (§1) plus the CLI. Migration adds provider ids and capability
descriptors; it adds no tool, no request field, and no response field, and it
does not relax the additive-only policy in §8. `docs/tool-matrix.md` remains
the schema budget, and the `tools/list` fingerprint in §7 remains the harness's
cache key.

CodeGG native wrappers remain the normal model-facing API. `providers` and
`timeout_ms` are accepted but hidden from the advertised schema: a harness maps
its own intents onto the canonical `goal` / `profile` / `workflow` / `sources`
fields, and uses `providers` only where a human explicitly asked for a named
source.

### 12.4 Retirement prerequisites

A consumer may, on the pinned >= 0.4.0 version and after verifying
`provider_status`:

- delete its own external-search HTTP clients, including the deprecated Kagi v0
  client and the Google News RSS parser;
- remove `backend="builtin"` and `fallback_to_builtin` from its search call
  sites, so failures surface loudly with a typed reason instead of degrading to
  a second implementation;
- delete the `google_news` hint and its branches;
- keep its native wrappers as the model-facing API.

No consumer-specific field may be added to the MCP request or response schema
to support retirement. If that appears necessary, it is a defect in this
contract rather than a feature gap.

---

**Back to:** [overview.md](overview.md)
