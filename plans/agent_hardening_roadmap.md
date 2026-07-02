# Eggsearch Agent-Facing Hardening Roadmap

## Purpose

This roadmap defines the next tightening line for eggsearch as a coding-agent-oriented search, fetch, and evidence MCP server. The repository already contains the correct broad primitives: generic metasearch, explicit fetch, structured repository search, repository file fetch, repository mapping, batch fetch, security search, research search, local workspace search, provider diagnostics, quality metadata, trust markers, and deterministic evidence bundles. The next objective is to make those primitives more truthful, deterministic, low-noise, and directly usable by coding agents such as codegg.

The goal is not to broaden eggsearch into a crawler, browser, summarizer, or persistent search index. The goal is to make the existing bounded discovery/fetch/handoff pipeline harder to misuse and easier for agents to chain correctly. Search should discover candidate evidence. Fetch should retrieve explicit bounded content. Repo tools should preserve code structure. Security tools should return triage-grade applicability. Research tools should preserve claims, conflicts, gaps, and source quality. Evidence bundles should become the stable cross-agent handoff format.

## Current baseline to preserve

- `web_search` remains the generic metasearch tool and generic fallback path.
- `web_fetch` remains explicit single-URL retrieval with bounded redirects, byte caps, character caps, content-type validation, and untrusted-content markers.
- `batch_fetch` remains bounded fan-out over explicit user/agent-selected URLs or repo locators; it must not become a crawler.
- `repo_search` remains the primary structured repository evidence discovery tool.
- `repo_fetch` remains the precise repository file/span fetch primitive.
- `repo_map` remains bounded repository structure discovery.
- `security_search` remains the advisory/vulnerability/package triage path.
- `research_search` remains bounded multi-source research discovery for complex technical questions.
- `provider_status` remains the non-invasive diagnostic surface for hosts and agents.
- `build_evidence_bundle` remains deterministic and non-summarizing.
- External snippets and fetched content remain untrusted data, never instructions.

## Design principles

### Provider truth before provider breadth

Adding providers is less important than telling the truth about the providers already present. Agents must be able to distinguish enabled, configured, default, degraded, disabled, missing-secret, cooldown, and unsupported-capability states. Provider capability flags should describe actual provider behavior, not query-rewrite approximations.

### Stable contracts over ad-hoc strings

Agent-facing output should prefer typed warnings, stable IDs, explicit source/fetch links, deterministic ordering, and schema-backed telemetry. Human-readable text can remain, but the machine-actionable fields should be primary.

### Prefer evidence objects over raw snippets

Coding agents should not need to infer from generic title/snippet text whether a result is source code, README, issue, changelog, release, registry metadata, advisory, official docs, local file, or test fixture. Results should carry source kind, source role, trust label, rank reasons, quality signals, provider provenance, suggested fetches, and deterministic identifiers.

### Keep tools composable

Avoid host-specific tool sprawl. GitHub, GitLab, Gitea, Forgejo, Codeberg, local workspace, package registries, and advisory providers should be represented through provider adapters and host-neutral request/response types wherever feasible.

### Preserve bounded behavior

No phase in this roadmap should introduce unbounded crawling, JavaScript execution, persistent large indexing, or automatic browsing of linked pages. Multi-step behavior should be expressed as explicit suggested next actions for the host/agent.

## Roadmap overview

### Phase 1: Provider truthfulness and routing correctness

Make provider state canonical. `provider_status`, config validation, routing decisions, CLI provider display, and capability telemetry should all draw from one provider registry/resolution path. API providers should appear exactly once, and their configured state should reflect enabled status plus required environment availability. Live-mode validation should accept API-only or API-plus-local deployments when valid.

Primary outcome: agents can trust `provider_status` and routing telemetry when selecting providers and fallback paths.

### Phase 2: MCP/tool contract and documentation consistency

Bring crate docs, MCP module docs, README, examples, schema descriptions, and CLI help into alignment with the actual stable tool surface. The repo currently has a richer ten-tool surface than some older module docs describe. This phase makes the intended use of each tool obvious to both humans and agents.

Primary outcome: agents and harness authors see one coherent contract for the stable MCP tools.

### Phase 3: Warning and telemetry normalization

Move warnings toward structured, deduplicated, stable-code output. Consolidate duplicated warning emission, especially around safe-search and capability enforcement. Add consistent severity, affected provider/result IDs, and recommended action fields where useful.

