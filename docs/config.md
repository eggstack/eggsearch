# Configuration

eggsearch reads `$XDG_CONFIG_HOME/eggsearch/config.toml` and falls back to defaults when the file is absent.

## Minimal Default

```toml eggsearch-config
[search]
mode = "live"
default_max_results = 10
max_results_cap = 50
max_query_chars = 512
timeout_ms = 8000
default_providers = ["duckduckgo", "brave", "yahoo"]
sanitize_output = true
cache_ttl_seconds = 300
cache_max_entries = 128

[search.providers]
duckduckgo = true
brave = true
startpage = true
yahoo = true

[fetch]
enabled = true
timeout_ms = 8000
max_bytes = 2000000
max_chars_default = 12000
max_chars_cap = 50000
redirect_limit = 5
allow_private_network = false
allow_localhost = false
sanitize_output = true

[local]
enabled = false
roots = []
```

## MCP Tool Schema Profile

The server advertises the complete ten-tool schema by default. Direct clients
with tight model contexts can opt into a smaller advertisement in the same
config file:

```toml
[mcp]
schema_profile = "compact"
brief_max_chars = 8000
excerpt_budget_chars = 4000
tool_preset = "full"
```

Compact mode keeps all ten tool names and required inputs, trims optional
schema guidance, and omits `outputSchema` metadata. Runtime argument parsing
and tool behavior are unchanged; advanced accepted inputs can still be sent.
Omitted `response_detail` defaults to `brief` in compact mode; explicit
`diagnostic`, `standard`, or `compact` calls override it. `brief_max_chars`
defaults to 8,000 and must be between 512 and 32,000. Brief output preserves
status, source identity, trust, critical warnings and security applicability;
when applicability records exceed the budget, per-state totals remain in
`applicability_summary`; omitted provider failures and batch member indexes
are also counted. The structured result and text fallback are both brief.
Switch to `schema_profile = "full"` for complete
schema guidance and the existing diagnostic-by-default response behavior.

`tool_preset` selects how many tools `tools/list` advertises. It defaults to
`full` and accepts `full`, `research`, `code`, or `minimal`; any other value is
a parse error naming the four accepted values. Resolution order is fixed:
contract metadata, then compact schema projection, then the preset filter.
`tool_preset` never changes the `response_detail` default — that stays
`schema_profile`'s job, and `full` plus `full` is byte-identical to the
pre-capability advertisement.

| Preset | Tools advertised |
|--------|------------------|
| `full` | all ten |
| `research` | `provider_status`, `web_search`, `web_fetch`, `research_search`, `repo_fetch`, `batch_fetch`, `build_evidence_bundle` |
| `code` | `provider_status`, `web_search`, `web_fetch`, `repo_search`, `repo_fetch`, `repo_map`, `batch_fetch`, `build_evidence_bundle` |
| `minimal` | `web_search`, `web_fetch`, `batch_fetch`, `build_evidence_bundle` |

Hidden tools are hidden from `tools/list` only. They still execute when called
directly, their argument schemas are unchanged, and no tool is removed or
renamed. `next_actions` are filtered to the active preset so a model is never
pointed at a tool it cannot see; each drop is counted in `next_actions_omitted`
rather than rewriting the suggestion. Measured advertisement sizes (full /
compact serialized bytes): `full` 73,161 / 19,611; `research` 65,560 / 16,498;
`code` 68,429 / 16,820; `minimal` 57,985 / 13,116. The compact `minimal`
preset is 66% of the compact `full` size — the remainder is mandatory schema
envelope, so no preset reaches half without a schema change.

`excerpt_budget_chars` defaults to 4,000 and must be between 256 and 32,000.
It is one response-wide ceiling on card passage text — `snippet` plus
`excerpts[].text` — measured in serialized characters including the
`<<<EXTERNAL_UNTRUSTED …>>>` framing, not a per-card cap and not a token
count. It applies to `compact` and `brief` only; `standard` and `diagnostic`
responses are unchanged. Within the budget, passages whose normalized token
shingles overlap by at least 0.80 collapse to the earliest occurrence, so an
identical snippet repeated across results is not paid for twice. The response
reports the outcome in a top-level `excerpt_economy` object
(`budget_chars`, `considered_count`, `admitted_count`, `redundant_count`,
`over_budget_count`, `clipped_count`, `chars_before`, `chars_after`), and a
card whose passages were removed carries `excerpts_suppressed`. Suppression
never reorders results, changes a stable ID, or drops a whole result.

## Provider Requirements

All 44 built-in providers:

