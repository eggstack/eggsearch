# Transport Consolidation Roadmap

Status: closed

Long-term references:

- `plans/000-long-term-specification.md#4`
- `plans/001-terminology-and-domain-model.md#4`
- `plans/002-long-term-roadmap.md`

Related ADRs:

- `plans/adrs/ADR-0002-eggfetch-transport-ownership.md`

## 1. Purpose and ownership boundary

Migrate eggsearch-owned outbound HTTP to `eggfetch-core` while preserving the
rmcp-owned Streamable HTTP client boundary and the SSRF/retry/truncation
policy ownership in eggsearch. Owns `FetchClient`, origin control
integration, redirect validation, and the HTML-engine encoding posture. Does
not own provider ranking or evidence semantics.

## 2. Work classification

### Invariants

- One shared eggfetch client; per-hop pinned resolved snapshots; total
  deadlines cover body streaming; `OriginController` sole retry authority.
- Bounded eggfetch feature graph; no local decompression stack, direct
  reqwest, or unpublished pin.

### Capabilities

- None user-visible; this is infrastructure qualification.

### Infrastructure

- eggfetch 0.1.7 adoption; 0.2.0 adoption with compression-workaround
  retirement; chunked gzip/Brotli proof through the production bounded-body path.

### Polish

- Dependency/size deltas recorded; current-state docs reconciled.

## 3. Non-goals

Moving SSRF/retry/truncation policy into the transport library; replacing the
rmcp Streamable HTTP client with a local adapter; broad redirect-policy
redesign.

## 4. Current state

Closed. Phase 17 implementation
(`5a739a3cc6cdd060911eeefad7b004661f4b5c94`), phase 18 qualification
(candidate `f9a661886376dd20c1539e20f990c43115ffdb90`, run `35427685324`),
and phase 24 adoption (candidate `bac6f49fc046a93d7c094a8f96e1027631f390f1`,
`eggfetch-core 0.2.0`, run `35692096012`). Upstream
`eggstack/eggfetch#24` consumed; six `.decompress(false)` workarounds removed
with downstream proof.

## 5. Target architecture

Achieved: all eggsearch-owned HTTP behind eggfetch with the phase-17
ownership boundary and phase-23 timeout/client-reuse semantics intact.

## 6. Dependency graph

```text
M001 eggfetch 0.1.7 consolidation (hard)
    |
    `--> M002 migration qualification + compression closure (hard, operational: seven-target qualify)
              |
              `--> M003 eggfetch 0.2.0 adoption + workaround retirement (hard)
```

Qualification is SHA-specific throughout.

## 7. Milestones

### M001 — eggfetch 0.1.7 HTTP transport consolidation

Historical plan:
`plans/archive/phase-17-eggfetch-0.1.7-http-transport-consolidation.md`.

### M002 — Transport migration qualification and upstream compression closure

Historical plan:
`plans/archive/phase-18-transport-migration-qualification-and-upstream-compression-closure.md`.

### M003 — eggfetch 0.2.0 adoption and compression-workaround retirement

Historical plan:
`plans/archive/phase-24-eggfetch-0.2.0-adoption-and-compression-workaround-retirement.md`.

## 8. Cross-cutting requirements

Updater/provider redirect bounds with downgrade denial; decoded-body limits
and compressed-response deadlines enforced; DuckDuckGo/Startpage live smoke
attempted and classified where reachable.

## 9. Verification strategy

Deterministic chunked gzip/Brotli + transfer-shape + limit + deadline
regressions in `provider_request_contract` (21/21), local gates, and exact-
candidate seven-target qualification.

## 10. Risks and decision points

None open. Any later production/dependency change invalidating the exact
candidate requires separate qualification.

## 11. Completion definition

Closed when crates.io-resolved `eggfetch-core 0.2.0` carries all
eggsearch-owned HTTP with the bounded feature selection and the 0.2.0
candidate passes seven-target qualify plus exact assembly.

## 12. Milestone status

| Milestone | Status | Implementation plan | Closure record | Blockers |
|---|---|---|---|---|
| M001 | closed | `plans/archive/phase-17-eggfetch-0.1.7-http-transport-consolidation.md` | `plans/closure/transport-consolidation/001-status.md` | — |
| M002 | closed | `plans/archive/phase-18-transport-migration-qualification-and-upstream-compression-closure.md` | `plans/closure/transport-consolidation/001-status.md` | — |
| M003 | closed | `plans/archive/phase-24-eggfetch-0.2.0-adoption-and-compression-workaround-retirement.md` | `plans/closure/transport-consolidation/001-status.md` | — |
