# Optional Features

eggsearch ships with keyless web, fetch, advisory, registry, and scholarly paths by default. The following optional features require Cargo feature flags and config changes.

---

## PDF Extraction

`web_fetch` handles PDF documents when the `pdf` Cargo feature is enabled and `[fetch].pdf_enabled = true` in config.

### Enabling

```toml
[fetch]
pdf_enabled = true
```

Build with the `pdf` feature:

```bash
cargo build --features pdf
```

### Capabilities

- Text-only extraction via `lopdf` — no OCR, no rendering, no image extraction
- Per-page quality classification: blank, scanned, CID-corrupt, and sparse text pages
- Document metadata (title, author, subject, keywords, creator, producer, dates)
- Bookmark/outline entries where available

### Page Selection

The `pages` field supports `1`, `1,3,5`, `1-5`, and `1,3,7-10` syntax (one-indexed).

### Limits

- 25 pages maximum
- 12k chars per page
- 50k chars total

### Limitations

- Extracted text may be incomplete or reordered — PDF layout does not always map to linear text flow
- Scanned or image-based PDFs produce little or no text
- `metadata_only` mode returns a minimal document with fetch context but no extracted body text
- PDF layout reconstruction and OCR are deferred

---

## Browser Rendering

Optional headless Chrome/Chromium rendering for JavaScript-heavy pages that ordinary HTTP fetching cannot render.

### Enabling

Requires the `browser` Cargo feature and config:

```toml
[fetch.browser]
enabled = true
policy = "http_only"
```

Build with the `browser` feature:

```bash
cargo build --features browser
```

### Render Policy

The `render` parameter on `web_fetch` controls transport selection:

| Value | Behavior |
|-------|----------|
| `http_only` (default) | HTTP only, never launches Chrome |
| `auto` | HTTP first, escalates to browser at most once for JavaScript shells or non-interactive verification pages |
| `browser` | Browser directly, no HTTP prefetch. Fails if no Chrome/Chromium executable is available |

`auto` does not escalate for interactive challenges, authentication pages, or rate-limited responses.

### Safety Properties

- **Public-network-only**: Rejects localhost, private IPv4/IPv6, link-local, and cloud metadata addresses regardless of `allow_localhost`/`allow_private_network` settings
- **No browser download**: Discovers an already-installed system Chrome/Chromium. Never downloads, installs, or manages browser updates
- **No challenge solving**: Interactive challenges (CAPTCHAs, Turnstile) are detected and reported as structured error codes — never clicked, simulated, or solved with external services
- **Deterministic executable path**: An explicitly configured invalid browser executable path fails deterministically — does not silently fall back to auto-discovery
- **Request interception**: All observable requests are intercepted and checked against the network policy
- **Bounded extraction**: DOM size, request count, navigation time, and post-load wait are all bounded by configuration
- **Existing sanitation pipeline**: Rendered DOM flows through the same HTML extraction, text bounding, and prompt-injection sanitation as ordinary HTTP fetches

### Interactive Challenge Outcomes

CAPTCHAs, Turnstile, and login walls return structured error codes through the MCP error response:

- `browser_manual_interaction_required`
- `browser_profile_requires_attention`

These are never automated.

### Response Metadata

The response includes:

- `transport` — `"http"` or `"browser"`
- `browser_escalated` — whether auto escalated
- `browser_profile` — display name when a persistent profile was used

Fresh raw cache entries can satisfy a new extraction mode, link setting, character bound, or PDF page selection without another network request.

### Configuration Reference

| Setting | Default | Description |
|---------|---------|-------------|
| `enabled` | `false` | Enable browser rendering escalation |
| `policy` | `"http_only"` | Render policy: `http_only`, `auto`, or `browser` |
| `executable` | auto-discovered | Path to Chrome/Chromium executable |
| `startup_timeout_ms` | `10000` | Browser startup timeout |
| `navigation_timeout_ms` | `20000` | Page navigation timeout |
| `post_load_wait_ms` | `1500` | Wait after page load |
| `verification_wait_ms` | `10000` | Wait for non-interactive verification |
| `max_requests` | `100` | Maximum requests per browser session |
| `max_dom_bytes` | `4000000` | Maximum DOM size |
| `global_concurrency` | `1` | Global browser concurrency |
| `per_origin_concurrency` | `1` | Per-origin browser concurrency |
| `block_media` | `true` | Block media autoplay |

---

## Browser Profiles

Persistent browser profiles allow a local operator to establish a dedicated browser session for an origin that requires authentication or interactive human verification.

### Key Properties

- Created only through CLI commands — MCP callers cannot create profiles or launch headed browsers
- Each profile requires explicit headed local setup via `browser-login`
- Restricted to its recorded exact origin
- Disabled by default

### Setup

```bash
# Create a profile and open a headed browser for login
eggsearch browser-login https://example.com --profile my-portal

# List all profiles
eggsearch browser-profiles list

# Inspect a profile
eggsearch browser-profiles inspect my-portal

# Remove a profile
eggsearch browser-profiles remove my-portal
```

### Using a Profile

Once established, use it in `web_fetch`:

```json
{
  "url": "https://example.com/dashboard",
  "browser_profile": "my-portal"
}
```

### Enabling

```toml
[fetch.browser.persistent_profiles]
enabled = true
```

### Configuration Reference

| Setting | Default | Description |
|---------|---------|-------------|
| `enabled` | `false` | Enable persistent browser profiles |
| `profiles_dir` | platform default | Custom profiles directory |
| `allowed_profiles` | empty (all allowed) | Allowlist of profile names |
| `profile_process_timeout_ms` | `30000` | Timeout for profile-scoped browser processes |

### Storage and Isolation

- Profile metadata: `$XDG_DATA_HOME/eggsearch/browser-profiles/<opaque-id>/profile.toml`
- Chrome data: sibling `chrome-data/` directory
- Each profile uses opaque directory IDs for cache partitioning
- `browser-login` and profile-scoped MCP fetches use the same Eggsearch-owned `chrome-data` directory
- Chrome manages cookies and storage within the profile directory — eggsearch never exports, logs, or serializes cookies
- Process-local cache is not invalidated when a profile is removed from the CLI (cache is process-scoped)
