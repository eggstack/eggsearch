# B003 — Distribution and Live Qualification

Status: ready
Source: `plans/subsystems/browser-artifact-availability-roadmap.md` M003
Primary class: infrastructure and evidence
Baseline: `f265f08` (branch `feat/auto-20261110-0a674cdc`)
References: `plans/adrs/ADR-0014-browser-artifact-availability-and-render-bounds.md` § Decision 7, § Verification; `plans/adrs/ADR-0007-pdf-artifact-availability.md` § Verification gate; `plans/implementation/pdf-ingestion-fidelity/006-artifact-availability.md`; `docs/retrieval-quality-extraction-qualification.md`; `docs/release.md`; `docs/installation.md`; `docs/python-library.md`; `architecture/packaging.md`.
Dependencies: hard B002 (default artifact availability). Interface: PDF roadmap M006 owns the PDF artifact acceptance; this plan supplies the wheel selection it must adopt. Operational: system Chrome/Chromium on the qualification host; hosted CI.

## Objective

Prove on one exact candidate that the new default artifacts are correct and
bounded: measure the seven-target size delta, decouple the Python wheel so it
carries PDF and not browser, re-run the extraction qualification with a realistic
escalation mix and idle teardown, complete installer and wheel smoke, pass hosted
CI, and re-verify all ten MCP tools on a live run including one PDF fetch and one
escalated render.

## Non-goals

- No production behaviour change. If a gate fails, return to the owning milestone
  rather than fixing it here.
- No change to `release/eggpack/` inputs or the generated release workflow.
- No new asset, asset name, or inventory entry.
- No browser capability in the wheel; no wheel-side Chrome discovery.
- No qualification threshold that would silently redefine "acceptable"; the
  record reports measurements and the maintainer judges them against ADR-0014.
- No PDF extraction change; PDF M006 owns fidelity and the PDF half of the size
  evidence.

## Current implementation evidence

- `.github/workflows/qualify-eggpack-candidate.yml` accepts an exact 40-character
  `candidate_sha`, builds all seven targets, and performs qualification without
  release writes. It is the vehicle for per-target size evidence.
- `release/eggpack/build-bindings.toml` requests one `direct` output per target
  with no flags; `packaging/release-targets.txt` lists the seven targets;
  `packaging/expected_release_assets.py` derives the 19-name inventory. Adding a
  feature to `default` needs no producer edit.
- `bindings/python/Cargo.toml` depends on `eggsearch` by path with no
  `default-features = false`, so the wheel inherits the default. The qualification
  workflow builds with `maturin build --locked`.
- `docs/retrieval-quality-extraction-qualification.md` records the four-arm
  measurement, the six-call-per-fixture design, the Chrome version, and the
  evidence file layout under
  `plans/closure/retrieval-quality-evidence-extension/evidence/`.
- The recorded browser arm used `render: "http_only"` for static HTML and both PDF
  fixtures and `render: "browser"` only for the JavaScript-only fixture. That is
  the routing the realistic mix must preserve and measure.
- `packaging/test-install.sh`, `packaging/test-install.ps1`,
  `packaging/release-smoke.sh`, and `packaging/release-smoke.ps1` exist and are the
  installer and asset smoke vehicles.
- `docs/python-library.md` does not currently state a browser asymmetry because
  none exists.

## Invariants that cannot regress

- Ten tools and every existing schema, argument, and response shape.
- One executable per target, version-free names, 19 assets.
- `src/` reqwest-free; default `reqwest` only through chromiumoxide.
- Explicit browser requests fail with `browser_not_compiled_in` or
  `browser_disabled`, never a silent HTTP substitution.
- Idle teardown and `max_requests` remain effective in every measured arm.
- No interactive challenge solving in any qualification fixture.
- Feature-absent builds keep their refusal codes and compile.

## Expected changes

1. **Wheel feature selection.** `bindings/python/Cargo.toml` gains
   `default-features = false, features = ["pdf"]`. Regenerate
   `bindings/python/Cargo.lock`; it must gain `lopdf` and must not gain
   `chromiumoxide` or `reqwest`.
2. **Documentation.** `docs/python-library.md` states the asymmetry; `docs/features.md`
   and `docs/installation.md` state that installed binaries include PDF and
   browser and that the wheel includes PDF only; `docs/release.md` records the new
   default feature set; `README.md` and `CHANGELOG.md` follow.
3. **Qualification harness (not committed as a product path).** A recorded,
   reproducible procedure using local fixtures:
   - 30 static HTML pages;
   - 10 JavaScript-shell pages that require rendering;
   - 3 text PDFs and 3 scanned PDFs;
   - 3 synthetic non-interactive challenge walls.
   Use a local origin with `allow_localhost` and `allow_private_network` enabled
   only for the run, and record the config. Preserve the existing evidence JSON
   shape and file naming under the workstream's closure evidence directory.
