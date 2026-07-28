# CI, Verification, and Manual Release Closure Follow-Up

**Repository:** `eggstack/eggsearch`  
**Baseline:** `89a3d6d66bd2e059934912f25af72059361913b6`  
**Status:** Small-model implementation handoff  
**Scope:** Narrow closure of the CI/manual-release simplification pass  
**Predecessor:** `plans/ci-verification-and-manual-release-simplification.md`  
**Release policy:** Manual maintainer publication to crates.io  
**GitHub Actions release role:** None

---

## 1. Objective

The principal simplification has landed correctly:

- the former forty-job push/pull-request workflow is now one Ubuntu job;
- the scheduled native-forge workflow is gone;
- the scheduled fuzz workflow is gone;
- the mutable release-verification ledger is gone;
- workflow-shape and release-record contract tests are gone;
- routine verification, release verification, and optional diagnostics are separated;
- crates.io publication is explicitly manual.

This follow-up closes only the residual defects that prevent the line of work from being considered complete:

1. `make check` still performs both a no-default compile check and a no-default full test pass, retaining avoidable duplication in the routine path;
2. `make native-forge-smoke` runs every provider test even when documentation supplies credentials for only one provider;
3. active agent documentation still references deleted Makefile targets;
4. active documentation and skills contain stale fuzz-target counts and command drift;
5. the simplified workflow has not yet been conclusively validated through one visible successful `CI / ci` run;
6. repository branch protection may still require names from the retired matrices and standalone jobs.

The desired final state is:

```text
routine local verification
    make check
    -> fmt
    -> clippy
    -> no-default compile isolation
    -> one all-features deterministic test pass

remote verification
    one GitHub Actions job: CI / ci
    -> make ci

release preparation
    make release-check
    -> routine verification
    -> rustdoc
    -> release build
    -> cargo publish --dry-run --locked

publication
    maintainer runs cargo publish --locked

optional native adapter diagnostics
    provider-specific commands
    -> only the selected provider test runs
```

This is a closure pass, not another verification redesign.

---

## 2. Fixed Decisions

The following decisions are authoritative for this pass.

### 2.1 Keep one no-default compile check, not a second full test pass

The routine gate will retain:

```bash
cargo check --locked --no-default-features
```

The routine gate will remove:

```bash
cargo test --locked --no-default-features
```

Rationale:

- the no-default configuration must continue to compile;
- `cargo test --locked --all-features` runs the substantive deterministic test inventory, including `mock` and `pdf`-gated coverage;
- a second full test execution under no-default features adds thousands of repeated tests and compilation work to the routine path;
- no current evidence demonstrates a release-critical behavioral test that only has meaning under `--no-default-features`;
- if implementation inspection discovers a specifically `cfg(not(feature = ...))` behavioral test that cannot be covered by the retained commands, name that test explicitly rather than restoring a blanket no-default full-suite pass.

Do not remove the no-default compile check.

### 2.2 Native forge diagnostics must be provider-specific

The ambiguous target:

```text
make native-forge-smoke
```

must not silently execute all credentialed provider tests when only one provider's credentials are supplied.

Use explicit targets:

```text
make native-forge-smoke-github
make native-forge-smoke-gitlab
make native-forge-smoke-codeberg
make native-forge-smoke-gitea
```

An optional aggregate target may be named:

```text
make native-forge-smoke-all
```

only if it is documented as requiring every provider credential and fixture. Do not keep `native-forge-smoke` as an ambiguous alias to all providers.

### 2.3 Active documentation must contain only executable commands

No active README, agent instruction, skill, architecture document, or testing guide may reference a Makefile target that does not exist.

Historical plan files may retain old commands as historical context. Do not rewrite old plans.

### 2.4 Avoid brittle exact counts outside the authoritative inventory

Active contributor and agent guidance should not repeatedly claim an exact number of fuzz targets. The fuzz target count has changed several times and has already drifted.

Use wording such as:

```text
Fuzz targets live under fuzz/fuzz_targets and are registered in fuzz/Cargo.toml.
```

`docs/test-inventory.md` may retain an exact enumerated list because its purpose is inventory, but its heading and list must match `fuzz/Cargo.toml` at the implementation commit.

### 2.5 No release evidence system may return

Do not reintroduce:

- release subject `R`;
- evidence commit `E`;
- exact-SHA workflow dispatch;
- release artifact hashing;
- native-adapter release claims;
- scheduled provider verification;
- scheduled fuzz verification;
- CI publication;
- tag-triggered publication;
- a replacement release-verification ledger.

### 2.6 One successful remote CI run is enough

Closure requires one successful workflow run with this shape:

```text
workflow: CI
job: ci
job count: 1
result: success
```

Do not create a new document or artifact manifest to record it. A link in the implementation completion report or commit/PR notes is sufficient.

---

## 3. Non-Goals

This pass does **not** authorize:

- deleting behavioral tests;
- deleting property tests;
- deleting adversarial corpora;
- deleting keyless-core tests;
- weakening fetch or forge trust boundaries;
- weakening subprocess limits;
- weakening filesystem containment;
- changing MCP schemas or tool names;
- changing provider routing behavior;
- adding or removing providers;
- adding new API-key requirements;
- adding a CI cache action;
- adding sccache;
- adding code coverage thresholds;
- adding another platform matrix;
- adding a non-blocking macOS workflow;
- adding a manually dispatched fuzz workflow;
- adding a manually dispatched provider workflow;
- adding a release automation framework;
- changing dependency versions;
- changing production code under `src/**` unless an existing command cannot compile without a narrowly justified correction;
- broadly rewriting architecture documentation;
- changing the manual crates.io release policy.

A production-code change during this pass is a warning that scope is drifting.

---

## 4. Execution Rules

1. Work in gate order.
2. Keep implementation commits narrow.
3. Do not mix product-feature work into this pass.
4. Do not restore any deleted workflow.
5. Do not restore deleted Makefile aggregate targets merely to satisfy stale documentation.
6. Update the documentation to the simplified command surface instead.
7. Verify every command copied into documentation against the final Makefile.
8. Keep live tests ignored by default.
9. Provider-specific smoke commands must use test-name filters that select only the intended provider tests.
10. Do not print credential values.
11. Do not require all optional credentials for a single-provider diagnostic.
12. Run active-reference searches after all edits.
13. Run the local routine gate with credentials blank.
14. Run the local release gate from a clean tree.
15. Inspect the resulting GitHub Actions run after push.
16. Treat branch-protection cleanup as an explicit repository-settings action, not a code change.
17. Do not publish a real crate while implementing this plan.

---

# Gate A — Remove the Remaining Routine-Gate Duplication

## A.1 Required outcome

The routine gate must perform:

```text
format check
clippy with warnings denied
no-default feature compilation
one all-features deterministic test pass
```

It must not perform a second full test pass under no-default features.

## A.2 Modify the Makefile

Current shape:

```make
check: fmt clippy feature-check test

feature-check:
	cargo check --locked --no-default-features

test:
	cargo test --locked --all-features
	cargo test --locked --no-default-features
```

Target shape:

```make
check: fmt clippy feature-check test

ci: check

fmt:
	cargo fmt --check

clippy:
	cargo clippy --locked --all-targets --all-features -- -D warnings

feature-check:
	cargo check --locked --no-default-features

test:
	cargo test --locked --all-features
```

Do not replace the removed no-default full test pass with separate `mock` and `pdf` test passes.

## A.3 Verify no unique no-default-only behavioral dependency

Before deleting the command, inspect for tests conditionally compiled only when optional features are absent.

Suggested searches:

```bash
rg -n '#\[cfg\(not\(feature' src tests
rg -n 'cfg!\(not\(feature' src tests
rg -n 'no[_-]default' src tests
```

Classification:

- compile-only conditional code is covered by `cargo check --no-default-features`;
- ordinary tests that also compile under all features are covered by `cargo test --all-features`;
- if one meaningful test is genuinely exclusive to no-default mode, add a focused command for that test only and document why;
- do not restore the blanket no-default test suite without concrete evidence.

## A.4 Update command descriptions

Every active description of `make check` or `make ci` must match the final four-command sequence.

Expected wording:

```text
fmt + clippy + no-default compile check + all-features deterministic tests
```

Remove claims that routine CI runs no-default full tests.

Likely files:

```text
AGENTS.md
docs/architecture/testing.md
docs/test-inventory.md
skills/eggsearch-dev/SKILL.md
skills/eggsearch-release/SKILL.md
```

## A.5 Gate A acceptance criteria

