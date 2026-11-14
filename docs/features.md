# Capabilities and Optional Features

eggsearch ships with keyless web, fetch, advisory, registry, and scholarly paths by default. Default builds also compile in PDF extraction and browser rendering. The remaining optional features require source-build Cargo features or config changes.

---

## PDF Extraction

`web_fetch` handles PDF documents in default builds. The `pdf` Cargo feature is
compiled in, and `[fetch].pdf_enabled` defaults to the compiled-in feature state.

### Enabling

Nothing is required in a default build. The explicit form is:

```toml
[fetch]
pdf_enabled = true
```

A source build with `--no-default-features` leaves PDF out, so `[fetch].pdf_enabled`
defaults to `false`; add the feature back explicitly:

```bash
cargo build --no-default-features --features pdf
```

The published Python wheel enables `pdf` only
(`default-features = false, features = ["pdf"]`), so it carries PDF extraction and
no browser rendering. See the [Python library guide](python-library.md).

### Capabilities

- Text-only extraction via `lopdf` — no OCR, no rendering, no image extraction
- Per-page quality classification: blank, scanned, CID-corrupt, and sparse text pages
- Image presence is always detected for classification, so an image-only page is
  reported as scanned even when `include_media` is off; `include_media` only adds
  the image count to page metadata
- Document metadata (title, author, subject, keywords, creator, producer, dates)
- Bookmark/outline entries where available

### Page Selection

The `pages` field supports `1`, `1,3,5`, `1-5`, and `1,3,7-10` syntax (one-indexed).

### Limits

- 16 MiB response body (`[fetch].pdf_max_bytes`, range 1 byte to 256 MiB); the
  generic `[fetch].max_bytes` cap still applies to every non-PDF response
- 64 MiB decompressed content (`[fetch].pdf_max_decompressed_bytes`, range
  1 MiB to 1 GiB), measured over the whole document before parsing; a document
  that would expand further fails with `pdf_decompression_bound_exceeded`
- 25 pages maximum
- 12k chars per page
- 50k chars total

A PDF that exceeds `pdf_max_bytes` — whether declared in `Content-Length` or
truncated while streaming — fails with `content_too_large` naming the limit.
It is never reported as `pdf_parse_error`.

The decompressed-content budget is enforced before the parser runs, so a
decompression bomb is refused by configuration rather than by the allocator.
Flate streams are measured, ASCII85 streams are decoded under the same cap,
filters `lopdf` cannot decode are used verbatim (and a stage decoded before
such a filter is still counted), and an `LZWDecode` stream is refused because
its expansion cannot be measured. A filtered stream whose `/Length` is an
indirect reference is refused for the same reason: the extent cannot be
measured before parsing.
Encrypted documents are refused with `pdf_encrypted` whether or not a password
is supplied; `pdf_password` stays in the schema for compatibility but is
unsupported.

### Limitations

- Extracted text may be incomplete or reordered — PDF layout does not always map to linear text flow
- Scanned or image-based PDFs produce little or no text
- `metadata_only` mode returns a minimal document with fetch context but no extracted body text
- `pdf_ocr = "auto"` extracts text and reports scanned pages as scanned with an
  OCR-unavailable note per page; no OCR is attempted and no text is recovered for
  an image-only page
- `pdf_ocr = "always"` fails closed with `pdf_ocr_unavailable`, because it demands
  a capability this build does not have
- PDF layout reconstruction and OCR are deferred

---

## Bounded Challenge Solving

Some keyless search engines answer automated queries with a published
computational proof-of-work challenge instead of results. ADR-0009 permits
solving those in-process; it permits nothing else.

### Enabling

Off by default. Opt in per deployment:

```toml
[search.challenge]
enabled = true
```

### Capabilities

- Solves the methods the engine set actually encounters: Anubis `fast`, `slow` (an alias of `fast`), `preact`, `metarefresh`, and WASM `sha256`, plus ALTCHA `PBKDF2/SHA-256` (Mojeek); `argon2id` and `hashx` stay unsupported
- One shared solver module under `src/meta/challenge/`; engines call it rather than carrying per-engine copies
- At most one solve per provider per search request, and one per `cookie_ttl_seconds`
- A solved request runs through the normal parser and the normal four-outcome classification

### Limits

- Four ceilings apply to every solve: `max_difficulty`, `max_attempts`, `timeout_ms`, and the `cookie_ttl_seconds` reuse window
- A solve that reaches any ceiling fails; it never loops or retries
- Pass cookies are held in process memory only and never appear in logs, `provider_status`, diagnostics, or fixtures
- The solver sends the operator's configured user agent unchanged and adds no evasion headers

### Explicitly not permitted

Interactive CAPTCHA or visual puzzle answering, OCR of challenge images,
human or third-party solving services, credential or login bypass, session or
cookie theft, proxy rotation, user-agent spoofing, and retry widening to
conceal an outage. The browser path separately never clicks, simulates input,
or uses external services for interactive challenges.

