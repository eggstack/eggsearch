# Plan 009 — Corrective Applicability Provenance, Identity, and Parse-Status Closure

Status: implementation plan

Source roadmap:
`plans/subsystems/dependency-evidence-hardening-roadmap.md`

Milestone: M008 corrective correctness closure

Primary class: invariant + corrective capability

Baseline: `a10f23ba92be05d4b42ba1f373c895895cbf88aa`

Applicable ADR:
`plans/adrs/ADR-0004-typed-dependency-evidence-semantics.md`

Original milestones and closure records corrected by this plan:

- M001 typed evidence/applicability:
  `plans/closure/dependency-evidence-hardening/001-status.md`
- M002 core ecosystems/dispatch:
  `plans/closure/dependency-evidence-hardening/002-status.md`
- M007 budgets/diagnostics/qualification:
  `plans/closure/dependency-evidence-hardening/007-status.md`

## Objective

Close three correctness gaps discovered after M007 qualification without
reopening or rewriting the historical M001-M007 closure records:

1. make source provenance participate in advisory applicability so an exact
   version from a path/workspace/VCS/replacement/custom source cannot silently
   inherit public-registry advisory identity;
2. replace the remaining global case-folded package matcher with explicit
   ecosystem-specific identity semantics and a conservative exact-match
   fallback;
3. ensure malformed structured dependency inputs cannot be converted into
   `ParseStatus::Complete` with zero findings by a bare-Vec parser wrapper.

The corrected candidate must retain the typed evidence, bounded parsing,
deterministic ordering, passive-workspace, and compatibility invariants already
established by M001-M007.

## Why the original verification did not catch these defects

### Provenance propagation was tested, provenance enforcement was not

M001 verified that `DependencyFinding.provenance`, `version_source`, and
`dependency_relation` survived into findings/assessments. The matching index
in `src/meta/advisory_range.rs`, however, is keyed only by ecosystem and
package identity. An exact-version finding can therefore reach
`resolved_finding_assessment()` even when its source proves that it is a
workspace/path/VCS/replaced artifact rather than the public-registry artifact.

The original closure evidence proved data preservation, not that provenance
was used as an applicability gate.

### Identity tests covered normalization examples, not negative ecosystem cases

The identity seam added PyPI normalization and retained a broad
case-insensitive fallback. `index_key()` then lowercases its package key for
all ecosystems. Tests assert positive PyPI/npm cases but do not assert that an
ecosystem with case-significant identity, such as a Go module path, remains
distinct.

The accepted ADR requires ecosystem-specific canonicalization, not one global
case-insensitive policy.

### Parse-report tests did not cover Vec-returning structured parser failures

The dispatcher introduced `DependencyParseReport`, but several structured
parsers still return `Vec<DependencyFinding>`. For example, Cargo TOML,
Composer JSON, and Poetry/uv TOML paths can return an empty Vec on a syntax
error; the dispatcher then wraps that Vec with
`DependencyParseReport::complete(...)`.

M007 tested diagnostics/budgets and parsers that already owned report status,
but did not include a table-driven malformed-input contract for every
recognized structured file routed through a compatibility Vec wrapper.

## Invariants

- Existing MCP tool names and request fields remain unchanged.
- Existing serialized `DependencyFinding` fields remain additive-compatible.
- An advisory match may never claim `Affected` or `NotAffected` from a
  dependency artifact whose source identity is incompatible or unverified for
  that advisory ecosystem.
- A valid empty dependency file and a malformed dependency file remain
  distinguishable.
- Unknown identity/provenance degrades to Unknown/InsufficientEvidence rather
  than false safety or false affectedness.
- No package manager, build system, repository code, Git ref, image registry,
  or network lookup is executed to resolve identity.
- Existing root containment, safe-open, 1 MiB per-file cap, aggregate budgets,
  deterministic ordering, and truncation diagnostics remain intact.
- M001-M007 historical closure records are not rewritten to hide the later
  discovery.

## Non-goals

- Resolving VCS refs or registry package metadata over the network.
- Establishing cryptographic artifact equivalence between a fork and an
  upstream registry package.
- Reconstructing package-manager effective graphs.
- Adding new dependency ecosystems or lockfile versions.
- Reworking advisory range syntax/comparison beyond identity/provenance gating.
- Closing the subsystem planning status; that is M009/Plan 010 after this
  corrective milestone has its own accepted closure record.

## Required production changes

### 1. Add an explicit advisory-identity/provenance compatibility gate

Introduce one deterministic helper at the dependency-finding/applicability
boundary, with a small result vocabulary such as:

- `Compatible` — source evidence is compatible with applying an advisory for
  this ecosystem/package;
