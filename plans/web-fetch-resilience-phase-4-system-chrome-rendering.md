# Phase 4 — Optional System-Chrome Rendering

**Repository:** `eggstack/eggsearch`  
**Roadmap:** `plans/web-fetch-pdf-and-browser-resilience-roadmap.md`  
**Predecessor:** `plans/web-fetch-resilience-phase-3-origin-control-and-cache.md`  
**Status:** Implementation handoff  
**Scope:** Classification-driven rendering through an already-installed Google Chrome or Chromium binary

---

## 1. Objective

Add a narrowly scoped optional browser transport for public pages that ordinary HTTP fetching cannot usefully render.

This phase should support:

1. discovery of an already-installed full Chrome or Chromium executable;
2. one supervised browser process controlled through the Chrome DevTools Protocol;
3. isolated ephemeral browser contexts;
4. bounded navigation and DOM extraction;
5. request interception and destination policy checks;
6. detection of ordinary JavaScript shells and browser-verification pages;
7. one HTTP-to-browser escalation under `render = auto`;
8. explicit `render = browser` for caller-requested rendering;
9. structured results for interactive challenges and unavailable browser capability.

This phase must not download a browser, solve CAPTCHAs, click Turnstile controls, rotate proxies, synthesize browser fingerprints, or use the user's ordinary Chrome profile.

---

## 2. Fixed Decisions

### 2.1 Browser support is optional

Recommended feature:

```toml
browser = ["dep:chromiumoxide"]
```

Use another Rust CDP client only if repository inspection shows it is materially smaller or more maintainable. Do not add Playwright, Node.js, Python, Selenium, or WebDriver infrastructure.

Default builds and default runtime behavior remain HTTP-only unless browser support is compiled and enabled.

### 2.2 Use a system browser

Eggsearch must discover an existing executable. It must not:

- download Chromium;
- install Chrome;
- invoke a package manager;
- bundle a browser archive;
- manage browser updates.

Supported initial browser family:

```text
Google Chrome stable
Chromium
```

Do not add Firefox, WebKit, Edge-specific handling, or `chrome-headless-shell` in this phase.

### 2.3 Full Chrome unified headless mode is preferred

Launch the normal installed browser with headless mode and CDP control. Do not use `--dump-dom` as the production transport because it cannot enforce the required request policy or collect sufficient navigation metadata.

A `--dump-dom` local spike may be used during implementation, but it must not become the final architecture.

### 2.4 Automatic escalation is narrow

Public request policy:

```rust
pub enum RenderPolicy {
    HttpOnly,
    Auto,
    Browser,
}
```

Semantics:

- `HttpOnly`: never launch Chrome;
- `Auto`: attempt HTTP first and escalate once only for approved classification reasons;
- `Browser`: use Chrome directly after validating the target and origin gate.

`Auto` must not escalate ordinary authentication, denial, rate-limit, not-found, or server-error responses.

### 2.5 No challenge interaction

Allowed:

- load a page in Chrome;
- wait briefly for a non-interactive browser verification page to resolve naturally;
- reuse cookies only in Phase 5 explicit profiles;
- report an interactive challenge.

Forbidden:

- finding challenge iframe coordinates;
- clicking checkboxes;
- simulating mouse motion;
- solving CAPTCHAs;
- recursive challenge retries;
- hidden external solving services;
- stealth plugins;
- fingerprint randomization.

### 2.6 Browser policy is stricter than ordinary convenience

The existing HTTP transport pins validated DNS addresses into `reqwest`. Chrome resolves destinations independently. Exact parity is not readily guaranteed.

Therefore:

- browser transport must reject localhost/private-network targets regardless of ordinary `allow_localhost`/`allow_private_network` settings in the initial implementation;
- browser transport must be documented as public-network-only;
- every observable request must be intercepted and checked;
- unsupported or uninspectable request behavior must fail closed.

Private-network browser rendering may be considered later only with a demonstrably safe design.

---

## 3. Transport Contract

### 3.1 Introduce a small transport result

Recommended internal shape:

```rust
pub enum FetchTransportKind {
    Http,
    Browser,
}

pub struct TransportResponse {
    pub transport: FetchTransportKind,
    pub requested_url: String,
    pub final_url: String,
    pub status: Option<u16>,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    pub content_type: Option<String>,
    pub redirects: Vec<String>,
    pub timing: TransportTiming,
    pub classification: Option<FetchDisposition>,
}
```

