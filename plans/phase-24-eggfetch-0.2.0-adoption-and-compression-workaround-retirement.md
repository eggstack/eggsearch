# Phase 24 — eggfetch 0.2.0 adoption and compression-workaround retirement

Status: planned / ready for handoff

Planning baseline: `0c33d576a802b84ffdae5b8b52364a50646d111c` (`eggsearch` 0.3.9 on `main`)

Depends on:

- Phase 17 eggfetch transport consolidation implemented;
- Phase 18 transport-migration qualification/upstream compression handoff implemented;
- Phase 23 performance corrective requalification implemented;
- published `eggfetch-core 0.2.0` / tag `v0.2.0`;
- upstream issue `eggstack/eggfetch#24`, fixed by `37ab02b3873a4f0ce7018bd716e326bcf0595230`.

Upstream release evidence:

- `eggfetch v0.2.0` release commit: `8959ca890ee34f4cf456aed648315322f1e83ef7`;
- issue #24 corrective commit: `37ab02b3873a4f0ce7018bd716e326bcf0595230`;
- upstream regression coverage includes fragmented gzip/Brotli/deflate/zstd streaming, empty source items, one-byte fragmentation, high-level HTTP/1.1 `Content-Length` vs `Transfer-Encoding: chunked`, and decompression-disabled/raw exact-byte behavior;
- the release declares no intentional breaking Rust/Python/C/CLI/HTTPX API, feature-graph, or MSRV change from the 0.1.7 integration baseline.

## Context

Phase 17 moved eggsearch-owned outbound HTTP from direct reqwest to
`eggfetch-core 0.1.7`. Phase 18 then proved a transport-shape-sensitive
upstream defect: valid gzip and Brotli response bodies decoded correctly when
delivered with `Content-Length`, but failed through eggfetch's automatic
streaming decompressor when the same compressed bytes arrived through HTTP/1.1
chunked transfer.

Eggsearch deliberately did not work around that defect by adding a local
decompression stack. Instead, six HTML scrape engines temporarily called
`.decompress(false)`, causing those requests to avoid compressed response
handling while JSON/API paths retained normal automatic decompression.

The affected HTML engines are:

- Brave HTML;
- DuckDuckGo;
- Mojeek;
- SearXNG;
- Startpage;
- Yahoo.

Upstream eggfetch 0.2.0 now contains the issue #24 correction. The old private
stream-to-`AsyncRead` adapter was replaced with
`tokio_util::io::StreamReader` beneath the existing buffered decoder layer,
and high-level regressions now prove that identical compressed bytes decode
identically regardless of `Content-Length` vs chunked transfer shape.

This phase consumes that published correction and retires only the temporary
downstream workaround that it makes unnecessary.

## Objective

Adopt published `eggfetch-core 0.2.0` without reopening the Phase 17 transport
architecture or regressing Phase 23 timeout/performance semantics.

The desired end state is:

- eggsearch resolves published `eggfetch-core 0.2.0`;
- the existing bounded eggfetch feature set remains unchanged unless the
  published crate mechanically requires a manifest adjustment;
- the six HTML scrape engines no longer call `.decompress(false)` solely for
  issue #24;
- automatic gzip/Brotli decompression is restored for those providers;
- a deterministic eggsearch regression proves chunked gzip and Brotli through
  the same high-level response/streaming path used by
  `read_bounded_body()`;
- response-size bounds remain enforced on decoded bytes;
- request/total deadlines, redirect policy, SSRF controls, retry ownership,
  sanitization, and provider parsing are unchanged;
- direct production reqwest remains absent except for the existing rmcp-owned
  transitive boundary;
- the exact production candidate passes local correctness/package gates and
  the normal seven-target non-publishing release qualification;
- Phase 17/18 historical records remain historical rather than being rewritten
  to pretend they originally used 0.2.0.

## Ownership boundary

Preserve the established split:

```text
eggsearch
  owns provider request policy, SSRF authorization, bounded body reads,
  application retries/circuit state, redirect authorization, parsing,
  provider regressions, and release qualification

eggfetch
  owns HTTP transfer decoding and automatic response decompression,
  including chunk-boundary correctness for gzip/Brotli bodies

rmcp
  continues to own its Streamable HTTP client and transitive reqwest path
```

Do not move decompression implementation into eggsearch.

## Work item 1 — Freeze the adoption baseline and dependency graph

Before editing production code:

1. record the exact current `main` SHA;
2. confirm the Phase 23 timeout semantics are present;
3. record:
   - `cargo tree -e normal`;
   - `cargo tree -e features -i eggfetch-core`;
   - resolved `eggfetch-core` version;
   - direct/transitive reqwest ownership;
