# web_fetch Resilience Closure Roadmap

**Repository:** `eggstack/eggsearch`  
**Planning baseline:** `2b95328e409e5f19074c1d8e2118fc4a7ce5561d`  
**Latest audited implementation baseline:** `ebcc9f3785a1a97e9f220956ea0268c16be7895d`  
**Status:** Final corrective closure pending  
**Scope:** Close the remaining browser-orchestration, cache-correctness, profile-isolation, and contract/documentation gaps from the PDF and browser resilience roadmap  
**Primary constraint:** Finish the implemented line of work without introducing a general crawler, heavy validation apparatus, runtime download system, or expanded CI matrix

---

## 1. Purpose

The resilience roadmap has produced useful implementation in four areas:

- PDF page selection, quality classification, metadata, outlines, labels, warnings, and bounded extraction;
- process-local origin concurrency, retry/backoff, circuit state, and cache primitives;
- optional system-Chrome rendering with explicit `http_only|auto|browser` transport behavior;
- explicit local persistent browser profiles with manual headed login, origin scoping, profile locking, and cache partitioning.

The first three closure passes corrected the majority of the original audit findings. A final audit at `ebcc9f3785a1a97e9f220956ea0268c16be7895d` found a small set of end-to-end correctness gaps that prevent this roadmap from being truthfully marked closed.

The remaining work is defined in one final narrow pass:

`plans/web-fetch-resilience-closure-pass-4-final-correctness.md`

No additional resilience phase should be created unless implementation of that pass reveals a genuinely new blocker.

---

## 2. Current State

### 2.1 Completed and retained

The following behavior is now the stable baseline and must not regress:

- default Cargo features remain empty;
- PDF support remains optional through `pdf`;
- Chrome support remains optional through `browser`;
- no browser, PDFium library, OCR model, Python runtime, or Node.js runtime is downloaded automatically;
- ordinary HTTP remains the default and preferred transport;
- `render=http_only` stays HTTP-only;
- `render=browser` invokes browser transport directly;
- `render=auto` performs at most one approved browser escalation;
- browser rendering is public-network-only and bounded;
- interactive CAPTCHA/Turnstile challenges are detected but never solved;
- browser/manual-interaction failures use machine-readable MCP error data;
- explicitly configured invalid browser paths fail deterministically;
- persistent browser profiles are created only through explicit local CLI actions;
- profile cache scope uses opaque IDs rather than display names;
- derived cache keys include anonymous/profile scope;
- raw HTTP cache entries store original bounded response bytes;
- raw LRU byte accounting and oversized-entry rejection are corrected;
- profile invalidation removes both raw and derived cache entries for that opaque scope;
- PDF layout reconstruction and OCR are explicitly deferred;
- manual release remains outside GitHub CI;
- routine verification remains the existing `make check` path plus narrowly targeted tests.

### 2.2 Remaining final correctness gaps

The final pass must close these concrete defects:

1. `browser_profile` is resolved, locked, and used for cache identity, but MCP browser execution still uses the shared anonymous browser lifecycle and its temporary user-data directory. Manual-login cookies/storage are therefore not actually reused by profile-scoped `web_fetch`.
2. Browser navigation always creates an isolated CDP browser context. A persistent-profile fetch must use the default context of a browser launched with the profile's Eggsearch-owned `chrome-data` directory.
3. Fresh raw-cache entries cannot currently produce a missing derived representation locally. A derived miss falls through to the network path rather than re-running extraction on cached raw bytes.
4. Browser result conversion currently drops the rendered DOM from `raw_body`; the cache writer can therefore store an empty raw body for a successful browser fetch.
5. Cache hits do not fully preserve `transport` / `browser_escalated` provenance.
6. Browser calls currently instantiate default navigation settings instead of consistently using the configured browser runtime values.
7. `browser-login` preflight and launch do not consistently use the same explicitly configured browser executable.
8. The browser-login completion prompt and completion mechanism do not agree.
9. Final closure must be demonstrated with focused deterministic tests and the existing `make check`, not with new CI/release infrastructure.

