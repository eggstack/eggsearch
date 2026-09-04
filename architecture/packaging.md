# Release packaging architecture

**Location:** `packaging/`, `.github/workflows/release-binaries.yml`

Phase 6 adds a binary distribution boundary without changing the MCP
application architecture. The crate remains one default-feature binary and
`eggsearch mcp stdio` remains the client-owned lifecycle.

## Contract

`packaging/release-targets.txt` is the compact contract table:

```text
rust-target|public-asset|host-family|architecture
```

The seven rows are mirrored in the release workflow and both installers.
Executable names are stable and do not contain a version; the tag
`vX.Y.Z` is the version namespace. Every executable is accompanied by a
standard `<asset>.sha256` file.

Linux GNU builds use pinned Zig/cargo-zigbuild with a glibc 2.17 floor.
Default features are used for every release asset. Optional PDF and browser
features are not separate binary products.

## Release workflow

The release-only workflow runs for `v*` tag pushes or an explicit
`workflow_dispatch` tag. Preflight checks:

- the tag is SemVer-shaped and exactly matches the root package version;
- the checkout is the commit named by the tag and is clean;
- `Cargo.lock` exists;
- the exact crate version is visible on crates.io.

Linux x86-64 and ARM64 use cargo-zigbuild; ARMv7 is built and checked under
QEMU before it can be attached. macOS uses native Intel and Apple Silicon
runners. Windows x86-64 and ARM64 use separate native runner labels. All
native jobs run `--version`, `--help`, and a keyless MCP initialize plus
`tools/list` smoke. The ARMv7 hosted job uses the full CLI smoke under QEMU;
its protocol path is intentionally not treated as reliable release evidence.

The assembler verifies all seven binaries and checksums from the same workflow
run, attaches the reviewed installer bytes, and creates or updates a draft
release. A published release is never overwritten. No GitHub job publishes the
crate or silently publishes a partial matrix.

## Bootstrap policy

The Unix and PowerShell installers:

1. map only known host aliases to the public target contract;
2. download the binary and checksum;
3. verify the checksum before executing the candidate;
4. require an `eggsearch` identity and matching pinned version;
5. atomically replace the destination.

They invoke Cargo only for an unsupported target or a confirmed binary HTTP
404. Transport, authorization, rate-limit, checksum, execution, and identity
failures stop immediately. They never invoke `sudo` or request UAC elevation.
Service registration is deliberately absent until a later deployment phase.

## Self-update

`eggsearch update --check` uses crates.io `crate.max_stable_version` as the
stable-version authority. A normal update requests the exact `vX.Y.Z` asset and
checksum from this contract, verifies bounded bytes and candidate identity, and
replaces `std::env::current_exe()` through the cross-platform `self-replace`
primitive. Only an unsupported host or a confirmed exact-asset HTTP 404 may
enter an exact-version Cargo build under a temporary `--root`; transient
network, status, checksum, and identity failures are hard stops. No updater
path invokes elevation or restarts processes. Persistent-service restart
integration is reserved for phase 9.
