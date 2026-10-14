# M006 — M004 Exact-SHA Distribution Qualification and Closure Reconciliation

Status: ready (hard dependency satisfied: M005 closed at `plans/closure/python-native-library/005-status.md`, candidate `baf00cfc8e72f5975010f62e40d5d56985f77c78`; M004 remains active)
Primary class: infrastructure (release qualification and evidence control)
Source: `plans/subsystems/python-native-library-roadmap.md` M006; closes remaining exit conditions of active M004 (`plans/implementation/python-native-library/004-wheel-sdist-and-release-qualification.md`).
Planning baseline: `main` `1434c1e55ddd92947042cff9918ffc0fe130ed01`, 2026-10-14.
Dependencies: M001–M003 closed (`plans/closure/python-native-library/001-status.md` through `003-status.md`); M005 must close first. Operational: hosted exact-SHA CI, wheel artifact and native consumer evidence; publisher authorization is deliberately outside this plan.
ADR: `plans/adrs/ADR-0005-python-native-library-boundary.md`; normative `plans/003-planning-process.md` sections 2.4, 2.5, 7–10; `plans/000-long-term-specification.md` sections 6, 7, 9.

## Scope and observed evidence

M004 already added independently versioned `bindings/python` packaging, `abi3-py311` PyO3 wheels, a 5-native-platform x 4-CPython-version consumer matrix, a clean extracted-sdist rebuild, wheel hashing, and dependency auditing. On baseline `1434c1e55ddd92947042cff9918ffc0fe130ed01`, GitHub Actions `Python native qualification` run `38522548258` (2026-10-14) shows 27 successful jobs: five wheel builds, 20 interpreter/platform smoke jobs, one extracted-sdist rebuild, and one dependency audit. Root `CI` `38522548209`, `Release drift guard` `38522548221`, and `Egress feature qualification` `38522548365` also passed on this SHA.

**Do not call M004 closed based on these baseline runs.** `plans/closure/python-native-library/004-status.md` is absent; the candidate will change when M005 adds lifecycle tests/config API. GitHub Actions artifacts expire, and green workflow conclusion without package hashes, native consumer install evidence, ABI notes, package naming and closure status is not persistent qualification. The M004 contract deliberately excluded publishing to PyPI.

This plan supplies a bounded **evidence + final qualification** pass rather than recreating the wheel pipeline or expanding supported architectures.

## Non-goals and invariants

No automated publication, PyPI secret/provisioning change, first PyPI release, TestPyPI publishing, changes to existing immutable GitHub release, Eggpack producer regeneration, binary asset matrix change, second Rust SKU, Python provider rewrite, TypeScript/C ABI, PyPy/free-threaded CPython/ARMv7/Windows ARM64 qualification without evidence. Keep root Rust 1.89, root `#![deny(unsafe_code)]`, the ten MCP/CLI contracts, Mode-Off no-remote-dispatch and transport/sanitization policies unchanged.

M004 closure means artifacts are qualified for a **separately authorized release**, not that `pip install eggsearch` resolves to these unpublished bits on PyPI.

## Ordered work packages

### WP1 — Establish post-corrective immutable candidate

Start only after `plans/closure/python-native-library/005-status.md` is accepted. Determine exact root/bindings Cargo versions, `bindings/python/pyproject.toml` metadata and tag policy, pinned PyO3/maturin versions, `abi3-py311` wheel tags, Cargo lock graph and Git SHA. Confirm supported target list with no extrapolation to unsupported CPython free-threaded builds. Ensure any new commit after qualification **invalidates** existing proof; schedule a new exact-SHA qualifier and do not reuse run `38522548258` as evidence for changed code.

### WP2 — Require complete native qualification and traceable artifacts

Execute the configured GitHub `Python native qualification` workflow against the exact candidate with required successful jobs: five native build targets (Linux x86_64/aarch64 with verified manylinux2014 policy, macOS x86_64/arm64, Windows x86_64), and Python 3.11, 3.12, 3.13, 3.14 installed-wheel smoke **on each of the five targets**, independent of Rust being present in consumer virtual environments. Tests must exercise import, canonical inventory, all ten operations in no-network fixtures, concurrency/cancellation/close and trust/Mode-Off behavior. Record wheel/platform tag, ABI, package metadata, source SHA, hosted run/job IDs, filename, SHA-256, and GitHub upload artifact digest. Do not assert that cross-compilation alone constitutes native qualification.

