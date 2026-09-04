# Build & CI Deep Dive

**Location:** `Cargo.toml`, `Makefile`
**Purpose:** Build configuration, CI pipeline, feature flags, routine gates, and release handoff.

---

## Cargo.toml

### Package Metadata

```toml
[package]
name = "eggsearch"
version = "0.3.8"
edition = "2021"
rust-version = "1.88"
authors = ["eggstack"]
license = "MIT"
```

### Features

| Feature | Dependencies | Purpose |
|---------|--------------|---------|
| `default` | (none) | Minimal build |
| `mock` | (none) | Test-only mock engine harness |
| `pdf` | `lopdf` | PDF text extraction |
| `browser` | `chromiumoxide` | Headless Chrome/Chromium rendering |
| `live-smoke` | `mock` | Live network smoke tests |

### Dependencies

#### Core Dependencies

| Dependency | Version | Purpose |
|------------|---------|---------|
| `tokio` | 1 (full) | Async runtime |
| `clap` | 4 (derive) | CLI argument parsing |
| `anyhow` | 1 | Error context (CLI) |
| `serde` | 1 (derive) | Serialization |
| `serde_json` | 1 | JSON serialization |
| `schemars` | 1 | JSON Schema generation |
| `thiserror` | 1 | Error derive macros |
| `tracing` | 0.1 | Structured logging |
| `tracing-subscriber` | 0.3 | Log formatting |
| `rmcp` | 3.2.0 (server, transport-io, transport-streamable-http-server, macros) | MCP protocol and Streamable HTTP server |
| `axum` | 0.8 (http1, tokio) | Persistent HTTP listener and health routing |
| `tokio-util` | 0.7 | Cancellation tokens shared with rmcp HTTP transport |

#### HTTP & Parsing

| Dependency | Version | Purpose |
|------------|---------|---------|
| `reqwest` | 0.12 (rustls-tls, gzip, brotli, stream, json) | HTTP client |
| `scraper` | 0.20 | HTML parsing |
| `ego-tree` | 0.6.2 | DOM tree traversal |
| `pulldown-cmark` | 0.12 | Markdown parsing |
| `regex` | 1 | Pattern matching |
| `url` | 2 (serde) | URL parsing |
| `urlencoding` | 2 | Percent-encoding |

#### Utilities

| Dependency | Version | Purpose |
|------------|---------|---------|
| `toml` | 0.8 | Config parsing |
| `dirs` | 5 | Platform directory resolution |
| `chrono` | 0.4 | Date/time handling |
| `xxhash-rust` | 0.8 (xxh3) | Fast hashing |
| `libc` | 0.2 | Unix APIs |
| `lru` | 0.12 | LRU cache |
| `futures` | 0.3 | Async utilities |

#### Optional Dependencies

| Dependency | Version | Feature | Purpose |
|------------|---------|---------|---------|
| `lopdf` | 0.42 | `pdf` | PDF text extraction |
| `chromiumoxide` | 0.9 | `browser` | Headless Chrome |

### Dev Dependencies

| Dependency | Version | Purpose |
|------------|---------|---------|
| `pretty_assertions` | 1 | Improved assertion diffs |
| `tempfile` | 3 | Temporary files |
| `httpmock` | 0.7 | HTTP mock server |
| `criterion` | 0.5 | Benchmarks |
| `proptest` | 1 | Property-based testing |

### Build Profile

```toml
[profile.release]
lto = "thin"
codegen-units = 1
strip = true
```

---

## Makefile Targets

### Primary Targets

| Target | Command | Purpose |
|--------|---------|---------|
| `check` | `fmt + clippy + feature-check + test` | Local CI gate |
| `ci` | `check` | Alias for `check` |
| `fmt` | `cargo fmt --check` | Format check |
| `clippy` | `cargo clippy --locked --all-targets --all-features -- -D warnings` | Lint check |
| `feature-check` | `cargo check --locked --no-default-features` | No-default compile check |
| `test` | `cargo test --locked --all-features` | All tests |

### Release Targets

| Target | Command | Purpose |
|--------|---------|---------|
| `release-check` | `check + docs-check + release-build + publish-check` | Pre-release gate |
| `docs-check` | `RUSTDOCFLAGS="-D warnings" cargo doc --locked --all-features --no-deps` | Docs check |
| `release-build` | `cargo build --locked --release` | Release build |
| `publish-check` | `cargo publish --dry-run --locked` | Pre-publish check |

The binary-first updater is covered by the all-features test suite. Its routine
tests use local HTTP fixtures and fixture executables; they do not contact
crates.io or GitHub and never replace the test runner.

### Packaging target

| `packaging-check` | `./packaging/check-contract.sh` | Cross-check target/asset declarations and installer guards |

### Smoke Targets

| Target | Command | Purpose |
|--------|---------|---------|
| `fuzz-smoke` | Quick fuzz runs for 3 targets | Fuzz smoke test |
| `live-smoke` | `cargo test --features live-smoke --test corpus_runner -- --ignored` | Live network tests |
| `native-forge-smoke-*` | Live forge API smoke tests per host | Forge API tests |

---

## CI Pipeline

### Single Job: `ci`

```yaml
jobs:
  ci:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          toolchain: "1.88"
      - run: make ci
```

### Pipeline Steps

1. **Format check** — `cargo fmt --check`
2. **Clippy lint** — `cargo clippy --locked --all-targets --all-features -- -D warnings`
3. **No-default compile** — `cargo check --locked --no-default-features`
4. **All tests** — `cargo test --locked --all-features`

---

## Release Process

### Pre-Release Checklist

1. **Run `make release-check`** — Must pass all checks
2. **Bump version** in `Cargo.toml`
3. **Update `CHANGELOG.md`**
4. **Update `docs/release.md`** with release notes

### Publishing

```bash
cargo publish --locked
```

### Post-Release

1. **Verify on crates.io**
2. **Tag release** in git
3. **Update documentation**

---

## Platform Support

### Supported

- Linux (x86_64, aarch64)
- macOS (x86_64, aarch64)

### Unsupported

- Windows (uses Unix-specific APIs: `openat2`, `setsid`, process groups)

---

## Documentation

### RUSTDOCFLAGS

```bash
RUSTDOCFLAGS="-D warnings" cargo doc --locked --all-features --no-deps
```

Zero warnings required.

### Docs.rs Configuration

```toml
[package.metadata.docs.rs]
all-features = true
targets = ["x86_64-unknown-linux-gnu", "x86_64-apple-darwin", "aarch64-apple-darwin"]
```

---

## Development Workflow

### Quick Start

```bash
# Clone and build
git clone https://github.com/eggstack/eggsearch
cd eggsearch
cargo build

# Run tests
make check

# Start MCP server
cargo run -- mcp stdio
# Start persistent loopback HTTP MCP
cargo run -- mcp serve
```

### Feature Development

```bash
# With mock engine
cargo run --features mock -- search "test query"

# With PDF support
cargo run --features pdf -- fetch https://example.com/doc.pdf

# With browser rendering
cargo run --features browser -- fetch https://example.com
```

### Debugging

```bash
# Verbose logging
cargo run -- -vv search "test query"

# Trace level
cargo run -- -vvv mcp stdio
```

---

[← Back to Overview](overview.md)
