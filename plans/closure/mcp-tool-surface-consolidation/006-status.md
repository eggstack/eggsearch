# Tool-Surface Consolidation M006 — Agentic Evaluation Closure Status

Status: closed

Source implementation plan:

- `plans/implementation/mcp-tool-surface-consolidation/006-agentic-tool-surface-evaluation.md`

Source subsystem roadmap:

- `plans/subsystems/tool-surface-consolidation-roadmap.md`

Applicable ADR:

- `plans/adrs/ADR-0001-ten-tool-surface-and-adapter-ownership.md`

Repository baseline reviewed: `08f72ab0` (dependency evidence M010 closure, main HEAD before M006 touch-up).

Hard/operational dependencies: M001-M005 implementation landed
(`4b7e725`, `b1e6ea7`, `c102ac6`, `a905712`, `5a99bc2`); M006 is
operational on that baseline per the roadmap dependency graph. M001-M005
closure records remain pending and are not required before evaluation;
evaluation runs against post-consolidation bytes, never pre-consolidation
bytes presented as consolidation evidence.

Implementation commits:

- `b6f62cb830f51c2a05e0d6ccce4a07cc3cd0a0e6` — deterministic tool-surface evaluation corpus and regression gate
- `6c095c8b35dc85200d2dd543d7ae458a23f5f530` — merge 001-005 surface work into 006 baseline (73037 definition bytes, 1614 instruction bytes, 43/43 top-1)
- `e177907503e9fc14c64c87bd07a3154d7a5e92ab` — correct evaluation README gates and fingerprint version (300/80000/2000, 0.3.9)

Closure candidate: `fbf8457a21dc6d586ba5f903903278b3dfc8ce24` (adds M007
ratchet ceilings for two unrelated meta modules; no tool-definition,
contract, or corpus delta, so M006 gates are exercised unchanged on this
candidate).

## 1. Executive finding

M006 is complete. The deterministic Layer 1/Layer 2 harness runs in
routine CI without network or model access; the opt-in Layer 3 live
comparison stays out of routine CI. Post-consolidation measurements are
recorded with a content-derived fingerprint, and future tool or schema
growth that regresses context budget or discovery accuracy fails CI.
All ten capabilities remain discoverable after condensation.

## 2. Requirement-to-evidence matrix

| Requirement (plan acceptance) | Evidence | Result |
|---|---|---|
| Labeled corpus covers generic web, repo, fetch, security, research, evidence, diagnostics | `tests/fixtures/tool_surface/cases.json` (43 cases) + `README.md` category table | pass |
| Ambiguity fixtures test semantic discrimination | 7 `ambiguity` cases with `why` plus documented acceptable alternatives | pass |
| Deterministic discovery tests run in CI without network/model | `tests/tool_surface_evaluation.rs` (4 tests, no network, no model) in `make check` | pass |
| Baseline and post-change schema/context byte metrics available | `README.md` pre (77952/5275/273) vs post (73037/1614/260) + `tool_surface_live.rs` baselines | pass |
| Tool-search ranking quality measured, not assumed | `tool_surface_discovery_accuracy` (top-1/recall@3/MRR + per-category) | pass |
| Optional live evaluation across model families, out of routine CI | `tests/tool_surface_live.rs` (`#[ignore]`d comparison gated by `EGGSEARCH_EVAL_MODEL`) | pass |
| Future additions regress context or accuracy loudly | Byte budgets (300/80000/2000/512) + accuracy floors (0.90/0.95/0.90) + fingerprint determinism | pass |
| All capabilities discoverable after condensation | Corpus covers every tool as `expected_primary`; runner asserts coverage | pass |
| Reporting carries commit, fingerprint, config, model, aggregates, failures, deltas | `tool_surface_live.rs` report contract (9 keys) + `--nocapture` per-category output | pass |
| No live transcripts with secrets committed | Hygiene guard + plan non-goal; only metric reports kept | pass |

## 3. Before/after context measurements (exact candidate)

Deterministic run on the closure candidate (`--nocapture`):

- total definition bytes: 73037 (~18260 estimated tokens, bytes/4 ceiling)
- longest description: `repo_search` 186 chars (contract cap 300)
- server instructions: 1614 bytes (cap 2000)
- compact top-3 discovery: at most 260 bytes (cap 512)
- fingerprint: `eggsearch-0.3.9|tools=batch_fetch:150,build_evidence_bundle:144,provider_status:148,repo_fetch:150,repo_map:146,repo_search:186,research_search:160,security_search:159,web_fetch:159,web_search:160|bytes=73037`
- deterministic discovery: 43/43 top-1 (1.000), recall@3 43/43 (1.000), MRR 1.000

Per-category top-1: ambiguity 7/7, diagnostics 4/4, evidence 2/2,
exact_error 4/4, fetch 5/5, generic_web 5/5, repo 6/6, research 5/5,
security 5/5.

Pre-consolidation baseline (v0.3.8, before Plans 001-005, recorded in
`tests/fixtures/tool_surface/README.md`): 77952 definition bytes
(~19488 tokens), longest description `repo_fetch` 825 chars, server
instructions 5275 bytes, compact top-3 at most 273 bytes. Description
and instruction cuts account for nearly all improvement; total bytes
moved less because structured output schemas (M003) are model-visible
by design. Context reduction is measured end to end; bytes moving from
definitions into discovery output do not count as success (Layer 2
asserts compact 769 bytes stays below full 73037 bytes).

## 4. Production implementation evidence

- `tests/fixtures/tool_surface/cases.json`: 43 labeled fixtures with
  `id`, `category`, `query`, `expected_primary`, `acceptable` (always
  includes primary), `expected_followups`, `forbidden_primary`, `why`.
