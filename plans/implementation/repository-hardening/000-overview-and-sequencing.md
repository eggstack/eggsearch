# Repository Hardening — Overview and Sequencing

Status: active; M008 corrective ready, M007 conditionally closed, M001 conditionally closed via M007, M002 conditionally closed, M006 blocked

Source roadmap:
`plans/subsystems/repository-hardening-roadmap.md`

Planning baseline: `c49c600b76e690bb1bc52f641554cca6bf79f36f`

## Purpose

This file is the compact handoff sequence for the repository-hardening
workstream. Each numbered plan is independently executable. Do not collapse the
plans into one large change: dependency updates, CI trust, transport feature
changes, process execution, and module decomposition have different failure
modes and should produce separate closure evidence.

## Sequence

| Milestone | Plan | Current status | Hard dependency |
|---|---|---|---|
| M001 | `001-dependency-security-and-policy-gate.md` | conditionally closed via M007 | none |
| M002 | `002-ci-release-supply-chain-provenance.md` | conditionally closed | none |
| M003 | `003-rmcp-http-verification-on-eggfetch.md` | closed | none |
| M004 | `004-process-and-unsafe-boundary-hardening.md` | closed through M007 | none |
| M005 | `005-forge-safety-maintenance-decomposition.md` | closed | M004 (closed) |
| M006 | `006-chromiumoxide-zero-reqwest-closure.md` | blocked | M003 + upstream chromiumoxide release |
| M007 | `007-corrective-closure-evidence-and-process-ratchet.md` | conditionally closed | none; full closure operationally depends on first successful `schedule` dependency-security run; see `plans/closure/repository-hardening/007-status.md` |
| M008 | `008-windows-startup-cross-target-portability-corrective.md` | ready | none; hard gate for next tagged release |

## Execution deviation

M001-M005 were implemented in one coordinated source candidate because the
dependency policy gate, package manifests, transport dependency shape, shared
process runner, and static guards cross the planned boundaries. Separate
requirement matrices and closure records preserve milestone-specific evidence.
The follow-up commit only makes the Cargo tree guard independent of CI color.

Post-closure review found two acceptance-evidence defects: M001's scheduled
dependency-security workflow had not actually executed, and M004's static
process/unsafe ratchet was scoped to known migrated files rather than the full
production source tree. M007 corrects those gaps without rewriting the original
closure records.

M001-M004 may be implemented in parallel on separate branches, but their
eventual merge candidates must be rebased/requalified if they touch a shared
manifest, lockfile, CI guard, or maintenance document.

## Merge-order guidance

Prefer M001 first because it establishes the dependency-security gate that will
evaluate subsequent lockfile changes. M002 is logically independent and can
land before or after M001. M003 should land before M006 and should not wait for
chromiumoxide. M004 should land before M005 so forge decomposition does not
happen concurrently with process-helper movement.

A practical serial order is:

```text
M001 -> M002 -> M003 -> M004 -> M005
                         |
                         `---- M006 when upstream dependency becomes available
```

## Shared invariants

Every handoff must preserve:

- ten stable MCP tools and CLI behavior;
- CodeGG wire compatibility;
- default feature behavior and optional browser/PDF/egress capability;
- eggfetch ownership of eggsearch HTTP transport;
- loopback-only Streamable HTTP;
- existing SSRF/redirect/body/deadline policy;
- release target and 16-asset contract;
- exact-candidate closure evidence.

## Closure rule

Each milestone receives its own closure record under:

`plans/closure/repository-hardening/NNN-status.md`

Do not mark a later milestone closed based on a prior milestone's CI run.
M006 remains blocked until an upstream artifact can satisfy its explicit
dependency gate; do not substitute a permanent local fork merely to clear the
registry.

M007 may proceed immediately. If its code/guard corrections land before the
first real scheduled dependency-security event, close M007 conditionally and
retain that single operational condition; a workflow_dispatch run is wiring
evidence, not scheduled-run evidence.


## Current portability corrective

M008 is the current dependency-ready corrective handoff. Routine Linux CI is
green on baseline `a5f1b0dcd89d90d8ff4fb3bc264bdfcace7acb19`, but cross-target run `36979619135` fails both
Windows release targets because `process_record_contents()` calls a
`process_start_token` helper that is only defined for Linux/macOS. Do not cut
v0.4.0 until M008 closes with green ordinary Windows CI, seven-target egress
qualification, and release-binaries qualification.