- [ ] `make check` runs `cargo fmt --check`.
- [ ] `make check` runs locked all-target/all-feature clippy with `-D warnings`.
- [ ] `make check` runs `cargo check --locked --no-default-features`.
- [ ] `make check` runs `cargo test --locked --all-features` exactly once.
- [ ] `make check` does not run `cargo test --locked --no-default-features`.
- [ ] `make check` does not run separate `mock` or `pdf` full test suites.
- [ ] `make ci` remains an alias or equivalent wrapper around `make check`.
- [ ] Active documentation describes the final command sequence accurately.

---

# Gate B — Make Native Forge Smoke Tests Provider-Specific

## B.1 Required outcome

A maintainer with only GitHub credentials must be able to run the GitHub native adapter smoke tests without the command attempting GitLab, Codeberg, or Gitea tests.

The same must be true for every other provider.

## B.2 Replace the ambiguous Makefile target

Remove or rename:

```make
native-forge-smoke:
	cargo test --features live-smoke --test native_forge_smoke -- --ignored
```

Add provider-specific targets. Recommended implementation:

```make
.PHONY: native-forge-smoke-github native-forge-smoke-gitlab native-forge-smoke-codeberg native-forge-smoke-gitea native-forge-smoke-all

native-forge-smoke-github:
	cargo test --locked --features live-smoke --test native_forge_smoke -- --ignored native_github

native-forge-smoke-gitlab:
	cargo test --locked --features live-smoke --test native_forge_smoke -- --ignored native_gitlab

native-forge-smoke-codeberg:
	cargo test --locked --features live-smoke --test native_forge_smoke -- --ignored native_codeberg

native-forge-smoke-gitea:
	cargo test --locked --features live-smoke --test native_forge_smoke -- --ignored native_gitea

native-forge-smoke-all:
	cargo test --locked --features live-smoke --test native_forge_smoke -- --ignored
```

`native-forge-smoke-all` is optional. If retained, document all required environment values.

Use exact filters matching the current test names. Verify with:

```bash
cargo test --features live-smoke --test native_forge_smoke -- --list
```

If a provider has multiple tests, use a shared provider prefix so the filter selects all tests for that provider and no others.

## B.3 Credential requirements

Document provider requirements separately.

### GitHub

```text
GITHUB_TOKEN
GITHUB_SLASH_REF for the slash-reference test
```

### GitLab

```text
GITLAB_TOKEN
```

### Codeberg

```text
CODEBERG_TOKEN
```

### Gitea/Forgejo

```text
GITEA_TOKEN
GITEA_INSTANCE_URL
```

Do not imply that a GitHub-only run needs the other credentials.

## B.4 Keep direct behavior assertions

Do not weaken `tests/native_forge_smoke.rs`.

Retain assertions for:

- native mode;
- full commit SHA;
- non-empty repository entries;
- coherent requested and resolved references;
- non-zero request count;
- non-zero observed bytes;
- observed bytes not exceeding aggregate limits;
- pinned provenance where the provider response exposes it;
- provider-specific base URL behavior.

This gate changes invocation ergonomics only.

## B.5 Update active documentation

Update all active command examples, including at minimum:

```text
README.md if it contains a command
AGENTS.md
docs/architecture/testing.md
docs/release.md if native diagnostics are mentioned
skills/eggsearch-dev/SKILL.md
skills/eggsearch-release/SKILL.md
```

Preferred GitHub example:

```bash
GITHUB_TOKEN=... \
GITHUB_SLASH_REF=fixture/slash-ref \
make native-forge-smoke-github
```

Preferred GitLab example:

```bash
GITLAB_TOKEN=... \
make native-forge-smoke-gitlab
```

Do not show an unfiltered test-binary command with only one provider credential.

## B.6 Gate B acceptance criteria

- [ ] No ambiguous `native-forge-smoke` target runs all providers.
- [ ] GitHub target selects only `native_github*` tests.
- [ ] GitLab target selects only `native_gitlab*` tests.
- [ ] Codeberg target selects only `native_codeberg*` tests.
- [ ] Gitea target selects only `native_gitea*` tests.
- [ ] Provider-specific targets use `--locked`.
- [ ] Live tests remain `#[ignore]`.
- [ ] No provider-specific command requires unrelated credentials.
- [ ] Direct adapter correctness assertions remain unchanged.
- [ ] Active documentation uses provider-filtered commands.

---

