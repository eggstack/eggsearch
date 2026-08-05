# Closure Pass 3 — Contracts, Documentation, and Finalization

**Repository:** `eggstack/eggsearch`  
**Roadmap:** `plans/web-fetch-resilience-closure-roadmap.md`  
**Predecessors:**

- `plans/web-fetch-resilience-closure-pass-1-browser-orchestration.md`
- `plans/web-fetch-resilience-closure-pass-2-cache-and-profile-correctness.md`

**Planning baseline:** `2b95328e409e5f19074c1d8e2118fc4a7ce5561d`  
**Status:** Corrective implementation handoff  
**Scope:** Normalize agent-facing outcomes, make browser discovery deterministic, formally defer PDF layout/OCR, align active documentation, and close the resilience line with proportional verification

---

## 1. Objective

Complete the resilience roadmap after the browser-orchestration and cache-correctness passes are implemented.

This pass should not add major functionality. It closes contract and documentation inconsistencies that would otherwise make the implemented behavior difficult for codegg or another MCP client to use reliably.

Required outcomes:

1. manual browser interaction and profile-attention states use one machine-readable contract;
2. explicit browser executable configuration is authoritative and deterministic;
3. capability reporting separates compile, configuration, discovery, and usability state;
4. PDF layout/OCR Phase 2 is formally deferred rather than implied to be partially available;
5. documentation reflects actual behavior and does not overclaim browser/profile/cache support;
6. focused tests and the existing routine gate establish closure without adding verification bureaucracy.

---

## 2. Normalize Manual-Interaction Outcomes

### 2.1 Current inconsistency

The baseline response includes `manual_interaction_required`, but ordinary successful responses initialize it to `false`, while interactive challenge cases return an internal tool error string such as `browser_profile_requires_attention`.

This creates two competing contracts and makes the response field ineffective.

### 2.2 Choose one primary contract

Use a structured tool error for terminal browser/manual-attention outcomes. This best matches existing MCP error handling because there is no useful fetched document to return.

Recommended fields:

```text
code
message
origin
profile_name          optional
manual_interaction_required = true
next_action           optional bounded operator guidance
```

Recommended error codes:

```text
browser_manual_interaction_required
browser_profile_requires_attention
browser_unavailable
browser_startup_failed
browser_navigation_failed
browser_policy_blocked
browser_deadline_exceeded
```

If the existing `ToolError` type cannot carry structured data without disproportionate changes, return a stable JSON error payload through the repository's established MCP error mechanism. Do not rely solely on parsing prose prefixes.

### 2.3 Success response

On successful fetches:

- omit `manual_interaction_required`, or return `false` consistently if schema stability requires it;
- include `transport` and `browser_escalated` from Pass 1;
- include profile display name only when a persistent profile was actually used;
- never include profile path, opaque ID, cookies, storage, or browser debug data.

### 2.4 Operator guidance

For a selected profile that needs attention, guidance may include:

```text
eggsearch browser-login <origin> --profile <name>
```

This is guidance for the local operator. It must not imply that the MCP caller can launch the headed browser or supply credentials.

---

## 3. Deterministic Browser Executable Semantics

### 3.1 Explicit path is authoritative

Define discovery behavior exactly:

```text
configured executable present and valid
    -> use it
configured executable present but invalid/unusable
    -> explicit configuration error
no configured executable
    -> auto-discover supported system Chrome/Chromium
```

Do not silently ignore an invalid explicit path and fall back to a different browser from `PATH`.

Reasons:

- operator typos should not be hidden;
- deterministic deployment matters more than convenience;
- tests should not depend on whether a CI host happens to have Chrome installed;
- a configured browser may have been selected for version/policy reasons.

### 3.2 Discovery result

Keep discovery output bounded:

```rust
pub enum BrowserDiscoveryState {
    Available(BrowserDiscovery),
    NotConfigured,
    ExplicitPathInvalid { path: RedactedPathOrDisplaySafePath },
    NotFound,
    VersionUnsupported { version: String },
}
```

