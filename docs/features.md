# Capabilities and Optional Features

eggsearch provides keyless web, fetch, advisory, registry, and scholarly paths by default. Current source builds enable PDF extraction and browser rendering by default, and the next release carries both into the CLI binaries and the Python wheels. The published `v0.4.4` binaries predate that change and include neither feature; the published `eggsearch==0.4.4` wheel is PDF-only. A system Chrome/Chromium executable is a runtime dependency for browser rendering only; no browser is bundled. Other optional capabilities require Cargo features or config changes.

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

`fetch` keeps two distinct measures. `chars_returned` is the M036 readable-text
measure: Unicode scalar values in `text` plus bounded visible metadata; a repo
item is not charged twice for structural `document`/`lines` copies. `payload_chars`
is the serialized character count of the response after the selected
`response_detail` projection; `total_payload_chars` is the exact sum across
returned item responses. Both are additive telemetry and the aggregate budget
uses that same projected payload.

The aggregate `max_total_chars` budget caps the sum of projected item response
payloads, not the full MCP envelope (item wrappers, aggregate counts, warnings,
addressing keys, and projection stamps are excluded). Optional returned structure
is removed before text clipping for both web and repo items: `document.blocks`/
`document.chunks`, `document`, then `lines`; web items then trim links and remove
description/title. Budget drops appear in `degraded_fields` and produce one
`batch_payload_budget_degraded` warning; structural drops do not set `truncated`,
which continues to mean text was clipped. An item that cannot fit is a failed
item (`ok: false`, `response: null`, `chars_returned: 0`) with an attributable
`batch_total_budget_exhausted` error and warning. A repo item missing `owner`,
`repo`, or `path` fails before dispatch with an error naming the field and
accepted shape.

### Inline HTML whitespace fidelity

HTML extraction retains source whitespace at inline text, link, and code
boundaries before normalization in both text and Markdown modes. Repeated
whitespace collapses, but adjacent words are not merged and no separator is
invented where the source contains none. Raw code-block whitespace is unchanged.

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

For a repo item in `fetch`, `prefer_local: true` tries an enabled local workspace first. A
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

A web item sent to canonical `fetch` extracts PDF text when the binary includes
the `pdf` Cargo feature and `[fetch].pdf_enabled` is true. Current source builds
enable this feature by default; the published `v0.4.4` binaries do not include it,
while the published `eggsearch==0.4.4` wheel does. The config value defaults to
the compiled-in feature state.

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

The Python extension builds with `default-features = false` and
`features = ["pdf", "browser"]`. Every Python wheel target uses this one
feature set; there is no browser/no-browser wheel variant, and `egress` is not
compiled. A wheel built from current source includes both PDF extraction and
browser rendering. The published `eggsearch==0.4.4` wheel remains PDF-only. See
the [Python library guide](python-library.md).

### Capabilities

- Text-only extraction via `lopdf`, with no OCR, rendering, or image extraction
- Per-page quality classification: blank, scanned, CID-corrupt, and sparse text pages
- Image presence is always detected for classification, so an image-only page is
  reported as scanned even when `include_media` is off; `include_media` only adds
  the image count to page metadata
- Document metadata (title, author, subject, keywords, creator, producer, dates)
- Bookmark/outline entries where available

### Page Selection

A web item's `pdf.pages` option supports `1`, `1,3,5`, `1-5`, and `1,3,7-10`
syntax (one-indexed). Multi-item fetches validate syntax before fetching; page
numbers beyond the fetched document fail only that item. Non-PDF URLs and
`metadata_only` use the ordinary fetch behavior.

An explicitly named PDF under an enabled `[local]` root is also extracted by
`fetch` with a repo item whose `host` is `"workspace"`. Detection uses a case-insensitive `.pdf`
extension or `%PDF-` prefix. Local reads use `[local].pdf_max_file_bytes`
(default 16 MiB, range 1 byte to 256 MiB) and the same PDF feature and
`[fetch].pdf_enabled` gates. Page selection is not available on repo items; local
PDFs remain excluded from local indexing and repo maps.

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

## Document Formats

