# Closure Pass 1 — Browser Transport Orchestration

**Repository:** `eggstack/eggsearch`  
**Roadmap:** `plans/web-fetch-resilience-closure-roadmap.md`  
**Planning baseline:** `2b95328e409e5f19074c1d8e2118fc4a7ce5561d`  
**Status:** Corrective implementation handoff  
**Scope:** Make the existing optional Chrome subsystem operational through `web_fetch` without expanding into a general browser automation framework

---

## 1. Objective

Wire the already-implemented browser modules into the real MCP fetch path.

At the baseline, `run_web_fetch` validates `render`, optionally resolves a persistent profile, and then performs the request through the ordinary `FetchClient::fetch()` path. The browser subsystem is therefore structurally present but not functionally selected by `web_fetch`.

This pass must make transport selection explicit and observable:

```text
http_only -> ordinary HTTP only
auto      -> ordinary HTTP, then at most one approved browser escalation
browser   -> browser transport directly
```

The completed path must retain the existing URL policy, output limits, sanitization, origin control, cache policy, and logical deadline.

---

## 2. Fixed Semantics

### 2.1 `http_only`

- Never launch or connect to Chrome.
- Use the existing HTTP client.
- Preserve current behavior for HTML, text, code-host transformations, and PDFs.
- Browser profiles are invalid with `http_only`; reject the contradictory request clearly rather than silently ignoring the profile.

### 2.2 `browser`

- Require the `browser` feature, browser configuration enabled, and a usable discovered/configured Chrome/Chromium executable.
- Validate the target using the browser network policy before launch/navigation.
- Invoke the browser transport directly.
- Use an ephemeral Eggsearch-owned user-data directory unless an explicit persistent profile is selected.
- Do not make a preliminary ordinary HTTP request merely to classify the page.
- Respect the same final output and sanitization contract as HTTP fetching.

### 2.3 `auto`

- Begin with ordinary HTTP.
- Classify the HTTP response/result once.
- Escalate to browser only for documented renderable cases:
  - JavaScript application shell with insufficient useful content;
  - high-confidence script-dependent page where ordinary extraction is empty or materially unusable;
  - recognized non-interactive browser verification that is expected to resolve by allowing normal script execution.
- Do not escalate for:
  - 400, 401, generic 403, 404, 409, or other caller/access failures;
  - 429 or origin backoff;
  - 5xx unless the classifier has a specific browser-renderable reason, which should normally be false;
  - interactive CAPTCHA/Turnstile/challenge pages;
  - invalid redirect or policy failures;
  - unsupported content types;
  - PDF parse/extraction failure;
  - oversized response;
  - timeout after the logical deadline is exhausted.
- Escalate at most once.

### 2.4 Profiles

When `browser_profile` is supplied:

- resolve the profile through `ProfileManager`;
- retain the full `BrowserProfileMetadata`, not only the display name;
- validate exact origin match;
- use the opaque profile ID for cache scope and lifecycle paths;
- acquire the existing profile lock before launching a profile-scoped browser;
- pass only the Eggsearch-owned `chrome-data` directory to Chrome;
- release the lock after the browser request completes or fails;
- reject profile use when browser rendering is not selected or available.

MCP callers must not create, remove, inspect, or mutate profiles.

---

## 3. Orchestration Shape

### 3.1 Introduce one narrow internal request orchestrator

Move transport choice out of the oversized `run_web_fetch` body into a small internal helper in the existing fetch/MCP boundary.

Recommended shape:

```rust
struct FetchExecutionContext<'a> {
    requested_url: &'a str,
    render_policy: RenderPolicy,
    browser_profile: Option<ResolvedBrowserProfile>,
    deadline: Instant,
    attempt_budget: usize,
}

struct FetchExecutionResult {
    response: WebFetchResponse,
    transport: FetchTransportKind,
    attempt_count: usize,
    browser_escalated: bool,
    profile_scope: Option<String>,
}
```

The exact types may differ, but keep responsibilities explicit:

1. HTTP/browser transport selection;
2. deadline and attempt accounting;
3. browser profile resolution;
4. classification and one-shot escalation;
5. conversion into the existing extraction/sanitization response.

Do not create a public trait/plugin system for transports.

### 3.2 Reuse existing browser code

Use the existing modules under `src/fetch/browser/`:

```text
classify.rs
discover.rs
intercept.rs
lifecycle.rs
navigate.rs
profiles.rs
types.rs
```

Do not duplicate browser discovery, target validation, challenge classification, lifecycle, or navigation logic in `mcp/tools.rs`.

If the existing browser return type does not map cleanly to `WebFetchResponse`, add one focused conversion helper near `navigate.rs` or the fetch module boundary.

### 3.3 Share extraction and sanitization

Browser DOM output must flow through the existing HTML structural renderer and sanitation logic used for ordinary HTTP HTML.

Required properties:

- content remains `external_untrusted`;
- prompt-injection marker scanning remains active;
- output character bounds remain active;
- link extraction remains bounded;
- metadata and final URL remain accurate;
- transport metadata identifies browser usage;
- browser-only headers/cookies are not exposed.

Do not maintain separate browser-specific HTML extraction behavior unless strictly required by the DOM representation.

---

## 4. Deadline and Attempt Accounting

One logical request deadline covers:

```text
origin semaphore wait
HTTP attempt and redirects
classification
browser discovery/startup
profile lock wait
browser navigation and post-load wait
DOM retrieval
extraction and sanitation
```

