# Corrective Plan: Phase 3–5 Production Closure

Status: ready for implementation

Baseline reviewed: `7329f9a56f9f4aa7edf03c69f5995f02523c3ed7`

Depends on:

- `plans/phase-03-remote-repository-intelligence.md`
- `plans/phase-04-local-workspace-search-engine.md`
- `plans/phase-05-agent-workflow-optimization.md`

Primary goal: close the remaining correctness, safety, and integration gaps in the Phase 3–5 implementation so eggsearch can be treated as a release-candidate retrieval substrate for codegg.

This pass is corrective. It must not expand into provider proliferation, broad architecture decomposition, persistent indexing, model-based summarization, or later roadmap phases.

---

## 1. Executive Summary

The first five roadmap phases have been attempted and the repository is materially stronger. Phase 1 is effectively closed and Phase 2 has a credible property/fuzz/fault-injection foundation. The remaining production blockers are concentrated in three areas:

1. Remote repository maps do not yet preserve correct commit provenance, hard response-byte bounds, or useful nested structure.
2. Local inventory search does not yet behave correctly for active coding worktrees, particularly untracked files, index/worktree invalidation, first-use activation, command bounds, and path-policy consistency.
3. Phase 5 domain primitives exist, but they are not yet integrated into public search responses, grouping, ranking, next actions, or end-to-end codegg fixtures.

Implementation order is mandatory:

1. Remote-tree safety and provenance.
2. Remote-tree structure and classification.
3. Local inventory lifecycle and dirty-worktree correctness.
4. Agent workflow response integration.
5. Public-contract fixtures and release verification.

Do not begin response-schema expansion until remote and local evidence provenance is trustworthy.

---

## 2. Scope and Non-Goals

### In scope

- `src/meta/forge_adapter.rs`
- `src/core/repo_map.rs`
- `src/mcp/tools.rs` repo-map integration
- forge configuration and endpoint validation
- `src/meta/local_inventory_cache.rs`
- `src/meta/local_backend.rs`
- local inventory and repository identity telemetry
- `src/core/evidence_role.rs`
- `src/core/workflow_coverage.rs`
- `src/core/conflict.rs`
- `src/core/retrieval_status.rs`
- public response types for repository, security, research, and general search where required
- deterministic ranking, grouping, and next-action integration
- offline fixtures, contract tests, live smoke tests, and release gates

### Explicit non-goals

- Adding new search engines or code hosts
- Full repository cloning
- Persistent database-backed indexing
- Filesystem watchers
- Tree-sitter or LSP implementation beyond preserving the existing extension boundary
- Broad split of `src/mcp/tools.rs` or `src/meta/adapter.rs`
- Provider canary infrastructure beyond the live smoke coverage required to verify this corrective pass
- Model-generated interpretation or summaries
- Breaking schema changes

Any public contract change in this pass must be additive and guarded by schema/serialization tests.

---

## 3. Required Invariants

The corrective implementation is not complete unless all of the following hold.

### Remote tree invariants

- No forge response body is retained beyond the configured byte cap.
- The cap is enforced while streaming, not after full buffering.
- Response-level `commit_sha` is a commit identifier, never a blob or tree identifier.
- Entry object identifiers remain distinct from commit identity.
- Commit-pinned URLs use a verified commit SHA.
- Mutable URLs are labeled mutable when no commit SHA can be resolved.
- Nested entries up to `max_depth` contribute to repository classification.
- `max_entries`, `max_depth`, page, total-byte, timeout, and concurrency bounds are hard limits.
- Partial maps remain explicit and useful.
- Configured forge endpoints cannot silently widen network policy.

### Local workspace invariants

- The inventory is built automatically on first eligible search.
- Repeated warm searches do not perform unconditional full-tree walks.
- Tracked and permitted untracked files are searchable according to explicit policy.
- Ignored, hidden, skipped-directory, binary, symlink, and size policies are identical across inventory, fallback search, and direct fetch.
- External Git commands have hard runtime and output-byte limits.
- Inventory invalidation detects relevant HEAD, index, and worktree changes.
- Stale or missing entries are validated before content use.
- No inventory path can escape its canonical configured root.
- Dirty-worktree freshness and limitations are exposed to callers.

### Agent workflow invariants