4. **Metrics per arm.** Escalation count and rate, cold and warm p50/p95 latency
   split by static, JavaScript, PDF, and challenge; peak process-tree RSS during
   rendering; RSS after `idle_timeout_ms` plus one poll interval; relaunch latency
   after teardown; `max_requests` session behaviour; per-target binary size.
5. **Live MCP verification.** A scripted stdio run against the candidate binary
   covering: `initialize`, `tools/list` (ten tools, all four behavior hints,
   canonical-only schema properties), and one live call of every tool. The run
   must include one PDF `web_fetch`, one `web_fetch` with `render: "auto"` that
   escalates, the alias forms (`max_length`, `start_index`, `question`), and the
   duplicate canonical-plus-alias rejection. Record per-tool outcome, latency, and
   a bounded response summary; do not embed page content beyond a short marker.
6. **Installer and release smoke.** Run `packaging/test-install.sh` and
   `packaging/test-install.ps1` against the qualified assets, and
   `packaging/release-smoke.sh` on the candidate assets.

## Storage, protocol, and compatibility effects

- Wheel behaviour changes: PDF remains available, browser does not.
- No binary asset change beyond the feature default already shipped by B002.
- No wire or response-shape change.
- The qualification record is evidence, not a product path; no fixture server,
  script, or config is committed into production source.

## Ordered work packages

1. Decouple the wheel, regenerate the lock, and verify the dependency delta.
2. Run the candidate qualification workflow at the exact SHA and collect the seven
   binaries and their sizes.
3. Run the realistic-mix qualification with teardown and record the JSON evidence.
4. Run wheel build and matrix (`python-native-qualification.yml`) and record the
   wheel size and dependency audit.
5. Run installer and release smoke on both platforms.
6. Run the live MCP verification and record the per-tool table.
7. Run hosted `ci.yml` on the candidate and record run IDs.
8. Write documentation updates and the closure record.

## Verification

```bash
python3 packaging/check-http-dependency-shape.py
python3 packaging/check-planning-consistency.py
cargo test --locked --all-features --test browser_transport
cargo test --locked --all-features --test web_fetch_integration
cargo test --locked --features mock --test web_fetch_integration pdf
make check
make packaging-check
make producer-drift
gh workflow run qualify-eggpack-candidate.yml -f candidate_sha=<sha>
gh workflow run ci.yml --ref <branch>
```

`make check` stops at the workstation-only `mavis-trash: unsupported OS: Plan9`
in `packaging/check-contract.sh`; every check before that point must pass and the
known stop must be recorded.

## Static guards and documentation

- `packaging/check-http-dependency-shape.py` must show the allowed root set in
  both graphs and the wheel lock must not contain `chromiumoxide` or `reqwest`.
- `packaging/check-dependency-policy.sh` must pass; the wheel dependency audit is
  part of the Python qualification lane.
- `docs/test-inventory.md`, `architecture/testing.md`, and
  `skills/eggsearch-dev/SKILL.md` only if a committed suite is added. The
  qualification harness stays out of product source.
- Docs listed in Expected changes § 2.

## Acceptance and stop conditions

Accept when:

- All seven targets build with the new default and their sizes are recorded.
- The wheel contains PDF support and no `chromiumoxide`/`reqwest`, and its matrix
  passes.
- The realistic mix shows escalation only on JavaScript-shell and challenge
  fixtures, static and PDF fetches stay on HTTP, peak RSS is recorded, and the
  post-teardown footprint returns to roughly the HTTP-only baseline.
- Installer and release smoke pass on both platforms.
- The live MCP run passes all ten tools, the four hints, canonical-only schema
  properties, the three alias forms, the duplicate rejection, one PDF fetch, and
  one escalated render.
- Hosted `ci.yml` passes on the candidate, and `make check`, `make packaging-check`,
  and `make producer-drift` pass apart from the recorded workstation stop.

Stop and report if:

- Any target fails to build with the default feature set.
- The wheel gains `chromiumoxide` or `reqwest`, or the wheel matrix fails.
- The realistic mix escalates on static or PDF fixtures, or teardown does not
  return the footprint to baseline.
- Any of the ten tools fails its live call, or a schema property is no longer
  canonical-only.
- Any regression appears in the ten-tool surface, trust markers, SSRF policy, or
  cache behaviour.

## Required closure evidence

Exact candidate SHA; seven-target build inventory and size table with deltas
against the previous default; the realistic-mix evidence files with the metrics
listed above; wheel matrix run ID, wheel size, and dependency delta; installer and
release smoke outcomes; hosted CI run IDs; the live per-tool verification table
with the PDF and escalated-render markers; documentation updated; known
limitations (armv7 runtime absence, Chrome version dependence, wheel asymmetry,
residual reqwest edge) classified by severity; recommendation.