| Provider | Kind | Requires | Notes |
|----------|------|----------|-------|
| `duckduckgo` | html_scrape | — | Default generic provider |
| `brave` | html_scrape | — | Default generic provider |
| `startpage` | html_scrape | — | Selectable; the upstream Anubis proof-of-work wall is solved when `[search.challenge].enabled = true` |
| `yahoo` | html_scrape | — | Default generic provider |
| `mojeek` | html_scrape | — | |
| `searxng` | json_api | `base_url` | Requires `[search.searxng]` config |
| `brave_api` | api_key | `BRAVE_API_KEY` | |
| `exa` | api_key | `EXA_API_KEY` | Exa semantic search (native freshness/domain filters, highlights on excerpt demand) |
| `tavily` | api_key | `TAVILY_API_KEY` | Tavily search (native safe-search/freshness/language/region/domain/news, chunks on excerpt demand) |
| `serpapi` | api_key | `SERPAPI_API_KEY` | SerpApi Google search (native safe-search/language/region; `base_url` is the full endpoint URL) |
| `kagi` | api_key | `KAGI_API_KEY` | Kagi Search API v1 (native safe-search/freshness/region/domain/timestamps; billed separately from a Kagi subscription) |
| `github_code` | api_key | `GITHUB_TOKEN` | |
| `github_issues` | api_key | `GITHUB_TOKEN` | |
| `github_releases` | api_key | `GITHUB_TOKEN` | |
| `gitlab_code` | api_key | `GITLAB_TOKEN` | |
| `gitlab_issues` | api_key | `GITLAB_TOKEN` | |
| `gitlab_releases` | api_key | `GITLAB_TOKEN` | |
| `gitea_code` | api_key | custom env | Requires `base_url` in `[search.api.gitea_code]` |
| `gitea_issues` | api_key | custom env | Requires `base_url` in `[search.api.gitea_issues]` |
| `gitea_releases` | api_key | custom env | Requires `base_url` in `[search.api.gitea_releases]` |
| `osv` | json_api | — | Security advisory search |
| `github_advisory` | api_key | `GITHUB_TOKEN` | GitHub Security Advisories |
| `nvd` | json_api | — | NIST National Vulnerability Database |
| `cisa_kev` | json_api | — | CISA Known Exploited Vulnerabilities |
| `rustsec` | json_api | — | RustSec Advisory Database |
| `crates_io` | json_api | — | crates.io package metadata |
| `pypi` | json_api | — | PyPI package metadata |
| `npm_registry` | json_api | — | npm package metadata |
| `go_pkg` | json_api | — | Go Proxy module metadata |
| `maven_central` | json_api | — | Maven Central artifact metadata |
| `nuget` | json_api | — | NuGet package metadata |
| `rubygems` | json_api | — | RubyGems gem metadata |
| `packagist` | json_api | — | Packagist package metadata |
| `openalex` | json_api | — | OpenAlex scholarly search |
| `crossref` | json_api | — | Crossref scholarly search |
| `semantic_scholar` | api_key | `SEMANTIC_SCHOLAR_API_KEY` | Semantic Scholar scholarly search |
| `sourcegraph` | api_key | `SOURCEGRAPH_API_KEY` | Sourcegraph code search |
| `firecrawl_developer` | json_api | optional `FIRECRAWL_API_KEY` | Firecrawl Developer Index (keyless-optional; enable in [search.providers]) |
| `wikipedia` | json_api | — | MediaWiki Action API article search (enable in [search.providers]) |
| `arxiv` | structured_api | — | arXiv Atom metadata API with shared request pacing (enable in [search.providers]) |
| `pubmed` | json_api | optional `NCBI_API_KEY` | NCBI E-utilities PubMed search (optional `NCBI_API_EMAIL` contact identity) |
| `hn_algolia` | json_api | — | Hacker News story search with native freshness filters |
| `github_repositories` | json_api | optional `GITHUB_TOKEN` | GitHub repository discovery (not code search) |
| `local_workspace` | local | — | Requires `[local]` config |

`provider_status` returns `routable: true` only when a provider is both enabled and fully configured. Non-routable providers include a `skip_reason` explaining why (e.g. "API key not configured", "SearXNG base_url not configured") and a machine-readable `skip_code` (e.g. `missing_api_key`, `missing_searxng_config`, `disabled_by_user`, `cooldown_active`).

The `Kind` column is the `provider_status` `kind` field. `structured_api` was added alongside `html_scrape`, `json_api`, `api_key`, and `local` for structured non-JSON upstream contracts (the arXiv Atom feed). It is purely additive: existing wire values are unchanged, and harnesses should treat an unrecognized `kind` string as an opaque category.

## Search Defaults

The shipped generic search defaults require no API keys:

- `duckduckgo`
- `brave`
- `yahoo`

Other built-in providers can be enabled explicitly. Credentialed providers
are disabled by default and produce provider-scoped skip telemetry when
credentials are absent.

| Setting | Default |
|---------|---------|
| `mode` | `live` |
| `default_max_results` | `10` |
| `max_results_cap` | `50` |
| `max_query_chars` | `512` |
| `timeout_ms` | `8000` |
| `sanitize_output` | `true` |
| `cache_ttl_seconds` | `300` (`0` disables; max `86400`) |
| `cache_max_entries` | `128` (`1`–`4096`) |

Profiles are advisory. The built-in profiles are `generic`, `coding`, `security`, and `research`. eggsearch skips unavailable providers and reports warnings instead of failing the request.

`repo_search` canonical `goal` (`understand`, `architecture`, `debug`, `migration`, `security`, `dependency`, `performance`, `compare`, `pre_change`, `post_change`) selects workflow coverage; `goal = "debug"` enables exact-error behavior. Legacy `mode = "exact_error"`, `profile`, `workflow`, and `include_*` booleans remain accepted for backward compatibility.

## Intent-Aware Provider Routing

`web_search` already derives a retrieval intent from the query. Routing can use
that intent to add specialist providers to the default candidate set — for
example asking `osv` on a security query, or `github_code` on a code query. Routing only **adds**: it never removes, reorders, or displaces an
explicitly requested provider, a profile's providers, or the shipped default
list.

```toml
[search.routing]
intent_mode = "apply"     # "off" | "apply"
max_extra_providers = 2   # 0..=4
```

