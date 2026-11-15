# B003 — Distribution and Live Qualification — Closure

Status: blocked
Recommendation: blocked; do not publish or push until the failed gates are returned to their owners and requalified.
Implementation candidate: `62387208ac8bc20efb545b68f0704f466bab7295`
Plan: `plans/implementation/browser-artifact-availability/003-distribution-and-live-qualification.md`
Roadmap: `plans/subsystems/browser-artifact-availability-roadmap.md` M003
ADR: `plans/adrs/ADR-0014-browser-artifact-availability-and-render-bounds.md`

## Candidate disposition

Qualification is SHA-specific. This record now tracks `62387208ac8bc20efb545b68f0704f466bab7295`; prior live-tool and resource evidence for `5605cc240349c65b6442fa73569516cad8e2ad95` is retained below only as historical evidence and does not qualify this candidate.

The wheel matrix is a stop-condition failure: the wheel builds and dependency audit pass, but all installed-wheel smoke jobs and the extracted-sdist smoke fail at `test_typed_requests_cover_canonical_rust_input_schemas` (`bindings/python/tests/test_native.py:139`). The realistic-mix attempt also did not produce qualification evidence: the local fixture stayed on HTTP, while explicit browser rendering was denied by the browser network interceptor's private/local-target policy (`src/fetch/browser/intercept.rs:23-40`). The hosted Eggpack arm64 job was cancelled before steps because runner configuration `macos-14-arm64-us-default` is unsupported; the generated release workflow uses the same `macos-14` label. Return that runner issue to the Eggpack producer owner rather than altering the generated workflow here. No production behavior was changed in this qualification pass.

## Exact-candidate evidence

