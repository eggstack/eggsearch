# Release packaging architecture

**Location:** `packaging/`, `.github/workflows/release-binaries.yml`, `src/platform.rs`, `src/update.rs`

Release packaging adds a binary distribution boundary without changing the
MCP application architecture. The crate remains one default-feature binary;
`eggsearch mcp stdio` remains the client-owned lifecycle and
`eggsearch mcp serve` is the explicit foreground persistent lifecycle.
`src/platform.rs` is the Rust mirror of the target contract, `src/update.rs`
is the binary-first self-update client, and `src/startup.rs` owns the
post-update restart hook. Operator install steps live in `docs/installation.md`.

## Target contract

`packaging/release-targets.txt` is the compact contract table with rows of
`rust-target|public-asset|host-family|architecture`. Seven rows, fixed:

| Rust target | Public asset | Host family | Arch |
|-------------|--------------|-------------|------|
| `x86_64-unknown-linux-gnu` | `eggsearch-x86_64-unknown-linux-gnu` | `linux` | `x86_64` |
| `aarch64-unknown-linux-gnu` | `eggsearch-aarch64-unknown-linux-gnu` | `linux` | `aarch64` |
| `armv7-unknown-linux-gnueabihf` | `eggsearch-armv7-unknown-linux-gnueabihf` | `linux` | `armv7` |
| `x86_64-apple-darwin` | `eggsearch-x86_64-apple-darwin` | `darwin` | `x86_64` |
| `aarch64-apple-darwin` | `eggsearch-aarch64-apple-darwin` | `darwin` | `aarch64` |
| `x86_64-pc-windows-msvc` | `eggsearch-x86_64-pc-windows-msvc.exe` | `windows` | `x86_64` |
| `aarch64-pc-windows-msvc` | `eggsearch-aarch64-pc-windows-msvc.exe` | `windows` | `aarch64` |

Executable names are stable and carry no version; the tag `vX.Y.Z` is the
version namespace. Windows assets keep the `.exe` suffix, non-Windows assets
never do. Linux GNU builds use pinned Zig/cargo-zigbuild with a glibc 2.17
floor. Default features build every release asset; PDF, browser, and egress
features are never separate binary products.

`src/platform.rs` mirrors the same seven rows as `RELEASE_TARGETS` with
`rust_target`, `asset`, normalized `os` (`macos` for the `darwin` rows), and
`arch`. `target_for_host()` normalizes common aliases before lookup
(`darwin` to `macos`, `amd64`/`x64` to `x86_64`, `arm64` to `aarch64`,
`armv7l`/`armv7` to `armv7`) and returns `None` for unsupported hosts such as
`freebsd` or bare `arm`. `current_target()` resolves the running process via
`std::env::consts`. `asset_url()` and `checksum_url()` build the exact
`.../releases/download/v{version}/{asset}` and `{asset}.sha256` URLs from the
GitHub base. Never invent target or asset names: the txt file, the Rust
constant, the workflow matrix, both installers, and the install docs must
agree exactly.

## Release inputs and validation entry points

`packaging/release-inputs.txt` lists every path that must exist in the release
checkout: `Cargo.toml`, `Cargo.lock`, `packaging/release-targets.txt`,
`packaging/release-smoke.sh`, `packaging/release-smoke.ps1`,
`packaging/install.sh`, `packaging/install.ps1`,
`packaging/release-validate.sh`, `packaging/check-contract.sh`,
`packaging/test-install.sh`, `packaging/test-install.ps1`.

`packaging/release-validate.sh` has three subcommands: `tree [COMMIT]`
checks every input path exists, `candidate` additionally requires the Cargo
package version to be release-shaped, and
`assets DIST MODE COMMIT TAG VERSION` proves exact set equality for the
expected 16 files (seven binaries, seven checksums, two installers), prints
expected versus observed names with missing/unexpected diffs, re-verifies
every checksum, and requires the Unix binaries to be executable.
`packaging/check-contract.sh` runs the full keep-in-sync gate locally (see
below). `packaging/check-egress-qualify-contract.sh` plus
`tests/egress_qualify_contract.rs` guard the separate non-publishing egress
feature qualification across the same seven targets.

