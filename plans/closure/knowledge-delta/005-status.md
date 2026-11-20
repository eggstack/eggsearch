# M005 Verified Model-Cutoff Coverage and Provenance — Closure Evidence

Status: closed

Source plan: `plans/implementation/knowledge-delta/005-verified-model-cutoff-coverage.md`

Roadmap: `plans/subsystems/knowledge-delta-corrective-addendum.md` M005

Corrective to: M001, plan `plans/archive/implementation/knowledge-delta/001-cutoff-aware-knowledge-brief.md`,
closure `plans/closure/knowledge-delta/001-status.md`.

Decision authority: accepted ADR-0020 (cutoff-aware knowledge brief) and accepted ADR-0006
(keyless deterministic retrieval). No new ADR and no change to cutoff-comparison semantics.

## Scope

M001 shipped a correct resolver with one verified row, so almost every real agent model
resolved to `unknown`. M005 makes the catalog a maintained, reviewable artifact:

1. `src/meta/knowledge_delta/catalog.json` is the checked-in manifest. Each entry records
   `model_id`, aliases, date, precision, source, HTTPS source URL, verification date, origin,
   one of four statuses, and a short evidence note.
2. Only entries with status `verified` populate `BUNDLED_CUTOFF_SNAPSHOT`. An entry is
   `verified` only when an official vendor page states the cutoff verbatim at that
   precision.
3. `Catalog::validate()` rejects duplicate model IDs, duplicate or shadowing aliases,
   dates that contradict their declared precision, a `verified` entry without an HTTPS
   source URL and verification date, an unsupported status or origin, and more than 128
   entries. `Catalog::verify_snapshot()` rejects any field-level disagreement in either
   direction between the manifest and the bundled snapshot.
4. `model_resolution.catalog_as_of` reports the newest `verified_at` in the shipped
   snapshot, so a stale catalog is visible without a schema break.
5. `docs/knowledge-delta-cutoffs.md` documents the statuses, the resolution precedence, the
   verification procedure, the update steps, and a redistribution note.

Ranking, cutoff comparison, identity, and tool surfaces are untouched.

## Exact code candidate

Implementation candidate `e817dfde35372aa307f5580a144d7ededcb55574`. The manifest is 6,313
bytes with sha256 `d6ec6abad6b97e294b034cde81072b2b44cecdc3f83a81c1c5bc71339c77fed8`. This
closure commit carries this record, the plan status, the roadmap and addendum status, and the
registry rows together.

## Coverage actually verified

The checked set is the priority inventory, not every model the vendors publish.

| Model ID | Cutoff | Precision | Family | Verbatim source wording |
|---|---|---|---|---|
| `claude-fable-5-2` | 2026-06 | month | Anthropic | `Reliable knowledge cutoff: Jun 2026` |
| `claude-haiku-5-5` | 2026-06 | month | Anthropic | `Reliable knowledge cutoff: Jun 2026` |
| `claude-opus-5-5` | 2026-06 | month | Anthropic | `Reliable knowledge cutoff: Jun 2026` |
| `claude-sonnet-5-5` | 2026-06 | month | Anthropic | `Reliable knowledge cutoff: Jun 2026` |
| `gpt-5.5` | 2025-12-01 | day | OpenAI | `Dec 01, 2025 knowledge cutoff` |
| `gpt-6-astra` | 2026-04-30 | day | OpenAI | `Apr 30, 2026 knowledge cutoff` |
| `gpt-6-luna` | 2026-05-18 | day | OpenAI | `May 18, 2026 knowledge cutoff` |
| `gpt-6.1-terra` | 2026-06-01 | day | OpenAI | `Jun 01, 2026 knowledge cutoff` |
| `gpt-6.2-astra` | 2026-05-18 | day | OpenAI | `May 18, 2026 knowledge cutoff` |
| `gpt-6.2-sol` | 2026-06-01 | day | OpenAI | `Jun 01, 2026 knowledge cutoff` |
| `grok-4.7` | 2026-05 | month | xAI | `The knowledge cut-off date of Grok 4.7 is May 2026.` |

Verified coverage is **11 of 16** listed IDs across **three** provider families. The
acceptance target was six verified IDs across two families "if supported by source
evidence"; the sources supported more, so no substitution was needed.

Five further IDs are recorded with an explicit disposition rather than a guessed date:
`deepseek-flash`, `deepseek-v4-pro` (official pricing page publishes no cutoff),
`gemini-4-carbon`, `glm-5.2`, and `glm-5.3` (official model pages publish no cutoff).
No `unresolved_conflict` or `alias_unconfirmed` entry was warranted by the sources
checked. The two pre-existing aliases (`openai/gpt-6-astra`, `xai/grok-4.7`) were retained
and are covered by the alias-resolution test.

## Requirement-to-evidence mapping

