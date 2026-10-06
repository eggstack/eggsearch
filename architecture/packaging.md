# Release packaging architecture

**Location:** `release/eggpack/`, `packaging/`, `.github/workflows/release-eggpack.yml`, `.github/workflows/release-provenance.yml`, `src/platform.rs`, `src/update.rs`

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
expected 19 files (seven binaries, seven checksums, two product wrappers, the
Eggpack release manifest, two generated exact installers), derived from the
distribution contract by `expected_release_assets.py`, prints
expected versus observed names with missing/unexpected diffs, re-verifies
every checksum, and requires the Unix binaries to be executable.
`packaging/check-contract.sh` runs the full keep-in-sync gate locally (see
below). It also fails closed on two crate-wide invariants that are not about
release targets: `packaging/check-workflow-pins.py` requires every non-local
`uses:` in `.github/workflows/*.yml` to be pinned to a full 40-hex commit SHA,
and `packaging/check-planning-consistency.py` rejects closed-range shorthand
in planning status tables. `packaging/check-egress-qualify-contract.sh` plus
`tests/egress_qualify_contract.rs` guard the separate non-publishing egress
feature qualification across the same seven targets.

Dependency policy is a sibling gate, not part of the release tree:
`packaging/check-dependency-policy.sh` runs `check-dependency-policy.py` and
`check-http-dependency-shape.py`, installs pinned `cargo-deny 0.20.2` on
demand, and runs `cargo deny check` against the root `deny.toml`. It is a
`make check` step and is replayed weekly by
`.github/workflows/dependency-security.yml`.

## Publication state

The first binary release was `v0.3.9` at `0cbbeee7`. `v0.4.0` is crate and
tag only — it never received a GitHub Release and is never revisited.
`v0.4.1` is the current version in `Cargo.toml` and is published as an
immutable 16-asset release (seven binaries, seven checksums, two installers)
carrying a SLSA provenance attestation over all 16 names; a published release
is never overwritten. Historical releases stay 16 assets; only releases
produced after the Eggpack cutover carry the 19-file inventory. Treat `plans/registry.md` and `CHANGELOG.md` as the authority for
which milestone is open next.

## Producer authority: Eggpack

Release production is owned by Eggpack. `release/eggpack/` holds the complete,
static, identity-free authority set:

| File | Owns |
|---|---|
| `distribution.toml` | seven targets, asset names, install names, checksum sidecars |
| `pack.toml` | build strategy, host, floor, qualification intent per target |
| `build-bindings.toml` | one direct Cargo output per target |
| `qualification-bindings.toml` | bounded core smoke per executing target |
| `consumer-validators.json` | required product-owned validator per target |
| `workflow-shape.json` | the static render seam (derived, not hand-maintained) |
| `github-policy.json` | runners, action pins, release inputs, staging, cross-tool provisioning |
| `github-template.json` | draft release template |
| `install-policy.toml` | empty: every target is a direct artifact |
| `installer-presentation.json` | product wrappers are the public install surface |

`packaging/gen-release-workflow-shape.py` derives `workflow-shape.json` from
the other files. Hand-maintaining it would make it a second copy of every
producer fact, which is exactly the drift this arrangement exists to remove.

`.github/workflows/release-eggpack.yml` is **generated**. Never edit it by
hand; regenerate it and prove the result:

```bash
eggpack ci generate \
  --workflow-shape release/eggpack/workflow-shape.json \
  --contract release/eggpack/distribution.toml \
  --github-policy release/eggpack/github-policy.json \
  --output .github/workflows/release-eggpack.yml
eggpack ci check \
  --workflow-shape release/eggpack/workflow-shape.json \
  --contract release/eggpack/distribution.toml \
  --github-policy release/eggpack/github-policy.json \
  --workflow .github/workflows/release-eggpack.yml
```

`.github/workflows/release-drift.yml` runs that `ci check` on every push and
pull request, so drift fails required CI rather than a release.

The generated workflow is `workflow_dispatch`-only and requires an exact
existing `release_tag`. It renders 27 jobs: `preflight`, `resolve`, seven
`build_*`, seven `qualify_build_*`, seven `validate_build_*` consumer
validators, `required_gate`, `aggregate`, and `stage`.

