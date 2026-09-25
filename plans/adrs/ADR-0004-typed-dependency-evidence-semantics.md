# ADR-0004: Typed Dependency Evidence and Applicability Trust Semantics

Status: accepted

Date: 2026-09-25

Decision owners: project maintainers

Related specification sections:

- `plans/000-long-term-specification.md#3`
- `plans/000-long-term-specification.md#4`
- `plans/000-long-term-specification.md#7`
- `plans/000-long-term-specification.md#9`

Affected subsystem roadmaps:

- `plans/subsystems/dependency-evidence-hardening-roadmap.md`

## Context

Eggsearch parses dependency manifests, lockfiles, workflow files, and container
references into `DependencyFinding` records that can drive
`security_search` applicability assessment. The current representation has one
optional `version` string plus source/confidence metadata. That is insufficient
for ecosystems that distinguish a declared constraint from a selected version,
or where the same package identity can resolve from a registry, VCS checkout,
workspace/path replacement, container digest, mutable branch, or other source.

The current applicability path also treats any non-empty finding version as
eligible for advisory range evaluation and computes assessment confidence from
the advisory-range side without bounding it by the dependency evidence. This
can promote manifest constraints or checksum observations into stronger claims
than the source establishes.

## Decision drivers

- Security applicability must not turn a requirement, checksum observation, or
  mutable reference into proof of an installed resolved version.
- Existing `security_search` response compatibility should be preserved
  additively where possible.
- Parser output must retain enough provenance and target context to avoid
  registry/package identity false matches.
- Parsing remains passive and bounded. Eggsearch must not execute package
  managers, build scripts, repository configuration, or workspace code.
- The solution must remain deterministic and compatible with eggsearch's
  application-first, lightweight dependency policy.

## Considered options

### Option A — Keep `version` and improve per-format heuristics

Each parser continues emitting one version-shaped string and downstream code
decides how much to trust it based on filename and confidence.

This has low migration cost but preserves the core ambiguity. Every new format
must rediscover which strings are selections, requirements, references, or
integrity metadata, and downstream consumers can regress by forgetting the
filename-specific rule.

### Option B — Typed dependency evidence with additive compatibility projection

Each finding explicitly separates resolved version, requested/required version,
source/reference provenance, and target context. The existing `version` field
is retained as a compatibility/display projection, but it is not the authority
for applicability. Applicability consumes only evidence explicitly classified
as a resolved version, except an explicit request-field version supplied by the
caller.

### Option C — Execute ecosystem package managers to obtain authoritative graphs

Invoke commands such as `cargo metadata`, `go list -m all`, Maven dependency
resolution, or package-manager install/list operations.

This could improve fidelity but violates the passive local-workspace trust
boundary, makes results environment-dependent, adds execution and timeout
surface, and can run repository-controlled code or trigger network access.

## Decision

Choose Option B.

The dependency evidence model will add explicit typed fields sufficient to
represent at least:

- exact resolved version;
- declared/requested version requirement;
- immutable or mutable source reference, with a reference kind when known;
- package provenance/source kind and optional source locator;
- target/environment context when the lock format has multiple resolution
  graphs;
- evidence that is integrity/checksum metadata rather than selection metadata.

The exact Rust names may differ, but those distinctions are normative.

The existing `DependencyFinding.version` field remains serialized for
compatibility. New code MUST NOT infer that `version.is_some()` means the
version was selected or installed. Parsers should populate the legacy field
from the most useful human-readable token while also populating the typed
fields that carry authoritative semantics.

Applicability rules are:

1. `Affected` or `NotAffected` may be derived from dependency-file evidence
   only when an exact resolved version is available and package/ecosystem
   identity is compatible with the advisory.
2. A manifest requirement, branch/tag, interpolated variable, checksum-only
   observation, or unknown reference cannot independently establish
   `Affected` or `NotAffected`; the result is `Unknown` or
   `InsufficientEvidence` with an explanatory reason when an assessment is
   emitted.
3. Assessment confidence is bounded by both dependency-evidence confidence and
   advisory-range confidence. Downstream code cannot promote Medium/Low
   dependency evidence to High solely because the advisory has structured
   ranges.
4. Source provenance is part of applicability. A demonstrable path/workspace
   replacement or different VCS/package source must not be silently treated as
   the public-registry artifact without compatible identity evidence.
5. Package comparison uses ecosystem-specific canonicalization where the
   ecosystem defines it. It must not apply one global normalization rule.

## Consequences

Positive:

- parser findings express what the source actually proves;
- security applicability no longer relies on filename-specific trust
  assumptions;
- lockfile/parser modernization can proceed independently behind one contract;
- provenance, target framework/RID, and mutable-reference distinctions are
  available to agents without inventing certainty.

Negative:

- `DependencyFinding` becomes wider;
- parser constructors and fixtures require migration;
- consumers using the legacy `version` field need documentation that it is a
  compatibility projection;
- some current apparent applicability results will become Unknown or
  InsufficientEvidence because they were previously overconfident.

## Compatibility and migration

The MCP tool name and request schema remain unchanged. Existing
`DependencyFinding` fields remain present. New fields are additive and use
serde defaults/skip-serialization where possible. Existing clients that ignore
unknown fields continue to work.

The implementation may retain an internal compatibility constructor while
migrating parsers, but security applicability MUST switch to the typed resolved
version in the first milestone so later parser work cannot reintroduce the
trust escalation.

No package-manager subprocess execution is introduced.

## Security implications

This decision is fail-safe: ambiguous dependency evidence loses confidence
rather than being interpreted as a concrete installed version. Parser
diagnostics must distinguish unsupported/partial parsing from a valid empty
dependency set so absence cannot be mistaken for evidence of safety.

## Verification

Closure requires contract tests covering resolved versus required versions,
confidence composition, source/provenance mismatch, target-context
preservation, legacy serialization, and a regression proving that
`go.sum`-style integrity observations cannot produce High-confidence
applicability.

## Supersession

None.