- Evidence-role classification is actually populated on supported result paths.
- Workflow coverage is computed from returned evidence and retrieval failures.
- Empty evidence and failed retrieval are never conflated.
- Conflict metadata is emitted only for structured, directly comparable values.
- Grouping and ranking use evidence roles deterministically.
- Next actions contain concrete values whenever those values are already known.
- Public outputs remain bounded and deterministic.
- Existing clients that ignore new fields continue to deserialize successfully.

---

## 4. Workstream A: Bounded Forge Response Reader

### Problem

The forge adapters currently call `Response::text().await` and reject oversized bodies only after allocation. This does not enforce a hard memory bound.

### Tasks

1. Introduce one shared bounded response reader for forge API calls.
2. Reject an honest `Content-Length` larger than the per-response cap before reading the body.
3. Stream body chunks and append through one bounded helper.
4. Stop reading immediately when the cap is reached.
5. Return a typed failure such as `ResponseTooLarge` rather than matching strings.
6. Track both per-response bytes and request-total bytes across pagination and metadata probes.
7. Apply the helper to:
   - GitHub tree responses;
   - GitHub repository metadata responses;
   - GitHub Contents fallback responses;
   - GitLab tree pages;
   - GitLab project metadata;
   - Gitea/Forgejo/Codeberg tree pages;
   - forge repository metadata.
8. Ensure JSON parsing occurs only after the bounded reader succeeds.
9. Do not log body contents on parse or status errors.

### Tests

- Honest `Content-Length` over cap is rejected before body consumption.
- Chunked response whose first chunk exceeds the cap is bounded.
- Multi-chunk response crosses the cap exactly.
- Page 1 succeeds and page 2 exceeds the request-total cap.
- Metadata probe plus tree response respect the total budget.
- Malformed JSON below the cap returns typed malformed-response failure.
- Oversized error body does not become part of an error string.

### Acceptance criteria

- No forge adapter uses `.text().await` or `.bytes().await` without a prior hard bound.
- Tests prove retained bytes never exceed the configured limit.
- Partial results are preserved only when the adapter has already accumulated safe, valid pages.

---

## 5. Workstream B: Correct Ref and Commit Provenance

### Problem

GitHub tree-entry SHAs are blob/tree identifiers, but current response assembly can treat them as commit SHAs. The response-level `commit_sha` can therefore be incorrect, and generated permalinks may not be stable or valid.

### Data-model changes

Introduce explicit internal fields:

```text
resolved_ref_name
resolved_commit_sha
entry_object_sha
entry_object_kind
```

Do not overload one `sha` field for multiple Git object types.

### Tasks

1. Resolve the requested ref to an actual commit SHA before or alongside tree retrieval.
2. For GitHub:
   - resolve branch/tag/ref through the commits or Git-ref endpoint;
   - request the tree associated with the resolved commit/tree;
   - preserve the commit SHA separately from the returned tree SHA;
   - preserve each entry blob/tree/submodule SHA as `entry_object_sha`.
3. For GitLab:
   - resolve the effective ref/commit through an appropriate project/repository endpoint;
   - do not report the request string as a commit SHA.
4. For Gitea/Forgejo/Codeberg:
   - resolve the commit when the API exposes it;
   - otherwise leave `commit_sha` absent and mark URLs mutable.
5. Build commit-pinned browser/raw URLs only from `resolved_commit_sha`.
6. Retain branch/tag URLs as fallbacks where pinning is unavailable.
7. Add an additive optional entry object identifier field to `RepoMapEntry` if needed.
8. Ensure suggested fetches prefer commit-pinned locators when available.
9. Make provenance explicit in telemetry:
   - requested ref;
   - effective ref;
   - resolved commit;
   - mutable/pinned URL mode;
   - resolution failure reason.

### GitHub Contents fallback correction

The Contents fallback must:

- use the resolved requested ref, not the repository default implicitly;
- preserve full relative paths;
- distinguish directory, file, symlink, and submodule entries;
- never overwrite better tree entries with weaker fallback metadata;
- expose that the fallback is shallow/partial.

### Tests

