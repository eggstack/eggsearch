# Dependency Evidence Hardening — Overview and Sequencing

Status: sequencing plan

Source roadmap:
`plans/subsystems/dependency-evidence-hardening-roadmap.md`

Applicable ADR:
`plans/adrs/ADR-0004-typed-dependency-evidence-semantics.md`

Baseline: `e0dab301cc04bf5e1e232177e2e2647262a1fdd7`

## Purpose

Coordinate the dependency-parser hardening work so format fixes do not extend
the current ambiguous `version` model and so final fuzz/resource work measures
the completed semantics rather than a moving parser surface.

## Required order

1. Implement M001 first. No format milestone may add new evidence semantics
   before the typed finding/applicability boundary is stable.
2. After M001 closes, M002-M006 may run independently or in parallel. Each must
   use the typed evidence fields and parser diagnostics established by M001.
3. Implement M007 only after all format milestones intended for this
   workstream have closed. M007 is the aggregate budget/adversarial
   qualification gate.
4. Write one closure record per milestone. Do not mark the roadmap closed from
   implementation commits alone.

## Shared implementation rules

- Preserve the current local dependency-file authorization and safe-open path.
- Never execute package managers, build tools, scripts, hooks, or repository
  configuration.
- Prefer existing parsers already in the graph: `toml` and `serde_json`.
- For XML, qualify `quick-xml` with minimal/default-disabled features before
  adding it.
- For YAML-generated formats, do not add deprecated `serde_yaml`. Compare a
  narrow generated-format parser with a maintained YAML implementation and
  record the footprint/maintenance decision.
- Preserve deterministic output and source locations where the source format
  permits them.
- Do not globally normalize package names. Use ecosystem-defined identity
  rules.
- The legacy `DependencyFinding.version` field remains compatibility output;
  it is not the applicability authority after M001.

## Milestone map

| Plan | Scope | Starts when |
|---|---|---|
| 001 | typed evidence + applicability trust | immediately |
| 002 | Cargo, Go, Python, Windows/Unix dispatch | M001 closed |
| 003 | NuGet/.NET + Maven/Gradle | M001 closed |
| 004 | Bundler + Composer | M001 closed |
| 005 | npm + Yarn + pnpm | M001 closed |
| 006 | GitHub Actions + OCI/Docker | M001 closed |
| 007 | budgets + diagnostics + property/fuzz/corpus + qualification | M001-M006 closed |

## Closure evidence shared by all milestones

Each closure record must state:

- exact implementation SHA;
- focused tests added and commands run;
- canonical gate outcome where required;
- before/after examples for every corrected false-positive/false-negative path;
- any format version intentionally unsupported;
- compatibility effect on serialized dependency findings;
- residual risk and whether another corrective plan is required.

## Stop conditions

Write a corrective plan rather than widening a milestone if:

- a format requires package-manager execution to determine the truth;
- supporting a syntax requires evaluating arbitrary project code;
- compatibility would require silently preserving an unsafe applicability
  interpretation;
- a proposed parser dependency materially expands binary/dependency footprint
  without measured maintenance benefit;
- a parser cannot distinguish unsupported syntax from a valid empty result;
- a fix requires changing the ten-tool MCP surface.
