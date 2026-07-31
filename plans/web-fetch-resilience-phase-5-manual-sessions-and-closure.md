# Phase 5 — Manual Local Sessions and Closure

**Repository:** `eggstack/eggsearch`  
**Roadmap:** `plans/web-fetch-pdf-and-browser-resilience-roadmap.md`  
**Predecessor:** `plans/web-fetch-resilience-phase-4-system-chrome-rendering.md`  
**Status:** Implementation handoff  
**Scope:** Explicit headed browser login, Eggsearch-owned persistent profiles, challenge-aware reuse, documentation, and final bounded verification

---

## 1. Objective

Complete the browser-resilience line of work by allowing a local operator to establish a dedicated browser session manually for an origin that requires authentication or interactive human verification.

This phase adds:

1. named Eggsearch-owned browser profiles;
2. a headed `browser-login` CLI workflow;
3. origin/profile association and explicit selection from `web_fetch`;
4. reuse of profile cookies and storage through the Phase 4 browser transport;
5. cache partitioning by profile identity;
6. structured challenge and session-expiry outcomes;
7. operator commands to list and remove profiles;
8. final capability, documentation, and closure checks.

This phase does not automate login, type credentials, click CAPTCHA/Turnstile controls, export cookies, inspect the user's ordinary browser profile, or create a remote browser service.

---

## 2. Fixed Decisions

### 2.1 Profiles are explicit and local

Persistent profiles must be created only through an operator CLI action. MCP callers cannot create new profiles or cause a headed browser to launch.

Recommended command surface:

```text
eggsearch browser-login <origin> [--profile <name>]
eggsearch browser-profiles list
eggsearch browser-profiles remove <name>
eggsearch browser-profiles inspect <name>
```

Exact Clap nesting may follow existing CLI conventions. Keep the command set small.

### 2.2 Never use the ordinary Chrome profile

Eggsearch must use a dedicated data root, for example:

```text
~/.local/share/eggsearch/browser-profiles/<profile-id>/
```

Equivalent platform-specific application-data directories should use existing project path helpers.

Do not point Chrome at:

- the user's default Chrome profile;
- Chrome's global user-data directory;
- an arbitrary directory supplied by an MCP caller.

### 2.3 Manual means manual

The headed browser opens the requested origin. The operator may:

- log in;
- complete MFA;
- accept a site consent flow;
- complete a CAPTCHA or Turnstile challenge personally;
- close the browser or signal completion.

Eggsearch must not:

- find or click controls;
- enter usernames/passwords;
- read credentials from environment variables;
- automate MFA;
- use an external solver;
- claim a challenge was solved automatically.

### 2.4 Profiles are origin-scoped by default

Each profile records an allowed origin set. The initial command creates a profile for one normalized origin.

A profile must not be usable for arbitrary unrelated origins unless the operator explicitly extends its allowlist through a future command. This phase need not implement multi-origin editing; one profile/one origin is sufficient.

Subdomains may be handled according to a documented rule:

- exact origin only; or
- explicitly recorded parent-domain scope.

Prefer exact origin initially.

### 2.5 MCP profile selection is explicit

Recommended request field:

```rust
pub browser_profile: Option<String>
```

Rules:

- omitted means ephemeral browser context;
- named profile must exist and be allowed for the requested origin;
- profile selection is valid only with browser-capable render policy;
- a caller cannot pass a filesystem path;
- names are validated and normalized;
- the response reports the non-secret profile name/ID used;
- profile contents and cookies are never serialized.

### 2.6 Persistent cache is profile-partitioned

Phase 3 defines cache scope. This phase must use a stable non-secret profile identifier in that scope.

Deleting a profile should also remove or invalidate its profile-scoped derived/raw cache entries when the cache backend supports it. If exact purge is difficult, rotate/remove the profile scope ID so old entries cannot be read by a recreated profile of the same display name.

---

## 3. Profile Metadata Model

### 3.1 Separate metadata from browser storage

Recommended directory shape:

```text
browser-profiles/
    <opaque-id>/
        profile.toml
        chrome-data/
```

Recommended metadata:

```rust
pub struct BrowserProfileMetadata {
    pub id: String,
    pub display_name: String,
    pub allowed_origin: String,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub browser_family: String,
    pub browser_major_version: Option<u32>,
    pub schema_version: u32,
}
```

Do not store cookies, local-storage values, passwords, or tokens in `profile.toml`; Chrome owns its storage under `chrome-data`.

### 3.2 Profile names

Validate display names strictly:

```text
ASCII letters, digits, hyphen, underscore
bounded length
no path separators
no dot segments
no leading/trailing whitespace
```

