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

All 18 built-in providers:

| Provider | Kind | Requires | Notes |
|----------|------|----------|-------|
| `duckduckgo` | html_scrape | — | Default generic provider |
| `brave` | html_scrape | — | |
| `startpage` | html_scrape | — | Default generic provider |
| `yahoo` | html_scrape | — | Default generic provider |
| `mojeek` | html_scrape | — | |
| `searxng` | json_api | `base_url` | Requires `[search.searxng]` config |
| `brave_api` | api_key | `BRAVE_API_KEY` | |
| `github_code` | api_key | `GITHUB_TOKEN` | |
| `github_issues` | api_key | `GITHUB_TOKEN` | |
| `github_releases` | api_key | `GITHUB_TOKEN` | |
| `gitlab_code` | api_key | `GITLAB_TOKEN` | |
| `gitlab_issues` | api_key | `GITLAB_TOKEN` | |
| `gitlab_releases` | api_key | `GITLAB_TOKEN` | |
| `gitea_code` | api_key | custom env | Requires `base_url` in `[search.api.gitea_code]` |
| `gitea_issues` | api_key | custom env | Requires `base_url` in `[search.api.gitea_issues]` |
| `gitea_releases` | api_key | custom env | Requires `base_url` in `[search.api.gitea_releases]` |
| `osv` | html_scrape | — | Security advisory search |
| `local_workspace` | local | — | Requires `[local]` config |

`provider_status` returns `routable: true` only when a provider is both enabled and fully configured. Non-routable providers include a `skip_reason` explaining why (e.g. "API key not configured", "SearXNG base_url not configured").

## Search Defaults

The shipped generic search defaults favor:

- `duckduckgo`
- `startpage`
- `yahoo`

Other built-in providers can be enabled explicitly.

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

### Coding Agent

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

### Security Search

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

[search.api]
# optional fallback providers
```

### Research Search

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

The fetch side is bounded by both byte and character caps. The byte cap is a hard response limit; the character cap is an extraction limit.

`allow_private_network = true` and `allow_localhost = true` are operator escape hatches. Keep them off for general MCP exposure.

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

## Provider Status

`provider_status` reports configuration-derived provider descriptors, code-host summaries, cached health snapshots, server capabilities, tool capabilities, and workflow recipes. The `probe` field is reserved and currently has no effect.

Each provider descriptor includes:

- `id`, `display_name`, `kind` — identity
- `enabled`, `default`, `configured` — config state
- `requires_api_key` — whether an API key env var is needed
- `routable` — whether the provider can actually be queried right now
- `skip_reason` — human-readable explanation when `routable` is `false`
- `capabilities` — feature flags (code search, freshness, etc.)

Use `provider_status` to decide whether to call specialized tools or fall back to generic search.
