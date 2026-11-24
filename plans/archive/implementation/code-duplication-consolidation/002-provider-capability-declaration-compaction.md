# M002 — Provider Capability Declaration Compaction

Status: closed
Closure: `plans/closure/code-duplication-consolidation/002-status.md`
Date: 2026-11-22
Subsystem: code duplication consolidation
Milestone class: polish (mechanical, fully pinned by tests)
Baseline: `fb9d6538f8ea2e21e08add43fb3f71ebe6ceaa64` on `feat/auto-20261117-044c4885`
Decision authority: no new ADR. Capability semantics are owned by
`tests/provider_capability_contract.rs`; this milestone must not change a single flag.

References: `plans/subsystems/code-duplication-consolidation-roadmap.md` §4, §7;
`architecture/engines.md` § Provider capabilities; `docs/provider-setup.md`;
`tests/provider_capability_contract.rs`.

Dependencies: none.

## Objective

Express each provider's capability set as `ProviderCapabilities::none()` plus the flags that
provider actually enables, instead of a 24-field literal per provider, and pin the resulting
matrix with one table-driven test before and after the change.

## Current evidence

Measured 2026-11-22 on the baseline:

- `built_in_provider_descriptor` (`src/core/provider.rs:578`) is a 44-arm match over
  `KNOWN_PROVIDER_IDS` (44 ids).
- 39 of those arms write a full `ProviderCapabilities { … }` literal of 24 fields
  (`src/core/provider.rs:180`–`234`); five use `ProviderCapabilities::none()`.
- That is roughly 936 lines of explicit `false` fields inside one function.
- `ProviderCapabilities::none()` already exists (`src/core/provider.rs:238`), so the struct-update
  form needs no new API.
- `tests/provider_capability_contract.rs` asserts individual claims for the providers that matter
  (brave_api, exa, tavily, firecrawl_developer, serpapi, kagi, nvd) and the keyless family, but it
  does not pin all 24 flags for all 44 ids.

## Invariants that cannot regress

- Every descriptor's 24 capability flags are identical to the baseline for all 44 ids.
- `id`, `display_name`, `kind`, `requires_api_key`, `configured`, `routable`, `skip_reason`, and
  `skip_code` are unchanged.
- The capability claim strings in `tests/provider_capability_contract.rs` remain the source of
  truth. If the pinned matrix disagrees with a claim, the milestone stops and reports a defect;
  it does not edit the claim to match the code.
- No provider is added, removed, renamed, or re-credentialed.

## Work packages

1. **Pin the matrix first.** Add `capability_matrix_matches_the_pinned_table` to
   `tests/provider_capability_contract.rs` with a sparse table:
   `const MATRIX: &[(&str, &[&str])]` listing, per id, only the flag names that are `true`. The
   test builds an all-false struct from one explicit literal, flips the named flags, and asserts
   equality with `descriptor(id).capabilities`. It also asserts the table covers exactly
   `KNOWN_PROVIDER_IDS`, once each. This test must pass before any production edit; it is the
   equivalence proof.
2. **Compact the declaration.** Replace each full literal with
   `ProviderCapabilities { supports_x: true, ..ProviderCapabilities::none() }`, keeping the field
   order and doc comments. Do not reorder `ProviderCapabilities` fields.
3. **Remove dead explicit `false` fields** and confirm `cargo fmt` and clippy are clean.
4. **Run the pin after each family** (HTML scrapers, forges, advisories/registries, scholarly,
   API-key providers) so a mistake is attributed to one batch.
5. **Record the alternative that was not taken.** The roadmap defers a `const PROVIDERS` table;
   note in the closure record that the static table remains available if a future provider count
   makes the match unwieldy.

## Expected production changes

| File | Change |
|---|---|
| `src/core/provider.rs` | 39 full capability literals become `none()` plus overrides |
| `tests/provider_capability_contract.rs` | New pinned-matrix test |

No other file should change. `docs/provider-setup.md` and `architecture/engines.md` describe
native enforcement per provider and stay true because no flag changes.

## Non-goals

- Introducing a `const PROVIDERS: &[ProviderStatic]` table or a capability builder API.
- Changing any capability flag, provider kind, credential requirement, or default fan-out.
- Moving provider descriptors out of `core`.
- Touching `provider_status` capability keys (they stay callable through the alias set).

## Compatibility and migration

None. Descriptors serialize identically; the change is source-shape only.

## Verification

```sh
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --test provider_capability_contract
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --test provider_routing
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --lib core::provider
RUSTUP_TOOLCHAIN=1.89.0 make check
```

Failing-before evidence: flip one flag in the new matrix test and show it fails against the
baseline code; restore it. Record the failure.

## Acceptance and stop conditions

- The matrix test passes before and after the compaction, for all 44 ids.
- The descriptor function loses the duplicated 24-field blocks; the diff contains no flag change.
- `make check` passes and the provider suites are unchanged.
- **Stop:** if the matrix test exposes a flag that no claim string covers and that looks wrong,
  record it as a finding and ask for a maintainer decision. Do not "fix" a capability silently in
  a polish milestone.

## Required closure evidence

The closure record carries the pinned-matrix test output, the before/after line counts for
`built_in_provider_descriptor`, the one-flag failing-before run, and a statement that all 44
descriptor flag vectors are byte-identical to the baseline.