Use an opaque generated ID for the directory/cache scope so display-name reuse cannot access deleted-profile cache state.

### 3.3 Filesystem permissions

On Unix-like systems:

- create the profile root with owner-only permissions where practical;
- avoid group/world-readable metadata and browser data;
- reject symlinked profile directories/files;
- use atomic metadata writes;
- delete profile trees without following symlinks.

On Windows, use the safest practical application-data directory and document the platform limitation rather than building a custom ACL framework.

Reuse existing safe filesystem helpers when available. Do not create a broad new secure-storage subsystem.

### 3.4 Profile locking

Prevent simultaneous headed login and headless fetch use of the same profile.

A small lock file or in-process state plus an exclusive filesystem lock is sufficient. Required outcomes:

- one profile cannot be opened by two Eggsearch browser processes;
- a stale lock can be detected and reported/recovered safely;
- do not delete Chrome's internal lock files while a process may be alive;
- profile-busy errors are actionable.

Avoid a cross-platform distributed locking framework. Use an existing crate only if it is small and clearly needed.

---

## 4. `browser-login` Workflow

### 4.1 Command validation

The command must:

1. require the optional browser feature/capability;
2. normalize and validate the requested HTTP(S) origin;
3. reject paths as the profile scope if only origins are supported;
4. reject localhost/private-network origins in the initial browser threat model;
5. discover/validate Chrome through Phase 4;
6. create or open the dedicated profile;
7. acquire the profile lock;
8. launch Chrome headed at the origin.

The command is an operator action and may produce terminal guidance. It is not an MCP tool call.

### 4.2 Completion behavior

Keep completion simple. Accept one of these designs:

**Process-close completion:**

- operator closes the Chrome window;
- Eggsearch records successful session setup if the process exited normally.

**Terminal-confirm completion:**

- Chrome remains open;
- terminal asks operator to press Enter after finishing;
- Eggsearch closes the browser and records completion.

Do not inspect page content to decide that login/challenge succeeded unless a simple optional origin-load check is already available through Phase 4. The operator is authoritative.

### 4.3 Browser launch constraints

Use the profile's `chrome-data` directory and ordinary headed Chrome. Do not add stealth flags.

Disable or avoid:

- remote debugging exposure beyond local process needs;
- downloads where practical;
- extension loading;
- automatic password-store integration if it creates cross-profile access;
- use of the system default profile.

If Chrome requires a debugging port, bind to loopback and use an unpredictable/ephemeral port. Do not expose it on LAN interfaces.

### 4.4 Profile update

After completion:

- atomically update `last_used_at`;
- record browser family/major version for diagnostics;
- release the lock;
- report the profile name and allowed origin;
- do not print cookies or storage values.

A failed browser launch must not leave a half-created profile marked ready. Either remove the incomplete directory or mark metadata state explicitly.

---

## 5. Headless Reuse

### 5.1 Request validation

When `browser_profile` is supplied:

- resolve display name to opaque ID;
- verify metadata schema;
- verify exact allowed origin;
- acquire profile lock;
- require browser rendering capability;
- select Phase 4 browser transport;
- use profile cache scope;
- update `last_used_at` after a completed attempt;
- release the lock on every path.

Do not fall back silently to an ephemeral profile when the named profile is missing, busy, corrupt, or disallowed for the origin.

### 5.2 Browser process strategy

Persistent and ephemeral contexts may require different Chrome process/user-data handling.

Preferred simple design:

- retain the warm ephemeral Chrome process for anonymous rendering;
- launch a separate bounded Chrome process when using a persistent profile;
- allow only one in-flight request per persistent profile;
- close the process after the request or after a very short profile-specific idle window.

Do not attempt to multiplex many persistent user-data directories into one Chrome process if the CDP/browser model makes that unsafe.

The extra process cost is acceptable because profile use is explicit and uncommon.

### 5.3 Session expiry and challenge recurrence

If a profile-scoped fetch returns:

- login form;
- authentication-required page;
- interactive challenge;
- persistent verification page;

return a structured result such as:

```text
browser_profile_requires_attention
```

Include the operator command needed to reopen the profile:

```text
eggsearch browser-login <origin> --profile <name>
```

Do not launch it automatically from MCP.

### 5.4 Cookies and storage

Chrome manages cookie/local-storage persistence in the dedicated profile directory.

Eggsearch must not:

- export cookies through response fields;
- log cookie headers;
- copy cookies into `reqwest` automatically;
- provide a cookie-dump CLI;
- merge storage between profiles;
- use profile cookies for unrelated origins.

HTTP and profile-browser cache scopes remain distinct even if they target the same URL.

---

