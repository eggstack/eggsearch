# Provider Setup

eggsearch supports 34 search providers across six categories: web search (HTML scrapers), API-key providers, aggregators, security advisory databases, package registries, scholarly search, and special-purpose providers. Providers can be enabled individually in config and selected per-request or via `default_providers`.

## Provider Categories at a Glance

| Category | Examples | User Requirement |
|----------|----------|-----------------|
| **Keyless defaults** | DuckDuckGo, Startpage, Yahoo | none |
| **Keyless specialist** | OSV, NVD, CISA KEV, RustSec, OpenAlex, Crossref, all package registries | none |
| **Optional configured endpoint** | SearXNG, self-hosted forge base URL | operator configuration |
| **Optional credentialed** | GitHub/GitLab/Gitea code search, Sourcegraph, Brave API, Semantic Scholar, GitHub Advisory | opt-in credential |
| **Optional local** | local workspace | configured local root |

All credentialed providers are disabled or non-routable unless explicitly configured. Missing optional credentials produce provider-scoped skip telemetry and never make the server globally unhealthy.

## Web Search Providers

These providers require no API key and work via HTML scraping.

### DuckDuckGo (default)

- ID: `duckduckgo`
- Enabled by default: yes
- Included in `default_providers`: yes
- No configuration needed
- Rate limits: moderate; provider enters cooldown after repeated failures and recovers automatically

### Brave Search (HTML)

- ID: `brave`
- Enabled by default: yes
- Included in `default_providers`: no
- Enable: set `brave = true` in `[search.providers]` (already true by default)
- No API key required

### Startpage (default)

- ID: `startpage`
- Enabled by default: yes
- Included in `default_providers`: yes
- No configuration needed

### Yahoo (default)

- ID: `yahoo`
- Enabled by default: yes
- Included in `default_providers`: yes
- No configuration needed

### Mojeek

- ID: `mojeek`
- Enabled by default: **no**
- Independent search engine with its own index
- Enable: set `mojeek = true` in `[search.providers]`

## API-Key Providers

These providers require an API key stored in an environment variable. All are disabled by default.

### Brave Search API

- ID: `brave_api`
- Enable and configure in `[search.api.brave_api]`:

```toml
[search.api.brave_api]
enabled = true
api_key_env = "BRAVE_API_KEY"
```

The environment variable `BRAVE_API_KEY` must be set at runtime.

### GitHub Code Search

- ID: `github_code`
- Enable in `[search.api.github_code]`:

```toml
[search.api.github_code]
enabled = true
api_key_env = "GITHUB_TOKEN"
```

### GitHub Issues Search

- ID: `github_issues`
- Enable in `[search.api.github_issues]`:

```toml
[search.api.github_issues]
enabled = true
api_key_env = "GITHUB_TOKEN"
```

### GitHub Releases Search

- ID: `github_releases`
- Enable in `[search.api.github_releases]`:

```toml
[search.api.github_releases]
enabled = true
api_key_env = "GITHUB_TOKEN"
```

All three GitHub providers share the same `GITHUB_TOKEN` environment variable.

### GitLab Code Search

- ID: `gitlab_code`
- Enable in `[search.api.gitlab_code]`:

```toml
[search.api.gitlab_code]
enabled = true
api_key_env = "GITLAB_TOKEN"
base_url = "https://gitlab.com"
```

### GitLab Issues Search

- ID: `gitlab_issues`
- Enable in `[search.api.gitlab_issues]`:

```toml
[search.api.gitlab_issues]
enabled = true
api_key_env = "GITLAB_TOKEN"
base_url = "https://gitlab.com"
```

### GitLab Releases Search

- ID: `gitlab_releases`
- Enable in `[search.api.gitlab_releases]`:

```toml
[search.api.gitlab_releases]
enabled = true
api_key_env = "GITLAB_TOKEN"
base_url = "https://gitlab.com"
```

For self-hosted GitLab, set `base_url` to your instance URL instead of `https://gitlab.com`.

### Gitea Code Search

