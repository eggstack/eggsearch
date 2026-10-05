# Build & CI

**Location:** `Cargo.toml`, `Makefile`, `.github/workflows/`
**Purpose:** Toolchain, feature flags, dependency ownership, routine gates, and release handoff.

Single library + binary crate, not a workspace. Stable contract is MCP tools (10) plus CLI; the Rust module tree is an implementation detail. Dependency flow: `core <- meta <- mcp <- commands`; `fetch` is independent of `meta`.

---

## Toolchain

| Item | Value | Source |
|------|-------|--------|
| Edition | 2021 | `Cargo.toml` |
| `rust-version` / MSRV | 1.89 | `Cargo.toml` |
| CI toolchain | `dtolnay/rust-toolchain@6bed0761…` (40-hex SHA pin) with `toolchain: "1.89"` + `components: rustfmt, clippy` | `.github/workflows/ci.yml` |
| Egress qualification MSRV | `cargo +1.89.0 check --locked --all-features` | `.github/workflows/egress-feature-qualify.yml` |
| Release profile | `lto = "thin"`, `codegen-units = 1`, `strip = true` | `Cargo.toml [profile.release]` |

Package metadata in `Cargo.toml [package]`: version `0.4.1`, edition `2021`, `rust-version = "1.89"`, license `MIT`, authors `eggstack`, repository `https://github.com/eggstack/eggsearch`, homepage `https://docs.rs/eggsearch`, readme `./README.md`, and one `[[bin]]` named `eggsearch` at `src/main.rs` alongside the library.

CI blanks all credential env vars (`GITHUB_TOKEN`, `GH_TOKEN`, `GITLAB_TOKEN`, `CODEBERG_TOKEN`, `GITEA_TOKEN`, `FORGEJO_TOKEN`, `SOURCEGRAPH_API_KEY`, `BRAVE_API_KEY`, `SEMANTIC_SCHOLAR_API_KEY`, `NVD_API_KEY`), so missing credentials are provider-scoped skips, never global failures. Tests must pass keyless and must not require network.

Docs.rs: `all-features = true`, targets `x86_64-unknown-linux-gnu`, `x86_64-apple-darwin`, `aarch64-apple-darwin`. Docs gate: `RUSTDOCFLAGS="-D warnings" cargo doc --locked --all-features --no-deps`.

---

## Feature flags matrix

| Flag | Pulls in | Purpose |
|------|----------|---------|
| `default` | nothing | Minimal build; routine gate still compiles `--no-default-features` separately |
| `mock` | nothing | Test-only engine harness (`src/meta/mock.rs`); **required for integration/corpus tests** — plain `cargo test` misses most of them |
| `pdf` | `dep:lopdf` (0.42, `default-features = false`) | PDF text extraction |
| `browser` | `dep:chromiumoxide` (0.9, `default-features = false`) | Headless Chrome/Chromium rendering |
| `egress` | `dep:eggress-outbound` + `dep:eggress-uri` + `dep:eggress-core` (all `=1.0.8`, `default-features = false`) | Opt-in listener-free HTTP/SOCKS proxy-chain route for provider upstreams only; source-build opt-in, excluded from prebuilt/default binaries |
| `live-smoke` | implies `mock` (`live-smoke = ["mock"]`) | Live network smoke tests; `#[ignore]`d by default, manual only |

Rules enforced by `tests/static_guards.rs`:

- `default` must not enable `egress`.
- All `eggress-*` normal dependencies must be `optional = true`.
- Test-only TLS tooling (`rustls`, `tokio-rustls`, `rcgen`) lives under `[dev-dependencies]`, never in normal dependencies.
- The egress feature budget forbids `eggress-embed`, `eggress-runtime`, `eggress-server`, `eggress-admin`, `pproxy-compat`, `pproxy-legacy`, `legacy-crypto`, `insecure-tls`, `russh`, `eggress-transport-ssh`, `eggress-transport-quic`, `eggress-udp`, `native-tls`, `openssl`.

Single-suite examples:

```bash
cargo test --locked --features mock --test web_search_integration
cargo test --locked --all-features --test dispatch_fault_injection
cargo test --locked --all-features --test tool_surface_live -- --ignored
```

---

## Dependency ownership

### eggsearch-owned HTTP goes through eggfetch-core

`eggfetch-core 0.2.0` (`default-features = false`) is the single owner of transport, pooling, TLS, decompression, total deadlines, bounded hard-error reads, pinned routing, redirect mechanics, and typed failures for all eggsearch-owned outbound requests.

