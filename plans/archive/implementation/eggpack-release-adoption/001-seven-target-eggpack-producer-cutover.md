# Eggpack Release Adoption M001 — Seven-Target Eggpack Producer Cutover

Status: ready

Primary class: capability / cross-repository adoption

Source roadmap:

- `plans/subsystems/eggpack-release-adoption-roadmap.md`

Eggsearch implementation baseline:

- `33f508d87b9623b785bd151f46779f01b2409eb3`

Paired producer plan:

- `eggstack/eggpack@325d44e3a29ab1fe48c3a757f5e59b9017762847`
- `eggstack/eggpack: plans/implementation/ecosystem-adoption/003b-eggsearch-seven-target-eggpack-producer-cutover.md`

Producer preflight closure:

- `eggstack/eggpack: plans/closure/ecosystem-adoption/003a-status.md`

Long-term references:

- `plans/000-long-term-specification.md#6`
- `plans/001-terminology-and-domain-model.md#6`
- `plans/002-long-term-roadmap.md`
- `plans/003-planning-process.md`

## 1. Objective

Replace Eggsearch's hand-maintained seven-target release build/finalize/draft writer with Eggpack's checked-in generated producer pipeline while preserving Eggsearch's public release, installer, updater, and provenance guarantees.

The implementation is consumer-owned. It must not modify Eggpack production code.

At closure:

- Eggpack is the sole producer authority for target/asset/checksum/build/qualification/finalization/staging facts;
- Eggsearch remains authority for crates.io/tag sequencing, public wrapper behavior, updater/service behavior, provenance, and manual publication;
- the old hand-maintained workflow no longer writes release drafts.

## 2. Baseline release guarantees

The current `v0.4.1` release establishes the compatibility floor:

- seven required targets;
- stable version-free public binary names;
- glibc 2.17 GNU compatibility;
- ARMv7 `--version` / `--help` runtime execution under QEMU/container;
- native macOS x86-64/AArch64;
- native Windows x86-64/AArch64;
- exact checksum sidecars;
- public `install.sh` and `install.ps1`;
- Cargo fallback only for unsupported host or exact binary 404;
- binary-first updater using the same public names;
- Artifact Attestations for seven binaries + two public installers;
- manual publication after draft review;
- immutable published release.

M001 may add evidence files but may not regress any of those guarantees.

## 3. Planned public inventory change

The current exact release inventory is 16 files.

Eggpack ProductWrappers staging intentionally adds three files:

- `release-manifest.json`;
- `install-exact.sh`;
- `install-exact.ps1`.

Post-cutover exact inventory is therefore 19 files.

The change is additive:

- all current 16 names stay;
- `install.sh` / `install.ps1` remain the public friendly wrappers;
- the exact installers are producer-qualified evidence and deterministic exact-version install surfaces;
- `release-manifest.json` is final-byte producer evidence.

Update every exact-count guard/doc from 16 to 19 in the same cutover.

Do not make the public wrappers delegate to the exact installers in this milestone; Bootstrap M003 already found that preserving 404-only fallback would require a new stable outcome protocol.

## 4. Producer configuration

Create a checked-in `release/eggpack/` tree that is the producer authority.

At minimum include:

- `distribution.toml`;
- pack config;
- build bindings;
- qualification bindings;
- consumer-validator map;
- reusable workflow shape;
- GitHub runner/provider policy;
- cross-tool provisioning policy;
- install policy;
- installer presentation;
- draft template/staging policy.

Exact filenames may follow the established Eggsact/StegoEggo convention, but the set must be explicit and documented.

The distribution contract must retain the seven public binary names exactly.

`packaging/release-targets.txt` may remain only as a **consumer compatibility table** used by updater/install/docs guards. It must not drive the generated producer workflow after cutover. The packaging check must compare it against the Eggpack contract rather than treat both as coequal producer inputs.

## 5. Toolchain migration and glibc proof

Remove the release workflow's direct `mlugg/setup-zig` / hand-installed cargo-zigbuild authority.

Eggpack cross-tool policy:

- Zig `0.14.1`;
- cargo-zigbuild `0.23.3`;
- x86-64 Zig archive SHA-256 `24aeeec8af16c381934a6cd7d95c807a8cb2cf7df9fa40d359aa884195c4716c`;
- AArch64 Zig archive SHA-256 `f7a654acc967864f7a050ddacfaa778c7504a0eca8d2b678839c21eea47c992b`.

For each GNU target:

- x86-64;
- AArch64;
- ARMv7;

run `readelf --version-info` against the real generated candidate and fail if any `GLIBC_*` requirement exceeds 2.17.

Record the candidate SHA-256 with the glibc evidence.

If the bump violates the floor, stop. Do not change the documented floor and do not add legacy Zig support locally.

## 6. ARMv7 required consumer validator