- Branch ref resolves to commit SHA.
- Tag ref resolves to commit SHA.
- Direct commit input remains stable.
- Blob SHA never appears as response `commit_sha`.
- Generated GitHub permalink includes the resolved commit SHA.
- Fallback on a non-default branch requests that branch.
- Missing ref produces `RepoRefNotFound` rather than repository-not-found.
- Mutable URL mode is explicit when commit resolution fails.

### Acceptance criteria

- `RepoMapResponse.commit_sha` is either a verified commit SHA or `None`.
- Every commit-pinned URL is backed by the same verified commit SHA.
- Object SHAs and commit SHAs are independently represented and tested.

---

## 6. Workstream C: Nested Repository Map Assembly

### Problem

The adapters retrieve recursive trees but response assembly currently ignores entries whose paths contain `/`. This reduces remote maps to root-level listings and prevents meaningful monorepo and nested-project classification.

### Compatibility strategy

Preserve the existing `root_entries` field as root-only for compatibility. Add or populate an additive bounded `entries`/`tree_entries` field if the public contract needs all retained entries. At minimum, all retained entries must feed classification even if only root entries remain in the compatibility field.

### Tasks

1. Process every retained entry whose depth is within the configured `max_depth`.
2. Define depth consistently:
   - root entry depth = 1;
   - `src/lib.rs` depth = 2;
   - reject entries deeper than `max_depth`.
3. Classify nested files and directories for:
   - manifests and lockfiles;
   - source roots;
   - tests;
   - examples;
   - benchmarks;
   - documentation;
   - CI configuration;
   - security policy/advisories;
   - migrations;
   - generated/vendor/build/dependency paths;
   - submodules.
4. Deduplicate directory summaries when many files imply the same directory.
5. Preserve deterministic path ordering.
6. Enforce global `max_entries` after filtering and before public response assembly.
7. Report whether truncation came from:
   - provider truncation;
   - page cap;
   - byte cap;
   - entry cap;
   - depth cap;
   - deadline.
8. Populate language hints from all retained file entries rather than root files only.
9. Populate manifests from nested workspaces.
10. Improve suggested fetches so they can target nested manifests, entry points, architecture documents, and tests.
11. Avoid fetching file contents during ordinary path classification.

### Tests

Create fixtures for:

- Rust workspace with nested crates;
- JavaScript/TypeScript monorepo;
- Python package under `packages/`;
- nested CI and security files;
- multiple manifests;
- migration directories;
- generated/vendor-heavy tree;
- submodule and symlink entries;
- max-depth boundary;
- max-entry truncation with deterministic output.

### Acceptance criteria

- A remote monorepo map surfaces nested packages, source roots, tests, docs, manifests, and migrations.
- `root_entries` remains root-only if retained for compatibility.
- Classification and suggested actions use the full bounded tree.

---

## 7. Workstream D: Forge Endpoint Safety and URL Construction

### Problem

Custom forge endpoint validation is weaker than the fetch safety policy. HTTP private addresses can pass, DNS names are not classified, IPv6 handling is incomplete, and credential-bearing requests may be directed to unintended endpoints.

Self-hosted Gitea/Forgejo deployments can legitimately be private, so the solution must be explicit policy rather than an unconditional public-only assumption.

### Tasks

1. Replace ad hoc string-prefix checks with a shared configured-endpoint validator.
2. Validate:
   - scheme;
   - embedded credentials;
   - host presence;
   - literal IP classification;
   - DNS-resolved address classification;
   - redirects, if redirects are allowed;
   - normalized API base path.
3. Default policy:
   - HTTPS required for credential-bearing endpoints;
   - public addresses allowed;
   - loopback/private endpoints denied unless an explicit operator setting allows configured internal forges;
   - plaintext HTTP denied for credential-bearing endpoints;
   - optional HTTP only for an explicit local-development override with no credentials.
4. Reuse fetch address classification rather than maintaining a second range table.
5. Ensure tokens are sent only to the exact validated origin.
6. Disable automatic cross-origin redirects for forge API clients.
7. Percent-encode owner, repository, namespace, ref, and path segments appropriately per host API.
8. Do not construct a fake default Gitea hostname when no base URL is configured. Report missing endpoint configuration as a capability failure.
9. Derive browser and raw origins from validated configuration, not string slicing alone.
10. Redact endpoint credentials and tokens from all errors and telemetry.

### Tests

