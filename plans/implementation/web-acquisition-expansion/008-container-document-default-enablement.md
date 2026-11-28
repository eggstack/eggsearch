# M008 — Container-Document Default Enablement

Status: closed
Date: 2026-11-26
Subsystem: web acquisition expansion
Milestone class: capability
Baseline: `40e362f1b16f6d2fb5efb4fb91a0c01a85c64144`
Decision authority: ADR-0027 (accepted 2026-11-26); ADR-0026 Decision 2, format,
dependency, bounds, and outcome contract preserved; ADR-0007 (the measured
default-enablement template); ADR-0014 §4; ADR-0024 §1 and §5.

References: `plans/adrs/ADR-0027-container-document-default-enablement.md`;
`plans/implementation/web-acquisition-expansion/007-container-document-extraction-fidelity-correctives.md`;
`plans/closure/web-acquisition-expansion/006-status.md`; `Cargo.toml`;
`bindings/python/Cargo.toml`; `bindings/python/Cargo.lock`; `Makefile`;
`tests/static_guards.rs`; `tests/documents.rs`; `tests/config_validation.rs`;
`src/core/config.rs`; `src/fetch/limits.rs`; `release/eggpack/pack.toml`;
`docs/features.md`; `docs/config.md`; `docs/python-library.md`;
`architecture/build.md`; `architecture/packaging.md`;
`architecture/python-library.md`; `architecture/fetch.md`;
`docs/test-inventory.md`; `architecture/testing.md`;
`skills/eggsearch-dev/SKILL.md`.

Long-term requirements: `plans/000-long-term-specification.md` §4 (bounded
acquisition) and §6-7 (artifact and distribution obligations);
`plans/001-terminology-and-domain-model.md` §4-5 (document and format
identity).

## Dependencies

- **Hard: M007 (satisfied).** Its `renderer_version` bump is what makes the flip
  safe for a URL a feature-off build already cached, and its extraction fixes are
  what make the capability shippable at all. M007's candidate is recorded in
  `plans/closure/web-acquisition-expansion/007-status.md`.
- **Operational: hosted wheel qualification.** The `Python native qualification`
  matrix must be green on the feature change; a local wheel build is not
  evidence.
- **Soft: Python PyPI distribution.** The wheel feature list is shared state;
  this milestone changes it, and the Python roadmap records the change rather
  than owning it.

## Readiness

Dependency-ready: M007 is closing, and its `renderer_version` 6 bump and
extraction correctives are on the candidate this milestone builds on. ADR-0027 is
accepted, so no architectural decision is outstanding; the interface contract this
milestone consumes — the `documents_capabilities` triad,
`[fetch].documents_enabled`, and the `document_*` outcome set — is already shipped
and documented by M006.

## Objective

Make container-document extraction a property of the artifact an operator
installs. `documents` joins the default feature set so the one published binary
per target and the shipped wheel carry it, the wheel lockfile carries its
dependencies, the feature stays switchable off by an operator without a rebuild,
and the per-target size cost is measured on the candidate rather than asserted.

User-visible exit: installing the published binary or wheel is enough to read a
DOCX, XLSX, PPTX, EPUB, or CSV from an explicit URL, `provider_status` reports
`documents_capabilities.compiled` as `true`, and the documentation no longer
asks the user to compile anything.

## Non-goals

- No extraction, bound, or outcome change. M007 owns all of it; if a defect
  appears during this milestone, it is a new corrective plan, not a silent fix
  here.
- No new MCP tool, alias, argument, config key, error kind, or wire string.
- No second SKU, no feature-suffixed asset, no release-matrix growth, no
  installer change. The seven asset names and the 19-name public inventory are
  unchanged.
- No change to `browser` or `pdf` availability; `egress` stays out of the
  default set and `mock` stays test-only.
- No removal of `[fetch].documents_enabled` — ADR-0027 alternative 3 is
  rejected and the switch is the operator refusal path.
- No MSRV move and no new dependency edge beyond what the `documents` feature
  already declares.
- No payload or projection change: enabling a format family must not alter the
  size of a response that does not contain one.

## Current implementation evidence

