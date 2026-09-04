# Phase 7 — Binary-First Self-Update

Status: planned
Depends on: phase 6 asset contract
Baseline for planning: `f595683b8ebdec0afb13363ec9e8ad7654f9824b` (`eggsearch` 0.3.8)
Roadmap: `plans/deployment-roadmap.md`
Reference implementation: `eggstack/gregg` binary-first updater

## Objective

Add a safe, deterministic `eggsearch update` path that uses crates.io as the stable-version authority, prefers the exact matching GitHub Release binary for the current host, falls back to exact-version Cargo compilation only when the release binary is genuinely absent/unsupported, verifies the replacement before mutation, and never requires an operator to reinstall manually for routine upgrades.

This phase lands before service supervision. It should expose clean update outcomes and hooks/state that phase 9 can extend so a running persistent service is restarted after a successful binary replacement while a stopped service remains stopped.

## CLI contract

Add:

```text
eggsearch update
eggsearch update --check
```

`--check` performs version discovery/comparison only and never downloads/replaces/compiles.

Do not add an unattended scheduler in this phase.

The existing Clap-generated `eggsearch --version` remains the canonical candidate identity/version probe unless implementation finds a compelling compatibility need for a separate `version` subcommand.

## Non-goals

- No background auto-update daemon or cron schedule.
- No downgrade command.
- No GitHub `latest` authority for self-update.
- No package-manager-aware update path (Homebrew/apt/etc.).
- No service restart implementation until phase 9.
- No update of configuration files or provider credentials.

## Invariants

1. `env!("CARGO_PKG_VERSION")` is the installed executable's version identity.
2. crates.io stable metadata is the only authority for deciding whether an update is available.
3. GitHub Release bytes are requested from the exact `v<crates-version>` tag.
4. Downloaded bytes are checksum-verified before execution and identity/version-verified before replacement.
5. Cargo fallback is triggered only by unsupported target or confirmed exact-asset 404.
6. Network/integrity failures never silently trigger a long compile.
7. No code path invokes `sudo` or elevates itself.
8. Replacement is atomic/safe for Unix and handles Windows running-image semantics deliberately.
9. A local version newer than crates.io is never downgraded automatically.
10. Update tests are deterministic and use local/mock HTTP rather than live crates.io/GitHub in routine CI.

## Production changes

### 1. Add a shared platform/release contract module

Create a small production module such as `src/platform.rs` or `src/release.rs` that owns:

- normalized runtime OS/architecture detection;
- host -> Rust target mapping;
- supported binary target set;
- target -> asset filename mapping;
- exact GitHub asset/checksum URL construction;
- public repository/crate/program constants needed by updater logic.

Required target mapping must agree with phase 6:

```text
linux/x86_64   -> x86_64-unknown-linux-gnu
linux/aarch64  -> aarch64-unknown-linux-gnu
linux/arm      -> armv7-unknown-linux-gnueabihf only when runtime detection can prove ARMv7 hard-float compatibility
macos/x86_64   -> x86_64-apple-darwin
macos/aarch64  -> aarch64-apple-darwin
windows/x86_64 -> x86_64-pc-windows-msvc
windows/aarch64-> aarch64-pc-windows-msvc
```

Be conservative about Rust's `std::env::consts::ARCH == "arm"`: do not claim an ARMv7 asset for an incompatible ARM ABI without a reliable runtime qualifier. If exact differentiation is not available in-process, mark generic 32-bit ARM unsupported and let the shell installer handle `uname -m=armv7l`; document the limitation rather than installing a wrong binary.

Add a regression test that compares this module's public asset names with the phase-6 release matrix/fixture.

### 2. Add semantic version handling

Use a small well-maintained SemVer implementation (`semver` crate preferred) rather than ad-hoc lexical comparison.

Policy:

- parse current `CARGO_PKG_VERSION`;
- parse crates.io `max_stable_version`;
- require a normal stable release for automatic update (no prerelease unless a future explicit channel design exists);
- current == latest -> `AlreadyCurrent`;
- current < latest -> update available;
- current > latest -> report local version is ahead and do nothing.

Do not reinterpret malformed registry data as an update.

