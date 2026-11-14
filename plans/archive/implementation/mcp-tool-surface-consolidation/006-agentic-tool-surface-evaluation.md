# Plan 006 — Agentic Tool-Surface Evaluation and Regression Gate

Status: implementation plan
Scope: eggsearch evaluation fixtures with cross-repo CodeGG execution support
Depends on: Plans 001–005

## Objective

Create an evaluation harness that measures whether agents select and use eggsearch tools correctly under the proposed condensed/progressive-disclosure surface. Existing correctness tests are strong, but they do not answer the key agent-facing question: given overlapping search/fetch/research/security capabilities, does a model reliably find the right tool with low context cost and few redundant calls?

This plan establishes repeatable, model-aware measurements and a regression gate for future tool additions or schema growth.

## Non-goals

- Do not make CI depend on paid model APIs.
- Do not promote one vendor/model's behavior to a universal semantic contract.
- Do not replace deterministic unit/contract/fuzz testing.
- Do not optimize benchmark scores by embedding task-specific answers into descriptions.

## Evaluation layers

### Layer 1 — Deterministic retrieval/discovery tests

No external model required. Test the catalog/discovery algorithm against a labeled query corpus.

Each fixture contains:

```yaml
query: "check whether tokio 1.x is affected by CVE-..."
expected_primary: security_search
acceptable: [security_search]
expected_followups: [repo_fetch, web_fetch, build_evidence_bundle]
forbidden_primary: [provider_status]
```

Measure:

- top-1 accuracy;
- recall@3;
- mean reciprocal rank;
- false-positive sibling tools;
- discovery result bytes.

### Layer 2 — Synthetic tool-choice provider tests

Use deterministic/mock providers to verify CodeGG's surface mechanics:

- core/deferred distinction;
- tool search -> hydration;
- next-action -> hydration;
- policy ceiling preservation;
- schema/result byte accounting.

### Layer 3 — Optional live model evaluations

Provide an opt-in runner that can exercise configured model providers. It must be excluded from default `make check` and routine CI.

Record provider/model/version and exact tool-surface fingerprint with each result.

## Required task corpus

Build a curated corpus spanning at least these categories.

### Generic web discovery

- current package release lookup;
- documentation lookup;
- news/freshness query;
- domain-restricted discovery;
- generic security news where `web_search(intent=security)` is sufficient.

Expected primary: `web_search`.

### Repository investigation

- find where a function/type is defined;
- understand repository architecture;
- find implementation and tests for a feature;
- investigate a known GitHub issue/release change;
- inspect a package migration within a repository.

Expected primary: `repo_search`, with `repo_fetch`/`repo_map` as follow-ups.

### Known-target fetching

- explicit URL already supplied;
- explicit repository file/path supplied;
- known file and symbol/range supplied;
- several known URLs/files supplied.

Expected primary: `web_fetch`, `repo_fetch`, or `batch_fetch`, not a search tool.

### Exact-error investigation

- Rust compiler error;
- Python exception;
- TypeScript compiler error;
- toolchain/build error with an exact quoted message.

Expected primary: canonical repo/debug search path, testing legacy exact-error translation where applicable.

### Security

- CVE lookup;
- GHSA lookup;
- package/version applicability;
- lockfile dependency assessment;
- generic secure-coding information that should not invoke package applicability machinery.

Expected primary: `security_search` for structured advisory/application cases, generic web/repo search where appropriate.

### Research

- library comparison;
- architecture decision;
- ecosystem survey;
- performance investigation requiring multiple evidence classes;
- migration planning across several sources.

Expected primary: `research_search` when multi-source workflow semantics are useful, not for simple lookups.

### Evidence handoff

- package already selected search/fetch responses for a subagent;
- explicitly request a deterministic evidence bundle without new retrieval.

Expected primary: `build_evidence_bundle` only after evidence exists.

### Diagnostics