### Outcome contract

A challenged provider that is disabled, above the difficulty ceiling,
unsupported, or out of budget reports `error_class: blocked` with a reason
naming the bound — never an empty result. A solved page that fails to parse is
`parse_error`, not `blocked`.

## Browser Rendering

Headless Chrome/Chromium rendering for JavaScript-heavy pages that ordinary HTTP fetching cannot render. The capability ships in default builds and uses a Chrome/Chromium already installed on the host.

### Enabling

The `browser` Cargo feature is compiled in by default and
`[fetch.browser].enabled` defaults to the compiled-in feature state, so no
config is required. The explicit form is:

```toml
[fetch.browser]
enabled = true
policy = "auto"
```

A source build with `--no-default-features` leaves browser rendering out, and an
explicit `render = "browser"` or `browser_profile` request then fails with
`browser_not_compiled_in` instead of quietly falling back to HTTP. Add the
feature back explicitly:

```bash
cargo build --no-default-features --features browser
```

Rendering requires a discovered Chrome/Chromium executable. eggsearch never
downloads, installs, or manages one.

### Render Policy

The `render` parameter on `web_fetch` controls transport selection. When the
caller omits it, the configured `[fetch.browser].policy` applies; a per-call
`render` overrides the configured policy.

| Value | Behavior |
|-------|----------|
| `http_only` | HTTP only, never launches Chrome |
| `auto` (default) | HTTP first, escalates to browser at most once for JavaScript shells or non-interactive verification pages |
| `browser` | Browser directly, no HTTP prefetch. Fails if no Chrome/Chromium executable is available |

`auto` does not escalate for interactive challenges, authentication pages, or rate-limited responses.

### Error Outcomes

An explicit `render = "browser"` or `browser_profile` request reports a stable
code rather than silently downgrading to HTTP:

| Code | Condition |
|------|-----------|
| `browser_not_compiled_in` | The binary was built without the `browser` feature |
| `browser_disabled` | The feature is compiled but `[fetch.browser].enabled = false` |
| `browser_unavailable` | Enabled, but no Chrome/Chromium executable was discovered |

`auto` never errors. A page that warranted escalation but could not get it is
returned over HTTP with the structured warning `browser_unavailable` and
`browser_escalated: false`.

### Safety Properties

- **Public-network-only**: Rejects localhost, private IPv4/IPv6, link-local, and cloud metadata addresses regardless of `allow_localhost`/`allow_private_network` settings
- **No browser download**: Discovers an already-installed system Chrome/Chromium. Never downloads, installs, or manages browser updates
- **No challenge solving**: Interactive challenges (CAPTCHAs, Turnstile) are detected and reported as structured error codes — never clicked, simulated, or solved with external services. This rule is scoped to the browser path; engine-level proof-of-work solving under ADR-0009 is a separate, opt-in, non-interactive mechanism
- **Deterministic executable path**: An explicitly configured invalid browser executable path fails deterministically — does not silently fall back to auto-discovery
- **Request interception**: All observable requests are intercepted and checked against the network policy
- **Bounded extraction**: DOM size, request count, navigation time, and post-load wait are all bounded by configuration. The session request bound is enforced: reaching `max_requests` closes the browser session and the next render relaunches, and an idle interval with no in-flight render tears the shared browser down
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
| `enabled` | `true` when compiled | Enable browser rendering escalation; `false` in a `--no-default-features` build |
| `policy` | `"auto"` | Default render policy when `render` is omitted: `http_only`, `auto`, or `browser` |
| `executable` | auto-discovered | Path to Chrome/Chromium executable |
| `startup_timeout_ms` | `10000` | Browser startup timeout |
| `navigation_timeout_ms` | `20000` | Page navigation timeout |
| `post_load_wait_ms` | `1500` | Wait after page load |
| `verification_wait_ms` | `10000` | Wait for non-interactive verification |
| `max_requests` | `100` | Renders per browser session; reaching it closes the session and the next render relaunches |
| `idle_timeout_ms` | `90000` | Idle interval with no in-flight render after which the shared browser process is closed (minimum `5000`) |
| `max_dom_bytes` | `4000000` | Maximum DOM size |
| `global_concurrency` | `1` | Global browser concurrency |
| `per_origin_concurrency` | `1` | Per-origin browser render cap (combined with `origin_browser_concurrency` via `min`) |
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

---

## Outbound Proxy-Chain Routing

Optional listener-free HTTP/SOCKS proxy-chain route beneath eggfetch for
fixed operator-owned provider upstreams. Source-build opt-in only: prebuilt
and default binaries do not include it. Build with the `egress` feature:

```bash
cargo build --features egress
```

