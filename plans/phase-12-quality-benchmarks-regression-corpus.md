# Phase 12 Plan: Quality, Benchmarks, and Regression Corpus

## Objective

Build an offline mocked evaluation corpus and optional live smoke-test suite so eggsearch quality does not regress silently. The corpus should cover coding-agent retrieval workflows: repository overview, API lookup, symbol search, exact-error investigation, migration planning, package lookup, CVE/package triage, local workspace routing, code-host raw fetch transforms, and architectural research.

This phase should make ranking, routing, warnings, evidence metadata, and suggested fetch behavior testable over representative scenarios. It should not rely on live web availability for core correctness.

## Current baseline

The repo has a large test suite and many targeted unit/integration tests. It also has deterministic ranking, provider diagnostics, structured fetches, repo maps, security/research search paths, and local workspace support.

The missing layer is a curated regression corpus that tests whole workflows and expected evidence shapes rather than only individual helpers.

## Non-goals

Do not add model-based grading. Do not require live web for default CI. Do not snapshot arbitrary live search result text. Do not create brittle tests that fail because a public website changed title text. Do not benchmark against proprietary services as a required check.

## Corpus design principles

1. Offline by default.
   - Use mock providers, fixture HTTP responses, and deterministic local repos.
2. Workflow-level, not only function-level.
   - Assert final response shape, warnings, group types, suggested fetches, and trust markers.
3. Stable expected outputs.
   - Avoid exact full-response snapshots if fields legitimately vary.
   - Prefer targeted JSON assertions.
4. Covers negative cases.
   - Provider unavailable, native filters not enforced, malformed dependency files, missing symbol, stale local checkout, no advisory range.
5. Compatible with future providers.
   - Tests should not forbid extra provider metadata unless the specific behavior is central to the scenario.

## Proposed layout

Add a dedicated test/corpus directory:

```text
tests/corpus/
  README.md
  scenarios/
    repo_map_axum.json
    repo_search_symbol.json
    exact_error_rust.json
    migration_crate_version.json
    security_osv_applicability.json
    local_workspace_match.json
    research_architecture_decision.json
  fixtures/
    repos/
      rust_workspace_small/
      node_package_small/
      polyglot_service/
    http/
      osv/
      registries/
      code_hosts/
    search_results/
      repo_symbol_results.json
      exact_error_results.json
      research_sources.json
  expected/
    repo_map_axum.expected.json
    ...
```

If JSON scenario runners are too heavy initially, implement Rust fixtures directly and add the directory structure as the target for later expansion.

## Scenario schema

Define a lightweight scenario format:

```json
{
  "id": "repo_search_symbol_rust",
  "description": "Find a Rust symbol in a repo and suggest source fetches",
  "tool": "repo_search",
  "request": { ... },
  "mock_providers": [ ... ],
  "local_workspace": { ... },
  "expect": {
    "groups_present": ["source_files", "official_docs"],
    "warnings_contain": [],
    "suggested_fetches": {
      "min": 1,
      "preferred_tool": "repo_fetch",
      "contains_source_kind": "source_file"
    },
    "trust": "external_untrusted"
  }
}
```

Keep the schema permissive. It should assert important behavior without freezing every rank score or warning string unless that string is a public contract.

## Workstream 1: Corpus runner

### Implementation

Add a test helper that can load scenario JSON and execute the corresponding tool path with mock providers and fixture local workspace.

Start with a small subset:

- `repo_search`.
- `repo_fetch` using local fixture server or direct test URL override.
- `repo_map`.
- `security_search`.
- `research_search`.

If invoking MCP wrappers is easier than lower-level adapter calls, use MCP wrapper functions so tests cover schema-level behavior.

### Tests

- Scenario parser rejects invalid scenario.
- Scenario runner executes a simple mock `web_search` or `repo_search` case.
- Expected assertion failures produce useful messages.

## Workstream 2: Repository workflows

Create scenarios for:

1. Repo-only discovery.
   - Request: `repo_search` with owner/repo and empty query.
   - Expect: docs/source/examples/releases subqueries, warnings only where providers cannot enforce.
2. Repo map.
   - Request: `repo_map` over fixture repository.
   - Expect: README, manifest, source root, tests/examples/docs, suggested fetches.
3. Symbol search.
   - Request: `repo_search` with symbol/path/language hint.
   - Expect: source file group, structured repo_fetch suggested fetch.
4. Exact-error mode.
   - Request: compiler/toolchain error.
   - Expect: exact phrase/error code subqueries, issues/changelog prioritized, redaction if sensitive markers present.
5. Span fetch.
   - Request: symbol/match text with block expansion.
   - Expect: selected span, bounded lines, correct warnings on missing symbol.

## Workstream 3: Package and migration workflows

