# Mode-Off Repository Locality M002 — Immutable Corrected Eggsearch Release and CodeGG Handoff

Status: ready
Primary class: capability / immutable release qualification
Planning baseline: eggsearch `main` at `90e996ef6c83c969e30c30a36bc9d968a85dc00f`.
Roadmap: `plans/subsystems/mode-off-repository-locality-corrective-roadmap.md`
Normative: `plans/000-long-term-specification.md`, `plans/002-long-term-roadmap.md`, `plans/003-planning-process.md`.
Historic release precedent: `plans/implementation/codegg-legacy-search-parity/005-v0.4.1-corrective-release.md`, `plans/closure/codegg-legacy-search-parity/005-status.md`.
Current producer precedent: `plans/implementation/eggpack-release-adoption/001-seven-target-eggpack-producer-cutover.md` and its M001 closure.
Release contracts: `docs/release.md`, `release/eggpack/`, `packaging/release-targets.txt`, `packaging/release-validate.sh`, `packaging/check-workflow-pins.py`, `packaging/check-planning-consistency.py`.
Downstream consumer: `dbowm91/codegg`, C002 `plans/implementation/search-eggsearch-pair-compatibility-corrective/002-v041-local-only-offline-dispatch-requalification.md`.

## 1. Objective

Once Mode-Off local repository dispatch is **strictly corrected and independently tested** in upstream M001, publish a new immutable eggsearch patch release carrying that fix and supply a precise, reproducible compatibility contract to CodeGG. The user has explicitly said CodeGG is far from operational/release-ready; **eggsearch's corrected upstream release must not be interpreted as a CodeGG release**.

The present public Cargo version on main is `0.4.2`. Choose the next available, valid **patch** version at implementation time (normally `0.4.3` if unoccupied), after verifying tags/crates/releases. Never overwrite `v0.4.1` or `v0.4.2`; don't fix this by moving a tag or publishing different bytes under an existing number.

## 2. Dependencies and release boundaries

**Hard:** M001 strict closure with:
- Mode-Off local workspace matches and **zero counted provider engine and independent HTTP/package/forge invocations** across positive, negative and failure cases;
- unchanged live-mode auto/explicit-provider routing and ten-tool MCP wire shape;
- passed focused regression suite, offline `make check`, docs, source evidence.

**Operational:** Publish authority for crates.io and GitHub, exact candidate build/qualify runner, all required native target builds and smoke environments, attestation/checksum provenance, ability to review and publish a draft release. If unavailable, keep M002 blocked/conditionally complete in the registry; do not label CodeGG unblocked based on main-branch-only code.

**Interface:** CodeGG `scripts/test-eggsearch-pair.sh` currently pins `v0.4.1` and the exact source SHA. A *future downstream plan* will update its chosen pin only after this immutable fixed tag exists and the zero-remote proof is confirmed; eggsearch M002 does not write to CodeGG.

## 3. Invariants and non-goals

- Preserve all ten stable MCP tool IDs, their argument/result envelopes, provider inventory/disposition and compat mappings (e.g. GitHub repository provider ID, retired Google News). This is a patch correctness fix, not a provider expansion or new tool family.
- Preserve normal `search.mode="live"` and default provider behavior. `providers=[]` continues to mean auto/default selection in live mode. Local-only Mode Off alone must deny provider/resolver dispatch.
- Do not add a new external provider, turn local-only into raw network fallback, or change archive installer/updater semantics.
- Use the **current post-Eggpack** release producer/contracts; historical `v0.4.1` asset counts/workflows are reference evidence, not commands to blindly rerun. Verify the exact modern asset inventory from `release/eggpack/distribution.toml` and current workflows (v0.4.2 used 19 assets).
- Existing tagged releases, crates.io versions, attestations and lockfiles are immutable history; preserve them.
- No CodeGG TUI/agent release, installer, provider credentials, live DuckDuckGo test, or changes in CodeGG repository as part of this eggsearch milestone.
- No cache/persistence migration, network-sandbox claim or indefinite background qualification process.

## 4. Ordered work packages

### WP1 — Immutable fix baseline and version-selection audit

1. Review M001's strict closure record and confirm it tests a failing baseline / mutation and the exact zero-call matrix, including independent package metadata resolution.
2. Confirm `main` version and latest **published** immutable crate/tag/GitHub release are consistent. Pick a previously unused patch version; record chosen version, source SHA and the history of `v0.4.1` / `v0.4.2`.
3. Inspect the current Eggpack producer workflow generated from `release/eggpack/` and `docs/release.md`; identify exact supported targets, asset inventory, glibc/MSRV, provenance/attestation and publication order. Confirm optional Windows ARM64 and ARMv7 requirements from current contract.
4. Prepare a candidate commit with the M001 code + focused tests, Cargo.toml/Cargo.lock version bump, concise changelog entry and any corresponding package/docs version text. Do not smuggle in unrelated code. Capture `CANDIDATE_SHA`.

