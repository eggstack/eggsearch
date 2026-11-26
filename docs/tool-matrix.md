# eggsearch Tool Matrix

Compact reference for the nine canonical MCP tools.

| Tool | Purpose | Key Inputs | Output | Trust | When to Use |
|------|---------|------------|--------|-------|-------------|
| `web_search` | Live metasearch over configured providers | `query`, optional `intent`, `freshness`, `date_range` (`YYYY-MM-DD` start/end, mutually exclusive with `freshness`), `include_domains`/`exclude_domains` (lowercase hostnames, max 32; natively enforced by `exa`/`tavily`, otherwise local enforcement), `language`/`region` (best-effort unless enforced; native on Brave API and Tavily when representable), `excerpt_count` (0-3, default 0), `max_results`. Advanced `providers`/`timeout_ms` remain accepted but are hidden from the ordinary schema | `Vec<SourceCard>` plus `next_actions` and `capability_enforcement` telemetry (`requested`/`enforced`/`approximated`/`not_enforced`). When `intent` is omitted the response also carries `derived_intent` and `derived_intent_reasons` (deterministic lexical derivation; an explicit `intent` is authoritative and suppresses both; compact detail keeps the intent string only). Cards carry `excerpts` (bounded source passages, only when requested) and `metadata.published_at` (RFC 3339, when a provider returned parseable timestamp evidence). Source cards only; the tool never returns a synthesized answer | `external_untrusted` | General web research and source discovery. Source cards include `evidence_role`; `conflict_metadata` present when sources disagree. |
| `fetch` | Bounded fetch of explicitly selected web/repo targets or bounded site URL inventory discovery | `items`: one tagged `FetchItem` or a non-empty array; every item has `type: "web"`, `type: "repo"`, or `type: "site"`. A web item carries its URL and controls: `extract_mode`, `max_chars`, document `offset`/`limit`, `include_links`, `pdf`, `render`, `browser_profile`, `include_screenshot`, `cache_policy`/`max_cache_age_seconds`, `focus`, `focus_max_chunks`, and `focus_max_chars`, and an opt-in `snapshot` comparison (`compare`, `key`, `max_hunks`, `max_diff_chars`). A repo item carries its structured locator/ref and line/context/symbol/match-text/block-expansion/`prefer_local`/focus controls. A site item carries its entry URL, optional URL limit, subdomain policy, one-page link-discovery switch, and query-normalization policy; it returns a `site_map` inventory and never fetches discovered URLs. Request-level: `max_items`, `max_chars_per_item`, `max_total_chars`, `timeout_ms`, `continue_on_error`, collection `offset`/`limit`, `response_detail`. | Always a batch-style response with ordered `results[]`; each item carries its target type, optional label/stable ID, success state, response or error, returned-character count, and truncation state. A site result carries `response.site_map` with sorted URLs, source/lastmod metadata, per-source and skip/duplicate counts, index/robots metadata, and `truncated`; discovery never fetches those URLs. Aggregate counts, truncation, total characters, warnings, telemetry, and collection-addressing metadata are retained. A container document (CSV/EPUB/DOCX/XLSX/PPTX, `documents` feature) carries `document.format`, ordered one-indexed `document.parts`, and `document.parts_total`, with response-level `text` still bounded. A keyed comparison adds `change` with `status` (`new`/`same`/`changed`/`removed`), previous/current fetch timestamps, character counts, and bounded hunks. Screenshot image blocks stay out of structured JSON. Each item reports the readable measure (`chars_returned`) and projected payload (`payload_chars`); the aggregate budget uses that projected shape, degrades returned document structure (`document.blocks`/`document.chunks`, `document`, `lines`) and web metadata before clipping text, marks budget drops in `degraded_fields`, and omits an item only when it still cannot fit. | `external_untrusted` or `local_trusted` | Use web/repo items for explicit targets, a `site` item for a bounded URL inventory, or mix item types; use `repo` for structured repository targets |
| `provider_status` | Diagnostic report of provider config, health, capabilities, and workflow recipes | none required; optional `recipe_detail` (`none`, `summary`, `full`), `probe` (bool, bounded live liveness check) | Provider list with `routable`, `skip_reason`, and `skip_code` fields, `health_views` (per-provider health, including `parse_shape` = `ok`/`empty`/`shifted` for observed providers), `code_hosts`, `health` (snapshots), `probe` (`requested`/`implemented`/`started`/`succeeded`/`failed`/`skipped`/`outcomes`), `server_capabilities` (including the PDF and container-document flags), `pdf_capabilities`, `documents_capabilities`, `tool_capabilities`, `quality_metadata`, `workflow_recipes`, and a diagnostic-only `search_cache { entries, hits, misses, coalesced, bypassed, evictions }` block when the process-local search cache is enabled | `local_trusted` | Diagnostics and troubleshooting only; not a normal research step. Hosts may inspect during bootstrap; agents call only when provider availability itself is relevant |
| `repo_search` | Structured repository evidence discovery with grouped bundles | `query`, `goal` (understand, architecture, debug, migration, security, dependency, performance, compare, pre_change, post_change), `sources` (code, docs, registry, issues, pull_requests, releases, examples, changelog, migration_guides, security; omit for goal defaults), optional repo locator fields, package fields, `include_local`. Legacy `profile`/`mode`/`workflow`/`include_*`/`providers`/`timeout_ms` remain accepted but are hidden from the ordinary schema | `RepoSearchResponse` with grouped `SourceCard` bundles and `next_actions` | `external_untrusted` or `local_trusted` | Find code, issues, releases, docs, and repo metadata. With search mode off, an enabled local workspace is searched under explicit no-remote-dispatch policy; absent or disabled local search is rejected. Source cards include `evidence_role`; `conflict_metadata` present when sources disagree. Local matches carry `symbol_provenance` (`structured`/`regex_fallback`), `enclosing_symbol`, and `is_exact_definition`; structured definitions score above lexical matches with `Exact` confidence. Local results require a query, path/file, or symbol match; a remote-scoped request includes them only with explicit `include_local: true`. A content or symbol query continues beyond the path-ranked first window inside a bounded scan (512 files, 8 MiB, 1.5 s, request deadline); `local_search_truncated` means a bound cut that scan short, while `local_search_completed_no_matches` means it completed. |
| `repo_map` | Bounded repository-structure discovery | `host` (optional; defaults to github under live dispatch), `owner`, `repo`, optional `ref_name`, `max_entries`, `max_depth` | `RepoMapResponse` with important files and directories plus additive `packages`, `language_distribution`, `modules`, `entrypoints`, `top_symbols`, `test_relationships`, `build_configs`, `structure_truncated` | `external_untrusted` or `local_trusted` | Understand repo layout before detailed search |
| `security_search` | Security-oriented retrieval with normalized vulnerability metadata | `query`, optional `ecosystem`, `package`, `version`, `cve_id`, `ghsa_id`, `severity_min`, `assess_applicability`, `goal` (usually omit; defaults to security_review), `include` (kev, exploit_context, defensive_guidance, vendor_advisories). Legacy `workflow`/`include_*`/`providers`/`timeout_ms` remain accepted but hidden | `SecuritySearchResponse` with advisories, applicability, and `next_actions` | `external_untrusted` | Vulnerability lookup and package security triage. Identifier queries include enabled native advisory providers by default; native vulnerabilities carry an optional bounded `summary`. Source cards include `evidence_role`; `conflict_metadata` present when sources disagree. |
| `knowledge_brief` | Cutoff-aware candidate developments | `topic` (passed through unchanged), optional `model_id`, `cutoff_override` (`date`, `precision`, `source`, `reason`; must not be labeled provider-official), `max_items` (default and cap 8), `max_chars` (default 6000), `refresh` (`live`, `prefer_live`, or `index`; omitted means live and writes nothing), `retain`, `related_topics`, `language` | Ranked `deltas` with `kdelta_` ids, bounded excerpts, date basis, precision, temporal relationship, `model_resolution`, coverage, and `next_actions` naming only `fetch` and `research_search`. A temporal relationship describes dated evidence against a published cutoff. It does not claim that a model lacks a fact. Coverage accounts for selected cards with `selected_cards`, `dated`, `undated`, `at_or_before_cutoff`, and `not_after_cutoff`; it lists selected providers and those advertising `result_timestamps`, and gives every empty result an `empty_state_reason`. `coverage.index_status` is `not_opened`, `disabled`, `open`, `degraded`, or `offline`, and an `index` refresh that finds nothing distinguishes `index_not_seeded`, `index_stale`, and `no_matching_index_hits`. `model_resolution.catalog_as_of` reports the newest verification date in the bundled cutoff catalog. Each non-emitting reason also appears as a `knowledge_brief_<reason>: ...` warning. Dates come only from provider-declared publication timestamps, never prose, snippets, or fetched HTML. | `external_untrusted` | Use when a topic needs dated public evidence compared with a model cutoff. Full and Research presets advertise it; Code and Minimal do not. A hidden tool stays callable. |
| `research_search` | Research-oriented multi-source evidence discovery | `query`, optional `research_domain` (alias `domain`), `desired_source_types` (aliases `sources`, `source_types`), `goal`, `depth`, `offset`/`limit` (half-open window over non-empty result groups), `compare_targets`, `include` (counterpoints, primary_sources, recent_discussion, security_considerations). Legacy `workflow`/`include_*`/`providers`/`timeout_ms` remain accepted but hidden | `ResearchSearchResponse` with grouped evidence, claims, gaps, and `next_actions` | `external_untrusted` | Architectural comparison and multi-source evidence gathering. Source cards include `evidence_role`; `conflict_metadata` present when sources disagree. |
| `build_evidence_bundle` | Package selected evidence into a deterministic container | `goal`, `sources`, `fetches`, and the bounded `max_*` limits. Pass each `sources`/`fetches` entry verbatim from a prior tool response: nested `metadata`, `locator`, `selected_span` and `trust_markers` are accepted but deliberately not re-advertised, so the advertised schema stays bounded | `EvidenceBundle` with deterministic IDs, gap analysis, and trust summary | preserves input trust | Multi-agent handoff of gathered evidence |

