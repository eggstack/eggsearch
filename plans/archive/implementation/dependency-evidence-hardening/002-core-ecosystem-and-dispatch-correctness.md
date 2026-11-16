# Plan 002 — Cargo, Go, Python, and Cross-Platform Dispatch Correctness

Status: implementation plan

Source roadmap:
`plans/subsystems/dependency-evidence-hardening-roadmap.md`

Milestone: M002

Primary class: capability + infrastructure

Baseline for planning: `e0dab301cc04bf5e1e232177e2e2647262a1fdd7`

Hard dependency: M001 closed.

## Objective

Correct the core manifest/lock semantics for Cargo, Go, and Python while
making dependency-file dispatch path-safe on Windows and Unix. Reuse the typed
evidence contract from M001 rather than adding format-specific trust rules in
security orchestration.

## Current implementation evidence

- `parse_dependency_file()` derives the basename with
  `path.rsplit('/')`, so Windows absolute paths do not match exact filename
  arms.
- Cargo TOML/lock parsing is line-oriented despite `toml = "0.8"` already
  being a runtime dependency.
- Cargo aliases, target tables, workspace inheritance, git/path sources, and
  lockfile `source` provenance are not represented.
- `go.mod` requirements are labeled `LockFile` and `Transitive`, while Go
  defines them as minimum required versions and marks indirect requirements
  explicitly.
- `go.sum` is emitted as High-confidence lock evidence even though upstream
  documentation states it may contain multiple and no-longer-needed versions.
- Python requirements parsing does not implement PEP 508 direct references,
  wildcard equality, arbitrary equality, or package-name canonicalization.

## Invariants

- No package-manager command is executed.
- Existing file byte/root safety remains unchanged.
- Structured TOML parsing must not add another TOML implementation.
- Requirement/environment syntax that cannot be evaluated is preserved as
  requirement/context, never guessed into an installed version.
- Finding order remains deterministic.

## Required production changes

### 1. Cross-platform dispatch

Replace slash-only basename extraction with a path-aware implementation that
handles native paths and add a conservative compatibility fallback for
backslash-separated path strings received on non-Windows hosts.

Add dispatch recognition for `vendor/modules.txt` only when the path clearly
identifies the Go vendor manifest. Do not broaden arbitrary `modules.txt`
files.

### 2. Cargo.toml

Use the existing `toml` dependency to parse:

- `[dependencies]`, `[dev-dependencies]`, `[build-dependencies]`;
- target-specific dependency tables;
- string requirements;
- inline/table requirements;
- `package =` rename identity;
- `workspace = true` without pretending the version is locally resolved;
- `git`, `rev`, `tag`, `branch`, `path`, and custom registry/source
  information that affects provenance.

Manifest versions are requirements, including strings that look exact under
Cargo's compatible-version rules.

### 3. Cargo.lock

Use structured TOML parsing for package entries. Exact package versions are
resolved evidence, but preserve `source` so git/path/non-crates.io entries do
not automatically inherit crates.io provenance. Retain source lines only when
they can be obtained cheaply and correctly; otherwise prefer `None` over an
incorrect line number.

### 4. Go

Parse `go.mod` require blocks/single requires as minimum requirements, with
`// indirect` reflected in relation. Parse and retain `replace` directives,
including local path and module/version replacements, so provenance is not
silently attributed to the original public module.

Demote `go.sum` to integrity/checksum observations. It must never provide
resolved-version applicability evidence.

Add a bounded parser for `vendor/modules.txt` headers so vendored module
versions can be represented as exact resolved evidence when the file is
present. Preserve replacement information encoded in the vendor manifest where
available.

Do not attempt to compute MVS in eggsearch.

### 5. Python requirements

Implement the subset of the current dependency-specifier grammar needed to
extract package identity, extras, requirement text, direct URL, and environment
marker without evaluating the marker.

At minimum distinguish:

- exact equality versus wildcard equality;
- arbitrary equality `===`;
- ranges and compatible release `~=`;
- direct references `name @ URL`;
- extras;
- environment markers.

A requirement is not a resolved version even when it is `==1.2.3`; keep the
requirement text typed as such. Lockfile parsers remain the source of exact
resolved evidence.

Apply PyPI canonical package-name comparison by lowercasing and collapsing
runs of `.`, `_`, and `-` to `-` at comparison time. Preserve the
display/original name in findings.

Before adding `pep508_rs` or another runtime crate, compare its dependency
graph/footprint to a bounded local parser. Prefer the maintained crate only if
it materially reduces custom grammar risk under eggsearch's footprint gate.

### 6. Poetry/uv/Pipfile lock semantics

Use existing TOML/JSON parsers where practical. Exact lock versions are
resolved evidence. Preserve source/path/git metadata when present rather than
assuming every package came from PyPI.

## Required tests

Use upstream-shaped fixtures, including:

- Windows and Unix paths for every dispatcher arm touched;
- Cargo renamed dependency, target dependency, workspace dependency, git/path
  dependency, and lock source;
- Go direct/indirect require, replace-to-module, replace-to-path, stale
  `go.sum` version, and vendored module manifest;
- Python normalized-name equivalents;
- PEP 508 direct URL, extras/marker, `==1.2.*`, `===token`, and ordinary
  range;
- Poetry/uv non-registry source fixture if supported by the lock schema.

Regression requirement: a stale version present only in `go.sum` must not
produce an Affected/NotAffected assessment.

## Verification

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --locked dependency_parse
cargo test --locked --features mock --test security_applicability_regression
cargo test --locked --features mock --test security_workflow
cargo test --locked --all-features
```

If a Python parser dependency is added, also record:

```bash
cargo tree --locked
cargo build --locked --release
make bench-check
```

## Documentation updates

Update `architecture/security.md` with Go/Cargo/Python evidence semantics and
`architecture/maintenance.md` if new helper ownership is introduced.

## Acceptance criteria

- Windows/Unix filename dispatch is equivalent.
- Cargo requirements are never misclassified as resolved versions.
- Cargo package rename/source provenance is preserved.
- `go.sum` cannot drive resolved applicability.
- Go replacements and indirect relation are represented.
- `vendor/modules.txt` can provide exact vendored versions without execution.
- Python requirement parsing obeys the current dependency-specifier grammar
  boundaries and package-name canonicalization.
- All affected tests and canonical gates pass.

## Stop conditions

Do not implement MVS, execute `go list`, evaluate Python environment markers,
or recursively resolve Cargo workspace manifests within this milestone.
