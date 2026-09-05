# Release packaging

eggsearch publishes default-feature executables as GitHub Release assets. The
release tag is the version namespace; executable names do not contain a
version. Each executable has an adjacent SHA-256 file in the standard
`digest  filename` format.

The public target contract is in `release-targets.txt` and is mirrored in the
workflow and installers. `install.sh` and `install.ps1` are attached to each
draft release as reviewed source bytes.

`release-smoke.sh` and `release-smoke.ps1` verify version/help startup and a
keyless MCP initialize plus `tools/list` handshake. ARMv7 release jobs run the
CLI smoke under QEMU; MCP protocol smoke is performed on native jobs because
the QEMU user-mode stdio path is not a reliable release gate on hosted runners.

The installers never elevate. They install to `/usr/local/bin` or
`%ProgramFiles%\\Eggsearch` when already privileged, and otherwise use
`$HOME/.local/bin` or `%LOCALAPPDATA%\\Eggsearch`. Cargo compilation is only a
fallback for unsupported targets or a confirmed 404 for the exact binary.

Pass `--service` to `install.sh` or `-Service` to `install.ps1` only when a
persistent service is wanted. The script delegates to the installed binary's
`startup install` command. Manager templates are embedded in that binary and
mirrored in `packaging/systemd/`, `packaging/launchd/`, and
`packaging/windows/`; the scripts contain no service-manager logic and never
invoke elevation.

After binary installation, `eggsearch integrate list` reports the available
client adapters. `eggsearch integrate <client>` renders a registration without
mutation; `--apply` is required for native CLI or strict JSON changes. The
integration layer uses the same stdio command and loopback HTTP endpoint as the
release smoke and startup runtime.
