# Configuration Deep Dive

**Path:** `src/core/config.rs` + `src/config.rs`
**Purpose:** TOML configuration model, provider resolution, validation, and CLI config loading.

---

## Config Loading

### Default Path

`default_config_path()` resolves via `dirs::config_dir()`:

```
$XDG_CONFIG_HOME/eggsearch/config.toml            (Linux)
~/Library/Application Support/eggsearch/config.toml (macOS)
%APPDATA%/eggsearch/config.toml                     (Windows)
eggsearch.toml                                      (fallback, current directory)
```

### `--config` Override

`src/config.rs` is a thin delegation module. `load(Option<&Path>)`
uses the caller-supplied path when `--config <path>` is given and
falls back to `default_config_path()` otherwise, then delegates to
`AppConfig::load()`.

### Load Semantics

- Missing file returns `AppConfig::default()` silently. A clean
  install with no config file is a healthy keyless server, not an error.
- Present files are parsed as TOML, then `validate()` runs. The first
  violated invariant fails the load with `CoreError::Config`.
- `AppConfig::save()` creates parent directories as needed and writes
  pretty TOML. Round-trip (`save` then `load`) preserves all sections.

---

## Section / Field Map

Root type `AppConfig` holds four sections: `search`, `fetch`,
`local`, `egress`. Every section deserializes with `#[serde(default)]`
so partial files inherit production defaults.

### `[search]` — `SearchSection`

| Field | Type | Default | Purpose |
|-------|------|---------|---------|
| `mode` | `Mode` | `live` | `off` disables all tools; `live` allows metasearch |
| `default_max_results` | `usize` | `10` | Result count when the caller omits it (`max_results` is a TOML alias) |
| `max_results_cap` | `usize` | `50` | Hard cap on caller-requested `max_results` |
| `max_query_chars` | `usize` | `512` | Maximum accepted query length |
| `timeout_ms` | `u64` | `8000` | Per-request search timeout |
| `default_providers` | `Vec<String>` | `["duckduckgo", "startpage", "yahoo"]` | Providers queried when the caller names none |
| `providers` | `BTreeMap<String, bool>` | 8 entries (see below) | Per-provider enable/disable flags |
| `searxng` | `SearxngConfig` | disabled | Self-hosted SearXNG upstream (`enabled`, `base_url`) |
| `api` | `BTreeMap<String, ApiProviderConfig>` | empty | API-key providers (`enabled`, `api_key_env`, `base_url`) |
| `live` | `LiveConfig` | empty | Reserved NO-OPs (`user_agent`, `respect_robots_txt`); setting them logs a startup warning only |
| `sanitize_output` | `bool` | `true` | Tier 2 framing + Tier 3 marker scan on result text |
| `profiles` | `BTreeMap<String, ProfileConfig>` | empty | Named profiles; each holds an ordered `providers` list |
| `exact_error` | `ExactErrorConfig` | defaults | Compiler/runtime error search mode |
| `multiquery_concurrency` | `usize` | `8` | Global in-flight `(subquery, provider)` dispatch cap |
| `multiquery_provider_concurrency` | `usize` | `2` | Per-provider in-flight dispatch cap |

Default `providers` map: `duckduckgo = true`, `brave = true`,
`startpage = true`, `yahoo = true`, `mojeek = false`,
`searxng = false`, `osv = true`, `firecrawl_developer = false`.

`Mode::parse` accepts only `"off"` and `"live"`. Former aliases
(`"ask"`, `"local"`, `"local_only"`, `"localonly"`) are rejected.

### `[fetch]` — `FetchSection`

