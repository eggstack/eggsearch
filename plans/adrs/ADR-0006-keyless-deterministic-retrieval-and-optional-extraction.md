# ADR-0006: Keyless Metasearch and Deterministic Retrieval with Optional Extraction

Status: accepted
Date: 2026-10-29
Decision owners: project maintainers
Affected subsystems: search capability; retrieval quality and comparative evaluation; performance; tool-surface consolidation; optional browser/PDF.
Related: `plans/000-long-term-specification.md` ``1, 3, 5, 7, 8; `plans/001-terminology-and-domain-model.md`; `plans/002-long-term-roadmap.md`; `plans/adrs/ADR-0001-ten-tool-surface-and-adapter-ownership.md`; `plans/003-planning-process.md`.

## Context and forces

Eggsearch originated from a Rust rewrite of SearXNG-style metasearch. Its keyless HTML search engines are an intentional, first-class product capability, not a temporary fallback for paid APIs. Public-facing upstream HTML changes and rate limits are inherent maintenance costs of keyless metasearch; they are not evidence for abandoning it. Eggsearch additionally supports optional commercial/API sources and explicit bounded fetch.

The project now needs comparative search-quality and agent-context evidence before altering ranking, dispatch, or extraction. Existing deterministic tests prove many contracts but cannot prove live relevance. Optional `pdf` and `browser` features materially affect extraction utility and footprint and must be measured rather than assumed dispensable.

## Considered alternatives

1. Replace keyless HTML engines with authenticated APIs: rejected; loses the no-key baseline and the defining metasearch design.
2. Require an inference model/embedding service for ranking or summarization: rejected; non-deterministic, costlier, larger runtime, and changes the trust/ownership boundary.
3. Bake browser/PDF into the default release binary: not selected; existing feature/release policy forbids multiple SKUs and values the lean default.
4. Preserve all supported providers, optimize deterministic search and evidence, and evaluate feature variants independently: **selected**.

## Decision

- Keyless HTML upstreams (including the shipped default providers) remain first-class, independently selectable retrieval paths. Do not hide, degrade, replace, or make them conditional on an API credential as a purported reliability fix.
- Search planning, source fusion, ranking, selection, fetch, and evidence handoff remain **deterministic, bounded, and inference-free**. No required LLM, embeddings, vector database, generated answer, classifier model, or hosted reranking service in any execution path.
- Existing optional semantic search **providers** such as Exa are permitted as explicitly configured upstream search sources. Their remote internals do not give Eggsearch authority to generate answers or alter provenance; no semantic model is required to use Eggsearch.
- A provider may fail, throttle, or change HTML. Preserve explicit per-provider attempt/skip/failure evidence and tested graceful degradation. Do not implement CAPTCHA solving, anti-bot evasion, credential bypass, or unbounded retries to conceal an outage. Prefer compliant pacing and isolation of provider-specific breakage.
- `pdf` and `browser` remain valuable optional source-build capabilities and receive distinct and combined extraction evaluations alongside the default HTTP build. The Python wheel and published default binary remain unchanged unless separately authorized through existing distribution policy.
- Comparative experiments are **measurement-gated**. No optimization may silently change explicit-provider selection, keyless defaults, RRF/tie ordering without explicit accepted contract changes, retrieval status semantics, content trust, SSRF, CodeGG policy, or cache isolation.
- Benchmarking may use external reference providers as baselines only with operator-owned credentials and opt-in budgets. It never introduces a runtime credential requirement. Any downstream model-assisted task-completion evaluation belongs to the host harness, not Eggsearch.

## Consequences and tradeoffs

Advantages: preserves self-hostable/keyless utility, inspectable decision-making, reproducible regression gates, SBC-friendly core, and existing source trust. Costs: HTML adapter maintenance, imperfect result availability, manual/opt-in live evaluation to avoid CI network and billing, and feature-specific test builds with optional Chrome installed.

## Compatibility, security, and migration

No new MCP tools, breaking schemas, or source-layout migrations. Preserve the exact ten-tool contract, `eggfetch-core` transport ownership, bounded redirects and resolved-address pinning, prompt-injection markers, Mode-Off zero-remote-dispatch, and optional operator-configured local scope. Browser remains public-network-only, origin-scoped and manual-login-controlled; PDFs remain bounded and text-only with limitations surfaced. Preserve default compile and release assets and do not mandate feature-enabled builds on SBCs.

## Verification and status

The new `plans/subsystems/deterministic-retrieval-quality-roadmap.md` supplies measurable baselines, quality/effectiveness acceptance, optional extraction comparisons, and host-facing context-efficiency evidence. Existing search, performance, hardening, and tool-surface workstreams remain closed; this decision supplements them, not reopens them. No production behavior change is authorized solely by adopting this ADR.
