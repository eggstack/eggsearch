# Phase 18 — Transport Migration Qualification and Upstream Compression Closure

Status: planned
Depends on: phase 17 implementation `5a739a3cc6cdd060911eeefad7b004661f4b5c94`; phase 17 closure record `eb014eb92ff06ad1653eb31518ae612d7de4dec1`
Baseline for planning: `eb014eb92ff06ad1653eb31518ae612d7de4dec1` (`eggsearch` 0.3.9 on `main`)
Related upstream: `eggstack/eggfetch` 0.1.7

## Why this corrective phase exists

Phase 17's transport migration is implemented and the code-level checks are substantially complete, but its own acceptance criteria required the complete seven-target release qualification on the exact migration candidate. The phase 17 closure record explicitly states that this qualification was not run before the phase was marked implemented.

That is a closure-evidence defect, not a reason to redo the migration. Phase 18 exists to exercise the missing release qualification, bind the evidence to an exact SHA, and reconcile the phase 17 record without weakening the repository's closure rule.

The migration also exposed a separate upstream transport defect: eggfetch 0.1.7's automatic streaming decompression fails on at least some valid chunked Brotli and gzip responses while the same compressed bytes decode when served with `Content-Length`. eggsearch currently avoids that path for HTML scrape engines by requesting identity encoding. That workaround is intentionally narrow and should not turn into an eggsearch-owned decompression stack.

This phase therefore has two closure tracks:

1. prove the migrated eggsearch candidate across the full release matrix;
2. preserve the narrow identity-encoding workaround while handing the chunked-compression defect back to eggfetch with a deterministic reproducer and explicit tracking evidence.

## Objective

Close the remaining qualification/evidence gap around the eggfetch 0.1.7 migration without reopening its architecture.

The desired end state is:

- a complete non-publishing seven-target qualification run is recorded against an exact eggsearch candidate SHA;
- all release-matrix jobs and final asset assembly pass;
- the phase 17 closure record points at that qualification evidence instead of saying it remains outstanding;
- the chunked gzip/Brotli defect has a deterministic upstream eggfetch reproducer and durable tracking reference;
- eggsearch retains only the existing identity-encoding workaround for affected HTML scrape engines;
- no reqwest compatibility layer, local decompression stack, or unpublished eggfetch pin is introduced;
- any future release SHA that differs from the qualified SHA is re-qualified under the normal release rule before publication.

## Corrective ownership boundary

Phase 18 must preserve the Phase 17 ownership split.

```text
eggsearch
  owns release qualification, provider request policy, SSRF authorization,
  truncation semantics, OriginController policy, and the temporary decision
  to request identity encoding for affected HTML scrape engines

eggfetch
  owns streaming transfer decoding and automatic content decompression,
  including correctness for chunked gzip/Brotli response bodies

rmcp
  continues to own its Streamable HTTP client and transitive reqwest path
```

Do not solve an eggfetch decoder defect by rebuilding a decoder pipeline inside eggsearch.

## Work item 1 — Freeze and identify the qualification candidate

Before starting the release matrix:

1. fetch current `main` and record the exact candidate SHA;
2. verify it contains Phase 17 implementation commit
   `5a739a3cc6cdd060911eeefad7b004661f4b5c94`;
3. verify the tree is clean;
4. run:
   - `make check`;
   - `make packaging-check`;
   - `make release-check`;
5. record the exact Rust version and `eggfetch-core` resolved version/features;
6. confirm direct production `reqwest` remains absent and the static feature-budget guards pass.

The planning baseline is `eb014eb92ff06ad1653eb31518ae612d7de4dec1`, but do not hard-code that SHA as successful qualification if implementation changes production code, dependencies, build configuration, packaging, or release workflow before the run.

A qualification result is evidence only for the SHA that the workflow preflight reports as `QUALIFIED_SHA`.

## Work item 2 — Run the complete seven-target qualification in non-publishing mode

Use the existing `.github/workflows/release-binaries.yml` manual dispatch surface.

Required invocation semantics:

```text
mode = qualify
ref  = <exact candidate SHA>
```

Do not use release mode and do not create or update a GitHub Release as part of this corrective phase.

The run must exercise all existing targets:

1. `x86_64-unknown-linux-gnu`;
2. `aarch64-unknown-linux-gnu`;
3. `armv7-unknown-linux-gnueabihf`;
4. `x86_64-apple-darwin`;
5. `aarch64-apple-darwin`;
6. `x86_64-pc-windows-msvc`;
7. `aarch64-pc-windows-msvc`.

The final `assemble` job must also pass its exact asset-set validation.

Record:

- workflow run ID and canonical URL;
- `QUALIFIED_SHA`;
- package version reported by preflight;
- all seven target results;
- final assembly result;
- qualification artifact name;
- exact asset count and checksum-validation result.

Do not treat an individually rerun target as sufficient if final assembly has not passed against the same workflow attempt/candidate.

Transient runner failures may be rerun, but distinguish infrastructure retry from product correction in the closure record.

