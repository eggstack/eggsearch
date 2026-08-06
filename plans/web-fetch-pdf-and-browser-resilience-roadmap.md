# web_fetch PDF and Browser Resilience Roadmap

**Repository:** `eggstack/eggsearch`  
**Planning baseline:** `699e25fd3dff5980629514dee4746fba581f7905`  
**Status:** Implementation handoff  
**Scope:** Expanded PDF interpretation, bounded fetch resilience, optional system-Chrome rendering, and local manual browser sessions  
**Reference reviewed:** `dondai1234/master-fetch`  
**Primary constraint:** Preserve Eggsearch as a lightweight, keyless, local-first MCP server

---

## 1. Purpose

This roadmap extends `web_fetch` in the areas where the current implementation has the largest practical gaps for coding and research agents:

1. PDF extraction should report structure and extraction quality rather than only returning page-indexed plain text.
2. Scanned or font-corrupted PDF pages should be recoverable through an optional, page-local OCR path.
3. Repeated requests should respect origin backoff, conditional caching, and bounded concurrency.
4. JavaScript-rendered public pages should be fetchable through an already-installed Google Chrome or Chromium binary when ordinary HTTP extraction is insufficient.
5. A local operator should be able to establish a dedicated browser session manually for sites that require login or an interactive verification step.

The target is not a general crawler, stealth browser, CAPTCHA solver, proxy rotator, or Cloudflare circumvention framework. Eggsearch should render public pages through a normal locally installed browser and should stop cleanly when interactive human verification is required.

---

## 2. Current Baseline

Eggsearch already has the correct core trust and resource boundaries:

- one explicit HTTP(S) URL per `web_fetch` call;
- manual redirect handling;
- URL, DNS, and resolved-address validation;
- private-network and localhost restrictions by default;
- streaming response byte caps;
- bounded extraction output;
- prompt-injection framing and marker scanning;
- structured `FetchDocument` output;
- optional `pdf` support using `lopdf`;
- lightweight default features;
- one routine repository verification path through `make check`.

This line of work must preserve those properties. New transports and extractors are subordinate to the existing fetch policy rather than alternative policy implementations.

---

## 3. Fixed Decisions

### 3.1 Lightweight defaults remain authoritative

The default build must not download or bundle a browser, OCR model, PDFium library, SQLite runtime, Node.js runtime, or Python environment.

Heavy capabilities must remain optional Cargo features or runtime integrations:

```text
pdf              existing lightweight lopdf extraction
pdf-layout       optional page rendering/layout backend
pdf-ocr          optional page OCR backend
browser          optional Chrome DevTools Protocol transport
cache-sqlite     optional persistent cache
```

The exact feature names may be adjusted if the implementation finds a clearer naming scheme, but the dependency separation must remain.

### 3.2 System Chrome is the browser fallback

When browser support is enabled, Eggsearch should discover and use an already-installed full Google Chrome or Chromium executable. It must not automatically download Chromium.

Preferred discovery order:

```text
google-chrome-stable
google-chrome
chromium
chromium-browser
standard macOS Chrome/Chromium application paths
standard Windows Chrome/Chromium installation paths
```

`chrome-headless-shell` and alternative browser engines are out of scope for the initial implementation.

### 3.3 Browser rendering is classification-driven

Ordinary HTTP fetching remains the default. Automatic browser escalation is allowed only after the HTTP response is classified as a plausible JavaScript shell, incomplete client-rendered document, or recognized non-interactive browser verification page.

Do not automatically escalate:

- HTTP 401;
- ordinary HTTP 403;
- HTTP 404;
- HTTP 429;
- generic 5xx responses;
- explicit CAPTCHA or interactive Turnstile pages;
- repeated failures from an origin whose circuit is open.

At most one browser escalation is allowed per logical fetch request.

### 3.4 No automated challenge interaction

Do not implement:

- Turnstile checkbox clicking;
- CAPTCHA solving;
- human-like mouse simulation;
- randomized browser identities;
- synthetic navigator, canvas, WebGL, or font fingerprints;
- rotating proxies;
- fake referrer generation;
- recursive challenge-solving loops.

A non-interactive challenge may be allowed to resolve naturally in Chrome. An interactive challenge must produce a structured `manual_interaction_required` result.

### 3.5 PDF OCR is page-local and quality-driven

OCR must not become the default PDF extractor. The normal sequence is:

```text
metadata/outline inspection
    -> lightweight page text extraction
    -> per-page quality classification
    -> optional rendering/OCR only for degraded pages
    -> structured document assembly
```

`ocr = auto` must render only pages classified as scanned, image-only, or severely font-corrupted. Whole-document OCR is allowed only when explicitly requested and bounded by configured page and pixel limits.

### 3.6 Verification remains proportionate

This roadmap does not authorize a new matrix-heavy CI system, a browser farm, external challenge-site tests, OCR benchmark infrastructure, or mutable evidence ledgers.

Each phase should use:

- focused unit tests for pure classification and parsing logic;
- a small number of deterministic local fixtures;
- targeted feature compilation when optional dependencies are added;
- the existing `make check` routine gate.

Live browser and real-site checks are maintainer-run diagnostics and must not block ordinary CI.

---

## 4. Target Architecture

The final fetch flow should be conceptually:

```text
web_fetch request
    |
    v
validate URL and policy
    |
    v
cache lookup / origin gate
    |
    v
HTTP transport
    |
    +--> PDF bytes --> PDF inspection/text/layout/OCR pipeline
    |
    +--> useful HTML/text --> existing document extraction
    |
    +--> JS shell or non-interactive browser verification
              |
              v
        optional system-Chrome transport
              |
              +--> rendered DOM --> existing HTML document extraction
              +--> interactive challenge --> manual_interaction_required
              +--> denial/rate limit --> bounded failure + origin backoff
```

New internal boundaries should remain small:

```text
src/fetch/
    client.rs                 orchestration and response assembly
    transport/
        mod.rs                transport result contract
        classify.rs           response/escalation classification
        browser.rs            optional system-Chrome transport
        origin.rs             backoff/concurrency state
    cache/
        mod.rs                cache contract and memory implementation
        sqlite.rs             optional persistent implementation
    pdf/
        mod.rs                backend-independent pipeline
        text.rs               lopdf fast path
        quality.rs            page quality classification
        layout.rs             optional renderer/layout backend
        ocr.rs                optional OCR backend
```

This is a target organization, not a requirement to move files mechanically. Avoid refactoring unrelated fetch code solely to match the diagram.

---

## 5. Public Request and Response Direction

The existing minimal request must remain valid:

```json
{ "url": "https://example.com" }
```

The line of work may add these bounded optional fields to `WebFetchArgs`:

```text
pages             PDF page selection such as "1-5,8,12-14"
pdf_ocr           never | auto | always
pdf_password      optional password, never logged or cached by default
include_media     include bounded page/image metadata
render            http_only | auto | browser
cache             default | bypass | refresh
browser_profile   optional named operator-created Eggsearch profile
```

The exact schema may be split into nested option structs if that improves MCP schema clarity. Do not add a separate tool merely to avoid extending `web_fetch` unless implementation demonstrates that the resulting schema becomes materially confusing.

Responses should expose structured provenance rather than hiding fallback behavior:

```text
transport_used         http | browser
browser_escalated      true/false
browser_reason         js_shell | explicit | verification_page | null
cache_status           hit | miss | revalidated | bypassed
pdf_quality_score      0.0..1.0
pdf_content_ok         true/false
pdf_ocr_pages          page numbers
pdf_page_metadata      bounded per-page status records
manual_interaction     structured reason when required
```

All extracted and rendered content remains `external_untrusted` and passes the existing sanitation path.

---

## 6. Phases

### Phase 1 — PDF quality, navigation, and request contract

Improve the existing `lopdf` path without adding a heavyweight runtime. Add page-range selection, richer metadata, outline extraction where feasible, per-page quality classification, scanned/CID-corruption warnings, honest document-level quality, and the minimal request/response fields required by later phases.

Detailed plan: `plans/web-fetch-resilience-phase-1-pdf-quality-and-navigation.md`.

### Phase 2 — Optional PDF rendering and OCR

Add optional page rendering/layout and OCR backends. Render only selected degraded pages under `auto`, preserve page provenance, and keep layout/OCR dependencies outside the default build. Complex semantic table reconstruction is explicitly deferred.

Detailed plan: `plans/web-fetch-resilience-phase-2-pdf-layout-and-ocr.md`.

### Phase 3 — Origin resilience and caching

Add per-origin concurrency, `Retry-After` handling, bounded jittered backoff, a short circuit breaker, conditional HTTP revalidation, and a two-layer raw/derived cache. Begin with memory caching; add SQLite only as an optional persistence backend if the memory contract is stable.

