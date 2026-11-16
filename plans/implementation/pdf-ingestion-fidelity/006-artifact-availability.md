# M006 — Artifact Availability

Status: ready
Source: `plans/subsystems/pdf-ingestion-fidelity-roadmap.md` M006
Primary class: capability
Baseline: `9d4d16fc32500196a1b88554b898cd8eddc84731`
References: `plans/adrs/ADR-0007-pdf-artifact-availability.md` § Decision, § Verification and status; `plans/subsystems/pdf-ingestion-fidelity-roadmap.md` § 3, § 4, § 10; `Cargo.toml` § features; `src/core/config.rs` § FetchConfig; `src/fetch/client.rs` § PDF gates; `src/fetch/cache.rs`; `src/mcp/tools/provider_status.rs`; `release/eggpack/build-bindings.toml`; `release/eggpack/distribution.toml` § targets; `packaging/expected_release_assets.py`; `packaging/test-install.sh`; `packaging/test-install.ps1`; `bindings/python/Cargo.toml`; `docs/retrieval-quality-extraction-qualification.md`; `architecture/build.md` § Feature flags matrix; `architecture/packaging.md` § Target contract; `docs/features.md`; `docs/config.md`; `docs/python-library.md` § Compatibility and features.
Dependencies: ADR-0007 accepted (decision satisfied); hard M001. M002–M005 are not gating, but the milestone records their status at handoff because ADR-0007 notes that enabling the default raises the cost of an extraction regression.

## Objective

Make PDF ingestion a property of the artifact an operator installs, not of a source build. Concretely: `pdf` joins the default feature set so the one published binary per target and the shipped wheel carry extraction, `[fetch].pdf_enabled` defaults to `true` when the feature is compiled in so a capable binary is not a refusing binary, and `provider_status` reports one coherent capability story across compiled-in, enabled, and budget state — with the +1,349,120 B (+7%) cost measured per target rather than asserted.

## Non-goals

- No extraction, chunking, or payload change: M003, M004, and M005 own those, and this milestone changes no extracted character.
- No `renderer_version` bump. No cached field changes, so the value M001 lands stays put.
- No second SKU, no new asset name, no feature-suffixed binary, no producer-contract edit.
- No change to `browser` or `egress` availability; `egress` must remain out of `default`.
- No new MCP tool, argument, config key, error kind, or renamed wire string.
- No OCR, rasterization, or renderer; no second PDF parser.
- No MSRV bump and no new dependency edge beyond what the existing `pdf` feature already declares.

## Current implementation evidence

- `Cargo.toml` declares `default = []` and `pdf = ["dep:lopdf"]`, with `lopdf 0.42` optional and `default-features = false`.
- `[fetch].pdf_enabled` has three defaults that must agree: the bare `#[serde(default)]` attribute, `FetchConfig::default()`, and `FetchLimits::default()`, all currently `false`. Production resolves through `Config::fetch_limits()`, which copies the config field, so the first two decide the shipped default; `FetchLimits::default()` is what test clients construct, and leaving it `false` would make default-build tests exercise the refusal path instead of extraction. No validation rule references the field, so no code path rejects `pdf_enabled = true` in a build without the feature.
- Both fetch gates test compiled-in state first: `kind.is_pdf && !cfg!(feature = "pdf")` yields `PdfNotCompiledIn`, and only then `!limits.pdf_enabled` yields `PdfDisabled` (`src/fetch/client.rs` § PDF gates, two sites). The refusal taxonomy therefore already cannot report `pdf_disabled` on a feature-less build, whatever the config says.
- `provider_status` computes `pdf_capabilities { compiled, configured, usable }` but sets `server_capabilities.pdf_fetch` and `pdf_text` from the compiled-in bit alone. A feature-enabled binary with `pdf_enabled = false` already advertises `pdf_fetch: true` while `pdf_capabilities.usable` is `false`.
- Producer facts carry no feature information: `release/eggpack/build-bindings.toml` requests one `direct` output per target with no flags, `distribution.toml` names assets `{product}-{target}` plus a `.sha256` sidecar (`.exe` on the two Windows rows), and `packaging/expected_release_assets.py` derives the 19-name inventory from that contract. The generated workflow builds `cargo +stable build --release --locked --target <triple> --package eggsearch --bin eggsearch` with neither `--features` nor `--no-default-features`.
- The wheel inherits the default: `bindings/python/Cargo.toml` depends on `eggsearch` by path with no `default-features = false`, while `bindings/python/Cargo.lock` contains no `lopdf` and the qualification workflow builds `maturin ... --locked`.
- Four documents state the pre-decision posture and are falsified by it: `docs/features.md` requires `cargo build --features pdf` and `pdf_enabled = true`; `docs/config.md` lists `pdf_enabled` defaulting to `false`; `architecture/build.md` § Feature flags matrix lists `default` as enabling nothing; `architecture/packaging.md` § Target contract states PDF is never part of the default product, and `docs/python-library.md` § Compatibility and features states the binding enables no `pdf` feature.
- `tests/config_validation.rs` `default_config_has_expected_values` asserts `!cfg.fetch.pdf_enabled`, which the `--all-features` run inside `make check` will fail once the default becomes feature-conditional.
- Measured cost at baseline: 20,659,408 B PDF arm against 19,310,288 B HTTP-only, +1,349,120 B (+7%), on a macOS x86_64 host with neither arm carrying browser (`docs/retrieval-quality-extraction-qualification.md` § Results).
- `deny.toml` uses an `all-features` graph and already carries a dated RUSTSEC ignore naming `ttf-parser` reached through optional `lopdf`, so dependency policy resolves the PDF graph today.