Eggfetch remains the HTTP, pooling, destination-TLS, decompression,
redirect-mechanics, and deadline owner. Eggress owns only physical TCP
proxy-hop establishment. No listener, service lifecycle, admin endpoint,
system-proxy mutation, or full embed runtime is introduced. Direct behavior
is unchanged when no route is configured. Chain failures fail closed and
never fall back to direct. Retry policy stays with `OriginController`;
request/total timeouts stay with eggfetch/eggsearch with no second outer
Eggress deadline.

Supported base protocols are ordinary TCP `http`, `socks4`, and `socks5`
(`httponly` accepted as an HTTP variant). Shadowsocks, Trojan, WebSocket,
SSH, QUIC/H3, UDP, legacy crypto, pproxy compatibility, and insecure TLS
remain excluded.

Each hop `host` accepts a bare DNS hostname, an IPv4 literal, or a bare
IPv6 literal such as `::1` or `2001:db8::1`. The `host` field never takes a
scheme, userinfo, path, bracket wrapping, or an appended port; `port`
remains the sole port source.

### Enabling

```toml eggsearch-config-parse-only
[egress]
enabled = true

[[egress.hops]]
scheme = "http"
host = "127.0.0.1"
port = 8080

[[egress.hops]]
scheme = "socks5"
host = "127.0.0.1"
port = 1080
username = "alice"
password_env = "EGRESS_PROXY_PASS"
```

Proxy passwords are never stored in configuration. Each hop names the
environment variable holding its password via `password_env`; the value is
resolved at connector-build time and a missing or empty variable fails the
request closed. `AppConfig::save` never writes raw secrets because only
variable names are persisted. Diagnostics and `Debug` output use the
redacted route form (`****:****@host:port`) and never log full proxy URIs
or credentials. Proxy authentication is a hop handshake and is never
forwarded to the destination as a normal header.

### Traffic scope

When routed, only vendored provider search-engine upstreams use the
configured chain. The following remain direct by design and are guarded by
static tests:

- `web_fetch`, `batch_fetch`, and `repo_fetch` dynamic targets keep
  resolved-address pinning on the direct route. Eggfetch 0.2.0 rejects the
  custom-dialer plus `resolved_addresses` combination, so these SSRF-sensitive
  paths cannot silently migrate to proxy DNS.
- Forge tree APIs, package-resolver fetches, the self-updater,
  startup/integration loopback health probes, rmcp Streamable HTTP transport,
  and browser subprocess traffic remain direct in this phase.

### Build qualification

`egress` stays a non-default source-build feature; prebuilt release binaries
never include it. The opt-in feature is compile-qualified across the seven
maintained release targets by the `egress-feature-qualify` workflow, which
requires exact set equality with the first column of
`packaging/release-targets.txt` so a missing or extra matrix target fails.
Path filters cover the provider-route construction seam
(`src/fetch/egress.rs`, `src/core/config.rs`, `src/meta/engines/mod.rs`,
`src/meta/adapter/builders.rs`, `src/meta/adapter/mod.rs`,
`src/mcp/state.rs`, egress tests, Cargo files, the release-target contract,
and the workflow itself). The same contract is enforced locally by
`packaging/check-egress-qualify-contract.sh` (`make packaging-check`) and
`tests/egress_qualify_contract.rs` (`cargo test`). No egress-enabled binary
is ever published.

### Troubleshooting

- `egress route is configured but this binary was built without the
  egress feature` — rebuild with `--features egress` or set
  `[egress].enabled = false`.
- `unsupported egress scheme` — only `http`, `socks4`, `socks5`
  (`httponly`) are accepted in this phase.
- `credential env ... is missing or empty` — export the named variable
  before starting the server; unset secrets fail closed.
- `authentication` / `connection` / `timeout` dial errors carry
  kind/stage/hop/protocol facts without credentials; a failed chain never
  retries direct.

## Representation Negotiation

Enabled by default; there is nothing to turn on.

- Every HTML-ish fetch sends `Accept: text/markdown;q=1.0, text/html;q=0.9, */*;q=0.1`.
  Origins that support Markdown for Agents (Cloudflare-fronted sites, for example) return the
  smaller representation; origins that ignore the header are unaffected.
- PDF, JSON/XML/CSV/YAML/TOML, source/config, binary, and code-host raw targets keep the plain HTML
  policy, and `extract_mode = "metadata_only"` never negotiates because page metadata lives in the
  HTML head.
- A 406/415, an unsupported content type, or an empty Markdown body falls back to HTML once, so a
  fetch costs at most two requests.
- The representation is a cache dimension (`md1`/`html1`), so a Markdown body can never be served
  for an HTML cache key. Responses report `representation`, and origin token headers
  (`x-markdown-tokens`, `x-original-tokens`) are surfaced when present.
- `include_links` works on Markdown bodies: links are extracted from the parsed Markdown AST with
  the same cap and classification as the HTML path.