---

## 3. Fixed Closure Decisions

### 3.1 Browser orchestration remains one-shot

One logical request may use:

```text
ordinary HTTP
    -> optional single browser escalation
    -> terminal result
```

Do not alternate repeatedly between HTTP and browser. Browser escalation does not reset the logical request deadline.

### 3.2 Browser fallback is not access-control bypass

The browser path may render JavaScript and reuse an explicitly established local profile. It must not:

- solve or click interactive challenges;
- rotate proxies or identities;
- synthesize fingerprints;
- modify browser characteristics to evade detection;
- retry generic 401/403/429 responses through browser automatically;
- use a profile outside its recorded origin;
- render private, loopback, link-local, or metadata-service targets.

### 3.3 Anonymous and persistent browser state stay separate

Anonymous browser execution remains ephemeral and isolated.

Persistent profile execution must use the profile's Eggsearch-owned `chrome-data` directory established by `browser-login`.

Do not introduce a warm multi-profile browser pool during closure. A request-scoped profile browser process is acceptable and preferred for simplicity and isolation.

### 3.4 Raw cache means reusable transport representation

For HTTP transport, raw cache contains original bounded HTTP response bytes.

For browser transport, raw cache contains the bounded rendered DOM bytes used for extraction.

A fresh raw entry must be sufficient to derive another supported representation without another network fetch.

### 3.5 Profile scope uses opaque IDs

The display name is operator-facing metadata. Cache keys and invalidation use the profile's opaque immutable ID.

Recreating a removed profile with the same display name must not expose the old profile's cached content.

### 3.6 Phase 2 remains deferred

Do not add PDFium, OCR models, model download code, native packaging, or an OCR CI matrix during closure.

Supported PDF behavior remains text extraction plus quality classification. Layout reconstruction and OCR require a separate future approved plan based on concrete need.

### 3.7 Verification remains proportional

The closure does not justify:

- a new CI workflow;
- a browser-version/platform matrix;
- public-site tests;
- automated CAPTCHA/login tests;
- a test-evidence registry;
- long-running fuzz additions;
- benchmark gates;
- release automation.

Use deterministic local fixtures, browser seams/fakes where practical, the existing ignored local Chrome smoke where useful, and the normal `make check` gate.

---

## 4. Pass Sequence and Status

### Pass 1 — Browser transport orchestration

File: `plans/web-fetch-resilience-closure-pass-1-browser-orchestration.md`

Status: **Implemented with final follow-up required for persistent-profile state reuse.**

Landed behavior includes:

- real `http_only|auto|browser` transport selection;
- direct browser mode;
- one-shot auto escalation;
- shared logical deadline behavior;
- structured browser availability/challenge handling.

### Pass 2 — Cache and profile correctness

File: `plans/web-fetch-resilience-closure-pass-2-cache-and-profile-correctness.md`

Status: **Mostly implemented with final follow-up required for raw-cache re-derivation and browser raw/provenance handling.**

Landed behavior includes:

- raw HTTP bytes rather than extracted text;
- derived scope isolation;
- opaque profile ID cache scope;
- both-tier scope invalidation;
- corrected raw-cache byte accounting.

### Pass 3 — Contracts, documentation, and closure

File: `plans/web-fetch-resilience-closure-pass-3-contracts-and-finalization.md`

Status: **Implemented, but closure declaration was premature.**

Landed behavior includes:

- structured manual-interaction contracts;
- deterministic explicit browser-path handling;
- structured capability reporting;
- formal PDF layout/OCR deferral;
- documentation alignment.

### Pass 4 — Final correctness

File: `plans/web-fetch-resilience-closure-pass-4-final-correctness.md`

Status: **Pending implementation.**

Required result:

- manual-login state is actually reused by profile-scoped MCP browser fetching;
- persistent and anonymous browser contexts cannot cross;
- fresh raw HTML/PDF entries can produce missing derived representations without a network request;
- browser DOM bytes are stored as the browser raw representation;
- browser transport provenance survives cache hits;
- configured browser runtime limits are used by execution;
- browser-login uses one consistent executable and completion contract;
- focused tests and `make check` pass.

