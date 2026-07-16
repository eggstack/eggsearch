# Eggsearch Production Retrieval Substrate Roadmap

Status: proposed execution roadmap
Audience: implementation agents and maintainers
Target: eggsearch `main`, beginning after v0.3.5
Primary consumer: codegg and other long-running coding-agent harnesses

## 1. Purpose

Eggsearch has reached the point where the primary challenge is no longer provider count or MCP surface breadth. The repository already exposes a coherent ten-tool search, fetch, repository, security, research, diagnostics, and evidence-handoff API. The remaining work is to make those tools reliable enough to serve as infrastructure for long-running coding-agent workflows.

This roadmap converts the current implementation from a strong release-quality beta into a production retrieval substrate with five defining properties:

1. Every advertised safety and resource bound is mechanically enforced.
2. Search and fetch behavior remains deterministic under malformed input, partial failures, cancellation, and provider drift.
3. Remote and local repository understanding is useful enough for architecture and implementation work, not merely URL discovery.
4. Agent-facing outputs communicate provenance, incompleteness, degradation, and recommended follow-up accurately.
5. The implementation remains lightweight, bounded, auditable, and deployable as a single stdio MCP process.

The roadmap deliberately preserves eggsearch's current architectural identity. It does not turn eggsearch into a general crawler, browser automation service, source-code database daemon, or model-based research agent. Optional richer backends may be integrated behind stable capability boundaries, but deterministic behavior and conservative defaults remain mandatory.

## 2. Current-State Assessment

The following areas are already strong and should be treated as constraints rather than rewritten casually:

- Stable ten-tool MCP surface.
- Deterministic content-derived identities.
- Explicit `external_untrusted` and `local_trusted` trust semantics.
- Structured warning and next-action contracts.
- Bounded explicit fetching rather than crawling.
- Redirect revalidation and DNS answer pinning.
- Search profiles for coding, security, research, and generic use.
- Provider health and cooldown tracking.
- Evidence bundles designed for multi-agent handoff.
- Extensive offline fixture, schema, documentation, and workflow tests.

The principal gaps are:

- Several remaining correctness defects in fetch bounds, local traversal policy, and deadline accounting.
- Incomplete adversarial and property-level testing around execution boundaries.
- Remote `repo_map` capability that is largely metadata-only without a local checkout.
- Local repository search that performs bounded filesystem scanning with regex enrichment rather than indexed repository intelligence.
- Agent workflows whose contracts are strong but whose retrieval quality and incompleteness reporting can be improved.
- Provider reliability that depends on manual live smoke testing despite unstable upstream HTML.
- Large orchestration files that increase review and regression risk.
- Limited long-running operational telemetry and release supply-chain gates.

## 3. Guiding Invariants

Every phase must preserve the following invariants.

### 3.1 Safety

- Remote content is always instruction-untrusted.
- Local content is provenance-trusted only, never instruction-trusted.
- Fetch remains explicit and single-target unless the caller invokes bounded batch fetch.
- Private, loopback, link-local, reserved, multicast, and documentation address behavior must be controlled by explicit policy.
- Redirects must never bypass initial target validation.
- Byte, character, item, file, depth, concurrency, and timeout caps must be hard limits, not advisory values.
- Operator escape hatches must be narrow, independently testable, and accurately documented.

### 3.2 Determinism

- Equivalent inputs produce stable identities and stable ordering.
- Task completion order must not affect final result order.
- Partial completion and degradation must be represented explicitly.
- Cached data must expose freshness and invalidation semantics.

### 3.3 Lightweight deployment

- Default operation remains a single Rust binary over MCP stdio.
- No mandatory database or resident indexing service is introduced.
- Optional syntax or code-intelligence integrations degrade cleanly to native fallbacks.
- Low-power configurations remain supported.

### 3.4 Contract compatibility

- Existing stable fields and warning codes remain compatible unless a documented semver migration is required.
- New fields should be additive and optional where possible.
- Human-readable warnings are not used as the programmatic contract.
- Capability reporting must describe effective runtime behavior, not aspirational features.

## 4. Execution Sequence

The roadmap is divided into nine phases. The first five have accompanying detailed execution plans.

### Phase 1: Correctness and Security Closure

Objective: remove known invariant violations before building new capabilities.

Primary outcomes:

- Hard byte cap is enforced across prefetched and streamed chunks.
- `allow_localhost` and `allow_private_network` have independent, documented semantics.
- Hidden-file policy no longer controls skipped-directory policy.
- Symlink handling matches configuration semantics.
- Local file traversal stops globally at the configured bound.
- Deadline telemetry distinguishes complete, partial, skipped, and interrupted work.
- CI and local release gates run the complete documented contract suite.

Exit gate: no known correctness defect from the current review remains open, and each defect has a regression test that fails against the pre-fix behavior.

Detailed plan: `plans/phase-01-correctness-security-closure.md`

### Phase 2: Retrieval Engine Hardening

Objective: move from example-based regression tests to systematic boundary validation.

Primary outcomes:

- Property tests for bounds, ordering, canonicalization, and identities.
- Fuzz targets for URLs, redirects, HTML, sanitization, locators, and PDFs where supported.
- Deterministic adversarial corpora for malformed documents and protocol edge cases.
- Fault-injection coverage for timeout, cancellation, panic, partial body, and provider failure combinations.
- Resource-bound tests that assert memory-relevant input limits without relying on wall-clock timing alone.

Exit gate: core fetch, rendering, path, identity, and dispatch boundaries have automated adversarial coverage suitable for continuous execution.

Detailed plan: `plans/phase-02-retrieval-engine-hardening.md`

### Phase 3: Remote Repository Intelligence

Objective: make `repo_map` a useful remote repository structure tool rather than a primarily local capability.

Primary outcomes:

- Native bounded tree adapters for GitHub, GitLab, Gitea, Forgejo, and Codeberg.
- Provider-neutral tree and repository metadata interfaces.
- Pagination, depth, entry, byte, and timeout enforcement.
- Manifest, CI, security-policy, documentation, source, test, generated, and vendor classification.
- Honest fallback behavior for unsupported hosts or unavailable credentials.

Exit gate: a caller can map a public remote repository on each supported host without cloning it, with deterministic ordering and explicit truncation metadata.

Detailed plan: `plans/phase-03-remote-repository-intelligence.md`

### Phase 4: Local Workspace Search Engine

Objective: replace repeated broad filesystem scans with a layered, cacheable, low-latency local retrieval architecture.

Primary outcomes:

- Repository inventory cache with deterministic invalidation.
- Fast path and content search using Git-aware or ripgrep-style execution where available.
- Native fallback with hard limits when external commands are unavailable.
- Optional syntax-aware symbol indexing using a bounded feature or adapter.
- Optional codegg/LSP/SCIP augmentation without coupling the core server to codegg.
- Accurate dirty-state, generated/vendor, and worktree metadata.

Exit gate: repeated local queries avoid full-tree rereads, preserve low-power operation, and provide materially better path, text, and symbol retrieval.

Detailed plan: `plans/phase-04-local-workspace-search-engine.md`

### Phase 5: Agent Workflow Optimization

Objective: improve the usefulness of eggsearch outputs for coding, security, and architectural research agents without adding model-dependent judgment.

Primary outcomes:

- Better intent-specific grouping and next actions.
- Stronger fetch candidate selection and provenance ranking.
- Explicit evidence coverage, partial-result, contradiction, and gap metadata.
- Repository-aware workflows for API comprehension, architecture review, migration analysis, debugging, and security review.
- Contract fixtures demonstrating correct codegg consumption.

Exit gate: codegg can use structured responses to choose productive next actions and determine whether evidence is sufficient without parsing prose warnings.

Detailed plan: `plans/phase-05-agent-workflow-optimization.md`

### Phase 6: Provider Reliability and Drift Detection

Objective: detect upstream breakage before users experience silent degradation.

Workstreams:

- Scheduled provider canaries.
- Recorded response fixtures for each provider.
- Captcha, consent, block-page, and empty-result detection.
- Provider success, latency, parse-failure, and duplicate-rate telemetry.
- Automatic temporary quarantine distinct from ordinary cooldown.
- Conservative adaptive weighting based on deterministic operational data.

Exit gate: provider parser or access regressions are visible through CI or scheduled diagnostics with actionable failure classes.

### Phase 7: Architecture Decomposition

Objective: reduce review risk and make each workflow independently testable.

Workstreams:

- Split MCP tools into one module per tool plus shared validation.
- Split adapter responsibilities into routing, dispatch, aggregation, health, ranking, and response assembly.
- Introduce narrow internal service traits only where they improve testability.
- Preserve public API and avoid speculative abstraction.

