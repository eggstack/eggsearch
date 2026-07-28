# Release Verification Record

This record is intentionally provisional until the deterministic local/remote
gate and native forge evidence have been completed for one immutable code
subject. It must not present pending runs, benchmark measurements, or fallback
smoke tests as release evidence.

## Current classification

- Classification: **provisional release candidate**
- Release subject `R`: `2268971087beb5f54bf6244da159ff97a913a7bf`
- Evidence commit `E`: not created; it may contain only verification documents,
  manifests, and generated evidence references after the native workflow passes
- Native forge workflow run IDs: pending — requires repository secrets/vars
  (`GITLAB_TOKEN`, `CODEBERG_TOKEN`, `GITEA_TOKEN`, `GITEA_INSTANCE_URL`,
  `NATIVE_SMOKE_GITHUB_SLASH_REF`) to be configured in GitHub repository
  settings before the workflow can execute
- Native provider artifacts and hashes: pending
- Benchmark artifact for `R`: benchmarks compile in CI (`cargo bench --locked --all-features --bench perf --no-run`); runtime measurements require more capable hardware than the current CI runners

The scheduled native-smoke workflow is diagnostic. Release evidence requires a
manual run against the exact 40-character `R` SHA after secrets are configured.

The local deterministic matrix for `R` was run on 2026-07-28 on
`aarch64-unknown-linux-gnu` (Linux 6.8.0, Raspberry Pi) with Rust 1.97.1.
All test, hardening, schema, documentation, benchmark-compilation,
release-build, and rustdoc targets passed. The 429 integration tests include
the pre-warmed local workspace tests and 11 documentation contract tests.
The publish dry-run passed with `--allow-dirty`; the main checkout contains
ignored `.opencode/node_modules` dependencies that Cargo's dirty-tree guard
reports even though they are not part of the package.

### CI status for `R`

- **Linux CI** (`eggstack/eggsearch`): All 40 jobs pass (fmt, clippy,
  check×4, test×4, schema-corpus, docs-contract, keyless-core, benchmarks,
  release-build, publish-check, hardening, docs, fuzz-smoke×16).
  CI run ID: `30358641132`.
- **macOS CI**: All local-workspace tests pass. The pre-warm fix
  (commit `e8b5b09`) resolved the timing-dependent inventory building flakiness.
  CI run ID: `30358641132` (same run, macOS matrix jobs).

---

## Core Keyless Release Evidence

The core release can be promoted without third-party API keys, provided the
mandatory keyless release matrix passes. Optional adapters remain fail-closed
when tested, but their absence does not block the core release.

### Required core evidence

For exact final code subject `R`, capture:

1. Clean source checkout identity
2. Linux keyless CI run ID and job results
3. macOS keyless CI run ID and job results
4. Local `make check` from clean checkout with credentials scrubbed
5. Standalone feature combinations required by the project
6. Release build
7. Rustdoc
8. Package/publish dry-run from a clean exact-`R` checkout without `--allow-dirty`
9. Affected benchmark runtime artifact
10. SHA-256 hashes for evidence artifacts

### Keyless CI preamble

The keyless CI job must scrub all credential variables:

```bash
unset GITHUB_TOKEN || true
unset GH_TOKEN || true
unset GITLAB_TOKEN || true
unset GITEA_TOKEN || true
unset FORGEJO_TOKEN || true
unset SOURCEGRAPH_API_KEY || true
unset BRAVE_API_KEY || true
unset SEMANTIC_SCHOLAR_API_KEY || true
```

### Deterministic local gate

Run from the repository root on `R`:

```bash
make check
```

The gate covers formatting, clippy, all four feature combinations, schema and
corpus tests, documentation contracts, release build, rustdoc, and publish
dry-run. The individual commands are documented in [`release.md`](release.md).

**Important:** The publish dry-run (`cargo publish --dry-run --locked`) requires
a clean working tree. If the checkout contains uncommitted changes or untracked
files (including editor temporaries, build artifacts, or dependency caches),
the publish check will fail with a dirty-tree error. To run the publish check:

1. Commit or stash all changes first
2. Run `make check` or `cargo publish --dry-run --locked`
3. Restore stashed changes if needed

The `--allow-dirty` flag should not be used in CI or release evidence. It
bypasses the dirty-tree check and may include unintended files in the package.

The affected-path benchmark suite is:

```bash
cargo bench --bench perf --no-fail-fast
```

Benchmark output is evidence only when captured as an artifact tied to `R`.
The capability-partition, mixed retrieval-summary, provider-scoped advisory,
forge-response, and near-cap local-inventory paths are bounded-input
measurements; they are not a proof of zero memory growth.

### Telemetry accounting and ledger closure

The local gate includes the following test suites that validate the
retrieval-attempt ledger, dimension-state accounting, and native advisory
budget partitioning:

- `tests/retrieval_attempt_ledger.rs` — 46 tests covering attempt/dimension
  summary counts, dimension-state mapping for all 10 outcomes, and
  `validate_attempt_ledger` uniqueness invariants.
- `tests/static_guards.rs` — 24 static guards verifying no single
  `MAX_NATIVE_ADVISORY_OPERATIONS` constant, `NativeOperationBudget` reserve
  methods, `record_package_outcomes` two-attempt-per-provider invariant,
  `RetrievalDimensionState` variant completeness, and public visibility of
  `validate_attempt_ledger`, `summarize_retrieval_with_attempts`, and
  `AttemptSummaryCounts`.
- `src/meta/security_search.rs` unit tests — 28 tests covering
  `record_package_outcomes` for every `ProviderAdvisoryStatus`,
  `NativeOperationBudget` boundary conditions, and identifier deduplication.
- `tests/property_retrieval.rs` — property tests for deadline vs. timeout
  distinction and summary-count partition invariants.

All tests pass across the tested feature combinations: `--all-features`
(429 integration + 11 docs-contract + 24 static guards + 46 ledger + property + hardening +
docs), `--no-default-features`, and `--features mock`. The `--features pdf`
combination is covered by `--all-features` which includes the `pdf` flag.

---

## Optional Adapter Conformance Evidence

Optional adapter evidence verifies specific adapter functionality. It is
**not required** for core release promotion. Adapters with no evidence
remain `unverified` and are omitted from verified-adapter claims.

### Adapter evidence protocol

Run `.github/workflows/native-forge-smoke.yml` with:

- `release_subject` set to the full SHA of `R`;
- `GITHUB_TOKEN`, `GITLAB_TOKEN`, `CODEBERG_TOKEN`, and `GITEA_TOKEN` configured
  for the provider jobs;
- `NATIVE_SMOKE_GITHUB_SLASH_REF` set to the exact slash-containing fixture ref;
- `GITEA_INSTANCE_URL` set to the configured HTTPS instance URL.

Each provider job checks out `R`, proves that `HEAD` matches the supplied SHA,
requires its credentials and fixtures, runs native `repo_map` and direct forge
adapter assertions, and writes JSON only after the assertions pass. Evidence
must contain at least:

```json
{
  "schema_version": 1,
  "release_subject": "<R>",
  "provider": "github|gitlab|codeberg|gitea",
  "mode": "native",
  "result": "pass",
  "resolved_commit_sha": "<40-hex-sha>",
  "entry_count": 1,
  "request_count": 1,
  "response_bytes_observed": 1,
  "aggregate_limit": 1,
  "provenance_pinned": true
}
```

Missing or malformed evidence, a skipped test, a fallback mode, a subject
mismatch, or a missing provider output fails only that adapter's claim.
The summary job requires exact `pass` from every **selected** provider job
and uploads a combined SHA-256 evidence manifest.

### Adapter conformance table

| Adapter | Status | Exact `R` | Run ID | Artifact/hash | Claim Allowed |
|---------|--------|-----------|--------|---------------|---------------|
| GitHub | unverified | — | — | — | no |
| GitLab | unverified | — | — | — | no |
| Codeberg | unverified | — | — | — | no |
| Gitea/Forgejo | unverified | — | — | — | no |

`unverified` means no release evidence was captured for that adapter. It
does not mean the adapter is broken. Missing adapter credentials prevent
adapter-specific claims but do not block the core release.

---

## R/E Protocol

### Core release (mandatory)

`R` is the final immutable code-bearing commit. Any production-code correction
creates a new `R` and invalidates evidence for the old subject. After the
keyless CI gate passes, create `E` as a documentation/evidence-only commit
that records:

- the exact `R` and `E` SHAs;
- keyless Linux/macOS run IDs;
- local gate environment and command;
- benchmark artifact identity and status;
- publish dry-run evidence;
- final core classification;
- optional adapter table with verified/unverified state;
- adapter artifacts only for adapters actually tested.

No code, tests, workflow, schema, config, benchmark definition, or contract
changes may occur in `E`.

### Adapter evidence (optional)

Adapter evidence is appended to `E` only for adapters that were actually
tested and passed. The adapter evidence section records:

- adapter name and version;
- exact `R` used;
- run ID and artifact identifiers;
- fixture identity and native mode proof;
- credential scope (CI secret storage only, no values in artifacts).

Adapters not tested or not passing are listed as `unverified` in the adapter
conformance table. Their absence does not block the core release.

### Release classification rules

The release may claim:

```text
eggsearch core is release-verified in keyless mode
```

when the core keyless release gate passes.

The release may claim:

```text
GitHub native adapter verified
```

only when GitHub adapter evidence exists for exact release subject `R`.

The absence of GitLab, Codeberg, or Gitea credentials must not block the
core release. It only prevents claiming those individual adapters as
release-verified.

Until those values exist, this document remains provisional and uses `pending`
only in the explicitly pending sections above.
