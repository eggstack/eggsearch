# Repository Hardening M002 — CI and Release Supply-Chain Provenance

Status: conditionally closed

Recommendation: conditionally closed

Implementation commits: `cb4048180e29cdeaffb77f2cc86eaec2a8e33b7b`, `85322ac127033bdc4058177ea09cff67417308d0`

## Requirement evidence

| Requirement | Evidence | Result |
|---|---|---|
| Pin external Actions to immutable SHAs | Every external workflow `uses:` entry carries a full 40-character commit SHA and human-readable version; versions and SHAs are listed below | pass |
| Enforce pinning mechanically | `packaging/check-workflow-pins.py` handles actions, reusable workflows, comments, local paths, and negative fixtures; wired into `make check` through packaging-check | pass |
| Preserve least privilege and manual publication | Workflow-wide permission is read-only; release assembly alone receives release/OIDC/attestation permissions; release stays draft until manual publication | pass |
| Generate and verify release artifact provenance | Release-only `actions/attest` covers the seven binaries and two installers; release job runs `gh attestation verify` and writes the verifier output to the run summary | implemented; first tagged-release evidence pending |
| Keep release contract and qualification behavior | Asset validation still requires 16 files; qualification does not attest or alter a GitHub Release | pass |
| Enable future release immutability | `gh api repos/eggstack/eggsearch/immutable-releases` returned `enabled: true`, `enforced_by_owner: false` | pass |

## Action references

| Action | Version label | Commit |
|---|---|---|
| `actions/checkout` | v4.2.2 | `11bd71901bbe5b1630ceea73d27597364c9af683` |
| `actions/setup-python` | v5.6.0 | `a26af69be951a213d495a4c3e4e4022e16d87065` |
| `actions/upload-artifact` | v4.6.2 | `ea165f8d65b6e75b540449e92b4886f43607fa02` |
| `actions/download-artifact` | v4.3.0 | `d3f86a106a0bac45b974a628896c90dbdf5c8093` |
| `actions/attest` | v3.0.0 | `daf44fb950173508f38bd2406030372c1d1162b1` |
| `docker/setup-qemu-action` | v3.6.0 | `29109295f81e9208d7d86ff1c6c12d2833863392` |
| `mlugg/setup-zig` | v2.2.1 | `d1434d08867e3ee9daa34448df10607b98908d29` |
| `dtolnay/rust-toolchain` | stable snapshot 2026-09-28 | `6bed0761d98439e5a578e2877258200ad565ba87` |

## Verification and operational condition

- `python3 packaging/check-workflow-pins.py`: pass across all four workflow files.
- `make check`, `make packaging-check`, `make docs-check`: pass.
- `make release-check`: pass, including release build and publish dry-run.
- Exact-candidate GitHub CI passed: run [`36467672672`](https://github.com/eggstack/eggsearch/actions/runs/36467672672).
- Non-publishing SHA-specific release qualification passed across all eight targets: run [`36467682962`](https://github.com/eggstack/eggsearch/actions/runs/36467682962). Qualification did not attest or publish artifacts; first tagged-release evidence remains pending.
- Repository immutability setting is enabled and API-verified; existing published releases are unchanged.
- This change does not publish a new eggsearch version. No release-mode attestation was generated in this implementation run. The first future tagged release must verify the attestation subject repository/workflow/commit and confirm that the published release is immutable.

That first tagged-release verification is the single named operational
condition for full closure. Checksums remain required and continue serving
download integrity; attestations make build provenance verifiable and do not
claim vulnerability freedom.
