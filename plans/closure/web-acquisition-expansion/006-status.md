# Web Acquisition M006 — Documents Capability Reporting and Operator Switch — Status

Status: closed
Date: 2026-11-25
Milestone: M006 — Documents Capability Reporting and Operator Switch
Milestone class: corrective
Decision authority: ADR-0026 Decision 2; ADR-0007; M004 closure record
Baseline: `d932115d1f86af51710747d243455792ff63a6fb`

## Candidate and changes

Local candidate: the baseline plus this change set, uncommitted in the worktree
on `docs/user-facing-polish-20261123`. M004 closed with two gaps recorded: no
operator switch equivalent to `[fetch].pdf_enabled`, and no capability
report for container documents. Both are closed here as corrective work in their
own registered milestone rather than as an amendment to a closed plan.

- `src/core/config.rs` (2,055 lines): `default_documents_enabled()`,
  `FetchSection.documents_enabled`, the default, and the `fetch_limits()`
  mapping. The module ratchet moved from 2,043 to 2,055 lines / 80,768 bytes.
- `src/fetch/limits.rs`: `FetchLimits.documents_enabled`, defaulting to
  `cfg!(feature = "documents")`.
- `src/fetch/types.rs`: `FetchError::DocumentDisabled`,
  `FetchErrorKind::DocumentDisabled`, and the `document_disabled` code.
- `src/fetch/client.rs`: the gate on both the pre-body path and raw-cache
  re-derivation, so a cached body cannot bypass the setting.
- `src/mcp/tools/provider_status.rs`: the triad, the
  `documents_capabilities` block, and `server_capabilities.container_documents`.
- `tests/documents.rs`: the disabled gate and feature-off capability honesty.
- `tests/mcp_tools.rs`, `tests/provider_routing.rs`,
  `tests/config_validation.rs`: the capability and default assertions.
- `tests/static_guards.rs` and `architecture/maintenance.md`: the ratchet
  delta.
- `docs/config.md`, `docs/features.md`, `docs/tool-matrix.md`,
  `architecture/fetch.md`: the operator switch and the report.

## Four-combination matrix

| Feature | `documents_enabled` | compiled | configured | usable | Fetch outcome |
|---|---|---|---|---|---|
| present | `true` | true | true | true | extraction |
| present | `false` | true | false | false | `document_disabled` |
| absent | `true` | false | false | false | `unsupported_content_type` |
| absent | `false` | false | false | false | `unsupported_content_type` |

The third row is the honesty case this plan's stop condition names: a build
without the feature reports `compiled = false` and leaves the setting inert,
rather than claiming a configuration it cannot honour.

## Requirement-to-evidence matrix

| Criterion | Evidence | Outcome |
|---|---|---|
| Triad matches the feature and the setting in all four combinations | `provider_status_documents_capabilities_track_feature_and_config`; `capability_report_is_honest_without_the_feature` | pass |
| `server_capabilities.container_documents` agrees with the triad | same two cases | pass |
| Default equals the compiled feature state | `documents_enabled_defaults_to_the_compiled_feature` | pass |
| The setting parses and is honoured | `documents_enabled_parses_from_toml`; `disabled_setting_refuses_container_extraction` | pass |
| Feature absent behaves exactly as before | `docx_without_the_feature_keeps_the_unsupported_outcome`, `csv_without_the_feature_keeps_the_table_preview` | pass |
| A cache-derived body cannot bypass the switch | the gate sits in `derive_from_raw_scoped` as well as the pre-body path | pass |
| Disabling never degrades into reading bytes as text | typed error before extraction on both paths | pass |

## Implementation note

The `server_capabilities` flag ships as `container_documents`, not
`documents_fetch` as first drafted. `tests/docs_tool_names.rs` rejects
backticked `*_fetch` names in documentation that are not registered MCP tools,
and that guard is protecting a real ambiguity: a capability flag should not read
like a tool. Renaming the key is better than widening the guard's allowlist, and
the plan text carries the final name.

## Verification on this candidate

| Command | Outcome |
|---|---|
| `RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --test documents` | pass: 8 passed |
| `RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --test documents` (feature off) | pass: 3 passed |
| `cargo test --locked --all-features --test mcp_tools provider_status_documents` | pass: 1 passed |
| `cargo test --locked --all-features --test provider_routing` | pass: 99 passed |
| `cargo test --locked --features mock --test config_validation documents_enabled` | pass: 2 passed |
| `cargo test --locked --all-features --test static_guards` | pass: 69 passed |
| `RUSTUP_TOOLCHAIN=1.89.0 make check` | pass |
| `make packaging-check` | pass |

## Limitations

- The switch is compile-time scoped: a build without the feature cannot enable
  extraction by configuration.
- There is still no per-format switch; disabling refuses all five formats.
- Size delta for a default-set move and part selection over document parts
  remain out of scope; both are recorded in the M004 closure record.
