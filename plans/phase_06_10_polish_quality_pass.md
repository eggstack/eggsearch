# Phase 6–10 Polish and Quality Pass

## Purpose

This plan tightens the phase 6–10 implementation after the first feature pass. The recent work substantially improved eggsearch as an agent-facing retrieval layer: code-aware evidence, workflow recipes, security remediation output, research evidence modeling, and local workspace identity/trust metadata are now present. The remaining work is quality-oriented rather than architectural.

This pass should reduce response bloat, improve research claim usefulness, harden local workspace fetch behavior, make security wording safer, finish code-span linkage semantics, and make verification more visible for future handoffs.

## Current state summary

Since the phase 6–10 plan files, the repo has implemented:

- Phase 6: code context extraction, expanded source roles, imports, complementary suggested fetches, evidence gaps, and docs polish.
- Phase 7: `AgentWorkflowRecipe`, built-in recipe catalog, `workflow_recipes` in provider status, and `next_actions` across search responses.
- Phase 8: richer security applicability, insufficient-evidence state, remediation actions, source quality/rank enums, security summaries, and security suggested-fetch reason codes.
- Phase 9: deterministic research claims, conflicts, source quality, evidence gaps, and evidence bundle preservation.
- Phase 10: local workspace identity, match confidence/reasons, file classification flags, local evidence gaps, and local trust docs.

The implementation is directionally strong. This plan focuses on the loose edges most likely to matter in codegg and other agent harnesses.

## Non-goals

- Do not add new major MCP tools.
- Do not introduce LLM summarization inside eggsearch.
- Do not expand into persistent indexing or background crawling.
- Do not execute project code or security payloads.
- Do not redesign the phase 6–10 data model unless a field is demonstrably misleading.
- Do not break existing response compatibility.

## Workstream 1: Response-size and verbosity control

### Problem

Phase 7 added `workflow_recipes` to `provider_status`, and phases 6–10 added richer metadata across search responses. This is useful but can create unnecessarily large responses for small tasks. Coding agents need a compact default path with optional detail expansion.

### Required behavior

Add a compact/full detail policy for large metadata fields:

- `provider_status` should default to either compact recipes or bounded full recipes.
- Expose an argument such as `include_recipes`, `recipe_detail`, or `detail` if the provider-status args already support extension.
- If adding args is too disruptive, keep full recipes but cap text-heavy fields and add a documented response-size budget.
- Search response `next_actions` should remain bounded at 5 or fewer items.
- Research claims/conflicts/gaps and source quality should have explicit caps documented in code and docs.
- Evidence bundle metadata should avoid duplicating large source/fetch content.

### Implementation guidance

Preferred provider-status policy:

```rust
pub enum RecipeDetail {
    None,
    Summary,
    Full,
}
```

Default should be `Summary` or whatever preserves current compatibility with acceptable size. Summary recipes should include `id`, `title`, `goal`, `support`, required/optional capabilities, and maybe step tool names, but omit verbose `suitable_when`, `avoid_when`, long trust notes, and detailed fallback text.

If MCP schema compatibility makes enum args difficult, implement `include_recipes: Option<bool>` first and document default behavior.

### Tests

- Provider status with recipes disabled omits or empties `workflow_recipes`.
- Provider status summary recipes are smaller than full recipes and retain recipe IDs/support.
- Provider status full recipes include step/fallback/trust-note detail.
- `next_actions` never exceeds 5 entries.
- Research claims/conflicts/gaps/source-quality lists respect caps.
- Snapshot-style tests validate large response fields do not accidentally become unbounded.

### Acceptance criteria

- Agents can request compact or full workflow guidance.
- Default responses are not bloated for routine capability checks.
- All rich metadata lists have hard caps and tests.

## Workstream 2: Research claim quality pass

### Problem

The phase 9 research evidence model is structurally useful, but deterministic claim text can be generic. A claim like `Evidence suggests Primary Sources supports the research topic` is not very useful to an agent. The claim model needs more specific, source-grounded wording and better evidence-gap linkage without pretending to synthesize final conclusions.

### Required behavior

Improve claim text generation so it is specific to group kind, workflow, query, and source class. Claims should remain conservative and evidence-linked.

Examples:

- Bad: `Evidence suggests Primary Sources supports the research topic`.
- Better: `Primary sources were found for the architecture decision, but fetched source bodies are still required before relying on details`.
- Better: `Benchmark-oriented sources exist, but reproducibility and version context still need verification`.
- Better: `Security-related sources were found; applicability requires package/version context or advisory fetches`.

### Implementation guidance

- Add a small deterministic phrase builder keyed by `ResearchResultGroupKind`, `ResearchWorkflow`, `ResearchDomain`, and detected source classes.
- Include source-quality notes like `official docs present`, `benchmark source present`, `only secondary sources`, or `counterpoint source present`.
- Link `missing_evidence` directly to gap kinds.
- Keep text short; avoid report-style prose.
- Do not claim a technical conclusion that was not directly represented by result grouping/source metadata.

### Tests