4. confirm the current feature selection is exactly:
   - `standard-http1`;
   - `advanced-routing`;
   - `redirects`;
   - `tls-rustls`;
   - `json`;
   - `compression-gzip`;
   - `compression-brotli`;
5. locate every issue-#24 workaround and guard before removing anything.

The baseline currently expected by this plan is `0c33d576...`, but execution
must record the actual starting SHA if `main` advances before handoff.

## Work item 2 — Adopt the published eggfetch 0.2.0 crate

Change the direct dependency from the 0.1.7 line to the published 0.2.0 line.

Expected manifest shape:

```toml
eggfetch-core = { version = "0.2.0", default-features = false, features = [
    "standard-http1",
    "advanced-routing",
    "redirects",
    "tls-rustls",
    "json",
    "compression-gzip",
    "compression-brotli",
] }
```

Requirements:

- resolve from crates.io; do not use a Git revision or path override;
- update `Cargo.lock`;
- retain `default-features = false`;
- do not enable H2, H3, proxy, cookie, auth, retry, or additional compression
  features opportunistically;
- do not alter eggsearch's MSRV unless the published dependency mechanically
  requires it and that requirement is independently verified;
- do not broaden the dependency bump into unrelated package upgrades.

After the lock update, inspect the diff rather than accepting all solver churn.
Unexpected unrelated dependency movement must be explained or constrained.

## Work item 3 — Retire the six issue-#24 request workarounds

Remove the issue-#24-specific `.decompress(false)` call from exactly the
known HTML scrape paths:

1. Brave HTML;
2. DuckDuckGo;
3. Mojeek;
4. SearXNG;
5. Startpage;
6. Yahoo.

Do not mechanically remove decompression controls from any unrelated path
without proving they exist for the same historical reason.

After removal, these requests should use the normal client/request automatic
decompression behavior supplied by eggfetch 0.2.0.

Do not manually set `Accept-Encoding` to reproduce the old identity behavior.
The purpose of this phase is to restore the normal compressed-response path.

## Work item 4 — Replace workaround guards with positive compression contracts

Phase 18 added regression coverage that deliberately asserted the six HTML
engines disabled decompression / requested identity encoding. Those tests must
not simply be deleted without replacement.

Update the relevant static/request-contract tests so they now prove:

- none of the six issue-#24 engines carries the temporary
  `.decompress(false)` workaround;
- requests remain otherwise structurally unchanged;
- automatic compression negotiation is allowed for the configured
  gzip/Brotli feature set;
- JSON/API clients retain their existing behavior;
- unrelated raw-response call sites, if any, remain intentional;
- direct production reqwest remains absent.

Prefer behavior-oriented assertions over source-text assertions where practical.
If a static guard remains useful, invert it narrowly to reject reintroduction of
the obsolete workaround in the six affected modules.

## Work item 5 — Add an eggsearch-level chunked decompression regression

Add a deterministic local loopback test that exercises the same high-level
eggfetch body-stream path consumed by eggsearch.

The test must:

1. create a deterministic plaintext payload;
2. create valid gzip and Brotli forms using test-only tooling already available
   through the dependency graph, or fixed validated fixtures if that avoids new
   production dependencies;
3. serve each compressed payload over loopback with:
   - `Content-Encoding: gzip` / `br`;
   - `Transfer-Encoding: chunked`;
   - deliberately small transfer chunks so the body reaches eggfetch across
     multiple stream items;
4. fetch via the eggsearch client configuration using automatic decompression;
5. pass the response through `read_bounded_body()` or the nearest
   production-equivalent helper without bypassing eggfetch's
   `bytes_stream()`;
6. assert the collected bytes exactly equal the original plaintext for both
   codecs.

A complementary `Content-Length` control is strongly preferred so the test
preserves the original issue's transfer-shape comparison.

This is a downstream integration regression, not a duplicate decoder-unit
suite. Keep it small and focused on the eggsearch/eggfetch seam.

## Work item 6 — Re-prove body limits and timeout semantics after decompression

The upstream correction changes internal stream adaptation, while Phase 23
qualified eggsearch's timeout/client reuse behavior. Re-prove both boundaries
together.

Required deterministic assertions:

- a decoded body that fits beneath the eggsearch bound succeeds;
- a compressed body whose decoded output exceeds the eggsearch bound is
  rejected by the existing bounded-body logic;
- the size bound is not accidentally applied only to compressed wire bytes;
- total/request deadline behavior remains bounded while consuming a compressed
  chunked response;
