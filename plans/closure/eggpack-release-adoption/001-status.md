# Eggpack Release Adoption Milestone 001 — Seven-Target Eggpack Producer Cutover

Status: **conditionally closed** — production implementation is complete and live-staged; the named condition is that publication of the `v0.4.2` draft is a manual maintainer action that was not authorized in this environment.

Source plan: `plans/implementation/eggpack-release-adoption/001-seven-target-eggpack-producer-cutover.md`

Producer-side closure (authoritative for the Eggpack subsystem): `eggstack/eggpack` — `plans/closure/ecosystem-adoption/003b-status.md`.

## 1. Repository baselines reviewed

| Role | Commit |
|---|---|
| Reviewed baseline at plan authoring | `33f508d87b9623b785bd151f46779f01b2409eb3` |
| Implementation base (pre-rebase) | `4ccf35cf54435e4b96b481b482a59cb6d14dccf6` |
| Concurrent upstream commit absorbed by rebase | `d444acce42e2eaa002ea7f48d3eae38d1c7b559c` (26 application defect fixes) |
| Baseline correction | `28a0060` |
| Cutover | `eabbf802449e32902110ae5adea4017a491c7161` |
| Release candidate tag | `v0.4.2` → `eabbf802449e32902110ae5adea4017a491c7161` |

`d444acc` landed while the cutover was in flight and touches application source only — not `Cargo.toml`, `Cargo.lock`, `CHANGELOG.md`, `packaging/`, or `.github/`. The two cutover commits were rebased onto it and **the whole local gate was re-run on the rebased base**, not on the pre-rebase tree. Every gate result below is from the post-rebase run.

## 2. Executive finding

Release production in eggsearch is now owned by Eggpack through one checked-in authority set. The seven public target names, their seven checksum sidecars, and the two public installers are unchanged; the public surface grows additively to 19 assets. The generated writer staged a real draft for `v0.4.2` with exactly those 19 assets, on seven hosted runners including `windows-11-arm`, with the ARMv7 runtime proof executed by a required product-owned validator, and with no OIDC write permission anywhere in generated code. A second dispatch of the identical tag refused to overwrite the staged bytes, which is the no-clobber invariant behaving correctly.

## 3. Requirement-to-evidence matrix

| Plan §17 criterion | Evidence | Result |
|---|---|---|
| 3.1 producer contract checked in | `release/eggpack/` — 10 files | pass |
| 3.2 generated workflow + drift guard | `release-eggpack.yml` (75,695 bytes, 27 jobs); hosted run `37530846521` `ci check: match` | pass |
| 3.3 all seven target names unchanged | every name cross-checked against published `v0.4.1`; 16 historical names byte-identical | pass |
| 3.4 ARMv7 required validator passes on the exact staged binary | run `37531104901`, `outcome: passed`, `candidate_sha256 a6889036…` | pass |
| 3.5 Windows ARM64 published and passes | `aarch64-pc-windows-msvc.exe`, 21,561,344 bytes, built on `windows-11-arm` | pass |
| 3.6 additive 16 → 19 inventory guarded | `packaging/expected_release_assets.py` derives the set from the contract | pass |
| 3.7 Linux x86-64 and aarch64 pass | both `qualify` and `validate` jobs success | pass |
| 3.8 glibc ≤ 2.17 on all three GNU targets | enforced per target by `validate-release-binary.py` before staging could aggregate | pass |
| 3.9 Windows x86-64 passes | both jobs success; 25,757,696 bytes | pass |
| 3.10 macOS x86-64 and aarch64 pass | both jobs success | pass |
| 3.11 target identity derives from the contract | installers, updater, and docs all asserted equal to `release-targets.txt`, which is asserted equal to the contract | pass |
| 3.12 release id and source revision prove provenance | `release-manifest.json` in the staged draft; verified locally |
| 3.13 external installer smoke on published assets | **not performed** — `v0.4.2` is not published | conditional |

## 4. Production implementation evidence

### 4.1 Producer authority

`release/eggpack/` is the single authority for the release contract, build policy, qualification policy, consumer validators, runner mapping, action pins, staging policy, and installer presentation. `packaging/gen-release-workflow-shape.py` derives the render seam `workflow-shape.json` from those files rather than restating them, which neither earlier Eggpack consumer does — at seven targets a hand-maintained shape would be a second copy of every producer fact.