### WP2 — Source candidate qualification

At exactly `CANDIDATE_SHA`, run all appropriate offline gates:

```bash
cargo fmt --all -- --check
cargo test --locked --lib meta::adapter::tests -- --test-threads=1
cargo test --locked --test provider_routing -- --test-threads=1
cargo test --locked --test dispatch_fault_injection -- --test-threads=1
python3 packaging/check-planning-consistency.py
make check
make docs-check
make release-candidate-check
make release-check
git diff --check
```

Record every run, target, version output, test count and skip; do not mark an unrun gate passed. A **locked, no-network regression** proving Mode-Off local repository requests create no provider jobs/resolver calls is mandatory and must be rerun on exact candidate. The paired CodeGG process gate is downstream and not replaced by an eggsearch-only unit result.

### WP3 — Exact-candidate hosted artifact qualification

Use the current Eggpack release producer, generated workflow shape, and documented `mode=qualify` exact-SHA flow for this repository. Require:
- preflight and all currently mandatory target-build/runtime checks passing;
- candidate identity consistent across source commit, crate package, version output and staged build;
- asset names/counts and checksums matching the **current** distribution contract;
- no tag/release mutation during qualification;
- static/regression security checks still green;
- recorded workflow run ID and jobs/attestation applicability, not a guessed release success.

If qualification discovers an unrelated blocker, stop and record the issue. Do not weaken the zero-provider guarantee to complete publication.

### WP4 — Publication with immutable provenance

Only after the exact candidate qualifies: publish the crate, verify registry visibility, create the new version tag at `QUALIFIED_SHA`, run the tag-triggered release producer, inspect artifacts/checksums/attestations back to repository/workflow/source SHA, verify downstream binary provenance and native executable startup (including the required Windows targets), then publish the verified draft according to `docs/release.md`.

Record crate version, exact release tag/commit and asset digests. No backfilling or repointing earlier tags. If credentials/workflow availability block publication, classify M002 as **blocked**, not closed, and leave the tested source candidate available for further development.

### WP5 — Precise CodeGG consumer handoff

Publish a bounded contract matrix in `docs/codegg-integration.md`, `architecture/codegg-contract.md` or the existing owning documentation:
- supported new eggsearch version, immutable source SHA, package/release manifest and binary digest/provenance;
- observed Mode-Off positive local-only behavior with explicit local backend and `include_local=true`;
- zero **remote engine, package resolver, forge and other outbound path** attempts under Mode Off;
- `include_local=false`, local-disabled and missing-local fallbacks remain policy-denied, not auto-routed;
- live mode provider semantics, ten MCP tools and structured `retrieval_summary` remain compatible;
- minimum CodeGG requalification: upgrade pin/static version owners, update `CODEGG_EGGSEARCH_BIN` or immutable source pointer, verify explicit private Mode-Off config and local fixture, run real native `repo_search` plus current `websearch`/`security_search` denial tests and `scripts/verify.sh quick` on same CodeGG SHA.

This handoff **does not close CodeGG C002 itself** or authorize legacy search backend retirement. Only CodeGG's independently run pinned-binary paired-process qualification may clear that downstream gate.

## 5. Failure modes and security

The release must not claim zero network by observing empty output or timeout. The functional proof is counted zero calls to every upstream outbound seam; a single `SearchEngine`, package resolver or forge adapter call during Mode-Off path blocks publication. Test local symlink/path boundaries and trust labeling remain unaffected (covered by M001). No secrets, user workspace data or local fixture content should appear in release logs, attestation subjects or telemetry by accident. Cancellation/timeouts and empty local results may return partial/skipped evidence but must never auto-dispatch remote work.

## 6. Acceptance and stop conditions

**Strict M002 closure requires:**
1. upstream M001 strict closure plus exact candidate SHA and regression evidence;
2. all current offline gates and exact-SHA multi-target qualify green;
3. immutable newly published crate/tag/GitHub Release with the *current* full artifact inventory and attestations/provenance;
4. a reproducible local-only Mode-Off zero-call fixture trace against the release candidate;
5. a downstream handoff with explicit CodeGG test instructions, without claiming CodeGG already passes.

**Stop** if patch version is already occupied, upstream policy change is not backward compatible, release producer cannot guarantee artifact identity/provenance, or any provider/resolver counter is nonzero. Never overwrite tags or skip provenance to force release.

Closure: `plans/closure/mode-off-repository-locality-corrective/002-status.md` with source/qualification/publication SHAs, CI run IDs, native targets, artifact/digest inventory, positive and adversarial no-remote test matrix, exact public tag/URL, security and migration assessment, CodeGG handoff link, residual issues and release/consumer distinction.