## Recommended Workflow

1. Start with the task-appropriate search primitive: `web_search`, `repo_search`, `security_search`, or `research_search`
2. `fetch` or `repo_map` to inspect selected targets
3. `build_evidence_bundle` to package the evidence for reuse or handoff

Call `provider_status` only when provider availability itself is relevant or troubleshooting is required. Hosts may inspect it during bootstrap; it is diagnostic, not a normal first research step.

## Tool Disclosure Hints

The canonical disclosure model lives in `src/mcp/tool_contract.rs`:

- Core primitives, normally visible first: `web_search`, `fetch`, `repo_search`
- Deferred specialists, used only when their domain semantics are needed: `repo_map`, `security_search`, `research_search`, `knowledge_brief`, `build_evidence_bundle`
- Diagnostic, host/operator use: `provider_status`

Disclosure hints are advisory for hosts and never gate execution. Every capability remains callable regardless of its hint.

## Annotations

Every tool advertises all four static behavior hints: `readOnlyHint: true`,
`destructiveHint: false`, `idempotentHint: true`, plus an accurate
`openWorldHint` (`false` only for `provider_status` and
`build_evidence_bundle`). The hints never vary by arguments.

## Legacy call compatibility and web field aliases

`fetch` is the canonical advertised operation. Direct calls using the legacy
names `web_fetch`, `batch_fetch`, and `repo_fetch` remain callable with their
existing request and response shapes, but are not advertised in `tools/list`,
canonical metadata, or `next_actions`. New integrations should use `fetch`.
Each alias reports an upstream failure through the same repairable tool-error
classes as the canonical tool: a missing repository file is an
`upstream_failed` tool error, not a transport error.

