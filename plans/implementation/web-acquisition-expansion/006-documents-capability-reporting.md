# M006 — Documents Capability Reporting and Operator Switch

Status: closed
Date: 2026-11-25
Subsystem: web acquisition expansion
Milestone class: corrective
Baseline: `d932115d1f86af51710747d243455792ff63a6fb`
Decision authority: ADR-0026 Decision 2; ADR-0007 (a default-set move needs its own measured acceptance); M004 closure record

References: `plans/closure/web-acquisition-expansion/004-status.md`;
`src/core/config.rs`; `src/fetch/limits.rs`; `src/mcp/tools/provider_status.rs`;
`docs/features.md` § Document Formats.

## Objective

M004 shipped container-document extraction behind a non-default `documents`
feature, and its closure record names two gaps that are true even while the
feature stays optional: there is no operator switch equivalent to
`[fetch].pdf_enabled`, and `provider_status` reports nothing for container
documents. An agent therefore cannot discover support before calling, and an
operator cannot disable extraction in a build that has it. Close both without
changing default behaviour.

This is corrective work, so it is a new plan rather than an amendment to M004.

## Contract

- `[fetch].documents_enabled`, bool, defaults to the compiled-in feature state
  (`cfg!(feature = "documents")`), mirroring `pdf_enabled`.
- Feature absent: behaviour is unchanged and a container URL still returns
  `unsupported_content_type`. M004's feature-off contract is not weakened;
  the setting is inert because the code is absent.
- Feature present and `documents_enabled = false`: a detected container format
  fails with the typed `document_disabled` outcome before extraction, mirroring
  `pdf_disabled`. It never falls back to reading bytes as text.
- `provider_status` gains
  `documents_capabilities { compiled, configured, usable, formats }` and
  `server_capabilities.container_documents`, mirroring the PDF triad. `formats`
  lists the five supported labels.
- No change to the advertised tool count, aliases, default feature set, or the
  request surface.

## Ordered work packages

1. **Config and limits.** `FetchSection.documents_enabled` with a
   feature-derived default, `FetchLimits.documents_enabled`, and the
   `fetch_limits()` mapping.
2. **Gate.** Both the pre-body path and raw-cache re-derivation refuse a detected
   container with `document_disabled` when the feature is compiled in and the
   setting is off. A cache-derived body must not bypass the switch.
3. **Capability report.** The triad plus both payload placements.
4. **Tests.** The feature/config matrix, the disabled gate on both paths, and
   default parity.
5. **Docs and ratchet.** `docs/config.md`, `docs/features.md`,
   `docs/tool-matrix.md`, `architecture/fetch.md`, and the
   `src/core/config.rs` ratchet delta in `architecture/maintenance.md` with
   `tests/static_guards.rs`.

## Verification

`tests/documents.rs` covers the feature-on and feature-off matrix;
`tests/mcp_tools.rs` and `tests/provider_routing.rs` assert the capability
triad; `tests/config_validation.rs` covers the default and the round-trip.
Broad: `make check` under Rust 1.89 and `make packaging-check`.

## Acceptance and stop conditions

- The triad matches the compiled feature and the configured value in all four
  combinations.
- A build without the feature behaves exactly as before, including the
  `unsupported_content_type` outcome.
- Disabling the setting never degrades into treating a container as text.
- **Stop:** if reporting `compiled = false` honestly would require the feature
  to be compiled in. Honesty beats a tidy triad.

## Non-goals

- Making `documents` a default feature, or recording its size delta.
- Part selection over document parts.
- Per-format enable switches.
- Any change to OCR, layout, or writing/conversion.

## Closure evidence

The four-combination matrix, gate evidence on both fetch paths, the docs diff,
the ratchet delta, and `make check`.