# Gate C — Remove Stale Active Command References

## C.1 Required outcome

Active documentation must not reference deleted Makefile targets.

Known stale references currently include:

```text
make schema-corpus
make docs-tests
make hardening
```

in `AGENTS.md`.

These targets no longer exist and must not be restored.

## C.2 Correct `AGENTS.md`

In the specific-suite section, replace deleted Make aliases with direct Cargo commands or remove redundant examples.

Recommended compact form:

```bash
cargo test --locked --all-features --test schema_identity_registry --test fetch_safety --test security_applicability_corpus --test research_evidence_corpus --test recipes_next_actions --test evidence_bundle_handoff

cargo test --locked --all-features --test docs_config_snippets --test docs_provider_inventory --test docs_tool_names --test docs_safety_vocabulary --test docs_keyless_contract --test static_guards

cargo test --locked --all-features --test property_sanitize --test property_identity --test property_identity2 --test property_identity3 --test property_fetch_limits --test property_fetch_redirects --test property_fetch_url_edge --test property_fetch_response --test property_render_safety --test property_render_code --test property_render_metadata --test property_local_fs --test property_local_fs_extended
```

However, avoid turning `AGENTS.md` into another orchestration ledger. It is acceptable to retain only a few high-value focused examples and direct readers to `docs/architecture/testing.md` for the full inventory.

## C.3 Audit all active paths for deleted targets

Run:

```bash
rg -n 'make (schema-corpus|docs-tests|hardening|test-all|test-no-default|test-mock|test-pdf|publish-check|release-build|docs-check|native-forge-smoke)(\s|$)' \
  README.md AGENTS.md Makefile docs skills tests .github
```

Classify each match:

- current target and valid usage;
- focused release-only target and valid usage;
- deleted target requiring correction;
- historical text under `plans/`, which should not be changed.

Exclude `plans/` from active-path cleanup.

## C.4 Correct terminology

Use these terms consistently:

```text
make check          routine local gate
make ci             remote-equivalent routine gate
make release-check  local release/package gate
cargo publish       real manual publication
```

Do not call `make check` the “full release gate.”

Do not describe `cargo publish --dry-run` as ordinary development verification.

## C.5 Gate C acceptance criteria

- [ ] `AGENTS.md` contains no command for a nonexistent Makefile target.
- [ ] Active docs contain no command for a nonexistent Makefile target.
- [ ] Skills contain no command for a nonexistent Makefile target.
- [ ] Deleted targets are not restored.
- [ ] Routine and release terminology is consistent.
- [ ] Historical plans remain unchanged.

---

# Gate D — Correct Fuzz Inventory and Release-Skill Drift

## D.1 Required outcome

Active documentation must match the registered fuzz targets and final Makefile/release commands.

## D.2 Use `fuzz/Cargo.toml` as the source of truth

At the baseline commit, `fuzz/Cargo.toml` registers twenty-three binaries:

```text
validate_url
strip_control_chars
scan_injection_markers
extract_content
build_document_chunks
extract_pdf_text
validate_redirect_target
validate_content_type
extract_content_bytes
canonicalize_url
sanitize_pipeline
validate_redirect_chain
parse_content_length
chunk_boundary
mixed_utf8_extract
bounded_response_reader
workflow_kind_parse
classify_absence
detect_entity_scoped_conflicts
retrieval_failure_expansion
attempt_summary_generation
workflow_resolution
research_role_mapping
```

Verify the final list from the file rather than copying this list blindly if another commit has changed it.

## D.3 Update `docs/test-inventory.md`

The fuzz inventory section must list every registered `[[bin]]` target exactly once.

Update the heading count to the actual number at the implementation commit.

Do not include deleted or unregistered targets.

Add a maintenance note:

```text
Source of truth: fuzz/Cargo.toml [[bin]] entries.
```

Do not add a generator or CI check in this pass.

## D.4 Remove repeated brittle counts elsewhere

Update active prose in:

```text
AGENTS.md
skills/eggsearch-dev/SKILL.md
other active docs found by search
```

Replace:

```text
16 fuzz targets
```

with:

```text
Fuzz targets are registered in fuzz/Cargo.toml and implemented under fuzz/fuzz_targets/.
```

Exact counts may remain only in `docs/test-inventory.md`.

## D.5 Correct the release skill individual commands

