# web_fetch Resilience Closure Pass 4 — Final Correctness

**Repository:** `eggstack/eggsearch`  
**Implementation baseline:** `ebcc9f3785a1a97e9f220956ea0268c16be7895d`  
**Status:** Implementation handoff  
**Scope:** Close the remaining persistent-profile execution, raw-cache re-extraction, browser-cache provenance, configured-browser-runtime, and browser-login correctness gaps  
**Priority:** Final narrow corrective pass only  

---

## 1. Objective

The previous three closure passes corrected the overall `web_fetch` architecture:

- `render=http_only|auto|browser` now has real transport behavior;
- HTTP-to-browser escalation is one-shot and policy-bounded;
- raw HTTP response bytes are carried into the cache path;
- derived cache entries are profile-scope aware;
- profile cache scope uses opaque IDs;
- browser/manual-interaction errors are machine-readable;
- explicit invalid browser executable configuration is deterministic;
- PDF layout/OCR is explicitly deferred.

The line of work is not yet fully closed because a small number of end-to-end correctness contracts remain incomplete. This pass must address only those gaps.

The required outcome is that a manually established browser profile actually participates in profile-scoped `web_fetch`, cached raw content can be re-extracted without another network request, browser-derived cache entries retain correct bytes and provenance, configured browser runtime limits are honored, and the browser-login CLI uses the same executable/session semantics documented for MCP fetching.

Do not broaden this pass into another resilience phase.

---

## 2. Current Defects to Correct

### 2.1 Persistent profile state is not used by MCP browser execution

Current profile handling in `run_web_fetch` correctly:

- resolves the requested profile;
- verifies its origin;
- acquires the profile lock;
- retains the opaque ID for cache scope;
- retains the display name for MCP metadata.

However, the actual browser fetch still uses the shared anonymous `BrowserLifecycle`.

That lifecycle currently launches Chrome with:

- a generated temporary `eggsearch-browser/ctx-*` user-data directory;
- incognito behavior;
- an isolated browser context.

The profile-specific `chrome-data` directory populated by `eggsearch browser-login` is therefore not used for the MCP request.

This means cookies/storage established during manual login do not reach the subsequent profile-scoped headless fetch.

This is a release-blocking correctness issue for the persistent-profile feature.

### 2.2 Creating a CDP browser context defeats persistent-profile state

`browser_fetch_with_policy` currently creates a new browser context for every browser fetch.

For anonymous rendering, isolated contexts are desirable.

For an explicitly selected persistent profile, the request must operate in the browser instance's default profile context so that the profile's cookies/local storage/session state are visible.

Do not attempt to import/export cookies manually.

### 2.3 Fresh raw-cache hits cannot currently produce a missing derived representation

The raw cache now contains original bounded HTTP response bytes, and derived keys correctly include extraction semantics and profile scope.

But on a fresh raw-cache hit:

1. `web_fetch` computes the requested derived key;
2. if that derived entry exists, it returns it;
3. if it does not exist, execution falls through to the ordinary network path.

That violates the core two-tier cache contract.

A fresh raw HTML/PDF entry must be sufficient to create a new derived representation locally.

Examples:

- text -> markdown extraction on the same cached HTML;
- `include_links=false` -> `include_links=true` on cached HTML;
- PDF pages `1-3` -> pages `8-10` from the same cached PDF bytes;
- a different bounded `max_chars` class where the raw body remains sufficient.

### 2.4 Browser-rendered raw cache entries can be empty

`browser_result_to_response` currently returns `raw_body: None` even though the complete bounded rendered DOM is available in the browser transport response.

The cache writer converts missing `raw_body` into an empty byte slice.

A successful cacheable browser fetch can therefore create a raw entry whose body is empty.

The browser transport's raw cache representation must be the bounded rendered DOM bytes that were used for extraction.

### 2.5 Browser cache provenance is not fully retained on cache hits

The derived cache response currently does not preserve enough transport metadata to reconstruct a correct cached MCP response.

At minimum, cache hits need to preserve whether the source representation was:

- ordinary HTTP;
- browser-rendered;
- browser-rendered after `auto` escalation.

A browser result must not later appear indistinguishable from an ordinary HTTP result merely because it was served from cache.

Do not add a large diagnostics payload. Preserve only the small existing transport contract.

### 2.6 Browser calls use default runtime settings instead of configured settings

`ServerState` builds the shared browser lifecycle with configured values such as:

- startup timeout;
- navigation timeout;
- post-load wait;
- verification wait;
- DOM byte limit;
- request limit;
- media policy;
- concurrency values.

But `run_web_fetch` currently creates `BrowserConfig::default()` when invoking the browser path.

The effective navigation/runtime settings must come from the configured browser settings, not defaults.

### 2.7 `browser-login` executable selection is inconsistent

The browser-login command preflight recognizes an explicitly configured executable, but the launch helper rediscovers with `discover_browser(None)`.

An executable configured at a nonstandard path can therefore pass preflight and fail during actual launch.

The same resolved executable must be used for the complete browser-login operation.

### 2.8 `browser-login` completion prompt does not match implementation

The CLI tells the operator to press Enter when finished, but the current wait path listens for `Ctrl-C`/signal behavior.

The visible instruction and implementation must agree.

Prefer a simple explicit completion mechanism. Do not add a TUI or background supervisor.

---

## 3. Fixed Architectural Decisions

### 3.1 Keep anonymous browser execution warm and ephemeral

Do not replace the existing anonymous warm-browser behavior.

Anonymous `render=browser` and `render=auto` should continue using:

- Eggsearch-owned temporary browser state;
- isolated anonymous execution;
- cleanup on lifecycle shutdown;
- no ordinary user Chrome profile.

### 3.2 Persistent profile execution should remain simple and serialized

Do not create a multi-profile warm browser pool in this pass.

Use the existing profile lock as the serialization boundary.

Preferred implementation shape:

- anonymous browser requests continue using the shared warm lifecycle;
- a request with `browser_profile` creates a profile-scoped browser execution using that profile's existing `chrome-data` directory;
- the profile lock remains held for the entire profile-scoped browser execution;
- the profile-scoped browser process is closed when that fetch completes;
- the persistent `chrome-data` directory is never deleted by browser lifecycle cleanup.

This is intentionally less optimized than a browser pool but is substantially easier to reason about and avoids cross-profile state leakage.

### 3.3 Persistent profiles must use the default browser profile context

For profile-scoped execution:

- launch Chrome with `--user-data-dir=<profile chrome-data>`;
- do not force incognito mode;
- do not create a new incognito CDP browser context for the fetch;
- create/navigate the page in the default browser context so existing cookies/storage are visible.

Anonymous execution may continue creating isolated browser contexts.

### 3.4 Do not expose profile paths through MCP

MCP inputs continue accepting profile display names.

Internally:

- display name -> resolve metadata;
- metadata -> opaque profile ID;
- opaque ID -> Eggsearch-owned profile directory;
- profile directory -> `chrome-data` path.

Do not return `chrome-data` paths, opaque IDs, cookies, storage values, or browser command lines through ordinary MCP responses.

### 3.5 Re-extraction should reuse the existing extraction pipeline

Do not implement a second cache-only renderer.

Refactor the minimum necessary shared function so both:

- fresh network response bytes; and
- cached raw response bytes

flow through the same detection, PDF/HTML extraction, rendering, sanitation, trust-marker, and bounding logic.

The preferred result is one internal "derive response from raw representation" boundary rather than duplicating large sections of `FetchClient::fetch` inside `run_web_fetch`.

### 3.6 Browser raw representation is rendered DOM, not network HAR

For browser transport, the raw cache representation is the bounded rendered DOM returned by the browser execution.

Do not attempt to cache:

- every subresource;
- request/response HAR data;
- browser storage;
- screenshots;
- script execution state.

### 3.7 Keep HTTP revalidation limited to HTTP raw entries

Browser DOM entries do not have meaningful HTTP validators in the current architecture.

Do not perform ETag/Last-Modified revalidation against browser-derived raw entries.

Track enough raw-entry provenance to distinguish HTTP raw content from browser DOM raw content when making revalidation decisions.

---

## 4. Implementation Sequence

Implement in the following order. Do not reorder unless a compile dependency requires it.

### Step 1 — Add an explicit browser execution mode to lifecycle/navigation

Modify the existing browser lifecycle rather than introducing a separate browser subsystem.

Add the minimum internal representation necessary to distinguish:

```text
AnonymousEphemeral
PersistentProfile { user_data_dir }
```

Exact type naming is implementation-defined.

Required behavior:

#### AnonymousEphemeral

- use Eggsearch temporary user-data directory;
- clean it up on close/drop;
- preserve current anonymous isolation;
- incognito/default isolation behavior may remain as currently implemented.

