# B003 — Distribution and Live Qualification — Closure

Status: closed
Recommendation: closed; all M003 acceptance gates passed on the exact candidate. PR-linked Python run 43897293446 attempt 7 and all displayed PR checks passed. PR #16 merged on 2026-11-16T21:05:25Z as `aea34eeb89749e3196712d9e9b9741f75c58e760`; no release was published.
Implementation candidate: `fb99a43734bd4f2329bdfa2ba4315b403c2cd985`
Plan: `plans/implementation/browser-artifact-availability/003-distribution-and-live-qualification.md`
Roadmap: `plans/subsystems/browser-artifact-availability-roadmap.md` M003
ADR: `plans/adrs/ADR-0014-browser-artifact-availability-and-render-bounds.md`

## Candidate disposition

Qualification is SHA-specific. This record now tracks `fb99a43734bd4f2329bdfa2ba4315b403c2cd985`. Earlier hosted results on `cdc9445ae121d4f0e0508be9df1b94c3289621f0` and prior candidates are historical and do not qualify this candidate.

The approved public fixture, macOS arm64 installer/release smoke, and live ten-tool MCP checks have been rerun locally on `fb99a43` and passed. The exact-SHA Windows job on `cdc9445` exposed a line-ending-sensitive assertion in `packaging/test-install.ps1:12`: the test expected LF while the Windows checkout supplied CRLF. Commit `fb99a43` normalizes line endings before checking mappings. On this exact SHA, hosted CI (`43897293997`), egress (`43897294001`), and Eggpack drift (`43897293439`) passed. Qualifier run `43901942424`, attempt 2, completed successfully: all seven builds, qualification stages, and consumer validations passed, including the previously unavailable aarch64 macOS consumer check. `required_gate` job `133470803916` and `aggregate` job `133471719996` passed; the aggregate outcome is `complete`, and artifact `eggpack-candidate-qualified` records source revision `fb99a43734bd4f2329bdfa2ba4315b403c2cd985` and the seven-target manifest. Python full-matrix run `43916322850`, attempt 4, completed successfully on this exact SHA with all 34 jobs passing. Earlier attempts had two aarch64 macOS consumer cells cancelled before runner assignment; targeted same-SHA retries completed them. The PR-associated Python run `43897293446`, attempt 7, passed all 34 jobs, and all displayed PR checks are green. PR #16 merged as `aea34eeb89749e3196712d9e9b9741f75c58e760` on 2026-11-16T21:05:25Z. Earlier-SHA results do not transfer.

## Exact-candidate evidence — `fb99a43734bd4f2329bdfa2ba4315b403c2cd985`

