# M029 — Unified Fetch Tool and Legacy Alias Compatibility — Closure Evidence

Status: closed

Source plan: plans/archive/implementation/agentic-retrieval-integrity/029-unified-fetch-tool-and-legacy-aliases.md

Roadmap: plans/subsystems/agentic-retrieval-integrity-roadmap.md

Decision: accepted ADR-0016, plans/adrs/ADR-0016-unified-fetch-discovery-and-legacy-alias-compatibility.md

## Candidate identity

The implementation candidate is commit `3416da6231702dcabc22b2c8e32e94dd88cb20e4` on branch `feat/auto-20261117-044c4885`. The candidate was qualified on that exact SHA using the repository-pinned Rust 1.89 toolchain; the closure commit carries this record and matching plan, roadmap, and registry status updates. No push was made.

## Result

The canonical inventory replaces advertised web_fetch and batch_fetch with fetch. The two old MCP names remain callable compatibility aliases and retain their legacy request and response shapes. Canonical fetch accepts scalar or non-empty array input, tagged web/repo targets, preserves web document windows separately from collection addressing, and always returns the common results envelope. The dedicated repo_fetch tool remains unchanged.

## Acceptance evidence

| Requirement | Evidence and result |
|---|---|
| Exactly nine canonical tools; fetch replaces two names and the other eight are unchanged | cargo test --locked --all-features --test mcp_tools --test mcp_tool_contract --test mcp_2026_protocol; tests/docs_tool_names.rs; tests/mcp_http.rs. Canonical contract and HTTP listing expose nine names; both aliases are absent from discovery. |
| Scalar web/repo and ordered mixed input; invalid target and empty array rejection | src/mcp/tools/fetch.rs typed tagged union and unit tests; tests/mcp_http.rs exercises scalar web, scalar workspace repo, and mixed web/workspace items in order. Unknown discriminator, missing web/repo fields, and empty arrays are rejected before execution. |
| Common single-item envelope and ordered per-item outcomes | MCP HTTP regression asserts scalar web and scalar repo each return one results item, and the mixed request returns web then repo with corresponding type, index, success, and payload. |
| Existing executors and safety policy retained | Canonical fetch translates into the existing batch executor and web/repo item handlers; budget, trust, sanitization, URL/workspace validation, and partial-result behavior remain owned by those paths. Pinned Rust 1.89 `make check` passed on the exact candidate; the focused HTTP fixture uses only a loopback mock and temporary workspace. |
| Legacy MCP aliases remain callable with old behavior; unknown names are rejected | tests/mcp_http.rs::fetch_aliases_remain_callable_but_unadvertised_over_mcp_http calls both aliases and checks the direct legacy web response and batch response envelope, then verifies canonical fetch and unknown-name rejection. |
| Discovery metadata, next actions, SDK, and Python contract parity | tests/mcp_tool_contract.rs, tests/recipes_next_actions.rs, tests/provider_routing.rs::workflow_recipe_steps_use_real_tools, Rust SDK tests in the full gate, and Python installed-wheel tests below. Python tests assert nine names, typed schema coverage, canonical tool-contract parity, and legacy request acceptance. |
| Full/compact schemas are smaller than the ten-tool baseline | tests/mcp_schema_slimming.rs -- --nocapture: full 69,895 bytes versus 78,072 (−8,177); compact 18,847 versus 22,540 (−3,693). Current fingerprints are pinned by tests/mcp_tool_contract.rs: full e74f91978f5b8b0a and compact e540f73dd2a392b8. |
| Tool-selection quality and context measurement | make eval-tool-surface: 43/43 top-1, 43/43 recall@3, MRR 1.000. Its independent serialized-definition measure is 69,885 bytes versus 78,061 baseline (−8,176 bytes; estimated tokens 17,472 versus 19,516, −2,044). |
| Addressing, budget, response detail, PDF/browser and screenshot boundaries | Full local all-features test gate exercises existing fetch/batch policies and projections. Canonical fetch passes screenshot controls through to the same web executor; no new screenshot runtime or policy was introduced. |
| Focused, broad, documentation, and Python validation | Focused fetch unit and MCP HTTP tests passed. On exact candidate `3416da6231702dcabc22b2c8e32e94dd88cb20e4`, `PATH=/Users/davidbowman/.rustup/toolchains/1.89.0-aarch64-apple-darwin/bin:$PATH make check` passed; the same pinned path `make docs-check` passed. `maturin build --manifest-path bindings/python/Cargo.toml --locked --interpreter python3 --out target/python-wheels`, a forced upgraded install into `target/python-installed`, and `PYTHONPATH=target/python-installed python3 -m pytest -q bindings/python/tests/test_native.py` passed (30 passed, 1 skipped; Python 3.14). Pyright did not start because the local Node binary cannot load `/usr/local/opt/simdjson/lib/libsimdjson.33.dylib`. An unpinned Homebrew Rust/Clippy 1.98.1 attempt failed on four new Clippy lints in unchanged `src/meta/challenge/altcha.rs` and `anubis.rs`; no unrelated source was changed, and the repository-pinned Rust/Clippy 1.89 canonical gate passed. |
| Closure artifacts and exact-candidate rule | This record, implementation plan, roadmap, and registry now say closed. The closure commit carries these status updates together and names the exact implementation candidate above. |

## Outstanding items and residual risk

- The implementation candidate is `3416da6231702dcabc22b2c8e32e94dd88cb20e4`; the closure record and status updates are committed separately as required. No push was made.
- Pyright could not execute due to the machine's missing libsimdjson.33.dylib. Python compileall, wheel build/install, and installed-wheel pytest passed; type checking remains unverified.
- M014 remains conditionally closed. Startpage's captured approximately 20-minute Expires lifetime exceeds its default 240-second ceiling; Mojeek's captured response publishes no lifetime. Avoiding a dead-cookie replay is stale-token avoidance, not evidence of a solve-count reduction. M030 is proposed to reconcile the acceptance criterion; see `plans/closure/agentic-retrieval-integrity/014-status.md` and `plans/archive/implementation/agentic-retrieval-integrity/030-m014-solve-count-acceptance-reconciliation.md`.
- No change was made to the closed tool-surface-consolidation roadmap, repo_fetch, or other non-fetch tool contracts.