Enabled feature budget (locked by `eggfetch_feature_budget_stays_bounded`):

- `standard-http1`, `advanced-routing`, `redirects`, `tls-rustls`, `json`, `compression-gzip`, `compression-brotli`

Forbidden (selective split, must stay off):

- `logical-retry`, `basic-auth`, `proxy`, `tls-native-roots`, `http2`, `http3`, `cookies`, `multipart`

Consequences:

- No direct `reqwest` dependency in `Cargo.toml` and no `reqwest::` usage in production `src/` (guard `no_direct_reqwest_in_production_source`). The resolved graph contains no `reqwest` path from `rmcp`; the only remaining `reqwest` edge is `chromiumoxide` under the `browser` feature.
- Retry authority belongs to `OriginController` alone; eggfetch `logical-retry` stays disabled so retry policy is not split across two owners.
- Engines use automatic gzip/Brotli decompression; no `.decompress(false)` workaround remains after eggfetch-core 0.2.0 (guard `html_scrape_engines_use_automatic_decompression`).
- HTML scrape and JSON API engines pair an explicit `engine_timeout()` total deadline with `read_bounded_body()` streaming byte caps.
- The `egress` route (`src/fetch/egress.rs`, `EggressDialer` beneath the eggfetch `Dialer` seam) is provider-upstreams-only, fail-closed, credential-indirected via `password_env`. Dynamic `FetchClient` targets keep resolved-address pinning and never use egress; loopback health (`startup`, `integrations/common`, `update`, `forge_adapter`, `package_resolver`, `mcp/http`) and browser traffic stay direct (guards `egress_route_stays_out_of_*`).

### rmcp features required for integrate --apply

`rmcp 3.2.0` enables exactly `server`, `client`, `transport-io`, `transport-child-process`, `transport-streamable-http-server`, and `macros` (`Cargo.toml`). There is no Streamable HTTP **client** feature, and none is needed: `integrate --apply` verifies the registered server over two independent paths.

- Stdio: a real rmcp client session over `rmcp::transport::TokioChildProcess` with `ServiceExt` (`src/integrations/common.rs`). This is what requires the `client` and `transport-child-process` features — do not trim them.
- Loopback HTTP: a hand-rolled MCP verifier in `src/integrations/http_verification.rs` built on `eggfetch_core::Client` — bounded 64 KiB body, 5 s total deadline, redirects disabled, session limit 256, protocol `2025-06-18`, driven through `/healthz` then initialize / notifications-initialized / `tools/list` / session delete.

`src/mcp/http.rs` is the server side, built on `rmcp::transport::streamable_http_server`, which is why `transport-streamable-http-server` is enabled.

### Direct Tokio features stay explicitly qualified

```toml
tokio = { version = "1", features = ["fs", "io-std", "io-util", "macros", "net", "process", "rt-multi-thread", "signal", "sync", "time"] }
```

Guard `tokio_feature_policy_stays_explicit` forbids `"full"` and pins exactly the ten features above. Each feature traces to production API use. `tokio-util 0.7` carries the `CancellationToken` shared with the loopback Streamable HTTP server transport. `axum 0.8` uses `default-features = false` plus `http1` and `tokio` only.

Supporting deps: `http 1` (shared header/status types), `scraper 0.20` + `ego-tree 0.6.2` (HTML), `pulldown-cmark 0.12`, `regex 1`, `url 2` with `serde`, `urlencoding 2`, `clap 4` with `derive`, `anyhow 1`, `serde`/`serde_json 1`, `schemars 1`, `thiserror 1`, `tracing 0.1` + `tracing-subscriber 0.3` (`env-filter`, `fmt`), `chrono 0.4` (`std`, `clock`, `serde`), `xxhash-rust 0.8` (`xxh3`), `libc 0.2`, `lru 0.18`, `futures 0.3`, `semver 1`, `self-replace 1`, `sha2 0.10`, `tempfile 3`, `toml 0.8`, `dirs 5`, `quick-xml 0.41` (`default-features = false`, no optional features; streaming csproj/POM parsing over `&str`, no encoding/serde/async surface). Windows-only: `windows-service 0.8.1` under `cfg(windows)`. Criterion bench harness: `[[bench]] name = "perf", harness = false`.

### Package include and test-only deps

Published crate contents (`include`) cover `src/**/*.rs`, `benches/**/*.rs`, `docs/**/*.md`, `architecture/**/*.md`, `packaging/systemd/*`, `packaging/launchd/*`, `packaging/windows/*`, plus root `README.md`, `LICENSE`, `CHANGELOG.md`. Excluded: `.opencode/`, `.github/`, `plans/`, `fuzz/`.