`skills/eggsearch-release/SKILL.md` currently describes `make release-check` correctly but its manually expanded command list drifts from the Makefile.

The expanded sequence must match the final target:

```bash
cargo fmt --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo check --locked --no-default-features
cargo test --locked --all-features
RUSTDOCFLAGS='-D warnings' cargo doc --locked --all-features --no-deps
cargo build --locked --release
cargo publish --dry-run --locked
```

Do not include the removed no-default full test pass.

Use `--locked` consistently where the Makefile uses it.

The preferred instruction remains:

```bash
make release-check
```

The expanded sequence is secondary diagnostic documentation.

## D.6 Correct the development skill

In `skills/eggsearch-dev/SKILL.md`:

- rename “Full CI gate” to “Routine verification gate”;
- keep `make check` as the first command;
- move release-only commands out of the routine quick-command block or label them explicitly as release-only;
- remove stale exact fuzz-target counts;
- update any wording that still refers to “native release evidence”;
- state that native smoke tests are optional provider-specific diagnostics.

## D.7 Gate D acceptance criteria

- [ ] `docs/test-inventory.md` fuzz list matches `fuzz/Cargo.toml` exactly.
- [ ] The inventory heading count is correct.
- [ ] No other active file claims a brittle exact fuzz-target count.
- [ ] Release skill expanded commands match `make release-check`.
- [ ] Release skill uses `--locked` consistently.
- [ ] Development skill calls `make check` routine verification, not a release gate.
- [ ] Development skill contains no retired release-evidence language.

---

# Gate E — Final Active-Reference and Command Audit

## E.1 Required outcome

The simplified system must be internally coherent before remote validation.

## E.2 Search for retired release infrastructure

Run:

```bash
rg -n 'release-verification|R/E Protocol|evidence commit|release subject|native-smoke-release-manifest|Core Keyless Release Evidence|Optional Adapter Conformance Evidence|all four provider jobs|must mirror' \
  README.md AGENTS.md Makefile .github docs skills src tests Cargo.toml
```

Expected result:

- no active release-process dependency on retired infrastructure;
- product-level uses of “evidence” remain untouched;
- historical plans are excluded.

## E.3 Search for retired workflows and job names

Run:

```bash
rg -n 'fuzz-smoke|fuzz-campaign|Native Forge Smoke|native-forge-smoke.yml|check \(|test \(|schema-corpus|docs-contract|publish-check|release-build' \
  README.md AGENTS.md .github docs skills tests Makefile
```

Classify carefully:

- `publish-check` and `release-build` are valid Makefile release-only targets;
- old GitHub job names and deleted workflows are stale;
- `schema-corpus` or `docs-contract` may appear as conceptual test categories, but not as nonexistent Make targets or required CI jobs.

## E.4 Makefile dry-run validation

Run:

```bash
make -n check
make -n ci
make -n release-check
make -n native-forge-smoke-github
make -n native-forge-smoke-gitlab
make -n native-forge-smoke-codeberg
make -n native-forge-smoke-gitea
```

Required observations:

### `make check`

```text
cargo fmt --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo check --locked --no-default-features
cargo test --locked --all-features
```

No other Cargo test pass should appear.

### `make release-check`

Must add:

```text
RUSTDOCFLAGS=-D warnings cargo doc --locked --all-features --no-deps
cargo build --locked --release
cargo publish --dry-run --locked
```

### Provider targets

Each target must contain a filter for only its provider.

## E.5 YAML sanity

Inspect `.github/workflows/ci.yml` directly.

Required shape:

- one workflow file for routine CI;
- one job named `ci`;
- Ubuntu runner;
- no strategy matrix;
- no tag trigger;
- no schedule trigger;
- no workflow dispatch;
- no secrets dependency;
- no artifact upload/download;
- no benchmark command;
- no fuzz command;
- no live-provider command;
- no release build;
- no publish dry-run;
- no real publish;
- command is `make ci`.

Do not add a YAML parser contract test.

## E.6 Gate E acceptance criteria

- [ ] Active-path searches reveal no retired release infrastructure dependency.
- [ ] Active-path searches reveal no deleted workflow references.
- [ ] Active command examples resolve to real targets or real Cargo commands.
- [ ] `make -n check` shows exactly one all-features test pass.
- [ ] Provider smoke dry-runs are provider-filtered.
- [ ] CI YAML still contains exactly one job.
- [ ] No new workflow was added.