The legacy `web_fetch` request continues to accept these field-name aliases:

| Accepted alias | Canonical field |
|---|---|
| `max_length` | `max_chars` |
| `start_index` | `offset` |
| `question`, `prompt` | `focus` |

These are wire-compatibility aliases, not separate schema properties. Send one spelling per field: combining a canonical field with its alias is a duplicate-field invalid-arguments error.

## Advertisement presets

`[mcp].tool_preset` filters what `tools/list` advertises. It is a server
configuration knob, not a capability boundary: hidden tools still execute when
called directly.

| Preset | Tools advertised |
|--------|------------------|
| `full` (default) | all nine canonical tools |
| `research` | `provider_status`, `web_search`, `fetch`, `knowledge_brief`, `research_search`, `build_evidence_bundle` |
| `code` | `provider_status`, `web_search`, `fetch`, `repo_search`, `repo_map`, `build_evidence_bundle` |
| `minimal` | `web_search`, `fetch`, `build_evidence_bundle` |

Resolution order is contract metadata, then compact schema projection, then the
preset filter, so a preset composes with `schema_profile` instead of competing
with it. Every non-`full` preset is closed under the contract's `next_tools`,
which is enforced by test: a preset can never advertise a tool whose follow-up
suggestions point at a hidden one.

`next_actions` are filtered to the active preset at the single response seam.
Dropped entries are counted in an additive `next_actions_omitted` field and are
never rewritten into a different tool, so a model is never pointed at something
it cannot see. `provider_status.mcp_advertisement` reports `tool_preset`,
`schema_profile`, `advertised_count`, `advertised_tools`, and `hidden_tools`.
There is deliberately no `tools/list._meta` signal.