- HTTP loopback rejected by default.
- HTTP private address rejected by default.
- HTTPS private DNS name rejected unless internal-forge policy enabled.
- Internal forge accepted when explicitly configured.
- Credential-bearing HTTP endpoint rejected even with internal policy.
- Cross-origin redirect rejected.
- IPv6 loopback/private/documentation ranges handled correctly.
- Nested GitLab namespaces and refs with slashes are encoded correctly.
- Gitea without base URL reports structured configuration failure.

### Acceptance criteria

- Configured forge endpoints obey one documented policy matrix.
- No API token is transmitted before endpoint validation succeeds.
- Network-policy behavior is covered by the same classification corpus as fetch safety where practical.

---

## 8. Workstream E: Local Inventory Activation and Lifecycle

### Problem

The local search path can use a fresh cached inventory, but ordinary search does not reliably build the inventory on first use. This risks remaining on the legacy full-tree walker indefinitely.

### Tasks

1. Make inventory acquisition part of the normal search path:
   - cache hit: use fresh inventory;
   - cache miss: build inventory;
   - stale inventory: rebuild or safely fall back according to timeout budget;
   - build failure: structured fallback reason and legacy bounded search.
2. Prevent duplicate simultaneous builds for the same configuration.
3. Do not hold the cache mutex while performing filesystem or Git I/O.
4. Record:
   - cold build time;
   - warm cache hit;
   - stale rebuild;
   - fallback path;
   - build truncation;
   - inventory age;
   - backend used.
5. Add explicit invalidation API for tests and future host integration.
6. Ensure the first search after startup can still respect the request deadline.
7. Decide whether an inventory build may consume the whole request budget; document and test the behavior.

### Tests

- First search builds and uses inventory.
- Second search reuses it without full traversal.
- Concurrent first searches result in one build or bounded duplicate work with no corruption.
- Build timeout falls back deterministically.
- Cache poisoning through a failed partial build is impossible.
- Configuration change invalidates the inventory.

### Acceptance criteria

- Warm searches demonstrably avoid unconditional full-tree traversal.
- Inventory use does not depend on an undocumented external prewarming call.

---

## 9. Workstream F: Git Fast Path for Active Worktrees

### Problem

`git ls-files --cached` omits untracked files, and the synchronous subprocess has no hard runtime or stdout bound. This is unsuitable for active agent worktrees.

### Tasks

1. Introduce an explicit local inventory policy for untracked files.
2. Default coding-agent behavior should include:
   - tracked files;
   - untracked files not excluded by Git ignore policy;
   - exclude ignored files;
   - always apply eggsearch hidden, skip-directory, binary, symlink, and size policy.
3. Use a machine-safe invocation equivalent to:

```text
git ls-files -z --cached --others --exclude-standard
```

4. Parse NUL-delimited output to support unusual paths.
5. Do not invoke through a shell.
6. Introduce a bounded command runner with:
   - execution timeout;
   - stdout byte cap;
   - stderr byte cap;
   - child termination on timeout;
   - exit-status capture;
   - redacted diagnostics.
7. Preserve fallback to the native walker when Git is absent, malformed, timed out, or exceeds output limits.
8. Handle submodules and linked worktrees explicitly.
9. Report tracked/untracked status on inventory entries where available.
10. Record Git backend degradation through structured telemetry.

### Tests

- Untracked source file is found before staging.
- Ignored untracked file is excluded.
- Hidden and skipped parent directory rules apply to Git output.
- NUL-delimited path with spaces and unusual Unicode parses correctly.
- Timeout kills a hung command fixture.
- Oversized stdout triggers bounded fallback.
- Missing Git binary uses native fallback.
- Linked worktree and submodule behavior is explicit.

### Acceptance criteria

- A codegg-created unstaged file becomes searchable immediately after inventory refresh.
- No Git subprocess can outlive its configured bound.

---

## 10. Workstream G: Local Path Policy and Invalidation Correctness

### Tasks

1. Centralize path-component policy for:
   - hidden components;
   - `SKIP_DIRS`;
   - binary extensions;
   - symlinks;
   - canonical root containment;
   - maximum file size;
   - Git ignore behavior.
2. Use that policy in:
   - native inventory build;
   - Git inventory build;
   - legacy local search fallback;
   - direct local `repo_fetch` path validation.
