# Standalone MCP Local-Inference M008 — Closure Status

Status: active (schema corrective and focused contract test pass; compact browser invocation and hosted qualification pending)

Source implementation plan:
`plans/implementation/standalone-mcp-local-inference/008-compact-browser-render-reachability-corrective.md`

Source subsystem roadmap:
`plans/subsystems/standalone-mcp-local-inference-roadmap.md`

## 1. Discovery

During M003 paired small-model browser extraction trials, the full
`web_fetch` schema advertised the `render` option but the compact schema did
not. Compact models therefore could not select `auto` or `browser` from the
advertised arguments. The compact trial omitted the option and fetched the
page through HTTP. A restricted full-profile request explicitly used browser
rendering on `example.com` and returned HTTP 200; attempts on the React
reference page timed out or returned HTTP errors. These results establish the
compact schema reachability gap, but not successful paired browser extraction.

## 2. Candidate and verification

Implementation candidate release executable SHA-256:
`507724a14a1926488913a50b1864543f1e899d8a3df6e1b44d1a90dcc4110b43`.

The compact schema now preserves `render` and constrains it to the runtime's
accepted values `http_only`, `auto`, and `browser`. The focused
`mcp_schema_slimming` suite passes 20 tests. `make docs-check` passes.

The paired LFM3 2B browser call used the same `example.com` prompt for full
and compact. Full/diagnostic called `web_fetch` with `render="browser"`, used
18,805 input and 259 output tokens, and returned HTTP 200 with
`transport="browser"`; the model extracted the heading and paragraph
accurately and retained the external-content warning. Compact/brief failed
before a model turn completed because Chrome could not acquire the shared
`chromiumoxide-runner/SingletonLock`. LM Studio returned HTTP 500 and no usage
counters. This is an infrastructure failure, not a compact-model outcome, and
does not satisfy successful compact browser invocation acceptance.

The canonical `make check` attempt passed its earlier formatting, lint, and
compile stages but stalled without output for more than five minutes in the
unchanged `bounded_command` integration suite on this macOS host. It was
interrupted. The exact committed candidate still requires hosted CI.

Remaining evidence: committed-candidate hosted CI, an uncontended compact
browser API call with actual usage and extraction results, and verification
that the compact schema remains within its size budget on the built candidate.
The original `~/.lmstudio/mcp.json` is restored at SHA-256
`b8cb90299d832914cba5f92dbb259684a084045cc503fd186fea4b4cee3fec7c`.

## 3. Disposition

M008 remains active until its acceptance criteria pass. M003 remains active and
owns the broader small-model tool-use and context qualification. The config
restoration hash is verified, and no API authentication or MCP permission
setting was changed.
