# Eggpack Release Adoption Roadmap

Status: active

Long-term references:

- `plans/000-long-term-specification.md#6`
- `plans/001-terminology-and-domain-model.md#6`
- `plans/002-long-term-roadmap.md`
- `plans/003-planning-process.md`

Cross-repository producer roadmap:

- `eggstack/eggpack: plans/subsystems/ecosystem-adoption-roadmap.md`

## 1. Purpose and ownership boundary

Adopt Eggpack as Eggsearch's producer-side release authority while preserving Eggsearch-owned runtime, install/update, service, provenance, and publication policy.

This workstream is new rather than an extension of the closed historical Binary Distribution and Deployment roadmap. The prior roadmap remains immutable closure evidence for the hand-maintained release system being replaced.

Eggpack owns producer contract/build/qualification/finalization/staging facts. Eggsearch retains product policy and the consumer-facing release/install/update behavior.

## 2. Work classification

### Invariants

- seven required binary targets remain supported;
- current public binary names remain unchanged;
- glibc 2.17 remains the maximum GNU symbol requirement for the three Linux GNU binaries;
- ARMv7 retains real runtime execution evidence;
- Windows ARM64 remains required;
- product wrappers retain latest/exact selection and Cargo fallback semantics;
- binary-first self-update semantics remain unchanged;
- GitHub Artifact Attestation coverage is not reduced;
- published releases remain immutable and manually published;
- only one workflow owns build/staging release writes after cutover;
- Eggpack generated jobs never gain provenance OIDC permissions.

### Capability

- one checked-in Eggpack producer configuration drives the seven-target release;
- generated workflow is drift-checked;
- Eggpack stages exact draft assets with no-clobber semantics;
- product-owned provenance attests the exact staged bytes;
- the first public Eggpack-produced Eggsearch release preserves install/update behavior.

### Infrastructure

- `release/eggpack/` contract/config/bindings/policies;
- ARMv7 required consumer validator;
- product-owned provenance workflow;
- packaging/static-guard updates for generated authority.

### Polish

- documentation and architecture status reconciliation after live cutover;
- optional later cleanup of duplicate consumer compatibility tables only when it does not weaken runtime independence.

## 3. Non-goals

- moving self-update logic into Eggpack;
- moving crates.io/tag/version policy into Eggpack;
- generalized producer provenance in Eggpack;
- changing the seven public binary names;
- dropping ARMv7 or Windows ARM64;
- automatic publication;
- changing search/MCP behavior;
- using Eggpack as a runtime dependency.

## 4. Current state

The hand-maintained release system is closed historical evidence and successfully published immutable `v0.4.1` with seven binaries, seven checksum sidecars, two public installers, and Artifact Attestations.

Eggpack Ecosystem M003a has now completed the compatibility preflight and found no producer prerequisite:

- all seven targets render through current Eggpack;
- Windows ARM64 maps through `windows-11-arm`;
- Zig 0.13.0 may migrate to the Eggpack-qualified 0.14.1/0.23.3 pair, with post-build glibc re-proof;
- ARMv7 uses Structural core qualification plus a required Eggsearch-owned validator;
- provenance stays in a separate read-only Eggsearch workflow;
- Eggpack no-clobber staging replaces the current `--clobber` writer.

The paired implementation plans are:

- Eggpack: `plans/implementation/ecosystem-adoption/003b-eggsearch-seven-target-eggpack-producer-cutover.md`;
- Eggsearch: `plans/implementation/eggpack-release-adoption/001-seven-target-eggpack-producer-cutover.md`.

## 5. Release-contract transition

Historical public release inventory: exactly 16 files.

Post-cutover inventory: exactly 19 files:

- existing 7 binaries;
- existing 7 checksum sidecars;
- existing `install.sh` + `install.ps1`;
- additive `release-manifest.json`;
- additive `install-exact.sh`;
- additive `install-exact.ps1`.

The three additions are producer evidence and qualified exact-bootstrap surfaces. They do not replace the two public product wrappers.

This additive change must be explicit in packaging guards, release docs, and closure evidence.

## 6. Dependency graph

```text
historical binary distribution M001-M006 [CLOSED]
                 |
                 +--> public v0.4.1 release + provenance [CLOSED]
                 |
                 v
Eggpack Ecosystem M003a compatibility preflight [CLOSED]
                 |
                 v
Eggpack Release Adoption M001 [CONDITIONALLY CLOSED]
                 |
                 +--> Eggpack Ecosystem M003b closure
                 |
                 `--> first public Eggpack-produced Eggsearch release
```

Hard dependency: Eggpack M003a closure.

Interface dependency: paired Eggpack M003b plan, registered.

Operational closure dependency: maintainer-authorized live release evidence unless explicitly conditionally closed with a named publication condition.

## 7. Milestones

### M001 — Seven-target Eggpack producer cutover

Class: capability / cross-repository adoption

Implementation plan:

`plans/implementation/eggpack-release-adoption/001-seven-target-eggpack-producer-cutover.md`

Status: conditionally closed. Closure record: `plans/closure/eggpack-release-adoption/001-status.md`.

Outcome: the cutover landed at `eabbf80` and hosted run `37531104901` staged eggsearch `v0.4.2` as draft `405157598` with the exact 19-asset inventory: all seven targets built, qualified, and consumer-validated (including the required ARMv7 runtime proof and Windows ARM64 on `windows-11-arm`), glibc 2.17 re-proven on all three GNU targets after the Zig 0.14.1 / cargo-zigbuild 0.23.3 bump, provenance moved to a separate read-only workflow with no OIDC in any generated job, `--clobber` replaced by fail-closed staging, and the legacy writer deleted so exactly one writer exists.

Named condition: publishing the `v0.4.2` draft is a manual maintainer action that was not authorized. See the closure §12 for the ordered next steps.

## 8. Verification strategy

- `make check`;
- `make release-check`;
- `eggpack ci generate` + `eggpack ci check`;
- seven-target hosted qualification on exact candidate;
- glibc 2.17 proof on all GNU binaries;
- ARMv7 runtime validator;
- Windows ARM64 hosted evidence;
- exact 19-file draft inventory;
- no-clobber rerun proof;
- provenance subject parity;
- public installer/updater smoke on the first live release.

## 9. Risks and decision points

- toolchain bump may violate the glibc floor;
- ARMv7 runtime proof may require a capability not available in the bounded validator environment;
- provenance API permissions may prevent subject-parity readback;
- existing packaging guards assume 16 files and must be intentionally migrated to 19;
- retaining the old release writer too long creates dual authority.

Each is a stop condition in M001 rather than permission to weaken the release contract.

## 10. Completion definition

This subsystem closes when Eggsearch has one Eggpack-owned producer release authority, the old hand-maintained release writer is retired, a real release preserves the seven-target/install/update/provenance contract, and the paired Eggpack Ecosystem M003b closure accepts the evidence.

## 11. Milestone status

| Milestone | Status | Implementation plan | Closure record | Blockers |
|---|---|---|---|---|
| M001 seven-target Eggpack producer cutover | conditionally closed | `plans/implementation/eggpack-release-adoption/001-seven-target-eggpack-producer-cutover.md` | `plans/closure/eggpack-release-adoption/001-status.md` | Implementation `eabbf80`; hosted run `37531104901` staged `v0.4.2` as draft `405157598`, 19 assets, all seven targets green. Named condition: publication is manual. Open Medium: release build is not byte-reproducible across attempts, so a rerun refuses instead of reusing |