- ID: `gitea_code`
- Enable in `[search.api.gitea_code]`:

```toml
[search.api.gitea_code]
enabled = true
api_key_env = "FORGEJO_TOKEN"
base_url = "https://git.example.com"
```

### Gitea Issues Search

- ID: `gitea_issues`
- Enable in `[search.api.gitea_issues]`:

```toml
[search.api.gitea_issues]
enabled = true
api_key_env = "FORGEJO_TOKEN"
base_url = "https://git.example.com"
```

### Gitea Releases Search

- ID: `gitea_releases`
- Enable in `[search.api.gitea_releases]`:

```toml
[search.api.gitea_releases]
enabled = true
api_key_env = "FORGEJO_TOKEN"
base_url = "https://git.example.com"
```

Gitea/Forgejo providers require a `base_url` pointing to your Gitea or Forgejo instance. The token is typically named `FORGEJO_TOKEN` but any environment variable name works.

**Never commit real keys.** Always use env-var indirection.

## Aggregator Providers

### SearXNG

- ID: `searxng`
- Requires a self-hosted SearXNG instance
- Enable in both `[search.providers]` and `[search.searxng]`:

```toml
[search.providers]
searxng = true

[search.searxng]
enabled = true
base_url = "https://searx.example.org"
```

Both flags must be set and `base_url` must be non-empty for the SearXNG provider to activate. The engine appends `/search` to the configured `base_url`.

## Special Providers

### OSV

- ID: `osv`
- Open Source Vulnerability database
- Enabled by default: yes
- No API key needed
- Used primarily by `security_search` for vulnerability lookups

## Security Advisory Providers

### GitHub Security Advisories

- ID: `github_advisory`
- Requires `GITHUB_TOKEN`
- Enable in `[search.api.github_advisory]`:

```toml
[search.api.github_advisory]
enabled = true
api_key_env = "GITHUB_TOKEN"
```

### NIST National Vulnerability Database

- ID: `nvd`
- No API key needed
- Enabled by default: yes
- Advisory lookup by CVE ID

### CISA Known Exploited Vulnerabilities

- ID: `cisa_kev`
- No API key needed
- Enabled by default: yes
- KEV catalog for exploit status checks

### RustSec Advisory Database

- ID: `rustsec`
- No API key needed
- Enabled by default: yes
- Rust-specific security advisories

## Package Registry Providers

All package registry providers use JSON APIs and require no API key. They provide package metadata, version history, and structured changelogs.

### crates.io

- ID: `crates_io`
- Rust package metadata from crates.io

### PyPI

- ID: `pypi`
- Python package metadata from PyPI

### npm

- ID: `npm_registry`
- Node.js package metadata from npm

### Go Proxy

- ID: `go_pkg`
- Go module metadata from the Go module proxy

### Maven Central

- ID: `maven_central`
- Java/JVM artifact metadata from Maven Central

### NuGet

- ID: `nuget`
- .NET package metadata from NuGet

### RubyGems

- ID: `rubygems`
- Ruby gem metadata from RubyGems

### Packagist

- ID: `packagist`
- PHP package metadata from Packagist

## Scholarly Search Providers

### OpenAlex

- ID: `openalex`
- No API key needed
- Open-access scholarly literature search with DOI lookup

### Crossref

- ID: `crossref`
- No API key needed
- Scholarly literature search with DOI lookup

### Semantic Scholar

- ID: `semantic_scholar`
- Requires `SEMANTIC_SCHOLAR_API_KEY`
- Enable in `[search.api.semantic_scholar]`:

```toml
[search.api.semantic_scholar]
enabled = true
api_key_env = "SEMANTIC_SCHOLAR_API_KEY"
```

## Code Search Providers

### Sourcegraph

- ID: `sourcegraph`
- Requires `SOURCEGRAPH_API_KEY`
- Enable in `[search.api.sourcegraph]`:

```toml
[search.api.sourcegraph]
enabled = true
api_key_env = "SOURCEGRAPH_API_KEY"
```

