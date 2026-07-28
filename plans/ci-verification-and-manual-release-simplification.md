# CI, Verification, and Manual Release Simplification

**Repository:** `eggstack/eggsearch`  
**Baseline:** `62f78c67fc8fe45fbae8bc8600e6ba15e30cc992`  
**Status:** Small-model implementation handoff  
**Scope:** Reductive CI, verification, and release-infrastructure correction  
**Primary objective:** Restore fast iteration without discarding substantive correctness tests  
**Release authority:** A maintainer publishing manually to crates.io  
**GitHub Actions release role:** None

---

## 1. Objective

The repository currently applies a release-engineering and verification model that is disproportionate to the size and deployment model of eggsearch. Ordinary pushes and pull requests fan out into a large matrix of repeated checks, fuzz jobs, packaging checks, documentation checks, platform combinations, and release-oriented evidence work. The same test binaries are often executed more than once under differently named jobs or Makefile targets. Optional credentialed provider checks have also become entangled with release identity and evidence bookkeeping even though the supported core product is explicitly keyless.

This plan reduces the apparatus to a small, legible model:

```text
ordinary development
    -> one local command: make check

push / pull request
    -> one required GitHub Actions job: make ci

manual release preparation
    -> one local maintainer command: make release-check

publication
    -> maintainer runs cargo publish --locked

optional live/provider diagnostics
    -> ignored tests run manually when relevant
```

The goal is not to reduce correctness. The goal is to stop repeatedly proving the same correctness through many independently maintained layers.

The desired final state is:

```text
one required CI job
one routine local verification target
one explicit release-only local target
no GitHub publishing workflow
no CI-controlled release cadence
no R/E evidence-commit protocol
no scheduled credentialed-provider workflow
no per-target fuzz fan-out on every commit
no packaging checks during ordinary iteration
substantive tests retained and executed once
```

---

## 2. Authoritative Decisions

The following decisions are fixed for this implementation pass.

### 2.1 GitHub Actions is a merge-safety check, not a release system

GitHub Actions must only answer whether the committed code passes the routine project gate. It must not:

- publish to crates.io;
- create a GitHub release;
- determine release cadence;
- wait on tags before publication;
- construct release evidence manifests;
- upload release-verification artifacts;
- require a release subject SHA input;
- require an evidence-only commit;
- require third-party provider credentials;
- run live provider conformance as a release gate.

### 2.2 crates.io publication is manual

A maintainer decides when to release, prepares the version and changelog, runs the local release gate, and invokes:

```bash
cargo publish --locked
```

No GitHub workflow may contain a real publish command or crates.io token.

### 2.3 Optional provider adapters remain optional

GitHub, GitLab, Codeberg, Gitea/Forgejo, Sourcegraph, Brave API, Semantic Scholar, and any other credentialed adapter must remain optional enhancements. Their absence or temporary drift must not block core CI or a core release.

Ignored native-provider smoke tests may remain available for maintainers, but they are diagnostics rather than release evidence.

### 2.4 Preserve substantive tests; remove duplicated orchestration

Do not use this pass as authorization to broadly delete behavioral, safety, property, corpus, schema, fetch-boundary, subprocess, filesystem, retrieval-accounting, or keyless-core tests.

The primary reduction comes from:

- executing the test inventory once rather than repeatedly;
- removing CI jobs that only provide separately named green boxes;
- removing workflow-shape and mutable-release-document tests;
- moving packaging and documentation builds out of the ordinary development loop;
- moving fuzzing and live network checks to explicit maintainer commands.

### 2.5 The current release evidence protocol is retired

The `R` release-subject and `E` evidence-commit protocol is not part of the target design. The source commit, version in `Cargo.toml`, crates.io record, changelog entry, and git tag are sufficient provenance for this project.

The release-chain requirements in `plans/final-keyless-proof-and-release-chain-corrective-closure.md` are superseded by this plan wherever they conflict with the decisions above. Behavioral keyless-core and retrieval-correctness work in that plan remains independently valid; only its release-evidence, exact-subject, evidence-commit, workflow-manifest, and release-blocking benchmark requirements are superseded.

---

## 3. Current-State Findings to Correct

The implementer must understand the duplication being removed.

### 3.1 Current pull-request/push fan-out

`.github/workflows/ci.yml` currently expands into forty jobs:

```text
cargo check feature matrix       4 jobs
cargo test OS/feature matrix     8 jobs
keyless-core                     1 job
clippy                           1 job
schema-corpus                    1 job
docs-contract                    1 job
benchmark compilation            1 job
fmt                              1 job
release build                    1 job
publish dry-run                  1 job
rustdoc                          1 job
fuzz-smoke targets              19 jobs
                                -------
total                           40 jobs
```

