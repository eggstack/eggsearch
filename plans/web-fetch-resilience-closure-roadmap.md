# web_fetch Resilience Closure Roadmap

**Repository:** `eggstack/eggsearch`  
**Planning baseline:** `2b95328e409e5f19074c1d8e2118fc4a7ce5561d`  
**Status:** Corrective implementation handoff  
**Scope:** Close the remaining browser-orchestration, cache-correctness, profile-isolation, and contract/documentation gaps from the PDF and browser resilience roadmap  
**Primary constraint:** Finish the implemented line of work without introducing a general crawler, heavy validation apparatus, runtime download system, or expanded CI matrix

---

## 1. Purpose

The original resilience roadmap produced useful implementation in four areas:

- PDF page selection, quality classification, metadata, outlines, labels, warnings, and bounded extraction;
- process-local origin concurrency, retry/backoff, circuit state, and cache primitives;
- an optional system-Chrome browser subsystem with public-network policy and challenge detection;
- explicit local persistent browser profiles with headed manual login and origin scoping.

The repository is not yet ready to declare that roadmap complete. The remaining defects are concentrated and should be closed directly rather than by adding another broad feature phase.

This roadmap defines three corrective passes:

1. wire the browser transport into the actual `web_fetch` orchestration path;
2. repair raw/derived cache semantics and profile isolation;
3. align response contracts, browser discovery behavior, documentation, and Phase 2 status, then close the line of work.

The passes are intentionally narrow. They should modify the existing fetch orchestration and cache/profile modules rather than introduce a replacement architecture.

---

## 2. Current State

### 2.1 Completed and retained

The following behavior should be treated as the stable baseline:

- default Cargo features remain empty;
- PDF support remains optional through `pdf`;
- Chrome support remains optional through `browser`;
- no browser, PDFium library, OCR model, Python runtime, or Node.js runtime is downloaded automatically;
- ordinary HTTP remains the default and preferred transport;
- browser rendering is public-network-only and bounded;
- interactive CAPTCHA/Turnstile challenges are detected but never solved;
- persistent browser profiles are created only through explicit local CLI actions;
- Eggsearch never uses the operator's ordinary Chrome profile;
- manual release remains outside GitHub CI;
- routine verification remains the existing `make check` path plus narrowly targeted tests.

### 2.2 Remaining correctness gaps

The closure work must address these concrete defects:

1. `run_web_fetch` validates `render` and resolves `browser_profile`, but the network attempt still uses the ordinary `FetchClient::fetch()` path. Browser rendering is therefore not operational through the MCP tool.
2. The cache's “raw” entry is populated from `resp.raw_text`, not the original fetched response bytes. PDFs and HTML cannot be re-extracted from cache under different options.
3. Derived cache entries are not explicitly scoped by anonymous/profile identity.
4. Profile cache scope currently uses the display name rather than the opaque profile ID.
5. Profile removal cannot completely invalidate derived entries because derived keys have no scope.
6. Raw-cache insertion/accounting is unnecessarily complex and may mishandle replacement or entries larger than the configured byte cap.
7. `manual_interaction_required` is present in responses but does not represent the actual challenge path.
8. An explicitly configured invalid Chrome path can silently fall back to auto-discovery, weakening operator intent and deterministic tests.
9. PDF layout/OCR Phase 2 remains unimplemented. This should be formally deferred rather than left appearing partially complete.
10. Current-head verification should be demonstrated through focused deterministic commands, not through a new evidence ledger or broad live test suite.

---

## 3. Fixed Closure Decisions

### 3.1 Browser orchestration remains one-shot

One logical request may use:

```text
ordinary HTTP
    -> optional single browser escalation
    -> terminal result
```

Do not alternate repeatedly between HTTP and browser. Browser escalation does not reset attempt counters or the logical request deadline.

### 3.2 Browser fallback is not access-control bypass

The browser path may render JavaScript and reuse an explicitly established local profile. It must not:

- solve or click interactive challenges;
- rotate proxies or identities;
- synthesize fingerprints;
- modify browser characteristics to evade detection;
- retry generic 401/403/429 responses through browser automatically;
- use a profile outside its recorded origin;
- render private, loopback, link-local, or metadata-service targets.

### 3.3 Raw cache means original bounded response bytes

A raw cache entry must contain the bounded body representation received from the selected transport before HTML/PDF extraction. It must not contain only extracted text.

The cache is still not an RFC-complete shared HTTP cache. It only needs correct Eggsearch-local semantics.

### 3.4 Profile scope uses opaque IDs

The display name is operator-facing metadata. Cache keys and invalidation use the profile's opaque immutable ID.

Recreating a removed profile with the same display name must not expose the old profile's cached content.

### 3.5 Phase 2 is deferred

Do not add PDFium, OCR models, model download code, native packaging, or an OCR CI matrix during closure.

The repository should explicitly document:

- PDF text extraction and quality reporting are supported;
- layout reconstruction and OCR are deferred;
- `pdf_ocr != never` is unavailable and returns a clear capability result;
- a future implementation requires a separate approved plan based on a concrete operational need.

