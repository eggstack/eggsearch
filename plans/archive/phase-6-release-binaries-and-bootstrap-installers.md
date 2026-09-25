# Phase 6 — Release Binaries and Bootstrap Installers

Status: implemented
Depends on: none
Baseline for planning: `f595683b8ebdec0afb13363ec9e8ad7654f9824b` (`eggsearch` 0.3.8)
Roadmap: `plans/deployment-roadmap.md`
Reference implementation: `eggstack/gregg` release-binary and installer work

## Objective

Establish a stable GitHub Release binary contract for common desktop and SBC platforms and provide copy/paste bootstrap installers that install a verified prebuilt executable whenever possible, with exact-version Cargo compilation as a narrow fallback.

This phase should solve the immediate deployment problem: installing eggsearch on Raspberry Pi/Le Potato class systems must normally be a download rather than a local Rust compile.

Persistent MCP service registration is not implemented here. The installer may reserve a future `--service` flag only if it fails clearly until phase 9 lands; preferably add the flag in phase 9 so documentation never advertises unavailable behavior.

## Non-goals

- Do not add apt, RPM, Homebrew, Winget, Scoop, Chocolatey, MSI, PKG, Docker, or other package-distribution pipelines.
- Do not add service managers, `croncheck`, restart, or persistent HTTP in this phase.
- Do not publish board-branded binaries.
- Do not publish separate `pdf` or `browser` feature variants.
- Do not make GitHub Actions publish the crate to crates.io; crate publication remains an explicit maintainer action.

## Invariants

1. Release asset naming is stable and shared by workflow, installers, later updater code, and documentation.
2. Release binaries use eggsearch default features and the exact tagged crate version.
3. Linux GNU artifacts have an intentional libc portability floor.
4. A checksum must be verified before a downloaded candidate is executed.
5. Candidate `eggsearch --version` must match the requested release before installation.
6. Cargo fallback is used only for an unsupported target or a confirmed 404 for the exact asset; transient download/integrity failures are hard failures.
7. The installer never invokes `sudo` itself.
8. Normal `make check` stays network-free; release workflow jobs may use GitHub/crates.io and execute produced artifacts.

## Public target and asset contract

Use the following target-to-asset mapping:

| Host | Rust target | Asset |
|---|---|---|
| Linux x86-64 | `x86_64-unknown-linux-gnu` | `eggsearch-x86_64-unknown-linux-gnu` |
| Linux ARM64 | `aarch64-unknown-linux-gnu` | `eggsearch-aarch64-unknown-linux-gnu` |
| Linux ARMv7 hard-float | `armv7-unknown-linux-gnueabihf` | `eggsearch-armv7-unknown-linux-gnueabihf` |
| macOS Intel | `x86_64-apple-darwin` | `eggsearch-x86_64-apple-darwin` |
| macOS Apple Silicon | `aarch64-apple-darwin` | `eggsearch-aarch64-apple-darwin` |
| Windows x86-64 | `x86_64-pc-windows-msvc` | `eggsearch-x86_64-pc-windows-msvc.exe` |
| Windows ARM64 | `aarch64-pc-windows-msvc` | `eggsearch-aarch64-pc-windows-msvc.exe` |

Every executable must have an adjacent `<asset>.sha256` file containing the SHA-256 digest and filename in a standard one-line format. Release assets also include `install.sh` and `install.ps1`.

Do not include the version in the asset filename. The release tag `vX.Y.Z` is the version namespace.

## Production changes

### 1. Add `packaging/` as the release/deployment source tree

Create at minimum:

```text
packaging/
  README.md
  install.sh
  install.ps1
```

Later phases may add `packaging/systemd`, `packaging/launchd`, and Windows startup assets. Keep the installer source in the repository and attach the exact reviewed script bytes to each GitHub Release.

If `Cargo.toml` packaging exclusions/includes would accidentally ship `packaging/` in the crate, make the intended crate-package behavior explicit. The bootstrap scripts do not need to be present after `cargo install` because startup templates that must survive Cargo install will be embedded in the binary in phase 9.

### 2. Add a release-only workflow

Create `.github/workflows/release-binaries.yml` separate from `.github/workflows/ci.yml`.

Triggers:

```text
push tags: v*
workflow_dispatch: explicit existing tag input
```

Permissions should be least-privilege: `contents: write` only where the release-assembly job needs it; add the minimal `id-token`/attestation permission only if artifact attestations are implemented.

Ordinary pushes/PRs must not build the whole release matrix.

### 3. Add release preflight

Before expensive matrix jobs:

- resolve the triggering tag;
- require `v<semver>` form;
- read `package.version` from the root `Cargo.toml` and require exact equality with the tag after removing `v`;
- require the checked-out commit to be the tagged commit;
- ensure checkout state is clean;
- verify `Cargo.lock` is present and builds use `--locked`;
- query crates.io for `eggsearch/<version>` and require the exact version to be visible before GitHub binary publication;
- print actionable failure text for registry-index delay rather than silently creating assets for an unpublished crate.