- `tests/tool_surface_evaluation.rs`: corpus well-formedness (unique
  IDs, known-tool references, acceptable-includes-primary, why
  non-empty, every tool as expected primary), byte budgets, discovery
  accuracy with forbidden-primary exclusion, Layer 2 synthetic
  mechanics (next-action targets within ten tools, priority 1-5,
  unknown-target sanitization, compact-below-full).
- `tests/tool_surface_live.rs`: Layer 3 report contract in CI (9 keys,
  zero-delta offline baseline) plus one `#[ignore]`d manual comparison
  driven by `EGGSEARCH_EVAL_MODEL`/`EGGSEARCH_EVAL_SURFACE`.
- `Makefile` target `eval-tool-surface` runs the deterministic suite
  with `--nocapture` for per-category output.
- No production tool, adapter, or contract code changed by M006 beyond
  the README gate correction; the harness scores live tool definitions
  via the canonical registry (`MAX_TOOL_DESCRIPTION_LEN`, contract
  purposes, `is_known_tool`, `sanitize_next_actions`).

## 5. Verification executed

Against candidate `fbf8457`:

```bash
cargo fmt --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features --test tool_surface_evaluation -- --nocapture
cargo test --locked --all-features --test tool_surface_live
cargo test --locked --all-features --test tool_surface_live -- --ignored
cargo test --locked --all-features --test static_guards
cargo test --locked --all-features --test mcp_tool_contract --test mcp_schema_slimming --test mcp_2026_protocol --test mcp_projection
cargo test --locked --all-features
make bench-check
make docs-check
make eval-tool-surface
./packaging/check-repo-hygiene.sh
./packaging/check-contract.sh
```

Outcomes:

- `cargo fmt --check`: pass
- `cargo clippy --locked --all-targets --all-features -- -D warnings`: pass
- `tool_surface_evaluation`: 4 passed, 0 failed (corpus well-formed, byte budgets, 43/43 accuracy, Layer 2 mechanics)
- `tool_surface_live` (CI): 1 passed, 1 ignored (report contract; comparison opt-in)
- `tool_surface_live -- --ignored` without model: prints skip notice, passes contract assertion
- `static_guards`: 44 passed, 0 failed
- `mcp_tool_contract` 14, `mcp_schema_slimming` 16, `mcp_2026_protocol` 13, `mcp_projection` 14: all pass
- `cargo test --locked --all-features`: 81 suites ok, 0 failed (5296 tests per `docs/test-inventory.md`, plus 4 doctests)
- `make bench-check`: pass (Criterion compile gate)
- `make docs-check`: pass (`RUSTDOCFLAGS="-D warnings" cargo doc`)
- `make eval-tool-surface`: pass (same 4 tests with per-category output)
- hygiene (`check-repo-hygiene.sh`): ok
- packaging (`check-contract.sh`): ok

Live-model quality thresholds remain opt-in/manual per the plan; no
live-model pass is claimed as closure evidence.

## 6. Invariant review

Ten stable tools with adapter ownership and canonical resolution
preserved; backend capability separated from disclosure; legacy fields
retained via compatibility path. No tool renamed, removed, or merged;
no mega-tool introduced. Disclosure hints remain advisory, never
policy-enforcing. Local workspace bounds, fetch safety, and
failure-vs-absence semantics untouched.

## 7. Failure and recovery review

Forbidden-primary cases assert diagnostics (`provider_status`) and
handoff (`build_evidence_bundle`) are never ranked first for ordinary
research. Unknown next-action targets are ignored, never hydrated;
reason-less actions are dropped. Malformed-argument and repair rates
are live-manual metrics; the deterministic gate pins the
preconditions (known-tool validity, priority bounds, hydration caps).

## 8. Migration and compatibility review

No wire change. The harness reads live `tools/list` definitions, so
any future description, schema, or instruction growth is measured
against the same budgets. Baseline fingerprint deltas are explicit in
the live report (`token_byte_deltas_from_baseline`).

## 9. Security review

No new I/O, network, execution, or authorization surface. Corpus
queries are static fixtures; the runner never fetches URLs or calls
models. Hygiene rejects tracked transcripts; live runs must not commit
secrets or uncontrolled fetched content.

## 10. Documentation and operations

- `tests/fixtures/tool_surface/README.md`: corrected gates
  (300/80000/2000/512) and 0.3.9 fingerprint; documents fixture shape,
  categories, metrics, baselines, and manual live command.
- `architecture/testing.md` (tool-surface evaluation section) and
  `docs/test-inventory.md` (2 suites, 43-case corpus, `make
  eval-tool-surface`) already describe the harness; unchanged by this
  closure.
- `Makefile` `eval-tool-surface` is the deterministic entry point;
  `tool_surface_live -- --ignored` with `EGGSEARCH_EVAL_MODEL` is the
  manual multi-model entry point.

## 11. Residual findings

| Severity | Finding |
|---|---|
| Low, accepted | Deterministic scorer is BM25/keyword approximation, not a model; it guards regression, not absolute agent behavior. Live-model first-tool and completion rates remain manual. |
| Low, accepted | Token estimates are bytes/4 ceiling, not tokenizer counts; used for deltas, never as a sole quality signal. |
| None high/medium | No correctness, compatibility, or safety regression identified. |

## 12. Roadmap disposition

M006 moves to closed with this record as controlling evidence.
M001-M005 remain closing (implementation landed, closure records
pending) and are unaffected. M007 was ready before M006 closure and
remains ready; closing M006 does not block it.

## 13. Registry updates

Covered in the same closure commit: M006 marked closed with closure
record `plans/closure/mcp-tool-surface-consolidation/006-status.md`;
subsystem remains active until M007 closes and M001-M005 closure
records land.
