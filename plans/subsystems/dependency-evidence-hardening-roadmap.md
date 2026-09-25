# Dependency Evidence Hardening Roadmap

Status: active

Long-term references:

- `plans/000-long-term-specification.md#3`
- `plans/000-long-term-specification.md#4`
- `plans/000-long-term-specification.md#7`
- `plans/000-long-term-specification.md#9`
- `plans/001-terminology-and-domain-model.md#1`
- `plans/003-planning-process.md`

Related ADRs:

- `plans/adrs/ADR-0004-typed-dependency-evidence-semantics.md`

Architecture references:

- `architecture/security.md`
- `architecture/hardening.md`
- `architecture/testing.md`
- `architecture/maintenance.md`

## 1. Purpose and ownership boundary

Harden eggsearch's multi-ecosystem dependency evidence extraction so parsed
findings accurately distinguish resolved selections, declared requirements,
source references, integrity observations, and target-specific resolution
contexts before those findings reach security applicability.

This subsystem owns:

- `src/meta/dependency_parse/` format dispatch and ecosystem parsing;
- the dependency-evidence semantics in
  `src/core/security_applicability.rs`;
- the dependency-finding/applicability boundary inside
  `src/meta/security_search.rs`;
- dependency-parser diagnostics, resource budgets, property tests, corpus
  fixtures, and fuzz targets.

It does not own local path authorization or file opening, advisory-provider
retrieval, generic version-range semantics unrelated to dependency evidence,
or package-manager execution.

This is a new subsystem rather than a reopening of the closed maintenance
workstream because the discovered defects form a durable cross-format trust
boundary with a public `security_search` evidence consumer.

## 2. Work classification

### Invariants

- Ambiguous dependency metadata never becomes stronger applicability evidence
  than the source proves.
- Unsupported or partially parsed input is distinguishable from a valid empty
  dependency set.
- Parsing is passive: no repository code, package manager, build tool, hook, or
  network lookup is executed by dependency extraction.
- Local-file access remains root-contained, race-resistant, and byte-bounded.
- Output ordering and truncation are deterministic.
- Existing MCP tool names and request compatibility remain stable.

### Capabilities

- Current dependency formats produce correct package identity, resolution
  semantics, provenance, relation, and target context.
- `security_search` uses resolved dependency evidence without silently
  promoting requirements/checksum observations.
- Agents receive structured warnings when dependency parsing is partial,
  unsupported, or bounded.

### Infrastructure

- Typed dependency evidence contract and ecosystem-aware identity helpers.
- Format/version detection and parse-report diagnostics.
- Finding/file aggregate budgets and deterministic deduplication.
- XML/YAML parsing support only where justified by measured maintenance and
  footprint cost.

### Polish

- Realistic generated fixtures, fuzz coverage, parser documentation, and
  footprint/performance characterization.

## 3. Non-goals

- Executing `cargo metadata`, `go list`, Maven/Gradle, Bundler, Composer,
  npm/Yarn/pnpm, NuGet, Docker, or GitHub Actions.
- Reconstructing a complete build graph when the checked-in evidence cannot
  prove one.
- Evaluating arbitrary Maven/Gradle properties, Python environment markers,
  GitHub expressions, Docker build arguments, or package-manager scripts.
- Turning dependency parsing into SBOM generation.
- Adding broad new package ecosystems before the current ten ecosystem
  boundaries are correct.
- Extracting the parser into a standalone crate during this hardening
  workstream. Extraction may be reconsidered after the contract stabilizes.

## 4. Current state

Baseline: `e0dab301cc04bf5e1e232177e2e2647262a1fdd7`.

The parser is decomposed by ecosystem and already inherits a strong local-file
reader boundary: dependency files are validated against configured workspace
roots and capped at 1 MiB before parsing. Malformed-input unit tests establish
basic no-panic behavior.

The audit found semantic hardening gaps:

- `DependencyFinding.version` conflates requirements and resolved versions;
- applicability ignores finding confidence/provenance and can promote weak
  evidence to High confidence;
