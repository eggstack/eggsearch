# M003 — LM Studio Direct-MCP Qualification and Consumer Documentation

Status: blocked (M001/M002 dependencies closed; live LM Studio consumer qualification remains; see `plans/closure/standalone-mcp-local-inference/003-status.md`)
Primary class: capability
Planning baseline: eggstack/eggsearch main `a666517c39adf28f32fad04198c74cb70d935c5b` (rebase baseline to exact M002 candidate at start)
Roadmap: `plans/subsystems/standalone-mcp-local-inference-roadmap.md`
ADRs: ADR-0001 and ADR-0006. No CodeGG implementation work.

## Objective

Prove the new opt-in compact schema and brief result modes actually improve local-model use in LM Studio while preserving the ability to call all ten Eggsearch tools, understand retrieval uncertainty and fetch needed evidence. Deliver a reproducible installed-binary LM Studio configuration and tested troubleshooting workflow, rather than relying on simulated schema sizes.

## Research/consumer constraints

LM Studio documents local/remote MCP servers in `mcp.json` and warns about context-heavy servers: https://lmstudio.ai/docs/app/mcp . As of the researched documentation, LM Studio API MCP integrations can select configured servers and restrict visible tools through `allowed_tools`, which can reduce prompt costs: https://lmstudio.ai/docs/developer/core/mcp . Verify feature/version behavior during implementation; do not assume `allowed_tools` in the API implies interactive chat dynamically fetches tool schemas, or that all clients use both `structuredContent` and JSON text.

Eggsearch default binary transport is client-owned `eggsearch mcp stdio`; optional HTTP is loopback-only via `eggsearch mcp serve`. Do not make local inference dependent on starting a separate cloud service, building on SBCs, or a proprietary UI plugin. Use the actual CLI syntax and executable-path quoting on each OS before publishing examples.

## Ordered work packages

1. Create `docs/integrations/lm-studio.md` (or a well-named existing operator doc section): copyable `mcp.json` JSON for an **installed binary** with the opt-in compact profile (pass `--config` to the actual CLI or use supported env/config controls). Provide standard full-profile fallback, Windows PowerShell executable path, macOS and Linux paths, stdio stdout-is-protocol / stderr-is-logs expectations, and one loopback-HTTP alternative if client support is confirmed. Never hardcode private paths or credentials; no remote/public bind.
2. Record an exact SHA, binary version, LM Studio version/build, local model and context length; enumerate MCP `tools/list` from the actual child process and record schema bytes, tokenizer counts, advertised names and prompt-use attribution. Compare `full + diagnostic`, `compact + brief` and optional client-side `allowed_tools` to avoid confusing profile savings with fewer enabled tools.
3. Build deterministic offline fixtures that replay realistic tool calls through the real MCP stdio server (loopback HTTP as additional protocol qualifier): web search, page/PDF fetch, repo search + specific fetch, research grouping, advisory/version applicability, grouped batch failures, and an evidence bundle. Mock upstream calls in CI; no credential or external network requirement. Confirm status-first brief and explicit diagnostic fallback.
4. Run optional interactive LM Studio qualification with at least one smaller tool-capable local model and one larger reference local model (model sizes are examples, not hard requirements). Use >= 8k context if the application permits, record exact tokenizer and actual observed prompt token usage. Compare tool-choice success, argument validity, repair/retry count, target evidence found, truthful failure interpretation, calls to answer, total latency and prompt-processing time on the same prompts. Include tasks requiring `security_search` and `research_search` to demonstrate capability preservation. Do not rely only on 43-case offline keyword ranking results.
5. Characterize how LM Studio actually handles `outputSchema`, `structuredContent` versus text fallback, `tools/list` refreshing and tool visibility across new chat/session after config changes. Record discrepancies and a verified rollback path. Compare at least one other generic MCP inspector/client if available; keep any vendor-specific advice in docs rather than adding vendor detection to Eggsearch.
6. Implement only minimal fixes necessary for discovered compatibility defects in the already-owned profile or projection modules; otherwise open new corrective milestones and keep the current M003 unclosed. Update `docs/integrations.md`/`docs/installation.md`/`README.md`, `docs/test-inventory.md`, `architecture/mcp.md` as appropriate. Explain optional client `allowed_tools` filtering as an intentional narrowing of per-chat discovery, NOT a server capability loss.

## Compatibility, security and resources

Full mode must continue working in ordinary MCP clients and CodeGG; no provider or transport regression, secret/proxy bypass, config-driven SSRF relaxation, startup service change, or new live agent backend in Eggsearch. Test prompt-injection markers and retrieval-failure truthfulness through the exact text visible to LM Studio. No raw diagnostic JSON dumps in brief mode unless explicitly requested. Do not claim local-model benchmark wins without actual measured outcomes.

Test a fresh installation from precompiled release assets where available; no compilation on low-resource ARM boards. Offline smoke can run on normal hosted CI. A live LM Studio instance is an operational acceptance dependency for closing the consumer claim; if unavailable, document an unexecuted gate and leave M003 blocked/closing accordingly, without misrepresenting offline results.

## Verification

```bash
cargo fmt --check
cargo test --locked --all-features --test mcp_2026_protocol
cargo test --locked --all-features --test mcp_tool_contract
cargo test --locked --all-features --test tool_surface_evaluation -- --nocapture
cargo test --locked --features mock --test mcp_tools
cargo test --locked --features mock --test web_search_integration
cargo test --locked --features mock --test web_fetch_integration
make check
make docs-check
make bench-check
```

Check exact suite names against `architecture/testing.md` before running; add a dedicated installed-binary/LM Studio compatibility test runner if the existing MCP protocol tests cannot exercise the targeted path. Avoid a permanent CI dependency on LM Studio or a model download.

## Acceptance / stop

- A validated `mcp.json` configuration from a published or candidate installed Eggsearch executable connects and the host sees all ten tool names in compact presentation; both generic web and specialist tasks run correctly.
- Capture **actual model-visible** schema/result token counts for full vs compact, plus bytes, tool-call failures, completed-task evidence and latency; do not substitute byte/4 estimates for tokenizer or hosted context metrics. Report negative outcomes honestly.
- No capability, safety, inference-free or CodeGG regression; full profile and explicit diagnostic work. User documentation has one clear lightweight default, opt-in full fallback and detailed troubleshooting.
- If measured compact savings come at a substantial cost in tool correctness, repair, safety or specialist reachability, stop and register a corrective pass with concrete fixtures; do not silently publish a lower-quality default.
- Live LM Studio smoke performed and recorded, or M003 remains explicitly unclosed.

Closure evidence in `plans/closure/standalone-mcp-local-inference/003-status.md`: implementation SHA, installed binary and client versions, configs and nonsecret fixtures, per-task comparisons, test commands/outcomes, residual findings, docs links, and recommended release disposition. Do not claim a public release was made by finishing this plan.
