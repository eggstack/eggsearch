# Repository Hardening — Overview and Sequencing

Status: blocked; M001/M003-M005 closed, M002 conditionally closed, M006 externally blocked

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
| M001 | `001-dependency-security-and-policy-gate.md` | closed | none |
| M002 | `002-ci-release-supply-chain-provenance.md` | conditionally closed | none |
| M003 | `003-rmcp-http-verification-on-eggfetch.md` | closed | none |
| M004 | `004-process-and-unsafe-boundary-hardening.md` | closed | none |
| M005 | `005-forge-safety-maintenance-decomposition.md` | closed | M004 (closed) |
| M006 | `006-chromiumoxide-zero-reqwest-closure.md` | blocked | M003 + upstream chromiumoxide release |

## Execution deviation

M001-M005 were implemented in one coordinated source candidate because the
dependency policy gate, package manifests, transport dependency shape, shared
process runner, and static guards cross the planned boundaries. Separate
requirement matrices and closure records preserve milestone-specific evidence.
The follow-up commit only makes the Cargo tree guard independent of CI color.

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