3. Apply policy to every relative path component, not only the final filename.
4. Reject absolute paths and parent traversal before joining paths.
5. Validate each inventory entry before content read:
   - still exists;
   - still a regular permitted file;
   - remains inside root;
   - still below size cap;
   - fingerprint matches or stale state is handled.
6. Expand invalidation signals:
   - HEAD change;
   - Git index fingerprint/change;
   - tracked worktree status change;
   - untracked file-set change where feasible within bounds;
   - TTL;
   - explicit invalidation.
7. Avoid claiming real-time freshness without a watcher.
8. Expose dirty state and freshness confidence.
9. Detect overlapping canonical roots and avoid duplicate indexing or make duplication explicit.

### Suggested bounded fingerprints

Use cheap metadata, not full content hashing by default:

- HEAD commit;
- index file metadata or an inexpensive Git index signal;
- bounded status fingerprint;
- path + size + high-resolution modification time where available;
- content fingerprint only when the file is selected for reading.

### Tests

- Tracked file under `vendor/` is excluded consistently.
- Hidden parent path is excluded when hidden files are disabled.
- Entry replaced by symlink after inventory build is rejected before read.
- File enlarged beyond cap after inventory build is rejected.
- Staged addition invalidates inventory.
- Untracked addition invalidates or is caught by documented refresh semantics.
- Deleted file does not produce stale content.
- Overlapping roots do not duplicate results.

### Acceptance criteria

- Search and direct fetch enforce the same local-path policy.
- Active-worktree changes are reflected within documented freshness bounds.

---

## 11. Workstream H: Phase 5 Public Response Integration

### Problem

Evidence roles, workflow coverage, conflicts, and retrieval status exist as isolated domain types, but they are not yet consistently computed and returned by public tools.

### Response-contract additions

Add optional, additive fields where applicable:

```text
workflow_coverage
retrieval_summary
conflict_metadata
evidence_role_summary
```

Do not remove or reinterpret existing fields.

### Tasks

1. Populate `SourceMetadata.evidence_role` for all result conversion paths where deterministic metadata is available.
2. Define one shared post-processing stage that:
   - assigns roles;
   - builds role counts;
   - computes retrieval summary;
   - computes workflow coverage when a workflow is known;
   - detects structured conflicts;
   - produces role-aware groups/ranking/actions.
3. Integrate the stage into:
   - `repo_search`;
   - `security_search`;
   - `research_search`;
   - `web_search` where a stable workflow/intent mapping exists.
4. Keep `repo_map` focused on structure, but use evidence roles in suggested actions where relevant.
5. Map provider and dispatch outcomes into retrieval dimensions:
   - success;
   - no match;
   - skipped by policy;
   - capability unavailable;
   - failed;
   - deadline-interrupted;
   - truncated.
6. Ensure coverage status becomes `indeterminate_due_to_failures` when required evidence could not be evaluated because retrieval failed.
7. Never mark evidence absent merely because a provider failed.
8. Bound all new lists and explanations.

### Workflow selection

Use explicit request workflow/profile/mode where provided. Otherwise apply a documented deterministic mapping, for example:

- repository query with symbol/API hints → API comprehension or pre-change evidence;
- exact-error mode → error investigation;
- migration comparison → version migration;
- security search → security review;
- performance research workflow → performance investigation;
- no safe mapping → omit coverage rather than guess.

### Tests

- Stable serialization of all new optional fields.
- Legacy fixture without new fields still deserializes.
- Required role missing after successful retrieval → `insufficient`.
- Required role indeterminate after provider failure → `indeterminate_due_to_failures`.
- Empty optional roles do not degrade required coverage.
- Role assignment deterministic under randomized input order.

### Acceptance criteria

- The Phase 5 types are observable through actual MCP tool responses.
- Public response fixtures prove absence/failure semantics end to end.

---

## 12. Workstream I: Role-Aware Grouping, Ranking, Conflicts, and Actions

### Grouping

1. Preserve existing broad groups for compatibility.
2. Add role summaries or optional role-oriented groups without duplicating source cards unnecessarily.
3. Prevent one domain/provider from crowding out required evidence roles.
4. Keep global and per-group limits deterministic.

### Suggested fetch ranking

Apply explicit ordered factors:

1. Verified commit-pinned provenance.
2. Required evidence role for active workflow.
3. Exact repository/path/symbol/package/advisory match.
4. Primary or official authority.
5. Structured metadata availability.
6. Source diversity and information gain.
7. Relevant freshness.
8. Provider health/retrieval likelihood.
9. Mutable-link penalty when pinned alternative exists.

Expose machine-readable rank reasons. Do not hide the only candidate for a missing required role.

### Conflict detection

Invoke conflict detection only when values are directly comparable and share a canonical entity key. Initial supported classes:

- affected/fixed version ranges for one advisory/package;
- release date/version identity;
- provider metadata for one canonical advisory/release;
- commit-pinned versus mutable content;
- benchmark values only when metric, unit, workload, and comparison context match.

Do not infer semantic contradictions from prose snippets.

### Concrete next actions

Generate executable actions from known values:

- `repo_fetch` with resolved host/owner/repo/commit/path/line range;
- `repo_search` with concrete symbol/path/test directory;
- `web_fetch` with authoritative URL;
- `security_search` with package ecosystem, version, and advisory ID;
- `research_search` requesting a specific missing evidence role;
- `build_evidence_bundle` with actual source/fetch IDs.

Each action must state:

- target tool;
- concrete input template;
- evidence gap addressed;
- source IDs involved;
- priority;
- evidence role;
- why the action is productive.

### Tests

- Pinned source outranks mutable equivalent.
- Required-role candidate is retained despite lower generic score.
- Duplicate-domain crowding is bounded.
- Non-comparable benchmarks do not create conflicts.
- Concrete action templates validate against target tool schemas.
- No placeholders remain where values are already known.

### Acceptance criteria

- Phase 5 behavior changes agent-visible output, not only internal types.
- Ranking and action generation remain deterministic and explainable.

---

## 13. Workstream J: Codegg End-to-End Contract Fixtures

Create offline end-to-end fixtures that invoke public tool handlers and serialize the actual MCP responses.

Required scenarios:

1. Map a nested Rust workspace from a remote GitHub fixture.
2. Map a GitLab nested namespace and non-default branch.
3. Map a configured private/internal Forgejo endpoint under explicit policy.
4. Search a local worktree containing an untracked source file and ignored build output.
5. Modify/stage/delete files between searches and verify invalidation.
6. Understand a Rust API before modification.
7. Locate implementation and tests for a symbol.
8. Investigate an exact compiler error with one provider failure.
9. Compare versions for migration and surface missing migration guidance.
10. Assess a vulnerability against a local lockfile with partial advisory retrieval.
11. Review architecture from repo map plus selected fetches.
12. Build an evidence bundle with role, coverage, conflict, and retrieval linkage.

Each fixture must assert:

- deterministic IDs;
- resolved repository identity;
- correct commit provenance;
- mutable/pinned status;
- nested classification;
- hard-limit telemetry;
- evidence roles;
- workflow coverage;
- retrieval summary;
- conflict metadata where applicable;
- structured warnings;
- concrete next actions;
- evidence-bundle linkage;
- schema compatibility.

Add these fixtures to a named required CI/release target rather than leaving them as manual tests.

---

## 14. Workstream K: Verification and Release Gates

### Required local commands

At minimum:

```bash
cargo fmt --check
cargo check --all-features
cargo check --no-default-features
cargo test --locked --all-features
cargo test --locked --no-default-features
cargo test --locked --features mock
cargo test --locked --features pdf
cargo clippy --all-targets --all-features -- -D warnings
cargo doc --all-features --no-deps
cargo publish --dry-run --locked
make schema-corpus
make docs-tests
make hardening
```

Run bounded fuzz smoke targets affected by this pass:

```text
validate_url
validate_redirect_target
parse_content_length
chunk_boundary
canonicalize_url
```

Add a forge-response bounded-reader fuzz/property target if the helper is sufficiently pure.

### Live smoke verification

Live smoke tests must remain separate from deterministic CI, but evidence should be captured for:

- GitHub public repository map;
- GitLab public repository map;
- Codeberg public repository map;
- configured Gitea/Forgejo fixture or disposable test instance;
- non-default branch;
- nested repository structure;
- rate-limit/authentication failure behavior.

Do not claim all host adapters production-ready solely from HTTP mocks.