This is excessive for routine changes and produces long feedback loops, repeated toolchain setup, repeated dependency compilation, and noisy failure surfaces.

### 3.2 Feature coverage is more duplicated than it appears

The workflow separately checks and tests:

```text
--all-features
--no-default-features
--features mock
--features pdf
```

`--all-features` already includes `mock` and `pdf`. Separate `mock` and `pdf` jobs therefore primarily prove feature isolation, not distinct product behavior. Feature-isolation compilation can be covered with a small number of sequential commands inside one job rather than separate full jobs.

### 3.3 Named suites rerun tests already discovered by Cargo

The current Makefile runs:

```text
test-all
hardening
schema-corpus
docs-tests
```

after `test-all` has already run integration tests discoverable under `tests/`. The CI workflow similarly gives selected integration binaries standalone jobs even though `cargo test --all-features` already executes them.

The resulting named-job visibility is not worth the repeated execution and maintenance burden.

### 3.4 Development verification includes release packaging

`make check` currently includes:

- benchmark compilation;
- release compilation;
- rustdoc with denied warnings;
- `cargo publish --dry-run --locked`.

A publish dry-run is sensitive to package state and clean-tree conditions and is not an appropriate routine editing-loop check. Release compilation and documentation publication checks also do not need to run after every small change.

### 3.5 Release documentation contradicts workflow triggers

The release documentation expects tag-related GitHub jobs, while the current CI workflow only triggers for `main` pushes and pull requests. The repository also declares that the Makefile, workflow, and documentation must mirror one another exactly, creating three independently maintained sources of truth that have already drifted.

### 3.6 Optional adapter verification has become release infrastructure

`.github/workflows/native-forge-smoke.yml` contains:

- scheduled execution;
- four credentialed provider jobs;
- exact release-subject validation;
- JSON evidence production and validation;
- artifact retention;
- summary-job result indirection;
- artifact download and combination;
- SHA-256 manifest construction.

`tests/native_forge_smoke.rs` is correspondingly coupled to release-subject and evidence-directory environment variables, and `tests/native_forge_workflow_contract.rs` implements a custom workflow parser to enforce that infrastructure.

This entire chain is unnecessary for optional maintainer diagnostics.

---

## 4. Non-Goals

This plan does **not** authorize:

- deleting broad behavioral test coverage;
- weakening fetch URL validation or redirect validation;
- weakening bounded-read or bounded-subprocess guarantees;
- removing property tests merely because they are property tests;
- removing adversarial corpora;
- removing keyless-core behavioral tests;
- removing feature flags;
- changing MCP tool names or schemas;
- changing provider routing behavior;
- adding API-key requirements;
- removing optional provider implementations;
- adding a replacement release service;
- adding release-plz, cargo-release, semantic-release, Changesets, or another release orchestrator;
- adding a GitHub Personal Access Token or crates.io token to Actions;
- introducing a new shell-script framework;
- introducing a task runner solely to replace the Makefile;
- introducing third-party caching actions as part of this pass;
- adding code coverage thresholds;
- adding minimum benchmark thresholds;
- adding required nightly Rust jobs;
- adding scheduled CI under another name;
- redesigning the test architecture beyond what is needed to remove orchestration duplication.

---

## 5. Execution Rules

1. Work in gate order.
2. Keep each implementation commit narrowly scoped.
3. Do not mix production-feature changes into this pass.
4. Do not add a new workflow to compensate for deleting an old workflow.
5. Do not preserve obsolete release machinery behind a renamed target.
6. Do not leave active documentation pointing to retired release evidence.
7. Do not rewrite historical plan files; mark this plan as superseding conflicting release instructions.
8. Do not delete an integration test merely because a standalone CI job for it is removed.
9. Verify which tests are included by `cargo test --all-features` before deleting named Makefile invocations.
10. Any test requiring live network access or secrets must remain ignored by default.
11. CI must pass with all recognized credential environment variables blank.
12. CI must not require a clean working tree because GitHub checkout is already clean and routine local development may not be.
13. `make check` must be safe and useful in a dirty development tree.
14. `make release-check` may require a clean tree and package-ready metadata.
15. The final documentation must distinguish routine verification, optional diagnostics, and manual publication.

---

# Gate A — Collapse Required CI to One Job

## A.1 Required outcome

Replace the forty-job workflow with one required Ubuntu job that invokes one repository-owned command.

Target shape:

```yaml
name: CI

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

permissions:
  contents: read

concurrency:
  group: ci-${{ github.workflow }}-${{ github.ref }}
  cancel-in-progress: true

env:
  CARGO_TERM_COLOR: always

jobs:
  ci:
    name: ci
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          toolchain: "1.88"
          components: rustfmt, clippy
      - name: Run routine verification
        run: make ci
        env:
          GITHUB_TOKEN: ""
          GH_TOKEN: ""
          GITLAB_TOKEN: ""
          CODEBERG_TOKEN: ""
          GITEA_TOKEN: ""
          FORGEJO_TOKEN: ""
          SOURCEGRAPH_API_KEY: ""
          BRAVE_API_KEY: ""
          SEMANTIC_SCHOLAR_API_KEY: ""
          NVD_API_KEY: ""
```

Exact YAML formatting may differ, but the semantics must remain this small.

## A.2 Remove the feature and OS matrices

Delete the separate `check` and `test` matrices.

Do not retain matrix syntax with one entry. A one-job workflow should be structurally one job, not a dormant matrix that invites re-expansion.

Routine cross-feature coverage will be handled sequentially by `make ci`.

## A.3 Remove standalone duplicate suite jobs

Delete standalone jobs for:

```text
keyless-core
schema-corpus
docs-contract
benchmarks
release-build
publish-check
docs
```

Rationale:

- keyless-core tests remain part of the ordinary all-features test inventory and run with credentials blank;
- schema/corpus and docs-contract integration binaries remain in the test inventory;
- benchmark compilation is optional/release-adjacent;
- release compilation, rustdoc, and publish dry-run belong to `make release-check`.

Before removing a standalone test job, confirm that its test binary is discoverable by the retained Cargo invocation. If a test is excluded through unusual Cargo configuration, update `make ci` to name it once rather than keeping a separate GitHub job.

## A.4 Remove all per-target fuzz jobs

Delete the entire `fuzz-smoke` job matrix.

Do not replace it with another required fuzz job. Fuzzing becomes an explicit local/manual diagnostic described in Gate E.

The fuzz target source files remain in the repository.

## A.5 Do not add caching complexity

Do not add `Swatinem/rust-cache`, custom cache keys, cache restore/save steps, sccache, or artifact reuse in this pass. Reducing forty jobs to one should provide the dominant improvement. Caching can be reconsidered only after measured evidence shows the one-job workflow is still unacceptably slow.

## A.6 Credential-free CI environment

The one verification step must explicitly blank recognized credential variables. Include at minimum:

```text
GITHUB_TOKEN
GH_TOKEN
GITLAB_TOKEN
CODEBERG_TOKEN
GITEA_TOKEN
FORGEJO_TOKEN
SOURCEGRAPH_API_KEY
BRAVE_API_KEY
SEMANTIC_SCHOLAR_API_KEY
NVD_API_KEY
```

Do not print inherited values. Do not require repository secrets.

`actions/checkout` may continue using its implicit GitHub token internally; the test command must receive blank credential variables.

## A.7 Branch-protection handoff

Repository code cannot fully update organization/repository branch-protection settings. Add a short maintainer note to the plan completion summary or release documentation:

```text
After the workflow lands, update branch protection so the only required check is CI / ci. Remove stale required names from the old matrices and standalone jobs.
```

Do not block the code change on UI access if the implementing agent cannot modify branch protection. Report it as the only external follow-up.

## A.8 Gate A acceptance criteria

- [ ] `.github/workflows/ci.yml` defines exactly one job.
- [ ] The job runs on `ubuntu-latest`.
- [ ] The job invokes `make ci`.
- [ ] No strategy matrix remains.
- [ ] No macOS required job remains.
- [ ] No benchmark, release-build, publish, rustdoc, fuzz, schema-only, docs-only, or keyless-only job remains.
- [ ] No repository secret is required.
- [ ] Recognized credential variables are blank during tests.
- [ ] Stale runs are cancelled through workflow concurrency.
- [ ] The workflow contains no tag trigger and no release trigger.
- [ ] The workflow contains no artifact upload or download.
- [ ] The workflow contains no crates.io token or `cargo publish` command.

---

# Gate B — Replace the Makefile with Clear Verification Tiers

## B.1 Required outcome

The Makefile must expose three clearly different classes of work:

```text
routine verification      make check / make ci
release-only verification make release-check
optional diagnostics      make bench-check / make fuzz-smoke / make live-smoke / make native-forge-smoke
```

## B.2 Routine local command

Define `make check` as the normal developer command. It must avoid packaging and clean-tree requirements.

Recommended target:

```make
.PHONY: check ci release-check fmt clippy test feature-check docs-check release-build publish-check bench-check fuzz-smoke live-smoke native-forge-smoke

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

This is the preferred minimal gate.

If inspection proves that meaningful tests exist only under a no-feature build and are not exercised under `--all-features`, use:

```make
test:
	cargo test --locked --no-default-features
	cargo test --locked --all-features