Exact type is flexible. Do not expose environment dumps or scan arbitrary filesystem locations.

### 3.3 Tests

Replace weakened “does not panic” assertions with deterministic tests using an injected discovery environment or explicit helper inputs.

Required behavior tests:

- invalid explicit path returns `ExplicitPathInvalid` even if Chrome exists elsewhere;
- no explicit path permits auto-discovery;
- configured valid path takes precedence over auto-discovery;
- empty configured string follows one documented rule—prefer treating it as absent after config normalization;
- unsupported browser family/version is reported clearly;
- tests do not depend on the CI runner's installed browser.

Do not add a platform/browser matrix.

---

## 4. Capability Reporting

### 4.1 Browser capability dimensions

`provider_status` should not report a single optimistic boolean derived only from feature compilation or executable presence.

Recommended compact structure:

```json
{
  "browser_rendering": {
    "compiled": true,
    "configured": true,
    "browser_discovered": true,
    "usable": true,
    "reason": null
  },
  "persistent_browser_profiles": {
    "compiled": true,
    "configured": false,
    "usable": false,
    "reason": "disabled"
  }
}
```

If preserving an existing boolean field is necessary, keep it as `usable` and add a compact detail object. Avoid a breaking schema change unless repository conventions allow it.

Do not launch a real page or headed browser from `provider_status`.

### 4.2 PDF capability dimensions

Report truthfully:

```text
pdf_text: compiled/configured/usable
pdf_layout: deferred/unavailable
pdf_ocr: deferred/unavailable
```

Do not claim `pdf_ocr` is usable because the request enum accepts `auto|always`. Those values are policy inputs, not implemented capability.

### 4.3 Cache capability

A small status block may report:

```text
memory_cache_enabled
persistent_cache = false
profile_scoping = true
```

Do not expose entry contents, URLs, byte counts by profile, cache paths, or opaque identifiers through ordinary provider status.

---

## 5. Formally Defer PDF Layout and OCR

### 5.1 Decision

Mark the original Phase 2 plan as deferred after investigation/implementation review.

Rationale:

- current `lopdf` extraction plus quality classification provides a useful lightweight baseline;
- PDFium introduces native runtime discovery/distribution complexity;
- OCR requires model files, loading behavior, additional dependencies, and platform packaging decisions;
- this complexity is disproportionate without a demonstrated codegg workflow requiring scanned-PDF recovery;
- automatic downloads are prohibited by project direction.

### 5.2 Documentation status

Update the original Phase 2 plan header or add a concise status note:

```text
Status: Deferred — not required for current closure
Reason: native/model distribution complexity exceeds current demonstrated need
Re-entry condition: a concrete scanned-PDF workload and approved dependency/runtime plan
```

Do not delete the plan. It remains useful research and future design guidance.

### 5.3 Active behavior

Retain:

- `pdf_ocr=never` default;
- explicit capability error or warning for `auto|always`;
- per-page quality metadata indicating scanned/CID-corrupt/unusable content;
- actionable statement that OCR is unavailable.

Do not add a fake OCR fallback, shell out to Tesseract, call a remote OCR API, or download models during this pass.

---

## 6. Documentation Alignment

Review and update only active affected documents:

```text
README.md
AGENTS.md
docs/architecture/fetch.md
docs/config.md
docs/safety.md
docs/tool-matrix.md
docs/test-inventory.md
plans/web-fetch-pdf-and-browser-resilience-roadmap.md
plans/web-fetch-resilience-phase-2-pdf-layout-and-ocr.md
```

Required corrections:

1. state that `render=http_only|auto|browser` is operational only after Pass 1;
2. describe exactly when `auto` escalates and when it does not;
3. state that profiles require explicit headed local setup and exact-origin reuse;
4. state that challenge interaction is manual and never automated;
5. describe raw/derived in-memory cache semantics after Pass 2;
6. state that profile cache isolation uses opaque IDs internally;
7. remove any claim that deleting a profile invalidates cache unless the integrated process actually performs it;
8. document process-local cache/invalidation limitations;
9. state that PDF layout/OCR is deferred;
10. preserve manual release and lightweight default-build guidance.

