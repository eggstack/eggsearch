# Provider Setup

eggsearch supports 34 search providers across six categories: web search (HTML scrapers), API-key providers, aggregators, security advisory databases, package registries, scholarly search, and special-purpose providers. Providers can be enabled individually in config and selected per-request or via `default_providers`.

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

If a provider listed in `default_providers` is disabled or lacks a valid API key, eggsearch emits a startup warning. Run `eggsearch doctor` to diagnose configuration issues.

## Troubleshooting

| Symptom | Cause | Fix |
|---------|-------|-----|
| Provider not returning results | Disabled in `[search.providers]` | Set the provider to `true` |
| API provider skipped | `api_key_env` not set or env var missing | Set the env var or disable the provider |
| SearXNG unavailable | Missing `base_url` or both flags not set | Set `base_url` in `[search.searxng]` and enable in `[search.providers]` |
| Gitea provider fails | Missing `base_url` | Set `base_url` in the `[search.api.<provider>]` section |
| Profile degraded warning | A profile provider is unavailable | Configure the missing provider or accept fallback to defaults |
| All providers fail | Network issue or all providers in cooldown | Check network connectivity; wait for cooldown expiry |

Run `eggsearch doctor` for a diagnostic summary of all configured providers, their enabled state, API key availability, and any misconfigurations.