```

Do not retain separate full runs for `--features mock` and `--features pdf`; `--all-features` covers those features. Use a compile-only isolation check only if the feature cannot otherwise be proven to compile independently and a real failure is demonstrated.

## B.3 Do not rerun selected suites after the aggregate test

Remove aggregate dependencies such as:

```make
check: ... test-all ... hardening schema-corpus docs-tests ...
```

when `test-all` already runs those integration tests.

Individual convenience targets may remain for focused developer use, but they must not be dependencies of `check` or `ci` after the same tests have already run.

For example, this is acceptable:

```make
property-tests:
	cargo test --locked --all-features --test property_sanitize --test property_identity
```

provided `check` does not call both `cargo test --all-features` and `property-tests`.

Prefer deleting low-value focused aliases unless they are referenced in contributor documentation.

## B.4 Release-only target

Define `make release-check` as the explicit maintainer packaging gate:

```make
release-check: check docs-check release-build publish-check

docs-check:
	RUSTDOCFLAGS="-D warnings" cargo doc --locked --all-features --no-deps

release-build:
	cargo build --locked --release

publish-check:
	cargo publish --dry-run --locked
```

`release-check` may fail on a dirty tree. This is intentional and must be documented.

Do not make `check` depend on `release-check`, `publish-check`, `release-build`, `docs-check`, or benchmark compilation.

## B.5 Optional benchmark target

Retain a manual compile target:

```make
bench-check:
	cargo bench --locked --all-features --bench perf --no-run
```

A runtime benchmark command may also be documented:

```bash
cargo bench --locked --all-features --bench perf
```

Neither command is a merge gate or mandatory release gate. Maintainers should run relevant benchmarks when a change plausibly affects a hot path.

## B.6 Optional fuzz target

Add one local fuzz convenience target only if it remains simple:

```make
fuzz-smoke:
	cd fuzz && cargo fuzz run validate_url -- -max_total_time=60
	cd fuzz && cargo fuzz run sanitize_pipeline -- -max_total_time=60
	cd fuzz && cargo fuzz run bounded_response_reader -- -max_total_time=60
```

Before writing the target, verify the repository's actual fuzz working-directory convention. Do not blindly use `cd fuzz` if current commands run from the root.

It is acceptable to omit a Makefile fuzz target and document direct commands instead. Do not add a loop script, target-discovery script, or fuzz orchestration framework.

## B.7 Live diagnostic targets

Keep live tests ignored by default:

```make
live-smoke:
	cargo test --features live-smoke --test corpus_runner -- --ignored

native-forge-smoke:
	cargo test --features live-smoke --test native_forge_smoke -- --ignored
```

These targets may fail because of provider drift, rate limits, credentials, region, or network conditions. They are diagnostics and must not be dependencies of any other target.

## B.8 Gate B acceptance criteria

- [ ] `make check` performs only routine deterministic verification.
- [ ] `make ci` is an alias or equivalent wrapper around `make check`.
- [ ] `make check` does not invoke `cargo publish --dry-run`.
- [ ] `make check` does not build benchmarks.
- [ ] `make check` does not build rustdoc.
- [ ] `make check` does not perform a release build.
- [ ] `make check` does not rerun named integration suites already covered by its aggregate test command.
- [ ] `make release-check` includes routine checks, rustdoc, release build, and publish dry-run.
- [ ] Optional live and native tests remain ignored and standalone.
- [ ] No Makefile target publishes a real crate.
- [ ] A dirty development tree can run `make check` successfully when the code itself passes.

---

# Gate C — Retire Release-Evidence Machinery and Document Manual Publication

## C.1 Required outcome

Replace the existing release-chain/evidence model with a short manual crates.io process.

`docs/release.md` becomes the single active release-process document. It must not claim that the Makefile, CI workflow, and documentation mirror one another exactly. It must instead distinguish their roles:

```text
Makefile / make check         routine deterministic local gate
GitHub Actions / make ci      remote repetition of routine gate
Makefile / make release-check local packaging gate
cargo publish --locked        explicit maintainer publication
```

## C.2 Delete the mutable verification record

Delete:

```text
docs/release-verification.md
```

Do not rename it to `release-evidence.md`, archive it under active docs, or replace it with another per-release ledger.

Historical plan files may continue to mention it as historical context. Active source, tests, workflows, README text, and active documentation must not depend on it.

Search active paths, excluding `plans/`, for:

```bash
rg -n "release-verification|release subject|evidence commit|R/E Protocol|native-smoke-release-manifest|Core Keyless Release Evidence|Optional Adapter Conformance Evidence" \
  README.md Makefile .github docs src tests Cargo.toml