## Work item 3 — Validate the qualification artifact as release-shaped output

After the matrix passes, inspect the qualification-only artifact rather than relying only on green job status.

Verify:

- seven executables are present with names matching `packaging/release-targets.txt`;
- seven checksum files are present;
- `install.sh` and `install.ps1` are present;
- the total assembled set is exactly 16 files;
- executable bits are preserved for Unix artifacts where applicable;
- checksum files validate their corresponding binaries;
- the embedded `--version` output matches the package version;
- Linux glibc floor remains within the documented contract;
- no artifact is accidentally built from a SHA different from `QUALIFIED_SHA`.

Record per-target file sizes as a new post-migration release baseline. This is characterization only; do not infer a pre/post size win because no pre-migration binary baseline exists.

## Work item 4 — Make the chunked-compression workaround explicit and regression-protected

Audit the HTML scrape engines currently using `.decompress(false)` / identity encoding because of the eggfetch 0.1.7 defect.

The known affected group recorded by Phase 17 is:

- Brave HTML scrape path;
- DuckDuckGo;
- Mojeek;
- SearXNG;
- Startpage;
- Yahoo.

For each affected path, verify the request contract deliberately disables automatic compressed response handling rather than doing so incidentally.

Add or tighten deterministic tests where needed so that:

- affected HTML engines request identity encoding / disable eggfetch decompression;
- JSON API engines that are known-good retain normal automatic decompression;
- the workaround does not disable response-size bounds, total deadlines, redirects, or sanitization;
- provider behavior remains keyless-testable and does not depend on live public services.

Do not duplicate gzip/Brotli decode code in eggsearch.

Do not broaden the workaround to unrelated clients such as updater, health probes, JSON APIs, or `web_fetch` unless a deterministic regression proves the same defect affects that path.

## Work item 5 — Produce a deterministic upstream eggfetch reproducer

Create or update a durable upstream eggfetch tracking item for the decoder defect. Prefer the repository's planning/issue convention rather than leaving the finding only in eggsearch's historical plan text.

The reproducer must be local and deterministic:

1. generate or embed a known plaintext payload;
2. encode it as gzip and Brotli;
3. serve the exact same compressed bytes from a loopback HTTP server in two transfer shapes:
   - with `Content-Length`;
   - with chunked transfer encoding;
4. request both through the relevant eggfetch 0.1.7 high-level streaming/decompression path;
5. demonstrate:
   - the `Content-Length` response decodes to the original bytes;
   - the chunked response currently fails or corrupts decoding;
6. ensure the test covers the body-stream path used by eggsearch rather than only a lower-level decoder helper.

At minimum preserve evidence for the observed Brotli and gzip cases if both remain reproducible.

The upstream tracking record must contain:

- exact eggfetch version/commit reproduced;
- minimal test/reproducer location;
- expected vs actual behavior;
- whether gzip, Brotli, or both fail;
- confirmation that the payload itself is valid with a stock decoder;
- confirmation that the issue is transfer-shape-sensitive rather than payload-sensitive;
- the downstream eggsearch workaround and why it is intentionally temporary.

If the defect no longer reproduces on eggfetch `main`, identify the fixing commit and still add a regression test upstream before calling the issue closed.

## Work item 6 — Do not consume an unpublished or unqualified upstream fix

Phase 18 is a closure pass, not an opportunistic eggfetch upgrade campaign.

If eggfetch fixes the chunked-compression defect during this work:

- do not pin eggsearch to a Git commit;
- do not consume an unpublished crate version;
- do not widen the eggfetch feature set;
- do not remove the identity workaround merely because upstream `main` looks fixed.

A later published eggfetch release can be adopted through a small separately registered dependency-bump plan that proves the fix and removes only the workaround that is no longer necessary.

If a published patch release is already available before Phase 18 implementation begins and the user explicitly chooses to adopt it, update the plan/registry scope first rather than silently folding that upgrade into this closure pass.

## Work item 7 — Reconcile Phase 17 closure evidence

Once Work items 1-5 are complete, correct the historical record.

Update `plans/phase-17-eggfetch-0.1.7-http-transport-consolidation.md` and `plans/registry.md` so they no longer say the seven-target qualification remains outstanding for the qualified migration candidate.

The closure note must identify:

- exact qualified SHA;
- qualification workflow run ID;
- seven-target pass;
- assembly pass and exact 16-file artifact set;
- local gate results;
- retained rmcp -> reqwest boundary;
- upstream chunked-compression tracking reference;
- continued identity-encoding workaround;
- statement that the qualification evidence is SHA-specific.

Do not rewrite the Phase 17 implementation SHA. The implementation remains
`5a739a3cc6cdd060911eeefad7b004661f4b5c94`; Phase 18 supplies missing qualification evidence for a descendant candidate containing that implementation.

Also preserve this release rule explicitly:

> If the first published release containing the migration is tagged at a SHA different from the Phase 18 `QUALIFIED_SHA`, run the seven-target qualification again against the exact release candidate before publication.

