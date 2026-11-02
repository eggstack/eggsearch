# Standalone MCP Local-Inference M008 — Closure Status

Status: closed at `772a073fd40e9f044017b26bae18a6efd0724b1d`

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

Implementation candidate commit: `772a073fd40e9f044017b26bae18a6efd0724b1d`.
Release executable SHA-256:
`507724a14a1926488913a50b1864543f1e899d8a3df6e1b44d1a90dcc4110b43`.

The compact schema now preserves `render` and constrains it to the runtime's
accepted values `http_only`, `auto`, and `browser`. The focused
`mcp_schema_slimming` suite passes 20 tests. `make docs-check` passes.

The paired LFM3 2B browser trial used the same `example.com` prompt, all ten
tool schemas, and the exact candidate executable. A local inference harness
supplied the schemas to LM Studio's `/v1/chat/completions` endpoint, executed
the selected MCP call over stdio, and returned the result for answer synthesis.
Full used 68,736 serialized schema bytes and 40,415 API input / 176 output
tokens across both turns. Its valid `web_fetch` request set `render="browser"`
and returned HTTP 200 with `transport="browser"`; the model extracted the
heading and paragraph accurately and retained the external-content warning.
The serialized tool result was 9,901 characters.

Compact used 19,339 schema bytes and 11,904 API input / 216 output tokens. Its
valid request also set `render="browser"` and returned HTTP 200; the brief
projection did not include the `transport` field. The model extracted the same
heading and paragraph and retained the external-content warning. Its result
was 3,817 characters and carried `output_truncated=true`; the model did not
mention that outer-output warning even though the relevant page text was
complete. For this one pair, compact reduced schema bytes by 71.9%, total input
tokens by 70.5%, and result characters by 61.4%. This qualifies compact
browser-call reachability and extraction for this simple page, not general
browser reliability or quality.

The canonical `make check` attempt passed its earlier formatting, lint, and
compile stages but stalled without output for more than five minutes in the
unchanged `bounded_command` integration suite on this macOS host. It was
interrupted. Hosted CI on the exact source commit `772a073fd40e9f044017b26bae18a6efd0724b1d`
passed Linux canonical, macOS canonical and filesystem portability, and
Windows portability in run `41220450246`. Release drift run `41220450259` and
Python native qualification run `41220450258` also passed. The separate egress
feature run `41220450274` passed its Linux, x86_64 macOS, Windows, MSRV, and
ARMv7 jobs; its Apple Silicon macOS egress job was cancelled by a scheduled
runner-image brownout and does not exercise this change.

The focused schema suite verifies that the compact tool surface remains within
budget. The original `~/.lmstudio/mcp.json` is restored at SHA-256
`b8cb90299d832914cba5f92dbb259684a084045cc503fd186fea4b4cee3fec7c`.

## 3. Disposition

M008 is closed at `772a073fd40e9f044017b26bae18a6efd0724b1d`. The compact
schema preserves all render modes within budget, the paired local-model API
trial successfully invoked and extracted through compact browser mode, and
hosted canonical qualification passed. M003 remains active and owns the broader
small-model tool-use and context qualification. The config restoration hash is
verified, and no API authentication or MCP permission setting was changed.