- `go.sum` checksum history is treated as a High-confidence lockfile;
- NuGet `packages.lock.json` parsing targets a `libraries` shape instead of
  the target-framework `dependencies -> resolved` lock model;
- Maven line scanning can confuse exclusions/dependency-management with
  declared dependencies;
- Gemfile.lock child constraints can be emitted as resolved versions;
- Windows basename dispatch uses a slash-only split;
- Yarn Berry and pnpm lockfile v9 are not modeled explicitly;
- OCI and GitHub Actions refs are collapsed into version strings;
- dependency-file count, finding count, and parse diagnostics are not bounded
  or surfaced as a coherent parse report;
- the fuzz harness has no dependency-parser target.

## 5. Research basis

Implementation should preserve the semantics documented by upstream formats:

- Cargo dependency requirements, rename/path/git/workspace semantics:
  https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html
- Go `require` as minimum requirement, MVS build list, `replace`, `go.sum`
  checksum history, and `vendor/modules.txt`:
  https://go.dev/ref/mod
- Python dependency grammar and package-name normalization:
  https://packaging.python.org/en/latest/specifications/dependency-specifiers/
  and
  https://packaging.python.org/en/latest/specifications/name-normalization/
- npm package-lock v1-v3:
  https://docs.npmjs.com/cli/v11/configuring-npm/package-lock-json/
- pnpm lockfile v9:
  https://github.com/pnpm/spec/blob/master/lockfile/9.0.md
- NuGet PackageReference lock behavior:
  https://learn.microsoft.com/en-us/nuget/consume-packages/package-references-in-project-files
- Maven dependency, scope, optional, exclusion, and dependency-management
  semantics:
  https://maven.apache.org/guides/introduction/introduction-to-dependency-mechanism
- Composer exact lock behavior and source/dist provenance:
  https://getcomposer.org/doc/01-basic-usage.md and
  https://getcomposer.org/doc/05-repositories.md
- GitHub Actions `uses` refs, local actions, reusable workflows, and Docker
  actions:
  https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax
- Docker `FROM` platform/tag/digest grammar:
  https://docs.docker.com/reference/dockerfile

A streaming XML parser is justified for Maven/.NET correctness if its
dependency footprint remains acceptable; `quick-xml` is the preferred
candidate to qualify. A general YAML dependency is not mandated: the JS lock
and workflow milestones must compare a generated-format parser against a
maintained YAML parser and keep the lower-risk/footprint option.

## 6. Target architecture

```text
bounded dependency file
        |
        v
format dispatcher + format/version detection
        |
        v
ecosystem parser
        |
        +--> typed DependencyFinding(s)
        |
        +--> parse diagnostics/completeness
        |
        v
deterministic finding budget + dedup
        |
        v
security applicability boundary
        |
        +--> exact resolved evidence -> range assessment
        |
        +--> requirement/reference/integrity evidence
        |      -> Unknown / InsufficientEvidence
        |
        v
security_search response + structured warnings
```

The legacy `version` field remains a compatibility projection. Typed resolved
version, version requirement, provenance/reference, relation, target context,
and evidence confidence are the authority.

## 7. Dependency graph

```text
M001 typed evidence + applicability trust boundary
    |
    +--> M002 Cargo / Go / Python + cross-platform dispatch
    |
    +--> M003 .NET / Maven / Gradle structured correctness
    |
    +--> M004 Ruby / Composer lock provenance
    |
    +--> M005 npm / Yarn / pnpm lockfile modernization
    |
    +--> M006 GitHub Actions / OCI reference semantics
             |
             +----------------------+
                                    v
M007 parser budgets, diagnostics, fuzz and qualification
    hard: M001-M006 behavior contracts
```

M002-M006 have a hard dependency on M001 because they must emit the typed
contract rather than extending the ambiguous legacy representation. M002-M006
may proceed in parallel after M001 closes. M007 depends on all format
milestones so the final adversarial corpus measures the completed surface.