| Field | Type | Default | Purpose |
|-------|------|---------|---------|
| `enabled` | `bool` | `true` | Whether fetch tools are available |
| `timeout_ms` | `u64` | `8000` | Request timeout |
| `max_bytes` | `usize` | `2_000_000` | Hard response body cap |
| `max_chars_default` | `usize` | `12_000` | Default extraction limit |
| `max_chars_cap` | `usize` | `50_000` | Hard extraction upper bound |
| `redirect_limit` | `usize` | `5` | Redirect chain limit |
| `allow_private_network` | `bool` | `false` | SSRF escape hatch: RFC 1918 / CGNAT / link-local |
| `allow_localhost` | `bool` | `false` | SSRF escape hatch: loopback, independent of the above |
| `include_links_default` | `bool` | `false` | Default link inclusion |
| `user_agent` | `String` | `eggsearch/<version>` | HTTP user agent |
| `sanitize_output` | `bool` | `true` | Tier 2 framing + Tier 3 marker scan on fetched text |
| `pdf_enabled` | `bool` | `false` | PDF extraction (requires the `pdf` feature) |
| `pdf_max_pages` | `usize` | `25` | PDF page budget |
| `pdf_max_chars_per_page` | `usize` | `12_000` | Per-page extraction cap |
| `pdf_max_total_chars` | `usize` | `50_000` | Total PDF extraction budget |
| `batch_max_items` | `usize` | `8` | Default batch item count |
| `batch_max_items_cap` | `usize` | `20` | Hard batch item cap |
| `batch_max_chars_per_item` | `usize` | `12_000` | Per-item char cap |
| `batch_max_total_chars` | `usize` | `50_000` | Total batch char budget |
| `batch_max_total_chars_cap` | `usize` | `120_000` | Hard batch char cap |
| `batch_concurrency` | `usize` | `4` | Concurrent batch fetches |
| `retry_max_attempts` | `usize` | `2` | Automatic retries per request (1 = none) |
| `retry_base_delay_ms` | `u64` | `250` | Backoff base |
| `retry_max_delay_ms` | `u64` | `4000` | Backoff ceiling |
| `origin_http_concurrency` | `usize` | `2` | Concurrent HTTP requests per origin |
| `origin_browser_concurrency` | `usize` | `1` | Concurrent browser requests per origin |
| `origin_circuit_failure_threshold` | `u8` | `3` | Retryable failures before the circuit opens |
| `origin_circuit_duration_ms` | `u64` | `60_000` | Circuit open duration |
| `cache` | `FetchCacheSection` | see below | In-memory fetch cache |
| `browser` | `FetchBrowserSection` | disabled | Headless rendering escalation |

`[fetch.cache]`: `enabled = true`, `memory_max_entries = 256`,
`memory_max_bytes = 67_108_864` (64 MB raw bodies),
`derived_max_entries = 512`, `derived_max_bytes = 67_108_864`
(64 MB extracted documents, independent budget),
`default_ttl_seconds = 900`.

`[fetch.browser]`: `enabled = false`, `policy = "http_only"`,
`executable = None` (auto-discovered; an explicit invalid path fails
deterministically with `ExplicitPathInvalid`, never falls back),
`startup_timeout_ms = 10_000`, `navigation_timeout_ms = 20_000`,
`post_load_wait_ms = 1_500`, `verification_wait_ms = 10_000`,
`max_requests = 100`, `max_dom_bytes = 4_000_000`,
`global_concurrency = 1`, `per_origin_concurrency = 1`,
`block_media = true`, plus `[fetch.browser.persistent_profiles]`:
`enabled = false`, `profiles_dir = None` (platform default),
`allowed_profiles = []` (empty allows all),
`profile_process_timeout_ms = 30_000` (hard ceiling
`MAX_PROFILE_PROCESS_TIMEOUT_MS = 120_000`).

### `[local]` — `LocalConfig`

Disabled by default (`enabled = false`, `roots = []`).

| Field | Default | Purpose |
|-------|---------|---------|
| `enabled` | `false` | Workspace search master switch |
| `roots` | `[]` | Filesystem roots, canonicalized at startup |
| `max_file_bytes` | `1_048_576` | Files larger than this are skipped |
| `max_indexed_files` | `50_000` | Bounded scan per search |
| `include_hidden` | `false` | Dotfile inclusion |
| `respect_gitignore` | `true` | `.gitignore` filtering |
| `follow_symlinks` | `false` | Symlink traversal |
| `structured_symbols` | `true` | Deterministic parser; `false` forces regex fallback |
| `max_parse_bytes` | `262_144` | Parser input cap per file |
| `max_symbols_per_file` | `256` | Retained symbols per file |
| `max_structured_files` | `200` | Structured parses per request |
| `max_total_symbols` | `5_000` | Retained symbols per request |
| `repo_map_structure_cap` | `500` | Total repo-map structural entries |