## Invariants that cannot regress

- One executable per target, version-free asset names, and the 19-name public inventory are unchanged. No asset is added, renamed, or split.
- Ten MCP tools, existing argument names, and existing wire strings are unchanged. `pdf_not_compiled_in` stays in the taxonomy for source builds that disable the feature.
- An explicit `[fetch].pdf_enabled = false` keeps refusing PDFs with `pdf_disabled` under the new default.
- All existing bounds stay in force: response byte caps, per-page and total character caps, page count, timeouts, deadlines, SSRF and redirect policy. `lopdf` stays the only PDF parser.
- `browser` and `egress` stay non-default; `architecture/build.md` requires that `default` never enables `egress`.
- No extraction-output change, so `renderer_version` is not bumped by this milestone and no cached document becomes stale.
- Producer authority stays `release/eggpack/` plus the generated workflow. This milestone changes no producer input.

## Expected production changes

1. **Feature default.** `Cargo.toml` gains `default = ["pdf"]`. `--no-default-features` remains the supported minimal and source-build path, and keeps compiling the feature-absent branches that `make check`'s `feature-check` exercises.
2. **Config default.** Add a `default_pdf_enabled()` helper returning `cfg!(feature = "pdf")`, use it in the `#[serde(default = ...)]` attribute on `pdf_enabled`, and use it in `FetchConfig::default()`. The feature-conditional value is what makes the default safe when the feature is absent: a source build resolves the field to `false` and its first gate is compiled-in anyway, so the two layers agree instead of contradicting each other.
3. **Capability agreement.** Bind `server_capabilities.pdf_fetch` and `pdf_text` to the same compiled-in ∧ configured predicate that produces `pdf_capabilities.usable`, keeping the existing object shape and adding no field. The effective budgets are validated positive values rather than gates, so they are limits, not a third usability term.
4. **Tests.** Make the default-config assertion feature-aware, and cover the absent-key default under default, `--no-default-features`, and `--all-features`. Assert the four (compiled, enabled) capability combinations.
5. **Wheel lockfile.** Regenerate `bindings/python/Cargo.lock` so the tracked lock gains `lopdf` and its transitive packages; the qualification lane builds `--locked` and fails on a stale lock.
6. **No producer change.** `release/eggpack/` and the generated workflow are untouched, and `make producer-drift` must show a clean diff. The default set is inherited by the existing build commands, which is the whole reason no producer edit is needed.

## Storage, protocol, and compatibility effects

- One default changes: absent `[fetch].pdf_enabled` now resolves to `true` in feature-enabled builds. Explicit values, including `false`, behave exactly as today.
- Response shape is unchanged; only the values inside `pdf_capabilities` and the two `server_capabilities` bits move for default installs, and they now agree with each other.
- Source builds with `--no-default-features` still return `pdf_not_compiled_in`; the message stays actionable and names the feature.
- Public asset names, installer URLs, checksum sidecars, and the 19-asset count are unchanged, so installs, updates, and existing `releases/latest/download` links keep resolving.
- Python wheels gain PDF ingestion through the inherited default; wheel size grows and `bindings/python/Cargo.lock` moves with it, while distribution mechanics are untouched.
- No migration and no cache invalidation.

## Ordered work packages

