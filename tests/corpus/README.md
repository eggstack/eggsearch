# Regression Corpus

Offline mocked evaluation corpus for eggsearch quality regression testing.
Covers coding-agent retrieval workflows without requiring live web access.

## Structure

```text
tests/corpus/
  README.md          — this file
  scenarios/         — JSON scenario definitions (reference format)
  expected/          — expected output snapshots (reference format)
```

The primary test implementations live in `tests/corpus_runner.rs` as
Rust-native fixtures. The JSON scenario files are a reference format for
future expansion and documentation of intent.

## Scenarios

| Scenario | Tool | What it tests |
|---|---|---|
| `repo_map_axum` | `repo_map` | Repository structure discovery with suggested fetches |
| `repo_search_symbol` | `repo_search` | Symbol search with source-file grouping and repo_fetch suggestions |
| `repo_search_docs` | `repo_search` | Docs intent with official_docs and source_files groups |
| `exact_error_rust` | `repo_search` | Exact-error mode with error-code subqueries and redaction |
| `security_cve_lookup` | `security_search` | CVE ID search with advisory grouping and KEV warning |
| `security_osv_applicability` | `security_search` | OSV package/version applicability assessment |
| `research_architecture` | `research_search` | Architecture decision workflow with counterpoints |
| `research_library_comparison` | `research_search` | Library comparison workflow with compare targets |
| `ranking_commit_pinned` | `repo_search` | Commit-pinned raw permalink outranks mutable URL |
| `ranking_exact_error` | `repo_search` | Exact-error issue evidence outranks generic docs |
| `ranking_migration` | `repo_search` | Migration request prioritizes changelog/release notes |
| `ranking_security` | `security_search` | Security request prioritizes advisory sources |
| `security_applicability_range_boundary` | `security_search` | `>= 2.0.0, < 3.0.0` boundary with below/inside/fixed versions |
| `security_applicability_unknown_syntax` | `security_search` | Unparseable range syntax returns Unknown |
| `security_applicability_unsupported_range` | `security_search` | Unsupported OSV GIT range returns Unknown |
| `security_applicability_multiple_ranges_affected_dominates` | `security_search` | Affected dominates NotAffected across ranges |
| `security_applicability_multiple_ranges_unknown_collapses` | `security_search` | Unknown + NotAffected collapses to Unknown |
| `live_smoke` | various | Optional live tests (feature-gated, ignored by default) |

## What assertions cover

- **Response shape**: groups, suggested_fetches, warnings, trust_markers,
  providers_queried present and correct type
- **Group kinds**: expected group kinds present (e.g. SourceFiles, OfficialDocs)
- **Source kinds**: source_kind metadata on cards matches expectation
- **Suggested fetches**: min count, preferred tool, structured repo_fetch
  locators present when applicable
- **Warnings**: expected warnings present (e.g. capability degradation)
- **Trust markers**: trust_markers object present
- **Ranking order**: rank_reasons present, expected ordering between
  competing candidates
- **Applicability**: security applicability status, confidence, and
  applicability-not-exploitability warning
- **Range boundary correctness**: `>= 2.0.0, < 3.0.0` with versions
  below (NotAffected), inside (Affected), and at fixed boundary
  (NotAffected) — catches inverted comparison bugs
- **Conservative Unknown**: unparseable ranges and unsupported range
  types (e.g. OSV GIT) return Unknown, not implicit affected/not_affected
- **Multi-range combination**: Affected dominates NotAffected;
  Unknown + NotAffected collapses to Unknown
- **Telemetry**: provider_selection, subqueries, capability_enforcement
  fields present

## What assertions do NOT cover

- Exact score values (scores are implementation detail)
- Exact warning text (prefixes are stable, full text is not)
- Full response snapshots (fields legitimately vary)
- Provider-specific internal behavior
- Network-dependent live content

## How to add scenarios

1. Define the mock engines with `MockEngine::success()` or
   `MockEngine::failure()` containing representative URLs and snippets.
2. Build a `ServerState` with `state_with_engines()`.
3. Call the appropriate tool (`run_repo_search`, `run_security_search`, etc.).
4. Assert on the JSON response shape using the patterns in existing tests.
5. Add a JSON scenario file in `scenarios/` documenting the intent.

## Running

```bash
# All corpus tests
cargo test --features mock --test corpus_runner

# Single scenario
cargo test --features mock --test corpus_runner repo_map_axum

# With output
cargo test --features mock --test corpus_runner -- --nocapture
```

## Live smoke tests

Optional live smoke tests are behind the `live-smoke` feature flag and
are ignored by default. They test real network access against stable
public targets.

```bash
cargo test --features live-smoke --test corpus_runner -- --ignored
```