Exit gate: orchestration hotspots are reduced to reviewable modules with no behavior change and complete contract parity.

### Phase 8: Observability and Performance

Objective: make long-running behavior measurable and predictable.

Workstreams:

- Structured tracing spans for routing, provider calls, aggregation, rendering, and indexing.
- Latency histograms and counters exposed through diagnostics without requiring a network listener.
- Cache hit, invalidation, queue depth, cancellation, and truncation metrics.
- Benchmarks for search aggregation, HTML rendering, local indexing, repository maps, and serialization.
- Low-power performance profiles and regression thresholds.

Exit gate: maintainers can identify where time and resources are spent during representative workloads.

### Phase 9: Release and Ecosystem Closure

Objective: complete the operational and supply-chain surface required for a stable release series.

Workstreams:

- `cargo audit` or equivalent RustSec gate.
- `cargo deny` policy for licenses, sources, duplicate versions, and bans.
- Semver and schema compatibility checks.
- Signed release artifacts, checksums, SBOM, and reproducible-build documentation.
- Packaged-binary MCP smoke test.
- Provider author guide, operator playbook, and troubleshooting decision trees.

Exit gate: release artifacts, contracts, dependencies, and operational guidance are independently verifiable.

## 5. Dependency Graph

The ordering is intentional.

- Phase 1 blocks all later phases because new behavior should not be built on known invariant violations.
- Phase 2 should precede major repository additions so new adapters inherit stronger boundary tooling.
- Phase 3 and Phase 4 may overlap after their shared repository model is agreed, but their storage and execution strategies should remain independent.
- Phase 5 depends on the richer evidence produced by Phases 3 and 4.
- Phase 6 can begin fixture capture during Phase 2, but automated provider policy changes should wait until core correctness is closed.
- Phase 7 should follow the first five phases unless a file must be split to make a phase safely implementable.
- Phase 8 should instrument stable workflows rather than temporary intermediate designs.
- Phase 9 is the release closure phase and should consume evidence from every earlier gate.

## 6. Release Milestones

### Milestone A: Correctness baseline

Contains Phase 1.

Release claim:

> All documented safety and resource bounds are enforced by regression tests.

### Milestone B: Adversarially hardened retrieval

Contains Phase 2.

Release claim:

> Core retrieval boundaries are continuously tested against malformed and adversarial input.

### Milestone C: Remote repository mapping

Contains Phase 3.

Release claim:

> Supported remote hosts provide bounded native repository structure discovery.

### Milestone D: Fast local repository retrieval

Contains Phase 4.

Release claim:

> Local repository search is incremental, low-latency, and repository-aware.

### Milestone E: Codegg workflow readiness

Contains Phase 5.

Release claim:

> Coding agents can select, validate, and hand off repository evidence through stable machine-readable contracts.

### Milestone F: Production operations

Contains Phases 6 through 9.

Release claim:

> Provider drift, runtime performance, dependency risk, and release artifacts are continuously verifiable.

## 7. Global Definition of Done

Each phase is complete only when all of the following are true:

- Behavior is implemented, not merely documented.
- New configuration is validated and appears in diagnostics.
- New warnings or telemetry have stable machine-readable forms.
- Unit, integration, contract, and negative tests cover the behavior.
- Existing feature matrices continue to pass.
- Documentation examples are covered by contract tests where feasible.
- Low-power and disabled-capability modes degrade honestly.
- No tool claims a capability that the runtime cannot execute.
- Commit history contains implementation-focused commits rather than a single opaque bulk change.

## 8. Non-Goals

This roadmap does not include:

- Browser automation or JavaScript execution.
- Arbitrary recursive web crawling.
- An embedded language model for ranking or summarization.
- Mandatory vector search.
- Mandatory persistent external databases.
- A network-accessible MCP transport.
- Full semantic-code-search parity with large hosted code indexes.
- Replacing codegg's LSP or build-system responsibilities.

## 9. Handoff Guidance

Implementation agents should execute one phase at a time and preserve the sequence of tests, implementation, documentation, and verification described in each detailed plan. When a phase reveals a contract incompatibility, the agent should stop expanding scope, document the incompatibility, and resolve it explicitly rather than silently changing response semantics.

The first implementation pass should begin with Phase 1. No new provider or repository feature should be merged ahead of the Phase 1 closure gate unless it is necessary to correct one of the identified defects.