```

Classify and remove obsolete references. Do not mechanically edit unrelated uses of the word `evidence` in the product's research/evidence-bundle functionality.

## C.3 Rewrite `docs/release.md`

The document should be concise and operational. Required content:

### Policy

```text
Release cadence is manual and maintainer-controlled.
GitHub Actions does not publish eggsearch.
The crate is published directly to crates.io with cargo publish.
Optional provider smoke tests do not block a core release.
```

### Preparation

1. Ensure intended changes are on `main`.
2. Choose the next SemVer version.
3. Update `Cargo.toml`.
4. Update `CHANGELOG.md`.
5. Commit the release preparation.
6. Ensure the working tree is clean.

### Verification

```bash
make release-check
```

The document must explain that this includes routine checks, release compilation, rustdoc, and `cargo publish --dry-run --locked`.

### Publication

```bash
cargo publish --locked
```

### Post-publication

```bash
git tag vX.Y.Z
git push origin vX.Y.Z
```

Creating a GitHub release from the changelog is optional and manual.

### Immutable-version warning

The document must explicitly state:

```text
Once crates.io accepts a version, that version cannot be overwritten. Any correction requires a new version bump and another changelog entry.
```

Do not imply that rerunning CI can repair an already published version.

## C.4 Publication order

Use this preferred order:

```text
release preparation commit
clean local release-check
cargo publish
create and push matching tag
optional manual GitHub release
```

This avoids pushing a release tag for a publication that crates.io rejects. The tag must point to the exact commit whose version was published.

If project convention strongly prefers tagging before publication, the implementer may preserve that order only if the documentation clearly explains how a failed publication is handled. The preferred order above is simpler.

## C.5 Remove CI-green release identity requirements

The release process may recommend checking the latest CI result, but it must not require:

- an exact GitHub run ID;
- a terminal evidence commit;
- artifact hashes;
- benchmark artifact identity;
- exact-SHA workflow dispatch;
- separate Linux and macOS evidence records;
- adapter conformance claims;
- a no-code-after-evidence rule.

`make release-check` on the clean release commit is the authoritative maintainer check.

## C.6 Update README development text

Replace language claiming that `make check` runs the full release gate.

Target meaning:

```text
make check runs formatting, clippy, feature compilation, and the deterministic test suite.
make release-check adds documentation, release-build, and package dry-run checks for maintainers preparing a crates.io release.
```

Keep the prominent keyless-installation statement.

## C.7 Gate C acceptance criteria

- [ ] `docs/release-verification.md` is deleted.
- [ ] `docs/release.md` describes manual crates.io publication.
- [ ] The document explicitly says GitHub Actions does not publish or determine release cadence.
- [ ] The document uses `make release-check` as the local release gate.
- [ ] The document uses `cargo publish --locked` as the publication command.
- [ ] The document warns that accepted crates.io versions are immutable.
- [ ] Tagging and GitHub release creation are manual post-publication steps.
- [ ] No active documentation requires R/E commits, evidence hashes, run IDs, or release artifacts.
- [ ] README distinguishes `make check` from `make release-check`.
- [ ] Optional adapter status does not block the documented release.

---

# Gate D — Demote Native Forge Validation to a Simple Maintainer Diagnostic

## D.1 Required outcome

Delete the scheduled and manually dispatched native-forge evidence workflow:

```text
.github/workflows/native-forge-smoke.yml
```

Do not replace it with another GitHub workflow in this pass.

The ignored Rust integration test remains the supported maintainer diagnostic.

## D.2 Simplify `tests/native_forge_smoke.rs`

The current test file describes itself as release-blocking evidence and requires:

```text
EGGSEARCH_RELEASE_SUBJECT
EGGSEARCH_NATIVE_SMOKE_EVIDENCE_DIR
```

Remove release-evidence responsibilities from the test.

Delete test-only machinery whose sole purpose is producing CI evidence:

- `release_subject()`;
- `evidence_dir()`;
- `write_evidence()`;
- `write_repo_map_evidence()` if it only serializes evidence;
- JSON artifact file writes;
- temporary evidence-file rename logic;
- timestamp fields used only in evidence JSON;
- assertions that exist only to satisfy the evidence schema.

Preserve direct behavioral assertions that prove the adapter actually used native mode and returned valid bounded/provenance-aware results, including where applicable:

- mode is `native`;
- commit SHA is present and full-length;
- entries are non-empty;
- resolved reference/commit information is coherent;
- request count is non-zero;
- observed bytes do not exceed aggregate bounds;
- provenance is pinned;
- provider-specific configuration is honored.

Update the module documentation to show a direct maintainer command without release-subject or evidence-directory variables.

Example:

```bash
GITHUB_TOKEN=... \
GITHUB_SLASH_REF=fixture/slash-ref \
cargo test --features live-smoke --test native_forge_smoke -- --ignored native_github
```

Do not print token values.

## D.3 Remove workflow-shape contract tests

Delete:

```text
tests/native_forge_workflow_contract.rs
```

The workflow it parses no longer exists. Do not replace the custom YAML parser with a parser for the simplified CI workflow.

CI structure should be reviewed directly and kept understandable, not enforced through hundreds of lines of parser assertions.

## D.4 Remove release-document contract tests

Delete:

```text
tests/release_document_contract.rs
```

This test validates mutable release-record wording and release-subject manifests. That contract is retired.

## D.5 Narrow `tests/docs_keyless_contract.rs`

Remove assertions whose only purpose is preserving the retired release verification model, including assertions for headings such as:

```text
Core Keyless Release Evidence
Optional Adapter Conformance Evidence
unverified adapter status in the release ledger
```

Keep only stable user-facing keyless invariants that materially protect the product contract, such as:

- README states that the default installation requires no API keys;
- the default install section does not require credential setup;
- keyless paths are documented before optional enhanced paths;
- codegg integration guidance does not require or prompt for optional keys.

Avoid increasing the file's substring-based scope. The objective is reduction.

## D.6 Remove stale test selectors

After deleting the workflow/release contract test binaries, remove their names from:

- Makefile focused targets;
- any CI command;
- documentation command examples;
- test-count claims;
- release verification references.

Do not update historical numeric test counts in old plans.

## D.7 Gate D acceptance criteria

- [ ] `.github/workflows/native-forge-smoke.yml` is deleted.
- [ ] No scheduled workflow remains for provider smoke tests.
- [ ] `tests/native_forge_smoke.rs` remains ignored and directly runnable.
- [ ] Native smoke tests no longer require a release-subject environment variable.
- [ ] Native smoke tests no longer require an evidence-directory environment variable.
- [ ] Native smoke tests no longer write release evidence artifacts.
- [ ] Direct adapter behavior and bounds assertions remain.
- [ ] `tests/native_forge_workflow_contract.rs` is deleted.
- [ ] `tests/release_document_contract.rs` is deleted.
- [ ] Release-ledger assertions are removed from `tests/docs_keyless_contract.rs`.
- [ ] No active command references deleted test binaries.

---

# Gate E — Establish Explicit Optional Verification Policy

## E.1 Required outcome

Document when non-routine checks should be run without turning them into permanent merge gates.

Add a compact section to `docs/release.md` or an existing contributor/development document. Do not create a large verification manual.

## E.2 Property and adversarial tests

Property, hardening, and adversarial integration tests that are already part of `cargo test --all-features` continue running in routine CI once.

Do not create separate jobs or duplicate Makefile dependencies for them.

## E.3 Fuzzing policy

Fuzz targets remain available. Run relevant targets manually when changing their associated trust boundary.

Recommended examples:

```bash
cargo fuzz run validate_url -- -max_total_time=60
cargo fuzz run sanitize_pipeline -- -max_total_time=60
cargo fuzz run bounded_response_reader -- -max_total_time=60
```

Use the repository's actual cargo-fuzz invocation path.

A maintainer may run a longer or broader fuzz sweep before a significant release, but no release claim or evidence artifact is required.

## E.4 Benchmark policy

Compile or run benchmarks when a change plausibly affects:

- search hot paths;
- bounded response reading;
- local inventory/cache behavior;
- evidence postprocessing;
- repository map construction;
- other measured paths in `benches/perf.rs`.

Commands:

```bash
make bench-check
cargo bench --locked --all-features --bench perf
```

Benchmarks are advisory. Do not fail CI or block publication because a noisy shared runner produces different measurements.

## E.5 Live web/provider policy

Run live tests only when:

- modifying a provider adapter;
- investigating reported provider drift;
- validating a provider-specific bug fix;
- preparing a release that materially changes remote provider behavior.

Failure classification:

```text
local deterministic regression -> fix before merge/release
provider drift/rate limit/region/network issue -> diagnose separately; does not block unrelated core work
missing optional credential -> test not run; does not block core work
```

## E.6 Platform policy

Routine CI uses Linux only.

For platform-specific changes:

- Linux-specific code must be tested on Linux;
- macOS-specific code should be tested locally on macOS when modified;
- broad cross-platform matrix testing is not required for every change;
- a maintainer may run `make check` on macOS before a release without recording evidence or run IDs.

Do not add a non-blocking macOS workflow in this pass. Non-blocking workflows still consume maintenance attention and create noisy red statuses.

## E.7 Gate E acceptance criteria

- [ ] Optional verification policy is documented in one compact location.
- [ ] Fuzzing is manual and source targets remain available.
- [ ] Benchmarks are manual/advisory.
- [ ] Live provider tests are manual and ignored.
- [ ] Platform-specific testing is risk-based rather than an always-on matrix.
- [ ] No new workflow is introduced for optional checks.

---

# Gate F — Remove Stale Claims and Validate the Simplified System

## F.1 Active-reference audit

Run searches over active project paths, excluding historical plans where appropriate:

```bash
rg -n "40 jobs|fuzz-smoke|publish-check|release-build|native forge smoke|Native Forge Smoke|release-verification|R/E Protocol|evidence commit|release subject|all four provider jobs|must mirror" \
  README.md Makefile .github docs src tests Cargo.toml
