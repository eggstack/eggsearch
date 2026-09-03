# Configuration Deep Dive

**Path:** `src/core/config.rs` + `src/config.rs`
**Purpose:** TOML configuration model, provider resolution, validation, and CLI config loading.

---

## Configuration Model

```
AppConfig
  ├── [search]     — SearchSection
  ├── [fetch]      — FetchSection
  └── [local]      — LocalConfig
```

### SearchSection

| Field | Type | Default | Purpose |
|-------|------|---------|---------|
| `mode` | `Mode` | `Live` | Server operating mode (`off` or `live`) |
| `default_max_results` | `usize` | `10` | Default results when not specified by client |
| `max_results_cap` | `usize` | `50` | Hard cap on `max_results` |
| `max_query_chars` | `usize` | `512` | Maximum query length |
| `timeout_ms` | `u64` | `8000` | Per-request timeout |
| `default_providers` | `Vec<String>` | `["duckduckgo", "startpage", "yahoo"]` | Default provider list |
| `providers` | `BTreeMap<String, bool>` | 8 built-in providers (incl. `firecrawl_developer=false`) | Per-provider enable/disable |
| `searxng` | `SearxngConfig` | disabled | SearXNG upstream adapter |
| `api` | `BTreeMap<String, ApiProviderConfig>` | empty | API-key backed providers (required-key plus keyless-optional `firecrawl_developer`) |
| `sanitize_output` | `bool` | `true` | Tier 2/3 sanitization toggle |
| `profiles` | `BTreeMap<String, ProfileConfig>` | empty | Named search profiles |
| `exact_error` | `ExactErrorConfig` | defaults | Compiler error search mode |
| `multiquery_concurrency` | `usize` | `8` | Global dispatch concurrency cap |
| `multiquery_provider_concurrency` | `usize` | `2` | Per-provider concurrency cap |

### FetchSection

| Field | Type | Default | Purpose |
|-------|------|---------|---------|
| `enabled` | `bool` | `true` | Whether fetch is available |
| `timeout_ms` | `u64` | `8000` | Request timeout |
| `max_bytes` | `usize` | `2,000,000` | Response body size cap |
| `max_chars_default` | `usize` | `12,000` | Default extraction limit |
| `max_chars_cap` | `usize` | `50,000` | Hard extraction upper bound |
| `redirect_limit` | `usize` | `5` | Redirect chain limit |
| `allow_private_network` | `bool` | `false` | SSRF: allow RFC 1918 |
| `allow_localhost` | `bool` | `false` | SSRF: allow loopback |
| `sanitize_output` | `bool` | `true` | Tier 2/3 sanitization |
| `pdf_enabled` | `bool` | `false` | PDF extraction (requires `pdf` feature) |
| `batch_max_items` | `usize` | `8` | Default batch item count |
| `batch_max_items_cap` | `usize` | `20` | Hard batch item cap |
| `batch_max_chars_per_item` | `usize` | `12,000` | Per-item char cap |
| `batch_max_total_chars` | `usize` | `50,000` | Total batch char budget |
| `batch_max_total_chars_cap` | `usize` | `120,000` | Hard batch char cap |
| `batch_concurrency` | `usize` | `4` | Concurrent batch fetches |

---

## Provider Resolution

### Provider Kinds

**Built-in providers** (`providers` map): `duckduckgo`, `brave`, `startpage`, `yahoo`, `mojeek`, `osv`, `firecrawl_developer` (disabled by default) — configured via enable/disable flags. `firecrawl_developer = true` alone routes keyless.

**API providers** (`api` map): `brave_api`, `exa`, `github_code`, `gitlab_code`, `gitea_code`, `sourcegraph`, `semantic_scholar`, `github_advisory`, `osv`, `nvd`, `rustsec`, `cisa_kev` — configured with `enabled`, `api_key_env`, and optional `base_url`. `exa` uses `EXA_API_KEY` and defaults to `https://api.exa.ai/search`.

**Keyless-optional providers** (`api` map, optional): `firecrawl_developer` — `[search.api.firecrawl_developer]` may name `FIRECRAWL_API_KEY`; when enabled with a resolvable non-empty value the bearer header is attached, otherwise the provider falls back keyless with a startup warning (never `missing_api_key`).

**SearXNG**: Requires both `searxng.enabled = true` and `providers.searxng = true` plus a non-empty `base_url`.

### Resolution Algorithm

```
resolve_providers(override_list):
  1. If override_list is empty → use default_providers, filter to enabled
  2. If override_list is non-empty:
     a. Deduplicate while preserving order
     b. Reject unknown provider IDs
     c. Reject disabled providers
  3. If resolved is empty → return error

effective_provider_ids():
  1. Collect all enabled built-in providers (excluding required-key `API_PROVIDER_IDS`; keyless-optional providers stay here)
  2. For each required API provider: check enabled, known, api_key_env set, env var present and non-empty
  3. Return union

provider_is_available(id):
  - Required API providers: enabled + known + api_key_env present + env var set and non-empty
  - Keyless-optional (`firecrawl_developer`): `providers[id] == true` (api entry only upgrades to keyed)
  - SearXNG: providers.searxng + searxng.enabled + base_url non-empty
  - Built-in: providers[id] == true
```

### Profile-Based Resolution

`resolve_profile_providers()` implements a three-tier fallback:

1. **Explicit providers** → validate and use exactly
2. **Profile providers** → from configured profile or built-in defaults
3. **Default providers** → fallback when profile has no available providers

Built-in profile defaults:
- **generic** → (empty, falls through to `default_providers`)
- **coding** → GitHub/GitLab/Gitea code/issues/releases + Brave API + SearXNG + DuckDuckGo + Startpage
- **security** → OSV + GitHub Issues + Brave API + DuckDuckGo + Startpage
- **research** → Brave API + SearXNG + DuckDuckGo + Startpage + Mojeek

---

## Validation

`AppConfig::validate()` checks invariants:

- `max_chars_cap >= max_chars_default`
- `max_bytes > 0`, `timeout_ms > 0`
- `default_max_results > 0`, `max_results_cap >= default_max_results`
- `max_query_chars > 0`
- All `default_providers` are known provider IDs
- All `providers` map keys are known provider IDs
- SearXNG: if enabled, `base_url` must be valid HTTP/HTTPS with a host
- API providers: if enabled, `api_key_env` must be non-empty; if env var missing/empty, warns at startup. Keyless-optional `firecrawl_developer` never errors on missing/empty optional credentials; it warns and continues keyless.
- Batch fetch: `batch_max_items > 0`, `batch_max_items_cap >= batch_max_items`, total chars caps validated
- Local: if enabled, roots must exist and be directories, `max_file_bytes > 0`, `max_indexed_files > 0`
- Live mode: at least one provider must be enabled or an API provider configured

---

## Config Loading

### CLI Config (`src/config.rs`)

Thin 14-line delegation module. `load(Option<&Path>)` resolves to `AppConfig::load()` with either a user-supplied path or `default_config_path()`.

### Default Config Path

```
$XDG_CONFIG_HOME/eggsearch/config.toml     (Linux)
~/Library/Application Support/eggsearch/config.toml  (macOS)
%APPDATA%/eggsearch/config.toml              (Windows)
eggsearch.toml                               (fallback)
```

If the file doesn't exist, `AppConfig::default()` is returned silently.

---

## Keyless Core Invariant

No config file and no credential environment variables must produce a healthy, useful server. Missing optional credentials are provider-scoped skips, never global failures.

---

**Back to:** [overview.md](overview.md)
