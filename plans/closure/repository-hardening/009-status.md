# Repository Hardening M009 — First Tagged-Release Provenance Reconciliation

Status: closed
Recommendation: closed
Implementation commit: `cf1e26afda19c4086ebd30add46bd78649e92323`
Reconciled milestone: M002 CI and release supply-chain provenance
Plan: `plans/implementation/repository-hardening/009-first-tagged-release-provenance-reconciliation.md`

## Decision

M002 is closed. Its only remaining condition was first-tagged-release
attestation verification and confirmation of release immutability. That
condition is met by the immutable `v0.4.1` release and evidence already
preserved in M002's closure and CodeGG M005 closure. This record reconciles
status only; it does not rewrite the original implementation results or mutate
the published release.

## Evidence audit

| Claim | Evidence | Result |
|---|---|---|
| Exact release source identity | Tag `v0.4.1` points to fix-prep commit `32769d684417567dfb30fc5adea1475cee7f0ed0`; release run `37072825644` consumed that tag | Pass |
| Release job and artifacts succeeded | `37072825644` completed successfully across assembly and all nine release steps; the published release has the 16-asset contract | Pass |
| Attestations cover the release subjects | Release workflow generated SLSA provenance for all 16 release subjects; sampled Linux and Windows binaries and installer verified with `gh attestation verify` exit 0 | Pass |
| Attestation binds to repository/workflow/commit | Verifier output binds to `eggstack/eggsearch`, `release-binaries.yml@refs/tags/v0.4.1`, commit `32769d6` | Pass |
| Release immutability is effective | Published release reports `draft=false`, publication time `2026-10-02T22:49:33Z`, and immutable 16-asset inventory; repository immutability setting was API-verified enabled | Pass |
| Historical evidence is traceable | M002 original closure § First-release evidence note and CodeGG M005 closure §2 record the same tag, run, subject count, and sampled verifier identities | Pass |

The release evidence predates the current Eggpack producer and its 19-asset
contract. This reconciliation closes the M002 criterion for the first tagged
release; it does not claim that v0.4.1 used the current producer or replace the
separate Eggpack M001 evidence for v0.4.2.

## Status synchronization

- M002 implementation plan and closure recommendation say closed by M009.
- Repository-hardening roadmap, sequencing overview, and registry agree.
- `packaging/check-planning-consistency.py` expects M002 closed while
  preserving M001/M007 conditional and M006 blocked states.
- M002's original closure record retains the initial conditional decision and
  appends the later reconciliation; no release evidence was overwritten.

## Verification

| Command | Result |
|---|---|
| `python3 packaging/check-planning-consistency.py` | Pass |
| `cargo test --locked --test static_guards planning_status_consistency_no_closed_range_shorthand -- --exact` | 1 passed |
| `make check` | Pass; 3,474 tests plus formatting, Clippy, no-default check, hygiene, dependency policy, and packaging checks |
| `git diff --check` | Pass |

M001/M007 remain conditional pending a successful scheduled dependency-security
run. M006 remains blocked on an upstream chromiumoxide release that makes HTTP
discovery optional without regressing browser launch.
