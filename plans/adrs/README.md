# Architecture Decision Records

This directory contains durable decisions affecting eggsearch architecture
across milestones or subsystems. Use an ADR when a question cannot be
answered safely inside one implementation plan without establishing a
reusable architectural contract.

## Naming

```text
ADR-NNNN-short-title.md
```

Numbers are monotonically increasing and never reused.

## Status lifecycle

```text
proposed -> accepted -> deprecated or superseded
          `-> rejected
```

Accepted ADRs are historical records. Do not rewrite an accepted ADR to make
a later decision appear original. Create a new ADR and mark the old one
superseded.

## Threshold

An ADR is normally required when a decision changes the ten-tool contract or
ownership boundary, introduces a new MCP/forge/storage protocol, selects a
durable transport dependency, changes trust or authorization semantics,
changes retry/circuit ownership, establishes a public compatibility contract,
or materially changes a long-term non-goal. An ADR is usually unnecessary for
local refactors, internal naming cleanup, or reversible optimizations
preserving established contracts.
