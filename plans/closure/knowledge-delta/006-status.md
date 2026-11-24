# M006 Opt-In Index Onboarding, Diagnostics and Safe Seeding — Closure Evidence

Status: closed

Source plan: `plans/archive/implementation/knowledge-delta/006-opt-in-index-onboarding-and-seeding.md`

Roadmap: `plans/subsystems/knowledge-delta-corrective-addendum.md` M006

Corrective to: M002 and M003, plans
`plans/archive/implementation/knowledge-delta/002-local-evidence-index.md` and
`003-source-driven-topic-discovery.md`, closures
`plans/closure/knowledge-delta/002-status.md` and `003-status.md`.

Decision authority: accepted ADR-0021 (opt-in, public-only retained evidence, no implicit
write or feed crawler) and accepted ADR-0006. No new ADR, no default change, no new tool.

## Scope

The index stayed correctly disabled by default, but an operator could not tell "off" from
"broken" or "empty" from "expired". M004 had already added `index_status` and the
`empty_state_reason` vocabulary; M006 extends those fields instead of adding a parallel one:

1. `coverage.index_status: disabled` is now distinct from `offline`. A request that asks to
   read or retain while `[knowledge_delta.index].enabled` is false never opens or creates
   the database, and returns one `knowledge_brief_index_disabled:` warning that names the
   configuration key and no filesystem path or topic.
2. `EvidenceIndex::status(partition)` returns a bounded, partition-scoped
   `IndexStatus { documents, newest_indexed_at }`, and the pure
   `classify_index_freshness(status, retention_days, now)` maps it to
   `NeverSeeded`, `Fresh`, or `Stale`. No clock is read inside the classifier.
3. An `index` refresh that emits nothing reports `index_not_seeded` when the partition holds
   no retained documents, `index_stale` when every retained document is older than
   `retention_days`, and keeps `no_matching_index_hits` when retained documents exist but
   none matches after the cutoff. The two new reasons are unreachable for any other refresh.
4. A status-probe failure degrades the index (`index_status: degraded`) and adds the
   `IndexError` message as a warning; it does not fail the call.
5. `model_resolution.catalog_as_of` is added here only because M005 needed the field and
   this milestone owns `execute.rs`; no other coverage key was added.
6. `docs/config.md` now documents the two-call setup (enable, then a live `retain` seed,
   then an index or prefer-live read) and each state. `docs/tool-matrix.md`,
   `docs/agent-workflows.md`, `architecture/research.md`,
   `architecture/evidence-workflow.md`, `docs/features.md`, and `docs/test-inventory.md`
   carry the same vocabulary.

No MCP tool, CLI command, schema migration, or default changed. The permission rejection
message, the `0700`/`0600` rules, the no-store/private/profile refusal, and the partition
model are untouched.

## Exact code candidate

Implementation candidate `e817dfde35372aa307f5580a144d7ededcb55574`. This closure commit
carries this record, the plan status, the roadmap and addendum status, and the registry rows
together.

## State model as shipped

| Situation | `index_status` | `empty_state_reason` | Warning |
|---|---|---|---|
| Ordinary live call, no retain or index refresh | `not_opened` | live reason for the result | unchanged |
| Read or retain while disabled | `disabled` | `no_cutoff_resolved`, `no_dated_sources`, or `all_sources_at_or_before_cutoff` | `knowledge_brief_index_disabled:` and, for an `index` refresh, the existing freshness warning |
| Enabled, never seeded, `refresh: index` | `open` | `index_not_seeded` | `knowledge_brief_index_not_seeded:` |
| Enabled, seeded, all documents past `retention_days` | `open` | `index_stale` | `knowledge_brief_index_stale:` |
| Enabled, seeded, no post-cutoff match | `open` | `no_matching_index_hits` | `knowledge_brief_no_matching_index_hits:` |
| Lookup or status probe failed | `degraded` | falls through to the dated-source reasons | the `IndexError` message |
| Could not open (corrupt, newer schema, read-only, full, insecure mode) | `offline` | falls through | the `IndexError` message |

## Requirement-to-evidence mapping

