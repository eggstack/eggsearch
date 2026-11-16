# B002 — Default Artifact Availability — Closure

Status: closed
Recommendation: closed
Implementation candidate: `5605cc240349c65b6442fa73569516cad8e2ad95`
Plan: `plans/archive/implementation/browser-artifact-availability/002-default-artifact-availability.md`
Roadmap: `plans/subsystems/browser-artifact-availability-roadmap.md` M002
ADR: `plans/adrs/ADR-0014-browser-artifact-availability-and-render-bounds.md`

## Implementation

- `default = ["pdf", "browser"]`; one executable per target and the 19-name
  release inventory are unchanged.
- `[fetch].pdf_enabled` defaults from the compiled-in `pdf` feature.
- `[fetch.browser].enabled` defaults from the compiled-in `browser` feature and
  `policy` defaults to `"auto"`; the configured policy is now the default render
  policy when the caller omits `render`, and an invalid policy fails validation.
- Explicit `render = "browser"` or `browser_profile` returns
  `browser_not_compiled_in`, `browser_disabled`, or `browser_unavailable`
  instead of silently using HTTP. `auto` never errors and emits a
  `browser_unavailable` structured warning when escalation was warranted but
  unavailable.
- `packaging/check-http-dependency-shape.py` allows `reqwest` in the default and
  all-features graphs only when its direct reverse-dependency set is exactly
  `["chromiumoxide"]`; production source and direct dependencies remain
  reqwest-free.
- Two live defects found during verification were fixed in the same change: PDF
  pre-parse scanning now resolves indirect `/Length` references instead of
  refusing filtered streams, and `auto` classifies JavaScript shells from
  document blocks rather than framing text or HTML source length.

## Requirement-to-evidence

| Requirement | Evidence | Outcome |
|---|---|---|
| Default build carries PDF and browser | `Cargo.toml`; `cargo test --locked --all-features --test config_validation` (feature-aware defaults); live PDF extraction and escalated render | pass |
| Feature-absent explicit request refuses honestly | `cargo test --locked --no-default-features --features mock --test web_fetch_integration web_fetch_explicit_browser_requires_the_feature` | pass |
| Disabled rendering refuses explicitly | `web_fetch_explicit_browser_with_rendering_disabled_reports_browser_disabled` | pass |
| `auto` degradation is observable | `web_fetch_auto_without_rendering_reports_browser_unavailable` and `web_fetch_auto_classifies_a_js_shell_by_document_text_not_framing` | pass |
| Configured policy is the default and is validated | `default_config_has_expected_values`; `validate_rejects_unknown_browser_policy`; live `render` omitted by the caller not exercised, explicit `auto` exercised | pass |
| armv7 compiles with the default set | `cargo zigbuild --locked --release --target armv7-unknown-linux-gnueabihf` on the final code; hosted candidate job `build_armv7_unknown_linux_gnueabihf` succeeded | pass |
| Dependency guard fails closed | `python3 packaging/check-http-dependency-shape.py` reports `reqwest default=['chromiumoxide'] all-features=['chromiumoxide']` | pass |
| PDF extracts in the default build | Live `web_fetch` of `dummy.pdf`: `content_type = application/pdf`, 150 chars returned, `pdf_page_sparse_text` warning, no refusal code | pass |
| JavaScript escalation works | Live `web_fetch` of `https://excalidraw.com/` with `render = "auto"`: `browser_escalated = true`, `transport = browser` | pass |
| Ten-tool surface unchanged | Live `tools/list`: 10 tools, all four hints, canonical-only schema properties; duplicate alias rejection still works | pass |

## Gates and commands

| Command | Outcome |
|---|---|
| `make check` | 6 320 passed, 0 failed, 28 ignored; packaging stops at the workstation-only `mavis-trash: unsupported OS: Plan9` |
| Hosted CI run `43567644057` | all three jobs succeeded; the macOS job passed on the rerun after the first runner stopped updating |
| `python3 packaging/check-http-dependency-shape.py` | pass, chromiumoxide-only in both graphs |
| `cargo check --locked --no-default-features` | pass |
| Hosted candidate `43567654611` | x86_64 and aarch64 Linux built and qualified natively; armv7 built and qualified structurally |

## Migration and compatibility

- Three absent-key defaults change for default-feature builds: `pdf_enabled`,
  `browser.enabled`, and `browser.policy`.
- Explicit configuration keeps its meaning; `--no-default-features` keeps the
  refusal codes reachable.
- No MCP schema, tool, or response-shape change beyond the new warning code and
  the `lifecycle` object inside an existing capability block.

## Security

- Browser rendering executes untrusted page JavaScript in a child process.
  Existing interception, SSRF policy, origin scoping, concurrency caps, DOM
  bounds, and sanitization remain in force; the idle teardown and session cap
  from B001 bound residency.
- The default graph gains a `reqwest` edge only through chromiumoxide's loopback
  DevTools client. Production source remains reqwest-free.

## Documentation

`docs/features.md`, `docs/config.md`, `docs/installation.md`, `docs/release.md`,
`docs/tool-matrix.md`, `docs/threat-model.md`, `docs/safety.md`,
`docs/python-library.md`, `docs/codegg-integration.md`, `README.md`,
`architecture/build.md`, `architecture/config.md`, `architecture/fetch.md`,
`architecture/packaging.md`, and `CHANGELOG.md`.

## Known limitations

- The macOS aarch64 candidate build job was cancelled by the runner before
  producing evidence; macOS x86_64 and Windows aarch64 qualification were still
  running when this record was written.
- The default graph now carries `reqwest` transitively. ADR-0014 accepts the
  edge; removing it remains deferred until an upstream chromiumoxide release
  makes HTTP discovery optional.

## Recommendation

Closed. The default artifact builds, refuses honestly when capability layers are
absent, and the live surface exercised all ten tools successfully.
