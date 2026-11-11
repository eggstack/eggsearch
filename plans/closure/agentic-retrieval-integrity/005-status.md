# M005 Deterministic Intent Derivation — Closure Status

Status: closing, applied reranking (closure commit outstanding)

Source plan: `plans/implementation/agentic-retrieval-integrity/005-deterministic-intent-derivation.md`

Roadmap: `plans/subsystems/agentic-retrieval-integrity-roadmap.md`

Planning baseline: `cd81f4d5e966364918fe2be25485b8ebbf424e71` (origin/main, 2026-11-10 UTC)

Implementation candidate: uncommitted worktree on the planning baseline. Closure commit outstanding.

## Result

`src/meta/intent.rs` derives retrieval intent from the query when the caller omits `intent`:

```rust
derive_intent(query: &str, hints: &RepoQueryHints) -> DerivedIntent { intent, reasons, confidence }
```

Fixed precedence, first match wins, over at most 512 characters: error shapes (compiler codes,
panic/traceback markers, stack frames) → security advisory identifiers and vulnerability vocabulary
→ dependency/version syntax → migration/upgrade → comparison → release/changelog → issue/PR →
code-shaped tokens and parsed repo hints → documentation vocabulary → news/recency → `web` with
reason `no_signal`. Confidence is `high` for identifiers/structural evidence, `medium` for
vocabulary, `low` for fallback. Patterns compile once (`LazyLock`); the function is pure — no clock,
network, randomness, or inference.

Reason codes are stable snake-case strings: `compiler_error_code`, `error_vocabulary`,
`panic_marker`, `stack_frame`, `vulnerability_identifier`, `vulnerability_vocabulary`,
`dependency_version`, `migration_vocabulary`, `comparison_vocabulary`, `release_vocabulary`,
`issue_vocabulary`, `repo_hint`, `symbol_hint`, `path_hint`, `file_hint`, `language_hint`,
`host_hint`, `code_token`, `docs_vocabulary`, `news_vocabulary`, `no_signal`.

## Ownership boundary

The planner never consumes derived intent. `build_search_plan()` still receives `req.intent`
verbatim, so `generic_query` and `provider_queries` are byte-identical to baseline for every
corpus query. `tests/retrieval_quality.rs::derived_intent_keeps_planner_output_and_provider_query_text`
proves it across the tier-1 queries plus eight queries that derive non-web intents, and
`web_search_integration::derived_intent_reporting::derived_intent_leaves_provider_query_text_byte_identical`
proves it through the tool path.

`web_search_with_intent(..., caller_intent: Option<SearchIntent>)` is the new entry point; the
existing `web_search` delegates with `Some(req.intent)`, so existing callers and tests are
unchanged. Responses gain `derived_intent` and `derived_intent_reasons` only when the caller omitted
intent; compact detail drops the reasons and keeps the intent string.

## Rollout gate

The first landing was deliberately observe-only because the tier-1 corpus could not exercise
intent: every case query classified as `web`/`no_signal`, making the comparison trivially identical
on every metric. That vacuous gate was corrected by adding four signal-bearing tier-1 cases whose
queries derive `security`, `releases`, `docs`, and `code` and whose provider lists place the
intent-appropriate document below a generic competitor under the neutral prior.

Measured over the enlarged 11-case tier-1 corpus:

| Metric | `intent = web` | derived | Delta |
|---|---:|---:|---:|
| recall@5 | 0.545455 | 0.681818 | **+0.136364** |
| MRR@5 | 0.442424 | 0.518182 | **+0.075758** |
| nDCG@5 | 0.375172 | 0.551254 | **+0.176082** |
| authoritative survival | 0.529412 | 0.705882 | **+0.176471** |
| duplicate slots (lower better) | 0 | 0 | 0 |
| distinct domains | 4.454545 | 4.454545 | 0 |

No aggregate or per-case regression; 13 signal-case metric improvements across all four new cases.
The seven pre-existing cases are byte-identical between the two replays. Therefore
`APPLY_DERIVED_INTENT_TO_RERANKING = true` and reranking consumes the derived intent when the
caller omitted one.

The gate is enforced, not documented: `derived_intent_rollout_gate_matches_the_recorded_decision`
recomputes the comparison and asserts the switch equals the derived decision. Before the flip it
failed with `left: false, right: true`; it fails again if a future change regresses a metric or if
the switch is flipped without evidence. `applied_reranking_consumes_the_derived_intent` asserts
that an omitted-intent query whose derived intent differs from `web` produces a different ordering
than the same query with explicit `web`, while explicit intent still wins and suppresses the
derived fields.

## Explicit-intent equivalence

`web_search` delegates with `Some(req.intent)`: no derived fields, rerank intent equals
`req.intent`, exactly the pre-refactor path. `explicit_intent_is_authoritative_and_unreported`
asserts both fields are absent for explicit `web`, `docs`, and `security`.

## Verification (combined candidate)

- `cargo test --locked --features mock --test web_search_integration`: 70 passed (five
  derived-intent tests).
- `cargo test --locked --features mock --test retrieval_quality`: 13 passed, 2 ignored.
- `cargo test --locked --features mock --test mcp_projection --test mcp_tools`: 24 + 5 passed.
- `cargo test --locked --all-features --test property_retrieval --test retrieval_attempt_ledger`:
  47 + 46 passed (three new proptests: determinism, 512-char bound independence, reason-code shape).
- `cargo test --locked --all-features --test provider_routing --test static_guards`: 87 + 60 passed.
- `cargo clippy --locked --all-targets --all-features -- -D warnings`: clean.
- `make eval-retrieval-quality` twice: identical metric blocks.
- `make check`: passed through the full `--all-features` suite (5,848 passed, 26 ignored, 0 failed)
  and hygiene; stops at the pre-existing `dependency-policy` advisories. `Cargo.lock` unchanged.

## Residual risk

- The classifier is lexical. A query with no signal stays `web`, so derivation is a bounded
  improvement, not a replacement for an explicit `intent`.
- Vocabulary classes are broad: "the issue with modern diets" derives `issues`, and "secure TLS
  configuration" stays `web` because `secure` is not `security`. Reason codes make this auditable
  and reranking is boost-capped at 30% of the maximum base score.
- `LazyLock` was used rather than `OnceLock`, matching `error_query.rs`; the compile-once property
  is equivalent.
- `CS####` is not in the existing exact-error vocabulary, so it has one additional pattern; all
  other error detection reuses `parse_query` vocabulary.
- A config that raises `search.max_query_chars` above 512 means only the first 512 characters are
  scanned for intent.
- Brief detail drops `derived_intent` because brief is an allow-list projection; the milestone
  specified compact, standard, and diagnostic only.