#### PersistentProfile

- use the supplied Eggsearch-owned profile `chrome-data` directory;
- never delete the supplied directory;
- never silently substitute a temporary directory;
- do not add `--incognito`;
- ensure permissions/path validation remains with the existing profile manager boundary.

Do not make this a public plugin abstraction.

### Step 2 — Make browser navigation context-aware

Update `browser_fetch_with_policy` or a small supporting helper to know whether it is operating anonymously or with a persistent profile.

Anonymous path:

- retain isolated browser context creation/disposal.

Persistent path:

- use the launched browser's default context;
- open the target page without `CreateBrowserContext`;
- close the page at completion;
- do not dispose the default context;
- preserve all existing target URL validation and challenge detection.

The resulting response/extraction path must remain shared.

### Step 3 — Wire resolved profile storage into `run_web_fetch`

After profile metadata resolution:

- keep `meta.id` for `CacheScope::Profile`;
- keep `meta.display_name` for response metadata/errors;
- obtain the Eggsearch-owned `chrome-data` path from `ProfileManager`;
- pass that path into profile-scoped browser execution;
- hold `_profile_lock` until the browser execution is complete or errors.

Do not attach the profile directory to the shared anonymous lifecycle.

Do not reuse one profile-scoped process for another profile.

### Step 4 — Use configured browser runtime settings everywhere

Remove `BrowserConfig::default()` from normal browser execution in `run_web_fetch`.

Use the browser config derived from `state.config.fetch.browser` or expose a read-only copy/accessor from `BrowserLifecycle` if that is cleaner.

The effective runtime values must include the operator-configured:

- navigation timeout;
- post-load wait;
- verification wait;
- max DOM bytes;
- max requests/resource policy values used by navigation;
- media blocking behavior;
- other currently implemented browser safety bounds.

Do not duplicate config parsing.

### Step 5 — Preserve browser DOM bytes as raw cache input

Change browser result conversion so the rendered DOM bytes survive into `WebFetchResponse.raw_body`.

Requirements:

- `raw_body` equals the bounded DOM representation used for extraction;
- no extra DOM fetch/parsing pass is introduced;
- the MCP serializer still skips `raw_body`;
- empty DOM may be stored only when the actual rendered DOM is genuinely empty;
- browser challenge/error responses are not inserted as successful cache entries.

### Step 6 — Add raw transport provenance to cache entries

Add the smallest field needed to `RawFetchCacheEntry` to distinguish raw representation source, for example:

```text
Http
BrowserDom
```

Reuse an existing transport enum if it fits cleanly.

Use this provenance for:

- deciding whether conditional HTTP revalidation is permitted;
- preserving transport metadata when deriving a new cached response;
- avoiding claims that browser DOM is an original HTTP body.

Do not persist detailed browser timing/classification in the raw cache.

### Step 7 — Extract/centralize "derive from raw bytes" logic

Create or expose one internal function that can construct a `WebFetchResponse`-compatible derived representation from:

```text
requested URL
final URL
status
content type / relevant headers
raw bounded bytes
extract mode
max chars
include links
PDF options
sanitize flag
transport kind
```

The helper should reuse the same existing code paths for:

- content classification;
- HTML rendering;
- PDF extraction;
- title/description generation;
- link resolution using final/base URL;
- sanitation and trust markers;
- structured document generation;
- output bounding.

Avoid copying the full `FetchClient::fetch()` body into `run_web_fetch`.

### Step 8 — Re-extract on fresh raw hit + derived miss

Change the fresh cache path to:

```text
raw hit
  -> derived hit: return cached derived response
  -> derived miss: derive from raw bytes locally
       -> insert derived entry
       -> return result
```

No network request is allowed in the second branch.

Required cases:

- HTML text/markdown changes;
- include-links changes;
- max-char class changes supported by the raw body;
- PDF page-selection changes;
- other fields already present in `ExtractionCacheKey`.

Use `raw_entry.final_url` as the base URL for relative links.

### Step 9 — Handle 304 revalidation + derived miss correctly

For stale HTTP raw entries with validators:

```text
conditional HTTP request
  -> 304
     -> refresh raw freshness/headers as appropriate
     -> derived hit: return it
     -> derived miss: derive from preserved raw bytes
```

Do not force a full network body download after a successful 304 merely because the requested derived representation was not cached.

For `BrowserDom` raw entries:

- skip conditional HTTP revalidation entirely;
- follow the documented cache freshness behavior for browser DOM entries;
- once stale, perform a browser fetch according to the requested render/profile policy rather than pretending HTTP validators apply.

### Step 10 — Preserve transport metadata in derived cache entries

Extend `CachedExtractedDocument` only as needed to preserve:

```text
transport
browser_escalated
```

Profile display name should continue to come from the active request/profile resolution rather than being stored as a secret-bearing cache property.

On cache hit:

- `transport` must match the representation that produced the raw body;
- `browser_escalated` must remain accurate if the cached representation came from an auto-escalated browser result;
- existing `cache_status` remains `hit`/`revalidated` as appropriate.

Do not expose internal cache provenance fields beyond the existing MCP transport fields.

### Step 11 — Correct browser-login executable resolution

Resolve browser discovery once for the command using the configured executable argument.

Pass the resolved `BrowserDiscovery`/path into `launch_headed_browser`.

Do not call `discover_browser(None)` inside the launch helper.

Expected behavior:

- explicit valid configured executable -> launch exactly that executable;
- explicit invalid configured executable -> fail deterministically;
- no configured executable -> use normal auto-discovery;
- no available executable -> profile may remain created, but launch does not proceed.

### Step 12 — Correct browser-login completion interaction

Make the prompt and implementation agree.

Preferred implementation:

- wait for an actual newline/Enter from stdin;
- retain the configured process timeout as a safety bound;
- also allow Ctrl-C to abort if straightforward within the current CLI runtime.

Acceptable simpler implementation:

- explicitly tell the operator to press Ctrl-C when complete and treat that signal as successful completion.

Prefer Enter because it is clearer for manual profile setup, but do not introduce async-terminal complexity solely to satisfy that preference.

### Step 13 — Update documentation only after behavior is correct

Update active docs to state:

- profile-scoped browser fetches use the Eggsearch-owned `chrome-data` directory created by `browser-login`;
- anonymous browser fetches remain ephemeral;
- raw cache can satisfy a new extraction request without redownloading while fresh;
- browser raw cache stores rendered DOM, not subresource/network traces;
- cached browser results preserve `transport`/`browser_escalated` metadata;
- browser runtime configuration is honored by both anonymous and profile-scoped execution;
- browser-login uses the configured Chrome executable consistently.

Only mark the closure roadmap `Closed` after all acceptance criteria below are met.

---

## 5. Expected File Scope

Primary files:

```text
src/mcp/tools.rs
src/mcp/state.rs
src/fetch/client.rs
src/fetch/cache.rs
src/fetch/browser/lifecycle.rs
src/fetch/browser/navigate.rs
src/fetch/browser/profiles.rs
src/fetch/browser/types.rs
src/commands/browser_login.rs
src/core/fetch.rs
```

Likely documentation updates:

```text
README.md
docs/architecture/fetch.md
docs/config.md
docs/safety.md
plans/web-fetch-resilience-closure-roadmap.md
```

Optional small supporting changes:

```text
src/fetch/mod.rs
tests/browser_transport.rs
tests/browser_profiles.rs
tests/integration.rs
```

Do not modify unrelated search-provider, repository-search, evidence, release, or local-workspace code unless compilation forces a trivial mechanical update.

---

## 6. Explicit Non-Goals

This pass must not add:

- a multi-profile browser pool;
- remote browser services;
- browser process persistence across daemon restarts;
- cookie import/export APIs;
- cookie serialization through MCP;
- access to the operator's normal Chrome profile;
- automated credential entry;
- CAPTCHA/Turnstile solving or clicking;
- stealth/fingerprint mutation;
- proxy rotation;
- Playwright/Puppeteer/Node.js;
- browser downloads;
- SQLite/persistent cache;
- general RFC HTTP cache implementation;
- PDFium/OCR;
- a crawler;
- new CI workflows;
- browser-version/platform CI matrices;
- public-site integration tests;
- benchmarks or release gates;
- GitHub release automation.

---

## 7. Focused Verification

Use deterministic fixtures and existing test boundaries.

### 7.1 Persistent profile execution tests

Required tests:

1. profile-scoped browser execution receives the resolved profile `chrome-data` directory;
2. anonymous browser execution does not receive a persistent profile directory;
3. persistent execution does not mark the supplied directory for cleanup;
4. anonymous execution still cleans its temporary directory;
5. persistent execution does not use incognito launch semantics;
6. persistent navigation uses the default browser context rather than creating an isolated context;
7. anonymous navigation still uses an isolated context;
8. profile lock remains held for the entire profile-scoped browser call and is released on success;
9. profile lock is released when browser execution fails;
10. profile A state/path is never supplied to profile B execution.