---

# Gate F — Local Verification

## F.1 Routine gate with credentials blank

Run:

```bash
env \
  GITHUB_TOKEN= \
  GH_TOKEN= \
  GITLAB_TOKEN= \
  CODEBERG_TOKEN= \
  GITEA_TOKEN= \
  FORGEJO_TOKEN= \
  SOURCEGRAPH_API_KEY= \
  BRAVE_API_KEY= \
  SEMANTIC_SCHOLAR_API_KEY= \
  NVD_API_KEY= \
  make check
```

Required result: pass.

Confirm from output:

- format check ran;
- clippy ran;
- no-default compile check ran;
- all-features tests ran once;
- no no-default full test pass ran;
- no network-dependent ignored test ran;
- no rustdoc build ran;
- no release build ran;
- no package dry-run ran;
- no benchmark ran;
- no fuzz target ran.

## F.2 Focused native test selection without network execution

Use test listing or ignored-test filtering to prove provider isolation without requiring credentials or network.

Examples:

```bash
cargo test --features live-smoke --test native_forge_smoke -- --list native_github
cargo test --features live-smoke --test native_forge_smoke -- --list native_gitlab
cargo test --features live-smoke --test native_forge_smoke -- --list native_codeberg
cargo test --features live-smoke --test native_forge_smoke -- --list native_gitea
```

Confirm each filter lists only its provider's tests.

Do not run live tests merely to complete this plan if credentials or stable fixtures are unavailable.

## F.3 Release gate

After committing all implementation changes and ensuring a clean tree, run:

```bash
make release-check
```

Required result: pass.

Confirm:

- routine gate passes;
- rustdoc passes with warnings denied;
- release build passes;
- `cargo publish --dry-run --locked` passes;
- no real package is published;
- no GitHub workflow is required for publication.

Do not use `--allow-dirty` as the normal solution.

## F.4 Gate F acceptance criteria

- [ ] Credential-scrubbed `make check` passes.
- [ ] Routine output contains only the intended four verification commands.
- [ ] Provider filters list only intended provider tests.
- [ ] Clean-tree `make release-check` passes.
- [ ] No real crates.io publication occurs.

---

# Gate G — Remote CI and Branch-Protection Closure

## G.1 Push the implementation

Push the completed implementation to `main` or through the repository's normal pull-request path.

Do not create a release tag.

Do not publish a crate.

## G.2 Inspect the GitHub Actions run

Required remote result:

```text
workflow: CI
job: ci
job count: 1
conclusion: success
```

Inspect the job steps and confirm:

- checkout;
- Rust 1.88 toolchain with rustfmt and clippy;
- one `make ci` verification step;
- no hidden matrix children;
- no scheduled workflow runs;
- no fuzz jobs;
- no packaging jobs;
- no provider jobs;
- no artifact jobs.

Record the run URL in the implementation completion report or commit/PR notes. Do not create a new release-verification file.

## G.3 Update branch protection

In repository settings, remove required checks associated with retired jobs, including any stale variants of:

```text
check (...)
test (...)
fmt
clippy
keyless-core
schema-corpus
docs-contract
benchmarks
release-build
publish-check
docs
fuzz-smoke (...)
native smoke summary
```

Configure the only required check as:

```text
CI / ci
```

If the implementing agent cannot edit repository settings, report this as the sole external action with exact instructions. Do not claim closure until a maintainer confirms the setting or explicitly accepts the external follow-up.

## G.4 Gate G acceptance criteria

- [ ] A visible `CI / ci` run succeeds.
- [ ] The run contains exactly one job.
- [ ] No retired workflow executes.
- [ ] No retired required-check names remain in branch protection, or the exact external follow-up is recorded.
- [ ] The run URL is included in completion notes, not committed as a mutable ledger.

---

## 5. Expected File-Level Changes

### Required modifications

```text
Makefile
AGENTS.md
docs/architecture/testing.md
docs/test-inventory.md
skills/eggsearch-dev/SKILL.md
skills/eggsearch-release/SKILL.md
```

### Likely modifications after active-reference search

```text
README.md
docs/release.md
docs/release-checklist.md
docs/architecture/overview.md
```

Modify these only when a concrete stale command or description exists.

### Files that should not change

```text
.github/workflows/ci.yml
```

The workflow already has the desired one-job structure. Change it only if validation finds an actual syntax or credential-scrubbing defect.