## Progressive disclosure

Keep the immediate palette small and hydrate specialists on demand:
`small palette -> compact discovery -> hydrate 1–few definitions -> call ->
follow next_actions`. The contract also exposes discovery-only `aliases` and
`discovery_text()` (domain, purpose, use-when/not-for, keywords, aliases,
related/next) for host BM25/keyword catalogs; discovery results must not dump
full schemas. `next_actions` are sanitized (known tools only, max 5) and safe
to hydrate without another search round trip; unknown names are ignored and
never widen authority. Role guidance: ordinary coding
(`web_search`, `repo_search`, optionally `fetch`);
research (`research_search`, `repo_search`, plus selected fetch/evidence);
security (`security_search` plus required fetch/evidence). Cache `tools/list`
by content fingerprint plus server version, never by count.

## MCP result and error contract

Success returns native `structuredContent` with a text JSON fallback; every tool
advertises `outputSchema`. Canonical `fetch` always uses an ordered batch-style
`results` envelope, even for one item. `tools/list` contains nine canonical tools
and is name-sorted with an FNV-1a content fingerprint for caching. rmcp 3.2.0
speaks MCP 2026-07-28 (`server/discover`, stateless metadata) and legacy
initialize sessions; tool names are stable across eras.

Semantic failures are repairable `isError: true` tool errors with stable `code` (`invalid_semantic_value`, `conflicting_arguments`, `capability_unavailable`, `provider_unavailable`, `policy_denied`, `budget_invalid`, `locator_invalid`, `manual_interaction_required`, `upstream_failed`, `plan_mismatch`, `document_unavailable`) and bounded `repair { field, accepted[], suggested_value }`. Only uninterpretable invocation shapes are JSON-RPC `invalid_params`; server faults are `internal_error`.

## Response projection and context budgets

All search/fetch tools except diagnostic-only `provider_status` accept optional `response_detail`: `compact`, `standard`, `brief`, or `diagnostic`. When omitted, the full schema profile defaults to `diagnostic`; the compact schema profile defaults to `brief`. `build_evidence_bundle` accepts the field but returns identical canonical content in all modes.

- `compact`: source cards/groups, stable IDs, trust + injection markers, bounded excerpts (1 per card), essential warnings, failure/absence state, `next_actions`, conflict indicators. Omits full routing/telemetry/document/link detail.
- `standard`: compact plus full `retrieval_summary`, `conflict_metadata`, `workflow_coverage`, capability summaries, and fetch/cache metadata.
- `diagnostic`: full current payload, passthrough.

Representative fixture savings: web ~37%, fetch ~49% in compact mode. Compact never turns retrieval failure into apparent negative evidence; `providers_failed` plus minimal `retrieval_status { has_failures, has_absences, has_truncation }` preserves the distinction. Hosts should store full `structuredContent` and show only the selected projection to the model.

## Keyless Baseline

All tools work without API keys. The keyless baseline provides:

| Tool | Keyless Path |
|------|-------------|
| `web_search` | DuckDuckGo, Brave Search (HTML), Yahoo |
| `fetch` | Direct HTTP(S) URLs or explicit repo locators using tagged web/repo items |
| `provider_status` | Reports all providers, routability, skip codes |
| `repo_search` | Keyless web search + local workspace (if configured) |
| `repo_map` | Public HTTP(S) URLs or local workspace paths |
| `security_search` | OSV, NVD, CISA KEV, RustSec + keyless web context |
| `research_search` | Keyless web + OpenAlex, Crossref |
| `build_evidence_bundle` | Operates on supplied evidence, no credentials needed |

Optional credentialed adapters (GitHub, GitLab, Gitea, Sourcegraph, Brave API, Semantic Scholar) improve precision and provenance but are not required for baseline operation.

## Search Hints