## 6. Profile Management Commands

### 6.1 List

`browser-profiles list` should show bounded non-secret information:

```text
display name
allowed origin
created time
last used time
browser family/version
state: ready | busy | incomplete | incompatible
```

Do not inspect or enumerate cookies/storage.

### 6.2 Inspect

`inspect` may show:

- metadata fields;
- directory size estimate if cheap and bounded;
- lock state;
- compatibility warning;
- cache scope identifier in redacted/short form if useful.

Do not show filesystem internals beyond what helps the operator.

### 6.3 Remove

Removal must:

- require the exact profile name;
- refuse while locked/in use;
- delete only the resolved Eggsearch-owned directory;
- not follow symlinks;
- remove/invalidate profile-scoped cache entries;
- report completion.

Avoid interactive confirmation if the repository's CLI convention favors scriptability, but provide `--force` only if necessary for stale/incomplete profiles. Never force-remove a profile with a live Chrome process.

### 6.4 Version compatibility

Chrome profiles may not be backward compatible across major browser downgrades.

If the recorded profile version is newer than the current browser major version:

- report an incompatibility warning;
- refuse headless reuse by default;
- allow the operator to remove/recreate the profile.

Do not implement profile migration.

---

## 7. Challenge and Abuse Boundaries

### 7.1 Structured outcomes

Ensure responses distinguish:

```text
browser_unavailable
manual_interaction_required
browser_profile_not_found
browser_profile_origin_mismatch
browser_profile_busy
browser_profile_incompatible
browser_profile_requires_attention
access_denied
rate_limited
```

Do not collapse these into generic network errors.

### 7.2 No automated evasion

Search the final implementation for prohibited behavior and dependencies:

```text
Turnstile coordinate/click logic
CAPTCHA solver APIs
mouse movement simulation
proxy rotation
browser fingerprint generation
canvas/WebGL spoofing
fake referrer defaults
recursive challenge retries
ordinary Chrome profile paths
```

This is a bounded code-review search, not a new static-analysis test suite.

### 7.3 Local multi-user considerations

Eggsearch is local-first, but a daemon may still serve multiple clients. Named browser profiles represent operator-controlled authenticated state.

Recommended initial policy:

- browser profiles disabled unless explicitly configured;
- profile selection allowed only when the server operator enables it;
- optionally allowlist profile names in config;
- do not expose profile creation/removal over MCP;
- document that all MCP clients able to name an enabled profile may access the rendered content available to that profile.

Do not build a complete user/role authorization system in this phase. If the daemon is shared with untrusted users, operators should leave persistent profiles disabled.

---

## 8. Configuration

Recommended surface:

```toml
[fetch.browser]
enabled = false
persistent_profiles_enabled = false
profiles_dir = ""
allowed_profiles = []
profile_process_timeout_ms = 30000
```

Rules:

- empty `profiles_dir` uses the platform application-data default;
- profile root must be local filesystem storage;
- caller cannot override it;
- `allowed_profiles` may be omitted for a solo-user deployment, but explicit enablement remains required;
- numeric settings remain capped.

Keep configuration compatible with Phase 4 ephemeral browser use.

---

## 9. Capability and Response Metadata

Capability reporting should distinguish:

```text
browser compiled
browser executable usable
ephemeral rendering enabled
persistent profiles enabled
profile count
profile names only when operator policy permits status exposure
```

`web_fetch` response metadata may include:

```text
transport_used = browser
browser_profile = <display name>
browser_profile_scope = persistent
manual_interaction_required = true/false
```

Never include:

- cookies;
- local storage;
- browser-data path;
- remote debugging endpoint;
- full opaque cache scope ID;
- credentials.

---

## 10. Documentation and Closure Work

### 10.1 User documentation

Document:

- how to compile/enable browser support;
- how Chrome discovery works;
- how to create a profile;
- how to select a profile in `web_fetch`;
- profile origin restrictions;
- session-expiry recovery;
- profile removal;
- filesystem sensitivity;
- shared-daemon warning;
- no challenge automation;
- no browser download;
- no ordinary Chrome-profile access.

### 10.2 Architecture documentation

Update the fetch architecture with:

```text
HTTP path
PDF path
origin/cache controller
browser escalation
persistent profile boundary
sanitation/trust path
```

Keep diagrams and prose concise. Do not duplicate the entire implementation plan.

### 10.3 Tool schema and examples

Add small examples:

```json
{
  "url": "https://docs.example.com/app",
  "render": "auto"
}
```

```json
{
  "url": "https://portal.example.com/",
  "render": "browser",
  "browser_profile": "portal"
}
```

Clarify that profile creation is a local CLI operation, not an MCP action.

