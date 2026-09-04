# Installation

## Binary-first bootstrap

Supported hosts should use the reviewed installer attached to the matching
GitHub Release. It downloads a default-feature executable, verifies the
adjacent SHA-256 file, checks `eggsearch --version`, and then installs it
atomically.

Unix:

```bash
curl -fsSL https://github.com/eggstack/eggsearch/releases/latest/download/install.sh | bash
```

PowerShell:

```powershell
irm https://github.com/eggstack/eggsearch/releases/latest/download/install.ps1 | iex
```

Pin a published version when reproducibility matters:

```bash
curl -fsSL https://github.com/eggstack/eggsearch/releases/latest/download/install.sh | bash -s -- --version 0.3.8
```

```powershell
$installer = irm https://github.com/eggstack/eggsearch/releases/latest/download/install.ps1; & ([scriptblock]::Create($installer)) -Version 0.3.8
```

The pinned Unix installer requests assets from the exact `vX.Y.Z` release.
The unpinned form uses `releases/latest/download` for convenience.

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
installer invokes `sudo`, requests UAC elevation, or installs a service. Add
the selected user-local directory to `PATH` when the installer advises it.

## Cargo fallback

Cargo is used only when the host target is unsupported or the exact binary
asset returns HTTP 404. A pinned invocation uses `cargo install eggsearch
--version X.Y.Z --locked`; an unpinned invocation uses `cargo install
eggsearch --locked`.

DNS/TLS/connect failures, GitHub authorization/rate-limit/server errors,
checksum download failures, malformed or mismatched checksums, candidate
execution failures, and candidate identity/version mismatches are hard
failures. They never trigger a potentially long local Rust compilation.

The install command only installs the executable. Persistent service
registration and startup supervision are separate future work and are not
available from these installers.
