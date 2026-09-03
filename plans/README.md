# Eggsearch Planning

This directory contains implementation handoff plans for work that is intentionally not part of the published crate artifact. `Cargo.toml` excludes `plans/` from packaging.

The active planning control surface is:

- `registry.md` — current workstreams, phase status, dependency order, and closure state.
- `roadmap.md` — architectural rationale, research evidence, invariants, and the ordered implementation program.
- `phase-*.md` — bounded implementation plans that should be independently executable and verifiable.

## Status vocabulary

Use `planned`, `in_progress`, `blocked`, `implemented`, or `superseded`. Do not mark a phase `implemented` until its acceptance criteria have been exercised against the exact candidate being handed off.

## Implementation discipline

Plans in this directory are normative for scope, invariants, and acceptance criteria, but the repository code remains the source of truth for exact symbols and line locations. Re-audit the named files against the current `main` head before editing because eggsearch is under active development.

Routine verification follows `AGENTS.md`: `make check` is the broad local gate; normal tests must remain deterministic and network-free. Credentialed or live-provider checks belong behind ignored/live-smoke paths and must never become required CI.

When a phase lands, update `registry.md` and the phase status in the same closure commit. If implementation evidence invalidates a later phase, revise the roadmap before starting that phase rather than silently expanding scope.