Phase 18 evidence must not be presented as transferable to arbitrary future commits.

## Work item 8 — Close Phase 18 only with evidence, not intent

Append an implementation record to this plan containing:

- implementation/closure SHA;
- candidate/qualified SHA;
- local gate commands and results;
- workflow run ID/URL;
- per-target results;
- assembled artifact name and exact file count;
- artifact checksum validation;
- per-target artifact sizes;
- upstream eggfetch tracking reference and reproducer commit;
- exact list of eggsearch engines retaining identity encoding;
- any deviations or failures encountered.

Then mark Phase 18 `implemented` and update `plans/registry.md` in the same closure change.

If qualification exposes a correctness, security, portability, or packaging defect:

- do not mark Phase 18 implemented;
- mark it blocked;
- fix the defect in a scoped corrective change;
- rerun all invalidated gates;
- rerun the seven-target qualification on the new exact candidate SHA.

## Invalidating changes after qualification

The qualification run is invalidated for release purposes by any later change to:

- `src/`;
- `Cargo.toml` or `Cargo.lock`;
- build profiles or Rust/MSRV configuration;
- `.github/workflows/release-binaries.yml`;
- `packaging/release-*`;
- installers or updater behavior;
- feature selection or dependency graph;
- target matrix or smoke tests.

Documentation-only plan/registry closure commits may record the evidence, but they do not magically qualify their new SHA. A later release from that newer SHA must follow the normal exact-candidate qualification rule.

## Non-goals

Phase 18 does not:

- redesign the Phase 17 eggfetch integration;
- reintroduce direct reqwest;
- replace rmcp's reqwest-backed client transport;
- add an eggsearch decompression implementation;
- add retry policy to eggfetch;
- change MCP tool surfaces;
- add or remove providers;
- require live third-party provider tests;
- publish a crate or GitHub Release;
- adopt an unpublished eggfetch revision;
- claim a binary-size reduction that cannot be measured from retained evidence.

## Suggested execution order

```text
1. freeze exact candidate + clean local gates
2. audit/lock identity-encoding workaround tests
3. create deterministic eggfetch chunked gzip/Brotli reproducer + upstream tracking
4. rerun local gates if eggsearch tests changed
5. workflow_dispatch release-binaries.yml mode=qualify ref=<exact SHA>
6. inspect assembled qualification artifact
7. record sizes/checksums/run evidence
8. reconcile Phase 17 closure record
9. append Phase 18 implementation record + registry closure
```

If Work item 4 changes production behavior or release-relevant files, perform the qualification only after those changes are committed.

## Acceptance criteria

Phase 18 is complete only when all of the following are true:

1. A clean exact eggsearch candidate containing Phase 17 is identified by SHA.
2. `make check`, `make packaging-check`, and `make release-check` pass for that candidate or an explicitly documented clean equivalent before qualification.
3. `release-binaries.yml` runs in `qualify` mode with an explicit immutable candidate ref.
4. The preflight-reported `QUALIFIED_SHA` exactly matches the intended candidate.
5. All seven release targets pass.
6. The final assembly job passes.
7. The assembled qualification artifact contains exactly seven binaries, seven checksums, and two installers.
8. The artifact checksums validate and the binaries report the expected version.
9. Per-target artifact sizes are recorded as a post-migration baseline without an unsupported pre/post claim.
10. Direct production reqwest remains absent; rmcp's transitive reqwest boundary is unchanged.
11. The eggfetch feature budget remains bounded to the Phase 17 selection.
12. Affected HTML scrape engines have deterministic regression coverage for the identity-encoding workaround.
13. No eggsearch-owned gzip/Brotli compatibility decoder is introduced.
14. A deterministic upstream eggfetch reproducer exists for the chunked compressed-response defect, or a fixing commit plus upstream regression test is identified.
15. The upstream tracking record identifies exact version/commit, transfer shape, codec coverage, expected behavior, and downstream workaround.
16. No unpublished eggfetch revision is consumed by eggsearch.
17. The Phase 17 plan and registry closure evidence are reconciled with the qualification run.
18. The closure record explicitly states that qualification is SHA-specific and must be rerun for a different eventual release candidate.
19. Any qualification failure caused by product code is fixed and the full invalidated qualification is rerun rather than waived.
20. Phase 18 is marked implemented only after the exact evidence above is committed to the plan and registry.

## Handoff notes

The important distinction is between implementation completeness and release qualification. Phase 17's transport architecture should not be churned merely to make the paperwork look cleaner. Exercise the missing matrix, record the exact evidence, and keep the existing security/ownership boundaries stable.

Likewise, the chunked-compression defect should remain upstream. The current identity-encoding workaround is acceptable because it reduces capability rather than creating a second decompression implementation. The closure criterion is durable upstream ownership plus regression protection, not forcing an eggfetch release on the same day.

If the seven-target matrix is green and the upstream reproducer is durably tracked, this phase should be small. If either exposes a real defect, stop treating it as closure-only and register the resulting corrective work explicitly.
