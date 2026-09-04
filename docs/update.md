# Self-update

`eggsearch update --check` queries crates.io and compares the installed
`eggsearch --version` identity with the latest stable `crate.max_stable_version`.
It only reports the result. `eggsearch update` performs the same comparison and
updates the executable that is currently running.

Equal versions are a successful no-op. A local version newer than crates.io is
reported as ahead and is never downgraded. Malformed registry metadata and
pre-release versions fail closed; there is no pre-release channel.

## Payload and verification

For a newer version `X.Y.Z`, the updater requests only the matching
`vX.Y.Z` GitHub Release asset and its adjacent checksum:

```text
https://github.com/eggstack/eggsearch/releases/download/vX.Y.Z/eggsearch-<target>
https://github.com/eggstack/eggsearch/releases/download/vX.Y.Z/eggsearch-<target>.sha256
```

The target and filename are shared with the [release contract](../architecture/packaging.md).
The downloaded bytes are bounded, SHA-256 verified, made executable where
needed, and run with `--version`. The candidate must identify as `eggsearch` and
report exactly `X.Y.Z` before replacement. Network, status, checksum, execution,
identity, and replacement failures never trigger a source build.

## Cargo fallback

An exact-version Cargo build is used only when the current host is unsupported
or the exact release asset returns HTTP 404. It runs:

```text
cargo install eggsearch --version =X.Y.Z --locked --root <temporary-root>
```

The resulting `<temporary-root>/bin/eggsearch[.exe]` is verified and replaces
the current executable. Cargo is not used when the release download is blocked,
times out, returns another HTTP error, or fails integrity or identity checks. If
Cargo is missing, install Rust from [rustup.rs](https://rustup.rs/) and retry.

## Permissions and lifecycle

The updater never invokes `sudo`, requests UAC elevation, changes configuration,
or restarts arbitrary processes. If the current executable's directory cannot be
written, it prints the resolved elevated rerun command, for example:

```text
sudo /usr/local/bin/eggsearch update
```

Before replacement, a normal update records whether exactly one registered
persistent manager has a healthy service. After verified replacement it restarts
that same manager and verifies `/healthz`; a stopped service remains stopped.
If replacement succeeds but restart fails, the command returns a nonzero typed
error that includes the installed version and exact restart command. An update
of a stdio-only installation does not restart any process. See
[Managed service](service.md).