## 8. Milestones

### M001 — Typed dependency evidence and applicability trust boundary

Class: invariant + infrastructure.

Objective: establish ADR-0004 in code, preserve the legacy response projection,
and make security applicability consume only evidence that actually proves a
resolved version.

Plan:
`plans/implementation/dependency-evidence-hardening/001-typed-evidence-and-applicability-boundary.md`.

Exit: requirements/checksum observations cannot produce resolved-version
Affected/NotAffected claims; assessment confidence is bounded by dependency
evidence; source/relation provenance survives into assessments.

### M002 — Cargo, Go, Python, and cross-platform dispatch correctness

Class: capability + infrastructure.

Objective: use structured TOML where already available, correct Go evidence
semantics, apply Python requirement/name rules, and make dispatch path-safe on
Windows and Unix.

Plan:
`plans/implementation/dependency-evidence-hardening/002-core-ecosystem-and-dispatch-correctness.md`.

Exit: Cargo aliases/path/git/workspace semantics are represented, `go.sum`
is integrity-only, `go.mod` requirements and replacements are not treated as
resolved selections, vendored Go versions can supply exact evidence when
present, Python names are canonicalized for comparison, and Windows dependency
paths dispatch identically to Unix paths.

### M003 — .NET, Maven, and Gradle structured correctness

Class: capability.

Objective: parse NuGet lock graphs correctly and replace unsafe XML line
heuristics with bounded structural parsing for csproj/POM inputs while keeping
Gradle heuristics conservative.

Plan:
`plans/implementation/dependency-evidence-hardening/003-dotnet-jvm-structured-correctness.md`.

Exit: NuGet target frameworks/RIDs and Direct/Transitive/Project-like types are
handled explicitly; Maven exclusions/dependency-management cannot masquerade
as direct dependencies; unresolved properties remain requirements/unknown.

### M004 — Ruby and Composer lock provenance

Class: capability.

Objective: distinguish Bundler resolved specs from child constraints and retain
GEM/GIT/PATH plus Composer source/dist provenance.

Plan:
`plans/implementation/dependency-evidence-hardening/004-ruby-composer-lock-provenance.md`.

Exit: nested Gemfile.lock requirements are never reported as resolved versions;
VCS/path sources do not silently inherit public-registry certainty; Composer
lock versions remain exact while custom source provenance is retained.

### M005 — npm, Yarn, and pnpm lockfile modernization

Class: capability.

Objective: support the current versioned lock models without treating root
workspaces, links, descriptors, peer suffixes, or unsupported lockfile versions
as ordinary registry packages.

Plan:
`plans/implementation/dependency-evidence-hardening/005-javascript-lockfile-modernization.md`.

Exit: npm v1-v3, Yarn Classic plus the supported modern Yarn shape, and pnpm
v6/v9 fixtures produce deterministic correct resolved findings or explicit
partial/unsupported diagnostics.

### M006 — GitHub Actions and OCI reference semantics

Class: capability.

Objective: type mutable/immutable action refs and OCI tags/digests, parse
Docker `FROM` options/stages safely, and route Docker-backed Actions into OCI
evidence rather than fake GitHub package versions.

Plan:
`plans/implementation/dependency-evidence-hardening/006-actions-oci-reference-semantics.md`.

Exit: full commit SHA, tag/branch, local action, Docker action, OCI tag,
OCI digest, variable, and stage alias cases are distinguishable and none is
misreported as an exact semantic package version.

### M007 — Parser budgets, diagnostics, fuzzing, and qualification

Class: invariant + infrastructure + polish.

Objective: bound aggregate parser work, surface completeness/unsupported
states, add property/fuzz/corpus coverage, and qualify the full hardening
workstream.

Plan:
`plans/implementation/dependency-evidence-hardening/007-budgets-diagnostics-and-adversarial-qualification.md`.