### 4.2 Generated workflow

Rendered with the pinned local tool and confirmed byte-stable across two renders; the working tree was unchanged after regeneration:

```text
eggpack ci generate … -> generated 75695 bytes                                   exit 0
eggpack ci check   … -> ci check: match (75695 bytes)                            exit 0
cmp render-a.yml render-b.yml    -> byte-identical
```

Static assertions over the rendered bytes, all enforced by `packaging/check-contract.sh` so they cannot regress silently:

- `--clobber` — 0 occurrences.
- `id-token`, `attestations: write`, `artifact-metadata` — 0 occurrences.
- `contents: write` — exactly 1, in `stage`.
- `push:` trigger — absent; dispatch-only, so the generated writer can never race a tag push.
- `windows-11-arm` — 3 jobs (build, qualify, consumer-validate for `aarch64-pc-windows-msvc`).

### 4.3 The seven targets, as run

| # | Target | Strategy | Runner | Floor | Qualification | Required validator | Live outcome |
|---|---|---|---|---|---|---|---|
| 1 | `x86_64-unknown-linux-gnu` | CargoZigbuild | `ubuntu-24.04` | glibc 2.17 | Native | yes | build/qualify/validate success |
| 2 | `aarch64-unknown-linux-gnu` | CargoZigbuild | `ubuntu-24.04-arm` | glibc 2.17 | Native | yes | build/qualify/validate success |
| 3 | `armv7-unknown-linux-gnueabihf` | CargoZigbuild | `ubuntu-24.04` | glibc 2.17 | **Structural** | **yes** | build success; structural qualify success; required validator `passed` |
| 4 | `x86_64-apple-darwin` | NativeCargo | `macos-15-intel` | none | Native | yes | build/qualify/validate success |
| 5 | `aarch64-apple-darwin` | NativeCargo | `macos-14` | none | Native | yes | build/qualify/validate success |
| 6 | `x86_64-pc-windows-msvc` | NativeCargo | `windows-latest` | none | Native | yes | build/qualify/validate success |
| 7 | `aarch64-pc-windows-msvc` | NativeCargo | `windows-11-arm` | none | Native | yes | build/qualify/validate success |

`required_gate` succeeded only after all seven required consumer validators had passed, including ARMv7.

### 4.4 ARMv7: structural core, required runtime proof

Core qualification for ARMv7 is `Structural`, so Eggpack executes nothing there and forbids a core smoke binding. The entry in `release/eggpack/qualification-bindings.toml` is therefore intentionally empty, and the runtime proof lives entirely in `packaging/validate-armv7-candidate.py`: 32-bit ARM ELF identity, GLIBC ceiling ≤ 2.17, and `--version` plus `--help` executed under a digest-pinned ARMv7 runtime. It fails closed when Docker, binfmt registration, `readelf`, or either pinned image is unavailable — a degraded proof is never reported as a pass.

Both container images are pinned by digest rather than tag, so a retagged upstream image cannot change what the proof actually ran.

### 4.5 glibc 2.17 after the toolchain bump

Zig moved 0.13.0 → 0.14.1 and cargo-zigbuild 0.20.1 → 0.23.3, using the same official archive digests already qualified by the Eggsact and StegoEggo cutovers. `mlugg/setup-zig` is no longer a build authority anywhere in this repository.

The floor is not asserted, it is enforced on the exact bytes: every natively-hosted target runs `packaging/validate-release-binary.py`, which reads `readelf --version-info`, takes the highest `GLIBC_<n>` requirement, and fails above 2.17. Because staging aggregates only after every required consumer validator passed, a binary with a higher ceiling cannot reach a draft. Independently confirmed locally against the published `v0.4.1` binaries: both the x86-64 and ARMv7 ELF binaries report a `(2, 17)` ceiling.

### 4.6 Inventory 16 → 19

The expected set is derived from the contract, so the guard cannot drift from the producer:

```text
$ python3 packaging/expected_release_assets.py --count
19
```

- 7 executables — names unchanged from `v0.4.1`.
- 7 `.sha256` sidecars.
- `install.sh`, `install.ps1` — product wrappers, copied byte-for-byte.
- `release-manifest.json`, `install-exact.sh`, `install-exact.ps1` — additive Eggpack producer evidence.