| # | Acceptance criterion | Evidence |
|---|---|---|
| 1 | A clean operator can enable, seed, restart, and read back related evidence | `tests/knowledge_delta_brief.rs::cold_start_enable_seed_restart_and_related_reuse`: disabled `retain` rejected and no directory created, enabled live `retain` persists one delta, the reopened index returns the same `kdelta_` id with `provenance: index_only`, trust `external_untrusted`, and `date_basis: publication`, and `related_topics` adds the neighbor with a source span and `coverage.expansion: added`. |
| 2 | The ordinary live workflow creates zero index files or writes | `::ordinary_live_call_writes_nothing` asserts `index_status: not_opened` and no `knowledge-delta` directory; `::disabled_index_refresh_reports_disabled_and_creates_no_file` asserts the same for an index refresh while disabled. |
| 3 | A disabled index is distinguishable from an unreadable one | `::disabled_index_refresh_reports_disabled_and_creates_no_file` (`disabled`) versus the pre-existing corruption, read-only, full, and permission tests in `tests/knowledge_delta_index.rs`, which still produce `offline` through `IndexError`. |
| 4 | Never-seeded, stale, and no-match are distinct | `::enabled_but_unseeded_index_reports_not_seeded`, `::stale_retained_index_reports_index_stale`, `::seeded_index_without_a_post_cutoff_match_reports_no_matching_hits`. |
| 5 | The classifier is bounded and deterministic | Unit tests in `src/meta/knowledge_delta/index.rs`: never seeded, zero retention, exactly at the boundary, one second past it; plus `status_counts_empty_and_retained_partitions`. |
| 6 | Status is partition-scoped and rejects an invalid partition | `tests/knowledge_delta_index.rs::status_reports_partition_scoped_documents` and `::status_rejects_an_invalid_partition`. |
| 7 | M004 vocabulary and the permission message do not regress | `::seeded_index_without_a_post_cutoff_match_reports_no_matching_hits` keeps `no_matching_index_hits`; `::index_permission_warning_names_directory_and_required_mode` still asserts the directory, `0755`, and `0700`. |
| 8 | Warnings carry no paths, topics, or raw excerpts | The three new warnings are fixed strings; the disabled path is taken before any path is computed. |
| 9 | Failure and repair cases stay covered | `tests/knowledge_delta_index.rs` retains corruption refusal, read-only write failure, `max_bytes` full without partial commit, private/invalid batch rollback, refused body classes, shared parent rejection, owner-only modes, two-process serialization, interrupted transaction rollback, and v1 migration scrubbing. |
| 10 | Exact-candidate broad gate passes | `make check` exit 0 and `make docs-check` exit 0 on `e817dfd`. |

## Verification

Using pinned Rust 1.89 on candidate `e817dfd`:

- `make check`: exit 0 — 6,549 passed, 0 failed, 28 ignored on `--all-features`.
- `make docs-check`: exit 0.
- `tests/knowledge_delta_index.rs`: 15 passed.
- `lib meta::knowledge_delta::index::tests`: 7 passed, covering the freshness classifier and the status probe.
- `tests/knowledge_delta_brief.rs`: 17 passed.
- `tests/knowledge_delta_discovery.rs`: 10 passed.
- `tests/mcp_tool_contract.rs`: 22 passed.
- `tests/static_guards.rs`: 67 passed.
- `cargo clippy --locked --all-targets --all-features -- -D warnings`: clean.

### One pre-existing assertion changed

The former empty-index test expected `no_matching_index_hits` for a brand-new database.
That is exactly the ambiguity this milestone removes, so the test now covers the unseeded
case as `index_not_seeded`, and a separate seeded-but-pre-cutoff record preserves
`no_matching_index_hits`. No other M004 assertion was weakened or removed.

## Known limitations and residual risk

1. No read-only CLI diagnostic was added. The plan allowed one "if simpler documentation and
   existing response diagnostics do not suffice"; the response fields, the disabled warning,
   and the two-call documentation were sufficient, and a new command would have widened the
   CLI surface and its docs contract. Ownership under `architecture/commands.md` was
   therefore not exercised.
2. `retain` while disabled is still rejected by the MCP tool's validation before the runner
   runs, so that path returns a validation error rather than a coverage object. The runner
   path is also covered (`disabled` with no write) for direct library callers.
3. Retention expiry only deletes during `retain`. A stale index is therefore observable
   through `index_stale` before it is rewritten, which is the intended diagnostic; a lookup
   never deletes rows.
4. The freshness probe is partition-scoped while expiry is global. A partition can report
   `Fresh` while another partition's rows are expiring; this matches what the caller can see
   and does not affect isolation.
5. `index_not_seeded` describes the partition, not the file: an enabled index with documents
   under another partition still reports `index_not_seeded` for an empty one.

## Compatibility and migration

Additive only: `index_status` gains the value `disabled`, and `empty_state_reason` gains
`index_not_seeded` and `index_stale`. Existing values, the schema, the storage format, the
permission rules, and the live-call behavior are unchanged. A consumer that treated
`offline` as "everything not open" must now read `disabled` too; the tool matrix and
architecture docs say so.

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
rather than omitted. The determinism gap they exposed is registered as M043 in `plans/archive/implementation/agentic-retrieval-integrity/043-local-repo-search-empty-result-determinism.md`.

## Recommendation

Accept M006 on `e817dfde35372aa307f5580a144d7ededcb55574`. An operator can now tell off from
broken, empty from expired, and seeded from matching, without a new tool or a default change.

This acceptance includes a green hosted CI run on the third attempt; the two earlier macOS failures on an unrelated environment-sensitive test are recorded above rather than omitted.