Exit: file/finding budgets are deterministic, truncation is explicit, parser
fuzz/property tests cover the production dispatch boundary, realistic
multi-ecosystem fixtures prevent format regressions, and canonical gates pass
on the exact candidate.

Sequencing:
`plans/implementation/dependency-evidence-hardening/000-overview-and-sequencing.md`.

Handoff checklist:
`plans/implementation/dependency-evidence-hardening/008-implementation-handoff-checklist.md`.

## 9. Cross-cutting requirements

### Compatibility

All changes to serialized `DependencyFinding` are additive. Existing tool
names and request fields remain stable. The legacy `version` field remains
available even after typed fields become authoritative.

### Security and authorization

Do not weaken `dependency_files` root containment, safe-open behavior, or the
1 MiB per-file cap. Do not execute project code or package managers. Do not
resolve local paths outside the configured workspace as part of parsing.

### Failure and recovery

Malformed, unsupported, and partially supported inputs fail soft with
diagnostics. They must not become indistinguishable from a successfully parsed
file with zero dependencies.

### Performance and resources

Parsing remains linear or near-linear in bounded input size. Avoid unbounded
recursive descent, alias expansion, or quadratic dedup. Any new production
parser dependency must be justified with `cargo tree`, release-binary size,
and `make bench-check` characterization.

### Determinism

Finding ordering, target-context ordering, deduplication, warning ordering, and
budget truncation must be deterministic across runs.

### Documentation

Update `architecture/security.md`, `architecture/hardening.md`,
`architecture/testing.md`, and `architecture/maintenance.md` as contracts
land. Update `docs/test-inventory.md` and `skills/eggsearch-dev/SKILL.md`
when suites or fuzz targets change.

## 10. Verification strategy

Focused verification belongs with each milestone. Final closure requires at
least:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --locked --features mock --test security_workflow
cargo test --locked --features mock --test security_applicability_contract
cargo test --locked --features mock --test security_applicability_regression
cargo test --locked --features mock --test security_applicability_corpus
make fuzz-smoke
make bench-check
make check
```

If a new parser dependency lands, record before/after release-binary size and
runtime dependency graph in the closure evidence.

## 11. Risks and decision points

- Full Maven/Gradle effective-model reconstruction is intentionally out of
  scope; unresolved interpolation must degrade to requirement/unknown evidence.
- Go has no lockfile build-list artifact; exact selected versions are only
  claimed from evidence such as a consistent vendored module manifest, not
  from `go.sum`.
- Modern Yarn does not promise a stable generic YAML API for third-party
  parsers; support should be fixture- and format-detection driven with an
  explicit unsupported path.
- A generic YAML crate can increase dependency footprint and attack surface.
  Adopt one only after the M005/M006 comparison demonstrates a net
  maintainability benefit under the footprint gate.
- Correct hardening will intentionally reduce some current High-confidence
  applicability results to Unknown/InsufficientEvidence.

## 12. Completion definition

This workstream closes when every supported dependency source has explicit
selection/requirement/reference semantics, known false-positive paths are
regressed, unsupported/partial formats are observable, aggregate work is
bounded, parser adversarial tests cover the public dispatch seam, and the
security applicability consumer can no longer overstate dependency evidence.

## 13. Milestone status

| Milestone | Status | Dependency |
|---|---|---|
| M001 typed evidence/applicability | closed | ADR-0004 accepted; closure `plans/closure/dependency-evidence-hardening/001-status.md` at `bce32e7` |
| M002 core ecosystems/dispatch | closed | Closure `plans/closure/dependency-evidence-hardening/002-status.md` at `e1464f6` |
| M003 .NET/JVM structured correctness | closed | Closure `plans/closure/dependency-evidence-hardening/003-status.md` at `0d934f5` |
| M004 Ruby/Composer provenance | ready | M001 closed |
| M005 JavaScript lockfiles | ready | M001 closed |
| M006 Actions/OCI refs | ready | M001 closed |
| M007 budgets/qualification | blocked | M001-M006 closure |