| # | Acceptance criterion | Evidence |
|---|---|---|
| 1 | Every priority lookup is classified with dated evidence or an auditable unknown | 16 manifest entries, each with a status, an official URL, `verified_at` `2026-11-20`, and a note. Five are `no_published_cutoff`, not absent. Test `tests/knowledge_delta_catalog.rs::verified_rows_exclude_unpublished_cutoffs`. |
| 2 | The snapshot grows beyond one verified row without falsely assigning a cutoff | Eleven snapshot rows, all `CutoffOrigin::ProviderOfficial`. Test `::embedded_catalog_matches_bundled_snapshot` compares every field in both directions. |
| 3 | At least six verified IDs across two families, when source evidence supports it | Eleven across OpenAI, Anthropic, and xAI. |
| 4 | Alias resolution and date precision are deterministic; no runtime network call | `resolve_cutoff` is unchanged and offline. Tests `::every_verified_model_resolves_from_the_bundled_snapshot`, `::every_alias_resolves_to_its_row_cutoff`, `::unknown_model_returns_none`, `::operator_override_precedes_the_bundled_snapshot`, `::date_and_precision_mismatch_is_rejected`. |
| 5 | Import hygiene rejects collisions, precedence inversions, and unbounded metadata | `Catalog::validate()` and tests `::duplicate_alias_across_entries_is_rejected`, `::alias_shadowing_another_model_id_is_rejected`, `::verified_entry_requires_source_url_and_verified_at`, `::catalog_size_is_bounded`. |
| 6 | A missing or extra snapshot row is a defect, not silent drift | Tests `::verified_entry_missing_from_snapshot_is_rejected` and `::snapshot_row_missing_from_catalog_is_rejected`. |
| 7 | Existing cutoff behavior, IDs, and override precedence do not regress | `property_knowledge_delta` 9/9, `knowledge_delta_brief` 17/17, `mcp_tool_contract` 22/22. |
| 8 | The manifest is inspectable and the local extension is safe | `docs/knowledge-delta-cutoffs.md` publishes the table and the procedure; only normalized facts and short citations are stored. |
| 9 | Exact-candidate broad gate passes | `make check` exit 0 and `make docs-check` exit 0 on `e817dfd`; see Verification. |

## Verification

Using pinned Rust 1.89 on candidate `e817dfd`:

- `make check`: exit 0 — 6,549 passed, 0 failed, 28 ignored on `--all-features`.
- `make docs-check`: exit 0.
- `tests/knowledge_delta_catalog.rs`: 14 passed.
- `tests/knowledge_delta_brief.rs`: 17 passed.
- `tests/property_knowledge_delta.rs`: 9 passed.
- `tests/static_guards.rs`: 67 passed.
- `cargo clippy --locked --all-targets --all-features -- -D warnings`: clean.
- `cargo fmt -- --check` and `git diff --check`: clean.

Source verification used `web_fetch` against official vendor pages on 2026-11-20. Three
pages were re-checked independently during closure review: `gpt-6.2-sol` returned
`Jun 01, 2026 knowledge cutoff`, `gpt-6-luna` returned `May 18, 2026 knowledge cutoff`,
and `glm-5.3` returned a model guide with no cutoff statement.

## Known limitations and residual risk

1. MiniMax is not represented. An attempt to locate an official MiniMax model page ended
   with an HTTP 402 balance error from the search path, and no model ID or URL was guessed.
   This is a coverage gap with a recorded cause, not a silent omission.
2. The catalog is refreshed by a human following `docs/knowledge-delta-cutoffs.md`; there is
   no scheduled vendor sync. `catalog_as_of` makes staleness visible but does not fix it.
3. Four Anthropic rows share a month-granularity source, so their cutoff is `month`
   precision. A caller comparing against a day-precision cutoff gets an indeterminate result
   in the shared month, which is the intended conservative behavior, not a defect.
4. Redistribution terms were not reviewed by counsel. The manifest stores facts and short
   citations only, and the operator doc states that current vendor terms must be checked
   before redistribution.

## Compatibility and migration

No tool, argument, schema, configuration, or storage change. `model_resolution` gains one
additive key, `catalog_as_of`. Existing callers that resolved `unknown` for a model may now
resolve a verified cutoff, which changes output values but not the shape or the precedence
rules.

## Hosted qualification

The candidate was pushed to `origin/feat/auto-20261117-044c4885`. On `bc52970` (this closure
commit, which differs from the candidate only in planning documents), CI run
`44812224641` needed three attempts:

1. The `ci` and `windows portability` jobs succeeded. The
   `macOS canonical verification and local filesystem portability` job failed on
   `tests/local_workspace_integration.rs::local_repo_search_finds_source_files`
   (`tests/local_workspace_integration.rs:793`), a temporary-git-repository fixture that this
   milestone does not touch.
2. `gh run rerun --failed` failed at the same assertion.
3. A third attempt passed all three jobs, including macOS canonical verification.

The compiled test binary in both failing attempts hashes `3ace87ef457959f8`, the same binary
that passed this test in the `a9fc806` macOS job (`44797522795`). A byte-identical test binary
cannot be changed by this milestone's source, so the two failures are environment-dependent
rather than a regression introduced here. Locally the exact test passes 8 of 8 repeats and the
full `make check` passes on the same tree. `Egress feature qualification` and
`Release drift guard` succeeded on the candidate in the same push.

The hosted pipeline is green on the final attempt; the two earlier macOS failures are recorded
rather than omitted.

## Recommendation

Accept M005 on `e817dfde35372aa307f5580a144d7ededcb55574`. The catalog is real, sourced,
validated in both directions against the shipped snapshot, and honest about the five models
whose vendors publish no cutoff.

This acceptance includes a green hosted CI run on the third attempt; the two earlier macOS failures on an unrelated environment-sensitive test are recorded above rather than omitted.