```

Every match must be classified as:

```text
still accurate and retained
updated for simplified model
removed as obsolete
```

Do not remove product-level uses of `evidence` related to search results or evidence bundles.

## F.2 YAML and Makefile sanity

Verify:

```bash
make -n check
make -n ci
make -n release-check
```

If a YAML linter is already available in the repository, use it. Do not add a new dependency solely for this plan. GitHub Actions parsing on push is acceptable final validation for the workflow syntax.

## F.3 Routine gate

Run:

```bash
make check
```

Required result:

- formatting passes;
- clippy passes with warnings denied;
- minimal/no-default compilation passes;
- all-features deterministic tests pass;
- no credentials are required;
- no live network test runs;
- no package dry-run runs;
- no benchmark runs;
- no rustdoc build runs;
- no release build runs.

## F.4 Credential-scrubbed routine gate

Run with credential variables absent or blank:

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
  make ci
```

Required result: pass.

## F.5 Release gate

From a clean tree, run:

```bash
make release-check
```

Required result:

- routine gate passes;
- release build passes;
- rustdoc with warnings denied passes;
- `cargo publish --dry-run --locked` passes;
- no real publication occurs;
- no GitHub workflow or evidence manifest is required.

If the implementation commit itself means the working tree is not clean, commit the changes before the final release-check. Do not use `--allow-dirty` as the documented solution.