This preserves the existing release order: prepare -> `cargo publish --locked` -> tag/push -> GitHub binary workflow.

### 4. Linux x86-64 and AArch64 portability builds

Prefer the already-proven Gregg release strategy:

```text
cargo-zigbuild + pinned Zig
x86_64-unknown-linux-gnu.<glibc-floor>
aarch64-unknown-linux-gnu.<glibc-floor>
```

Use glibc 2.17 as the initial target floor unless implementation evidence shows a dependency/toolchain incompatibility requiring a later floor. Any change to the floor must be intentional, documented in `docs/installation.md`/`docs/release.md`, and tested against representative old/SBC distributions.

Use native GitHub ARM runners for the AArch64 job when available so the built binary can be executed directly. Do not cross-build AArch64 and skip runtime verification when a native runner is available.

### 5. ARMv7 feasibility and release gate

ARMv7 is high-value specifically because local compile time on older SBCs is costly. Treat it as a release requirement with a technical qualification gate, not as a casual best-effort matrix row.

Implementation must:

1. determine whether `cargo-zigbuild` can produce `armv7-unknown-linux-gnueabihf` with an acceptable glibc floor for the current dependency graph;
2. if not, evaluate a pinned `cross`/container sysroot or similarly small cross-compilation mechanism;
3. verify architecture/linker output (`file`, dynamic loader/libc expectations, or equivalent);
4. execute at least `eggsearch --version` and `eggsearch --help` under QEMU/user emulation or an equivalent ARMv7 smoke environment before publishing;
5. if runtime smoke cannot be made trustworthy, do not publish the asset and leave phase 6 open with the exact blocker and proposed follow-up.

Do not substitute an untested binary merely to make the matrix look complete.

### 6. macOS builds

Build Intel and Apple Silicon targets on native macOS runners where practical. Each produced executable must pass:

```text
eggsearch --version
eggsearch --help
```

No code signing/notarization program is required for this phase. Document that GitHub release binaries are unsigned if that remains true. Do not add ad-hoc signing with unmanaged secrets.

### 7. Windows builds

Build native x86-64 with MSVC. Build Windows ARM64 on a native ARM64 runner when available and when the dependency graph compiles cleanly.

Each Windows artifact must pass PowerShell-native smoke:

```text
.\eggsearch-<target>.exe --version
.\eggsearch-<target>.exe --help
```

If ARM64 is blocked by a specific dependency/toolchain issue, record that blocker in phase status and installer docs; do not misclassify ordinary ARM64 Windows as x86-64.

### 8. Add an MCP stdio artifact smoke

For every architecture that can execute natively in CI, verify more than CLI startup. Add a small deterministic release-smoke helper that launches the staged executable as:

```text
eggsearch mcp stdio
```

and performs an MCP initialize handshake plus `tools/list`, checking that the server identity/version is correct and the expected baseline tools are exposed.

For emulated ARMv7, perform the same smoke if reliable under the chosen emulation. At minimum require CLI execution if protocol I/O through QEMU proves unstable for CI-specific reasons, and document that exception.

This smoke must use no provider credentials and no external search request.

### 9. Generate checksums after all binary mutation

Stage each final executable under the public asset name, set executable mode on Unix, run identity/version/protocol smoke, then compute SHA-256. Do not strip/sign/modify the file after hashing.

Each job should verify its generated checksum locally before artifact upload.

### 10. Add artifact provenance if low-friction

Use GitHub Artifact Attestations/Sigstore-backed build provenance for release executables when supported by the repository's Actions permissions without adding a bespoke key-management system.

Checksums remain required even if attestations are added. Installer verification in this workstream uses SHA-256; attestation verification may be documented as an optional maintainer/operator verification path.

### 11. Assemble one GitHub Release

A final release job downloads all per-target workflow artifacts and attaches:

- required executables;
- corresponding `.sha256` files;
- `install.sh`;
- `install.ps1`.

Prefer creating/updating a draft release for the exact tag and fail if required artifacts are missing. The workflow must be safe to rerun for the same tag without silently mixing bytes from different commits.

If the project intentionally wants release publication to remain manual, assemble a draft rather than auto-publishing. Record the final policy in `docs/release.md`.

## Unix installer contract

### 12. Implement `packaging/install.sh`

Requirements:

- Bash with an early POSIX-safe guard that explains `bash` is required if piped to `sh`;
- `set -euo pipefail` after the guard;
- repository constant fixed to `eggstack/eggsearch`;
- optional `--version X.Y.Z` pin; otherwise use `releases/latest/download` for bootstrap convenience;
- detect `uname -s` and `uname -m` and map only known aliases;
- map 64-bit Raspberry Pi/Le Potato (`aarch64`/`arm64`) to `aarch64-unknown-linux-gnu`;
- map `armv7l` to `armv7-unknown-linux-gnueabihf`;
- install to `/usr/local/bin` when already root, otherwise `$HOME/.local/bin`;
- never call `sudo`;
- print PATH advice if the selected user-local directory is not on PATH;
- require `curl` for binary bootstrap;
- verify SHA-256 with `sha256sum` or `shasum -a 256`;
- execute the temporary candidate with `--version` only after checksum verification;
- require the candidate output to identify `eggsearch` and, for pinned installs, the exact requested version;
- install atomically where practical (`install` to temporary sibling + rename, or equivalent) rather than leaving a truncated destination on failure.

### 13. Cargo fallback semantics

On unsupported architecture or confirmed asset HTTP 404:

- require `cargo` in PATH;
- install the exact pinned version when `--version` was supplied;
- for latest bootstrap, allow `cargo install eggsearch --locked` because crates.io is then the source of latest stable selection;
- prefer an explicit install root matching the intended destination where Cargo semantics permit it;
- verify the resulting executable with `--version`;
- surface a clear Rust installation instruction when Cargo is absent.

Do not fallback to Cargo for:

- DNS/TLS/connect timeout;
- GitHub 401/403/429/5xx;
- checksum-file download failure;
- checksum mismatch;
- candidate execution failure;
- candidate identity/version mismatch.

A 404 should be classified with an explicit HTTP status probe rather than treating every `curl` failure as missing asset.

## Windows installer contract

### 14. Implement `packaging/install.ps1`

Requirements mirror Unix semantics:

- optional `-Version X.Y.Z`;
- detect process/OS architecture without mapping ARM64 to AMD64;
- choose `%ProgramFiles%\Eggsearch` when already Administrator and `%LOCALAPPDATA%\Eggsearch` otherwise;
- use `Invoke-WebRequest`/supported PowerShell HTTP primitives;
- verify SHA-256 with `Get-FileHash` before executing the candidate;
- verify `--version` before install;
- Cargo fallback only for unsupported/404 cases;
- no automatic UAC/elevation request;
- print PATH advice when needed;
- use a replacement/install operation that cannot leave a partial executable on ordinary failure.

Service installation is phase 9 and must not be duplicated here.

## Deterministic and workflow tests

Add focused tests/helpers for:

- host alias -> Rust target mapping;
- target -> public asset name mapping;
- `.exe` suffix behavior;
- pinned tag URL and latest URL construction;
- unsupported target classification;
- fallback policy classification (404 vs transient/integrity failure);
- checksum parsing;
- candidate version parsing;
- installer argument parsing where practical.

Keep pure mapping logic in one small source-of-truth location where it can be shared or cross-checked by later Rust updater tests. It is acceptable for shell/PowerShell to duplicate a small target table, but add a release test that compares installer-declared public targets with the workflow/Rust contract so drift is caught.

## Documentation changes

Update:

- `README.md`: make the copy/paste Unix installer the primary installation path, retain `cargo install eggsearch` as source fallback/manual option, and add the PowerShell command;
- `docs/installation.md`: supported target matrix, install destinations, glibc floor, checksums, pinned installs, fallback rules, unsigned macOS note if applicable;
- `docs/release.md`: exact release ordering, tag/version preflight, target matrix, asset contract, rerun behavior, draft/publish policy;
- `CHANGELOG.md` when implementation lands;
- architecture/packaging docs if the repository maintains a deployment architecture index.

## Implementation note

The release contract, installers, local contract tests, and artifact smoke harness are implemented. The first tagged release created from this implementation is the hosted qualification candidate for the full GitHub Actions target matrix; the existing `v0.3.8` tag predates these assets and is intentionally not reused.

Do not document service installation until phase 9 is implemented.

## Acceptance criteria

Phase 6 is complete only when all of the following are exercised against the exact candidate:

1. release workflow can be manually dispatched for an existing test/release tag without ordinary CI changes;
2. tag and `Cargo.toml` version mismatch fails before matrix builds;
3. crates.io missing-version preflight fails clearly;
4. Linux x86-64 and AArch64 artifacts use the documented libc floor and execute `--version`/`--help`;
5. ARMv7 is either published only after architecture/runtime smoke or the phase remains non-complete with a concrete blocker;
6. macOS Intel/Apple Silicon and Windows x86-64 artifacts execute on native runners;
7. Windows ARM64 executes on a native runner or has a documented technical blocker and is clearly source-fallback in installer docs;
8. runnable target jobs pass a keyless MCP initialize + `tools/list` smoke;
9. every published executable has a verified SHA-256 file;
10. `install.sh` installs a mocked or real matching binary, rejects a checksum mismatch, and uses Cargo only for unsupported/404 cases;
11. `install.ps1` exercises the equivalent Windows behavior;
12. installed binary reports the expected version;
13. README copy/paste commands match the actual attached installer paths;
14. `make check` passes on the final candidate;
15. `registry.md` and this phase status are updated in the closure commit.