Detailed plan: `plans/web-fetch-resilience-phase-3-origin-control-and-cache.md`.

### Phase 4 — Optional system-Chrome rendering

Add Chrome/Chromium discovery, one warm browser process, isolated ephemeral contexts, CDP-controlled navigation, request interception, strict resource limits, rendered-DOM extraction, and classification-driven HTTP-to-browser escalation.

Detailed plan: `plans/web-fetch-resilience-phase-4-system-chrome-rendering.md`.

### Phase 5 — Manual local sessions and closure

Add an explicit headed `browser-login` workflow using Eggsearch-owned profiles, origin-scoped session reuse, challenge-aware structured outcomes, final documentation, and a bounded local diagnostic command. No automated challenge solving is permitted.

Detailed plan: `plans/web-fetch-resilience-phase-5-manual-sessions-and-closure.md`.

---

## 7. Phase Ordering and Dependencies

```text
Phase 1
  |
  +--> Phase 2
  |
  +--> Phase 3
          |
          v
        Phase 4
          |
          v
        Phase 5
```

Phase 2 and Phase 3 may proceed independently after Phase 1 if separate implementers are available. Phase 4 should use the origin-control and cache contracts established in Phase 3. Phase 5 depends on Phase 4.

Do not begin Phase 4 by embedding retry, cache, and origin state directly in the browser module; those concerns belong to Phase 3.

---

## 8. Verification Policy for All Phases

The routine gate is:

```bash
make check
```

This already covers format, clippy, no-default feature compilation, and one all-features deterministic test pass. Do not add duplicate full-suite commands to individual phase plans.

Targeted commands are allowed when a phase introduces a new optional feature, for example:

```bash
cargo check --locked --features pdf-layout
cargo test --locked --features pdf-ocr --test pdf_extraction
cargo check --locked --features browser
cargo test --locked --features browser --test browser_transport
```

Use the narrowest existing or newly added test target that proves the phase behavior. Avoid adding tests that merely restate type definitions or count files.

Live checks should be manual and advisory:

```text
one local text PDF
one local scanned PDF
one local deterministic JS fixture server
one ordinary public JS-rendered documentation page
one manual profile login smoke test
```

Do not place external Cloudflare, CAPTCHA, or authentication sites in CI.

---

## 9. Global Acceptance Criteria

This roadmap is complete when:

- [ ] default builds remain browser-free, OCR-free, PDFium-free, and SQLite-free;
- [ ] existing `web_fetch` calls remain compatible;
- [ ] PDF responses expose page selection, quality, and honest failure information;
- [ ] optional OCR is page-local under `auto` and reports which pages were replaced or augmented;
- [ ] origin rate limits and repeated failures cause backoff rather than aggressive retries;
- [ ] cache entries distinguish raw bytes from derived extraction and do not mix browser profiles;
- [ ] installed Chrome/Chromium can render a deterministic local JavaScript fixture;
- [ ] browser mode cannot access destinations prohibited by the browser transport policy;
- [ ] automatic rendering never clicks or solves interactive challenges;
- [ ] operators can create a dedicated Eggsearch browser profile through an explicit headed workflow;
- [ ] interactive challenges return a structured manual-action result;
- [ ] `make check` passes;
- [ ] no new CI matrix, scheduled live test, browser download, or release automation is introduced.

---

## 10. Explicitly Deferred Work

The following may be reconsidered only after this roadmap is implemented and used in practice:

- advanced PDF table reconstruction;
- mathematical formula reconstruction or LaTeX recovery;
- image captioning or multimodal figure interpretation;
- Firefox, WebKit, or Edge-specific transports;
- bundled browser management;
- remote browser services;
- proxy management;
- browser fingerprint synthesis;
- CAPTCHA-solving integrations;
- crawling or recursive link traversal;
- shared multi-user browser profiles;
- distributed cache or origin coordination;
- CI browser matrices;
- OCR quality benchmark suites.

These are not hidden acceptance criteria for this line of work.

---

## Closure

**Closed at:** Pass 3 contracts and finalization

**Implemented:**
- Phase 1 PDF quality/navigation
- process-local origin control and correct in-memory raw/derived caching
- optional system-Chrome rendering through `web_fetch`
- explicit local persistent browser profiles

**Deferred:**
- PDF layout reconstruction and OCR (Phase 2)

**Preserved constraints:**
- empty default features
- no runtime downloads
- no automated challenge solving
- manual crates.io release
- one routine `make check` gate