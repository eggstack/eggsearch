# Phase 4: Local Workspace Search Engine

Status: ready after Phase 3 model boundaries are stable
Depends on: Phase 1; Phase 2; shared repository classification from Phase 3
Primary goal: replace repeated broad filesystem scans with a layered, cacheable, low-latency local retrieval engine while preserving lightweight deployment.

## 1. Problem Statement

The current local backend walks configured roots for each search, reads candidate files, applies path and content scoring, and enriches symbols through regular expressions. This is a useful fallback, but repeated scans become increasingly expensive on monorepos and do not provide enough structural information for advanced coding-agent workflows.

This phase introduces an incremental repository inventory and layered search architecture. It must remain useful without external commands or persistent databases, while taking advantage of Git, ripgrep, syntax parsing, or codegg-provided intelligence when available.

## 2. Required Outcomes

- Repeated searches avoid full-tree traversal and rereading unchanged files.
- Inventory invalidation is deterministic and bounded.
- Search supports path, text, language, file, symbol, and repository identity constraints.
- Git-aware and ripgrep-like fast paths are optional optimizations, not mandatory dependencies.
- Native fallback remains safe and functional.
- Syntax-aware symbol support can be enabled without changing the MCP contract.
- Local provenance, dirty state, generated/vendor classification, and truncation are explicit.

## 3. Architectural Principles

- Separate inventory from query execution.
- Separate path metadata from content and symbol data.
- Never index outside configured canonical roots.
- Do not assume a root is a Git repository.
- Prefer bounded lazy content reads over eagerly retaining all source text.
- Permit in-memory operation by default.
- Make disk persistence optional and versioned if introduced.
- Expose backend capability and freshness through `provider_status`.

## 4. Workstream A: Repository Inventory Service

### Tasks

1. Introduce a local inventory abstraction containing:
   - canonical root identity;
   - repository/worktree identity;
   - relative path;
   - file kind;
   - size;
   - modified time where reliable;
   - language;
   - generated/vendor/test/example/config/lockfile classification;
   - Git tracked/untracked/ignored status when available;
   - content fingerprint strategy;
   - manifest and important-file roles.
2. Build inventory under hard file, directory, byte, and time limits.
3. Sort entries deterministically.
4. Track inventory completeness and truncation reasons.
5. Preserve multi-root isolation and avoid duplicate indexing of overlapping canonical roots.
6. Reuse repository classification rules from remote mapping where possible.

### Invalidation

Support at least:

- explicit invalidation;
- TTL-based freshness;
- root metadata change detection;
- Git HEAD/index change detection when applicable;
- per-file lazy validation before content use.

Do not claim real-time freshness unless a watcher is actually enabled.

## 5. Workstream B: Native Search Backend

Retain a pure-Rust fallback that requires no external executable.

### Tasks

1. Query the inventory before touching content.
2. Narrow candidate files by path, filename, language, role, and size.
3. Read content only for bounded candidates.
4. Enforce global bytes-read, files-read, result, and timeout limits.
5. Improve scoring so exact symbol/path/file matches outrank broad token matches.
6. Preserve deterministic tie-breaking.
7. Return separate telemetry for inventory scan, candidate selection, content reads, and result truncation.

### Acceptance

The fallback is never less safe than the current implementation and materially reduces repeated work after inventory construction.

## 6. Workstream C: Git-Aware Fast Path

### Tasks

1. Detect whether a configured root is a Git worktree.
2. Use Git metadata to enumerate tracked files efficiently when available.
3. Preserve configured hidden, ignored, skipped-directory, binary, size, and symlink policy.
4. Include untracked files only according to explicit policy.
5. Handle submodules and linked worktrees explicitly.
6. Fall back without error when Git is missing or the repository is malformed.
7. Record which backend produced each result.

Potential command usage must:

- avoid shell interpolation;
- pass arguments directly;
- set working directory explicitly;
- enforce timeout and output byte limits;
- reject paths outside the configured root.

## 7. Workstream D: Fast Text Search Adapter

Implement an optional adapter for `rg`, `git grep`, or both.

### Required behavior

- Detect executable availability at startup or first use.
- Use machine-readable output formats.
- Apply language, path, hidden, ignore, and file-size constraints consistently.
- Limit matches, output bytes, context lines, and runtime.
- Parse output defensively.
- Fall back to native search on absence or failure.
- Report degradation through structured telemetry, not fatal errors.

### Security requirements