Dev-dependencies: `pretty_assertions 1` (assertion diffs), `httpmock 0.7` (HTTP mock server), `criterion 0.5` (`default-features = false`, benches), `proptest 1` (`default-features = false` + `std`, property tests), `flate2 1`, `brotli 8`, plus TLS test tooling `rcgen 0.13`, `rustls 0.23`, `tokio-rustls 0.26` (all `default-features = false`, confined to `[dev-dependencies]` by guard).

### Fetch timeout and hot-path ownership

Fetch timeout overrides reuse the shared transport: `FetchClient::with_timeout_ms` clones the shared eggfetch client (`self.client.clone()`, never `Client::builder()`) for equal or shorter timeouts (`requested_timeout_ms <= base_timeout_ms`), and builds exactly one widened client via `build_transport_client` for longer timeouts (`TimeoutOverrideMode::Widened`). Batch setup performs one adjustment per call. Guards `timeout_overrides_retain_the_shared_fetch_client` and `fetch_timeout_paths_use_effective_limits_for_request_and_validation` lock this: `fetch` and `fetch_conditional` validate targets, apply the effective request timeout (`timeout(client_timeout(self.limits.timeout_ms))`), and revalidate redirects against the effective limits.

Performance hot paths use shared immutable inventory and derived-cache ownership and score/select candidates once; deterministic tie ordering is preserved when changing selectors.

### Dependency policy and action-pin gates

`make check` includes a dependency-policy step that the build itself does not otherwise cover. `packaging/check-dependency-policy.sh` runs, in order: `packaging/check-dependency-policy.py`, `packaging/check-http-dependency-shape.py`, a pinned `cargo install cargo-deny --version 0.20.2 --locked` (install-on-demand when `cargo deny --version` is not already exactly `cargo-deny 0.20.2`), then `cargo deny check`.

`deny.toml` at the repository root is the cargo-deny policy: `all-features` graph, advisories version 2 with `unmaintained = "all"` and `yanked = "deny"` plus three time-boxed, dated RUSTSEC ignores (async-std via test-only `httpmock`, fxhash via `scraper`, ttf-parser via optional `lopdf` PDF support, each marked for review by 2026-11-27), a 0.8-confidence license allowlist of 15 identifiers, `bans.multiple-versions = "warn"` with wildcards allowed, and unknown registry/git both denied with the crates.io index as the only allowed registry.

`.github/workflows/dependency-security.yml` runs the same script on a weekly `0 9 * * 1` cron plus `workflow_dispatch`, with `contents: read` only. Separately, `packaging/check-workflow-pins.py` runs inside `make packaging-check` and fails closed unless every non-local `uses:` in `.github/workflows/*.yml` is pinned to a full 40-hex commit SHA.

---

## Makefile target catalog

`make check` is the canonical gate, and `make ci` aliases it:

```bash
make check  # fmt + clippy + no-default check + all-features tests + hygiene + dependency-policy + packaging-check
```

| Step | Command | Gate |
|------|---------|------|
| `fmt` | `cargo fmt --check` | rustfmt clean; CI fails otherwise |
| `clippy` | `cargo clippy --locked --all-targets --all-features -- -D warnings` | Zero warnings required |
| `feature-check` | `cargo check --locked --no-default-features` | Compile-only; minimal build stays healthy |
| `test` | `cargo test --locked --all-features` | Full suite, keyless, network-free |
| `hygiene` | `./packaging/check-repo-hygiene.sh` | Rejects tracked transcripts, ANSI dumps at root, build outputs, oversized root blobs, editor temp files |
| `dependency-policy` | `./packaging/check-dependency-policy.sh` | Policy scripts plus pinned `cargo deny check` against `deny.toml` |
| `packaging-check` | `./packaging/check-contract.sh` | Exact target/asset declarations and installer guards |

`.github/workflows/ci.yml` declares two jobs on `push`/`pull_request` to `main`, both with `contents: read` and SHA-pinned actions: `ci` on `ubuntu-latest` (checkout, toolchain 1.89 with `rustfmt, clippy`, then `make ci` with credential env vars blanked) and `windows-portability` on `windows-latest` (`cargo check --locked --all-features --target x86_64-pc-windows-msvc` at pinned Rust 1.89) so Windows `cfg` breakage cannot hide behind Linux-green CI. Concurrency cancels in-progress runs on the same ref.

