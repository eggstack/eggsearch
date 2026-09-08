# Phase 13 — Structured Local Code Intelligence and Repo-Map Enrichment

Status: planned
Depends on: phase 11
Baseline for planning: `4a713ff82cec701534e285bbe3d330ae121f352c`
Roadmap: `plans/maintenance-codegg-quality-roadmap.md`
Primary downstream consumer: `dbowm91/codegg`

## Objective

Improve CodeGG-facing repository retrieval by adding deterministic structured code intelligence behind the existing local symbol/search abstractions and enriching `repo_map` with package/module/symbol/test/build structure. Preserve the regex backend as the dependency-light fallback and keep all richer analysis bounded and optional.

## Current problem

The local workspace subsystem already has strong filesystem safety, inventory caching, git-aware discovery, source classification, and bounded reads. Symbol discovery is primarily regex-based for Rust, Python, JavaScript, and Go. That is adequate for lexical retrieval but weak for agent questions such as “where is this defined?”, “what tests cover it?”, “what implements this trait/interface?”, or “which module owns this symbol?”.

For CodeGG, deterministic structural relationships are likely higher value than adding additional web-search providers or a required vector index.

## Non-goals

- No mandatory embeddings/vector database.
- No required background index daemon.
- No required rust-analyzer/LSP process for baseline operation.
- No whole-program static analysis or precise call graph promise.
- No compilation of user code as part of search.
- No cross-repository dependency crawler.

## Invariants

1. Existing local-root containment, safe-open, symlink, hidden-file, ignore, and byte limits remain authoritative.
2. Regex symbol discovery remains available and is used when structured parsing is disabled, unsupported, fails, or exceeds budget.
3. Parser failures are data-quality outcomes, not fatal search failures.
4. Structured analysis must be deterministic and model-free.
5. Parsing is bounded by file size, file count, language support, and request/aggregate time budgets.
6. Generated/vendor/lockfile classification continues to influence prioritization so structural indexing does not amplify low-value trees.
7. No parser backend may execute repository code or load arbitrary native plugins from the workspace.

## Production changes

### 1. Formalize symbol backend capabilities

Audit the existing `SymbolBackend` seam and extend it only as needed to represent structured capabilities. Suggested operations:

```text
symbols_in_file(path, text)
find_definition(symbol)
find_references(symbol)        # bounded lexical/structured references
find_enclosing_symbol(path, line)
find_implementors(symbol)      # only where parser semantics support it
```

Do not require every backend to implement every operation. Use capability flags or optional results rather than fake precision.

### 2. Add a structured parser backend

Select the smallest maintainable parser strategy after dependency audit. Tree-sitter is a likely candidate if its binary/dependency impact remains acceptable, but the implementation agent must compare alternatives against existing dependency goals.

Initial language priority for CodeGG:

1. Rust
2. Python
3. JavaScript/TypeScript
4. Go

At minimum, structured extraction should identify:

- functions/methods;
- structs/classes/types;
- traits/interfaces where applicable;
- impl/implementation relationships where cheaply representable;
- modules/namespaces;
- imports/use statements;
- test functions/modules by syntax and path.

If adding all four languages in one phase would create dependency or correctness risk, land Rust + one dynamic language first but keep the interface language-neutral and leave the phase non-complete until the documented minimum is reached or a corrective plan narrows scope explicitly.

### 3. Add bounded symbol inventory caching

Extend the existing workspace inventory/cache rather than create a second unrelated index. Cache parser-derived symbol metadata keyed by stable file identity/content hash where practical.

Invalidate on the same freshness/content signals already used by local inventory. Never trust stale symbol offsets after a file hash/mtime mismatch.

Suggested symbol record:

```text
path
language
name
kind
line_start
line_end
container/module
visibility (when reliably known)
relationship hints
content_hash/version
```

### 4. Improve local search scoring

Add structured boosts for exact definition, declaration, implementation, and test relationships. Preserve lexical/path scoring as fallback.

Search should prefer:

```text
exact definition > structured symbol match > lexical symbol match > ordinary text match
```

but avoid hard rules that suppress relevant docs/issues/source context.

Expose evidence metadata indicating whether a hit came from structured parsing or regex fallback so CodeGG can reason about confidence.

### 5. Enrich `repo_map`

Extend the response additively with bounded structural summaries. Useful fields include:

```text
workspace/package boundaries
manifest/build files
language distribution
major modules/directories
entrypoint candidates
important top-level symbols
source/test relationships
CI/build configuration
```

For Rust, package/workspace membership from `Cargo.toml`/workspace manifests is especially valuable. For Python/JS/Go, use their native manifest/module conventions where already supported by dependency/package resolution code.

Do not turn `repo_map` into an unbounded recursive AST dump. Add explicit caps for symbols per file/module and total structural entries.

### 6. Add source-to-test relationship heuristics

Provide deterministic “related tests” hints using, in descending confidence:

- syntax-level test modules/functions;
- path conventions (`tests/`, `*_test.*`, `test_*.py`, etc.);
- same-symbol/name references in test files;
- manifest/package boundaries.

Label heuristic relationships as such; do not claim coverage.

### 7. Add CodeGG contract fixtures

Add representative small repositories/fixtures covering:

- Rust workspace with crate/module/trait/impl/tests;
- Python package with class/function/tests;
- JS/TS package with exports/imports/tests;
- Go module with packages/interfaces/tests.

Tests should assert useful stable structural facts rather than parser-internal node shapes.

## Performance/resource budgets

Define and document explicit defaults for:

- max parseable file bytes;
- max structured files per request/inventory rebuild;
- max symbols per file;
- max total symbols retained per workspace;
- parser time budget per file and aggregate request;
- repo-map structural entry cap.

A budget breach must degrade to partial/regex evidence with telemetry, not hang or fail the whole MCP request.

## Verification

Required routine verification: `make check`.

Add benchmarks or targeted tests comparing regex-only and structured paths on representative fixtures. Benchmarks should focus on bounded overhead, not micro-optimizing parser internals prematurely.

## Acceptance criteria

- a structured symbol backend exists behind the established local-search abstraction;
- regex fallback remains functional and tested;
- at least Rust, Python, JS/TS, and Go structured definition extraction are covered, unless a corrective plan explicitly records a blocker;
- local search can distinguish exact structured definitions from lexical matches;
- `repo_map` returns bounded package/module/symbol/test/build metadata additively;
- source-to-test relationship hints are available with explicit confidence/heuristic semantics;
- parser failures/budget breaches degrade cleanly;
- no workspace code execution is introduced;
- CodeGG contract fixtures demonstrate improved repository navigation;
- `make check` passes on the exact candidate.