Rules:

- Do not reset the full request timeout when escalating.
- Do not reset retry counters when escalating.
- HTTP retries remain governed by the existing narrow origin policy.
- Browser execution gets the remaining deadline only.
- If insufficient time remains for browser startup/navigation, return a terminal timeout rather than launching work that cannot complete.
- A browser process that exceeds its bounded timeout must be terminated through the existing lifecycle cleanup path.

Recommended metadata:

```text
transport: http | browser
browser_escalated: bool
attempt_count
```

Do not expose internal process IDs, CDP endpoints, profile paths, or cache keys.

---

## 5. Browser Availability and Configuration

Resolve browser availability once per request or through the existing cached lifecycle/discovery state.

Distinguish these states:

```text
feature_not_compiled
browser_disabled
explicit_executable_invalid
auto_discovery_failed
available
startup_failed
```

For `render=auto`:

- if HTTP succeeds with useful content, browser availability is irrelevant;
- if escalation is needed but browser is unavailable, return the useful HTTP result when one exists plus a structured warning;
- if no useful HTTP content exists, return a structured browser-unavailable failure.

For `render=browser`:

- browser unavailability is a terminal structured failure.

Do not silently downgrade explicit `render=browser` to HTTP.

---

## 6. Challenge and Access-Control Behavior

Interactive challenge detection must remain terminal.

Examples:

```text
CAPTCHA
Turnstile requiring interaction
login challenge without a valid selected profile
MFA prompt
consent/interstitial requiring user action
```

Behavior:

- never click or solve the control;
- never loop or retry automatically;
- never switch to another profile;
- when a selected profile is involved, return `browser_profile_requires_attention` with profile display name and origin;
- otherwise return `manual_interaction_required` with a concise explanation;
- include the local CLI next action only as operator guidance:
  `eggsearch browser-login <origin> --profile <name>`.

A non-interactive verification page may receive one bounded wait under existing configuration. After that wait it either resolves or becomes a terminal result.

---

## 7. Code Changes

Expected files:

```text
src/mcp/tools.rs
src/mcp/state.rs
src/core/fetch.rs
src/core/warning.rs
src/fetch/browser/navigate.rs
src/fetch/browser/lifecycle.rs
src/fetch/browser/types.rs
src/fetch/mod.rs
```

Optional small changes:

```text
src/core/config.rs
src/fetch/client.rs
```

Do not change search providers or repository tools.

### 7.1 `WebFetchArgs`

Keep the existing public fields. Validate combinations:

- profile + `http_only` -> validation error;
- `render=browser` without browser feature/config -> capability error;
- invalid render value -> validation error;
- profile origin mismatch -> validation error;
- profile with browser disabled -> capability error.

### 7.2 Response fields

Add or normalize only fields needed for agents:

```text
transport
browser_escalated
browser_profile
browser_profile_scope
manual_interaction_required
```

Avoid returning a large browser diagnostics object in normal responses.

---

## 8. Focused Verification

Use existing deterministic test infrastructure and a fake/stub browser execution boundary where necessary.

Required focused tests:

1. `http_only` never calls the browser backend.
2. `browser` calls the browser backend without an HTTP prefetch.
3. `auto` returns useful HTTP content without calling browser.
4. `auto` escalates once for a JavaScript shell.
5. `auto` does not escalate for 401, 403, 404, 429, or private-network rejection.
6. `auto` does not escalate for an interactive challenge.
7. browser escalation uses remaining deadline and does not reset attempts.
8. browser-unavailable `auto` preserves useful HTTP output with warning.
9. browser-unavailable explicit browser mode returns a structured failure.
10. profile origin mismatch is rejected before browser launch.
11. profile lock is held across profile-scoped browser execution and released on error.
12. browser DOM flows through sanitation and output bounds.
13. transport metadata accurately identifies HTTP versus browser.
14. no test attempts to solve or click challenges.

Use one ignored local smoke test with installed Chrome only if the existing smoke test is insufficient. Do not add public-site dependencies to routine CI.

Recommended commands:

```bash
cargo test --locked --features browser --test browser_transport
cargo test --locked --features browser --test browser_profiles
cargo test --locked --all-features --test integration web_fetch
make check
```

Adjust test filters to repository conventions. Do not add a browser CI matrix.

---

## 9. Acceptance Criteria

- [ ] `render=http_only` performs HTTP only.
- [ ] `render=browser` invokes the existing browser transport directly.
- [ ] `render=auto` performs at most one documented browser escalation.
- [ ] HTTP and browser share one logical deadline and attempt budget.
- [ ] Browser escalation does not occur for 401/403/404/429, policy failures, PDF failures, or interactive challenges.
- [ ] Explicit persistent profiles are resolved to metadata and opaque ID before execution.
- [ ] Profile origin and lock requirements are enforced.
- [ ] Browser output uses the existing HTML renderer, sanitation, trust labels, and bounds.
- [ ] Explicit browser mode never silently downgrades to HTTP.
- [ ] Interactive challenges are reported without automated interaction.
- [ ] Response transport/profile metadata is accurate and bounded.
- [ ] No general transport plugin framework was introduced.
- [ ] No browser download, proxy, stealth, CAPTCHA, or ordinary-profile behavior was added.
- [ ] Focused tests pass.
- [ ] `make check` passes.