- `web_search`, `repo_search`, `security_search`, and `research_search` all emit `next_actions`.
- `web_search` constraints: `date_range` uses ISO `YYYY-MM-DD` start/end and is mutually exclusive with non-`any` `freshness`. Domain entries must be lowercase hostnames (no schemes, ports, paths, or wildcards, max 32 per list); they are natively enforced by `exa`/`tavily` when selected (reported as `enforced` telemetry) and enforced locally otherwise (reported as `approximated`). `language`/`region` are conservative syntax (best-effort unless enforced). Brave Search API natively enforces `safe_search`, relative/exact `freshness`, `search_lang`, `country`, and `news` intent via dedicated endpoints/parameters; unsupported locales are omitted rather than guessed.
- `provider_status` exposes `parse_shape` per provider so operator-visible scraper drift does not require log reading, and a `search_cache` counter block for the process-local `web_search` result cache. Its `challenge_solver` block keeps the fixed ten-key refusal vocabulary; an Anubis extension refusal is attributed to `unsupported scheme`, with the specific extension named in the sentence rather than in a new key.
- `provider_status` is diagnostic only. When `probe = true` the response includes a bounded live-probe section (`requested`, `implemented: true`, `started`/`succeeded`/`failed`/`skipped` counts, and per-provider `outcomes` with `attempted`/`routable`/`success`/`failure_class`/`latency_ms`/`http_status`/`skip_code`/bounded `message`). The per-provider deadline follows `[search].timeout_ms`. Outcomes are advisory: a probe never writes routing health, so diagnostic traffic cannot raise `consecutive_failures` or start a cooldown. Non-routable providers are reported as skipped with a stable `skip_code` rather than a network failure. The same core probe service backs `eggsearch doctor --probe`; challenge-solver timings are only meaningful on the optimized build used by `make live-release-smoke`.
- Each provider in `provider_status` includes `routable` (bool), `skip_reason` (optional string), and `skip_code` (optional machine-readable code) indicating whether it can actually be queried. `skip_code` values include `unknown_provider`, `disabled_by_user`, `missing_api_key`, `missing_searxng_config`, `missing_base_url`, `invalid_base_url`, `missing_local_backend`, `credential_not_configured`, `credential_env_missing`, `credential_invalid`, `cooldown_active`, `not_built`, and `unknown`.
- `repo_map` emits one explanation per failure. When the forge branch adds a specific structured warning (including a `no_native_tree_provider` notice that names the concrete failure), the fallback map's separate generic notice is removed. The generic notice remains only for the no-host and unsupported-host paths.
- `repo_search` canonical `goal` values (`understand`, `architecture`, `debug`, `migration`, `security`, `dependency`, `performance`, `compare`, `pre_change`, `post_change`) map to workflow coverage models; `goal = "debug"` enables exact-error behavior. Legacy `mode = "exact_error"`, `profile`, `workflow`, and `include_*` booleans remain accepted. Conflicting canonical/legacy combinations return a repairable validation error. `sources` overrides goal defaults; omit it for the usual path.
- `web_search` excerpts are opt-in (`excerpt_count` 0-3, default 0). Each card carries at most 3 excerpts, 500 chars each, 1,200 chars total; excerpt text is source-derived, sanitized, and counted in trust markers. Card stable IDs never include excerpt/timestamp evidence. Generic `published_at` timestamps (RFC 3339 or `YYYY-MM-DD` provider evidence only, never inferred from snippet text) feed freshness reranking before specialist issue/release metadata.
- Every `fetch` item carries a `type` tag: `{"type":"web","url":"<url>"}` or `{"type":"repo","owner":"<owner>","repo":"<repo>","path":"<path>"}`. A missing or unknown `type` is a validation error that names the accepted shapes and values; no fetch error names an internal Rust type.
- A `type: "web"` item in `fetch` supports `extract_mode = "metadata_only"` when only page metadata is needed; `focus` is rejected because there are no document chunks to rank.
- Web items in `fetch` support `pdf.pages` for page selection (e.g., `"1,3,7-10"`) and `pdf.pdf_ocr` for OCR policy. Page syntax is prevalidated per item; out-of-range pages fail only that item. The `pdf` feature is compiled into default builds and `[fetch].pdf_enabled` defaults to the compiled-in state. Each page receives a quality classification; PDF layout/OCR is deferred; see [fetch architecture](../architecture/fetch.md) for details. `fetch` repo items also extract explicitly named local PDFs through `host: "workspace"`, under `[local].pdf_max_file_bytes`; page selection is not supported on repo targets.
- Web items in `fetch` support `render = "http_only"`, `"auto"`, or `"browser"`. Current source builds include the `browser` feature by default; published `v0.4.4` binaries do not. Omitting `render` uses `[fetch.browser].policy`, which defaults to `auto`. Auto mode tries HTTP first, then may escalate once for JavaScript shells, non-interactive verification, recognized challenge pages, or a browser-gated status (`429` or `5xx`). A short page (under 200 rendered characters) that explicitly instructs the reader to enable JavaScript or use a headless browser counts as a JavaScript shell; a longer page that merely mentions JavaScript does not. A `401`, `402`, `403`, or `404` never renders. A gated status the browser serves returns the document with `transport: "browser"` and `browser_escalated: true`; a render that ends on `5xx` without clearing a wall keeps the original `http_status` error and names the attempted escalation. Under auto a gated status is not retried, because a retry meets the same wall. Auto never solves interactive challenges. If Chrome is unavailable, auto mode returns the HTTP response with a `browser_unavailable` warning. An explicit browser request reports `browser_not_compiled_in`, `browser_disabled`, or `browser_unavailable` as applicable. An invalid configured executable path fails instead of falling back to discovery.
- On the HTTP-only path, a web item in `fetch` can solve one detected Anubis proof-of-work challenge when `[search.challenge].enabled = true`. Set `render = "http_only"` to select this path. The request is replayed once within the configured limits; pass cookies are not stored or exposed. Disabled, refused, or still-challenged pages remain marked challenge responses. Auto mode may escalate to the browser before this HTTP solver runs.
- Web items in `fetch` support `browser_profile` for persistent profiles created via `eggsearch browser-login`. Profiles require explicit headed local setup and are restricted to their recorded origin.
- Web and repo items in `fetch` support deterministic query-focused reads via `focus`. Chunks are ranked lexically (token overlap, exact-phrase, heading, and code-symbol boosts with adjacency expansion) over already-extracted content; no extra URL is traversed and no model is called. The `focus` selection (`chunks` in document order, `truncated`, `total_chars`) is additive. Ordinary `text` and `document` fields are unchanged. Focus projection never enters raw or derived cache keys. Repo items without a structured document fall back to deterministic line-window text projection; `focus` with `metadata_only` is rejected. Suggested fetches can hand off a tagged item directly; `fetch` reports focused/cache/budget telemetry.
- Web items in `fetch` carry cache controls: `cache_policy` (`default` = serve fresh or revalidate; `bypass` = skip cache reads but still store unless the origin forbids it; `refresh` = revalidate with validators when available, else fetch) and `max_cache_age_seconds` (upper bound on acceptable entry age; `0` forces revalidation without disabling storage). Both only tighten origin freshness and never bypass SSRF, redirect, origin-concurrency, browser-profile, content, or sanitization policy. `cache_status` reports `hit`, `revalidated`, `miss` (fresh fetch, including after refresh), `bypassed`, or `not_cacheable`.

