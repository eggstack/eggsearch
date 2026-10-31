# M008 — Compact Browser-Render Reachability Corrective

Status: active
Primary class: corrective
Source roadmap: `plans/subsystems/standalone-mcp-local-inference-roadmap.md`
Corrects: M001 compact `web_fetch` schema, discovered during M003 small-model task qualification
Related plans: `plans/implementation/standalone-mcp-local-inference/001-opt-in-compact-tool-advertisement.md`; `plans/implementation/standalone-mcp-local-inference/003-lm-studio-consumer-qualification-and-docs.md`
Related closures: `plans/closure/standalone-mcp-local-inference/001-status.md`; current M003 results in `plans/closure/standalone-mcp-local-inference/003-status.md`

## Objective

Allow compact-profile models to request the existing optional browser-rendering behavior from `web_fetch`, without expanding unrelated arguments or weakening runtime policy.

## Discovery

M003's paired browser extraction task found that the full `web_fetch` schema advertises `render` (`http_only`, `auto`, `browser`) but the compact schema strips it. The runtime accepts the option, yet a compact model cannot select it from the advertised tool contract. In the observed task, compact omitted `render` and fetched over HTTP; explicit full-profile browser requests timed out or returned source HTTP errors. An example-domain browser request returned HTTP 200, so the path is available but the React task remains unqualified.

## Scope and invariants

- Add only the existing `render` parameter to compact `web_fetch` input schemas, preserving its accepted values.
- Keep all ten tool names, runtime validation, response contracts, and the compact schema size budget.
- Preserve fetch bounds, browser concurrency, SSRF protections, and default `http_only` behavior.
- Do not expose persistent browser profiles or relax browser, network, or authentication policy.

## Work packages

1. Preserve `render` in the compact `web_fetch` schema and add a contract test for its property and enum values.
2. Run focused schema tests, `make check`, and `make docs-check` on the exact candidate.
3. Repeat the browser extraction prompt with the same source/query for full and compact profiles; record actual input/output tokens, arguments, browser transport result, extraction fidelity, and failures. Keep host/API failures separate from model outcomes.
4. Update M003 evidence, this closure record, the roadmap, and registry. Keep M003 active until its broader task quality and repeatability gates pass.

## Verification

```bash
cargo fmt --check
cargo test --locked --all-features --test mcp_schema_slimming
make check
make docs-check
```

Native API qualification is opt-in operational evidence, not a network-dependent routine CI test.

## Acceptance

- Compact `web_fetch` advertises the three existing `render` values and the tool schema remains within its size limit.
- Regression coverage confirms the property and enum survive compact-schema rewriting.
- A paired API trial can expose and invoke `render="browser"` in compact mode; report content quality and API/browser failures without treating infrastructure errors as model outcomes.
- Exact candidate, local and hosted checks, API measurements, and restored MCP config hash are recorded in `plans/closure/standalone-mcp-local-inference/008-status.md`.