Create an Eggsearch-owned Python3 validator, e.g. `packaging/eggpack-armv7-validator.py`.

Eggpack configuration for ARMv7:

- build: CargoZigbuild;
- core qualification: Structural;
- no core smoke binding;
- required consumer validator: the Eggsearch script.

The script receives the exact candidate as argv[1].

It must:

1. reject missing/non-file candidate;
2. verify the ELF is 32-bit ARM;
3. run the glibc <= 2.17 symbol check;
4. execute `--version` and `--help` in an ARMv7 runtime;
5. compare version output to the exact candidate release version available from checked-in/repository context without consulting a moving release;
6. bound subprocess time/output;
7. return nonzero on any inability to perform the runtime proof.

Prefer a digest-pinned ARMv7 container/runtime image. Resolve and record the immutable image digest during implementation.

Do not restore a core `smoke` block for Structural qualification.

If the generated validator job cannot execute the runtime proof on hosted Ubuntu without an unsafe/general setup hook, stop and return to planning.

## 7. Generated workflow

Generate a new checked-in workflow through the real Eggpack renderer, initially alongside the historical writer.

Recommended path while qualifying:

`.github/workflows/release-eggpack.yml`

Required:

- workflow dispatch supports exact qualification candidate;
- seven required targets;
- Windows ARM64 maps to `windows-11-arm`;
- no `--clobber`;
- no automatic publish;
- no `id-token: write`;
- no `attestations: write`;
- `eggpack ci check` proves byte-exact drift state.

Do not delete the historical `.github/workflows/release-binaries.yml` until the new workflow has passed exact-candidate qualification and draft staging evidence.

At final writer cutover:

- delete/retire the old workflow as a release writer;
- update docs/guards to the generated workflow path;
- ensure only the Eggpack-generated workflow can create/update the release draft.

## 8. Product-owned provenance workflow

Create a separate provenance workflow, e.g.:

`.github/workflows/release-attest.yml`

It must not build, stage, replace, delete, or publish release assets.

Permissions:

- `contents: read`;
- `id-token: write`;
- `attestations: write`;
- `artifact-metadata: write` only if required by the pinned attestation action.

Required procedure:

1. accept/resolve one exact existing release tag;
2. checkout and verify the tag commit;
3. require the matching GitHub Release to exist and still be a draft;
4. enumerate the exact expected 19 remote assets;
5. download them without mutation;
6. compare local digest/size with GitHub's release-asset digest/size;
7. verify all seven binary checksum sidecars;
8. parse/validate `release-manifest.json` identity and binary artifact digests;
9. verify downloaded `install.sh` / `install.ps1` equal the checked-in wrapper bytes at the tag;
10. attest at least the historical subject set: seven binaries + two public wrappers;
11. SHOULD additionally attest `install-exact.sh` and `install-exact.ps1`;
12. run `gh attestation verify` on the new subjects;
13. write only attestations, never GitHub Release state.

### Historical subject parity

Before deleting the old writer, use a token/session capable of reading `v0.4.1` attestations and record its subject names/digests.

The new workflow must cover every historical subject.

If GitHub still prevents subject readback, implementation may land, but closure must remain conditional on a named provenance-read evidence condition. Do not claim parity without evidence.

## 9. Packaging contract and guards

Update the packaging system intentionally.

### Producer authority

Producer target/name/build/staging facts come from `release/eggpack/` + generated workflow.

### Consumer compatibility mirrors

Keep only what runtime/product code needs independently:

- `packaging/release-targets.txt` as frozen public compatibility table;
- `src/platform.rs` runtime mapping;
- product wrappers;
- updater target resolution.

Update `packaging/check-contract.sh` and associated tests so they:

- verify seven-row runtime/public compatibility;
- compare each public asset name to Eggpack contract expansion;
- verify generated workflow drift with `eggpack ci check`;
- assert exact post-cutover 19-file inventory;
- assert no `--clobber`;
- assert generated workflow has no provenance OIDC permissions;
- assert provenance workflow has no release-write permission/action;
- preserve action SHA pin checking.

Do not keep old workflow-matrix parsing as a second producer authority.

## 10. Installers and updater

Preserve current `packaging/install.sh` and `packaging/install.ps1` behavior.

Preserve:

- requested/latest version selection;
- registry authority;
- exact binary URL mapping;
- exact-asset 404-only Cargo fallback;
- unsupported-host Cargo fallback;
- hard stop on other HTTP/integrity/identity failures;
- no elevation;
- service hook semantics.

Preserve `src/update.rs` behavior and exact public binary names.

Add `release-manifest.json` and exact installers only as extra release assets; updater/installers must ignore them unless explicitly intended.

## 11. Local verification before hosted qualification

Run:

```bash
make check
make release-check
git diff --check
```

Run the pinned Eggpack tool against the checked-in producer inputs:

```text
eggpack ci generate ...
eggpack ci check ...
```

Verify byte-identical repeated generation.

Run packaging checks proving:

- all seven current names;
- exact 19-file contract;
- updater/installer mappings unchanged;
- action pins valid;
- old writer not yet removed before qualification.

## 12. Hosted qualification phase

Dispatch the generated Eggpack workflow against an exact commit without publishing.

Require:

- seven builds green;
- seven core qualification outcomes green;
- ARMv7 consumer validator green;
- required gate green;
- aggregate/finalize green;
- 19-file prepared staging inventory;
- no release mutation in qualification mode.

Capture per-target candidate SHA-256.

Verify:

- glibc <= 2.17 x86-64;
- glibc <= 2.17 AArch64;
- glibc <= 2.17 ARMv7;
- Windows ARM64 native smoke;
- macOS/native targets;
- ARMv7 version/help runtime.

If any mandatory compatibility proof fails, stop before writer cutover.

## 13. Draft writer cutover

After qualification:

1. ensure exact candidate is the intended release candidate;
2. retire/delete the old workflow's draft writer;
3. ensure generated workflow is the sole writer;
4. dispatch release staging only after normal crates.io/tag prerequisites;
5. verify exact 19-file draft;
6. rerun same exact release once to prove byte-identical reuse;
7. prove no clobber path exists;
8. run provenance workflow;
9. inspect draft;
10. publish manually.

Do not allow old and new workflows to mutate the same draft in parallel.

## 14. First live release evidence

Full subsystem closure requires one normal maintainer-authorized release through the new path unless the user explicitly chooses a conditional operational closure.

After publication verify:

- release immutable;
- exact 19 files;
- seven binary/checksum pairs;
- two public wrappers;
- two generated exact installers;
- release manifest;
- Unix installer smoke;
- Windows installer smoke;
- `eggsearch update --check`;
- at least one safe real A->B binary update path if an older supported release exists;
- all required attestation subjects verify to the exact repository/workflow/commit.

## 15. Documentation

Update in the same change:

- `architecture/packaging.md`;
- `docs/release.md`;
- `docs/installation.md` if asset inventory text changes;
- planning registry/roadmap;
- release skill/docs that currently say exact 16 assets;
- any test inventory/static guard descriptions.

Historical `v0.4.1` remains documented as a 16-file release; do not rewrite history to say it had 19.

## 16. Rollback

Before publication:

- restore the prior workflow from Git if new qualification/staging fails;
- delete only an unpublished draft that was intentionally created for the failed candidate;
- never overwrite published assets.

After publication:

- do not mutate the immutable release;
- fixes use a new version/tag/release.

## 17. Acceptance criteria

M001 closes only when:

1. paired Eggpack plan remains aligned;
2. `release/eggpack/` is checked in and owns producer facts;
3. generated workflow is drift-clean;
4. old writer is retired after qualification;
5. all seven binary names unchanged;
6. inventory transition 16 -> 19 is exact and documented;
7. glibc <= 2.17 proven for all three GNU targets after toolchain bump;
8. ARMv7 required validator proves runtime version/help;
9. Windows ARM64 remains required/green;
10. installers/updater preserve behavior;
11. draft staging is exact-tag, draft-only, no-clobber, exact-reuse;
12. generated workflow contains no provenance OIDC permission;
13. separate provenance workflow cannot mutate release state;
14. historical attestation subject parity is proven or named as a conditional closure item;
15. local gates are green;
16. hosted qualification is green;
17. one real public release proves the new path, or closure is explicitly conditional on publication;
18. public installer/updater/provenance evidence is recorded;
19. no medium-or-higher regression remains.

## 18. Stop conditions

Stop if:

- glibc 2.17 fails after toolchain migration;
- ARMv7 runtime validation cannot execute through the bounded validator seam;
- Windows ARM64 is not viable on current runner policy;
- exact 19-file additive inventory is rejected as a product contract;
- generated workflow needs OIDC/provenance permissions;
- provenance workflow needs release-write authority;
- no-clobber staging cannot replace current clobber behavior;
- updater/installer semantics change;
- Eggpack production code must change;
- release-relevant baseline changes materially before implementation.

## 19. Closure evidence

Create:

`plans/closure/eggpack-release-adoption/001-status.md`

Record:

- implementation commit(s);
- exact Eggpack pin/paired plan;
- checked-in producer config inventory;
- generated workflow drift proof;
- target/name parity;
- 16 -> 19 inventory diff;
- toolchain/digests;
- glibc evidence;
- ARMv7 validator evidence;
- Windows ARM64 evidence;
- draft staging receipt + rerun result;
- old-writer retirement;
- provenance subject parity + verification;
- public release/install/update evidence;
- unresolved findings;
- recommendation: closed / conditionally closed / corrective required.
