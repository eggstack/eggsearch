# Plan 004 — Ruby and Composer Lock Provenance

Status: implementation plan

Source roadmap:
`plans/subsystems/dependency-evidence-hardening-roadmap.md`

Milestone: M004

Primary class: capability

Baseline for planning: `e0dab301cc04bf5e1e232177e2e2647262a1fdd7`

Hard dependency: M001 closed.

## Objective

Correct Bundler lockfile indentation/source semantics and preserve Composer
lock provenance so child constraints, VCS/path sources, and custom package
sources do not masquerade as exact public-registry dependency evidence.

## Current implementation evidence

- `parse_gemfile_lock()` switches on `specs:` and accepts any line that can
  `strip_prefix("    ")`. Six-space child dependency constraints therefore
  also satisfy the parser and text inside parentheses is emitted as a
  High-confidence resolved version.
- The parser does not retain GEM/GIT/PATH source sections.
- `composer.lock` correctly obtains package name/version from JSON but
  unconditionally labels the ecosystem as Packagist and drops source/dist
  provenance.

## Required production changes

### 1. Bundler lockfile state machine

Parse the generated lock structure with explicit section and indentation
states. At minimum recognize:

- GEM source and its `specs`;
- GIT source metadata and specs;
- PATH source metadata and specs;
- top-level DEPENDENCIES separately from resolved specs;
- platform metadata only as context, not package identity.

Only resolved spec rows become exact-version findings. Nested child
requirements remain dependency-edge metadata or are omitted from findings;
they must never be emitted as resolved packages merely because they contain
parentheses.

Preserve source provenance and revision/ref/branch when present for GIT/PATH
sources. Registry-backed GEM specs may use RubyGems provenance when the source
actually establishes it.

### 2. Bundler relation semantics

Do not infer Direct solely from being a resolved spec. Use the top-level
DEPENDENCIES section to mark direct package identities where possible and
leave other resolved specs Transitive/Unknown deterministically.

Do not attempt to evaluate Gemfile Ruby code.

### 3. Composer lock evidence

Keep `packages` and `packages-dev` exact versions as resolved evidence.
Retain enough `source` / `dist` metadata to distinguish ordinary package
archives from VCS/path/custom provenance when the lockfile exposes it.

Do not assume that every Composer package came from Packagist solely because
the ecosystem is Composer/Packagist. Package identity may still use the
Packagist ecosystem for advisory lookup, but applicability confidence/provenance
must reflect a custom source when demonstrated.

Preserve dev-package context so future consumers can distinguish runtime and
development exposure without changing current response relation semantics
unnecessarily.

### 4. Version/reference semantics

Composer dev versions, source references, and branch aliases must remain
distinguishable from ordinary release versions. A VCS commit reference is
source provenance, not a semantic package version.

## Required tests

Bundler fixtures:

- resolved gem with child constraint;
- multiple resolved specs;
- top-level DEPENDENCIES direct marking;
- GIT source with revision/branch/ref;
- PATH source;
- malformed section transitions;
- same package name from different source sections.

Composer fixtures:

- packages and packages-dev;
- ordinary stable version;
- `v`-prefixed version;
- dev branch with source reference;
- custom source/dist URL metadata;
- missing/empty version must not serialize `Some("")`.

Regression requirement: `benchmark (>= 0.3)` nested under another gem must
never become a High-confidence resolved `benchmark` finding unless a separate
resolved spec for benchmark exists.

## Verification

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --locked dependency_parse
cargo test --locked --features mock --test security_applicability_regression
cargo test --locked --features mock --test security_workflow
cargo test --locked --all-features
```

## Documentation updates

Update `architecture/security.md` with Bundler/Composer provenance and
direct/transitive semantics.

## Acceptance criteria

- Child Bundler constraints are never emitted as resolved versions.
- GEM/GIT/PATH provenance is retained.
- Direct Bundler dependencies can be distinguished where the lockfile proves
  them.
- Composer exact lock versions remain resolved evidence while source references
  remain provenance/reference evidence.
- Empty or malformed versions do not become exact findings.
- Focused and broad tests pass.

## Stop conditions

Do not parse/evaluate Gemfile Ruby code or run Bundler/Composer. If lock
metadata cannot prove registry provenance, retain Unknown/custom provenance
rather than guessing.