- `Cargo.toml:43` is `default = ["pdf", "browser"]`; `documents` is
  `["dep:zip", "dep:calamine"]` with `zip = "=9.8.1"` and `calamine = "0.38"`.
- `tests/static_guards.rs:1911` `default_features_ship_pdf_and_browser` asserts
  that `default` contains `pdf` and `browser`, that it does not contain
  `egress`, and that `default_browser_enabled` is feature-derived. It does not
  mention `documents`, so it neither blocks nor protects this change.
- `[fetch].documents_enabled` is already feature-derived:
  `default_documents_enabled()` is `cfg!(feature = "documents")`
  (`src/core/config.rs:431`, used by the serde default at `:761` and
  `FetchSection::default` at `:871`), and `FetchLimits::default` sets the same
  expression (`src/fetch/limits.rs:208`). `Config::fetch_limits()` copies the
  config field (`src/core/config.rs:1377`), so no code change is needed for the
  shipped default to become `true`; the change is the feature set itself.
- `tests/config_validation.rs:9` asserts
  `config.fetch.documents_enabled == cfg!(feature = "documents")` and therefore
  passes in both worlds; `tests/mcp_tools.rs:313` already loops the full
  four-combination capability matrix. Neither needs an edit, and a candidate
  that requires one has changed production behaviour and violated this plan.
- `bindings/python/Cargo.toml:16` is
  `default-features = false, features = ["pdf", "browser"]`.
- `bindings/python/Cargo.lock` contains neither `zip` nor `calamine` (378
  packages); the root `Cargo.lock` contains both. The lockfile is a tracked
  release input (`packaging/release-inputs.txt`) and qualification builds with
  `--locked`, so it must be regenerated in the same change.
- The producer needs no edit: the generated release workflow builds with neither
  `--features` nor `--no-default-features`, so default-set membership is the
  only lever. `release/eggpack/` carries no feature information.
- `make check` is `fmt clippy feature-check test hygiene dependency-policy
  packaging-check`; `feature-check` is `cargo check --locked
  --no-default-features` and `test` is `cargo test --locked --all-features`.
  **No target runs the test suite with default features disabled**, so once
  `documents` is default the feature-off parity half of `tests/documents.rs`
  becomes unreachable in CI while remaining green-by-absence.
- Measured at `v0.4.4` (public release assets, read 2026-11-26) with `pdf` and
  `browser` and no container support: `aarch64-apple-darwin` 17,960,304;
  `aarch64-pc-windows-msvc` 21,655,040; `aarch64-unknown-linux-gnu` 18,524,080;
  `armv7-unknown-linux-gnueabihf` 17,710,392; `x86_64-apple-darwin` 19,779,368;
  `x86_64-pc-windows-msvc` 25,869,312; `x86_64-unknown-linux-gnu` 21,457,240
  bytes. Three already exceed ADR-0024 §5's 20 MB review figure, so that figure
  is a review trigger satisfied by a written reason, not an absolute ceiling.
- Six documents state the pre-decision posture and are falsified by the change:
  `docs/features.md` § Document Formats (not in the default set; `cargo build
  --features documents`), `docs/config.md` (`documents_enabled` is `false` in
  every default build and in the wheel), `architecture/build.md` (default
  features include `pdf` and `browser`), `architecture/packaging.md` (default
  features build every release asset; wheel feature list),
  `architecture/python-library.md` (wheel compiles `pdf` and `browser`), and
  `docs/python-library.md` (the same wheel feature list).

## Invariants that cannot regress

- Seven release asset names, `.exe` suffixes on the two Windows rows, `.sha256`
  sidecars, version-free naming, and the 19-name public inventory are unchanged.
  `make producer-drift` stays clean without a producer edit.
- Exactly nine advertised tools plus three aliases; every argument name and wire
  string unchanged.
- `document_disabled` remains reachable and documented: an operator who sets
  `[fetch].documents_enabled = false` refuses every detected container before
  parsing, on both the pre-body path and raw-cache re-derivation.
- `unsupported_content_type` remains reachable for a body that is neither text,
  HTML, PDF, nor a recognised container, and is what a build with the feature
  disabled still returns for a container URL.
- The feature-off build keeps compiling and keeps its behaviour, verified by a
  gate rather than by inspection.