Create scenarios for:

1. crates.io package lookup.
2. npm/PyPI lookup.
3. Go/Maven/NuGet/RubyGems/Packagist lookup after Phase 8 lands.
4. Version migration query with changelog/release suggested fetch.
5. Package resolution fallback when registry metadata unavailable.

Expected assertions:

- Registry evidence present.
- Source repository inferred where available.
- Changelog/release evidence prioritized for compare-version requests.
- Package-resolution warnings visible on fallback.

## Workstream 4: Security workflows

Create scenarios for:

1. CVE ID search.
2. GHSA ID search.
3. OSV package/version applicability after Phase 9 lands.
4. RustSec applicability fixture.
5. Local lock-file finding maps to advisory range.
6. Unknown applicability when version/range cannot be compared.

Expected assertions:

- Advisory source group present.
- Applicability status is correct.
- Confidence and reasons present.
- Applicability-not-exploitability warning present.
- Suggested fetch points to structured advisory and dependency file span where applicable.

## Workstream 5: Research workflows

Create scenarios for:

1. Architecture decision workflow.
2. API/library comparison workflow.
3. Performance investigation workflow.
4. Security review research workflow.
5. Counterpoints requested.

Expected assertions:

- Primary source or official docs group present when available.
- Counterpoint gap appears if requested but absent.
- Research workflow context/gaps are deterministic.
- Suggested fetches prioritize primary sources and reference implementations.

## Workstream 6: Local workspace workflows

Create fixture repos with Git metadata where possible:

- Clean Rust repo.
- Dirty repo with uncommitted file.
- Repo with malformed/unknown Git state.
- Repo with remote URL matching requested owner/repo.
- Repo with same owner/repo but different host to ensure no false match.

Expected assertions:

- Matching local checkout gets `local_trusted` evidence.
- Dirty warning appears.
- Unknown Git state warning appears.
- Host mismatch does not redirect or boost local evidence.
- `prefer_local` fetch obeys path traversal protections.

## Workstream 7: Code-host coverage workflows

After Phase 11 lands, add scenarios for:

- Codeberg repo_fetch.
- Gitea configured host repo_fetch.
- Forgejo configured host repo_fetch.
- Browser-to-raw web_fetch transform.
- Unsupported/unconfigured host rejection.

Expected assertions:

- Raw URL transform metadata present.
- SSRF safety and path validation remain enforced.
- Batch fetch handles supported and unsupported items independently.

## Workstream 8: Ranking regression checks

Add targeted assertions for suggested fetch ranking:

- Commit-pinned raw/source locator outranks mutable browser URL when both point to same source.
- Exact-error issue/changelog evidence outranks generic docs.
- Migration request prioritizes changelog/release notes.
- Security request prioritizes OSV/GHSA/RustSec/NVD over generic blog posts.
- Research request prioritizes official docs/spec/reference implementation over community discussion when primary sources requested.

Avoid exact numeric score assertions unless scores are intentionally public. Prefer order/category assertions and rank-reason presence.

## Workstream 9: Optional live smoke tests

Add ignored tests or a separate command for live checks:

```bash
cargo test --features live-smoke -- --ignored
```

Live tests should:

- Use public stable targets.
- Be skipped unless credentials/config are present.
- Assert broad behavior only.
- Never block default CI.

Examples:

- `repo_map` public GitHub repo.
- `repo_fetch` public file.
- OSV advisory lookup.
- Package registry lookup.

## Workstream 10: CI integration

If GitHub Actions exists, add corpus tests to default CI using offline fixtures. If no CI exists, create a minimal workflow in this phase or a separate small infra pass:

```yaml
- cargo fmt --check
- cargo clippy --all-features --all-targets -- -D warnings
- cargo test --all-features
```

If all-features is too heavy, define the project-supported feature matrix explicitly.

## Documentation

Add `tests/corpus/README.md` explaining:

- How to add scenarios.
- What assertions should and should not test.
- How to update expected outputs safely.
- Difference between offline corpus and live smoke tests.
- How to run targeted scenario tests.

Update main README with a short quality/regression section.

## Acceptance criteria

- Offline corpus runner exists or a clear Rust-fixture equivalent exists.
- Corpus covers repo map, repo search, exact-error, span fetch, package lookup, security applicability, local workspace, code-host transforms, and research workflows as phases land.
- Tests assert source kinds, suggested fetch categories, warnings, trust markers, provider diagnostics, and gaps.
- Ranking regressions are covered by targeted order/rank-reason assertions.
- Default tests do not require live web or credentials.
- Optional live smoke tests are ignored or feature-gated.
- CI runs formatting, clippy, and offline tests if repository policy permits.
- Documentation explains how to extend and maintain the corpus.