Check that Python 3.14 stable-ABI wheels work and that current supported Python matrix is accurate; no PyPy/free-threaded claim. Linux aarch64 runner execution is not proof for ARMv7. Any missing target stays explicitly unsupported or **blocks** M004 if advertised.

### WP3 — Rebuild independent sdist and audit supply chain

Build `maturin build --release --sdist --locked`, extract into a clean directory outside checkout, ensure it carries the binding crate + root Rust path dependency + lockfiles/license/readme, and rebuild **offline with prefetched locked Cargo crates**, then clean-install and run Python consumer suite. Validate no private paths/secrets/credentials/tracked build products in distributions. Run Cargo dependency policy, Python package dependency audit, action SHA pin checks and project metadata consistency. Verify `eggsearch` distribution name availability/ownership immediately before publication rather than making a permanent claim from an October 14 PyPI 404.

Capture durable SHA-256 digests (not only ephemeral 14-day artifact links), package wheel tags and `abi3` decisions. If clean sdist extraction requires source checkout or network fetch despite prefetched Cargo crates, record as a blocker.

### WP4 — Root compatibility and release boundary

Re-run `make check`, `make release-check` where environment permits, `cargo publish --dry-run --locked` and `cargo package --list` for root to prove that the independent Python binding has not changed the root published Cargo package or Eggpack release inputs. Confirm there is no Python wheel in the canonical seven-target binary assets, and that existing release workflow remains generated/owned by Eggpack. Run no-network Mode-Off SDK/Python agent fixture and source-trust tests on the final candidate. Fail closed on unknown status/partial failures. No publication or immutable release edits.

### WP5 — Close M004 and reconcile planning truth

Create `plans/closure/python-native-library/004-status.md` on the final candidate containing a requirement-to-evidence matrix, exact SHA, completed platform/interpreter/job table, five wheel filenames/tags/hashes and sdist hash, extracted-sdist clean rebuild and import proof, Python consumer fixture run, provenance/action/dependency evidence, binary/root compatibility gates, publication status (unpublished), unsupported targets, observed limitations, and severity-classified residual findings. Write `plans/closure/python-native-library/006-status.md` as a small final reconciliation cross-referencing the real M004 and M005 records, not duplicating their test tables.

Update `plans/subsystems/python-native-library-roadmap.md`, `plans/registry.md`, `docs/python-library.md`, package README, release guidance and any test inventory/architecture docs that actually changed. Close M004 and M006 **only together with the required exact-candidate evidence and both closure records in the same commit**; do not claim the workstream is published or prematurely archive still-actionable documents. If a high/medium gap appears, leave M004 active/blocked, record the deficiency, and issue another corrective rather than writing fictional green closure.

## Verification commands and exit

Canonical gates (record versions and actual invocation):
- `make check` and `make release-check` (report environment constraints instead of inventing a result)
- `cargo package --list`; `cargo publish --dry-run --locked`
- `rustup run 1.89.0 cargo check --locked --manifest-path bindings/python/Cargo.toml`
- `maturin build --manifest-path bindings/python/Cargo.toml --release --sdist --locked --out dist`
- `maturin build --manifest-path bindings/python/Cargo.toml --release --locked --out dist` for all five target builders
- `python -m pytest -q bindings/python/tests`; `pyright bindings/python/python/eggsearch bindings/python/examples`; `python -m compileall -q bindings/python/python/eggsearch bindings/python/examples`
- Hosted `.github/workflows/python-native-qualification.yml` on the **same exact candidate**, including wheel builds, 20 wheel-smoke jobs, sdist rebuild and dependency audit; main CI/release drift/egress qualification.

PASS means all M004 operational requirements have SHA-specific logs/attestations/digests, all claimed wheels install natively and pass Python tests in clean consumer venvs, sdist rebuilds outside checkout, root release compatibility remains proven, and closure records/registry are synchronized. Fail if any advertised wheel lacks native smoke, digests cannot be recovered, a candidate SHA differs, Rust/Python runtime regression exists, or documentation confuses qualification with PyPI availability.

## Handoff and closure

M006 is blocked until M005 closes. When M005 closes, set M006 ready and execute against the **post-M005** candidate. The final closure must explicitly retain M001–M003 historical evidence, mark M005 closed, mark M004 and M006 closed only after qualification, and state `Published to PyPI: no` unless independently authorized and verifiably accomplished. Separate package publishing can be planned later without reopening successful qualification.
