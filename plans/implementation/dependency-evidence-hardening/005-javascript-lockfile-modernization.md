# Plan 005 — npm, Yarn, and pnpm Lockfile Modernization

Status: implementation plan

Source roadmap:
`plans/subsystems/dependency-evidence-hardening-roadmap.md`

Milestone: M005

Primary class: capability

Baseline for planning: `e0dab301cc04bf5e1e232177e2e2647262a1fdd7`

Hard dependency: M001 closed.

## Objective

Make JavaScript dependency extraction explicitly format/version aware across
npm package-lock v1-v3, Yarn Classic and the supported modern Yarn lock shape,
and pnpm lockfile v6/v9. Preserve workspace/link/source distinctions and fail
with diagnostics rather than silently returning misleading package evidence.

## Current implementation evidence

- npm v2+ JSON parsing walks `packages`, including the root `""` entry,
  and derives package identity from the location key.
- npm v1 fallback walks only the top-level `dependencies` map and misses
  nested transitive dependency objects.
- Yarn parsing expects Classic-style `version "x"` and does not understand
  modern `version:` metadata or descriptor protocols.
- pnpm parsing assumes a v6-like `/name@version:` key and does not inspect
  `lockfileVersion`; v9 uses `<pkg_name>@<pkg_version>` package IDs plus a
  snapshots section and importer metadata.
- Quoted YAML keys, scoped names, peer suffixes, workspaces, links, and
  non-registry sources can be misidentified.

## Upstream format anchors

- npm v1-v3:
  https://docs.npmjs.com/cli/v11/configuring-npm/package-lock-json/
- Yarn Classic:
  https://classic.yarnpkg.com/en/docs/yarn-lock
- pnpm v9:
  https://github.com/pnpm/spec/blob/master/lockfile/9.0.md

For modern Yarn, build fixtures from current Yarn-generated lockfiles and pin
the supported structural contract in tests; unsupported future metadata
versions must produce explicit diagnostics.

## YAML parser decision

Before adding a production YAML crate, compare:

1. a narrow parser for the generated lockfile grammar and known versioned
   shapes; and
2. a maintained pure-Rust YAML parser such as `yaml-rust2`.

Do not add deprecated `serde_yaml`. If a generic YAML parser is chosen,
record `cargo tree`, release-binary size, compile impact, malformed-input
behavior, and alias/anchor resource behavior. The parser must be bounded and
must not permit unbounded alias expansion.

The implementation should prefer the option with lower combined custom-code
risk and runtime footprint, not simply the fewest source lines.

## Required production changes

### 1. npm package-lock / shrinkwrap

Read and validate `lockfileVersion`.

For v2/v3 `packages`:

- skip the root `""` project as an external dependency;
- distinguish workspace/link entries from registry package entries;
- preserve exact package versions where the entry represents a resolved
  package;
- retain source/resolved locator when it identifies git/tarball/non-registry
  sources;
- correctly derive scoped package names from nested `node_modules` paths.

For v1 `dependencies`, recursively walk nested dependency objects under a
bounded depth/finding budget supplied by the parser framework, preserving exact
resolved versions and source specifiers.

Unsupported future lock versions return partial/unsupported diagnostics instead
of optimistic parsing.

### 2. Yarn Classic

Keep Classic header/entry support but parse grouped selectors correctly,
including scoped packages and comma-separated selectors. Exact `version`
within a resolved entry is resolved evidence; descriptor constraints are
requirements.

### 3. Modern Yarn

Detect the modern lock shape/`__metadata` and parse supported descriptor
entries, exact `version:`, and `resolution:` package identity without
confusing descriptor protocols with package names.

Workspace/link/portal/file/git-style protocols must retain non-registry
provenance and must not become npm registry applicability evidence solely from
their display name.

### 4. pnpm

Detect `lockfileVersion` and implement explicit v6 and v9 key parsing.

For v9:

- parse `packages` dependency IDs as `<name>@<version>`;
- strip peer-dependency suffixes from snapshot paths only where the format
  specifies them;
- use importers to identify direct dependencies and their specifiers;
- preserve workspace/link/file/git source semantics;
- avoid emitting snapshot peer suffixes or quotes as part of package/version
  identity.

For v6, retain relative/absolute dependency path semantics and source metadata.

Unknown major versions produce an explicit unsupported diagnostic.

## Required tests

Use checked-in generated fixtures, not only handcrafted single entries.

npm:

- v1 nested dependencies;
- v2/v3 root entry excluded;
- scoped package;
- workspace/link entry;
- git/tarball source;
- unsupported lock version.

Yarn:

- Classic grouped selectors and scoped package;
- modern metadata entry;
- `npm:`, workspace/link, file/git-like protocols;
- quoted keys;
- unsupported modern metadata version if versioned.

pnpm:

- v6 scoped package;
- v9 package ID;
- v9 peer suffix/snapshot;
- importer direct versus transitive;
- workspace/link;
- unsupported major.

Add cross-format assertions that no package identity contains YAML quote
characters or peer-suffix text after normalization.

## Verification

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --locked dependency_parse
cargo test --locked --features mock --test security_applicability_regression
cargo test --locked --features mock --test security_workflow
cargo tree --locked
cargo build --locked --release
make bench-check
cargo test --locked --all-features
```

If no YAML crate is added, record that outcome and the bounded generated-format
grammar used instead.

## Documentation updates

Update `architecture/security.md` with supported lockfile versions and
`architecture/hardening.md` if a YAML parser introduces new resource
constraints.

## Acceptance criteria

- npm v1-v3, supported Yarn Classic/modern, and pnpm v6/v9 fixtures produce
  correct deterministic resolved evidence.
- Root/workspace/link/non-registry entries do not masquerade as public npm
  registry packages.
- Unsupported future lockfile versions are observable.
- Scoped identities and peer suffixes are handled correctly.
- Any new YAML dependency passes footprint/resource qualification.

## Stop conditions

Do not attempt to execute package-manager resolution or support an unknown
future lock format by guessing from superficially similar YAML.