**Wrapper byte-identity is proven by digest rather than asserted.** The `v0.4.1` attestation records `install.sh` = `d860e7437de1c5b9…` and `install.ps1` = `27373018d6129ee5…`; the `v0.4.2` staging receipt records the same two digests. `releases/latest/download/install.sh` therefore resolves to a byte-identical file across the cutover, and no consumer-visible entry point changed.

Historical releases remain 16-asset releases. Only releases produced after this cutover carry 19.

### 4.7 Staging replaces clobber

The predecessor workflow uploaded with `gh release upload --clobber`. Staging is now Eggpack's fail-closed draft stager: exact existing tag, exact source revision, absent asset uploads, byte-identical asset is reused, differing asset or unexpected remote asset refuses, published or immutable release refuses, no tag mutation, no publish. The live receipt for `v0.4.2`:

```json
{ "release_id": "v0.4.2", "tag": "v0.4.2",
  "source_revision": "eabbf802449e32902110ae5adea4017a491c7161",
  "github_release_id": 405157598, "draft": true, "immutable": false,
  "assets": [ … 19 entries … ] }
```

### 4.8 Provenance separated from the generated writer

Artifact Attestation moved to `.github/workflows/release-provenance.yml`, so no Eggpack-generated job ever requests an OIDC write permission. That workflow is dispatch-only, read-only with respect to release state, and refuses to run unless the checkout is the tag commit with a clean tree and the release is still a draft. `packaging/verify-staged-release.py` then proves the staged inventory before anything is attested.

That verifier was exercised locally against the real staged draft, not only written from the schema:

```text
$ python3 packaging/verify-staged-release.py --tag v0.4.2 --staged-dir <19 downloaded assets>
verify-staged-release: ok (v0.4.2, 19 assets, draft-only,
  digests/sizes/sidecars/manifest/wrappers verified)          exit 0
```

Attestation subject parity with the historical set is established: `v0.4.1` attests 16 subjects (7 binaries, 7 sidecars, 2 wrappers); the new workflow attests those 16 plus the two generated exact installers — a strict superset, so nothing previously attested is dropped.

## 5. Verification executed

| Command | Result |
|---|---|
| `make check` (fmt, clippy, feature-check, test, hygiene, dependency-policy, packaging-check) | `CHECK_EXIT=0` |
| `make release-check` (adds release-candidate, docs, release build, publish dry-run) | `RELEASE_EXIT=0` |
| `make producer-drift` equivalent (regenerate + `ci check` + clean tree) | match, tree unchanged |
| hosted Release drift guard, run `37530846521` | success |
| hosted `cargo publish --locked` for `v0.4.2` | published, not yanked |
| hosted Eggpack release run `37531104901` attempt 1 | 27/27 jobs success, draft staged |
| hosted rerun, same tag/source, attempt 2 | **failed closed at `stage`** — see §6 |
| `python3 packaging/verify-staged-release.py` against the live draft | exit 0 |

One verification caveat, recorded rather than hidden: this development sandbox replaces `rm` with a trash wrapper that exits non-zero when the target is already absent. `packaging/test-install.sh` deletes a file that may not exist, so it aborts under `set -euo pipefail` in this environment. It was confirmed to fail identically on a clean tree before any change, and it passes when `rm` has its normal semantics. This is an environment artifact, not a repository defect, and hosted CI — where `make check` and `release-check` both run — is unaffected.

## 6. Migration and compatibility review

- **Updater**: `src/update.rs` is untouched. Binary-first update, checksum verification, candidate identity check, crates.io `max_stable_version`, and exact-version Cargo fallback for unsupported targets or confirmed 404 are all unchanged.
- **Installers**: `packaging/install.sh` and `packaging/install.ps1` are untouched and byte-identical in the staged draft. URL mapping, 404-only vs unsupported-host fallback, and exit-code behaviour are unchanged, and all three consumer mappings are asserted equal to `release-targets.txt`, which is asserted equal to the contract.
- **`release-targets.txt`**: kept, but reclassified from producer input to frozen *consumer compatibility* table. `packaging/check-contract.sh` now compares it against the contract instead of treating both as coequal.
- **External contract**: all 16 historical public names are byte-identical. The three additions are new files; none renames, replaces, or removes anything a consumer could have been depending on.

## 7. Invariant review