## F.6 GitHub Actions validation

After push, inspect the resulting workflow run.

Required shape:

```text
workflow: CI
jobs: 1
required job: ci
```

The run should not show separate matrix children, fuzz targets, packaging jobs, or provider jobs.

## F.7 Gate F acceptance criteria

- [ ] Active documentation and commands match the simplified model.
- [ ] `make check` passes.
- [ ] Credential-scrubbed `make ci` passes.
- [ ] Clean-tree `make release-check` passes.
- [ ] The GitHub workflow parses and starts successfully.
- [ ] One workflow run contains exactly one job.
- [ ] No deleted workflow/test/document is still referenced by active paths.
- [ ] No real crate is published during plan implementation.

---

## 6. Expected File-Level Changes

The implementation should normally touch only the following active files.

### Required modifications

```text
.github/workflows/ci.yml
Makefile
README.md
docs/release.md
tests/native_forge_smoke.rs
tests/docs_keyless_contract.rs
```

### Required deletions

```text
.github/workflows/native-forge-smoke.yml
docs/release-verification.md
tests/native_forge_workflow_contract.rs
tests/release_document_contract.rs
```

### Conditional modifications

Other active documentation or tests may be updated only when an explicit stale reference is found, for example:

```text
docs/provider-setup.md
docs/tool-matrix.md
docs/architecture/codegg-contract.md
```

Do not broadly rewrite architecture documentation.

### Files that should not require production behavior changes

```text
src/**
Cargo.toml dependency graph
Cargo.lock dependency graph
benches/**
fuzz/fuzz_targets/**
```

A production-code change is a warning that the pass is drifting. Stop and determine whether the change is truly required for orchestration simplification.

---

## 7. Suggested Commit Sequence

Use small commits so a smaller model or reviewer can diagnose failures.

### Commit 1 — CI and Makefile reduction

```text
ci: collapse verification to one routine job
```

Contents:

- replace `.github/workflows/ci.yml`;
- simplify Makefile targets;
- do not yet delete release docs/tests if that makes the commit hard to verify.

Verify:

```bash
make check
```

### Commit 2 — retire release evidence and native workflow

```text
chore: retire release evidence and scheduled adapter verification
```

Contents:

- delete native-forge workflow;
- delete release-verification record;
- delete workflow/release contract tests;
- simplify native smoke test evidence plumbing.

Verify:

```bash
make check
```

### Commit 3 — documentation alignment

```text
docs: codify manual crates.io release process
```

Contents:

- rewrite release documentation;
- update README development section;
- narrow keyless documentation contract tests;
- remove stale active references.

Verify:

```bash
make check
```

### Commit 4 — final validation fixes only, if needed