### 10.4 Completion report expectations

The implementer should record in the final PR/commit summary:

- features added;
- optional runtime requirements;
- commands run;
- deterministic fixture results;
- one local installed-Chrome smoke result if available;
- any platform/runtime not tested;
- deferred items.

Do not add a permanent evidence ledger or checked-in verification artifact.

---

## 11. Non-Goals

Do not implement:

- MCP profile creation/removal tools;
- automated login;
- credential storage outside Chrome;
- cookie export/import;
- ordinary browser-profile access;
- remote browser control;
- browser synchronization;
- multi-user RBAC;
- profile migration;
- challenge/CAPTCHA solving;
- proxy support;
- stealth/fingerprint features;
- private-network browser rendering;
- browser CI matrix;
- scheduled live browser checks;
- release automation;
- a profile database when filesystem metadata is sufficient.

---

## 12. Focused Verification

### 12.1 Deterministic tests

Required categories:

- profile name validation;
- origin normalization and exact-origin enforcement;
- metadata atomic write/read;
- symlink/path escape rejection;
- opaque ID/cache scope rotation;
- profile lock acquisition and busy result;
- incomplete profile cleanup;
- missing/incompatible profile errors;
- request selects persistent versus ephemeral transport;
- cache scope separation;
- profile removal stays inside profile root;
- shared-daemon configuration disables profile use when not enabled;
- response serialization excludes secret paths/storage.

Use temporary directories and fake browser controllers for most tests. Do not require real login or real CAPTCHA services.

### 12.2 Manual smoke checks

A bounded local checklist is sufficient:

1. discover installed Chrome;
2. create a profile against an ordinary test origin or local public-style fixture;
3. close/confirm the headed session;
4. use the profile for one headless rendered fetch;
5. observe cache scope separation from ephemeral fetch;
6. reopen the profile after a simulated session-attention result;
7. remove the profile;
8. confirm its directory and scoped cache are gone/inaccessible.

A real authenticated public site may be used manually by the maintainer, but its credentials, cookies, screenshots, and content must not be committed.

### 12.3 Commands

During development:

```bash
cargo check --locked --features browser
cargo test --locked --features browser --test browser_profiles
```

Then:

```bash
make check
```

Do not add real-profile tests to CI. Do not install Chrome in CI solely for this phase.

---

## 13. Final Acceptance Criteria

- [ ] Persistent profiles are disabled unless explicitly enabled by the operator.
- [ ] Profiles are created only through a local headed CLI workflow.
- [ ] Eggsearch never uses the user's ordinary Chrome profile.
- [ ] Profile directories use opaque IDs and bounded validated display names.
- [ ] Each profile is restricted to its recorded origin.
- [ ] Filesystem writes/removal avoid symlink/path escape.
- [ ] A profile cannot be used concurrently by login and headless fetch processes.
- [ ] MCP callers can select only existing enabled profile names, never paths.
- [ ] Persistent fetches use profile-partitioned cache scope.
- [ ] Cookies/storage are never logged or serialized.
- [ ] Expired sessions and recurring challenges return `browser_profile_requires_attention` or equivalent.
- [ ] MCP calls never launch a headed browser automatically.
- [ ] Profile list/inspect/remove expose only non-secret metadata.
- [ ] Shared-daemon risks are documented and profiles can remain disabled.
- [ ] No automated login, challenge solving, fingerprint spoofing, or proxy feature exists.
- [ ] Active documentation describes PDF quality/OCR, origin/cache behavior, ephemeral Chrome rendering, and manual profiles coherently.
- [ ] One bounded local Chrome/profile smoke sequence is documented in the implementation summary when Chrome is available.
- [ ] No CI browser matrix, scheduled live workflow, evidence ledger, or release automation is added.
- [ ] `make check` passes.

---

## 14. Roadmap Closure Criteria

After this phase, the roadmap may be considered closed when the combined implementation satisfies:

```text
lightweight default build
optional rich PDF extraction
page-local OCR under explicit policy
bounded origin backoff and cache
optional installed-Chrome rendering
strict public-network browser policy
manual operator-owned session establishment
no automated challenge interaction
```

Any remaining work involving table reconstruction, alternative browsers, private-network rendering, proxying, CAPTCHA solving, or distributed infrastructure is outside this roadmap and must not delay closure.

---

## 15. Handoff Notes

Keep the persistent-profile implementation intentionally boring: filesystem metadata, one headed command, one profile lock, explicit origin matching, and separate browser/cache scope. The line of work is complete when a solo local operator can establish and reuse a normal browser session safely—not when Eggsearch can imitate or evade arbitrary anti-bot systems.