- Claim text for primary-source group is specific and mentions primary source presence.
- Benchmark group claim mentions benchmark/reproducibility context.
- Security group claim mentions applicability/version context.
- Counterpoint group claim links to conflict/counterpoint source IDs.
- Only-secondary-source scenario produces a low-confidence claim and gap.
- Claim text does not include unsupported conclusions like `X is faster than Y` unless a deterministic result group explicitly encodes that relation.

### Acceptance criteria

- Research claims are useful as retrieval-state descriptions.
- Claims remain conservative and evidence-linked.
- Generic placeholder wording is removed or restricted to true unknown fallback cases.

## Workstream 3: Code-span identity and linkage polish

### Problem

Phase 6 added `CodeContext` and richer source roles, but the plan originally called for stronger code-span evidence semantics. Current metadata may not consistently expose a single span object that links source ID, fetch ID, repo locator, file path, line range, role, language, and enclosing symbol.

### Required behavior

Add or normalize a compact code-span object on fetch/search/evidence-bundle paths where code is present. Do this additively to avoid breaking compatibility.

Recommended fields:

```rust
pub struct CodeSpanEvidence {
    pub stable_id: String,
    pub source_id: Option<String>,
    pub fetch_id: Option<String>,
    pub locator_id: Option<String>,
    pub path: String,
    pub language: Option<String>,
    pub source_role: SourceRole,
    pub line_start: Option<u32>,
    pub line_end: Option<u32>,
    pub enclosing_symbol: Option<String>,
    pub enclosing_symbol_kind: Option<String>,
    pub imports: Vec<String>,
    pub trust: TrustLevel,
    pub permalink_url: Option<String>,
    pub raw_permalink_url: Option<String>,
}
```

If an equivalent type already exists, reuse/extend it rather than adding a duplicate.

### Implementation guidance

- Use existing phase 5 identity functions or add `compute_code_span_id` with versioned input.
- Include the span object in `RepoFetchResponse` when code is fetched.
- Preserve it in `EvidenceBundleFetchedItem` when present.
- Link `SourceCard.metadata.code_evidence` to the same identity where feasible.
- Do not duplicate the full content body.

### Tests

- `repo_fetch` with line range returns a stable code-span ID.
- Same locator/path/ref/line range returns same span ID.
- Different line ranges produce different span IDs.
- Evidence bundle preserves code-span metadata from fetched items.
- Code span links to `source_id`/`fetch_id` when provided.
- Non-code fetches omit code-span metadata.

### Acceptance criteria

- Coding agents can cite and hand off exact code spans without reconstructing identity from several fields.
- Code-span metadata is optional and backward-compatible.
- Stable span IDs are tested.

## Workstream 4: Local workspace fetch hardening verification

### Problem

Phase 10 improved local workspace metadata, but the visible changes were more about identity/classification than path safety. The plan explicitly called for traversal, symlink escape, binary, and large-file verification. This pass should audit and harden that surface.

### Required behavior

Local fetch must be path-safe, bounded, and explicit about symlink behavior.

Requirements:

- Reject `../` traversal outside configured roots.
- Canonicalize paths before reading.
- Reject or safely handle symlinks that resolve outside allowed roots.
- Enforce max bytes/chars before returning content.
- Detect binary files or invalid UTF-8 and avoid returning junk text.
- Preserve line slicing and truncation metadata.
- Emit structured warnings for truncation, binary/unsupported content, and path rejection where appropriate.

### Implementation guidance

- Centralize local path validation in one helper.
- Make symlink policy explicit in code comments and docs.
- Add tests using temp directories where possible.
- Use platform-tolerant tests for symlink behavior; skip symlink-specific test if the platform does not permit symlink creation.

### Tests

- `../` traversal is rejected.
- Encoded or normalized traversal attempts are rejected if local pseudo-URLs can encode paths.
- Symlink escaping root is rejected or handled according to policy.
- Symlink inside root is accepted if policy permits it.
- Large file truncates with structured metadata/warning.
- Binary file is rejected or returns a clear unsupported-content warning.
- Line slicing remains correct after canonicalization.

### Acceptance criteria

- Local fetch safety is explicitly tested.
- Local workspace trust model remains evidence-trusted, never instruction-trusted.
- Path hardening does not break normal local workspace fetches.

## Workstream 5: Security remediation wording safety audit

### Problem

Phase 8 added remediation actions and source classification, including exploit-discussion source class. Remediation text must stay defensive and must not transform exploit-discussion snippets into procedural exploit guidance.

### Required behavior

Security output must remain defensive, concise, and evidence-linked.

Audit/remediate:

- `SecurityRemediation.description`
- `SecurityRemediation.rationale`
- `SecurityContext.defensive_guidance`
- security suggested-fetch reason text
- docs examples around exploit discussion

Rules:

- Do not include exploit steps, payloads, exploit code, or operational attack instructions.
- When exploit discussion is present, represent it as urgency/context only.
- If applicability is `unknown` or `insufficient_evidence`, default to `manual_review` or `monitor_only`, not `no_action_supported_by_evidence`.
- Do not recommend fixed versions unless advisory metadata supports them.
- Preserve `KEV absent is not proof` semantics.