Per-target policy:

- Linux x86-64 and ARM64 build with cargo-zigbuild 0.23.3 against Zig 0.14.1 on
  the `<target>.2.17` sysroot and are qualified `native` on the matching runner.
- Linux ARMv7 builds the same way on x86-64 and is classified `structural`,
  because there is no hosted ARMv7 builder and a structural classification
  correctly executes nothing. It therefore has **no** core smoke binding; its
  runtime proof lives in the required product-owned
  `packaging/validate-armv7-candidate.py`, which asserts 32-bit ARM ELF
  identity, glibc <= 2.17, and `--version`/`--help` under a digest-pinned
  ARMv7 runtime. Because that validator is required, `required_gate` and
  `aggregate` fail closed if the ARMv7 proof does not pass.
- macOS Intel and Apple Silicon build natively on separate runners.
- Windows x86-64 and ARM64 build natively on separate runner labels
  (`windows-latest`, `windows-11-arm`); Windows ARM64 stays a required target.

Every natively-hosted target additionally runs the required
`packaging/validate-release-binary.py`, which proves exact workspace version
identity, `--help` success, and the glibc ceiling on ELF targets.

The generated workflow requests `contents: write` exactly once, in the `stage`
job, and never requests `id-token: write`, `attestations: write`, or
`artifact-metadata: write`. It contains no `--clobber` and no publish path.

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

## Draft staging

`aggregate` consumes every build, qualification, and consumer-validation
evidence handoff, enforces `required_gate`, and finalizes one release root plus
a `release-manifest.json` written **beside** that root, never inside it.

`stage` then prepares the staging payload and calls Eggpack's draft stager.
Its semantics are fixed and are not configurable:

- exact existing tag and exact source revision only;
- draft-only: a published or immutable release fails;
- absent asset -> upload; byte-identical asset -> reuse; differing asset -> fail;
- unexpected remote asset -> fail;
- no `--clobber`, no auto-publish, no tag mutation.

Operator recovery from a refused or incomplete draft is: inspect the refusal
and draft inventory, delete the stale draft **only** when intentionally
restarting the unpublished candidate, then rerun the same exact tag and source
once the draft is absent.

The resulting public inventory is exactly 19 files:

| Count | Files |
|---|---|
| 7 | versionless executables, unchanged public names |
| 7 | `.sha256` sidecars |
| 2 | `install.sh`, `install.ps1` (product wrappers, byte-identical to source) |
| 1 | `release-manifest.json` (Eggpack final-bytes evidence) |
| 2 | `install-exact.sh`, `install-exact.ps1` (Eggpack-rendered exact installers) |

The last three are additive producer evidence, not replacement entry points.
The public friendly install surface remains `install.sh` / `install.ps1`.

## Provenance

`.github/workflows/release-provenance.yml` is Eggsearch-owned and separate
from the Eggpack-generated writer, precisely so generated jobs never need an
OIDC write permission. It is `workflow_dispatch`-only and takes an exact
existing `release_tag`.

It requires the checkout to be the tag commit with a clean tree, requires the
GitHub Release to still be a draft, downloads the exact staged inventory
read-only, and then proves: the remote asset set is exactly the 19 names; each
file's local digest and size equal what GitHub reports; all seven
binary/checksum pairs self-verify; `release-manifest.json` declares the same
release id, source revision, and target inventory with matching artifact
digests and sizes; the public wrappers equal the checked-in wrapper bytes at
the tag; and both generated exact installers are present.

Only then does it attest, covering the historical subject set (seven binaries
plus the two public wrappers) and additionally the two generated exact
installers, and it verifies the results with `gh attestation verify`. It
mutates no tag, release metadata, or release asset. Publication stays a
separate human action.

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
per-target artifact names, merged exact asset validation presence, a
`packaging/` path referenced by the workflow that does not exist, installer
and Rust pair equality, docs coverage of all seven pairs, `.exe` suffix
discipline, Cargo-fallback and `Get-FileHash` marker presence, no-`sudo`
installer discipline, bash guard behavior, the egress qualification
contract, workflow action-SHA pinning, and planning-status consistency.
Change targets, inputs, workflow, matrix, installers, updater, or
install docs in the same change or the gate fails.
