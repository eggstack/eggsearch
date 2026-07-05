# Configuration

eggsearch reads `$XDG_CONFIG_HOME/eggsearch/config.toml` and falls back to defaults when the file is absent.

## Minimal Default

```toml
[search]
mode = "live"
default_max_results = 10
max_results_cap = 50
max_query_chars = 512
timeout_ms = 8000
sanitize_output = true

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

```toml
[search]
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

```toml
[search]
default_providers = ["osv", "duckduckgo", "startpage"]

[search.providers]
duckduckgo = true
startpage = true
osv = true

[search.api]
# optional fallback providers
```

### Research Search

```toml
[search]
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

```toml
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

```toml
[search.providers]
searxng = true

[search.searxng]
enabled = true
base_url = "https://search.example.org"
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

Use `provider_status` to decide whether to call specialized tools or fall back to generic search.