### `[egress]` — `EgressSection`

Listener-free outbound proxy-chain routing for fixed provider
upstreams. Disabled by default (`enabled = false`, `hops = []`).

Each `[[egress.hops]]` entry (`EgressHopConfig`) carries `scheme`
(`http`, `socks4`, `socks5`; `httponly` accepted as an HTTP variant),
`host` (bare hostname, IPv4 literal, or bare IPv6 literal such as
`::1` — schemes, userinfo, paths, brackets, and appended ports are
rejected; `port` is the sole port source), `port` (non-zero),
optional `username`, and optional `password_env` naming the credential
environment variable. Raw passwords are never a config field, and
`password_env` requires a non-empty `username`. An enabled route on a
binary built without the `egress` feature fails validation instead of
being silently ignored.

---

## Provider Resolution

### Provider Kinds

- **Built-in providers** (`providers` map, `KNOWN_PROVIDER_IDS`, 37
  ids): keyless HTML/JSON engines plus `searxng` and
  `local_workspace`, toggled by boolean flags.
- **Required-key API providers** (`api` map, `API_PROVIDER_IDS`, 15
  ids: `brave_api`, `github_code`, `github_issues`, `github_releases`,
  `gitlab_code`, `gitlab_issues`, `gitlab_releases`, `gitea_code`,
  `gitea_issues`, `gitea_releases`, `github_advisory`,
  `semantic_scholar`, `sourcegraph`, `exa`, `tavily`): each entry sets
  `enabled`, `api_key_env` (env var name, never the secret), and
  optional `base_url`. `api_provider_is_configured()` requires
  enabled + known + non-empty `api_key_env` + present non-empty env value.
- **Keyless-optional providers** (`OPTIONAL_API_PROVIDER_IDS`:
  `firecrawl_developer` only): `optional_api_key()` returns `Some(key)`
  only when the `api` entry is enabled with a resolvable non-empty
  value; every other state falls back keyless.
  `optional_api_key_misconfigured()` flags enabled-but-unusable entries
  so startup can warn precisely without failing or emitting
  `missing_api_key`.
- **SearXNG**: routable only when `providers.searxng = true` **and**
  `searxng.enabled = true` **and** `base_url` is non-empty.
- `credential_requirement()` classifies each id as `None`, `Optional`,
  or `Required`; `provider_configured_state()` applies the
  readiness gate per kind before the generic `enabled` check.

### Resolution Algorithm

`resolve_providers(override_list)`:

1. Empty overrides use `default_providers` filtered to effectively
   enabled ids; an empty result errors (`no default providers are
   enabled`).
2. Non-empty overrides dedupe preserving order, reject unknown ids
   (not in `KNOWN_PROVIDER_IDS` nor the configured `providers`/`api`
   maps), then reject disabled-or-unavailable ids.
3. Explicit `providers[id] = true` alone never enables a required-key
   API provider; only a configured `api` entry does.

`effective_provider_ids()` unions enabled built-ins (excluding
`API_PROVIDER_IDS`) with configured `api` entries.
`provider_is_available(id)` applies the per-kind gate above.
`misconfigured_default_providers()` lists `default_providers` entries
that would be silently filtered, for startup diagnostics.

### Profiles Are Advisory

`resolve_profile_providers()` implements a three-tier fallback:
explicit providers win exactly; otherwise the configured profile (or
built-in defaults) is filtered to available ids with per-id
`profile_provider_unknown` / `profile_provider_unavailable` warnings;
an empty result degrades to `default_providers` with a
`profile_degraded` warning. Built-in defaults: `generic` is empty
(falls through to `default_providers`); `coding` lists forge code /
issues / releases adapters plus `brave_api`, `searxng`,
`duckduckgo`, `startpage`; `security` lists `osv`,
`github_issues`, `brave_api`, `duckduckgo`, `startpage`; `research`
lists `brave_api`, `searxng`, `duckduckgo`, `startpage`, `mojeek`.
Profiles never require API keys; missing credentials are
provider-scoped skips, never request failures.