### Tests

- Exploit-discussion source produces defensive urgency metadata but no procedural instructions.
- Unknown applicability produces manual-review remediation.
- Insufficient evidence produces manual-review remediation.
- Fixed-version remediation includes only advisory-supported versions.
- No-action remediation appears only when not-affected is conclusive.
- Remediation text avoids a small denylist of attack-oriented verbs/phrases in generated text.

### Acceptance criteria

- Security output remains safely defensive.
- Remediation categories are evidence-gated.
- Tests cover exploit-discussion and unknown/insufficient-evidence cases.

## Workstream 6: Suggested-fetch reason-code normalization

### Problem

Security reason codes were improved in a follow-up. Similar normalization should be checked across repo, research, and generic suggested fetches. Agents should not have to parse free-text `reason` fields.

### Required behavior

All suggested-fetch types should have stable, documented reason codes when possible:

- Repo suggested fetches: `exact_source_match`, `nearby_test_candidate`, `example_candidate`, `manifest_context`, `lockfile_context`, `changelog_context`, `migration_context`, `security_policy_context`, `repo_root_context`.
- Research suggested fetches: `primary_source`, `counterpoint_source`, `benchmark_source`, `security_source`, `migration_source`, `official_docs_source`, `source_needs_fetch`.
- Security suggested fetches: already mostly covered; verify no group maps to `None` unless genuinely unknown.
- Web suggested fetches / next actions: `inspect_top_source`, `fetch_official_source`, `bundle_evidence`, etc.

### Tests

- Every generated repo suggested fetch has a non-empty reason code except explicit fallback/unknown cases.
- Every research suggested fetch has a non-empty reason code except explicit fallback/unknown cases.
- Security reason-code coverage includes all current `SecurityResultGroupKind` values.
- Reason codes are snake_case and documented.

### Acceptance criteria

- Agent harnesses can route suggested fetches by stable reason code.
- Free-text `reason` remains human-readable but is not required for control flow.

## Workstream 7: Verification and CI visibility

### Problem

Commit messages report passing tests and clippy, but GitHub status checks/workflow runs were not visible in the connector. Future handoffs should have explicit verification commands and, if practical, CI status.

### Required behavior

At minimum:

- Add or update docs describing the local verification matrix.
- Ensure `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test --all-features`, and `cargo test --no-default-features` are the expected gate.
- If the repo already has GitHub Actions, ensure these commands are represented.
- If it does not, add a lightweight CI workflow unless the project intentionally avoids CI.

### CI workflow guidance

A minimal Rust workflow should include:

- checkout
- stable Rust toolchain
- cache if already used or simple no-cache if not
- fmt
- clippy all targets/all features with warnings as errors
- test all features
- test no default features

Avoid adding heavyweight external services or live network requirements.

### Tests

Not applicable beyond CI itself, but docs should mention how to run the same commands locally.

### Acceptance criteria

- Future reviewers can verify implementation without relying on commit-message claims.
- CI or explicit verification docs cover the feature combinations this repo supports.

## Workstream 8: Documentation consistency pass

### Tasks

- Update `AGENTS.md`, README, `docs/agent-workflows.md`, and `docs/tool-matrix.md` to reflect any compact/full recipe behavior.
- Document all reason-code enums added or normalized in this pass.
- Document local symlink/path policy.
- Document code-span evidence fields and identity behavior.
- Document research-claim semantics as retrieval-state metadata, not final truth conclusions.
- Ensure docs examples deserialize against current schema tests.

### Acceptance criteria

- Docs match actual response shapes.
- Agent-facing docs state default compactness and how to request more detail.
- No stale claims remain from pre-polish behavior.

## Suggested commit structure

1. `docs: record phase 6-10 polish plan status and verification matrix`
2. `feat(status): add compact workflow recipe detail controls`
3. `fix(research): improve deterministic claim wording and gap linkage`
4. `feat(code): add stable code span evidence metadata`
5. `fix(local): harden workspace fetch path validation tests`
6. `fix(security): audit remediation text for defensive-only output`
7. `feat(fetches): normalize suggested fetch reason codes`
8. `ci: add rust verification workflow`
9. `docs: update agent-facing phase 6-10 metadata guidance`

## Completion checklist

- [ ] Provider status supports compact/full/no recipe output or has documented bounded defaults.
- [ ] Rich response metadata has explicit caps and tests.
- [ ] Research claims are specific retrieval-state statements, not generic placeholders.
- [ ] Code-span evidence metadata exists for code fetches and survives evidence bundling.
- [ ] Local fetch traversal/symlink/binary/large-file behavior is tested.
- [ ] Security remediation text is defensively constrained and evidence-gated.
- [ ] Suggested-fetch reason codes are stable and broadly populated.
- [ ] Verification matrix is documented and/or represented in CI.
- [ ] Agent-facing docs match actual schemas and examples deserialize.
- [ ] `cargo fmt --check` passes.
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` passes.
- [ ] `cargo test --all-features` passes.
- [ ] `cargo test --no-default-features` passes.