### 3.6 Verification remains proportional

The closure does not justify:

- a new CI workflow;
- a browser-version/platform matrix;
- public-site tests;
- automated CAPTCHA/login tests;
- a test-evidence registry;
- long-running fuzz additions;
- benchmark gates;
- release automation.

Use deterministic local HTTP fixtures, fake browser backends where practical, a small optional local Chrome smoke, and the existing `make check` gate.

---

## 4. Pass Sequence

### Pass 1 — Browser transport orchestration

File: `plans/web-fetch-resilience-closure-pass-1-browser-orchestration.md`

Required result:

- `render=http_only|auto|browser` has real transport semantics;
- explicit profiles are passed into browser lifecycle/navigation;
- HTTP-to-browser escalation occurs only for documented classifications;
- challenge and unavailable-browser outcomes are structured;
- one deadline and attempt budget cover the complete request.

### Pass 2 — Cache and profile correctness

File: `plans/web-fetch-resilience-closure-pass-2-cache-and-profile-correctness.md`

Required result:

- raw cache stores original bounded bytes;
- derived cache is scope-aware;
- profile scope uses opaque IDs;
- removal invalidates both cache tiers;
- byte accounting and oversized-entry handling are correct;
- cached PDF/HTML content can be re-extracted under different options.

### Pass 3 — Contracts, documentation, and closure

File: `plans/web-fetch-resilience-closure-pass-3-contracts-and-finalization.md`

Required result:

- manual-interaction outcomes have one consistent machine-readable contract;
- explicit invalid browser paths fail deterministically;
- capability reporting reflects compiled, configured, discovered, and usable state;
- Phase 2 is explicitly deferred;
- stale claims are removed from active documentation;
- focused verification passes and the roadmap is marked closed.

---

## 5. Implementation Boundaries

Prefer changes in these existing areas:

```text
src/mcp/tools.rs
src/mcp/state.rs
src/fetch/client.rs
src/fetch/cache.rs
src/fetch/origin.rs
src/fetch/browser/*
src/core/fetch.rs
src/core/config.rs
src/core/warning.rs
src/commands/browser_login.rs
src/commands/browser_profiles.rs
README.md
docs/architecture/fetch.md
docs/config.md
docs/safety.md
docs/tool-matrix.md
docs/test-inventory.md
```

Avoid unrelated provider, search, evidence, repository, release, and local-workspace refactors.

A small internal orchestration helper is acceptable if it removes duplicated logic from `run_web_fetch`. Do not create a generic transport plugin framework.

---

## 6. Closure Gate

This line of work is complete only when all of the following are true:

- [ ] `render = browser` demonstrably invokes the browser transport.
- [ ] `render = auto` escalates only once and only for approved JavaScript/non-interactive verification classifications.
- [ ] `render = http_only` never launches Chrome.
- [ ] Browser profiles are resolved to opaque IDs and passed to the browser process through Eggsearch-owned data directories.
- [ ] Interactive challenges never trigger automated interaction.
- [ ] Raw cache entries contain original bounded response bytes.
- [ ] Derived entries are partitioned by anonymous/profile scope.
- [ ] Profile removal invalidates both raw and derived cache entries for its opaque scope.
- [ ] Cache accounting respects entry and total-byte limits without double insertion.
- [ ] Cached PDF bytes can be re-extracted for a different page selection without network access.
- [ ] Cached HTML bytes can be re-rendered for a different extraction mode without network access.
- [ ] Explicit invalid browser executable configuration fails rather than silently falling back.
- [ ] Manual interaction is represented through one documented machine-readable contract.
- [ ] Capability reporting does not claim browser usability merely because the feature was compiled.
- [ ] PDF layout/OCR is explicitly documented as deferred and unavailable.
- [ ] No new automatic downloads, bypass behavior, CI matrix, release workflow, or evidence ledger was added.
- [ ] Focused tests pass.
- [ ] `make check` passes on the final closure commit.

---

## 7. Recommended Commit Shape

Keep the handoff easy to review:

1. one commit for Pass 1 implementation and focused tests;
2. one commit for Pass 2 implementation and focused tests;
3. one commit for Pass 3 contracts/documentation and any final narrow corrections.

Small format/clippy follow-ups are acceptable, but avoid mixing unrelated cleanup into this line of work.

---

## 8. Explicit Non-Goals

Do not add:

- recursive crawling;
- browser pools across machines;
- remote browser services;
- Playwright/Puppeteer/Node.js;
- automated login or credential entry;
- CAPTCHA/Turnstile interaction;
- stealth plugins;
- proxy management;
- browser fingerprint mutation;
- general cookie import/export;
- ordinary Chrome-profile access;
- SQLite unless separately justified after memory-cache correctness;
- PDFium/OCR dependencies;
- OCR models or model downloaders;
- screenshots or binary artifacts in MCP responses;
- browser/PDF performance benchmarks as release gates;
- expanded CI or release automation.