---

## Validation Rules

`AppConfig::validate()` fails fast on the first violated invariant:

- `[fetch]`: `max_chars_cap >= max_chars_default`; `max_bytes`,
  `timeout_ms`, `batch_max_items`, `batch_max_items_cap`,
  `batch_max_total_chars`, `batch_concurrency`, `retry_max_attempts`,
  `origin_http_concurrency`, `origin_browser_concurrency`,
  `origin_circuit_failure_threshold`, `origin_circuit_duration_ms`,
  cache entry/byte budgets, browser timeouts/request/DOM caps, and
  `profile_process_timeout_ms` (non-zero, within the 120 s ceiling)
  are all `> 0`; `batch_max_items_cap >= batch_max_items`;
  `batch_max_total_chars_cap >= batch_max_total_chars`;
  `retry_max_delay_ms >= retry_base_delay_ms`.
- `[search]`: `default_max_results > 0`; `max_results_cap > 0` and
  `>= default_max_results`; `timeout_ms > 0`; `max_query_chars > 0`;
  every `default_providers` entry and every `providers` map key must be
  a known provider id.
- SearXNG: when enabled, `base_url` must be a valid `http`/`https` URL
  with a host.
- API providers: unknown `api` ids log a forward-compatibility warning;
  enabled required-key entries with missing/empty `api_key_env` fail;
  enabled optional-key entries with missing/empty credentials warn and
  continue keyless; missing env values warn (the provider fails at
  runtime, not at load); `base_url`, when set, must be valid
  `http`/`https` with a host.
- `[local]`: when enabled, at least one root must exist and be a
  directory; all byte/symbol caps must be `> 0`.
- Live mode: at least one traditional provider must be enabled or one
  API provider configured with resolvable credentials; `off` mode
  permits an empty provider set.
- `[egress]`: validated per the hop rules above, including the
  `egress`-feature build gate.

---

## Keyless Healthy Default

`AppConfig::default()` with no file and no credential env vars must
produce a useful server: `duckduckgo`, `brave`, `startpage`, `yahoo`,
and `osv` route keyless. CI enforces this by blanking all credential
env vars; missing credentials surface as provider-scoped skips, never
global failures.

---

## `sanitize_output`: Production `true`, Tests `false`

Both `[search].sanitize_output` and `[fetch].sanitize_output` default
to `true`. Tier 1 (control-char stripping + length bounding) is always
on; the flag gates Tier 2 (`<<<EXTERNAL_UNTRUSTED ...>>>` framing) and
Tier 3 (injection-marker scan). Production `ServerState` construction
threads the configured values into the metadata adapter and the
`FetchClient`. Test harnesses deliberately set `false` (via
`from_engines_with_sanitize(..., false)` or direct field assignment)
to assert on raw untransformed content; `from_engines` itself defaults
to `false`, so tests opt out explicitly while shipped configs sanitize.

---

## Structured Symbol Budgets

`[local]` structured intelligence is dependency-free and deterministic.
The budgets that keep it bounded: `max_parse_bytes` (parser input per
file), `max_symbols_per_file`, `max_structured_files` (parses per
request), `max_total_symbols` (retained symbols per request), and
`repo_map_structure_cap` (total structural entries for `packages`,
`language_distribution`, `modules`, `entrypoints`, `top_symbols`,
`test_relationships`, `build_configs`). Budget breach degrades to
partial/regex evidence with telemetry (`structured_files_parsed`,
`structured_symbols_found`, `regex_fallback_files`,
`symbol_budget_breaches`), never to failure. `structured_symbols =
false` forces the regex backend everywhere.

---

**Back to:** [overview.md](overview.md)