- `Incompatible` — source evidence proves a different/local artifact;
- `Unverified` — source evidence is insufficient to claim registry identity.

Names may differ, but the three-way semantic distinction is required.

The helper must consume typed finding fields rather than parse free-form
warning text. At minimum consider:

- `reference_kind`;
- `reference_value`;
- `provenance`;
- ecosystem;
- package identity;
- resolved-version presence.

Required conservative behavior:

- Path, workspace, and local project sources are not public-registry exact
  applicability evidence.
- A Go `replace` to another module or local path is not silently assessed as
  the original module artifact.
- Cargo git/path sources are not silently assessed as crates.io artifacts
  solely because name/version match.
- Composer/npm/Ruby custom VCS/path/link/workspace sources remain unverified or
  incompatible unless the checked-in evidence itself establishes compatible
  registry identity.
- Ordinary lock entries whose source metadata establishes the ecosystem's
  normal registry remain eligible.
- Missing source metadata for formats where the lock format conventionally
  denotes the ecosystem registry may remain eligible when the parser contract
  already establishes that fact; document that per ecosystem.

Do not simply discard incompatible/unverified findings. Preserve them in
`dependency_findings` and emit an Unknown/InsufficientEvidence assessment or
structured warning explaining that exact-version applicability was withheld
because artifact provenance was not established.

Add a stable warning code if the current warning vocabulary cannot express
this condition, for example
`dependency_provenance_unverified_for_advisory`.

### 2. Make assessment dedup provenance/target aware

The current assessment dedup key is effectively advisory + package + version.
That can collapse semantically distinct evidence, such as one public-registry
finding and one replaced/forked finding with the same displayed version.

Change the key or merge logic so evidence that differs in applicability basis,
provenance, target context, or source cannot nondeterministically suppress a
more conservative assessment.

Preserve deterministic response ordering and avoid duplicating truly
equivalent findings.

### 3. Replace global case folding with an ecosystem identity matrix

Make `canonical_package_name()`, `packages_match()`, and the finding-index
key share one authoritative ecosystem-specific identity rule.

Required design rules:

- PyPI retains standardized name normalization: lowercase and collapse runs of
  `-`, `_`, and `.` to `-`.
- Go module paths must not be globally lowercased; case-distinct module paths
  remain distinct.
- NuGet remains case-insensitive.
- For npm, crates.io, RubyGems, Packagist/Composer, Maven, GitHub Actions, and
  OCI, document and test the chosen rule from the ecosystem's authoritative
  naming semantics before changing behavior.
- If no authoritative normalization rule is established for an ecosystem,
  use conservative exact identity rather than broad case folding.
- Display/original package names remain unchanged in findings and responses.

Remove the unconditional final `.to_lowercase()` from the shared index-key
path unless the selected ecosystem rule itself requires case folding.

Add a table-driven identity test matrix containing both positive and negative
pairs. At minimum include a negative Go case-difference test and preserve
existing PyPI normalization behavior.

### 4. Make structured parser failures own ParseStatus

Audit every dependency parser that invokes a structured syntax parser
(`toml`, `serde_json`, `quick-xml`, and any maintained YAML parser in
use). A syntax/shape failure must not disappear behind a bare Vec return that
the dispatcher labels Complete.

At minimum correct:

- `Cargo.toml`;
- `Cargo.lock`;
- `composer.lock`;
- `poetry.lock`;
- `uv.lock`;
- `Pipfile.lock` where malformed JSON currently collapses to empty findings.

Also inspect all other dispatcher arms using
`DependencyParseReport::complete(parser(...))` and include any parser that
can distinguish a syntax failure from a valid empty document.

Preferred implementation: structured parsers that can fail return
`DependencyParseReport` directly. A narrower typed internal result is
acceptable if the dispatcher still receives explicit status.

Semantics:

- syntax-invalid recognized file -> `Malformed`;
- valid syntax but unsupported recognized schema/version -> `Unsupported` or
  `Partial` with a stable diagnostic;
- valid supported file with no dependencies -> `Complete` + empty findings;
- partial recovery after a later syntax/section failure -> `Partial` with
  retained valid findings and a diagnostic;
- parser budget truncation remains `Partial`, as in M007.

### 5. Add regression guards that would have caught the original misses

Add focused contract tests rather than relying only on implementation-unit
tests:

1. Cargo git/path exact version cannot yield public crates.io
   Affected/NotAffected without compatible provenance.
2. Go vendored public module remains eligible, while a replaced module/path
   with the same displayed package/version is not treated as the original.
3. Two same package/version findings with different provenance cannot suppress
   one another through assessment dedup.
4. Go package identity with case difference does not match.
5. PyPI punctuation/case normalization still matches.
6. NuGet case-insensitive identity remains correct.
7. Every recognized structured parser has malformed-input and valid-empty
   cases proving distinct `ParseStatus`.