- Trust framing, sanitisation tiers, SSRF and redirect policy, and byte caps are
  untouched.

## Expected production changes

| File | Change |
|---|---|
| `Cargo.toml` | `default = ["pdf", "browser", "documents"]` |
| `bindings/python/Cargo.toml` | `features = ["pdf", "browser", "documents"]` |
| `bindings/python/Cargo.lock` | regenerated with `zip` and `calamine` |
| `Makefile` | the feature-off parity target described in package 3 |
| `tests/static_guards.rs` | the default-set guard covers `documents` and still rejects `egress` |
| `tests/documents.rs` | only if an invocation or gate change requires it; no assertion weakening |

No production source change is expected in `src/`. If package 3 cannot be
satisfied without one, stop rather than widen the milestone.

## Ordered work packages

1. **Default feature set.** Add `documents` to `Cargo.toml` `default`. Confirm
   `cargo check --locked --no-default-features` still passes so the lean shape
   stays compilable, and that the documented reproduction of the old build
   (`--no-default-features --features pdf,browser`) builds. Extend
   `tests/static_guards.rs` `default_features_ship_pdf_and_browser` to require
   `documents` as well, keep the `egress` rejection, and add the assertion that
   `default_documents_enabled` is feature-derived, mirroring the browser clause
   so the config default cannot drift from the feature.
2. **Wheel feature set and lockfile.** Add `documents` to
   `bindings/python/Cargo.toml`, regenerate `bindings/python/Cargo.lock`, and
   confirm the wheel imports and extracts from Python without a rebuild path.
   A `--locked` build is the acceptance evidence.
3. **Feature-off parity lane.** `make check` no longer runs the test suite with
   the feature disabled, so restore an explicit lane: a target that runs the
   documents suite with defaults disabled, wired into `check` alongside
   `feature-check`, with runtime cost stated in the closure record. Update
   `docs/test-inventory.md`, `architecture/testing.md`, and
   `skills/eggsearch-dev/SKILL.md` for the changed invocation, which the
   repository requires when a suite's requirement changes.
4. **Default coherence.** Prove with no production change that the four
   combinations still report honestly: feature compiled or not, setting true or
   false, on both fetch paths, with `tests/config_validation.rs` and
   `tests/mcp_tools.rs` green unmodified. A `document_disabled` refusal on a
   default build is part of the evidence, not an edge case.
5. **Size and footprint.** Measure every target on the exact candidate against
   the `v0.4.4` bytes above: absolute size per target, delta per target, and the
   wheel delta. State a written reason for each target over the 20 MB review
   figure. Do not present a delta percentage without the absolute figure beside
   it.
6. **Packaging and dependency policy on the default path.** `make
   dependency-policy` and `make packaging-check` must resolve `zip` and
   `calamine` on the default graph rather than only under `--all-features`, and
   `make producer-drift` must stay clean with no producer edit. The 1.89 MSRV
   default build is part of this package; `zip 9.8.1` sits exactly on 1.89.
7. **Documentation and release facts.** Correct the six statements listed in
   current evidence, document the two behaviour changes ADR-0027 names (a CSV
   URL now returns a `document`; ODF, RTF, and legacy binary Office now return
   `document_unsupported`), and add an `Unreleased` changelog entry for the
   default move. Update `docs/features.md` § Document Formats so the first
   sentence describes the shipped artifact and the source-build shape is the
   secondary paragraph, and record the `renderer_version` value the release
   carries.
8. **Capability report review.** Read `provider_status` output from a default
   build and confirm the triad, `server_capabilities.container_documents`, and
   the format list are accurate in every state a default install can reach,
   including the runtime-discovered absence of a browser, which must not affect
   the document triad.

## Failure and recovery semantics

The failure modes of this milestone are build and distribution failures, not
runtime ones, and each fails closed at a gate rather than at an operator:

- **A target does not build with the new default.** The release workflow fails at
  that target before publication; the milestone does not merge a partial matrix
  and does not reorder assets. Recovery is ADR-0027 alternative 1 — revert the
  default and keep the capability optional — not a per-target feature set.