- equal/shorter timeout overrides still use the shared client path;
- longer timeout overrides still receive the widened client/connect policy
  established in Phase 23;
- no new retry layer appears in eggfetch.

Do not redesign these mechanisms unless the 0.2.0 adoption exposes an actual
contract incompatibility.

## Work item 7 — Provider smoke for the original live failure modes

After deterministic tests pass, perform bounded live smoke against the two
providers that originally supplied the captured failure evidence:

- DuckDuckGo for chunked Brotli;
- Startpage for chunked gzip.

Where practical, record:

- response status;
- negotiated/wire `Content-Encoding`;
- whether transfer is chunked or otherwise multi-item;
- successful bounded body collection;
- parser success/non-empty result behavior.

Live third-party behavior may change independently of eggsearch. Therefore:

- a live success is useful release evidence;
- provider blocking, challenge pages, or transfer-shape changes are not by
  themselves proof of a regression;
- any reproducible eggfetch decompression error on a valid compressed body is a
  blocker and must be reduced to a deterministic local case before closure.

Do not weaken deterministic tests to accommodate live-provider variability.

## Work item 8 — Run dependency/API/security regression gates

At minimum run:

```text
cargo fmt --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo check --locked --no-default-features
cargo test --locked --all-features
RUSTDOCFLAGS="-D warnings" cargo doc --locked --all-features --no-deps
make hygiene
make packaging-check
make release-check
cargo publish --dry-run --locked
```

Also rerun repository-specific dependency/feature-budget guards.

Explicitly verify after the bump:

- no eggsearch-owned direct reqwest dependency returned;
- rmcp remains the only intentional normal-graph reqwest owner;
- the eggfetch feature budget did not widen;
- public MCP tool count/schemas are unchanged;
- provider set and routing policy are unchanged;
- no new public Rust API is introduced merely for the test seam.

If `cargo semver-checks` or the repository's equivalent API oracle is part of
the current normal gate, run it as well.

## Work item 9 — Characterize dependency and binary impact

Because this is a dependency correction rather than a performance phase, no
size win is required. Still record enough evidence to catch accidental
footprint regressions.

Record:

- before/after normal dependency graph delta;
- before/after `eggfetch-core` feature graph;
- representative release binary size on the local qualification host;
- whether `tokio-util`, compression crates, or TLS dependencies changed in a
  way that affects the final graph.

Do not block on small explained changes resulting directly from eggfetch 0.2.0.
Do block on unexplained feature expansion or substantial unrelated dependency
growth.

## Work item 10 — Run exact-candidate seven-target qualification

This dependency and production-request-policy change invalidates previous
release qualification evidence.

After all implementation/tests/docs are committed to a clean candidate, run
`.github/workflows/release-binaries.yml` using:

```text
mode = qualify
ref  = <exact immutable candidate SHA>
```

Require the existing seven targets:

1. `x86_64-unknown-linux-gnu`;
2. `aarch64-unknown-linux-gnu`;
3. `armv7-unknown-linux-gnueabihf`;
4. `x86_64-apple-darwin`;
5. `aarch64-apple-darwin`;
6. `x86_64-pc-windows-msvc`;
7. `aarch64-pc-windows-msvc`.

The final assembly job must pass exact asset-set validation.

Record:

- workflow run ID/URL;
- `QUALIFIED_SHA`;
- package version;
- all target results;
- assembly result;
- artifact name;
- exact asset count/checksum result.

Do not publish a crate or GitHub Release as part of this phase unless a
separately authorized release operation is requested.

## Work item 11 — Reconcile documentation and historical workaround state

Update current-state documentation that still describes the issue #24
workaround as active.

Rules:

- preserve Phase 17 and Phase 18 implementation records as historical evidence;
- do not rewrite old commands/results to claim they used eggfetch 0.2.0;
- annotate or cross-reference the later Phase 24 retirement where needed;
- current architecture/dependency docs should state eggfetch 0.2.0 and normal
  automatic gzip/Brotli handling;
- remove current-state statements that tell maintainers to preserve
  `.decompress(false)`;
- retain the upstream issue/fixing commit reference where useful for provenance.

If upstream issue #24 remains open despite the published correction, eggsearch
closure does not require mutating the upstream issue. Record its state
truthfully.

## Work item 12 — Close only with exact evidence

Append an implementation record to this plan containing:

- implementation SHA;
- final qualified candidate SHA;
- resolved `eggfetch-core` version and feature set;
- lock/dependency graph delta;
- exact list of removed workaround call sites;
- deterministic gzip/Brotli chunked regression results;
- decoded-size-limit and timeout regression results;
- live DuckDuckGo/Startpage smoke outcome, including environmental failures if
  any;
- local gate results;
- representative binary-size characterization;
- qualification run ID/URL and seven-target/assembly result;
- any deviations or blockers.

Then mark Phase 24 `implemented` and update `plans/registry.md` in the same
closure change.

If the new published dependency reproduces issue #24 or exposes another
correctness/security/portability regression:

- do not restore a broad permanent workaround and declare success;
- mark Phase 24 blocked;
- reduce the failure to a deterministic test;
- hand the transport defect upstream when appropriate;
- retain the smallest safe downstream mitigation only if necessary;
- register any follow-on corrective plan explicitly.

## Non-goals

Phase 24 does not:

- redesign the Phase 17 eggfetch integration;
- replace eggfetch with reqwest;
- replace rmcp's reqwest-backed Streamable HTTP client;
- add an eggsearch decompression implementation;
- enable additional eggfetch protocol/auth/proxy/retry features;
- change SSRF or redirect authorization policy;
- change OriginController retry/circuit semantics;
- add/remove search providers;
- alter MCP tools or schemas;
- redesign provider parsers;
- opportunistically update unrelated dependencies;
- publish eggsearch;
- close or edit the upstream eggfetch issue as a required side effect.

## Suggested execution order

```text
1. freeze current SHA + dependency/feature baseline
2. bump eggfetch-core to published 0.2.0 + update lockfile
3. inspect solver/dependency diff
4. remove six issue-24 .decompress(false) call sites
5. replace identity-workaround guards with positive compression contracts
6. add deterministic chunked gzip/Brotli read_bounded_body regression
7. re-prove decoded-size bounds + Phase 23 timeout behavior
8. run local correctness/API/security/package gates
9. run bounded DuckDuckGo/Startpage live smoke
10. record dependency/binary characterization
11. commit clean production candidate
12. run seven-target mode=qualify against exact SHA
13. inspect qualification artifact/checksums
14. reconcile current-state docs
15. append implementation record + mark Phase 24 implemented in registry
```

If documentation changes after the production candidate qualification, preserve
the qualified production SHA separately; documentation-only descendants do not
retroactively become qualified release candidates.

## Acceptance criteria

Phase 24 is complete only when all of the following are true:

1. `Cargo.toml` resolves published `eggfetch-core 0.2.0`; no Git/path pin is used.
2. `Cargo.lock` records the intended 0.2.0 dependency graph without unexplained unrelated churn.
3. The bounded eggfetch feature set remains equivalent to the Phase 17 selection.
4. The six known issue-#24 HTML engines no longer call `.decompress(false)` for the historical workaround.
5. No unrelated raw/decompression-disabled behavior is removed without justification.
6. Deterministic tests prove chunked gzip decodes correctly through eggsearch's production-equivalent body path.
7. Deterministic tests prove chunked Brotli decodes correctly through that same path.
8. At least one control proves transfer shape does not change decoded bytes.
9. Eggsearch's decoded-body size bound remains enforced.
10. Total/request deadline semantics remain bounded for compressed chunked responses.
11. Phase 23 equal/shorter shared-client and longer-timeout widened-client semantics remain intact.
12. Direct production reqwest remains absent; the rmcp transitive boundary is unchanged.
13. No eggsearch-owned decompression stack is introduced.
14. No new application retry layer is introduced in eggfetch.
15. Existing SSRF, redirect, sanitization, provider parsing, and MCP contracts remain unchanged.
16. Local correctness, clippy, docs, packaging, release, and publish-dry-run gates pass.
17. Dependency/feature and representative binary-size deltas are recorded.
18. DuckDuckGo and Startpage are smoke-tested when reachable; any live failure is classified rather than silently ignored.
19. A clean immutable candidate is qualified through the full seven-target non-publishing release workflow.
20. All seven target jobs and final exact-asset assembly pass for the same `QUALIFIED_SHA`.
21. Current-state docs no longer describe the issue #24 identity workaround as active.
22. Historical Phase 17/18 records remain truthful and are not rewritten as if they originally used 0.2.0.
23. The implementation record and registry status are updated only after the above evidence exists.

## Handoff notes

Keep this pass narrow. Upstream has already supplied the decoder correction and
regression suite; eggsearch should prove that the published artifact solves its
specific integration failure, remove the temporary policy reduction, and
re-qualify the resulting production candidate.