### Local Workspace

- ID: `local_workspace`
- Indexes local files under configured roots
- Requires the `[local]` section in config:

```toml
[local]
enabled = true
roots = ["/Users/you/projects"]
max_file_bytes = 1048576
max_indexed_files = 50000
include_hidden = false
respect_gitignore = true
follow_symlinks = false
```

Local results appear in `repo_search` and are fetched via `repo_fetch` with `host = "workspace"`.

## Provider Selection

### Default Providers

The `default_providers` list controls which providers are queried when a tool call does not specify explicit providers:

```toml
[search]
default_providers = ["duckduckgo", "startpage", "yahoo"]
```

When a default provider is unavailable (cooldown, misconfigured API key, disabled), it is skipped with a warning. The search still runs with the remaining providers.

### Per-Request Override

Any search tool (`web_search`, `repo_search`, `security_search`, `research_search`) accepts an optional `providers` field to override the default list for that request.

### Profiles

Search profiles (`generic`, `coding`, `security`, `research`) have their own provider ordering. When a profile is used and its providers are unavailable, eggsearch falls back to `default_providers`. A `profile_degraded` warning is emitted in this case.

### Misconfigured Defaults

If a provider listed in `default_providers` is disabled or lacks a valid API key, eggsearch emits a startup warning. Run `eggsearch doctor` to diagnose configuration issues. Each skipped provider includes a `skip_code` (e.g. `missing_api_key`, `disabled_by_user`, `missing_searxng_config`) for machine-readable diagnostics alongside the human-readable `skip_reason`.

## Skip Codes

`provider_status` returns a `skip_code` for every non-routable provider. These are stable snake_case strings for machine-readable diagnostics.

| Code | Display Name | Meaning | Cause | Fix | Retry? |
|------|-------------|---------|-------|-----|--------|
| `unknown_provider` | Unknown provider | Provider ID not in the built-in inventory | Typo in config or referencing a removed provider | Correct the provider ID in config | No |
| `disabled_by_user` | Disabled by user | Provider is explicitly disabled in config | `provider = false` in `[search.providers]` | Set to `true` or remove from config | No |
| `missing_api_key` | Missing API key | API-key provider has no key configured | Missing `[search.api.<id>]` section or env var not set | Add `[search.api.<id>]` with `enabled = true` and `api_key_env`, then set the env var | No |
| `missing_searxng_config` | SearXNG not configured | SearXNG provider missing `base_url` or not enabled | Missing `[search.searxng]` or `searxng = false` in `[search.providers]` | Enable in both `[search.providers]` and `[search.searxng]` with `base_url` | No |
| `missing_base_url` | Missing base URL | Provider requires a base URL that is not set | Missing `base_url` in `[search.api.<id>]` (Gitea, GitLab self-hosted) | Add `base_url` to the provider's API config section | No |
| `invalid_base_url` | Invalid base URL | Base URL is malformed or unreachable | Typo or incorrect URL in `base_url` | Correct the URL; must be valid HTTP(S) | No |
| `missing_local_backend` | Local backend not available | `local_workspace` provider has no backend | `[local]` section missing or `enabled = false` | Add `[local]` with `enabled = true` and `roots` | No |
| `credential_not_configured` | Credential not configured | Credential entry exists but is not fully configured | Incomplete `[search.api.<id>]` section | Complete the API config section | No |
| `credential_env_missing` | Credential environment variable not set | `api_key_env` is set but the env var is not present at runtime | Environment variable not exported in the shell | Export the env var or add it to your shell profile | No |
| `credential_invalid` | Credential invalid (empty) | Environment variable is set but empty | Env var exported with empty value | Set the env var to a valid value | No |
| `cooldown_active` | Cooldown active | Provider is temporarily suppressed after repeated failures | 3+ consecutive failures (rate limit, timeout, network error) | Wait for cooldown to expire (15–60s depending on failure class) | **Yes** — auto-recovers |
| `not_built` | Not built | Provider was excluded at compile time | Feature-gated or compiled out of the binary | Rebuild with the required feature flag | No |
| `unknown` | Unknown | Catch-all for unrecognized skip conditions | Edge case or internal error | Run `eggsearch doctor` for details | No |