- **The wheel does not build or does not qualify.** `--locked` fails on a stale
  `bindings/python/Cargo.lock`; regeneration is part of the change, and a wheel
  that imports but cannot extract is a failed qualification, not a partial pass.
- **A size figure is unexplained.** Publication pauses for review; there is no
  automatic rollback and no silent threshold change.
- **The parity lane cannot run in the gate.** Stop and re-plan rather than
  dropping the lane, because a lane that exists on paper is worse than none.

No work package adds a timeout, a retry, or a runtime fallback: extraction
behaviour is M007's contract, and a fallback that reads a container as text would
violate it.

## Verification

```bash
RUSTUP_TOOLCHAIN=1.89.0 cargo check --locked --no-default-features
RUSTUP_TOOLCHAIN=1.89.0 cargo check --locked --no-default-features --features pdf,browser
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --features documents --test documents
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --no-default-features --test documents
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features
make check
make packaging-check
make dependency-policy
make producer-drift
```

Plus, outside the gate: a release-profile build of the candidate on at least the
host target for the size figures, and the hosted `Python native qualification`
matrix on the wheel change. Rotate the local probe form

```bash
grep -n 'name = "calamine"' bindings/python/Cargo.lock
cargo metadata --locked --format-version 1 --no-deps | python3 -c 'import json,sys; d=json.load(sys.stdin); print({p["name"]: p["features"]["default"] for p in d["packages"]})'
```

to prove the lockfile and the declared default rather than reading them. At
baseline the first prints nothing with exit 1 and the second prints
`{'eggsearch': ['pdf', 'browser']}`; on the candidate the first names the
package and the second lists all three features.

Broad: `make check` on the exact candidate, and the hosted workflows for the
wheel and the release candidate. A green local run is not qualification.

## Acceptance and stop conditions

- The candidate's default feature set is `["pdf", "browser", "documents"]`, the
  guard enforces it, and `egress` is still excluded.
- `bindings/python/Cargo.lock` contains `zip` and `calamine` and a `--locked`
  wheel build succeeds; the hosted matrix is green.
- A default build reports `compiled: true`, `configured: true`, `usable: true`
  and extracts all five formats; setting `documents_enabled = false` yields
  `document_disabled` on both fetch paths; a build with the feature disabled
  still compiles and still returns `unsupported_content_type`.
- The feature-off parity lane runs in `make check` and its cases execute rather
  than being skipped, demonstrable by count.
- Per-target absolute sizes and deltas are recorded, with a reason for every
  target over the 20 MB review figure, and the wheel delta is recorded.
- `tests/config_validation.rs` and `tests/mcp_tools.rs` are green **unmodified**.
- **Stop and return to ADR-0027 alternative 1** if a target fails to build with
  the new default, if the wheel matrix fails to qualify, or if a size figure is
  unexplained past the review threshold.
- **Stop and re-plan** if the default flip requires a change in `src/`; the
  config and capability defaults are already feature-derived and a source change
  signals that assumption was wrong.
- **Stop and re-plan** if the feature-off parity lane cannot run within the gate
  budget; a parity lane that exists on paper only is worse than none.

## Handoff notes

Re-measure the published baseline before writing any size figure; the ones in
this plan were read on 2026-11-26 at `v0.4.4` and a newer release invalidates
them. Run the parity lane before the default flip as well as after, so a case
that was already skipped is distinguishable from one the change skipped. Treat
`tests/config_validation.rs` and `tests/mcp_tools.rs` as canaries: if either
needs an edit, the assumption that the config default is feature-derived is
wrong and the milestone must stop.

## Closure evidence

`plans/closure/web-acquisition-expansion/008-status.md` records: the candidate
SHA and the M007 candidate it builds on; the declared default feature set with
the guard output; the regenerated lockfile evidence and the `--locked` wheel
build; the hosted `Python native qualification` run identifier; the
four-combination capability and refusal matrix on both fetch paths; the
feature-off parity lane with its executed case count; the per-target absolute
size and delta table against the `v0.4.4` figures with the review-threshold
reasons; the wheel size delta; `make check`, `make packaging-check`,
`make dependency-policy`, and `make producer-drift` outcomes; the documentation
diff; the changelog entry; and the `renderer_version` the release carries.