| Setting | Default | Bounds | Meaning |
|---------|---------|--------|---------|
| `intent_mode` | `apply` | `off`, `apply` | `off` derives and reports the intent plus the routing decision but adds nothing. `apply` admits the intent's preferred providers. |
| `max_extra_providers` | `2` | `0`–`4` | Cap on providers added by one routing decision. `0` reports without adding. |

Precedence is fixed: an explicit `providers` argument wins, then a supplied
`profile`, then the default path plus intent additions, then the existing
cooldown and health filter.

| Derived intent | Preferred providers, in order | Capability each must declare |
|---|---|---|
| `web`, `docs` | none | — |
| `code` | `github_code`, `gitlab_code`, `gitea_code`, `sourcegraph` | declares code-search or repository-filter support |
| `issues` | `github_issues`, `gitlab_issues`, `gitea_issues`, `firecrawl_developer` | declares issue-search support |
| `releases` | `github_releases`, `gitlab_releases`, `gitea_releases` | declares release-search support |
| `security` | `osv`, `github_advisory`, `nvd` | declares security-search support |
| `news` | `tavily`, `brave_api` | declares news support |

`rustsec` is deliberately absent from the `security` row: it serves advisory-ID lookup only, so a routed keyword query would always fail as an unsupported operation rather than return evidence.

A candidate is admitted only when it is enabled in `[search.providers]`, was
actually built into the engine set, is not cooling down, is admitted by the
keyword-query allowlist, and satisfies the capability predicate for its table
row. Each disposition is reported in `provider_status` as `added`,
`already_selected`, `not_enabled`, `not_built`, `cooling_down`,
`keyword_unsupported`, `capability_missing`, or `cap_exceeded`. A candidate
excluded for capability reasons emits the `intent_routing_capability_excluded`
warning rather than being silently dropped; `keyword_unsupported` names a
provider that knows the subject but cannot answer a keyword query, such as
`rustsec`.

`intent_mode` ships `apply`. The default follows the tier-1 candidate-snapshot
gate, which measures seven shipped-default-set cases (one per disclosed intent)
against the `security-nvd-first` negative control: routing adds the specialist
for every non-empty preference row, `recall_at_5`, `MRR@5`, `nDCG@5`,
authoritative survival, and `distinct_domains_topk` all improve on those five
cases, no case regresses, and the control is detected with a 1.0 `recall_at_5`
drop across two identical runs.
`tests/retrieval_quality.rs::intent_routing_rollout_gate_matches_the_recorded_decision`
recomputes that comparison and fails if the default and the recorded decision
disagree.

## Offset Windows, Progress, and Tasks

Long-running reads accept a random-access window instead of returning one
truncated payload.

```toml
[mcp.tasks]
enabled = false
ttl_ms = 300000
max_concurrent = 4
poll_interval_ms = 1000
```

| Setting | Default | Bounds | Meaning |
|---------|---------|--------|---------|
| `enabled` | `false` | — | Exposes the MCP Tasks extension to clients that negotiate it. While off, every long call runs synchronously. |
| `ttl_ms` | `300000` | `1000`–`3600000` | Lifetime of a task record held in process memory. |
| `max_concurrent` | `4` | `1`–`16` | Concurrent task cap. Above it, a call silently falls back to the synchronous path — never an error. |
| `poll_interval_ms` | `1000` | `250`–`10000` | Suggested client polling interval reported with a task. |

**Offset windows** are always available and need no configuration. `web_fetch`,
`batch_fetch`, and `research_search` accept optional `offset` and `limit` for
random access over what they address. A read is the original request
re-specified with the window; nothing is stored server-side.

| Tool | Unit | `offset` | `limit` default | `limit` ceiling |
|------|------|----------|-----------------|-----------------|
| `web_fetch` | characters of the joined chunk text | zero-based scalar offset | the effective `max_chars` | `[fetch].max_chars_cap` |
| `batch_fetch` | items | zero-based item index | `max_items` | `[fetch].batch_max_items_cap` |
| `research_search` | non-empty result groups | zero-based group index | the `max_groups`-derived page size | 14 (`RESEARCH_PAGE_GROUPS`) |

