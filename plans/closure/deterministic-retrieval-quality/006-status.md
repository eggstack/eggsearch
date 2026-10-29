# M006 Comparative Qualification and Closure — Status

Status: conditionally closed (comparative evidence complete; macOS canonical gate has a registered corrective)

Source plan: `plans/implementation/deterministic-retrieval-quality/006-comparative-qualification-and-closure.md`

Roadmap: `plans/subsystems/deterministic-retrieval-quality-roadmap.md`

Implementation candidate: `10b06ab76434983cd9942e81bf2090fc9d6e31cc`

Report: `docs/retrieval-quality-comparison.md`

## Requirement-to-evidence

| Requirement | Evidence | Outcome |
|---|---|---|
| Reconcile M001-M005 and prior decisions | M001-M005 closure records; M004 no-change ranking decision; M005 context and CodeGG handoff | Complete; no ranking or routing change justified |
| Compare default, PDF, browser and combined feature arms | Four exact-candidate macOS release builds, executable sizes, CLI timings and process RSS in the report | Complete for build/runtime footprint; no paired feature quality benchmark is claimed |
| Exercise real configured browser rendering | Three ignored JS-rendering smoke runs against a public practice page; each verified inserted DOM content through browser transport | Complete as runtime smoke; not a quality corpus or rights-cleared fixture |
| Observe bounded keyless live contribution and failures | Two trials, three keyless sources, three cases, max 18 provider-query attempts total, zero billable requests; per-source outcomes recorded | Complete as diagnostic observation; zero judged live relevance hits, so no superiority claim |
| Reconcile docs and installed compatibility | Python 3.15 matrix and release-version claims corrected in candidate; default CLI and PyPI 0.4.4 wheel smoke passed | Complete; no publication or API change |
| Run exact-candidate canonical and focused gates | Formatting, clippy, benchmark, docs, packaging, evaluator and focused suites passed; `make check` and all feature suites hit the same two macOS failures | Conditional; corrective M011 registered |

## Verification

Passed on `10b06ab76434983cd9942e81bf2090fc9d6e31cc`:

- `cargo fmt --check`.
- `cargo clippy --locked --all-targets --all-features -- -D warnings`.
- `make bench-check`, `make docs-check`, `make packaging-check`.
- `make eval-retrieval-quality`: 8 passed, corpus fingerprint
  `fnv1a64:b882ce3d844e8ace`.
- `cargo test --locked --features 'mock pdf browser' --test batch_fetch_retrieval`: 13 passed.
- `cargo test --locked --no-default-features --test web_fetch_integration`: 70 passed.
- `cargo test --locked --features pdf --test web_fetch_integration`: 72 passed.
- `cargo test --locked --features browser --test browser_transport --test browser_profiles`: 98 passed.
- `cargo test --locked --all-features --test dispatch_fault_injection --test provider_routing --test provider_probe_conformance --test web_search_integration --test provider_request_contract`: 229 passed.
- `cargo test --locked --all-features --test tool_surface_evaluation --test mcp_projection --test mcp_schema_slimming --test mcp_tool_contract --test codegg_evidence_contract --test mcp_2026_protocol --test evidence_bundle_handoff --test mcp_tools`: 104 passed.
- `cargo test --locked --all-features --lib sdk::tests::mode_off_repo_search_through_sdk_keeps_remote_dispatch_at_zero`: 1 passed.
- Three runs of `cargo test --locked --features 'browser live-smoke' --test browser_live_smoke browser_live_smoke_javascript_rendered_fixture -- --ignored --nocapture`: passed.
- Default, PDF, browser and combined release builds; installed CLI and isolated
  Python wheel smoke passed without API keys.

`make check` failed in the library test phase. Direct feature-matrix runs
`cargo test --locked --no-default-features`,
`cargo test --locked --features pdf`,
`cargo test --locked --features browser`,
`cargo test --locked --features 'pdf browser'`, and
`cargo test --locked --all-features` each produced the same two macOS failures.
The passing test counts were 3,348, 3,405, 3,426, 3,483 and 3,484 respectively:

1. `mcp::state::tests::host_scopes_authenticate_and_cache_distinct_roots`:
   the test compares the macOS `/private/var/...` path with its `/var/...`
   alias.
2. `meta::repo_mapper::tests::structure_file_reader_rejects_symlink_escape_even_when_following_is_enabled`:
   an outside-root symlink was followed on macOS when following was enabled.
   This is a **high residual** because it violates the expected workspace
   containment boundary. It requires a corrective implementation and
   regression test before the macOS gate is green.

The corrective is registered in
`plans/implementation/repository-hardening/011-macos-symlink-containment-and-test-portability.md`.
It owns the safety repair and portable path assertion; M006 does not silently
absorb that implementation scope.

## Compatibility and residuals

No MCP or CLI contract, provider route, ranking, default feature, release
artifact, or Python availability promise changed. No release was published.
There were no billable provider calls. Live relevance evidence is inadequate
for quality comparison, and the browser page is diagnostic only. Resource
measurements are specific to the macOS x86_64 host; test-command RSS is not
isolated Chrome memory. These are low residuals for interpretation, not
release blockers for this no-change outcome.

## Disposition and handoff

Conditionally close M006 for its comparative evidence, report, exact-candidate
feature/browser qualification, compatibility checks, and no-change decision.
The required condition is successful completion of repository-hardening M011
and a green macOS canonical gate on the corrected candidate. M011 is the
separate corrective pass required by the planning process; M006 should be
updated to fully closed when that condition has evidence. No downstream
CodeGG implementation is transferred by this closure.