## Release workflow

`.github/workflows/release-binaries.yml` runs on `v*` tag pushes or
`workflow_dispatch` with `mode=qualify|release`. Dispatch takes an exact
branch/SHA `ref` for qualification or an exact `vX.Y.Z` `tag` for release.
Qualification resolves and prints one immutable `QUALIFIED_SHA`, runs before
crates.io publication, and uploads only a qualification-labelled artifact. It
never creates or edits a GitHub Release.

Preflight checks the candidate checkout: all `release-inputs.txt` paths
present, tree clean, version read from locked cargo metadata. Release mode
additionally requires the tag to be SemVer-shaped, the checkout to be the
commit named by the tag, the Cargo version to equal the tag without the `v`
prefix, and the exact crate version to be visible on crates.io. Every build
job checks out the resolved preflight commit, never a moving branch.

Per-target jobs:

- Linux x86-64 and ARM64 use pinned cargo-zigbuild against the
  `<target>.2.17` sysroot, assert the ELF class, reject any binary needing a
  glibc newer than 2.17, then run `packaging/release-smoke.sh`.
- Linux ARMv7 builds the same way, asserts 32-bit ARM ELF, and smokes
  `--version` plus `--help` under QEMU (`arm32v7/ubuntu:20.04`). Its protocol
  path is intentionally not release evidence; the hosted job runs the full
  CLI smoke while other targets run the full MCP smoke.
- macOS Intel and Apple Silicon build natively on separate runners, then run
  the Unix smoke script.
- Windows x86-64 and ARM64 build natively on separate runner labels, then run
  `packaging/release-smoke.ps1`, write lowercase `<asset>.sha256`, and hash
  twice to prove the checksum round-trips.

Every native job runs `--version`, `--help`, keyless MCP stdio initialize
plus `tools/list`, and loopback Streamable HTTP health/initialize plus
`tools/list` smoke through the shared smoke scripts.

## Checksums

Every executable ships with a sibling `<asset>.sha256` file containing one
lowercase hex line in `"<sha256>  <asset>\n"` form. Build jobs generate the
file with `sha256sum`/`shasum -a 256`/`Get-FileHash` and immediately verify
with the matching check command. The assembler re-verifies all seven
checksums from the same workflow run before attaching anything. The updater
parses the same single-line form strictly: exactly one line, 64 hex chars,
optional filename that must equal the requested asset. Anything else is a
hard checksum failure with no retry and no fallback.

## Installers

`packaging/install.sh` (bash-only, refuses `sh`) and `packaging/install.ps1`
share one bootstrap policy:

1. Map only known host aliases to the public target contract; unknown hosts
   fail with no download.
2. Resolve the exact `vX.Y.Z` asset and checksum URLs for the requested or
   default version (default follows the registry authority).
3. Download the binary and checksum, verify SHA-256 before executing the
   candidate.
4. Require the candidate to identify as `eggsearch <pinned-version>` via
   `--version`; identity or version mismatch stops immediately.
5. Atomically replace the destination (`$HOME/.local/bin` user-local, or
   `/usr/local/bin` when root) and preserve no partial file on failure.

Transport, authorization, rate-limit, checksum, execution, and identity
failures are hard stops. Cargo is invoked only for an unsupported target or a
confirmed binary HTTP 404, as `cargo install eggsearch --version =X.Y.Z
--locked` (Unix) or the `Invoke-WebRequest` plus `Get-FileHash -Algorithm
SHA256` equivalent path on Windows. Installers never invoke `sudo`, never
request UAC elevation, never render manager definitions, and never create a
cron fallback when a preferred manager lacks privilege. With explicit
`--service` they invoke the installed binary's own `startup install`
command after replacement.

`packaging/test-install.sh` and `packaging/test-install.ps1` exercise the
packaging behavior (installer flags, service hook, refusal cases) rather than
behavioral search/fetch suites.

## Artifact smoke