---

## 5. Implementation Boundaries

Prefer changes in these existing areas:

```text
src/mcp/tools.rs
src/mcp/state.rs
src/fetch/client.rs
src/fetch/cache.rs
src/fetch/browser/lifecycle.rs
src/fetch/browser/navigate.rs
src/fetch/browser/profiles.rs
src/fetch/browser/types.rs
src/core/fetch.rs
src/commands/browser_login.rs
README.md
docs/architecture/fetch.md
docs/config.md
docs/safety.md
```

Avoid unrelated provider, search, evidence, repository, release, and local-workspace refactors.

A small internal derivation helper or browser execution-mode enum is acceptable if it directly removes duplication or makes the anonymous/persistent boundary explicit.

Do not create a generic transport plugin framework, browser pool, cookie protocol, or replacement cache architecture.

---

## 6. Final Closure Gate

This line of work is complete only when all of the following are true:

- [x] `render = browser` invokes the browser transport.
- [x] `render = auto` escalates only once and only for approved JavaScript/non-interactive verification classifications.
- [x] `render = http_only` never launches Chrome.
- [ ] Browser profiles are resolved to opaque IDs and the associated Eggsearch-owned `chrome-data` directory is actually used by profile-scoped browser execution.
- [ ] Persistent profile browser execution uses session-bearing default profile context rather than an isolated anonymous context.
- [x] Interactive challenges never trigger automated interaction.
- [x] HTTP raw cache entries contain original bounded response bytes.
- [ ] Browser raw cache entries contain the bounded rendered DOM bytes rather than an empty placeholder.
- [x] Derived entries are partitioned by anonymous/profile scope.
- [x] Profile removal invalidates both raw and derived cache entries for its opaque scope.
- [x] Cache accounting respects entry and total-byte limits without double insertion.
- [ ] Cached PDF bytes can be re-extracted for a different page selection without network access.
- [ ] Cached HTML bytes can be re-rendered for a different extraction mode without network access.
- [ ] Browser cache hits preserve `transport` and `browser_escalated` provenance.
- [ ] Browser execution honors configured navigation/runtime limits rather than default values.
- [x] Explicit invalid browser executable configuration fails rather than silently falling back.
- [ ] `browser-login` launches the same explicitly configured executable recognized during discovery/preflight.
- [ ] `browser-login` completion instructions match the actual completion mechanism.
- [x] Manual interaction is represented through one documented machine-readable contract.
- [x] Capability reporting does not claim browser usability merely because the feature was compiled.
- [x] PDF layout/OCR is explicitly documented as deferred and unavailable.
- [x] No new automatic downloads, bypass behavior, CI matrix, release workflow, or evidence ledger has been added.
- [ ] Focused final-corrective tests pass.
- [ ] `make check` passes on the final closure commit.

Only after every unchecked item above is satisfied should this file's status be changed to `Closed` again.

---

## 7. Explicit Non-Goals

Do not add:

- recursive crawling;
- browser pools across machines;
- multi-profile warm browser pooling;
- remote browser services;
- Playwright/Puppeteer/Node.js;
- automated login or credential entry;
- CAPTCHA/Turnstile interaction;
- stealth plugins;
- proxy management;
- browser fingerprint mutation;
- general cookie import/export;
- ordinary Chrome-profile access;
- SQLite/persistent cache;
- general shared HTTP cache semantics;
- PDFium/OCR dependencies;
- OCR models or model downloaders;
- screenshots or binary artifacts in MCP responses;
- browser/PDF performance benchmarks as release gates;
- expanded CI or release automation.

---

## 8. Closure Handoff

Implementation should proceed directly from:

`plans/web-fetch-resilience-closure-pass-4-final-correctness.md`

The pass contains the file-level guidance, execution order, deterministic test requirements, explicit acceptance criteria, small-model constraints, and final closure definition.

No further roadmap expansion is expected after Pass 4 unless a concrete implementation blocker is discovered.
