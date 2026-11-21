# Capabilities and Optional Features

eggsearch provides keyless web, fetch, advisory, registry, and scholarly paths by default. Current source builds also enable PDF extraction and browser rendering by default. The published `v0.4.4` binaries predate that change and include neither feature. Other optional capabilities require Cargo features or config changes.

## Knowledge delta

`knowledge_brief` is in the default binary. It needs no Cargo feature. A
bundled, source-verified model-cutoff catalog ships with it, and operator or
caller overrides take precedence. The local evidence index is bundled rusqlite
and is off until `[knowledge_delta.index].enabled` is set; enabling it is the
first of two documented steps, the second being an explicit `retain` call that
seeds public evidence for later index or related-topic reads. `coverage`
reports whether the index was `not_opened`, `disabled`, `open`, `degraded`, or
`offline`, and an index refresh that finds nothing distinguishes an unseeded
index from a stale one and from a genuine no-match. There is no second SKU and
no system SQLite link. Linux and Windows default-binary footprint measurements
are still unrun; that is an operational gate, not a separate build.

## Retrieval Integrity Contracts

These contracts are always on in current builds; they need no feature flag or
configuration.

### Unified fetch budget

`fetch` keeps two distinct measures. `chars_returned` is the readable-text
measure: Unicode scalar values in the returned `text` plus bounded visible
metadata; a repo item is not charged twice for its structural `document`/`lines`
copy of the same text. `payload_chars` is the serialized character count of the
item's response value before `response_detail` projection, and
`total_payload_chars` sums it across emitted items; both are additive telemetry,
and projection may remove fields after measurement, so they conservatively
over-count a compact response.

The aggregate `max_total_chars` budget is enforced against that emitted payload.
Before any text is clipped, a repo item drops optional structure in order:
`document.blocks`/`document.chunks`, then `document`, then `lines`. Each removed
field is listed in `degraded_fields` and produces one
`batch_payload_budget_degraded` warning; a structural drop does not set
`truncated`, which continues to mean text was clipped. Only when no droppable
structure remains is `text` clipped, and an item that still cannot fit its
locator and metadata is a failed item (`ok: false`, `response: null`,
`chars_returned: 0`) with an attributable `batch_total_budget_exhausted` error
and warning. A repo item missing `owner`, `repo`, or `path` fails before
dispatch with an error that names the field and the accepted shape.

### Local search admission and scope

A local workspace file is a `repo_search` result only when the query, its path or
file name, its content, or a requested symbol matches. Source-role bonuses rank
candidates but never admit one. When the path-ranked first pass admits fewer than
the requested results, one bounded second pass scans the remaining inventory in
deterministic relative-path order and fills the remaining slots. A bounded
`git grep` literal hint skips files that cannot match when the root is a git
checkout; every candidate is still validated and read locally, and a git failure
falls back to the walk, and the hint is skipped when fewer than 32 candidates
remain unscanned. The pass stops at the result cap, 512 files read, 8 MiB of
content, 1.5 seconds, or the request deadline. The deadline bounds the scan; a
one-time inventory build is bounded separately and does not consume it.

A completed scan with no matches carries `local_search_completed_no_matches`;
`local_search_truncated` means a scan bound cut the scan short, admitted matches
were dropped, or the inventory cap could have hidden a match. An unscoped request
includes local results by default; a request that names a host, owner, repository,
or organization includes them only with explicit `include_local: true`, and
`include_local: false` disables them everywhere.

For `repo_fetch`, `prefer_local: true` tries an enabled local workspace first. A
matching checkout is fetched locally; otherwise the remote path remains the
fallback and the response warning explains that the local copy was skipped.
Authorized host scopes retain their existing error when no matching local
workspace is available.

### Native security advisories

For identifier-bearing security requests (`cve_id`, `ghsa_id`, `osv_id`,
`rustsec_id`, or `package`) with no explicit `providers`, `security_search`
adds every provider the configuration already enables and that advertises native
advisory lookup. It never enables a provider and never changes
`default_providers`. Native vulnerabilities carry an optional bounded `summary`
(500 characters, sanitized), and the availability and severity warnings describe
what actually happened: `native_advisory_search_unavailable` only when no enabled
provider advertises a native operation, `severity_unavailable` only when no
returned vulnerability carries severity.

---

## PDF Extraction

`web_fetch` extracts PDF text when the binary includes the `pdf` Cargo feature
and `[fetch].pdf_enabled` is true. Current source builds enable this feature by
default. Published `v0.4.4` binaries do not include it. The config value defaults
to the compiled-in feature state.

### Enabling

No config change is needed in a current source build. To enable extraction in a
build that includes the feature but has disabled it in config:

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

