# Repository Hardening M010 — Scheduled Dependency-Security Evidence Reconciliation

Status: closed
Recommendation: closed
Implementation commit: `31175e2f2ebb466b501721ce865948ede1670e58`
Reconciled milestones: M001 dependency security/policy; M007 corrective closure evidence and process ratchet
Plan: `plans/implementation/repository-hardening/010-scheduled-security-evidence-reconciliation.md`

## Decision

M001 and M007 are closed. The operational condition named by M007 was the
first successful GitHub Actions run of `.github/workflows/dependency-security.yml`
with event `schedule`. The recorded run meets that condition. The original
implementation and conditional-closure records remain intact with this later
operational evidence appended.

## Scheduled run evidence

| Field | Evidence |
|---|---|
| Workflow | `Dependency security policy` (`.github/workflows/dependency-security.yml`) |
| Run | [`38290328560`](https://github.com/eggstack/eggsearch/actions/runs/38290328560) |
| Event | `schedule` |
| Created | 2026-10-12 18:21:13 UTC |
| Head | `90e996ef6c83c969e30c30a36bc9d968a85dc00f` |
| Job | `cargo-deny`, job `114948595069`, completed success at 18:23:20 UTC |
| Policy summary | `advisories ok, bans ok, licenses ok, sources ok` |

`gh run view` confirms the run event, head SHA, completion outcome, and job
outcome. The job log confirms the cargo-deny summary. This is the required
scheduled event; prior `workflow_dispatch` wiring evidence is not substituted.

## Status synchronization

- M001 is closed via M007/M010; its original closure links this run.
- M007 is closed by M010; its closure records this run and the satisfied
  operational condition.
- The repository-hardening roadmap, sequencing overview, active registry, and
  planning consistency checker agree. M002 is closed by M009; M006 remains
  blocked on upstream chromiumoxide.
- The static guard still rejects closed-range shorthand.

## Verification

| Command | Result |
|---|---|
| `python3 packaging/check-planning-consistency.py` | Pass |
| `cargo test --locked --test static_guards` | 60 passed |
| `make packaging-check` | Pass: workflow pins, planning consistency, egress qualification contract |
| `make dependency-policy` | Pass: advisories, bans, licenses, and sources |
| `make check` | Attempted twice; both full parallel runs reached the unit suite and failed two unrelated updater candidate-execution tests under concurrent load. Each failing updater test passed alone. A serial full run was interrupted after stalling in the bounded-command suite. `make check` was green on the prepared 0.4.3 candidate before M010's planning-only changes. |
| `git diff --check` | Pass |

M010 changed planning status records and their consistency guard/test only; it
did not change production runtime code. The full-suite timing issue is
recorded rather than presented as a pass. No High/Medium finding was
introduced by this reconciliation.