- No shell invocation.
- No caller-controlled raw flags.
- No unrestricted glob injection.
- Canonical root containment before execution.
- Bounded stderr capture.

## 8. Workstream E: Symbol Index

### Baseline

Retain regex symbol matching as a fallback, but move it behind a `SymbolBackend` interface.

### Optional syntax-aware backend

Evaluate a bounded tree-sitter integration or equivalent parser adapter.

Required symbol data:

- name;
- kind;
- path;
- start/end lines;
- parent symbol where available;
- language;
- signature snippet bounded by characters;
- definition/reference distinction when support exists.

Requirements:

- Parse only candidate or changed files.
- Bound file size, parse time, node count, and retained symbol count.
- Support a limited initial language set aligned with codegg usage: Rust, Python, JavaScript/TypeScript, Go, and optionally C/C++.
- Report unsupported languages without pretending regex output is syntax-exact.
- Cache symbol summaries by content fingerprint.

## 9. Workstream F: Optional Codegg Intelligence Adapter

Define a narrow optional input boundary through which a host can supply richer repository intelligence such as:

- LSP definitions and references;
- workspace symbols;
- call/type hierarchy summaries;
- SCIP/LSIF symbol records;
- build-system package graph;
- open-buffer overlays.

Constraints:

- Eggsearch must not depend on codegg crates.
- Data is supplied through a generic versioned contract or local adapter.
- Host-supplied results carry explicit provenance and freshness.
- Native local results remain available when augmentation is absent.
- Overlay content must not silently replace on-disk provenance without being marked.

This workstream may stop at interface design and fixtures if codegg integration is scheduled separately.

## 10. Workstream G: Local Repository Matching

Improve matching between requested host/owner/repo identity and configured local roots.

### Tasks

- Normalize SSH and HTTPS remotes.
- Support multiple remotes.
- Handle case sensitivity according to host semantics.
- Distinguish exact, strong, ambiguous, and absent matches.
- Report branch, commit, detached HEAD, and dirty state.
- Detect worktree overlays and subdirectory package roots.
- Avoid silently selecting among ambiguous matches.

## 11. Workstream H: Query and Response Semantics

Additive telemetry should expose:

- backend used;
- inventory age;
- inventory completeness;
- files considered;
- files read;
- bytes read;
- symbol backend;
- fallback reason;
- result freshness;
- repository match confidence;
- dirty-state caveat;
- truncation and timeout reasons.

Suggested next actions should prefer:

- exact symbol definition fetch;
- enclosing implementation block;
- adjacent tests;
- relevant manifest or feature configuration;
- caller/reference search when augmented intelligence supports it.

## 12. Testing Strategy

### Fixtures

Create representative local repositories for:

- small Rust crate;
- Python package;
- TypeScript monorepo;
- nested multi-language workspace;
- dirty Git worktree;
- non-Git directory;
- ignored/generated/vendor-heavy repository;
- symlink and permission edge cases;
- overlapping roots;
- linked worktrees and submodules.

### Required tests

- Inventory cold build and warm reuse.
- File modification invalidation.
- Git HEAD and index invalidation.
- Deterministic ordering.
- Native versus fast-backend result equivalence for common cases.
- Backend absence and failure fallback.
- Hard file, byte, result, and timeout caps.
- Symbol cache invalidation.
- Ambiguous repository matching.
- Low-power configuration.

## 13. Performance Gates

Define benchmark repositories checked into fixtures or generated deterministically.

Track:

- cold inventory time;
- warm query latency;
- content bytes read per query;
- memory retained by inventory;
- symbol indexing time;
- invalidation cost;
- low-power profile behavior.

Avoid brittle absolute CI thresholds initially. Establish baselines and fail only on large regressions until runner variance is characterized.

## 14. Definition of Done

- Repeated local queries do not perform an unconditional full-tree reread.
- Inventory behavior and freshness are visible to callers.
- Native fallback remains complete enough for minimal deployments.
- Optional Git/text/syntax backends degrade cleanly.
- Search and fetch path policy remain identical.
- Symbol precision is improved for supported languages.
- Repository matching handles common worktree and remote configurations.
- New behavior is contract-tested and benchmarked.
- Full release gate passes.

## 15. Handoff Notes

Implement inventory and native query separation before adding external command adapters. Do not introduce a persistent database in the first pass. An in-memory versioned inventory with explicit invalidation is sufficient to validate the architecture and preserve eggsearch's lightweight operating model.