The most important failure mode to avoid is deleting `.decompress(false)`
because the release notes say the bug is fixed without adding a downstream
integration proof. The second is retaining the workaround indefinitely after
the published fix is proven. The correct endpoint is normal automatic
compression with deterministic seam coverage and no duplicate decoder logic.

## Implementation record

Status: implemented.

- Actual starting SHA: `ef0b28dc1a8d0e8aa0e94060ca0ecce7c669e13a` (main had
  advanced two plan-registration commits past the `0c33d576` planning
  baseline; no production code changed between them).
- Implementation / qualified candidate SHA: `bac6f49fc046a93d7c094a8f96e1027631f390f1`
  (single clean commit, 14 files, +372/-46).
- Resolved `eggfetch-core`: `0.2.0` from crates.io (no Git/path pin),
  `default-features = false`, feature set exactly `standard-http1`,
  `advanced-routing`, `redirects`, `tls-rustls`, `json`, `compression-gzip`,
  `compression-brotli` (identical graph to the 0.1.7 selection; MSRV
  unchanged at 1.89).
- Lock/dependency graph delta: only `eggfetch-core 0.1.7 -> 0.2.0`
  (checksum change) in the production graph, plus test-only `brotli`/`flate2`
  dev-deps with patch-level solver churn (`flate2` 1.1.9 -> 1.1.10,
  `miniz_oxide` 0.8.9 -> 0.9.1, new `zlib-rs` 0.6.8). `cargo tree --invert
  reqwest` confirms rmcp 3.2.0 remains the sole normal-graph reqwest owner.
- Removed workaround call sites (6): `.decompress(false)` deleted from
  `src/meta/engines/brave.rs`, `duckduckgo.rs`, `mojeek.rs`, `searxng.rs`,
  `startpage.rs`, `yahoo.rs`. No other production `decompress(false)` remains.
- Deterministic regression results (`provider_request_contract`, mock):
  21/21 pass, including chunked gzip and chunked Brotli decode through the
  production `build_http_client` + `read_bounded_body()` path with 7-byte
  wire chunks, a `Content-Length` vs chunked parity control for both codecs,
  and a searxng wire test proving automatic gzip/br advertisement.
- Decoded-size-limit and timeout results: tiny-limit rejection, wire-small /
  decoded-large rejection (bound applies to decoded bytes), and a stalled
  compressed-chunked total-deadline test all pass; Phase 23
  equal/shorter-shared vs longer-widened timeout semantics re-proved by the
  existing fetch-client unit tests and static guards (unchanged, passing).
- Live smoke (2026-09-22, bounded curl with `Accept-Encoding: gzip, br`):
  DuckDuckGo returned HTTP/2 202 (bot challenge, 14188 decoded bytes) and
  Startpage returned HTTP/2 200 with `content-encoding: gzip` (7470 decoded
  bytes). Both providers now serve HTTP/2 rather than the historical
  HTTP/1.1 chunked shape; no eggfetch decompression error observed. The
  DuckDuckGo 202 is an environmental challenge response, classified, not a
  regression.
- Local gates on the candidate: `cargo fmt --check` clean; `cargo clippy
  --locked --all-targets --all-features -- -D warnings` zero warnings;
  `cargo check --locked --no-default-features` passes; `cargo test --locked
  --all-features` passes (3235 unit + all integration suites, 0 failures);
  `RUSTDOCFLAGS="-D warnings" cargo doc` clean; `make hygiene` and `make
  packaging-check` pass; `cargo publish --dry-run` passes (pre-commit with
  `--allow-dirty`; clean-tree run equivalent post-commit).
- Binary characterization: default release binary 18,744,464 bytes
  (rustc 1.98.1, x86_64-apple-darwin) vs the Phase 23 baseline 18,744,624
  bytes (-160 bytes, negligible explained delta).
- Seven-target qualification: workflow run `35692096012`
  (`mode=qualify`, `ref=bac6f49fc046a93d7c094a8f96e1027631f390f1`,
  package 0.3.9) at
  <https://github.com/eggstack/eggsearch/actions/runs/35692096012>:
  preflight plus all seven targets plus `Assemble qualify release output`
  (exact asset-set validation) passed; complete qualification artifact
  `qualification-0.3.9-bac6f49fc046a93d7c094a8f96e1027631f390f1-complete`
  (65,525,073 bytes) alongside seven per-target artifacts.
- CI on main for the candidate: run `35692064078` (push) passed.
- Deviations/blockers: none. Upstream issue state recorded per the planning
  baseline (fixed by `37ab02b`, released in eggfetch 0.2.0); no upstream
  mutation was made or required.