Do not require a live authenticated public website.

Use seams/fakes around launch/context selection if necessary.

### 7.2 Raw-cache re-extraction tests

Required tests:

11. fresh cached HTML + missing derived text representation is derived without an HTTP request;
12. fresh cached HTML can be re-derived as markdown without an HTTP request;
13. fresh cached HTML can add link extraction without an HTTP request;
14. relative links derived from raw cache use `raw_entry.final_url` as base;
15. fresh cached PDF bytes can produce a different selected page range without an HTTP request;
16. newly derived cache entry is inserted and a second identical call becomes a derived hit;
17. raw bytes are not mutated by derivation;
18. password-protected/password-supplied PDF cache restrictions remain unchanged.

Use request counters in local deterministic fixtures to prove no second network request occurs.

### 7.3 Browser cache tests

Required tests:

19. successful browser result populates `raw_body` with rendered DOM bytes;
20. cacheable browser response stores non-empty DOM when DOM is non-empty;
21. browser raw entry provenance is `BrowserDom` or equivalent;
22. stale browser raw entry does not issue HTTP conditional revalidation;
23. cached browser result retains `transport="browser"`;
24. cached auto-escalated browser result retains `browser_escalated=true`;
25. challenge/manual-interaction browser outcomes are never inserted as successful cache entries.

### 7.4 Config propagation tests

Required tests:

26. custom navigation timeout reaches browser navigation;
27. custom post-load wait reaches browser navigation;
28. custom verification wait reaches browser navigation;
29. custom DOM byte limit is enforced instead of the default;
30. no normal execution path constructs `BrowserConfig::default()` when configured browser settings are available.

### 7.5 browser-login tests

Required tests:

31. configured explicit browser path is the executable passed to launch;
32. invalid explicit path does not fall back to auto-discovery;
33. auto-discovery remains available when no explicit path is configured;
34. displayed completion instruction matches the actual completion mechanism;
35. profile `chrome-data` directory used by login is the same path resolved for profile-scoped MCP execution.

### 7.6 Existing regression gates

Run the narrowest relevant test commands first, then the normal repository gate.

Recommended commands:

```bash
cargo test --locked --features browser --test browser_transport
cargo test --locked --features browser --test browser_profiles
cargo test --locked --all-features cache
cargo test --locked --all-features --test integration web_fetch
make check
```

Adjust filters only to match current repository test naming.

Do not add new CI workflows or a live browser test matrix.

The existing ignored local Chrome smoke may remain ignored.

---

## 8. Explicit Acceptance Criteria

This pass is complete only when every applicable item below is satisfied.

### Persistent profile correctness

- [ ] A `browser_profile` request resolves to the profile's opaque ID and Eggsearch-owned `chrome-data` directory.
- [ ] Profile-scoped headless Chrome launches with that exact persistent `chrome-data` directory.
- [ ] Profile-scoped execution does not silently use a temporary anonymous user-data directory.
- [ ] Profile-scoped execution does not force incognito mode.
- [ ] Profile-scoped navigation uses the default browser context so manual-login cookies/storage are available.
- [ ] Anonymous browser rendering remains ephemeral and isolated.
- [ ] Persistent profile directories are never removed by anonymous lifecycle cleanup/drop.
- [ ] The existing origin restriction remains enforced before launch.
- [ ] The profile lock covers the complete browser execution and is released on every exit path.
- [ ] No ordinary Chrome profile is read or modified.

### Cache correctness

- [ ] HTTP raw entries continue storing original bounded HTTP response bytes.
- [ ] Browser raw entries store the bounded rendered DOM bytes used for extraction.
- [ ] Browser raw entries never use an empty placeholder when non-empty DOM was rendered.
- [ ] Raw entries record enough transport provenance to distinguish HTTP from browser DOM.
- [ ] Fresh raw HTML + derived miss is re-extracted locally with zero network requests.
- [ ] Fresh raw PDF + different page selection is re-extracted locally with zero network requests.
- [ ] Fresh raw cache can satisfy changed extraction semantics represented by `ExtractionCacheKey` without redownloading.
- [ ] Newly re-derived representations are inserted into the derived cache.
- [ ] 304 + derived miss reuses preserved HTTP raw bytes instead of forcing a full body download.
- [ ] Browser DOM raw entries do not perform unsupported conditional HTTP revalidation.
- [ ] Existing `no-store`, `private`, `Vary`, password-PDF, error/challenge, and cache-scope policies remain intact.

