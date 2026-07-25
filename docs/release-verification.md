# Release Verification Record

This record is intentionally provisional until the deterministic local/remote
gate and native forge evidence have been completed for one immutable code
subject. It must not present pending runs, benchmark measurements, or fallback
smoke tests as release evidence.

## Current classification

- Classification: **provisional release candidate**
- Release subject `R`: `97ebae60df6f7b367f9152b32c848a9af0ed8721`
- Evidence commit `E`: not created; it may contain only verification documents,
  manifests, and generated evidence references after the native workflow passes
- Native forge workflow run IDs: pending
- Native provider artifacts and hashes: pending
- Benchmark artifact for `R`: pending; benchmark definitions compile with the
  repository, but no release-subject measurements are recorded here

The scheduled native-smoke workflow is diagnostic. Release evidence requires a
manual run against the exact 40-character `R` SHA.

The local deterministic matrix for `R` was run on 2026-07-24 on
`aarch64-unknown-linux-gnu` with Rust 1.97.1. All test, hardening, schema,
documentation, benchmark-compilation, release-build, and rustdoc targets
passed. The publish dry-run was rerun from an isolated clean worktree at `R`
and passed; the main checkout contains ignored `.opencode/node_modules`
dependencies that Cargo's dirty-tree guard reports even though they are not
part of the package.

## Deterministic local gate

Run from the repository root on `R`:

```bash
make check
```

The gate covers formatting, clippy, all four feature combinations, schema and
corpus tests, documentation contracts, release build, rustdoc, and publish
dry-run. The individual commands are documented in [`release.md`](release.md).

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

All tests pass across all four feature combinations: `--all-features`,
`--no-default-features`, `--features mock`, and `--features pdf`.

## Native forge evidence protocol

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
mismatch, or a missing provider output fails the release gate. The summary job
requires exact `pass` from all four provider jobs and uploads a combined
SHA-256 evidence manifest.

## R/E protocol

`R` is the final immutable code-bearing commit. Any production-code correction
creates a new `R` and invalidates native evidence for the old subject. After
the workflow passes, create `E` as a documentation/evidence-only commit that
records:

- the exact `R` and `E` SHAs;
- the native workflow run ID;
- every provider job result and artifact identifier;
- the combined manifest and evidence-file SHA-256 hashes;
- benchmark artifact identity and status;
- the final classification.

Until those values exist, this document remains provisional and uses `pending`
only in the explicitly pending sections above.