1. Add `default = ["pdf"]` and route all three `pdf_enabled` defaults through one feature-conditional helper using `cfg!(feature = "pdf")`; confirm `--no-default-features` still compiles.
2. Update `tests/config_validation.rs` for the feature-aware expectation, add absent-key and explicit-`false` cases, and assert `FetchLimits::default().pdf_enabled` matches the compiled-in state.
3. Align `provider_status` capability bits with `pdf_capabilities.usable` and assert all four combinations in `tests/mcp_tools.rs`.
4. Regenerate `bindings/python/Cargo.lock`; build one wheel locally with `maturin build --manifest-path bindings/python/Cargo.toml --release --locked` as a smoke before the matrix run.
5. Measure the same-SHA size pair per target: `cargo build --release --locked --target <triple>` against `--no-default-features` on the same tree, recording both byte counts.
6. Re-run the extraction qualification procedure in `docs/retrieval-quality-extraction-qualification.md` on the candidate; run installer smoke with `packaging/test-install.sh` and `packaging/test-install.ps1`.
7. Dispatch `.github/workflows/qualify-eggpack-candidate.yml` for the per-target artifacts and `.github/workflows/python-native-qualification.yml` for the wheel matrix; record outcomes per target.
8. Update the owning docs and append the `CHANGELOG.md` entry; prove no producer drift.

## Verification

- `cargo check --locked --no-default-features` and `cargo check --locked` (default set now resolves `lopdf`).
- `cargo test --locked --test config_validation` and `cargo test --locked --all-features --test config_validation`.
- `cargo test --locked --all-features --test mcp_tools`.
- `cargo test --locked --features mock --test web_fetch_integration`.
- `cargo +1.89.0 check --locked` and `cargo +1.89.0 check --locked --no-default-features`, so the MSRV claim covers the default graph rather than only `--all-features`.
- `make dependency-policy`; `make producer-drift`; `make publish-check`.
- `bash packaging/test-install.sh`; `pwsh packaging/test-install.ps1` on a Windows host.
- `make check` on the exact candidate, then `make release-check`.

## Static guards and documentation updates

- `packaging/check-contract.sh` stays green with no edit; `packaging/expected_release_assets.py` still yields 19 names.
- `tests/docs_tool_names.rs`, `tests/docs_provider_inventory.rs`, and `tests/static_guards.rs` stay untouched and green.
- No new suite: behavior extends `config_validation`, `mcp_tools`, and `web_fetch_integration` per `architecture/testing.md`; `docs/test-inventory.md` counts stay accurate because no suite is added.
- `docs/features.md` states PDF as an installed default with `pdf_enabled = false` as the refusal switch; `docs/config.md` records the feature-conditional default and the source-build exception.
- `docs/installation.md` states that an installed release ingests PDFs with no extra configuration; `docs/python-library.md` § Compatibility and features replaces the no-`pdf`-feature claim.
- `architecture/build.md` § Feature flags matrix and `architecture/packaging.md` § Target contract describe `pdf` in the default set while `browser` and `egress` stay opt-in; `skills/eggsearch-release/SKILL.md` § Feature Flags marks `pdf` default as yes.
- `CHANGELOG.md` gains an append-only entry under the existing unreleased section.

## Acceptance and stop conditions

- Default-feature and `--all-features` builds report `pdf_capabilities.compiled` and `usable` true, and `server_capabilities.pdf_fetch` agrees with them. A build with `pdf_enabled = false` reports `usable` false and matching bits.
- An installed release from the candidate ingests an ordinary text PDF with no source build and no configuration, and the refusal for an explicitly disabled install is `pdf_disabled`.
- `--no-default-features` still returns `pdf_not_compiled_in`.
- Per-target size delta is recorded for all seven published targets; the extraction qualification re-run, installer smoke, and wheel matrix outcomes are recorded; `make dependency-policy`, the MSRV default and no-default checks, and `make check` pass on the exact candidate.
- `make producer-drift` reports no change, and the staged inventory is still 19 assets.
- Stop if armv7, either Windows target, or any wheel-matrix cell fails to build or qualify: do not introduce a second SKU, revert to ADR-0007 alternative 1, and record the revert. Stop if the change would require hand-editing the generated release workflow. Stop if the default change alters the published asset inventory without a producer-authority update. Stop and escalate if `renderer_version` turns out to need a bump because some cached field changes.

## Closure evidence

`plans/closure/pdf-ingestion-fidelity/006-status.md` records the candidate SHA; the per-target size table with both same-SHA build sizes and the computed delta; the extraction qualification re-run results; installer and wheel matrix outcomes per platform; `make dependency-policy`, MSRV, and `make check` output; the `producer-drift` and 19-asset proofs; M002–M005 status at handoff; and any target that failed to build or qualify, or an explicit statement that none did.