- Text-only extraction via `lopdf`, with no OCR, rendering, or image extraction
- Per-page quality classification: blank, scanned, CID-corrupt, and sparse text pages
- Image presence is always detected for classification, so an image-only page is
  reported as scanned even when `include_media` is off; `include_media` only adds
  the image count to page metadata
- Document metadata (title, author, subject, keywords, creator, producer, dates)
- Bookmark/outline entries where available

### Page Selection

The `pages` field on `web_fetch` and each `batch_fetch` web item supports `1`,
`1,3,5`, `1-5`, and `1,3,7-10` syntax (one-indexed). Batch items validate
syntax before fetching; page numbers beyond the fetched document fail only that
item. Non-PDF URLs and `metadata_only` follow the same behavior as `web_fetch`.

An explicitly named PDF under an enabled `[local]` root is also extracted by
`repo_fetch` with `host = "workspace"`. Detection uses a case-insensitive `.pdf`
extension or `%PDF-` prefix. Local reads use `[local].pdf_max_file_bytes`
(default 16 MiB, range 1 byte to 256 MiB) and the same PDF feature and
`[fetch].pdf_enabled` gates. Page selection is not available on `repo_fetch` or
batch repo items; local PDFs remain excluded from local indexing and repo maps.

### Limits

- 16 MiB response body (`[fetch].pdf_max_bytes`, range 1 byte to 256 MiB); the
  generic `[fetch].max_bytes` cap still applies to every non-PDF response
- 64 MiB decompressed content (`[fetch].pdf_max_decompressed_bytes`, range
  1 MiB to 1 GiB), measured over the whole document before parsing; a document
  that would expand further fails with `pdf_decompression_bound_exceeded`
- 25 pages maximum
- 12k chars per page
- 50k chars total

A PDF that exceeds `pdf_max_bytes`, whether declared in `Content-Length` or
truncated while streaming, fails with `content_too_large` naming the limit.
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

- Extracted text may be incomplete or reordered because PDF layout does not always map to linear text flow
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

On by default and bounded. Disable per deployment:

```toml
[search.challenge]
enabled = false
```

### Capabilities

- Solves the methods the engine set actually encounters: Anubis `fast`, `slow` (an alias of `fast`), `preact`, `metarefresh`, and WASM `sha256`, plus ALTCHA `PBKDF2/SHA-256` (Mojeek); `argon2id` and `hashx` stay unsupported
- Search engines share the solver in `src/meta/challenge/`; fetch-path detect/solve/replay is owned by `src/challenge_replay/`
- With solving on (the default), `web_fetch`, `batch_fetch`, unified `fetch` web items, remote `repo_fetch`, and CLI `eggsearch fetch` attempt a bounded solve for a published Anubis or ALTCHA challenge
- A fetch-path ALTCHA solve takes its challenge document and its verification endpoint from the page's own `altcha-widget` publication, same-origin only; a cross-origin, scheme-changing, or malformed publication is refused before any request is issued
- A fetch-path pass cookie is reused for a later fetch to the same origin, from a process-local cache keyed by origin. The window is the upstream-published lifetime clamped by `cookie_ttl_seconds`; a fetch that crossed an origin stores nothing and reuses nothing
- A pair set the upstream refuses is evicted, and the same call then solves the challenge it was already holding and replays once with the freshly issued pair set, so a stale pair set costs no second fetch. A transport error or a non-2xx status is not a refusal: the stored pair set is kept and no solve is attempted
- One fetch call still performs at most one solve and one replay; a reused pair set performs no solve at all, and a refused pair set is presented at most once
- Under `render: "auto"` the solver runs before the browser, so a solvable Anubis or ALTCHA wall returns its document without launching Chrome. Only a wall the solver leaves unsolved escalates
- A script-driven wall the browser cannot get past is returned marked with `manual_interaction_required: true` and a `fetch_challenge_page` warning, never as an unmarked document
- Fetch-path solves use the `fetch` label in `provider_status.challenge_solver.providers`; search upstreams retain their own labels. A wall the browser completed is counted beside them as `browser_completions`
- At most one solve per provider per search request, and one per `cookie_ttl_seconds`
- A solved search request runs through the normal parser and the normal four-outcome classification

### Limits