```text
src/**
Cargo.toml
Cargo.lock
benches/**
fuzz/fuzz_targets/**
tests/native_forge_smoke.rs
```

`tests/native_forge_smoke.rs` should not require behavior changes; provider filtering belongs in invocation targets. A test rename is permitted only if needed to establish clean provider prefixes, but avoid it if current prefixes already work.

### No required deletions

This closure pass should normally delete no additional source or test files.

---

## 6. Suggested Commit Sequence

### Commit 1 — Routine gate and native smoke commands

```text
ci: close remaining verification command duplication
```

Contents:

- remove the no-default full test pass from `make check`;
- add provider-specific native forge smoke targets;
- optionally add an explicitly named all-provider target;
- keep release-check behavior unchanged.

Verify:

```bash
make -n check
make -n release-check
make -n native-forge-smoke-github
make -n native-forge-smoke-gitlab
make -n native-forge-smoke-codeberg
make -n native-forge-smoke-gitea
```

### Commit 2 — Active documentation closure

```text
docs: align verification and provider diagnostic commands
```

Contents:

- remove deleted Make target references;
- update routine command descriptions;
- update provider-specific smoke examples;
- correct release skill command expansion;
- correct development skill terminology;
- reconcile fuzz inventory with `fuzz/Cargo.toml`;
- remove brittle fuzz counts outside the inventory.

Verify:

```bash
rg -n 'make (schema-corpus|docs-tests|hardening)' README.md AGENTS.md docs skills
rg -n '16 fuzz targets' README.md AGENTS.md docs skills
```

Expected result: no active matches.

### Commit 3 — Validation-only corrections, if required

```text
chore: close CI simplification validation drift
```

Use only for defects found by:

- `make check`;
- `make release-check`;
- active-reference searches;
- provider-filter listing;
- the one-job GitHub Actions run.

Do not introduce unrelated cleanup.

---

## 7. Completion Evidence

The implementation completion report should include:

```text
implementation commit(s)
final Makefile routine command list
credential-scrubbed make check result
clean-tree make release-check result
provider-specific test-filter listing result
successful CI / ci run URL
branch-protection status
```

Do not commit generated logs, run manifests, hashes, or a release-evidence document.

A concise completion report is sufficient, for example:

```text
Closure complete.

- make check: pass with credentials blank
- make release-check: pass from clean tree
- routine CI: one job, CI / ci, pass
- native smoke commands: provider-specific filters verified
- active docs: no deleted Make targets
- fuzz inventory: matches fuzz/Cargo.toml
- branch protection: only CI / ci required
- no crate published
```

---

## 8. Final Closure Criteria

This line of work is complete only when all of the following are true.

### Routine verification

- [ ] `make check` runs four logical checks: fmt, clippy, no-default compile, all-features tests.
- [ ] The all-features test suite runs once.
- [ ] No no-default full test suite runs in the routine gate.
- [ ] No packaging, docs, benchmark, fuzz, or live test runs in the routine gate.
- [ ] Credential-scrubbed routine verification passes.

### Release verification

- [ ] `make release-check` includes routine checks, rustdoc, release build, and publish dry-run.
- [ ] Clean-tree release verification passes.
- [ ] No target performs a real publish.
- [ ] Release cadence remains manual.

### Native adapter diagnostics

- [ ] Native adapter commands are provider-specific.
- [ ] A single-provider command does not attempt unrelated providers.
- [ ] Provider-specific test filters are verified.
- [ ] Native tests remain ignored and manual.
- [ ] No native workflow or evidence artifact system exists.

### Documentation

- [ ] No active file references deleted Makefile targets.
- [ ] Routine and release command descriptions match the Makefile.
- [ ] Release skill expanded commands match `make release-check`.
- [ ] Fuzz inventory matches `fuzz/Cargo.toml`.
- [ ] Brittle exact fuzz counts are removed outside the inventory.
- [ ] No active release-evidence terminology remains.

### Remote repository state

- [ ] One visible `CI / ci` run passes.
- [ ] The run contains exactly one job.
- [ ] No retired workflow executes.
- [ ] Branch protection requires only `CI / ci`, or the exact external repository-settings action is explicitly acknowledged.
- [ ] No crate was published while implementing this closure.

When every criterion above is satisfied, the CI, verification, and manual-release simplification line of work may be closed.