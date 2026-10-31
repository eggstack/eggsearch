# M003 — Small Local-Model Tool-Use and Context Qualification

Status: active (M001/M002 dependencies closed; native API experiments establish a working harness, but paired small-model task coverage and quality disposition remain incomplete; see `plans/closure/standalone-mcp-local-inference/003-status.md`)
Primary class: capability
Planning baseline: eggstack/eggsearch main `a666517c39adf28f32fad04198c74cb70d935c5b` (rebase baseline to exact M002 candidate at start)
Roadmap: `plans/subsystems/standalone-mcp-local-inference-roadmap.md`
ADRs: ADR-0001 and ADR-0006. No CodeGG implementation work.

## Objective

Qualify whether small locally hosted models can select Eggsearch tools, provide valid arguments, and produce evidence-grounded answers while the opt-in `compact` tool schema and `brief` result mode reduce model context use. LM Studio is the available local inference host for these experiments, including MLX-backed models; it is a test harness, not the product whose UI behavior this milestone qualifies. Deliver reproducible paired trials comparing `full + diagnostic` with `compact + brief` on the same model, task, and evidence sources.

## Research/consumer constraints

The local inference host must expose actual model turns and usage counters through an API that can invoke the configured MCP server. LM Studio currently provides that path in this environment and supports MLX models. Host UI refresh, interactive chat presentation, and internal handling of `structuredContent` versus text fallback are outside this milestone; record host/version details only for reproducibility. Use MCP protocol tests for Eggsearch's own result contract.

Eggsearch default binary transport is client-owned `eggsearch mcp stdio`; optional HTTP is loopback-only via `eggsearch mcp serve`. Do not make local inference dependent on starting a separate cloud service, building on SBCs, or a proprietary UI plugin. Use the actual CLI syntax and executable-path quoting on each OS before publishing examples.

## Ordered work packages

1. Keep `docs/lm-studio.md` as optional host setup guidance for an installed binary, but do not make LM Studio UI behavior a closure dependency. Record the selected `mcp.json` profile and rollback hash for reproducibility without retaining credentials.
2. Record the exact Eggsearch candidate, host/API version, model identifier and quantization, context length, prompt, profile, and task data. Confirm all ten tools remain listed in the child-process smoke. Capture API-reported input/output tokens and returned tool calls/results; use serialized schema/result bytes as supporting measurements, not token estimates.
3. Compare `full + diagnostic` with `compact + brief` using paired trials on the same small model, exact prompt, allowed tool set, URL/query, and source document where applicable. Include direct fetch, web search, `research_search`, `security_search`, and at least one failure/applicability case. Repeat key cases to expose tool-choice variability. Include a larger reference model only as context; the acceptance target is small local models.
4. Score each turn for required-tool selection, argument validity, repairs/retries, evidence completeness and authority, truthful treatment of retrieval failures/uncertainty, successful task completion, API usage, and latency where consistently available. Report model/task variation; do not infer a general quality win from one successful pair or claim savings from wire bytes alone.
5. Keep deterministic offline MCP/schema/response contract tests as the regression gate. No model download or network-dependent model test belongs in routine CI. Use existing protocol tests to verify result shape; no LM Studio UI refresh or generic inspector task is required.
6. Implement only minimal fixes required by reproducible qualification defects, with focused fixtures. Otherwise leave code unchanged and report the measured tradeoff. Update setup docs only when commands or operator guidance changed.

## Compatibility, security and resources

Full mode must continue working in ordinary MCP clients and CodeGG; no provider or transport regression, secret/proxy bypass, config-driven SSRF relaxation, startup service change, or new live agent backend in Eggsearch. Test prompt-injection markers and retrieval-failure truthfulness through the exact tool result returned to the model. No raw diagnostic JSON dumps in brief mode unless explicitly requested. Do not claim local-model benchmark wins without actual measured outcomes.

Test a fresh installation from precompiled release assets where available; no compilation on low-resource ARM boards. Offline smoke can run on normal hosted CI. A live small-model API trial is an operational acceptance dependency for the model-use claim; an unavailable UI is not a blocker if the API returns actual model turns, tool calls, tool results, and usage counters.

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

Check exact suite names against `architecture/testing.md` before running; keep live model API trials operational and opt-in, and avoid a permanent CI dependency on LM Studio or a model download.

## Acceptance / stop

- A validated local inference API path invokes a candidate installed Eggsearch executable and exposes all ten tools in compact presentation; the paired task set covers generic retrieval and specialist tools.
- Repeated small-model trials capture host-reported input/output tokens for full versus compact and diagnostic versus brief, plus bytes, tool-call outcomes, completed-task evidence, and latency where available; do not substitute byte/4 estimates. Report negative outcomes honestly.
- No capability, safety, inference-free or CodeGG regression; full profile and explicit diagnostic work. User documentation has one clear lightweight default, opt-in full fallback and detailed troubleshooting.
- If measured compact savings come at a substantial cost in tool correctness, repair, safety or specialist reachability, stop and register a corrective pass with concrete fixtures; do not silently publish a lower-quality default.
- Live small-model API qualification performed and recorded, or M003 remains explicitly unclosed.

Closure evidence in `plans/closure/standalone-mcp-local-inference/003-status.md`: implementation SHA, installed binary and API host versions, model identifiers/configurations, nonsecret fixtures, per-task paired comparisons, usage/results, test commands/outcomes, residual findings, docs links, and recommended release disposition. Do not claim a public release was made by finishing this plan.