`make hygiene`, `make dependency-policy`, `make packaging-check`, and `make bench-check` are also runnable individually. Fuzz smoke (`make fuzz-smoke`) runs four targets at 60s each: `validate_url`, `sanitize_pipeline`, `bounded_response_reader`, and `dependency_parse` under `fuzz/`.

---

## release-check and bench-check

```bash
make release-check  # check + release-candidate-check + docs-check + release-build + publish-check
make bench-check    # compile-check the Criterion performance harness
```

| Target | Command | Purpose |
|--------|---------|---------|
| `release-candidate-check` | `./packaging/release-validate.sh candidate` | Required release tree, version, and local packaging syntax |
| `docs-check` | `RUSTDOCFLAGS="-D warnings" cargo doc --locked --all-features --no-deps` | Zero rustdoc warnings |
| `release-build` | `cargo build --locked --release` | Release binary |
| `publish-check` | `cargo publish --dry-run --locked` | Pre-publish validation |
| `bench-check` | `cargo bench --locked --all-features --bench perf --no-run` | Deterministic compile gate for the Criterion harness; characterization runs use exact candidates and are never CI thresholds |
| `eval-tool-surface` | `cargo test --locked --all-features --test tool_surface_evaluation -- --nocapture` | Deterministic 43-fixture tool-selection corpus (also inside `make check`) |

Live and forge smokes are explicit opt-in only and never part of `check`:

| Target | Command | Purpose |
|--------|---------|---------|
| `live-smoke` | `cargo test --features live-smoke --test corpus_runner -- --ignored` | Live network corpus |
| `native-forge-smoke-github/gitlab/codeberg/gitea/all` | `cargo test --locked --features live-smoke --test native_forge_smoke -- --ignored [filter]` | Per-host live forge diagnostics; maintainer-only, not release evidence |

The egress qualification workflow (`.github/workflows/egress-feature-qualify.yml`) triggers on the egress-route construction seams (`src/fetch/egress.rs`, `src/meta/engines/mod.rs`, `src/meta/adapter/{mod,builders}.rs`, `src/mcp/state.rs`, `src/core/config.rs`, `src/startup.rs`, `src/process.rs`, `src/update.rs`, `src/meta/safe_open.rs`), the egress suites and `tests/static_guards.rs`, `Cargo.toml`/`Cargo.lock`, `packaging/release-targets.txt`, `packaging/check-egress-qualify-contract.sh`, and both `egress-feature-qualify.yml` and `ci.yml`. It verifies the 7-target matrix equals `packaging/release-targets.txt` in exact set terms, runs `check-egress-qualify-contract.sh`, compile-checks `--features egress` per target (Linux x86_64/aarch64/armv7, macOS x86_64/aarch64, Windows x86_64/aarch64), and re-checks `--all-features` on MSRV 1.89. No egress-enabled binary is published.

---

## Platform support

7 release targets (`packaging/release-targets.txt`, `src/platform.rs`):

- Linux: x86_64, aarch64, armv7 (glibc 2.17 floor via Zig/cargo-zigbuild)
- macOS: x86_64 (Intel), aarch64 (Apple Silicon)
- Windows: x86_64, aarch64 (native runners, `.exe` assets, SCM service support)

Unix-only process controls (`openat2`, `setsid`, process groups) mean some supervision primitives degrade on Windows; the Windows service path uses SCM instead. See `packaging.md` for the full target/installer/update contract.

The binary-first updater is covered by the all-features test suite. Its routine tests use local HTTP fixtures and fixture executables; they never contact crates.io or GitHub and never replace the test runner. Update policy: crates.io `crate.max_stable_version` is the stable authority (never GitHub `latest`); only the exact `vX.Y.Z` release asset and checksum are requested; checksum plus exact `eggsearch --version` identity must pass before any candidate replacement; Cargo fallback is allowed only for unsupported hosts or a confirmed exact-asset HTTP 404.

---

## Development workflow

```bash
git clone https://github.com/eggstack/eggsearch
cd eggsearch
cargo build
make check
cargo run -- mcp stdio
cargo run -- mcp serve
```

Feature-scoped runs:

```bash
cargo run --features mock -- search "test query"
cargo run --features pdf -- fetch https://example.com/doc.pdf
cargo run --features browser -- fetch https://example.com
```

Diagnostics:

```bash
cargo run -- -vv search "test query"
cargo run -- -vvv mcp stdio
```

---

## Verification for build changes

```bash
make check
make release-check
make bench-check
```

Routine work stays network-free; keyless and offline by default. Live probes, native-forge smokes, and agentic evaluation run only on explicit opt-in targets with their documented env inputs.

---

[← Back to Overview](overview.md)
