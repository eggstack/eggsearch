# Standalone MCP Local-Inference M007 — Closure Status

Status: closed at `772a073fd40e9f044017b26bae18a6efd0724b1d`

Source implementation plan:
`plans/implementation/standalone-mcp-local-inference/007-compact-schema-reference-corrective.md`

Source subsystem roadmap:
`plans/subsystems/standalone-mcp-local-inference-roadmap.md`

## 1. Discovery

During M003's all-tools local-model API comparison, the full profile exceeded
the selected 24,000-token context with a 34,428-token request. The compact
request failed constrained decoding with `cannot resolve $ref
#/$defs/ResponseDetail, $defs not found`.

Inspection of the exact compact tool schemas found the cause in
`build_evidence_bundle`: its compact schema rewrite drops the root `$defs`
object while retaining the `response_detail` property that references
`ResponseDetail`. The failure is an Eggsearch-advertised schema defect. Earlier
schema tests did not resolve local references after the rewrite, and earlier
API tests restricted the allowed tool set.

## 2. Candidate and verification

Implementation candidate: `ff98098566a89f0603d1c07ad3cd88e5d36fae830bc8d6bde65cebc3456a71ea`
(SHA-256 of the rebuilt release executable).

The M008-compatible final branch candidate was rebuilt as release executable
SHA-256 `507724a14a1926488913a50b1864543f1e899d8a3df6e1b44d1a90dcc4110b43`
from source commit `772a073fd40e9f044017b26bae18a6efd0724b1d`. It retains the
M007 all-local-reference correction and includes M008's additive `render`
schema property.

The compact schema rewrite now retains the referenced `ResponseDetail`
definition. The regression test walks every compact tool schema and resolves
all local references against its root.

Local verification:

- `cargo test --locked --all-features --test mcp_schema_slimming`: 19 passed.
- On the final source candidate, `cargo test --locked --all-features --test mcp_schema_slimming`: 20 passed; `make docs-check` passed. Local `make check`
  stalled in the unchanged `bounded_command` integration suite, as recorded in
  M008's closure. Hosted canonical Linux and macOS gates passed on the exact
  source candidate, distinguishing the local process-group test stall from a
  reproducible regression.
- Compact executable smoke: all ten tools listed; full definitions measured
  72,923 bytes and compact definitions 19,494 bytes; `provider_status`
  completed successfully.
- Original `~/.lmstudio/mcp.json` was restored byte-for-byte; SHA-256
  `b8cb90299d832914cba5f92dbb259684a084045cc503fd186fea4b4cee3fec7c`.

All-tools LM Studio native API requalification used
`lfm3-2b-omni-abliter8`, a 24,000-token context, and no `allowed_tools` filter.
The full palette was rejected at 34,779 prompt tokens against 24,064
available. After the fix, a fresh compact-first request used 5,365 input and
362 output tokens, called `security_search` with valid arguments, and returned
the correct Log4j 2.14.1 affected-range conclusion with an advisory citation
while preserving Yahoo's 500 failure. This confirms compact schema
initialization and appropriate tool use for this task; it is one task, not the
broader M003 model/task matrix.

`make docs-check` passed. Hosted CI run `41220450246` passed Linux canonical,
macOS canonical and filesystem portability, and Windows portability on the
exact source candidate. Release drift run `41220450259` and Python native
qualification run `41220450258` also passed. The separate egress feature run
`41220450274` passed its Linux, x86_64 macOS, Windows, MSRV, and ARMv7 jobs;
the Apple Silicon macOS egress job was cancelled by a scheduled runner-image
brownout and does not cover the MCP schema change.

## 3. Disposition

M007 is closed at `772a073fd40e9f044017b26bae18a6efd0724b1d`. The final compact
schema suite resolves every local reference and passes its size budget. The
all-tools API qualification, hosted canonical gates, release drift, and Python
native qualification passed. M003 remains active for broader model/task
quality assessment.