`packaging/release-smoke.sh` (Unix) and `packaging/release-smoke.ps1`
(Windows) take `BINARY EXPECTED_VERSION`. They assert `--version` prints
`eggsearch <expected>`, `--help` succeeds, stdio initialize plus `tools/list`
exposes the full 10-tool set, and the loopback server answers `/healthz`,
initialize, and `tools/list`. ARMv7 runs the version/help subset under QEMU
with `--skip-mcp` semantics because emulated sockets are not trusted release
evidence.

## Draft assembly

The `assemble` job needs all five build jobs green, downloads every
per-target artifact with `merge-multiple`, copies the reviewed installer
bytes into `dist/`, normalizes Unix executable bits per the target table,
and runs `release-validate.sh assets` for exact 16-file set equality.
Qualify mode uploads a `qualification-<version>-<commit>-complete` artifact
and stops. Release mode creates (or reuses, only when still a draft) a draft
GitHub Release titled `eggsearch <version>` and uploads `dist/*` with
`--clobber`. A published release is never overwritten, no job publishes the
crate, and no partial matrix is ever silently published.

## Self-update: check versus update

`src/update.rs` owns binary-first self-update with no elevation on any path.
The registry authority is crates.io `crate.max_stable_version`; pre-release
versions are ineligible. Bounded clients cap the registry body (64 KiB),
checksum body (4 KiB), candidate output (16 KiB), and release asset
(128 MiB), with 20-second request, 10-second candidate, and 30-minute Cargo
timeouts.

`eggsearch update --check` is registry-only and non-mutating: fetch the
registry document, compare semver, and report `AlreadyCurrent`,
`LocalVersionAhead` (never downgrades), or `UpdateAvailable`. No bytes are
downloaded and no filesystem state changes.

A normal `eggsearch update` resolves the current host target, requests the
exact `vX.Y.Z` asset plus checksum, streams the download with a size cap,
verifies SHA-256, `chmod 0755` on Unix, and executes the staged candidate
with `--version` under a byte cap to prove it identifies as
`eggsearch <requested-version>`. Only then does it replace
`std::env::current_exe()` through `self_replace` (same-exe path) or an
fsync-staged atomic persist (explicit destination path). Only two situations
may enter the isolated exact-version Cargo fallback
(`cargo install eggsearch --version =X.Y.Z --locked --root <tempdir>`):
an unsupported host with no release target, or a confirmed exact-asset HTTP
404. Transient network errors, non-404 statuses, checksum mismatches, and
identity mismatches are hard stops. A pre-replacement writability probe turns
permission failures into a `PermissionDenied` error carrying the exact
elevated rerun command; the updater itself never elevates.

## Restart hook

`src/startup.rs` owns the canonical `mcp serve --bind 127.0.0.1:11320 --path
/mcp` runtime. After a successful replacement the updater queries
`startup_state()` and restarts only a previously healthy, singly-registered
persistent service (running manager, or cron) through its registered manager:
`systemctl restart`, `launchctl kickstart -k`, the cron identity-safe
restart, or the Windows SCM path, followed by a `/healthz` poll. Stdio
processes are client-owned and never restarted. Outcomes surface as
`UpdatedBinary`, `UpdatedFromCargo`, or `UpdatedAndRestarted`; a failed
restart keeps the new binary installed and returns `RestartFailed` with the
exact `restart` command to rerun. `restart_command()` renders that command
from the current runtime spec.

## Keep-in-sync rule

The seven target rows, the Rust `RELEASE_TARGETS`, the workflow matrix (plus
the explicit ARMv7 zigbuild line), both installer host maps, the updater
resolution, and `docs/installation.md` must move together. `make
packaging-check` runs `packaging/check-contract.sh`, which fails closed when
any of these drift: row shape or count, workflow target/asset order versus
`release-targets.txt` (ARMv7 handled as the dedicated QEMU job), unique
per-target artifact names, merged exact asset validation presence, installer
and Rust pair equality, docs coverage of all seven pairs, `.exe` suffix
discipline, Cargo-fallback and `Get-FileHash` marker presence, no-`sudo`
installer discipline, bash guard behavior, and the egress qualification
contract. Change targets, inputs, workflow, matrix, installers, updater, or
install docs in the same change or the gate fails.