CSV, EPUB, DOCX, XLSX, and PPTX are read as bounded, addressable documents when
the binary includes the `documents` Cargo feature. PDF extraction is separate,
stays on by default, and is described under [PDF Extraction](#pdf-extraction).

### Enabling

`documents` is not in the default feature set. A source build that needs the
container formats opts in explicitly:

```bash
cargo build --features documents
```

The feature adds the `zip` and `calamine` dependencies. It is additive: with
the feature absent, every request behaves exactly as before, CSV keeps its
existing bounded table preview, and a `.docx` or `.epub` URL returns the
ordinary unsupported-content outcome.

In a build that has the feature, `[fetch].documents_enabled` is the operator
switch. It defaults to the compiled-in state; setting it to `false` refuses
every detected container with `document_disabled` instead of extracting it.
`provider_status` reports the same state before a call:
`documents_capabilities { compiled, configured, usable, formats }` plus
`server_capabilities.container_documents`. Without the feature the triad is all
`false` and the setting does nothing, because the code is absent.

### Capabilities

- One part per addressable unit: a CSV row block, an EPUB spine document, a
  DOCX heading-delimited section, a spreadsheet sheet, or a presentation slide
- `extract_mode = "text"` and `extract_mode = "markdown"`; `metadata_only`
  returns metadata and no parts
- Fixed ordering: DOCX body order, `presentation.xml` slide order, workbook
  sheet order, EPUB spine order, and fixed 100-row CSV blocks. ZIP directory
  order is never used
- ISO 8601 dates and locale-independent numbers for spreadsheets
- Text only: no OCR, no layout geometry, no rendering, no macros

### Part Addressing

| Format | Part | `title` |
|---|---|---|
| CSV/TSV | One fixed 100-row block | `rows 1-100` |
| EPUB | One spine document | Section heading or spine item label |
| DOCX | One heading-delimited section | The heading text |
| XLSX | One sheet | The sheet name |
| PPTX | One slide | The title placeholder, when present |

Parts are one-indexed, and `parts_total` reports the count. This version
returns every part; selection over parts is not yet a request control, because
the web-fetch request shape has no part selector (only PDF page selection).

### Limits

| Bound | Value |
|---|---|
| Input archive | 16 MiB |
| Total decompressed data | 64 MiB |
| Inflation ratio | 100:1 |
| Archive entries | 4,096 |
| Parts | 4,096 |
| Characters before the caller budget | 200,000 |
| XML depth | 64 |

ZIP central-directory sizes are attacker-controlled, so actual inflated bytes
are counted through a bounded reader and the archive is pre-scanned before a
spreadsheet parser inflates it. A bound breach is a typed error, never a partial
success. Nothing is written to disk, and absolute or parent-traversal entry
names are rejected defensively. No DTD, external entity, `XInclude`, or
external media reference is resolved.

### Outcome Contract

| Outcome | Meaning |
|---|---|
| `document_empty` | The container parsed but carried no text |
| `document_malformed` | The archive or its XML is structurally invalid |
| `document_unsupported` | ODF, legacy binary Office, or RTF |
| `document_encrypted` | Password-protected OOXML (`EncryptedPackage`) |

A supported format returns the document object instead of the ordinary text
body. ODF, legacy `.doc`/`.xls`/`.ppt`, and RTF are refused explicitly
rather than guessed at.

### Limitations

- No OCR, image understanding, or scanned-document recovery
- No layout blocks, bounding boxes, forms, comments, or tracked changes
- No macro execution and no embedded-object or external-relationship resolution
- No ODF or legacy binary Office formats
- No document writing or conversion
- Local arbitrary-path parsing stays inside the existing authorized workspace rules

## Deterministic Change Comparison

A second fetch of the same document can answer "did this change" without
re-reading the whole page. Comparison is opt-in per web item, survives a process
restart, and never calls a model.

### Enabling

Enable the store:

```toml
[fetch.snapshots]
enabled = true
```

Then ask for a comparison on a web item:

```json
{"url": "https://example.com/release-notes", "snapshot": {"compare": "line"}}
```

With `compare = "none"` (the default) the request behaves exactly as before
and no store is opened.

### Request Fields

| Field | Default | Meaning |
|---|---|---|
| `compare` | `none` | `none` or `line` |
| `key` | absent | Caller label scoping independent comparisons, at most 128 bytes |
| `max_hunks` | `20` | Returned hunks, maximum 100 |
| `max_diff_chars` | `8192` | Returned diff characters, maximum 32768 |

### Change Statuses

| Status | Meaning |
|---|---|
| `new` | No stored snapshot for this key |
| `same` | Identical normalised text |
| `changed` | Differing text with a bounded line diff |
| `removed` | The origin returned 404 or 410 for a key that has a snapshot; the snapshot is retained. Counts are zero and `hunks` is empty, because there is no new text to compare |

`changed` carries `previous_fetched_at`, `current_fetched_at`,
`chars_added`, `chars_removed`, hunks ordered by `old_start` then
`new_start`, and a `truncated` flag. Each hunk records `old_start`,
`old_lines`, `new_start`, `new_lines`, and the diff `lines`, with three
lines of context. `chars_added` and `chars_removed` count characters in hunk
lines only, so they agree with the bounded output even when `truncated` is true.

### Identity and Scope

The snapshot key is (scope, normalised URL, extract mode, scoping fingerprint,
representation class, caller key). `CacheScope` participates, so a snapshot
written under one browser profile is invisible to anonymous requests and to
other profiles. Equality is SHA-256 over normalised text; addressing keeps the
content-derived FNV-1a rules.

### Bounds and Degradation

- 1 MiB of stored text per snapshot, 4,096 UTF-8-boundary-checked lines
- The diff guard is size-based, never wall-clock: more than 20,000 combined
  lines or 4,000,000 line pairs returns counts with `truncated: true` and no
  hunks, so repeated runs stay byte-identical
- Disabled store: the response carries `snapshot_store_disabled` and no
  `change` object; it never reports `new` as if a comparison happened
- Store error: the fetch still succeeds, the response carries
  `snapshot_store_unavailable`, and `change` is omitted
- A snapshot write failure never fails a successful fetch

### Storage and Privacy

Snapshots hold sanitised extracted text and metadata only: no raw HTML, no
headers, no credentials, no TLS material, and no browser profile data.
Browser-rendered responses are not snapshotted. The store is a SQLite file with
`secure_delete`, `journal_mode = DELETE`, a page limit derived from
`max_bytes`, a `0700` directory and a `0600` file. A newer schema is
refused rather than reset, and a corrupt file is reported rather than silently
recreated.

### Limitations

- No scheduling, polling, webhooks, or notifications
- No cross-host or cross-device snapshot sharing
- No semantic or model-based summarisation of change
- Browser-rendered responses are not compared in this version

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
- With solving enabled (the default), web items sent to canonical `fetch`, its direct-call `web_fetch` and `batch_fetch` aliases, and CLI `eggsearch fetch` attempt a bounded solve for a published Anubis or ALTCHA challenge
- A fetch-path ALTCHA solve takes its challenge document and its verification endpoint from the page's own `altcha-widget` publication, same-origin only; a cross-origin, scheme-changing, or malformed publication is refused before any request is issued
- A fetch-path pass cookie is reused for a later fetch to the same origin, from a process-local cache keyed by origin. The window is the upstream-published lifetime clamped by `cookie_ttl_seconds`; a fetch that crossed an origin stores nothing and reuses nothing
- A pair set the upstream refuses is evicted, and the same call then solves the challenge it was already holding and replays once with the freshly issued pair set, so a stale pair set costs no second fetch. A transport error or a non-2xx status is not a refusal: the stored pair set is kept and no solve is attempted
- One fetch call still performs at most one solve and one replay; a reused pair set performs no solve at all, and a refused pair set is presented at most once
- Under `render: "auto"` the solver runs before the browser, so a solvable Anubis or ALTCHA wall returns its document without launching Chrome. Only a wall the solver leaves unsolved escalates
- A script-driven wall the browser cannot get past is returned marked with `manual_interaction_required: true` and a `fetch_challenge_page` warning, never as an unmarked document
- Fetch-path solves use the `fetch` label in `provider_status.challenge_solver.providers`; search upstreams retain their own labels. A wall the browser completed is counted beside them as `browser_completions`
- CLI `eggsearch fetch` honours `[fetch.browser].policy` with no per-call flag: `auto` (the default) escalates on a browser-gated status, `http_only` keeps the error, and `browser` renders directly and reports the unavailable message when no executable was discovered. The CLI never gains profile-scoped or screenshot behaviour
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

Headless Chrome/Chromium rendering supports JavaScript-heavy pages that ordinary HTTP fetching cannot render. The `browser` feature is compiled into the default artifact set built from current source, including the CLI and the single Python wheel feature set; the published `v0.4.4` binaries predate it. Chrome/Chromium is an external runtime dependency and is never bundled. Its absence does not prevent installation, process startup, search, or HTTP fetching; only requests that need rendering are degraded.

### Enabling

In builds from current source, `[fetch.browser].enabled` defaults to `true`
because the feature is compiled in, so no config is required.
To enable rendering in a build that includes the feature but has disabled it in
config:

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

### Provider Result Pages

`brave` and `yahoo` gate keyless access on browser execution: Brave serves a proof-of-work page
with HTTP 429 and Yahoo answers a headless client with HTTP 500 and no body. When
`[fetch.browser].enabled` is true and a Chrome or Chromium executable was discovered, both engines
fall back to one bounded render of their result page and parse the rendered DOM; without a browser
they report the HTTP outcome and, when rendering was warranted, a
`browser_unavailable` warning with the capability reason. The fallback costs a
render per search (a few seconds) and is bounded by the request deadline.
Interactive CAPTCHAs are never attempted and are reported instead.

### Render Policy

For a web item sent to canonical `fetch`, `render` controls transport selection.
When the caller omits it, the configured `[fetch.browser].policy` applies; an
item-level `render` overrides the configured policy.

| Value | Behavior |
|-------|----------|
| `http_only` | HTTP only, never launches Chrome |
| `auto` (default) | HTTP first, then one bounded solve-and-replay when the page publishes an Anubis or ALTCHA challenge, then at most one browser escalation for a wall the solver left unsolved |
| `browser` | Browser directly, no HTTP prefetch. Fails if no Chrome/Chromium executable is available |

`auto` does not escalate for authentication or rate-limited responses. If browser
rendering finds an interactive challenge, it reports a manual-interaction error;
eggsearch never clicks or simulates it. A non-interactive script wall is completed by
letting the page's own script run, and the render polls the live classification for up
to `verification_wait_ms`. A wall that still has not resolved is returned marked as a
challenge page rather than as a document.

### Availability and Degradation

The table is normative for explicit browser requests and implicit (`auto`)
requests. A request that does not require rendering continues on its HTTP path.

| Condition | Explicit browser request | Implicit (`auto`) request |
|---|---|---|
| Feature not compiled | `browser_not_compiled_in` | HTTP outcome plus a `browser_unavailable` warning only if rendering was warranted |
| `enabled = false` | `browser_disabled` | HTTP outcome plus a `browser_unavailable` warning only if rendering was warranted |
| Enabled, no executable found | Structured `browser_unavailable` error with the discovery reason | HTTP outcome plus a `browser_unavailable` warning with the discovery reason only if rendering was warranted |
| Enabled, unrecognized Chrome/Chromium version output | Structured `browser_unavailable` error naming the output | HTTP outcome plus a `browser_unavailable` warning with the discovery reason only if rendering was warranted |
| Enabled, explicit path invalid | Structured `browser_unavailable` error naming the path; no fallback to auto-discovery | HTTP outcome plus a `browser_unavailable` warning only if rendering was warranted |
| Enabled and discovered but the launch fails | Structured `browser_unavailable`-class failure naming the launch error, never a silent HTTP fetch | Original HTTP outcome plus a `browser_unavailable` warning with the launch cause only if rendering was warranted |

The explicit tool error carries `data.code = "browser_unavailable"` and a
`data.message` naming the discovery, version, path, or launch cause. In `auto`,
HTTP remains the outcome when rendering is unavailable; no warning is added when
rendering was not warranted.

`provider_status.browser_capabilities` always reports `compiled`, `configured`,
`discovered`, `usable`, `reason`, and the lifecycle counters, in every one of
those states. If the browser detects an interactive challenge, the call returns
a manual-interaction error instead.

Auto mode also escalates when the origin answers the HTTP attempt with a
browser-gated status: `429`, or a `5xx` such as Yahoo's empty `500` to a client
that announces a headless browser. A rendered document replaces the error only
when the render ended on a `2xx`/`3xx` page or completed a recognised wall; a
`5xx` page the browser only re-rendered keeps the original `http_status` error,
with the attempted escalation named in the message. Under auto a gated status is
not retried, because a retry meets the same wall. A `401`, `402`, `403`, or
`404` never renders. This rule covers web items sent to `fetch`, direct-call
compatibility aliases, and CLI `eggsearch fetch`.

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

- `transport`: `"http"` or `"browser"`; names the pipeline that produced the response, not the fallback that engaged
- `browser_escalated`: whether auto mode escalated; alone records whether `auto` fell back after HTTP
- `browser_profile`: display name when a persistent profile was used

Fresh raw cache entries can satisfy a new extraction mode, link setting, character bound, or PDF page selection without another network request.

### Viewport Screenshots

A web item sent to `fetch` accepts the opt-in `include_screenshot` boolean. When
true, eggsearch forces browser rendering and captures the current
viewport as PNG after page extraction. The browser must be enabled and an installed
Chrome/Chromium executable must be available; capability failures are explicit,
with no HTTP-only fallback. An explicit `render = "http_only"` conflicts with
`include_screenshot = true`; when `render` is omitted, the opt-in overrides the
configured render policy.

The screenshot is delivered as native MCP image content, separate from the
unchanged structured JSON response. It is limited to 1280×720 pixels and 4 MiB.
Recognized PDF responses are not captured; repository and workspace items do not
support screenshots. Screenshot requests bypass cache reads and writes. In a multi-item `fetch`, each image is associated with its original zero-based
item index and label, in input order. A request containing a screenshot runs
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

Once established, select it on a web item sent to canonical `fetch`:

```json
{
  "type": "web",
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

## Dynamic-Fetch Proxy Routing

Optional single-endpoint proxy route for the web URLs an agent supplies to
`fetch` and `batch_fetch`. It is compiled into every build, unlike the
provider-only `[egress]` route, because it protects dynamic targets rather than
operator-owned upstreams. It is disabled by default.

The authorized resolved address still binds the physical connection: every
proxied request carries an eggfetch target pin, so the proxy tunnels to the
address eggsearch validated instead of resolving the origin name itself.
Eggfetch remains the HTTP, pooling, destination-TLS, decompression, redirect,
and deadline owner.

```toml eggsearch-config-parse-only
[fetch.proxy]
route = "proxy"
url = "http://127.0.0.1:3128"
username_env = "EGGSEARCH_PROXY_USER"
password_env = "EGGSEARCH_PROXY_PASSWORD"
no_proxy = ["localhost", "127.0.0.1", ".internal.example"]
```

- `direct` (default) never proxies. `url`, the credential variables, and
  `no_proxy` must be absent.
- `proxy` routes every web target except a `no_proxy` host match. A bypass is
  reported in the response `route` field; it is never silent.
- `proxy_required` routes every target and rejects a non-empty `no_proxy`: the
  configuration cannot express a direct path.

`url` accepts `http://` (HTTP CONNECT) and `socks5://` (local resolution).
`socks5h://`, `https://`, and inline credentials are rejected. Credentials
come only from the two environment variables, must be supplied together, and
are resolved per call. Proxy environment variables (`HTTP_PROXY`,
`HTTPS_PROXY`, `ALL_PROXY`, `NO_PROXY`) are never read; there is no PAC, no
rotation, and no per-call override.

Supported: an HTTPS target through HTTP CONNECT or local-resolution SOCKS5.
Refused with a typed error before proxy I/O: a plaintext `http://` target, and
`socks5h` remote resolution. A redirect that would change the route decision
for the hop fails closed. A missing or unusable route fails closed; a proxy is
never silently bypassed.

Failures are typed: `proxy_route_unconfigured`, `proxy_scheme_unsupported`,
`proxy_credentials_missing`, `proxy_authentication_failed`,
`proxy_connect_failed`, `proxy_target_unsupported`, and
`proxy_route_changed_by_redirect`.

Cache identity includes the route, so an entry fetched directly never satisfies
a proxied request or the reverse. `cache_only` performs no network and still
requires a matching route identity.

Scope: `fetch` and `batch_fetch` web items, plus knowledge-brief web fetches
that share the same handler. Provider upstreams,
`repo_fetch`, site discovery, browser rendering, forge, package-resolver,
updater, loopback health, and rmcp traffic stay direct. Browser-escalated
responses keep direct cache identity and never report proxy metadata. A solved
challenge cookie is origin-keyed in a process-wide cache; changing the egress
address can invalidate it, and a cookie solved on one route can be replayed on
another route to the same origin, but it is never presented to a different
origin. Multi-hop chains are supported by
pointing the single endpoint at an operator-run loopback relay; the relay stays
outside the trust boundary.

`provider_status` reports a redacted `fetch_proxy` summary: route class,
scheme, host, port, and whether credentials are present. It never reports
credential values, environment-variable names, or a serialized proxy.

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

- Canonical `fetch` web/repo items and its direct-call aliases `web_fetch`, `batch_fetch`, and `repo_fetch` keep
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