`web_fetch` also accepts `start_index` for `offset`, `max_length` for
`max_chars`, and `question` or `prompt` for `focus` as wire compatibility
aliases; see the [tool matrix](tool-matrix.md#compatibility-aliases). Aliases
do not change the window rules below.

Ranges are half-open `[from, to)` in Unicode scalar values and clamp rather
than error: an offset at or past the end returns an empty window with
`from == to == total` (and `text == ""` for `web_fetch`). A response is
addressed when at least one of `offset`/`limit` is supplied; addressed
responses carry `text_from`/`text_to`/`text_total`/`doc_id` (`web_fetch`),
`item_from`/`item_to`/`item_total` (`batch_fetch`), or `group_from` plus
`aggregate_scope`/`total_groups`/`returned_groups` (`research_search`), and
`next_offset` whenever more units remain. `web_fetch` also reports
`focus_scope: "document"` when a non-null `focus` accompanies the read.

`text_total` is the Unicode scalar count of every chunk text joined by a single
`\n`, measured before control-character stripping and before trust framing;
`document.text_chars_returned` stays window-scoped. `doc_id` is the deterministic
`doc_<16hex(url, title, kind)>` document identity, not a content hash, so two
different bodies at one URL under one title share it.

`limit = 0` is `invalid_semantic_value` with `repair.field = "limit"`; a `limit`
above the server maximum is clamped and reported in `structured_warnings` under
`max_results_clamped`. An addressed request whose plan bound the server had to
clamp is `plan_mismatch` (`focus_max_chars` above `[fetch].max_chars_cap`;
`max_items` or `max_total_chars` above their `[fetch]` caps; `max_results` above
`[search].max_results_cap` or `max_per_group` of 0). Reading by offset never
widens a bound: the effective timeout is
`min(caller timeout_ms, [fetch].timeout_ms)`, and no call exceeds its
character, item, or group ceiling.

**Progress** is emitted only when the client supplies a progress token. It is
rate-limited to one notification per 250 ms, capped at 64 notifications and
4 KiB per call, carries counts rather than content, and is sent without ever
delaying the response.

**Tasks** require the client to declare the extension; otherwise the call stays
synchronous. Cancellation is cooperative. Task ids are transport state and never
enter evidence identity.

## Challenge Solving

Some keyless upstreams answer a request with a computational proof-of-work
challenge instead of results. ADR-0009 permits solving those in-process and
permits nothing else: no interactive CAPTCHA, image puzzle, OCR, human or
third-party solving service, credential bypass, proxy rotation, or
user-agent spoofing. The capability is off by default.

```toml
[search.challenge]
enabled = false           # operator opt-in
max_difficulty = 5        # Anubis ceiling; difficulty 6 is ~16.8M candidates
max_attempts = 2000000    # candidate / PBKDF2-derivation ceiling per solve
timeout_ms = 2000         # wall-clock ceiling per solve
cookie_ttl_seconds = 240  # pass-cookie reuse window, per provider
```

| Setting | Default | Bounds | Meaning |
|---------|---------|--------|---------|
| `enabled` | `false` | — | Turns the solver on. While off, every challenged provider behaves exactly as it did before the capability existed. |
| `max_difficulty` | `5` | `1`–`10` | Refuses an Anubis challenge whose published `rules.difficulty` is above this. This is the operator cost knob, not a fixed engine constant. A `sha256` challenge publishes leading zero bits and is compared as `ceil(bits / 4)`, so the same ceiling stays fast-equivalent. |
| `max_attempts` | `2000000` | `1`–`100000000` | Hard candidate ceiling for one solve. A solve that reaches it fails; it never loops. The default matches the default difficulty ceiling of 5; raise it together with `max_difficulty`. |
| `timeout_ms` | `2000` | `100`–`30000` | Wall-clock ceiling for one solve. A difficulty-6 Anubis challenge needs at least `8000`. |
| `cookie_ttl_seconds` | `240` | `0`–`3600` | How long the replayed pass-cookie pair set is reused for that provider. `0` disables reuse, so every request re-solves. |

Two upstreams use this gate. `mojeek` publishes an ALTCHA challenge
(`PBKDF2/SHA-256`, cost 8000); a captured live challenge resolved at counter
291 in roughly 0.4 s. `startpage` publishes an Anubis challenge whose
difficulty is risk-scored per client: the honest `eggsearch` user agent
received difficulty 6 while a browser user agent received 4. Expected cost is
`16^difficulty` SHA-256 candidates. A measured difficulty-6 solve took 23.07M
candidates in 3.05 s on one core in a release build; difficulty 5 is roughly
1.05M candidates and difficulty 4 roughly 65.5K. Debug builds are about two
orders of magnitude slower and are not a valid baseline. Raising
`max_difficulty` to 6 therefore also requires raising `max_attempts`, whose
2,000,000 default matches the default ceiling of 5, plus `timeout_ms` and
`[search].timeout_ms` of at least 8000.

Anubis method coverage follows the published registry: `fast`, `slow` (the
deprecated alias of `fast`), `preact`, `metarefresh`, and the WASM `sha256`
method (ADR-0013). `metarefresh` echoes `challenge.randomData` after the wait
the page or `Refresh:` header published, a bounded JSON `wait` when neither
exists, or `difficulty + 1` seconds otherwise, and performs no hashing.
`sha256` publishes leading zero bits; because `max_difficulty` stays measured in
fast-equivalent nibbles, such a challenge is admitted when `ceil(bits / 4)` is
at or below the ceiling (4 admits 16 bits and rejects 17; 6 admits 24), and the
cost projection uses `2^bits`. `argon2id` and `hashx` remain
`unsupported scheme`. A `metarefresh` wait longer than `timeout_ms` is refused
with `challenge not solved: wall clock exhausted` rather than sleeping past the
budget.

`challenge.extensions` is parsed and bounded: absent, `null`, and `[]` are
solvable, and a present list holds at most 8 names of 1–64 bytes each from
ASCII alphanumerics, `-`, `_`, and `.`. A non-empty list containing any name
other than `css-load` is refused before the ceiling, any wait, any hash, and
any request, with `anubis challenge declares unsupported extensions [<names>]`
recorded under `unsupported scheme`. With `enabled = true`, `css-load` — the
one extension ADR-0012 permits — costs exactly one same-origin `GET` to
`{origin}{base_prefix}/.within.website/x/cmd/anubis/ext/css-load/{uuid}` with
no cookie, no authorization header, redirects disabled, and a discarded 4 KiB
body; a non-2xx status or transport failure refuses with `blocked` under
`unsupported scheme` and zero pass requests. `headless` and unknown names are
refused with zero requests.

Before hashing, the solver projects the published work (`16^difficulty`
candidates for the nibble methods, `2^bits` for `sha256`) against the configured
ceilings and refuses with the projection in the reason, for example
`anubis difficulty 7 projects 268M candidates, beyond the configured attempt
ceiling 2000000` or `anubis difficulty 5 projects 1M candidates (~136 ms),
beyond the configured wall-clock budget 50 ms`. The projection admits a
challenge; it never replaces the per-attempt or wall-clock ceiling. An already
spent challenge reports `already spent`, and an unusable published base prefix
reports `unsafe base prefix`, both before any request or hashing.

Pass-cookie identity is opaque: the solver never looks for a fixed cookie name.
It takes every non-deletion pair the pass-challenge response set — empty values
and deletion cookies dropped, duplicate names resolved by the last non-deletion
value — replays them joined with `; `, and derives the reuse window from the
longest usable published lifetime among them, clamped by `cookie_ttl_seconds`.
`spchal-auth`, `techaro.lol-anubis-auth`, and dynamic-suffix names work
unchanged, ALTCHA keeps its published `chllg` name, and there is no cookie-name
knob. The table applies to each replayed pair; the longest usable value wins:

| Response attribute | Stored window |
|---|---|
| neither `Max-Age` nor `Expires` | `cookie_ttl_seconds` (unchanged) |
| `Max-Age=N` | `min(N, cookie_ttl_seconds)`; `Max-Age=0` stores nothing |
| `Expires=<IMF-fixdate>` | `min(expiry - now, cookie_ttl_seconds)`; an expiry at or before now stores nothing |
| both `Max-Age` and `Expires` | `Max-Age` wins (RFC 6265 §5.3) |
| an attribute present but unusable | nothing is stored (fail closed) |

A stored window never exceeds `cookie_ttl_seconds`, so the setting stays the
operator ceiling, and `cookie_ttl_seconds = 0` disables reuse regardless of what
an upstream published. A pair set the upstream rejects on replay is evicted
immediately, so the next request starts from a challenge page instead of sending
tokens the server has already refused.

When `[egress]` is routed (`enabled = true` with at least one hop) and challenge
solving reuses pass cookies, `eggsearch doctor` warns that a pass token bound to
the client's egress address is re-challenged or rejected when the exit address
changes, and suggests `cookie_ttl_seconds = 0` when the route does not keep one
exit address per process. eggsearch does not detect egress addresses, never
shortens the window because a route exists, and never rotates exits; the warning
is the whole mitigation and rejection eviction is the recovery.

`web_fetch` uses the same opt-in. With `enabled = true`, a fetched page classified as an Anubis
challenge is solved once and the request replayed once with the cookie pairs the pass response
issued, so the caller receives the protected document instead of the wall. `max_difficulty`,
`max_attempts`, and `timeout_ms` apply unchanged, further bounded by the remaining `web_fetch` call
deadline; `cookie_ttl_seconds` does not apply, because a fetch-side pair set is used for that one
replay and never stored. When solving is off, refused, unsupported, or the replay is still
challenged, `web_fetch` keeps returning the marked page (`manual_interaction_required: true` plus one
`fetch_challenge_page` warning). `batch_fetch` does not solve yet.

Every `provider_status` call reports a `challenge_solver` block with
process-lifetime per-provider solve and refusal counters plus the last attempt's
candidate count and bounded-computation milliseconds; the field set is described
in `architecture/engines.md` § Challenge solving.

The solve is probabilistic: the nonce is uniform, so an attempt can legitimately
reach `max_attempts` without a hit and report `blocked` with
`challenge not solved: attempts exhausted`. At roughly twice the expected work
about one attempt in seven does; a later call usually succeeds. Treat it as a
retryable provider condition, not a defect.

Solving is attempted at most once per provider per search request and reused
for `cookie_ttl_seconds`. Pass cookies (the opaque Anubis pair set and ALTCHA's
`chllg`) are held in process memory only, are never written to disk, and never
appear in logs, `provider_status`, diagnostics, or fixtures. A challenged
provider that is disabled, above the ceiling, unsupported, or out of budget
still reports `error_class: blocked` with a reason naming the bound — never an
empty result. A solved page that fails to parse is `parse_error`, not `blocked`.

Raising `max_difficulty` does not by itself make `startpage` return rows: its
result extraction needs the payload parser described in
`architecture/engines.md` § Challenge solving.

## Provider Profiles

All profiles are advisory routing preferences. Unavailable providers are skipped with warnings, never errors. **Profiles never require API keys to produce useful results.** Missing optional credentials result in provider-scoped skips and degraded coverage, not request failure.

### Keyless Default

The shipped default configuration. No configuration file or API keys needed.

```toml eggsearch-config
[search]
mode = "live"
default_max_results = 10
max_results_cap = 50
max_query_chars = 512
timeout_ms = 8000
default_providers = ["duckduckgo", "brave", "yahoo"]
sanitize_output = true

[search.providers]
duckduckgo = true
brave = true
startpage = true
yahoo = true

[fetch]
enabled = true
timeout_ms = 8000
max_bytes = 2000000
max_chars_default = 12000
max_chars_cap = 50000
redirect_limit = 5
allow_private_network = false
allow_localhost = false
sanitize_output = true

[local]
enabled = false
roots = []
```

### Keyless Coding

Uses keyless web providers. Local workspace may be enabled without credentials. Forge-native adapters are optional enhancements.

```toml eggsearch-config
[search]
mode = "live"
default_max_results = 10
max_results_cap = 50
max_query_chars = 512
timeout_ms = 8000
default_providers = ["duckduckgo", "brave", "yahoo"]

[search.providers]
duckduckgo = true
brave = true
startpage = true
yahoo = true

[local]
enabled = false
roots = []
```

### Keyless Security

Uses OSV, NVD, CISA KEV, RustSec, and keyless web context. GitHub Advisory is optional.

```toml eggsearch-config
[search]
mode = "live"
default_max_results = 10
max_results_cap = 50
max_query_chars = 512
timeout_ms = 8000
default_providers = ["osv", "duckduckgo", "brave"]

[search.providers]
duckduckgo = true
brave = true
startpage = true
osv = true
```

### Keyless Research

Uses keyless web plus OpenAlex and Crossref. Brave API, SearXNG, and Semantic Scholar are optional.

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
startpage = true
```

### Enhanced Coding

Optional GitHub/GitLab/Gitea/Sourcegraph adapters improve forge-native precision and provenance. These are **not required** for baseline coding search.

```toml eggsearch-config
[search]
mode = "live"
default_max_results = 10
max_results_cap = 50
max_query_chars = 512
timeout_ms = 8000
default_providers = ["github_code", "github_issues", "github_releases", "duckduckgo", "brave"]

[search.providers]
duckduckgo = true
brave = true
startpage = true
yahoo = false
mojeek = false
searxng = false
osv = false

[search.api.github_code]
enabled = true
api_key_env = "GITHUB_TOKEN"

[search.api.github_issues]
enabled = true
api_key_env = "GITHUB_TOKEN"

[search.api.github_releases]
enabled = true
api_key_env = "GITHUB_TOKEN"
```

Repeat the same `search.api` pattern for `gitlab_code`, `gitlab_issues`, `gitlab_releases`, `gitea_code`, `gitea_issues`, and `gitea_releases` with the token environment variable used by your forge.

### Enhanced Security

GitHub Advisory token is optional. Improves advisory coverage with GitHub-sourced advisories.

```toml eggsearch-config
[search]
mode = "live"
default_max_results = 10
max_results_cap = 50
max_query_chars = 512
timeout_ms = 8000
default_providers = ["osv", "duckduckgo", "brave"]

[search.providers]
duckduckgo = true
brave = true
startpage = true
osv = true

[search.api.github_advisory]
enabled = true
api_key_env = "GITHUB_TOKEN"
```

### Enhanced Research

Brave API, SearXNG, and Semantic Scholar are optional enhancements for richer scholarly search.

```toml eggsearch-config
[search]
mode = "live"
default_max_results = 10
max_results_cap = 50
max_query_chars = 512
timeout_ms = 8000
default_providers = ["brave_api", "searxng", "duckduckgo", "brave", "mojeek"]

[search.providers]
duckduckgo = true
brave = true
startpage = true
mojeek = true
searxng = true

[search.searxng]
enabled = true
base_url = "https://search.example.org"
```

### Developer Index (keyless-optional)

Firecrawl Developer Index is an opt-in specialist for repo_search coding evidence. It routes keyless; a key only raises limits.

```toml eggsearch-config-parse-only
[search.providers]
firecrawl_developer = true

[search.api.firecrawl_developer]
enabled = true
api_key_env = "FIRECRAWL_API_KEY"
```

A missing or empty optional env var falls back keyless with a warning, never `missing_api_key`.

### Local Workspace

```toml eggsearch-config-parse-only
[local]
enabled = true
roots = ["/Users/alice/projects/checkout"]
max_file_bytes = 1048576
max_indexed_files = 50000
include_hidden = false
respect_gitignore = true
follow_symlinks = false
structured_symbols = true
max_parse_bytes = 262144
max_symbols_per_file = 256
max_structured_files = 200
max_total_symbols = 5000
repo_map_structure_cap = 500
```

When enabled, the local workspace inventory is built automatically on first search (auto-build on cache miss). The git fast path uses `git ls-files -z --cached --others --exclude-standard` with a bounded command runner (5s timeout, 16MB stdout / 64KB stderr caps, concurrent pipe drainage, cap breaches trigger immediate process termination). A `git status --porcelain=v2` hash is stored alongside the inventory for change detection between builds. Native directory walking is the fallback for non-git directories.

Structured code intelligence is dependency-free and deterministic (Rust, Python, JavaScript/TypeScript, Go): functions/methods, structs/classes/types, traits/interfaces, impl relationships, modules, imports, and syntax-level test items. The regex backend remains the fallback when parsing is disabled, unsupported, fails, or exceeds budget. Budgets breach to partial/regex evidence with telemetry (`structured_files_parsed`, `structured_symbols_found`, `regex_fallback_files`, `symbol_budget_breaches`), never to failure. `repo_map` gains bounded additive `packages`, `language_distribution`, `modules`, `entrypoints`, `top_symbols`, `test_relationships`, `build_configs`, and `structure_truncated` from the same local scan. No workspace code is executed.

### SearXNG

```toml eggsearch-config-parse-only
[search.providers]
searxng = true

[search.searxng]
enabled = true
base_url = "https://search.example.org"
```

### Gitea

```toml eggsearch-config-parse-only
[search]
default_providers = ["gitea_code", "gitea_issues", "gitea_releases", "duckduckgo"]

[search.providers]
duckduckgo = true
gitea_code = true
gitea_issues = true
gitea_releases = true

[search.api.gitea_code]
enabled = true
api_key_env = "GITEA_TOKEN"
base_url = "https://gitea.example.org"

[search.api.gitea_issues]
enabled = true
api_key_env = "GITEA_TOKEN"
base_url = "https://gitea.example.org"

[search.api.gitea_releases]
enabled = true
api_key_env = "GITEA_TOKEN"
base_url = "https://gitea.example.org"
```

### Low-Power / Conservative

Reduced caps for Raspberry Pi or low-bandwidth environments.

```toml eggsearch-config-parse-only
[search]
default_max_results = 6
max_results_cap = 20
timeout_ms = 6000
multiquery_concurrency = 4
multiquery_provider_concurrency = 1

[fetch]
timeout_ms = 6000
max_bytes = 1000000
max_chars_default = 8000
max_chars_cap = 24000
batch_max_items = 4
batch_max_items_cap = 8
batch_max_chars_per_item = 8000
batch_max_total_chars = 24000
batch_max_total_chars_cap = 60000
batch_concurrency = 2
```

## Fetch Defaults

| Setting | Default |
|---------|---------|
| `enabled` | `true` |
| `timeout_ms` | `8000` |
| `max_bytes` | `2_000_000` |
| `max_chars_default` | `12_000` |
| `max_chars_cap` | `50_000` (document/extraction budget and cost knob, not the returned-text ceiling; validation requires `max_chars_cap >= max_chars_default`) |
| `redirect_limit` | `5` |
| `allow_private_network` | `false` |
| `allow_localhost` | `false` |
| `include_links_default` | `false` |
| `sanitize_output` | `true` |
| `pdf_enabled` | the compiled-in `pdf` feature state: `true` in a default build, `false` under `--no-default-features` |
| `pdf_max_pages` | `25` |
| `pdf_max_chars_per_page` | `12_000` |
| `pdf_max_total_chars` | `50_000` |
| `pdf_max_bytes` | `16_777_216` |
| `pdf_max_decompressed_bytes` | `67_108_864` |

The fetch side is bounded by both byte and character budgets. The byte cap is a hard response limit. `max_chars_cap` is the document/extraction budget — how much of the page is extracted and addressable in `document.blocks` / `document.chunks` — and therefore the cost knob; it is not the returned-text ceiling. `max_chars_default` (and the caller's `max_chars`) bounds only the returned `text` window, clamped to `max_chars_cap`. `document.document_truncated` reports extraction stopping at the document budget; `document.text_truncated` reports window clipping.

`max_bytes` is the body cap for everything except PDF documents. A response
identified as PDF by `Content-Type: application/pdf` or by a `.pdf` URL path
uses `pdf_max_bytes` instead, so an ordinary paper is not refused or truncated
at the generic 2 MB cap. `pdf_max_bytes` must be between 1 and 268,435,456
(256 MiB); the default is 16 MiB. A PDF that is recognised only by its
`%PDF-` magic (no Content-Type, no `.pdf` path) is still read under
`max_bytes`, because the body cap is fixed before the first bytes arrive.
A PDF refused for declared length, or truncated at the effective cap, returns
`content_too_large` naming the configured limit — never a parse error.

`pdf_max_decompressed_bytes` caps how many bytes a PDF may expand into once
its streams are decompressed, measured over the whole document before the
parser runs. The default is 64 MiB, comfortably above the largest baseline
document and four times the `pdf_max_bytes` default; the validated range is
1,048,576 to 1,073,741,824 bytes. A document that would exceed it — by measured
expansion, by a declared xref `/W` geometry, or by using `LZWDecode`, whose
expansion cannot be measured — fails with `pdf_decompression_bound_exceeded`.
Encrypted documents are refused with `pdf_encrypted` whether or not a password
is supplied; `pdf_password` is accepted for wire compatibility and is
unsupported.

`allow_private_network = true` and `allow_localhost = true` are operator escape hatches. Keep them off for general MCP exposure.

When `allow_private_network = false` (the default), eggsearch blocks fetches to all RFC 1918 private ranges (`10.0.0.0/8`, `172.16.0.0/12`, `192.168.0.0/16`), carrier-grade NAT (`100.64.0.0/10`), link-local (`169.254.0.0/16`), multicast, reserved, and documentation addresses. When `allow_localhost = false` (the default), loopback addresses (`127.0.0.0/8`, `::1`) are blocked independently. The two flags are fully independent. Redirect targets are validated against these same ranges. See [safety.md](safety.md#blocked-address-ranges) for the full policy table.

## Reserved Live Settings

`[search].live.user_agent` and `[search].live.respect_robots_txt` are parsed for compatibility but are no-ops in the current build.

## Outbound Proxy-Chain Routing

Optional `[egress]` section for listener-free HTTP/SOCKS proxy-chain routing
of fixed provider upstreams. Disabled by default and direct unless enabled.
Requires a source build with `--features egress`; prebuilt/default binaries
reject an enabled route instead of silently ignoring it. Raw proxy passwords
are never stored: each hop references a credential environment variable.
Hop `host` accepts a bare hostname, IPv4 literal, or bare IPv6 literal
(`::1`, `2001:db8::1`); schemes, userinfo, paths, brackets, and appended
ports are rejected, and `port` remains the sole port source.

```toml eggsearch-config-parse-only
[egress]
enabled = false

[[egress.hops]]
scheme = "http"
host = "127.0.0.1"
port = 8080
```

See [features.md](features.md#outbound-proxy-chain-routing) for the full
operator contract, traffic scope, and troubleshooting.

## Local Workspace Defaults

Local workspace search is disabled by default.

| Setting | Default |
|---------|---------|
| `enabled` | `false` |
| `roots` | empty |
| `max_file_bytes` | `1_048_576` |
| `max_indexed_files` | `50_000` |
| `include_hidden` | `false` |
| `respect_gitignore` | `true` |
| `follow_symlinks` | `false` |
| `structured_symbols` | `true` |
| `max_parse_bytes` | `262_144` |
| `max_symbols_per_file` | `256` |
| `max_structured_files` | `200` |
| `max_total_symbols` | `5_000` |
| `repo_map_structure_cap` | `500` |

When enabled, local results use `local_trusted` trust labels and can be surfaced through `repo_search`, `repo_fetch`, and `repo_map`. They remain provenance-trusted, not instruction-trusted.

## Browser Rendering

The `[fetch.browser]` section configures headless Chrome/Chromium rendering. The `browser` Cargo feature is compiled into default builds, `enabled` defaults to that compiled-in state, and `policy` defaults to `auto`. Rendering uses an already-installed Chrome/Chromium; eggsearch never downloads or installs one.

| Setting | Default | Description |
|---------|---------|-------------|
| `enabled` | `true` when compiled | Enable browser rendering escalation; `false` in a `--no-default-features` build |
| `policy` | `"auto"` | Render policy used when the caller omits `render`: `http_only`, `auto`, or `browser`. A per-call `render` overrides it. |
| `executable` | auto-discovered | Path to Chrome/Chromium executable. An explicitly configured invalid path fails deterministically — it does not fall back to auto-discovery. |
| `startup_timeout_ms` | `10000` | Browser startup timeout |
| `navigation_timeout_ms` | `20000` | Page navigation timeout |
| `post_load_wait_ms` | `1500` | Wait after page load |
| `verification_wait_ms` | `10000` | Wait for non-interactive verification |
| `max_requests` | `100` | Renders per browser session (1–1000). The bound is enforced: reaching it closes the session, and the next render relaunches. |
| `idle_timeout_ms` | `90000` | Idle interval with no in-flight render after which the shared browser process is closed. Minimum `5000`; a lower value fails config validation. |
| `max_dom_bytes` | `4000000` | Maximum DOM size |
| `global_concurrency` | `1` | Global browser concurrency |
| `per_origin_concurrency` | `1` | Per-origin browser render cap (combined with `origin_browser_concurrency` via `min`) |
| `block_media` | `true` | Block media autoplay |

### Persistent Browser Profiles

The `[fetch.browser.persistent_profiles]` section configures named, origin-scoped persistent browser profiles.

| Setting | Default | Description |
|---------|---------|-------------|
| `enabled` | `false` | Enable persistent browser profiles |
| `profiles_dir` | platform default | Custom profiles directory |
| `allowed_profiles` | empty (all allowed) | Allowlist of profile names |
| `profile_process_timeout_ms` | `30000` | Timeout for profile-scoped browser processes |

Profiles are created through CLI-only headed login (`eggsearch browser-login`). MCP callers select profiles by name via the `browser_profile` field on `web_fetch`. Profile metadata lives in `$XDG_DATA_HOME/eggsearch/browser-profiles/<opaque-id>/profile.toml` with Chrome data in a sibling `chrome-data/` directory.

Profiles are disabled by default. When disabled, MCP callers cannot use `browser_profile`. Each profile is restricted to its recorded origin and uses opaque directory IDs for cache partitioning. `browser-login` waits for the operator to press Enter (or until the configured timeout), and uses the configured executable selected during discovery. Later profile-scoped MCP fetches launch against the same profile `chrome-data` directory using the browser's default context. Chrome manages cookies and storage within the profile directory — eggsearch never exports, logs, or serializes cookies. Process-local cache is not invalidated when a profile is removed from the CLI (cache is process-scoped).

## Provider Status

`provider_status` reports configuration-derived provider descriptors, code-host summaries, cached health snapshots, server capabilities, tool capabilities, quality metadata, and workflow recipes. Passing `probe: true` runs a bounded live liveness check through the shared probe service (same core as `eggsearch doctor --probe`).

Each provider descriptor includes:

- `id`, `display_name`, `kind` — identity
- `enabled`, `default`, `configured` — config state
- `requires_api_key` — whether an API key env var is needed
- `routable` — whether the provider can actually be queried right now
- `skip_reason` — human-readable explanation when `routable` is `false`
- `skip_code` — machine-readable skip code when `routable` is `false`
- `capabilities` — feature flags (code search, freshness, etc.)

Skip codes are stable snake_case strings: `unknown_provider`, `disabled_by_user`, `missing_api_key`, `missing_searxng_config`, `missing_base_url`, `invalid_base_url`, `missing_local_backend`, `credential_not_configured`, `credential_env_missing`, `credential_invalid`, `cooldown_active`, `not_built`, `unknown`.

`provider_status` is diagnostic: hosts may inspect it during bootstrap and agents call it only when provider availability itself is relevant or troubleshooting is required. It is not a normal first research step.