- Authority separation held: Eggpack owns producer facts; eggsearch keeps its consumer validators, wrappers, updater, and policy files.
- Determinism: two renders of the generated workflow are byte-identical.
- Fail-closed validation held: `deny_unknown_fields`, `schema_version == 1`, bounded counts and sizes, exact-match resolution throughout.
- Integrity ≠ authenticity: SHA-256 and sizes are treated as integrity facts only; no provenance signature claim is made anywhere.
- Draft-only staging: proven both when it wrote and when it refused.
- Sole writer: the legacy `release-binaries.yml` writer was deleted in the cutover commit, before the tag was pushed. There was never a period in which two workflows could mutate the same draft.

## 8. Documentation and operations

Updated: `AGENTS.md`, `architecture/packaging.md`, `architecture/overview.md`, `docs/release.md`, `docs/release-checklist.md`, `skills/eggsearch-release/SKILL.md`, `skills/eggsearch-architecture/SKILL.md`, `CHANGELOG.md` (new `0.4.2` section), and the `Makefile` (new `producer-drift` target, deliberately outside `check` so the routine local gate stays network-free and toolchain-free).

New operator surface: `make producer-drift`, the Release drift guard, the required ARMv7 validator, the required per-target validator, and the Release provenance workflow. `packaging/release-inputs.txt` now enumerates the full producer input set and tolerates `#` comments.

## 9. Unresolved findings

| Severity | Finding | Disposition |
|---|---|---|
| Medium | **The release build is not byte-reproducible across workflow attempts.** Attempt 2 of run `37531104901`, on the identical tag and source, passed all 21 per-target jobs plus `required_gate` and `aggregate`, then refused at `stage` with `same-name remote asset digest mismatch`. The rerun therefore proves the no-clobber refusal but does **not** prove byte-identical reuse. | The differing asset and its cause are **not identified**: the staging receipt is written only on success, so a refusal leaves no machine-readable evidence. The obvious suspect — per-attempt `CARGO_TARGET_DIR` — was disproved by inspecting the staged binary (zero occurrences of `runner.temp/eggpack`; stable registry paths). Remaining candidates are a floated hosted toolchain/image between attempts or a non-deterministic link step. Owner: eggsearch, for a build-reproducibility corrective if reuse-on-rerun is wanted. |
| Low | `src/core/error_query.rs` used `.filter(..).next_back()`, denied by current clippy as `filter_next`, blocking the local gate. Pre-existing and unrelated to the cutover. | Fixed in `28a0060` with the compiler's own `.rfind(..)` suggestion, in a separate commit so the release diff stays reviewable. |
| Low | `packaging/test-install.sh` deletes a possibly-absent file and aborts under `set -e` where `rm` is wrapped. | Environment artifact, confirmed on a clean tree before any change; passes with normal `rm` semantics and in CI. |

No critical or high finding remains.

## 10. Roadmap disposition and registry updates

`plans/subsystems/eggpack-release-adoption-roadmap.md` records M001 as conditionally closed with the closure path. `plans/registry.md` records the milestone, the closure record, the implementation and run IDs, and the named condition. The `AGENTS.md` handoff now points at the registry rather than restating release status.

## 11. Not performed, and therefore not claimed

- **The `v0.4.2` draft is not published.** Publication was not authorized here and remains a manual maintainer action.
- No external Unix or Windows installer smoke was run against *published* `v0.4.2` assets, because it is not published.
- `Release provenance` has not been dispatched against `v0.4.2`, because that is the step immediately before publication. Its verifier logic has been proven locally against the real staged draft (§4.8), so the dispatch is expected to pass, but no `v0.4.2` attestation exists and none is claimed.

## 12. Named remaining condition and next handoff

**Condition**: publish eggsearch `v0.4.2` (draft `405157598`). Risk while it stands: none to consumers — `v0.4.1` remains the published release and its assets are byte-identical where they matter. Cost of delay: only that the cutover's first Eggpack-produced release is not yet visible.

**Next steps, in order**:

1. `gh release view v0.4.2 --repo eggstack/eggsearch` and review the 19-asset draft.
2. Dispatch `Release provenance` with `release_tag=v0.4.2`.
3. Publish the draft.
4. Run the external Unix and Windows installer smoke against the published assets.
5. Open a separate build-reproducibility corrective if reuse-on-rerun is wanted (§9, Medium).