## Provider Health

eggsearch tracks per-provider health state using a built-in health registry. Health state is exposed in `provider_status` output via `health_views` (per-provider compact view) and `health` (full snapshots).

### Health States

| State | Meaning |
|-------|---------|
| `Healthy` | Provider has recorded at least one success, no active failures |
| `Degraded` | Provider has consecutive failures but is not yet in cooldown |
| `Cooldown` | Provider is temporarily suppressed; will auto-recover |
| `Unknown` | No health data recorded yet (fresh start or provider never queried) |

### Cooldown Behavior

After 3 consecutive failures (`COOLDOWN_THRESHOLD = 3`), a provider enters cooldown:

| Failure Class | Cooldown Duration | Recovery Trigger |
|---------------|-------------------|------------------|
| Rate limited | 60 seconds | Single successful query |
| Timeout | 15 seconds | Single successful query |
| Transport / HTTP error | 30 seconds | Single successful query |
| Other | 30 seconds | Single successful query |

A single successful query immediately clears cooldown, resets the failure counter, and restores the provider to `Healthy`.

### Health in `provider_status`

The `health_views` field in `provider_status` provides per-provider health views with:

- `status` — current health state (`Healthy`, `Degraded`, `Cooldown`, `Unknown`)
- `consecutive_failures` — current failure streak
- `last_error_class` — most recent failure class (e.g. `RateLimited`, `Timeout`, `NetworkError`)
- `last_error_message` — human-readable error from the last failure
- `cooldown_until` — remaining cooldown time (e.g. `"42s"`)
- `cooldown_reason` — why cooldown was triggered (e.g. `"rate limited"`, `"repeated timeouts"`)
- `last_latency_ms` — latency of the most recent query
- `last_success_at` — time since last success (e.g. `"15s ago"`)
- `last_failure_at` — time since last failure

### Routing Integration

When resolving which providers to query, eggsearch checks `is_in_cooldown()` for each candidate. Cooled-down providers are skipped with `skip_code: cooldown_active`. If all profile providers are unavailable, routing falls back to `default_providers`. If defaults are also unavailable, a `profile_degraded` warning is emitted.

## Troubleshooting

| Symptom | Cause | Fix | Diagnostic |
|---------|-------|-----|------------|
| Provider not returning results | Disabled in `[search.providers]` | Set the provider to `true` | `provider_status` → check `enabled` field |
| API provider skipped | `api_key_env` not set or env var missing | Set the env var or disable the provider | `provider_status` → check `skip_code: missing_api_key` |
| SearXNG unavailable | Missing `base_url` or both flags not set | Set `base_url` in `[search.searxng]` and enable in `[search.providers]` | `provider_status` → check `skip_code: missing_searxng_config` |
| Gitea/GitLab provider fails | Missing `base_url` | Set `base_url` in the `[search.api.<provider>]` section | `provider_status` → check `skip_code: missing_base_url` |
| Profile degraded warning | A profile provider is unavailable | Configure the missing provider or accept fallback to defaults | `provider_status` → check `skip_code` and `health_view` |
| Provider in cooldown | 3+ consecutive failures | Wait for cooldown to expire (15–60s) or fix the underlying issue | `provider_status` → check `skip_code: cooldown_active` and `health_view.status` |
| Credential env var missing | Env var not exported in shell | Export the env var or add to shell profile | `provider_status` → check `skip_code: credential_env_missing` |
| All providers fail | Network issue or all providers in cooldown | Check network connectivity; wait for cooldown expiry | `provider_status` → check `health` for all providers |
| Provider built with wrong features | `skip_code: not_built` | Rebuild with the required feature flag | `cargo build --features <feature>` |

Run `eggsearch doctor` for a diagnostic summary of all configured providers, their enabled state, API key availability, and any misconfigurations.