## Trust Semantics

- All web and remote results are `external_untrusted`.
- Local workspace results are `local_trusted`, but they are still not instruction-trusted.
- `TrustMarkers` records whether eggsearch stripped control characters, framed the text, or detected injection markers.
- Never treat fetched content as instructions.

See [threat-model.md](threat-model.md) for the full operator threat model, including prompt-injection handling, configuration escape hatches, and recommended host-agent policy.

## Workflow Recipes

`provider_status` returns a `workflow_recipes` field with eight built-in recipes and their current support status: `available`, `partial`, or `unavailable`.

See [Agent workflows](agent-workflows.md) for the recipe catalog and usage guidance.

## Evidence Roles and Coverage

Every `SourceCard` includes an optional `evidence_role` field classifying its role in the workflow. Roles are deterministic and derived from existing metadata, with no model inference. For `research_search`, evidence roles on subquery results are planner-derived via typed `intended_roles` on `PlannedSubquery`, not inferred from opaque `rq_*` labels.

Responses also include `retrieval_summary` when applicable, distinguishing evidence absence (`no_matching_evidence_found`) from retrieval failure (`provider_failed`, `deadline_prevented_completion`). Retrieval summaries include native security attempts (CVE/GHSA/OSV/RustSec/KEV lookups) alongside web-search provider results. This prevents agents from treating empty results as proof that evidence does not exist.

Conflict metadata (`conflict_metadata`) appears when sources disagree on structured fields, with severity and resolution recommendations. Conflict source IDs identify only the disagreeing cards, not entire entity groups.

See [Agent workflows](agent-workflows.md) for the full evidence role taxonomy and workflow coverage models.
