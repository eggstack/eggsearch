# Python Native Library M001 — Shared SDK Facade and Parity Closure Status

Status: closed

Source implementation plan:

- `plans/implementation/python-native-library/001-shared-sdk-facade-and-parity.md`

Source subsystem roadmap:

- `plans/subsystems/python-native-library-roadmap.md`

Applicable decisions:

- `plans/adrs/ADR-0001-ten-tool-surface-and-adapter-ownership.md`
- `plans/adrs/ADR-0005-python-native-library-boundary.md`

Implementation candidate: `5f686d02e15daa2c5db7ad6d889c06eaff3ceeee`.
Planning closure commit updates this record, the implementation plan,
roadmap, and registry together.

## 1. Delivered facade

`src/sdk.rs` adds `EggsearchClient`, `ToolName`, and structured `SdkError`.
The client builds one shared `ServerState` from validated `AppConfig`, clones
cheaply through `Arc`, accepts caller-supplied Tokio execution, and returns
tool JSON payloads without MCP envelopes. Dispatch is exhaustive and calls
the existing ten `src/mcp/tools/` handlers. Canonical names are checked
against `tool_contract::is_known_tool()` and enumerated by
`tool_contract::ALL_CONTRACTS`; malformed top-level values and typed argument
shape failures are `invalid_request`. Existing handler failures retain their
machine-readable code, message, repair hint, and data through
`ToolError::error_payload()`.

The facade introduces no PyO3 dependency, owned runtime, process, listener,
background task, provider policy, fetch policy, or new MCP operation. Root
Cargo metadata, features, package inventory, and default dependency graph are
unchanged. `src/lib.rs`, the normative specification, the maintenance
ownership map, the architecture index, and `AGENTS.md` now describe this
narrow native interface without granting general module-tree stability or
claiming that a Python package has shipped.

## 2. Ten-operation parity cases

The unit fixture `all_operations_match_their_existing_handler_results`
executes each facade operation and compares its complete `Result<Value,
SdkError>` to the same existing handler result using the same arguments and
shared state. All ten comparisons passed:

| Canonical operation | Fixture path | Result comparison |
|---|---|---|
| `web_search` | Mode-Off request | Exact JSON/error equality |
| `web_fetch` | Unsupported local-file URL | Exact JSON/error equality; no HTTP request |
| `batch_fetch` | Empty item list | Exact JSON/error equality |
| `provider_status` | Default, non-probing status request | Exact JSON equality |
| `repo_search` | Mode-Off request with local workspace disabled | Exact JSON/error equality |
| `repo_fetch` | Invalid owner locator | Exact JSON/error equality |
| `repo_map` | Invalid owner locator | Exact JSON/error equality |
| `security_search` | Mode-Off advisory request | Exact JSON/error equality |
| `research_search` | Mode-Off request | Exact JSON/error equality |
| `build_evidence_bundle` | Empty evidence input | Exact JSON equality |

The separate
`mode_off_repo_search_through_sdk_keeps_remote_dispatch_at_zero` fixture
installs a counting remote engine, indexes a local source file, then searches
through `EggsearchClient`. The local result is present, the reported provider
is `local_workspace`, and the remote call counter remains **0**. This covers
the native facade route through the same Mode-Off handler policy.

## 3. Verification

All gates below passed on candidate
`5f686d02e15daa2c5db7ad6d889c06eaff3ceeee`:

| Gate | Result |
|---|---|
| `cargo test --locked --all-features --lib sdk` | Pass; 4 SDK tests |
| `make check` | Pass; fmt, clippy, no-default-features check, all-features tests (3,478 tests), hygiene, dependency policy, packaging check |
| `RUSTDOCFLAGS="-D warnings" cargo doc --locked --all-features --no-deps` | Pass |
| `git diff --check` | Pass |

The all-features root gate includes `mcp_tools`, `evidence_contract`,
`provider_capability_contract`, and `static_guards`; it also exercises the
repository's pre-existing provider failure, projection, evidence identity,
warning, and local workspace contracts. The SDK-specific ten-operation test
uses deterministic local/Mode-Off inputs and makes no live network request.

## 4. Compatibility, security, and residuals

MCP tool registration and schemas, CLI behavior, Rust dependencies, release
assets, and Cargo package layout are unchanged. Every execution branch
delegates to the matching handler, so argument defaults, policy, budgets,
sanitization, stable IDs, warnings, partial results, and evidence semantics
remain owned by existing code. Mode-Off local repository search retains local
results and proves zero remote provider dispatch. The client creates no
Tokio runtime and no process or listener.

No high or medium residual finding was identified. PyO3 lifecycle, Python
types, cancellation, package metadata, wheel targets, sdist completeness,
and native Python consumer evidence remain explicitly outside M001 and are
gated by M002-M004.

Recommendation: **closed**. M001 acceptance criteria are met on the exact
candidate above. M002 is ready; M003 and M004 remain predecessor-gated.