Keep documentation operational. Do not add an evidence report, implementation diary, or separate release checklist for this closure.

---

## 7. Final Code Review Checklist

Perform one focused review of the completed line of work.

### 7.1 Browser execution

- no path silently ignores `render`;
- no browser launch occurs under `http_only`;
- `auto` escalates at most once;
- explicit browser mode does not downgrade silently;
- profile exact-origin and lock rules are enforced;
- no challenge clicking/solving code exists;
- browser cleanup runs on success, error, timeout, and cancellation.

### 7.2 Cache

- raw cache stores original bounded bytes/DOM;
- derived keys include scope;
- opaque profile ID is used internally;
- invalidation covers both tiers;
- byte accounting cannot exceed the cap;
- challenge/login/error/truncated/password-PDF results are not cached incorrectly;
- stale browser DOM is not conditionally validated as HTTP.

### 7.3 Contracts

- tool errors have stable codes;
- success metadata is truthful;
- capability reporting is conservative;
- no secret/path/profile-ID leakage occurs;
- documentation and schemas agree.

Fix only defects found in these areas. Do not use this review to refactor unrelated modules.

---

## 8. Proportional Verification

### 8.1 Required targeted commands

Run the focused suites introduced or affected by the closure:

```bash
cargo test --locked --features browser --test browser_transport
cargo test --locked --features browser --test browser_profiles
cargo test --locked --all-features --test integration web_fetch
cargo test --locked --all-features cache
```

Use repository-appropriate filters when exact test names differ.

### 8.2 Routine gate

Run once after focused tests:

```bash
make check
```

Do not add:

- a new workflow;
- repeated feature-combination matrices beyond the repository's current gate;
- public-site browser tests;
- authentication/CAPTCHA automation;
- long live browser tests in routine CI;
- release checks to routine iteration;
- evidence artifacts or signed verification records.

### 8.3 Optional local smoke

A manual local smoke may cover:

```text
simple HTTP page under http_only
a local fixture requiring JavaScript under browser mode
a local fixture causing auto escalation
profile setup against a controlled local/public test origin
```

Do not use a third-party protected site as acceptance evidence.

---

## 9. Final Acceptance Criteria

- [ ] Manual-interaction and profile-attention outcomes use stable machine-readable error codes.
- [ ] Success responses do not carry misleading manual-interaction state.
- [ ] Explicit invalid browser executable paths fail deterministically.
- [ ] Auto-discovery occurs only when no explicit executable is configured.
- [ ] Browser discovery tests are independent of CI host installation.
- [ ] Capability reporting distinguishes compiled, configured, discovered, and usable state.
- [ ] Browser capability does not launch a page during status reporting.
- [ ] PDF layout/OCR is explicitly marked deferred and unavailable.
- [ ] `pdf_ocr=auto|always` does not pretend to execute OCR.
- [ ] Active documentation matches the completed Pass 1 and Pass 2 behavior.
- [ ] No documentation overclaims profile invalidation, cache semantics, or browser fallback.
- [ ] No CAPTCHA solving, stealth, proxy, downloader, OCR runtime, or CI matrix was added.
- [ ] Focused test suites pass.
- [ ] `make check` passes on the final commit.
- [ ] The original resilience roadmap is updated to `Closed` with Phase 2 noted as deferred.

---

## 10. Closure Statement Template

When implementation is complete, update the original roadmap with a short factual closure section:

```text
Closed at: <commit>

Implemented:
- Phase 1 PDF quality/navigation
- process-local origin control and correct in-memory raw/derived caching
- optional system-Chrome rendering through web_fetch
- explicit local persistent browser profiles

Deferred:
- PDF layout reconstruction and OCR

Preserved constraints:
- empty default features
- no runtime downloads
- no automated challenge solving
- manual crates.io release
- one routine make check gate
```

Do not include generated test transcripts or mutable evidence ledgers.
