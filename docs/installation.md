# Installation

## Python package

The in-process native Python package is published separately from the Rust
crate and executable assets. Install it on a supported host with:

```bash
python -m pip install eggsearch
```

On ARMv7 or another SBC, require a prebuilt compatible wheel so an unsupported
host fails without compiling the source distribution:

```bash
python -m pip install --only-binary=:all: eggsearch
```

The published package supports CPython 3.11–3.15. Linux x86-64 and aarch64
wheels use manylinux2014 (glibc 2.17); the experimental ARMv7 hard-float wheel
requires glibc 2.28 or newer. macOS x86-64/arm64 and Windows x86-64 wheels are
also available. See the [Python library guide](python-library.md) for API and
platform details. ARMv7 remains experimental pending physical SBC measurements.

## Install from the published binary release

The Rust executable release is separate from the Python package. Install the
host-matched executable with the release installer:

```bash
curl -fsSL https://github.com/eggstack/eggsearch/releases/latest/download/install.sh | bash
```

PowerShell:

```powershell
irm https://github.com/eggstack/eggsearch/releases/latest/download/install.ps1 | iex
```

Pin a published executable version when reproducibility matters:

```bash
curl -fsSL https://github.com/eggstack/eggsearch/releases/latest/download/install.sh | bash -s -- --version 0.4.3
```

```powershell
$installer = irm https://github.com/eggstack/eggsearch/releases/latest/download/install.ps1; & ([scriptblock]::Create($installer)) -Version 0.4.3
```

Each installer downloads a default-feature executable, verifies the adjacent
SHA-256 file, checks `eggsearch --version`, and installs atomically. For a
source installation or an unsupported target, use:

```bash
cargo install eggsearch --locked
```

The pinned Unix installer requests assets from the exact `vX.Y.Z` release. The
unpinned form uses `releases/latest/download` for convenience.

## Release targets

| Host | Rust target | Public asset |
|---|---|---|
| Linux x86-64 | `x86_64-unknown-linux-gnu` | `eggsearch-x86_64-unknown-linux-gnu` |
| Linux ARM64 | `aarch64-unknown-linux-gnu` | `eggsearch-aarch64-unknown-linux-gnu` |
| Linux ARMv7 hard-float | `armv7-unknown-linux-gnueabihf` | `eggsearch-armv7-unknown-linux-gnueabihf` |
| macOS Intel | `x86_64-apple-darwin` | `eggsearch-x86_64-apple-darwin` |
| macOS Apple Silicon | `aarch64-apple-darwin` | `eggsearch-aarch64-apple-darwin` |
| Windows x86-64 | `x86_64-pc-windows-msvc` | `eggsearch-x86_64-pc-windows-msvc.exe` |
| Windows ARM64 | `aarch64-pc-windows-msvc` | `eggsearch-aarch64-pc-windows-msvc.exe` |

Linux GNU binaries target a glibc 2.17 portability floor. ARMv7 is published
only after architecture checks and QEMU runtime qualification. Windows ARM64
is built and smoked on a native ARM64 runner; a toolchain or runner outage
keeps that release job red rather than relabeling it as x86-64.

Every executable has an adjacent `<asset>.sha256` file in standard one-line
`digest  filename` format. GitHub macOS binaries are currently unsigned; no
notarization or managed signing identity is part of this distribution path.

## Install destinations and privilege

The Unix installer uses `/usr/local/bin` when running as root and
`$HOME/.local/bin` otherwise. PowerShell uses `%ProgramFiles%\Eggsearch` when
running as Administrator and `%LOCALAPPDATA%\Eggsearch` otherwise. Neither
installer invokes `sudo` or requests UAC elevation. Add `--service` (or
PowerShell `-Service`) explicitly to delegate registration to the installed
CLI; a failed registration leaves the verified binary installed and prints
instructions for recovery. See [Managed service](service.md).

## Cargo fallback

Cargo is used only when the host target is unsupported or the exact binary
asset returns HTTP 404. A pinned invocation uses `cargo install eggsearch
--version X.Y.Z --locked`; an unpinned invocation uses `cargo install
eggsearch --locked`.

DNS/TLS/connect failures, GitHub authorization/rate-limit/server errors,
checksum download failures, malformed or mismatched checksums, candidate
execution failures, and candidate identity/version mismatches are hard
failures. They never trigger a potentially long local Rust compilation.

The default install command only installs the executable. To run the persistent
local endpoint in the foreground, start it explicitly with `eggsearch mcp serve`.
Explicit service mode registers only the persistent HTTP endpoint.

## Updating an installed binary

The bootstrap installer is intended for first installation or selecting an
explicit destination. For routine upgrades, use the installed executable:

```bash
eggsearch update --check
eggsearch update
```

The updater uses crates.io as its stable-version authority and then consumes the
exact matching GitHub Release asset, with the same target names and checksum
contract documented above. See [Self-update](update.md) for verification,
Cargo fallback, permission, and lifecycle behavior.

## Fleet one-liners

Binary-only installation is the default.
Add `--service` only when the host should run the persistent loopback endpoint:

```bash
curl -fsSL https://github.com/eggstack/eggsearch/releases/latest/download/install.sh | bash
curl -fsSL https://github.com/eggstack/eggsearch/releases/latest/download/install.sh | bash -s -- --service
```

After installation, print or apply a client registration explicitly:

```bash
eggsearch integrate codegg --apply
eggsearch integrate codex --apply
```