| Requirement | Evidence | Outcome |
|---|---|---|
| Candidate identity | Commit `fb99a43734bd4f2329bdfa2ba4315b403c2cd985`; exact-SHA evidence is recorded. Qualification ran with a clean worktree and no source changes | pass |
| macOS arm64 release binary | `cargo build --locked --release --bin eggsearch`; version `0.4.4`; SHA-256 `f6dcee6b3bce92c96d7dc0d89c56e3bc7bd5e8f5abe26beb20cbbfb926a43ac3` | pass |
| Approved public realistic-mix fixture | `target/m003-public-fixture/evidence-candidate-fb99a43/evidence.json`: 294 samples, 39 expected browser escalations, HTTP static/PDF paths, cap teardown after render 13, relaunch on render 14, idle teardown, final process cleanup; sampled peak descendant RSS 1,180,352 KiB | pass |
| macOS arm64 installer and release smoke | `target/m003-installer-smoke/evidence-candidate-fb99a43/installer-smoke-macos-arm64.json`; checksum-verified install and release smoke passed; binary hash matches above. `packaging/release-smoke.sh` also passed | pass |
| Live ten-tool MCP verification | `target/m003-live-mcp/evidence-candidate-fb99a43.json`: ten advertised tools, all four behavior hints, canonical-only `web_fetch` schema; 14 calls; PDF fetch returned 200; JavaScript fixture returned 200 with `browser_escalated=true` and `transport=browser`; aliases passed and duplicate canonical-plus-alias input was rejected | pass |
| Windows PowerShell installer and release-smoke wrappers | Exact-SHA run `43901942424`, job `133432522588`; artifact `eggpack-windows-wrapper-evidence-fb99a43734bd4f2329bdfa2ba4315b403c2cd985` records source revision `fb99a43734bd4f2329bdfa2ba4315b403c2cd985`, target `x86_64-pc-windows-msvc`, release `v0.4.4`, binary SHA-256 `7c04b9504afda6db9796fe2ee079f7bf9743b1dc8531c1ea584335e2ae91d344`, PowerShell `7.6.9`, `test_install=passed`, and `release_smoke=passed`. PowerShell is unavailable locally | pass |
| Hosted seven-target qualification | Run `43901942424`, attempt 2, on exact SHA `fb99a43734bd4f2329bdfa2ba4315b403c2cd985`: all seven builds, qualification stages, and consumer validations passed, including aarch64 macOS; Windows installer/release-smoke wrappers passed and uploaded evidence. `required_gate` job `133470803916` and `aggregate` job `133471719996` passed. Aggregate summary is `complete`; artifact `eggpack-candidate-qualified` contains the seven-target manifest | pass |
| Python matrix | Full-matrix run `43916322850`, attempt 4, completed successfully with all 34 jobs passing on exact SHA `fb99a43734bd4f2329bdfa2ba4315b403c2cd985`: five CPython 3.11 native wheel builds, ARMv7 build and emulated smoke, extracted-sdist rebuild/smoke, dependency audit, and all 25 installed-wheel consumer cells across CPython 3.11–3.15. Target wheel byte sizes and the sdist size are recorded below. The Python lock contains `lopdf` and no `chromiumoxide` or `reqwest`; `cargo deny` and `pip-audit` passed. PR-linked run `43897293446`, attempt 7, passed all 34 jobs on this exact SHA and all displayed PR checks are green; PR #16 merged as `aea34ee` | pass |
| Hosted CI, egress, and Eggpack drift | Exact-candidate runs `43897293997` (CI), `43897294001` (egress), and `43897293439` (Eggpack drift) passed | pass |
| Local installer wrapper limitation | `packaging/test-install.sh` remains blocked on this workstation by `mavis-trash: unsupported OS: Plan9`; do not bypass the guard. The exact-SHA hosted CI run `43897293997` passed `make ci`, which includes the packaging contract and Unix installer test. Windows wrappers passed in qualifier run `43901942424`, attempt 2 | blocked locally; hosted checks passed |

## Exact-candidate Python distribution inventory — `fb99a43734bd4f2329bdfa2ba4315b403c2cd985`

Run `43916322850`, attempt 4, completed on the exact candidate. Wheel artifact files were measured from the downloaded artifacts; their SHA-256 manifests are included in the corresponding `wheel-*` artifacts. The extracted source distribution was also rebuilt offline and smoke-tested.

| Target | Wheel bytes |
|---|---:|
| `x86_64-unknown-linux-gnu` | 10,853,030 |
| `aarch64-unknown-linux-gnu` | 10,613,967 |
| `armv7-unknown-linux-gnueabihf` | 10,250,509 |
| `x86_64-apple-darwin` | 10,040,349 |
| `aarch64-apple-darwin` | 9,704,043 |
| `x86_64-pc-windows-msvc` | 9,521,344 |
| Source distribution | 1,509,523 |

## Exact-candidate seven-target size inventory — `fb99a43734bd4f2329bdfa2ba4315b403c2cd985`

The finalized manifest is in artifact `eggpack-candidate-qualified` from run `43901942424`, attempt 2; its `source_revision` is the exact candidate SHA and its aggregate outcome is `complete`. Baselines are the previous published `v0.4.4` assets.

| Target | Baseline bytes | Candidate bytes | Delta bytes | Delta |
|---|---:|---:|---:|---:|
| `x86_64-unknown-linux-gnu` | 21,457,240 | 28,983,792 | +7,526,552 | +35.1% |
| `aarch64-unknown-linux-gnu` | 18,524,080 | 24,914,040 | +6,389,960 | +34.5% |
| `armv7-unknown-linux-gnueabihf` | 17,710,392 | 24,172,376 | +6,461,984 | +36.5% |
| `x86_64-pc-windows-msvc` | 25,869,312 | 34,973,184 | +9,103,872 | +35.2% |
| `aarch64-pc-windows-msvc` | 21,655,040 | 29,321,728 | +7,666,688 | +35.4% |
| `x86_64-apple-darwin` | 19,779,368 | 26,781,072 | +7,001,704 | +35.4% |
| `aarch64-apple-darwin` | 17,960,304 | 24,299,360 | +6,339,056 | +35.3% |