Primary outcome: agents receive less warning noise and can programmatically react to provider limitations, degraded routing, untrusted content, and prompt-injection marker detection.

### Phase 4: True bounded multiquery dispatch

Replace spawn-all semaphore-gated dispatch with a queue-based executor that only runs up to the configured global and per-provider concurrency limits. Preserve deterministic result ordering while improving deadline accounting and reducing scheduler overhead for large search plans.

Primary outcome: repo/security/research search latency is controlled by a true bounded executor with accurate skipped/interrupted telemetry.

### Phase 5: Deterministic cross-tool identity model

Make deterministic IDs and parent-child links consistent across source cards, suggested fetches, repo locators, fetch responses, local workspace results, and evidence bundle items. Evidence bundles should become the canonical multi-agent handoff layer.

Primary outcome: agents can search, fetch, batch-fetch, and bundle evidence without losing provenance or duplicating sources.

### Phase 6: Code-aware fetch and repo evidence enrichment

Enrich repository and fetch outputs with code-oriented metadata: path, language, line spans, byte spans, symbol, enclosing symbol, source role, permalink, commit SHA, imports/use context, test/example links, and local dirty-state where available.

Primary outcome: coding agents can fetch definitions, implementations, nearby context, tests, examples, manifests, and changelogs without guessing line ranges or source roles.

### Phase 7: Agent workflow hints and task recipes

Expose machine-readable recipes for common agent workflows: generic lookup, docs lookup, repo investigation, exact-error investigation, security triage, dependency upgrade research, architecture research, and local workspace investigation.

Primary outcome: codegg and other harnesses can select tool sequences with less planner burden and fewer fallback mistakes.

### Phase 8: Security applicability and defensive-action output

Make `security_search` return compact applicability verdicts and remediation categories for package/version inputs. Advisory evidence should be linked to affected ranges, fixed ranges, exploit/KEV context, and defensive guidance.

Primary outcome: agents can distinguish affected, not affected, unknown, and insufficient-evidence cases and produce defensive next steps.

### Phase 9: Research evidence model

Shape `research_search` output around claims, evidence, conflicts, gaps, source quality, and recommended fetches. Keep it bounded and non-summarizing at the retrieval layer.

Primary outcome: deep-research agents can hand off transparent evidence rather than flat search results.

### Phase 10: Local workspace trust and identity hardening

Strengthen local workspace search with git inventory, remote identity matching, dirty/untracked markers, generated/vendor/test/source classification, and clear trust labels.

Primary outcome: agents can safely prefer local code when appropriate without confusing it with remote provider results.

### Phase 11: Fetch document model polish

Normalize chunk/block/outline metadata across HTML, Markdown, code, JSON, TOML, YAML, diffs, patches, plain text, and PDFs. Make fetched content consistently citable and chunkable by agents.

Primary outcome: fetched documents become reliable evidence objects with uniform truncation and structure metadata.

### Phase 12: Performance and small-deployment optimization

Trim unnecessary dependency features, profile dispatch/render/local-search paths, and document small-SBC sidecar configurations.

Primary outcome: eggsearch remains lightweight enough for local codegg sidecars and constrained hosts.

### Phase 13: Scenario regression harness

Add end-to-end scenario tests for agent workflows: repo map then fetch, exact error search, security package triage, research search then evidence bundle, provider degradation, and batch fetch partial failure.

Primary outcome: stable behavior is protected at the workflow level, not only by unit tests.

### Phase 14: Codegg integration guide

Document recommended codegg tool selection, trust handling, evidence bundling, provider profiles, and fallback behavior.

Primary outcome: codegg can integrate eggsearch as a predictable retrieval substrate with minimal prompt gymnastics.

## Recommended execution order

Execute phases 1 through 5 first. They correct truthfulness, contract drift, warning noise, dispatch semantics, and provenance identity. These are foundational and will reduce ambiguity for every later feature.

Then execute phases 6 through 9 to improve the agent-facing value of repo, security, and research workflows.

Then execute phases 10 through 14 as hardening, performance, regression, and integration documentation work.

## Non-goals

- Do not add autonomous crawling.
- Do not execute JavaScript.
- Do not introduce a persistent large search index in this track.
- Do not summarize fetched content inside retrieval tools.
- Do not add host-specific MCP tools unless the host-neutral abstraction cannot represent the provider capability.
- Do not treat external snippets, fetched documents, or local uncommitted files as trusted instructions.