| Requirement | Evidence | Outcome |
|---|---|---|
| Candidate identity | Detached worktree at `62387208ac8bc20efb545b68f0704f466bab7295`; local source tree clean | pass |
| Default-feature release binary on native macOS arm64 | `cargo build --locked --release --bin eggsearch`; version `0.4.4`; SHA-256 `4fea05ad8acf6c0da1a3a9ab424f80db826612837666884e0acdf5f68ae9946b`; size 24,860,256 bytes | pass |
| macOS arm64 installer and release smoke | `evidence/realistic-mix-001/installer-smoke-macos-arm64.json`; `packaging/install.sh` checksum-verified a temporary loopback asset, installed into a temporary home with no Cargo fallback; `packaging/release-smoke.sh` stdio and loopback HTTP MCP checks passed | pass |
| Hosted Eggpack target qualification | Exact-candidate [run 43738031441](https://github.com/eggstack/eggsearch/actions/runs/43738031441) concluded `cancelled`: six non-macOS-arm64 targets passed build/qualification and consumer validation; the macOS arm64 build was cancelled before steps because its runner configuration is unsupported. `aggregate` and `required_gate` were skipped. Raw qualification and consumer records are in `evidence/eggpack-43738031441.json`. | partial |
| Python wheel build and dependency audit | [Run 43734943969](https://github.com/eggstack/eggsearch/actions/runs/43734943969) on this candidate: configured wheel builds and dependency audit succeeded; wheel lock remains PDF-only, without `chromiumoxide` or `reqwest` | pass |
| Python installed-wheel and sdist smoke | [Run 43734943969](https://github.com/eggstack/eggsearch/actions/runs/43734943969): all installed-wheel smoke jobs and extracted-sdist smoke fail at `set(properties).issubset(request_type.__annotations__)` in `bindings/python/tests/test_native.py:139` | **fail; stop condition** |
| Realistic-mix matrix | `evidence/realistic-mix-001/qualify.py` and `run-config.toml` record the attempted local-fixture procedure. No successful `evidence.json` was produced. `auto` stayed on HTTP; explicit rendering failed because browser interception blocks private/local destinations even though the HTTP fetch config allowed localhost/private networks. | **blocked; no metrics qualify** |
| Hosted CI | [Run 43736811317](https://github.com/eggstack/eggsearch/actions/runs/43736811317) on exact candidate `62387208ac8bc20efb545b68f0704f466bab7295`: routine CI, Windows portability, and macOS canonical/local-filesystem checks all passed | pass |
| Live ten-tool MCP verification | Not rerun against this candidate. The predecessor candidate's 29/29 result below does not transfer. | outstanding |
| Standard installer/release-smoke wrappers | `packaging/test-install.sh`, `packaging/test-install.ps1`, and `packaging/release-smoke.ps1` were not run on this exact candidate; the local Mac check invoked `packaging/install.sh` and `packaging/release-smoke.sh` directly | outstanding |

## Exact-candidate seven-target size inventory

Sizes are uncompressed executable bytes. Baselines are the published `v0.4.4` assets. Six target results are from Eggpack run `43738031441`; macOS arm64 is from the native release build of the exact candidate on this host because its hosted job could not start. Hashes and per-target qualification/consumer outcomes are preserved in `evidence/eggpack-43738031441.json` and `evidence/realistic-mix-001/installer-smoke-macos-arm64.json`.

| Target | Baseline | Candidate | Delta | Exact-candidate evidence |
|---|---:|---:|---:|---|
| `x86_64-unknown-linux-gnu` | 21,457,240 | 28,983,792 | +7,526,552 (+35.1%) | hosted build, qualification, consumer validation passed |
| `aarch64-unknown-linux-gnu` | 18,524,080 | 24,914,040 | +6,389,960 (+34.5%) | hosted native build, qualification, consumer validation passed |
| `armv7-unknown-linux-gnueabihf` | 17,710,392 | 24,172,504 | +6,462,112 (+36.5%) | hosted structural qualification and consumer validation passed |
| `x86_64-pc-windows-msvc` | 25,869,312 | 34,973,184 | +9,103,872 (+35.2%) | hosted build, qualification, consumer validation passed |
| `aarch64-pc-windows-msvc` | 21,655,040 | 29,321,728 | +7,666,688 (+35.4%) | hosted native build, qualification, consumer validation passed |
| `x86_64-apple-darwin` | 19,779,368 | 26,781,072 | +7,001,704 (+35.4%) | hosted native build, qualification, consumer validation passed |
| `aarch64-apple-darwin` | 17,960,304 | 24,860,256 | +6,899,952 (+38.4%) | native local build, installer and release smoke passed; hosted job blocked by unsupported runner configuration |

The arm64 binary is the same candidate binary used for the direct installer smoke. That smoke addresses the prior requirement for installer evidence beyond Linux x86_64. The standard wrapper scripts remain outstanding, so the plan's full cross-platform installer acceptance is incomplete.

## Historical seven-target evidence — predecessor candidate only

The following measurements are from run `43567654611` and candidate `5605cc240349c65b6442fa73569516cad8e2ad95`. They are preserved for comparison and do not qualify the current candidate. Baselines are the published `v0.4.4` assets.

| Target | Baseline | Predecessor candidate | Delta | Status on predecessor |
|---|---:|---:|---:|---|
| `x86_64-unknown-linux-gnu` | 21,457,240 | 28,982,584 | +7,525,344 (+35.1%) | built, qualified natively, consumer evidence passed |
| `aarch64-unknown-linux-gnu` | 18,524,080 | 24,912,912 | +6,388,832 (+34.5%) | built, qualified natively, validated |
| `armv7-unknown-linux-gnueabihf` | 17,710,392 | 24,171,000 | +6,460,608 (+36.5%) | built, qualified structurally, validated |
| `x86_64-pc-windows-msvc` | 25,869,312 | 34,971,648 | +9,102,336 (+35.2%) | built, qualified, validated |
| `x86_64-apple-darwin` | 19,779,368 | 26,776,776 | +6,997,408 (+35.4%) | built, qualified, validated |
| `aarch64-apple-darwin` | 17,960,304 | — | — | hosted job cancelled |
| `aarch64-pc-windows-msvc` | 21,655,040 | 29,320,704 | +7,665,664 (+35.4%) | built, qualified, validated |

## Historical live verification — predecessor candidate only

A scripted MCP stdio session against candidate `5605cc240349c65b6442fa73569516cad8e2ad95` completed 29/29 checks. This result is not exact-candidate evidence for `62387208…`:

- `initialize` and `tools/list`: ten tools, all four behaviour hints, canonical-only schema properties.
- `web_search` returned five cards from DuckDuckGo, Brave, and Yahoo; Brave's 429 remained a warning.
- `web_fetch` returned example.com and extracted a PDF; aliases worked and duplicate canonical-plus-alias input was rejected.
- `web_fetch` with `render = "auto"` escalated the public JavaScript shell at `https://excalidraw.com/` to browser transport.
- With `idle_timeout_ms = 5000`, an 8 s wait produced `idle_teardowns = 1`, `resident = false`; a later call escalated again.
- `batch_fetch`, `repo_map`, `repo_fetch`, `repo_search`, `security_search`, `research_search`, `provider_status`, and `build_evidence_bundle` passed.
- Chrome descendant RSS during the render was 833,264 KiB on the macOS arm64 host and zero after the idle interval.

## Wheel failure detail

The failing assertion compares canonical Rust tool-schema property names against the Python `TypedDict` request annotations. Target wheel builds and the dependency audit pass, but the contract smoke does not. This is a Python binding/schema contract failure, not a reason to weaken the test or change browser behavior in M003. Return it to the owning Python binding workstream, correct it there, then rerun the complete wheel matrix against a new exact candidate.

## Realistic-mix limitation

The attempted harness uses the fixture counts and config prescribed by M003. The HTTP path accepts local/private allowances, but `src/fetch/browser/intercept.rs` independently rejects localhost, private IP literals, and DNS answers resolving to private addresses. Consequently the local JavaScript fixture cannot exercise browser escalation, and there is no valid escalation, latency, RSS, teardown, or relaunch data from this attempt. Do not weaken production SSRF controls as a qualification-only workaround. A fixture path compatible with the existing browser policy or an owner-reviewed policy change is needed before this matrix can pass.

## Remaining conditions and recommendation

1. Correct the Python request/schema contract in its owning workstream and rerun the complete wheel/sdist smoke matrix on the new exact candidate.
2. Obtain realistic-mix evidence using fixtures that can reach the browser without violating its private-network policy; record escalation, latency, RSS, teardown, and relaunch metrics.
3. Hosted CI passed on the exact candidate. Eggpack run `43738031441` concluded `cancelled`: six non-macOS-arm64 targets passed build, qualification, and consumer validation, while the arm64 job was cancelled before steps because `macos-14-arm64-us-default` is unsupported. The generated release workflow uses the same `macos-14` label; return this to the Eggpack producer owner and requalify after that workflow is corrected. The local arm64 build/size/installer evidence does not replace the hosted producer job.
4. Run the live ten-tool MCP verification on the exact candidate, including PDF fetch and an escalated render. Run `packaging/test-install.sh`, `packaging/test-install.ps1`, and `packaging/release-smoke.ps1` to complete the standard installer/release wrapper matrix.
5. Only after every acceptance criterion passes, update the closure to `closed`, then revisit publication/push.

M003 remains blocked. No release or push is authorized by this record.