The manifest records per-target SHA-256 values. The hosted macOS arm64 artifact size is the Eggpack build result; the separate native local build used for installer/release smoke is recorded above.

## Final disposition

1. The seven-target qualifier is complete on the exact candidate: run `43901942424`, attempt 2, including aarch64 macOS validation, `required_gate`, and aggregation. Its exact-SHA size inventory is recorded above.
2. The Python full matrix is complete on the same candidate: run `43916322850`, attempt 4, all 34 jobs passed; wheel sizes and dependency-audit evidence are recorded above.
3. All original M003 acceptance criteria are satisfied on `fb99a43734bd4f2329bdfa2ba4315b403c2cd985`, including the local realistic mix, macOS arm64 installer/release smoke, live ten-tool MCP, hosted CI (`43897293997`), egress (`43897294001`), Eggpack drift (`43897293439`), Eggpack qualification, Python matrix, and Windows wrappers.
4. PR #16 merged on 2026-11-16T21:05:25Z as `aea34eeb89749e3196712d9e9b9741f75c58e760`, from head `fb99a43734bd4f2329bdfa2ba4315b403c2cd985` into base `cd81f4d5e966364918fe2be25485b8ebbf424e71`. PR-linked Python run `43897293446`, attempt 7, had passed all 34 jobs and all displayed PR checks before the merge. An earlier normal-merge attempt was refused with “the base branch policy prohibits the merge” while the combined commit-status endpoint reported pending with no statuses; the merge outcome is recorded here. No release was published.

## Historical exact-candidate evidence — `62387208ac8bc20efb545b68f0704f466bab7295`

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

## Historical seven-target size inventory — `62387208ac8bc20efb545b68f0704f466bab7295`

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

## Historical wheel failure detail — `62387208ac8bc20efb545b68f0704f466bab7295`

The failing assertion compares canonical Rust tool-schema property names against the Python `TypedDict` request annotations. Target wheel builds and the dependency audit pass, but the contract smoke does not. This is a Python binding/schema contract failure, not a reason to weaken the test or change browser behavior in M003. Return it to the owning Python binding workstream, correct it there, then rerun the complete wheel matrix against a new exact candidate.

## Historical realistic-mix limitation — `62387208ac8bc20efb545b68f0704f466bab7295`

The attempted harness uses the fixture counts and config prescribed by M003. The HTTP path accepts local/private allowances, but `src/fetch/browser/intercept.rs` independently rejects localhost, private IP literals, and DNS answers resolving to private addresses. Consequently the local JavaScript fixture cannot exercise browser escalation, and there is no valid escalation, latency, RSS, teardown, or relaunch data from this attempt. Do not weaken production SSRF controls as a qualification-only workaround. A fixture path compatible with the existing browser policy or an owner-reviewed policy change is needed before this matrix can pass.

## Historical remaining conditions — `62387208ac8bc20efb545b68f0704f466bab7295`

1. Correct the Python request/schema contract in its owning workstream and rerun the complete wheel/sdist smoke matrix on the new exact candidate.
2. Obtain realistic-mix evidence using fixtures that can reach the browser without violating its private-network policy; record escalation, latency, RSS, teardown, and relaunch metrics.
3. Hosted CI passed on the exact candidate. Eggpack run `43738031441` concluded `cancelled`: six non-macOS-arm64 targets passed build, qualification, and consumer validation, while the arm64 job was cancelled before steps because `macos-14-arm64-us-default` is unsupported. The generated release workflow uses the same `macos-14` label; return this to the Eggpack producer owner and requalify after that workflow is corrected. The local arm64 build/size/installer evidence does not replace the hosted producer job.
4. Run the live ten-tool MCP verification on the exact candidate, including PDF fetch and an escalated render. Run `packaging/test-install.sh`, `packaging/test-install.ps1`, and `packaging/release-smoke.ps1` to complete the standard installer/release wrapper matrix.
5. Only after every acceptance criterion passes, update the closure to `closed`, then revisit publication/push.

At the time of this historical candidate evaluation, M003 remained blocked. The current candidate status and recommendation are recorded above.