- provider outage/configuration troubleshooting;
- determine why an explicitly selected provider is unavailable.

Expected primary: `provider_status`.

Include negative fixtures where `provider_status` must not be selected for ordinary research.

## Ambiguity fixtures

Explicitly test prompts that currently admit several plausible tools:

- "research security changes in crate X";
- "find CVE details for X";
- "compare versions X and Y";
- "investigate performance regression in repo X";
- "find release notes for repo X";
- "fetch the sources you found";
- "what changed in this repository?"

For each fixture, document why one primary tool is preferred and which alternatives are acceptable. The goal is to test semantic discrimination rather than enforce arbitrary single-tool answers.

## Metrics

Every live/synthetic run should capture, where available:

- initial tool-definition bytes and estimated tokens;
- hydrated tool-definition bytes/tokens;
- tool-search output bytes/tokens;
- tool-result bytes/tokens;
- number of tools initially advertised;
- number hydrated;
- primary-tool correctness;
- tool discovery calls;
- redundant/repeated calls;
- malformed argument attempts;
- repair success after a tool error;
- end-to-end task success;
- latency/time-to-first-useful-evidence;
- tool-surface fingerprint.

Do not reduce evaluation to token count alone. A smaller surface that materially harms completion is a regression.

## Baselines

Before landing schema/disclosure changes, record at least one baseline configuration approximating the current behavior:

- current full/immediate CodeGG search palette;
- current `tool_search` full-schema result behavior;
- current eggsearch MCP descriptions/schemas.

Then compare:

1. compact descriptions only;
2. compact discovery without full schemas;
3. top-k hydration;
4. next-action guided hydration;
5. response projection.

This decomposition helps identify which change produces the benefit or regression.

## Model diversity

When live evaluation is available, test more than one model family and at least one weaker/fast model in addition to a frontier reasoning model. Tool naming and schema complexity can have model-specific effects.

Do not hardcode provider credentials into fixtures. Use the existing environment/config mechanisms.

## Repository layout

Suggested eggsearch-owned artifacts:

```text
benchmarks/tool_surface/
  cases/*.yaml
  schema_metrics.rs
  README.md
```

If adding a new top-level directory conflicts with repository policy, place fixtures under `tests/fixtures/tool_surface/` and the runner under `benches/` or `tests/`.

Suggested CodeGG-side optional runner:

```text
tests/tool_surface_agentic.rs
scripts/eval_tool_surface.*
```

Follow each repository's existing test organization rather than introducing a parallel framework unnecessarily.

## Regression gates

Add deterministic gates to normal CI for:

- tool description maximum size;
- total advertised eggsearch definition bytes;
- compact discovery result maximum size;
- labeled BM25/keyword retrieval top-k accuracy;
- known-tool/next-action reference validity.

Keep live-model quality thresholds opt-in/manual until reproducibility and provider stability are sufficient.

## Reporting

Produce a machine-readable summary plus a concise Markdown report with:

- commit SHA;
- tool-contract fingerprint;
- configuration;
- model/provider identity when live;
- aggregate metrics;
- per-category failures;
- token/byte deltas from baseline.

Do not commit live transcripts containing secrets or uncontrolled fetched content.

## Acceptance criteria

- A labeled tool-selection corpus covers generic web, repo, fetch, security, research, evidence, and diagnostics workflows.
- Deterministic discovery tests run in CI without network/model access.
- Baseline and post-change schema/context byte metrics are available.
- Tool-search ranking quality is measured, not assumed.
- Optional live evaluation can compare multiple model families without entering routine CI.
- Future tool additions/schema expansions can be rejected when they regress context budget or discovery accuracy.
- The evaluation harness demonstrates that all capabilities remain discoverable after condensation.

## Verification

```bash
make check
make bench-check
# plus the new deterministic tool-surface evaluation target
```

The closure report must include before/after context-size measurements and at least deterministic discovery accuracy by task category.