- Four ceilings apply to every solve: `max_difficulty`, `max_attempts`, `timeout_ms`, and the `cookie_ttl_seconds` reuse window
- `max_difficulty` gates the Anubis family only. An ALTCHA challenge publishes no difficulty: it is bounded by `max_attempts`, which counts PBKDF2 counter iterations, and by `timeout_ms`. The published PBKDF2 `cost` multiplies the work each iteration performs, and a cost above the solver's ceiling is refused as malformed rather than started
- A solve that reaches any ceiling fails; it never loops or retries
- Pass cookies are held in process memory only and never appear in logs, `provider_status`, diagnostics, or fixtures. The fetch-side cache is bounded to 32 origins and 64 KiB and is never written to disk
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
naming the bound, not an empty result. A solved page that fails to parse is
`parse_error`, not `blocked`.

## Browser Rendering

Headless Chrome/Chromium rendering supports JavaScript-heavy pages that ordinary HTTP fetching cannot render. Current source builds include the `browser` Cargo feature by default, but published `v0.4.4` binaries do not. Rendering uses a Chrome or Chromium executable already installed on the host.

### Enabling

In current source builds, `[fetch.browser].enabled` defaults to the compiled-in
feature state, so no config is required. To enable rendering in a build that
includes the feature but has disabled it in config:

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
| `auto` (default) | HTTP first, then one bounded solve-and-replay when the page publishes an Anubis or ALTCHA challenge, then at most one browser escalation for a wall the solver left unsolved |
| `browser` | Browser directly, no HTTP prefetch. Fails if no Chrome/Chromium executable is available |

`auto` does not escalate for authentication or rate-limited responses. If browser
rendering finds an interactive challenge, `web_fetch` returns a manual-interaction
error; it never clicks or simulates it. A non-interactive script wall is completed by
letting the page's own script run, and the render polls the live classification for up
to `verification_wait_ms`. A wall that still has not resolved is returned marked as a
challenge page rather than as a document.

### Error Outcomes

An explicit `render = "browser"` or `browser_profile` request reports a stable
code rather than silently downgrading to HTTP:

| Code | Condition |
|------|-----------|
| `browser_not_compiled_in` | The binary was built without the `browser` feature |
| `browser_disabled` | The feature is compiled but `[fetch.browser].enabled = false` |
| `browser_unavailable` | Enabled, but no Chrome/Chromium executable was discovered |

When auto escalation is unavailable, the HTTP response is returned with the
structured warning `browser_unavailable` and `browser_escalated: false`. If the
browser detects an interactive challenge, the call returns a manual-interaction
error instead.

### Safety Properties

- **Public-network-only**: Rejects localhost, private IPv4/IPv6, link-local, and cloud metadata addresses regardless of `allow_localhost`/`allow_private_network` settings
- **No browser download**: Discovers an already-installed system Chrome/Chromium. Never downloads, installs, or manages browser updates
- **No interactive challenge solving in the browser**: CAPTCHAs and Turnstile are detected and reported as structured error codes. They are never clicked, simulated, or sent to external solving services. A non-interactive script wall is completed only by letting the page's own script run inside the sandboxed render; the solver in `src/meta/challenge/` stays separate and bounded by its own ceilings
- **Deterministic executable path**: An explicitly configured invalid browser executable path fails without silently falling back to auto-discovery
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

- `transport`: `"http"` or `"browser"`
- `browser_escalated`: whether auto mode escalated
- `browser_profile`: display name when a persistent profile was used

Fresh raw cache entries can satisfy a new extraction mode, link setting, character bound, or PDF page selection without another network request.

### Viewport Screenshots

`web_fetch` and each `batch_fetch` web item accept the opt-in `include_screenshot`
boolean. When true, eggsearch forces browser rendering and captures the current
viewport as PNG after page extraction. The browser must be enabled and an installed
Chrome/Chromium executable must be available; capability failures are explicit,
with no HTTP-only fallback. An explicit `render = "http_only"` conflicts with
`include_screenshot = true`; when `render` is omitted, the opt-in overrides the
configured render policy.

The screenshot is delivered as native MCP image content, separate from the
unchanged structured JSON response. It is limited to 1280×720 pixels and 4 MiB.
Recognized PDF responses are not captured; repository and workspace items do not
support screenshots. Screenshot requests bypass cache reads and writes. In
`batch_fetch`, each image is associated with its original zero-based item index
and label, in input order; any batch containing a screenshot request runs
synchronously rather than as a deferred task.

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

- Created only through CLI commands. MCP callers cannot create profiles or launch headed browsers
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
- Chrome manages cookies and storage within the profile directory. Eggsearch never exports, logs, or serializes cookies
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

### Troubleshooting

- If the binary reports that the egress feature is missing, rebuild with
  `--features egress` or set `[egress].enabled = false`.
- Only `http`, `socks4`, and `socks5` (`httponly`) schemes are accepted.
- If a credential environment variable is missing or empty, export it before
  starting the server. Unset secrets fail closed.
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