8. Dependency parse warnings surface the malformed/provenance condition through
   the existing structured-warning boundary.
9. Fuzz/property coverage includes recognized structured filenames with
   malformed payloads and asserts malformed inputs are never reclassified as
   Complete solely because findings are empty.

If a static guard can cheaply prevent reintroduction of
`Err(_) => return Vec::new()` behind Complete-wrapped structured parser
dispatch, add it. Do not add a brittle source-text guard if a behavioral
contract test provides stronger coverage.

## Ordered work packages

1. Write failing provenance, identity, parse-status, and dedup regressions.
2. Implement the shared identity matrix and remove unconditional global
   lowercasing.
3. Implement provenance compatibility classification and conservative
   assessment behavior.
4. Make assessment dedup preserve semantically distinct provenance/target
   evidence.
5. Migrate structured parsers to explicit report/error semantics.
6. Extend property/fuzz coverage.
7. Update architecture/security/testing/hardening documentation.
8. Run exact-candidate focused and canonical verification.
9. Write
   `plans/closure/dependency-evidence-hardening/008-status.md` and update the
   roadmap/registry in the same closure commit. Do not mark the overall
   subsystem closed yet; M009 remains the final reconciliation gate.

## Compatibility and migration

No request field or MCP tool name changes. Keep existing serialized finding and
assessment fields. New warning codes or optional applicability-basis metadata
are additive.

Correct behavior may intentionally change prior dependency-driven
Affected/NotAffected assessments to Unknown when source identity is local,
replaced, forked, VCS-derived, or otherwise not established as the advisory's
artifact. Record examples in closure evidence.

## Failure and recovery

Identity/provenance ambiguity is evidence weakness, not a parser crash.
Malformed structured input is a parse failure/partial state, not empty negative
evidence. Sibling dependency files continue contributing findings when one
file is malformed or provenance-incompatible.

## Required verification

Run at minimum:

```bash
cargo fmt --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --features mock --test security_applicability_contract
cargo test --locked --features mock --test security_applicability_regression
cargo test --locked --features mock --test security_applicability_corpus
cargo test --locked --all-features --test property_dependency_parse
cargo test --locked --all-features --test dependency_fixtures
cargo test --locked --features mock --test security_workflow
cargo test --locked --features mock --test static_guards
cargo test --locked --all-features
RUSTUP_TOOLCHAIN=nightly cargo fuzz run dependency_parse -- -max_total_time=60
make bench-check
make check
```

If production dependencies change, also record `cargo tree --locked` and
release-binary before/after size. No new runtime dependency is expected for
this corrective pass.

## Documentation updates

Update as needed:

- `architecture/security.md` — provenance compatibility and identity matrix;
- `architecture/testing.md` — new regression/property coverage;
- `architecture/hardening.md` — malformed structured-input invariant;
- `architecture/maintenance.md` — owner of the shared identity/provenance
  helper;
- `docs/test-inventory.md` / `skills/eggsearch-dev/SKILL.md` only if suite
  inventory or commands change.

## Acceptance criteria

- Public-registry advisory applicability is withheld when checked-in evidence
  proves or suggests a different local/VCS/replaced artifact and registry
  identity is not established.
- Same-version findings with distinct provenance/targets cannot collapse into a
  misleading single assessment.
- Package identity comparison is explicit per ecosystem with no unconditional
  global lowercasing.
- Go case-distinct module paths remain distinct.
- PyPI and NuGet required normalization semantics remain correct.
- Malformed Cargo/Composer/Python structured lock/manifest inputs cannot return
  `Complete` solely because their parser returned an empty Vec.
- Valid empty structured files still return `Complete`.
- Regression/property/fuzz tests would fail if any of the three discovered
  defect classes returns.
- Canonical gates pass on the exact candidate.

## Stop conditions

Stop and write another corrective plan rather than broadening this milestone if
any of the following is required:

- network/package-manager execution to prove artifact provenance;
- a breaking serialized contract;
- registry identity semantics cannot be established conservatively without an
  ADR change;
- the fix requires a general package graph resolver;
- the parser must evaluate project-controlled expressions/code.

## Closure evidence required

The M008 closure record must include:

- exact implementation SHA;
- the three original defect reproductions and corrected outcomes;
- ecosystem identity matrix with authoritative rationale;
- provenance compatibility table;
- malformed-versus-empty parser matrix;
- assessment dedup evidence for mixed provenance;
- focused test/fuzz outcomes;
- `make check` and all-feature outcome;
- dependency/binary-size delta if any;
- residual findings classified by severity;
- recommendation: closed, conditionally closed, or another corrective pass.
