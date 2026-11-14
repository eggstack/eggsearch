# B003 — Distribution and Live Qualification — Closure

Status: conditionally closed
Recommendation: conditionally closed
Implementation candidate: `5605cc240349c65b6442fa73569516cad8e2ad95`
Plan: `plans/implementation/browser-artifact-availability/003-distribution-and-live-qualification.md`
Roadmap: `plans/subsystems/browser-artifact-availability-roadmap.md` M003
ADR: `plans/adrs/ADR-0014-browser-artifact-availability-and-render-bounds.md`

## Implementation

- `bindings/python/Cargo.toml` builds the wheel with
  `default-features = false, features = ["pdf"]`.
- `bindings/python/Cargo.lock` gained `lopdf` and contains no `chromiumoxide`
  and no `reqwest`; it was also refreshed from `rmcp 3.2.0` to `3.9.0`, which the
  current source requires.
- Documentation records the installed-binary and wheel asymmetry.

## Wheel evidence

| Check | Evidence | Outcome |
|---|---|---|
| Wheel crate compiles locked with pdf only | `cargo check --locked --manifest-path bindings/python/Cargo.toml` | pass |
| No browser dependency in the wheel | lockfile: `chromiumoxide` = 0, `reqwest` = 0, `lopdf` = 1 | pass |
| Wheel matrix | `python-native-qualification.yml` not dispatched for this candidate | outstanding |

## Seven-target evidence (candidate `5605cc2`)

Sizes are from the hosted candidate qualification run `43567654611`; baselines
are the published `v0.4.4` assets.

| Target | Baseline | Candidate | Delta | Status |
|---|---:|---:|---:|---|
| `x86_64-unknown-linux-gnu` | 21,457,240 | 28,982,584 | +7,525,344 (+35.1%) | built, qualified natively, consumer evidence passed |
| `aarch64-unknown-linux-gnu` | 18,524,080 | 24,912,912 | +6,388,832 (+34.5%) | built, qualified natively, validated |
| `armv7-unknown-linux-gnueabihf` | 17,710,392 | 24,171,000 | +6,460,608 (+36.5%) | built, qualified structurally, validated |
| `x86_64-pc-windows-msvc` | 25,869,312 | 34,971,648 | +9,102,336 (+35.2%) | built, qualified, validated |
| `x86_64-apple-darwin` | 19,779,368 | — | — | build still running at closure time |
| `aarch64-apple-darwin` | 17,960,304 | — | — | build job cancelled by the runner |
| `aarch64-pc-windows-msvc` | 21,655,040 | — | — | build passed; validation still running at closure time |

The measured deltas are larger than the macOS arm measurement in
`docs/retrieval-quality-extraction-qualification.md` (+30.6%) because the
release profile, target, and dependency set differ. They are recorded, not
asserted as a threshold.

## Live verification (final candidate)

A scripted MCP stdio session against the default-feature binary completed
**29/29 checks**:

- `initialize` and `tools/list`: ten tools, all four behaviour hints,
  canonical-only schema properties.
- `web_search` returned five cards from duckduckgo, brave, and yahoo; brave's
  429 remained a warning.
- `web_fetch` returned example.com; a PDF returned `application/pdf` with
  extracted text; the local alias forms worked and the duplicate
  canonical-plus-alias form was rejected.
- `web_fetch` with `render = "auto"` escalated a public JavaScript shell
  (`https://excalidraw.com/`) to the browser transport.
- Idle teardown: after `idle_timeout_ms = 5000` and an 8 s wait,
  `idle_teardowns = 1`, `resident = false`, and a second call escalated again.
- `batch_fetch`, `repo_map` (native, 17 root entries), `repo_fetch`,
  `repo_search`, `security_search` (CVE-2024-3094), `research_search`,
  `provider_status` (44 providers), and `build_evidence_bundle` (deterministic
  bundle IDs) all passed.

Process-tree RSS during the escalated render was 833,264 KiB of Chrome
descendants on the macOS arm64 host; after the idle interval it was 0.

## Requirement-to-evidence

| Requirement | Evidence | Outcome |
|---|---|---|
| Wheel carries PDF, not browser | lockfile and compile check | pass |
| One executable per target, no asset change | candidate build jobs use the unchanged producer | pass |
| armv7 build | hosted build and structural qualification | pass |
| Linux native qualification | x86_64 and aarch64 `status = passed`, `support = required` | pass |
| Windows qualification | x86_64 qualified and validated | pass |
| macOS qualification | x86_64 running, aarch64 cancelled | outstanding |
| Installer smoke | x86_64 Linux consumer evidence `outcome = passed`; other targets not run | partial |
| Realistic-mix qualification | not run as a multi-case matrix; one live escalated case and one teardown observation recorded | outstanding |
| Live ten-tool verification | 29/29 scenario checks | pass |
| Hosted CI | run `43567644057`: Windows portability and Linux `ci` succeeded; macOS canonical pending | partial |

## Migration and compatibility

- The wheel is a separate artifact; the binary release inventory is unchanged.
- Python users who need rendering must use the Rust binary or a source build.
  This is documented in `docs/python-library.md` and `docs/features.md`.

## Security

- The wheel does not ship a browser child-process path or a second HTTP client.
- The binary keeps the B001/B002 bounds.

## Known limitations and outstanding conditions

1. `aarch64-apple-darwin` was cancelled by the hosted runner after one second;
   the cause is external to the candidate and the job must be re-run.
2. `x86_64-apple-darwin` and `aarch64-pc-windows-msvc` were still building or
   validating when this record was written.
3. Installer smoke is recorded only for `x86_64-unknown-linux-gnu`.
4. The realistic-mix qualification (static, JavaScript, challenge, and PDF
   fixtures with escalation rate and p95 latency) was not run as a matrix; the
   live session and the single teardown observation are the evidence so far.
5. Chrome version dependence remains an environment observation, not a
   threshold.

## Recommendation

Conditionally closed. The default artifacts, the wheel asymmetry, and the live
ten-tool surface are verified on the candidate, and four of seven targets have
build and size evidence. The named conditions above must be discharged before
this milestone can move to `closed`.