```text
fix: close CI simplification validation gaps
```

This commit may contain only corrections found by the final routine/release gates. Do not add new verification layers.

A single commit is acceptable if the implementing environment cannot safely stage multiple commits, but the implementation notes must still report the work by gate.

---

## 8. Failure Guidance

### 8.1 `cargo test --all-features` does not run an expected integration test

First verify whether the test has a crate-level feature gate, `required-features`, or unusual harness declaration. Prefer one explicit test command inside `make ci` over restoring a separate GitHub job.

### 8.2 No-default behavior has test-only branches

If meaningful tests compile only with no features, run both:

```bash
cargo test --locked --no-default-features
cargo test --locked --all-features
```

inside the same job. Do not restore a feature matrix.

### 8.3 One-job CI is slower than expected

Do not immediately add jobs or caching. First measure where time is spent. Forty independently initialized jobs can appear parallel while consuming substantially more aggregate time and generating delayed tail failures. Keep the one-job model unless measured wall-clock feedback is clearly worse.

### 8.4 `cargo publish --dry-run` reports a dirty tree

That is expected during active development. `make check` must still pass. Commit or stash changes before `make release-check`; do not add `--allow-dirty` to the documented release gate.

### 8.5 Deleted workflow is required by a static test

Delete or narrow the workflow-shape test. Do not recreate the workflow merely to satisfy a test whose subject has been intentionally retired.

### 8.6 Native smoke test fails after evidence code removal

Preserve its direct assertions and confirm only evidence-output dependencies were removed. Do not weaken adapter correctness assertions solely to make the simplification pass green.

### 8.7 Documentation tests fail because exact headings changed

Determine whether the assertion protects a stable user contract or obsolete process wording. Preserve stable keyless product claims; remove release-ledger and workflow-shape wording assertions.

### 8.8 Branch protection still requests deleted jobs

This is a GitHub settings issue. Update required checks to `CI / ci`. Do not reintroduce old jobs to satisfy stale protection configuration.

---

## 9. Final Completion Checklist

### CI complexity

- [ ] Exactly one GitHub Actions workflow is active for routine CI.
- [ ] Routine CI has exactly one job.
- [ ] No job matrix exists.
- [ ] No required macOS job exists.
- [ ] No per-target fuzz jobs exist.
- [ ] No packaging or documentation publication jobs exist.
- [ ] No artifact upload/download exists.
- [ ] No scheduled verification workflow exists.

### Local iteration

- [ ] `make check` is the documented routine command.
- [ ] `make check` does not require a clean tree.
- [ ] `make check` does not run release-only work.
- [ ] Substantive deterministic tests run once.
- [ ] Credential-free operation remains covered.

### Manual release

- [ ] `make release-check` is the documented local packaging gate.
- [ ] `cargo publish --locked` is the documented publication command.
- [ ] Release cadence is explicitly maintainer-controlled.
- [ ] GitHub Actions has no publication role.
- [ ] crates.io immutability and required version bump after publication are documented.
- [ ] Tagging is manual.
- [ ] GitHub release creation is optional and manual.

### Retired machinery

- [ ] R/E release-subject/evidence-commit protocol is removed from active docs.
- [ ] Mutable release-verification record is deleted.
- [ ] Native forge evidence workflow is deleted.
- [ ] Workflow-shape parser test is deleted.
- [ ] Release-record contract test is deleted.
- [ ] Native smoke tests no longer write evidence artifacts.
- [ ] Optional adapter credentials cannot block core CI or release.

### Correctness retained

- [ ] Behavioral keyless tests remain.
- [ ] Fetch and redirect safety tests remain.
- [ ] Bounded-read and subprocess tests remain.
- [ ] Property/adversarial tests remain in the Cargo test inventory.
- [ ] Retrieval-accounting tests remain.
- [ ] Schema/corpus tests remain unless independently proven obsolete.
- [ ] Ignored live/native provider diagnostics remain directly runnable.

### External follow-up

- [ ] Branch protection requires only `CI / ci`.
- [ ] Stale required-check names are removed in repository settings.

---

## 10. Definition of Done

This line of work is complete when a normal push or pull request produces one required CI job, `make check` gives a fast deterministic local answer without packaging work, `make release-check` provides a deliberate clean-tree packaging gate, and publication is performed manually by a maintainer through crates.io.

The repository must no longer treat release verification as an evidence-production subsystem. Optional provider conformance, fuzzing, benchmarks, macOS checks, and live network tests remain available where they add diagnostic value, but none may impede routine iteration or unrelated releases.

The final operating model must be understandable from the Makefile and `docs/release.md` without cross-referencing a mutable verification ledger, exact workflow artifacts, or a chain of release-subject and evidence-only commits.