### Performance verification

Record before/after measurements for:

- cold local inventory build;
- warm local query;
- legacy fallback query;
- Git enumeration with tracked and untracked files;
- remote map of a small repository;
- remote map at entry/depth/page caps;
- memory retained during oversized forge response fixture.

Avoid brittle absolute CI thresholds. Establish baselines and reject obvious regressions.

---

## 15. Implementation Sequence and Commit Boundaries

Recommended commit sequence:

1. `fix: bound forge response bodies during streaming`
2. `fix: separate forge commit and object provenance`
3. `fix: classify bounded nested repository trees`
4. `fix: enforce configured forge endpoint policy`
5. `fix: build local inventory on first search`
6. `fix: include bounded untracked files in git inventory`
7. `fix: unify local path policy and invalidation`
8. `feat: integrate workflow coverage and retrieval summaries`
9. `feat: apply role-aware ranking conflicts and concrete actions`
10. `test: add codegg end-to-end closure fixtures`
11. `docs: document phase 3-5 closure contracts`
12. `chore: complete release-candidate verification`

Keep each commit independently testable. Do not combine remote provenance changes with public workflow response changes.

---

## 16. Documentation Updates

Update as behavior lands:

- `docs/architecture/meta.md`
- `docs/architecture/core.md`
- `docs/architecture/overview.md`
- `docs/architecture/hardening.md`
- `docs/agent-workflows.md`
- `docs/tool-matrix.md`
- `docs/config.md`
- `docs/safety.md`
- `docs/test-inventory.md`
- `docs/release.md`
- `AGENTS.md`
- relevant `.opencode/skills/` documentation

Documentation must clearly distinguish:

- commit SHA versus object SHA;
- root entries versus retained nested entries;
- public versus explicitly allowed internal forge endpoints;
- tracked/untracked/ignored local inventory policy;
- TTL versus actual worktree freshness;
- evidence absence versus retrieval failure;
- available versus omitted workflow coverage.

Contract-test documentation vocabulary where practical.

---

## 17. Definition of Done

This corrective pass is complete only when all of the following are true:

### Remote repository intelligence

- Forge bodies are hard-bounded while streaming.
- Request-total byte limits include pagination and metadata probes.
- Commit and entry object identity are correct and distinct.
- Commit-pinned URLs use verified commit SHAs.
- Non-default branch fallbacks remain branch-correct.
- Nested entries contribute to classification and next actions.
- All host adapters expose structured, typed failure semantics.
- Configured endpoints obey documented egress policy.

### Local workspace search

- First search builds the inventory automatically.
- Warm search reuses it.
- Untracked non-ignored files are searchable under explicit policy.
- Git execution is bounded by time and output size.
- All path components use the same hidden/skip/symlink policy.
- Index and worktree changes invalidate or refresh within documented bounds.
- Entries are revalidated before content reads.
- Dirty/freshness telemetry is accurate.

### Agent workflow optimization

- Evidence roles are populated in real responses.
- Workflow coverage and retrieval summaries are returned where applicable.
- Conflicts are conservative and entity-scoped.
- Grouping/ranking use evidence roles deterministically.
- Suggested fetches and next actions prefer pinned, authoritative, required evidence.
- Concrete actions validate against target tool schemas.
- Codegg end-to-end fixtures cover success, degradation, truncation, and failure.

### Release evidence

- Full feature/test/clippy/docs/publish gates pass.
- Hardening and affected fuzz smoke targets pass.
- Live smoke evidence exists for every supported host family.
- No unresolved high-severity correctness or security defect remains in the touched paths.

---

## 18. Handoff Notes

Start with the forge bounded-reader and provenance corrections. They affect the trustworthiness of every later workflow layer and should be reviewed independently.

For local search, prioritize active-worktree correctness over additional indexing sophistication. A lightweight in-memory inventory that reliably includes current tracked and untracked source files is more valuable to codegg than a faster but stale index.

For Phase 5, resist adding more enums or standalone helpers until existing types are wired through public responses. The closure criterion is observable agent behavior and end-to-end contract coverage, not domain-model completeness.

When implementation is complete, preserve this plan as the audit baseline and add a concise status/verification document recording which acceptance criteria were met, exact commands run, live-smoke evidence gathered, and any intentionally deferred items.