### Transport/provenance correctness

- [ ] Derived cache responses preserve `transport` accurately.
- [ ] Derived cache responses preserve `browser_escalated` accurately.
- [ ] Cached anonymous HTTP content is not reported as browser content.
- [ ] Cached browser content is not reported as ordinary HTTP content.
- [ ] Profile display name remains operator-facing metadata only; opaque IDs/paths are not exposed through normal MCP output.

### Browser config correctness

- [ ] Browser execution uses configured runtime values rather than `BrowserConfig::default()`.
- [ ] Configured navigation timeout is honored.
- [ ] Configured post-load and verification waits are honored.
- [ ] Configured DOM/request/media bounds remain effective.
- [ ] Anonymous and profile-scoped browser execution use the same approved runtime safety limits unless an existing documented profile-specific setting says otherwise.

### browser-login correctness

- [ ] Explicit configured executable path is used for both discovery/preflight and launch.
- [ ] Invalid explicit executable path fails deterministically without silent auto-discovery.
- [ ] Auto-discovery still works when no explicit executable is configured.
- [ ] `browser-login` writes session state to the same `chrome-data` path later used by profile-scoped MCP fetching.
- [ ] Completion instructions match the implemented completion mechanism.
- [ ] Browser-login remains manual, local, and CLI-only.

### Scope/quality gates

- [ ] No multi-profile browser pool was introduced.
- [ ] No cookie export/import mechanism was introduced.
- [ ] No CAPTCHA/Turnstile automation, stealth, proxy rotation, or identity mutation was added.
- [ ] No runtime browser/PDF/OCR downloader was added.
- [ ] No new persistent cache or cache framework was added.
- [ ] PDF layout/OCR remains explicitly deferred.
- [ ] No new CI workflow, browser matrix, evidence ledger, benchmark gate, or release automation was added.
- [ ] Focused deterministic tests pass.
- [ ] Existing `make check` passes on the final implementation commit.
- [ ] The closure roadmap is marked `Closed` only after these criteria are satisfied.

---

## 9. Small-Model Execution Guidance

For implementation agents, treat this plan as a correctness patch, not a redesign.

Follow these rules:

1. Do not refactor unrelated code while touching `run_web_fetch`.
2. Prefer one small new enum/field over a generic browser-session abstraction.
3. Prefer a request-scoped profile browser lifecycle over a browser pool.
4. Do not duplicate the extraction pipeline. Extract a helper if needed.
5. Add tests immediately after each behavioral change instead of after all changes.
6. Keep profile display name and opaque cache ID as separate variables throughout.
7. Never pass the operator's ordinary Chrome directory into the new lifecycle mode.
8. Treat browser DOM as a distinct raw representation from HTTP response bytes.
9. Do not mark the roadmap closed based only on compilation or existing tests; specifically prove raw re-extraction and persistent profile path/context behavior.
10. Stop if the implementation begins requiring a new dependency, browser pool, cookie protocol, or general cache rewrite. Those are outside scope.

---

## 10. Suggested Commit Shape

Prefer one implementation commit for the full pass if it remains reviewable.

Acceptable split:

1. `fix(browser): use persistent profile state in profile-scoped fetches`
2. `fix(cache): rederive fresh raw entries and preserve browser provenance`
3. `fix(cli): align browser-login executable and completion behavior`
4. final docs/status adjustment if needed.

Do not split into additional phases.

---

## 11. Final Closure Definition

After this pass, the web-fetch resilience roadmap may be considered closed when the repository can truthfully demonstrate all of the following:

```text
HTTP default works
browser explicit works
browser auto one-shot escalation works
manual browser profile login is actually reused by MCP fetch
anonymous and profile browser state cannot cross
raw HTTP bytes are reusable
raw browser DOM is reusable
fresh raw cache can create a missing derived representation offline
PDF page reselection works from cached raw PDF
browser cache provenance survives cache hits
configured browser limits are honored
manual interaction remains manual
PDF OCR/layout remains deferred
verification remains lightweight
release remains manual
```

No further resilience sub-phase should be created merely for polish after these contracts are satisfied.
