# Plan 002 — CI and Release Supply-Chain Provenance

Status: conditionally closed

Closure record: `plans/closure/repository-hardening/002-status.md`. First
tagged-release attestation and immutability evidence remains an operational
condition.

Source roadmap:
`plans/subsystems/repository-hardening-roadmap.md`

Milestone: M002 CI and release supply-chain provenance

Primary class: infrastructure

Planning baseline: `c49c600b76e690bb1bc52f641554cca6bf79f36f`

Hard dependencies: none.

Relevant long-term requirements:

- `plans/000-long-term-specification.md#6`
- `plans/000-long-term-specification.md#9`

## Objective

Reduce workflow and binary-publication supply-chain risk by pinning executable
GitHub Actions to immutable commit SHAs, enforcing that policy mechanically,
and adding cryptographically verifiable provenance for release artifacts while
preserving the existing seven-target / 16-release-asset contract and manual
publish gate.

## Current evidence

Current workflows reference external Actions by movable tags, including
`actions/checkout@v4`, `actions/setup-python@v5`,
`actions/upload-artifact@v4`, `actions/download-artifact@v4`,
`docker/setup-qemu-action@v3`, `mlugg/setup-zig@v2`, and
`dtolnay/rust-toolchain@stable`.

GitHub's secure-use guidance states that pinning to a full-length commit SHA is
the immutable way to reference an Action. The release assembly job already uses
a narrow `contents: write` permission and produces a draft release only after
all target jobs succeed.

Current GitHub functionality can generate artifact attestations backed by
Sigstore/OIDC and can enforce immutable future releases so published tags/assets
cannot be altered.

Research references:

- https://docs.github.com/en/actions/reference/security/secure-use
- https://docs.github.com/en/actions/concepts/security/artifact-attestations
- https://docs.github.com/en/actions/how-tos/secure-your-work/use-artifact-attestations/use-artifact-attestations
- https://docs.github.com/en/code-security/concepts/supply-chain-security/immutable-releases
- https://docs.github.com/en/code-security/how-tos/secure-your-supply-chain/establish-provenance-and-integrity/prevent-release-changes

## Invariants

- Qualification mode never publishes or mutates a GitHub Release.
- Release mode continues to require the exact tag/package-version/commit match.
- The published release remains exactly the documented seven binaries, seven
  checksum files, and two installers unless a separate distribution plan changes
  that contract.
- Publication remains manual after draft assembly/review.
- Build jobs retain read-only repository permission.
- Provenance generation failure blocks a release candidate; it is not warning-only.
- Existing checksum files remain required; attestations supplement rather than
  replace the checksum/update compatibility contract.

## Non-goals

- Changing release targets or binary feature composition.
- Automatic release publication.
- Signing Git commits/tags as a mandatory contributor policy.
- Replacing GitHub Releases.
- Implementing in-process Sigstore verification in `eggsearch update`.
- Generating or publishing a new SBOM asset unless it can be done without
  changing the release-asset contract and is separately justified.
- Organization-wide Actions policy changes outside this repository.

## Required work

### 1. Pin every external Action to a full commit SHA

Inventory every `.github/workflows/*.yml` and `.yaml` `uses:` entry.

For each external Action:

1. resolve the currently intended version/tag to a commit owned by the expected
   upstream repository;
2. review the relevant release/changelog and source at that commit;
3. replace the tag with the full 40-hex commit SHA;
4. retain the human-readable version in adjacent YAML text so upgrades remain
   understandable.

Local actions/workflows referenced by repository path are exempt.

Do not pin to a shortened SHA.

### 2. Enforce action pinning in repository checks

Add a deterministic workflow-policy checker under the existing hygiene or
packaging contract. It must inspect all workflow files and reject external
`uses:` references that are not full 40-hex SHAs.

The checker must handle:

- normal action steps;
- reusable workflow `uses:` entries;
- local `./...` references;
- comments/whitespace without false acceptance.

Wire it into `make check` so a later feature cannot silently restore
`@v4`/branch/tag references.

### 3. Reconfirm least privilege per job

Keep workflow-level permissions read-only where possible. The release assembly
job may have the minimum extra permissions needed to:

- create/update the draft release (`contents: write`);
- obtain OIDC identity for provenance (`id-token: write`);
- write artifact attestations (`attestations: write`).

Do not grant those permissions to matrix build jobs or ordinary CI.

Review `pull_request` workflows specifically to ensure untrusted PR code never
receives release/write credentials.

### 4. Generate provenance attestations for release artifacts

In release mode only, after the assembled asset set has passed
`release-validate.sh assets`, generate GitHub artifact attestations for the
software users execute/download:

- seven release binaries;
- `install.sh`;
- `install.ps1`.

Checksums may be included in the attested subject glob if convenient, but
attestation metadata must not add files to the 16-asset GitHub Release contract.

Use the current `actions/attest` release pinned to a reviewed full commit SHA.
The attestation subject must bind to the exact candidate SHA/tag already
established by preflight.

Qualification-only artifacts should not generate public release attestations.

### 5. Add provenance verification to release qualification

Before a draft is considered publication-ready, exercise GitHub's supported
attestation verification path against at least one artifact from the exact
release-mode run and record the repository/workflow/commit identity observed.

The release summary should state:

- candidate commit;
- release tag/version;
- artifact-set validation;
- attestation generation result;
- the command/documented path maintainers can use to verify the artifacts.

Do not make `gh` an eggsearch runtime dependency.

### 6. Enable immutable releases operationally

Enable GitHub "release immutability" for the repository. This setting affects
future releases only.

Because this is a repository setting rather than source code, closure evidence
must record either:

- the setting is enabled and an API/UI verification was captured; or
- source changes are complete but the milestone remains conditionally closed
  with the single named operational blocker.

The first release after enablement should confirm that the published release is
reported immutable and that the tag/assets cannot be replaced.

### 7. Documentation

Update release/security documentation to explain:

- full-SHA workflow pinning policy and upgrade procedure;
- why checksums and provenance serve different purposes;
- how maintainers/users verify an artifact attestation;
- immutable-release behavior and the draft-before-publish requirement.

Do not describe attestations as proving that the artifact is vulnerability-free;
they prove provenance/integrity claims about the build.

## Failure and recovery semantics

- If an action tag cannot be resolved to a trusted upstream commit, stop using
  that action rather than pinning an unknown fork.
- Attestation service/OIDC failure blocks release mode but should not make normal
  PR CI unusable.
- A failed release matrix still produces no published release.
- Immutable release enablement must occur only after confirming the existing
  draft workflow can attach all assets before publication.

## Focused verification

At minimum:

```bash
make check
make packaging-check
./packaging/release-validate.sh candidate
# run the release-binaries workflow in qualification mode on the exact SHA
# run a release-mode rehearsal according to repository release policy
# verify generated attestations with GitHub's supported verifier
```

Also run the workflow pin checker directly against every workflow file and add
negative fixtures/cases for `@v4`, branches, short SHAs, and local paths.

## Acceptance criteria

- Every external Action/reusable workflow is pinned to a reviewed full SHA.
- A required repository guard prevents movable external Action refs.
- Ordinary CI/build jobs remain least-privilege.
- Release-mode artifacts receive verifiable provenance attestations bound to the
  exact repository/workflow/commit.
- The seven-target/16-asset release contract is unchanged.
- Qualification mode remains non-publishing.
- Future release immutability is enabled, or the implementation is explicitly
  conditionally closed on that one operational setting.
- Installer/updater behavior remains backward-compatible.

## Stop conditions

Stop if:

- provenance requires widening permissions outside the release assembly boundary;
- the attestation step would require changing the public asset contract merely
  to function;
- immutable release behavior conflicts with the existing draft/manual-publish
  flow;
- any workflow pin resolves to a fork or unverifiable source commit.

## Closure evidence required

Create `plans/closure/repository-hardening/002-status.md` containing:

- old tag refs -> pinned commit mappings;
- workflow-policy negative/positive test evidence;
- exact workflow candidate and run IDs;
- permission matrix before/after;
- provenance verification output for an exact artifact;
- immutable-release setting evidence or named operational blocker;
- confirmation that the release target/asset contract did not change.