Do not force the HTTP client into a complete rewrite. Extract only enough shared response shape to route browser DOM output through the existing HTML extraction and sanitation pipeline.

### 3.2 Define disposition classification

Recommended enum:

```rust
pub enum FetchDisposition {
    UsefulContent,
    JavascriptShell,
    NonInteractiveVerification,
    InteractiveChallenge,
    RateLimited,
    AccessDenied,
    AuthenticationRequired,
    ServerError,
    Unsupported,
}
```

Classification should be pure where possible and based on bounded data:

```text
HTTP status
content type
HTML title
visible/extracted text size
known app-root patterns
script density
known verification markers
challenge iframe/source markers
```

Do not build a continuously growing vendor signature database. Start with a small transparent rule set and an `Unknown`/`Unsupported` outcome.

### 3.3 Escalation decision

Escalate under `Auto` only when:

```text
HTTP response is successful or a recognized non-interactive verification response
AND
body is HTML
AND
classification is JavascriptShell or NonInteractiveVerification
AND
browser capability is available
AND
origin circuit permits the attempt
AND
logical request deadline has sufficient remaining time
```

At most one browser attempt follows the HTTP attempt. Browser failure returns a structured result; it does not loop back to HTTP.

---

## 4. Browser Discovery

### 4.1 Discover once

Add a startup or lazy discovery helper that caches the result.

Linux candidates:

```text
google-chrome-stable
google-chrome
chromium
chromium-browser
```

macOS candidates:

```text
/Applications/Google Chrome.app/Contents/MacOS/Google Chrome
~/Applications/Google Chrome.app/Contents/MacOS/Google Chrome
/Applications/Chromium.app/Contents/MacOS/Chromium
```

Windows candidates:

```text
Program Files Google Chrome path
Program Files (x86) Google Chrome path
LocalAppData Google Chrome path
standard Chromium paths where practical
```

Also support an explicit configured executable path.

### 4.2 Validate the executable

Before marking available:

- require a regular executable file or PATH resolution;
- run `--version` under a short timeout;
- capture a bounded version string;
- reject unexpected command failure;
- do not accept arbitrary shell command strings;
- launch directly without shell interpolation.

Cache:

```text
path
browser family
version
availability reason
```

Do not re-run version detection per request.

### 4.3 Capability reporting

Report:

```text
browser feature compiled
browser enabled in config
executable discovered
path source: configured or auto-discovered
browser family/version
usable/unavailable reason
```

Do not expose full user home paths if the existing status model avoids them; a redacted basename is sufficient.

---

## 5. Process Lifecycle

### 5.1 Use one warm browser process

Create the browser lazily on first browser request. Reuse it across requests while creating isolated contexts/pages.

Requirements:

- one process supervisor;
- bounded startup timeout;
- process exit detection;
- one restart after unexpected process death per logical request at most;
- no infinite restart loop;
- cleanup on server shutdown;
- temporary user-data directory for ephemeral mode;
- restrictive filesystem permissions;
- no use of the user's normal Chrome profile.

Do not launch a fresh Chrome process for every URL unless the selected CDP library cannot safely isolate contexts. If that limitation exists, document the cost and retain strict global concurrency.

### 5.2 Minimal launch arguments

Use only flags required for headless operation, isolation, and predictable local execution.

Recommended categories:

```text
headless mode
remote debugging/CDP connection
Eggsearch-owned user-data directory
no first-run UI
no default-browser prompt
disable downloads where possible
```

Do not copy large anti-detection flag lists from `master-fetch`. Avoid disabling browser security features.

### 5.3 Global and per-origin limits

Use Phase 3 origin controls:

```text
browser in-flight per origin: 1
global browser pages/contexts: 1 or 2
```

The browser process may remain warm, but pages and contexts must be closed after every request.

---

## 6. Browser Network Policy

### 6.1 Validate initial URL

Run the existing URL and public-network policy before creating a page.

For the initial phase, browser rendering must reject:

```text
localhost
private IPv4
link-local
cloud metadata ranges/hostnames
IPv6 loopback/link-local/unique-local
non-http(s) schemes
embedded credentials
```

### 6.2 Intercept requests

Use CDP request interception for:

- top-level navigation;
- redirects;
- frames;
- scripts;
- XHR/fetch;
- stylesheets;
- images if enabled;
- WebSocket handshakes where observable;
- service-worker requests where observable.

For each request:

1. parse URL;
2. reject non-HTTP(S) schemes;
3. apply blocked-hostname and literal-IP checks;
4. resolve hostname under a short timeout where practical;
5. reject any prohibited resolved address;
6. allow only when policy passes.

If interception cannot reliably cover a request category, disable the feature responsible for it or fail the page.

### 6.3 Resource policy

Default automatic rendering may block expensive resources that are usually unnecessary for text extraction:

```text
media
large images
fonts where layout does not depend on them
beacons
WebSockets
```

Do not block stylesheets or images unconditionally if doing so prevents ordinary page rendering or verification. Prefer a configurable conservative profile and measure using deterministic fixtures.

Bound:

```text
total requests
redirect count
total observed transferred bytes where available
DOM bytes/characters
navigation time
post-load wait
screenshot dimensions if screenshots are used internally for diagnostics
```

Do not return screenshots in the MCP response in this phase.

### 6.4 Disable risky browser capabilities

Where the CDP API supports it cleanly:

- deny downloads;
- reject `file:` navigation;
- avoid extension loading;
- isolate service workers or disable them for ephemeral contexts;
- deny geolocation, camera, microphone, notifications, clipboard, and MIDI permissions;
- do not persist credentials;
- do not expose local files.

Do not add a broad browser-hardening subsystem; set the relevant context permissions and fail closed.

---

## 7. Navigation and Readiness

### 7.1 Navigation sequence

Recommended sequence:

```text
create isolated context
create page
enable interception
navigate under remaining deadline
wait for DOMContentLoaded
inspect classification
if ordinary JS shell, wait bounded quiet period or selector/text threshold
if non-interactive verification, wait bounded resolution window
serialize final DOM
close page/context
```

Do not wait indefinitely for `networkidle`; modern pages may keep connections open.

### 7.2 Readiness criteria

Use bounded heuristics:

```text
minimum visible/extracted text increase
root/application container receives meaningful content
DOM mutation quiet period
optional caller-independent maximum post-load wait
verification title/marker disappears
```

Keep the implementation deterministic enough for local fixtures. Avoid a large framework of per-site selectors.

### 7.3 DOM extraction

Obtain the final serialized DOM and pass it through the existing HTML renderer.

Requirements:

- bound serialized DOM bytes before extraction;
- preserve final URL;
- retain browser transport metadata;
- apply existing prompt-injection sanitation and `external_untrusted` trust;
- do not execute page-provided instructions outside browser rendering;
- do not expose cookies, local storage, console values, or raw CDP events.

### 7.4 Status and header limitations

CDP may not expose browser response metadata identically to `reqwest`.

Capture the main document response status and headers when available. If unavailable, use `Option` and report the limitation rather than fabricating HTTP 200.

---

## 8. Challenge-Aware Outcomes

### 8.1 Detect interactive challenge

Recognize bounded indicators such as:

```text
Turnstile iframe/script markers
CAPTCHA iframe/script markers
"verify you are human" forms
interactive challenge controls
persistent challenge title after bounded wait
```

Return a structured result:

```rust
pub struct ManualInteractionRequired {
    pub origin: String,
    pub reason: ManualInteractionReason,
    pub browser_profile_supported: bool,
    pub message: String,
}
```

Do not return the challenge page as if it were useful article content.

### 8.2 Non-interactive verification

A recognized non-interactive page may be allowed a short bounded resolution window. If it resolves, continue extraction. If it remains, return a challenge/denial outcome.

Do not recursively retry or repeatedly reload.

### 8.3 Rate limits and denials

Browser-observed 429 or denial pages must feed Phase 3 origin state and must not be cached as useful content.

A browser page that visually says access denied while returning HTTP 200 should be classified as denial rather than useful content when the rule is high confidence.

---

## 9. Configuration

Recommended surface:

```toml
[fetch.browser]
enabled = false
policy = "auto"
executable = ""
startup_timeout_ms = 10000
navigation_timeout_ms = 20000
post_load_wait_ms = 1500
verification_wait_ms = 10000
max_requests = 100
max_dom_bytes = 4000000
global_concurrency = 1
per_origin_concurrency = 1
block_media = true
```

Keep defaults disabled and bounded. Validate all numeric settings with hard caps.

The public request `render` may override `policy` only within server capability and policy. A caller cannot enable a disabled browser runtime.

---

## 10. Non-Goals

Do not implement:

- bundled/downloaded Chromium;
- Playwright/Selenium/WebDriver;
- browser fingerprint spoofing;
- user-agent/TLS persona randomization;
- fake referrers;
- proxy configuration or rotation;
- CAPTCHA/Turnstile clicking;
- mouse/keyboard simulation;
- headful login workflow;
- persistent profiles;
- private-network browser rendering;
- recursive rendering/crawling;
- page screenshots in normal output;
- browser performance benchmarks;
- external Cloudflare tests in CI;
- browser platform CI matrix;
- scheduled live browser workflow.

Persistent operator-created profiles belong to Phase 5.

---

## 11. Focused Verification

### 11.1 Deterministic local fixture server

Add one small test fixture server exposing:

```text
/static             useful server-rendered HTML
/js-shell           empty root populated by JavaScript
/delayed            text appears after bounded delay
/noninteractive     challenge-like page that redirects/resolves locally
/interactive        persistent challenge marker
/redirect-public    bounded redirect
/redirect-private   redirect to prohibited local/private target for policy testing
/many-resources     request-count limit fixture
/oversized-dom      DOM size limit fixture
```

The private-target fixture should test policy through mocked/intercepted resolution where direct private navigation would be unsafe or flaky.

Do not use a real Cloudflare domain.

### 11.2 Required tests

- executable discovery order and explicit override;
- invalid executable rejection;
- browser unavailable result;
- `HttpOnly` never starts Chrome;
- `Auto` does not escalate useful HTML;
- `Auto` escalates deterministic JS shell once;
- `Auto` does not escalate 401/403/404/429/generic 5xx;
- explicit `Browser` rendering;
- final DOM passes through existing renderer/sanitation;
- top-level prohibited redirect is blocked;
- prohibited subresource is blocked;
- request-count and DOM-size limits;
- navigation timeout cleanup;
- page/context cleanup after success/failure;
- interactive challenge result;
- non-interactive bounded wait;
- origin circuit prevents escalation;
- browser content cache scope is distinct from anonymous HTTP.

Avoid tests that depend on exact Chrome error wording or pixel rendering.

### 11.3 Commands

During development:

```bash
cargo check --locked --features browser
cargo test --locked --features browser --test browser_transport
```

Then:

```bash
make check
```

Real installed-Chrome smoke checks should be manual and ignored by default, for example:

```bash
cargo test --locked --features browser --test browser_live_smoke -- --ignored
```

One local deterministic fixture and one ordinary public JS-rendered documentation page are enough for manual confirmation. Do not add a CI browser installation or matrix.

---

## 12. Documentation Updates

Document:

- system-browser prerequisite;
- discovery order and executable override;
- `http_only|auto|browser` semantics;
- automatic escalation rules;
- public-network-only browser restriction;
- resource and timeout limits;
- challenge-aware outcomes;
- no browser download;
- no challenge solving;
- no ordinary-profile access;
- manual diagnostic command.

Update the fetch architecture document and capability reporting documentation. Avoid unrelated provider documentation changes.

---

## 13. Acceptance Criteria

- [ ] Browser support remains an optional feature and disabled by default.
- [ ] Eggsearch never downloads or installs a browser.
- [ ] An explicit or auto-discovered full Chrome/Chromium executable is validated once.
- [ ] One supervised browser process serves isolated ephemeral contexts.
- [ ] `HttpOnly`, `Auto`, and `Browser` behavior is explicit and tested.
- [ ] `Auto` escalates only approved JS-shell/non-interactive classifications.
- [ ] One logical request performs at most one browser escalation.
- [ ] Browser attempts share the Phase 3 deadline, attempt budget, origin limits, and cache scope.
- [ ] Initial navigation, redirects, frames, and subresources are policy-checked.
- [ ] Browser mode rejects private/local destinations in this phase.
- [ ] DOM, request count, redirects, time, and concurrency are bounded.
- [ ] Rendered DOM uses the existing extraction and untrusted-content sanitation pipeline.
- [ ] Interactive challenges return `manual_interaction_required` and are never clicked.
- [ ] Challenge/denial pages are not cached as useful content.
- [ ] No anti-detection flag collection, proxy system, or fingerprint spoofing is added.
- [ ] No browser installation or live-site matrix is added to CI.
- [ ] `make check` passes.

---

## 14. Handoff Notes

The difficult part is policy and lifecycle control, not launching Chrome. Keep the CDP adapter narrow, use the existing extraction pipeline, and fail closed when request interception or destination validation is uncertain. Do not expand this phase into a stealth-browser project.