### 3. Query crates.io with existing HTTP infrastructure

Eggsearch already depends on async `reqwest` and `serde_json`; do not shell out to curl from the Rust updater solely for HTTP.

Use a bounded request to:

```text
GET https://crates.io/api/v1/crates/eggsearch
```

with an identifiable User-Agent, a short timeout, bounded response body, and normal TLS validation.

Read `crate.max_stable_version` as the stable-version authority. Fail clearly if the field is absent/malformed/non-stable.

Support an internal/test base URL injection through dependency injection/configuration that is not exposed as an unsafe general production override unless the repo already has an established pattern for testable HTTP endpoints.

### 4. Add `src/update.rs`

Define typed outcomes/errors rather than printing deep inside helpers. Suggested outcomes:

```text
AlreadyCurrent { version }
LocalVersionAhead { current, registry }
UpdateAvailable { current, latest }       # --check
UpdatedBinary { from, to }
UpdatedFromCargo { from, to }
UpdatedButRestartRequired { from, to, ... } # phase 9 may replace/extend this shape
```

Errors should distinguish:

- version lookup/parse;
- unsupported host;
- asset absent;
- download/network/status;
- checksum retrieval/parse/mismatch;
- candidate identity/version;
- permission denied;
- Cargo missing/failure;
- replacement failure.

Do not collapse these into a generic "update failed" because installer/fleet diagnostics depend on precise classification.

### 5. Download the exact release asset

For latest stable `X.Y.Z`, construct only:

```text
https://github.com/eggstack/eggsearch/releases/download/vX.Y.Z/eggsearch-<target>[.exe]
https://github.com/eggstack/eggsearch/releases/download/vX.Y.Z/eggsearch-<target>[.exe].sha256
```

Do not use `/releases/latest/` inside self-update.

Use bounded streaming download to a temporary file on the same filesystem as the destination when possible so the final replacement can be atomic.

Classify HTTP 404 specifically. Other 4xx/429/5xx and network failures are hard failures, not Cargo-fallback triggers.

### 6. Verify SHA-256 in-process

Add a direct SHA-256 dependency if needed (`sha2` or equivalent small audited crate).

Checksum contract:

- checksum response is bounded and must contain exactly a valid 64-hex digest plus optional expected filename in the documented release format;
- calculate digest over the complete downloaded candidate;
- compare normalized values;
- mismatch is a hard integrity failure;
- delete temporary candidate on failure.

Do not execute the file before checksum verification.

### 7. Verify candidate identity and exact version

After checksum passes:

- set executable permission on Unix temp candidate;
- execute only `candidate --version` with bounded output/timeout where feasible;
- require output to identify `eggsearch` and exact target version `X.Y.Z`;
- optionally run `candidate --help` if it materially improves identity validation without complicating Windows replacement;
- reject any candidate that exits nonzero or reports a different version.

The replacement target is `std::env::current_exe()`, not a guessed PATH entry.

### 8. Permission preflight

Before downloading a large asset when practical, determine whether the current executable can be safely replaced by the current identity.

If replacement requires elevation, return an actionable error such as:

```text
permission denied replacing /usr/local/bin/eggsearch
rerun:
  sudo /usr/local/bin/eggsearch update
```

Never invoke `sudo` internally.

The exact elevated command must use the resolved current executable path and preserve only safe, necessary arguments.

### 9. Replace safely on Unix and Windows

Use a proven cross-platform executable self-replacement mechanism (`self-replace` or equivalent) unless a smaller implementation is demonstrably correct on all supported targets.

Requirements:

- final candidate already verified before replacement;
- replacement either leaves the old executable intact or installs the complete new candidate;
- no in-place truncate/write of the running binary;
- Windows running-image constraints are explicitly tested on x86-64 and ARM64 where the target is supported;
- temporary/backup files are cleaned after success and bounded after failure.

If the chosen library has platform caveats, document them in the phase closure evidence.

### 10. Cargo fallback builds into a temporary root

When the exact asset is absent/unsupported and `cargo` is available, build the exact registry version into a temporary installation prefix rather than letting Cargo choose an unrelated user-global destination:

```text
cargo install eggsearch --version =X.Y.Z --locked --root <temp-prefix>
```

Then:

1. locate `<temp-prefix>/bin/eggsearch[.exe]`;
2. verify exact version/identity as for a downloaded candidate;
3. replace `current_exe()` through the same replacement path.

This avoids the common failure where a binary installed in `/usr/local/bin` is "updated" by Cargo into `~/.cargo/bin` while the running PATH still resolves the old executable.

If Cargo is absent, print exact Rust/Cargo/manual release instructions and fail.

A Cargo compilation error is not retried through alternate implicit strategies.

### 11. Preserve installation provenance only if useful

Do not add a database merely to remember whether the binary originated from Cargo or GitHub. The updater is intentionally location-based: it updates the currently executing binary.

If a tiny sidecar is needed later for service state, phase 9 may introduce it. Do not invent state in phase 7 without a concrete need.

### 12. Prepare restart integration seam

Phase 7 should keep update orchestration structured so phase 9 can implement:

```text
was persistent service running before update?
  yes -> replace -> restart same manager -> health verify
  no  -> replace -> leave stopped
```

Do not fake this in phase 7 by killing arbitrary `eggsearch` processes. A stdio child process must never be treated as a managed persistent service.

## Tests

Add deterministic tests for:

- stable semantic version parsing/comparison;
- prerelease rejection;
- current==latest/current<latest/current>latest outcomes;
- crates.io response missing/malformed `max_stable_version`;
- response body cap;
- all supported host/target mappings;
- exact asset/checksum URL construction;
- Windows `.exe` suffix;
- unsupported-host behavior;
- HTTP 404 -> Cargo fallback eligibility;
- 403/429/500/timeout -> no fallback;
- checksum parse/match/mismatch;
- candidate version match/mismatch;
- permission error rendering;
- Cargo command includes exact version, `--locked`, and isolated `--root`;
- successful mocked binary replacement using a test fixture/helper that never overwrites the test runner itself.

Where replacement cannot be safely unit-tested in-process, factor the filesystem swap primitive so it can operate on ordinary fixture paths, and retain a platform-specific integration smoke for actual self-replacement in release/ignored CI.

## Platform smoke

On release-capable runners, exercise an update integration scenario using temporary copies/fixture release endpoints:

1. stage an older fixture executable/copy;
2. serve registry/release metadata from a local HTTP fixture;
3. invoke updater logic against the staged path through a test harness;
4. verify replacement version and executable behavior.

Do not require mutating the real workflow runner's installed eggsearch binary.

Windows must receive explicit replacement smoke because running executable replacement semantics differ materially from Unix.

## Documentation changes

Add/update:

- `README.md`: `eggsearch update` brief usage;
- `docs/installation.md`: relationship between bootstrap installer and updater;
- `docs/update.md`: version authority, exact release URLs, fallback policy, permission behavior, no downgrade/prerelease policy, restart behavior marked as pending until phase 9;
- `docs/release.md`: release asset contract is consumed by updater and must not drift;
- `CHANGELOG.md`.

## Acceptance criteria

Phase 7 is complete only when:

1. `eggsearch update --check` reports current/latest without mutation;
2. equal stable versions exit successfully without download/replacement;
3. a newer mocked crates.io stable version selects the exact matching `vX.Y.Z` GitHub asset;
4. a valid binary + checksum + exact candidate version replaces a fixture executable safely;
5. checksum mismatch cannot execute or replace the candidate;
6. exact asset 404 enters exact-version Cargo fallback;
7. transient GitHub/network failures do not enter Cargo fallback;
8. Cargo fallback compiles to an isolated root and replaces the current target path rather than merely installing elsewhere;
9. missing Cargo on a fallback-required host produces actionable instructions;
10. insufficient replacement permission produces the exact elevated rerun command without invoking elevation;
11. local version newer than crates.io is not downgraded;
12. Windows replacement smoke passes on supported native Windows target(s);
13. Unix replacement smoke passes;
14. phase 6 target/asset contract and updater mapping agree under tests;
15. `make check` passes on the exact final candidate;
16. `registry.md` and this phase status are updated in the closure commit.
