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
default_providers = ["duckduckgo", "startpage", "yahoo"]
sanitize_output = true

[search.providers]
duckduckgo = true
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

## Provider Requirements

All 36 built-in providers:

| Provider | Kind | Requires | Notes |
|----------|------|----------|-------|
| `duckduckgo` | html_scrape | — | Default generic provider |
| `brave` | html_scrape | — | |
| `startpage` | html_scrape | — | Default generic provider |
| `yahoo` | html_scrape | — | Default generic provider |
| `mojeek` | html_scrape | — | |
| `searxng` | json_api | `base_url` | Requires `[search.searxng]` config |
| `brave_api` | api_key | `BRAVE_API_KEY` | |
| `exa` | api_key | `EXA_API_KEY` | Exa semantic search (native freshness/domain filters, highlights on excerpt demand) |
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
| `local_workspace` | local | — | Requires `[local]` config |

`provider_status` returns `routable: true` only when a provider is both enabled and fully configured. Non-routable providers include a `skip_reason` explaining why (e.g. "API key not configured", "SearXNG base_url not configured") and a machine-readable `skip_code` (e.g. `missing_api_key`, `missing_searxng_config`, `disabled_by_user`, `cooldown_active`).

## Search Defaults

The shipped generic search defaults require no API keys:

- `duckduckgo`
- `startpage`
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

Profiles are advisory. The built-in profiles are `generic`, `coding`, `security`, and `research`. eggsearch skips unavailable providers and reports warnings instead of failing the request.

`repo_search` also supports `mode = "exact_error"` for literal compiler/runtime/toolchain error text.

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
default_providers = ["duckduckgo", "startpage", "yahoo"]
sanitize_output = true

[search.providers]
duckduckgo = true
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
default_providers = ["duckduckgo", "startpage", "yahoo"]

[search.providers]
duckduckgo = true
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
default_providers = ["osv", "duckduckgo", "startpage"]

[search.providers]
duckduckgo = true
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
default_providers = ["duckduckgo", "startpage"]

[search.providers]
duckduckgo = true
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
default_providers = ["github_code", "github_issues", "github_releases", "duckduckgo", "startpage"]

[search.providers]
duckduckgo = true
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
default_providers = ["osv", "duckduckgo", "startpage"]

[search.providers]
duckduckgo = true
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
default_providers = ["brave_api", "searxng", "duckduckgo", "startpage", "mojeek"]

[search.providers]
duckduckgo = true
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
```

When enabled, the local workspace inventory is built automatically on first search (auto-build on cache miss). The git fast path uses `git ls-files -z --cached --others --exclude-standard` with a bounded command runner (5s timeout, 16MB stdout / 64KB stderr caps, concurrent pipe drainage, cap breaches trigger immediate process termination). A `git status --porcelain=v2` hash is stored alongside the inventory for change detection between builds. Native directory walking is the fallback for non-git directories.

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
| `max_chars_cap` | `50_000` |
| `redirect_limit` | `5` |
| `allow_private_network` | `false` |
| `allow_localhost` | `false` |
| `include_links_default` | `false` |
| `sanitize_output` | `true` |
| `pdf_enabled` | `false` |
| `pdf_max_pages` | `25` |
| `pdf_max_chars_per_page` | `12_000` |
| `pdf_max_total_chars` | `50_000` |

The fetch side is bounded by both byte and character caps. The byte cap is a hard response limit; the character cap is an extraction limit.

`allow_private_network = true` and `allow_localhost = true` are operator escape hatches. Keep them off for general MCP exposure.

When `allow_private_network = false` (the default), eggsearch blocks fetches to all RFC 1918 private ranges (`10.0.0.0/8`, `172.16.0.0/12`, `192.168.0.0/16`), carrier-grade NAT (`100.64.0.0/10`), link-local (`169.254.0.0/16`), multicast, reserved, and documentation addresses. When `allow_localhost = false` (the default), loopback addresses (`127.0.0.0/8`, `::1`) are blocked independently. The two flags are fully independent. Redirect targets are validated against these same ranges. See [safety.md](safety.md#blocked-address-ranges) for the full policy table.

## Reserved Live Settings

`[search].live.user_agent` and `[search].live.respect_robots_txt` are parsed for compatibility but are no-ops in the current build.

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

When enabled, local results use `local_trusted` trust labels and can be surfaced through `repo_search`, `repo_fetch`, and `repo_map`. They remain provenance-trusted, not instruction-trusted.

## Browser Rendering

The `[fetch.browser]` section configures optional headless Chrome/Chromium rendering. Browser rendering is disabled by default and requires the `browser` Cargo feature.

| Setting | Default | Description |
|---------|---------|-------------|
| `enabled` | `false` | Enable browser rendering escalation |
| `policy` | `"http_only"` | Render policy: `http_only`, `auto`, or `browser` |
| `executable` | auto-discovered | Path to Chrome/Chromium executable. An explicitly configured invalid path fails deterministically — it does not fall back to auto-discovery. |
| `startup_timeout_ms` | `10000` | Browser startup timeout |
| `navigation_timeout_ms` | `20000` | Page navigation timeout |
| `post_load_wait_ms` | `1500` | Wait after page load |
| `verification_wait_ms` | `10000` | Wait for non-interactive verification |
| `max_requests` | `100` | Maximum requests per browser session |
| `max_dom_bytes` | `4000000` | Maximum DOM size |
| `global_concurrency` | `1` | Global browser concurrency |
| `per_origin_concurrency` | `1` | Per-origin browser concurrency |
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

`provider_status` reports configuration-derived provider descriptors, code-host summaries, cached health snapshots, server capabilities, tool capabilities, quality metadata, and workflow recipes. The `probe` field is reserved and currently has no effect.

Each provider descriptor includes:

- `id`, `display_name`, `kind` — identity
- `enabled`, `default`, `configured` — config state
- `requires_api_key` — whether an API key env var is needed
- `routable` — whether the provider can actually be queried right now
- `skip_reason` — human-readable explanation when `routable` is `false`
- `skip_code` — machine-readable skip code when `routable` is `false`
- `capabilities` — feature flags (code search, freshness, etc.)

Skip codes are stable snake_case strings: `unknown_provider`, `disabled_by_user`, `missing_api_key`, `missing_searxng_config`, `missing_base_url`, `invalid_base_url`, `missing_local_backend`, `credential_not_configured`, `credential_env_missing`, `credential_invalid`, `cooldown_active`, `not_built`, `unknown`.

Use `provider_status` to decide whether to call specialized tools or fall back to generic search.
