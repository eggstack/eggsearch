# M002 — Optional Local Evidence Index — Closure

Status: closed
Recommendation: closed; functional acceptance and the operational Linux/Windows evidence gate pass on the exact candidate. No numeric footprint threshold is claimed.
Implementation candidate: c0f71909cfe2e6016512423e55b9d00fdb7220c5
Baseline: 0f52c8fc812c99e8aee75f515f1b1d8dd32a9789
Plan: plans/archive/implementation/knowledge-delta/002-local-evidence-index.md
Roadmap: plans/subsystems/knowledge-delta-roadmap.md M002
Related: accepted ADR-0020; accepted ADR-0021 (bundled rusqlite, disabled by default).

## Candidate disposition

The pushed implementation candidate is c0f71909cfe2e6016512423e55b9d00fdb7220c5. This record, archive, roadmap, and registry are closure-only changes after it. ADR-0021 is unchanged: bundled SQLite/FTS5 stays in the one default binary; no Cargo feature, second SKU, or system SQLite link was added.

## Acceptance matrix

| Criterion | Evidence on the candidate | Result |
|---|---|---|
| Opt-in FTS index stores selected public evidence; live neither opens nor writes it | tests/knowledge_delta_brief.rs: retain_uses_fetched_public_evidence_and_index_mode_is_offline, live_call_does_not_open_the_index_and_disabled_retain_is_rejected, retain_skips_fetched_evidence_not_marked_cacheable | pass |
| Refuse private, no-store, pass-cookie, and profile-scoped bodies; isolate partitions and files | tests/knowledge_delta_index.rs: refused_body_classes_never_commit_any_part_of_the_batch, precision_survives_restart_and_other_partitions_are_invisible, index_database_and_partition_directory_are_owner_only, explicitly_configured_database_rejects_a_shared_parent | pass |
| Restart, transactions, concurrency, corruption, migration erasure and bounds, and storage limits recover deterministically | tests/knowledge_delta_index.rs: interrupted_sqlite_transaction_rolls_back_on_connection_drop, two_processes_serialize_writes_and_read_the_same_database, corrupt_file_is_refused_and_a_second_connection_can_read, schema_v1_scrubs_query_text_and_rebuilds_source_subjects, schema_v1_migration_bounds_rebuilt_subjects_near_page_limit, max_bytes_returns_full_without_partial_batch_commit | pass |
| Bounded lexical results preserve source/date provenance, stale state, and current-cutoff rechecks | knowledge_delta index, brief, discovery, and property suites | pass |
| Linux and Windows target validation and footprint records exist for the exact candidate | Eggpack run 44745944881; four records below; each has threshold_applied=false | pass as qualification/evidence only |

## Linux and Windows footprint evidence

Each record is for c0f71909cfe2e6016512423e55b9d00fdb7220c5 and probe eggsearch --version. Linux RSS is KiB; Windows working set is bytes. Every source record states threshold_applied=false.

| Target | Stripped binary bytes | RSS / working-set samples | threshold_applied |
|---|---:|---|---|
| x86_64-unknown-linux-gnu | 31,381,560 | RSS KiB: 7,384; 7,428; 7,480; 7,400; 7,600 | false |
| aarch64-unknown-linux-gnu | 27,127,728 | RSS KiB: 6,476; 6,472; 6,460; 6,476; 6,476 | false |
| x86_64-pc-windows-msvc | 37,618,688 | working-set bytes: 2,437,120; 2,433,024; 4,792,320; 2,441,216; 6,754,304 | false |
| aarch64-pc-windows-msvc | 31,638,016 | working-set bytes: 7,417,856; 3,903,488; 3,907,584; 3,903,488; 3,903,488 | false |

The exact-SHA Eggpack build, qualification, and consumer validation jobs for Linux and Windows passed; the workflow aggregate and remaining target validations also passed. This closes the Linux/Windows operational evidence gate. No written numeric budget was applied, and no numeric threshold pass is asserted. The --version probe is not MCP serve startup. Experimental ARMv7 physical evidence remains unrun and non-gating.

## Verification

| Command | Result |
|---|---|
| RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --features mock --test knowledge_delta_brief --test knowledge_delta_discovery | pass |
| RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --features mock --lib ordinary_http_response_with_false_manual_interaction_flag_is_cacheable | pass |
| RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --test knowledge_delta_index | pass |
| RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --test property_knowledge_delta --test property_identity | pass |
| RUSTUP_TOOLCHAIN=1.89.0 make check | pass |
| RUSTUP_TOOLCHAIN=1.89.0 make docs-check | pass |
| make packaging-check | pass |

## Hosted qualification on c0f71909cfe2e6016512423e55b9d00fdb7220c5

| Exact-SHA CI | [44745935736](https://github.com/eggstack/eggsearch/actions/runs/44745935736) | success |
| PR CI | [44745899450](https://github.com/eggstack/eggsearch/actions/runs/44745899450) | success |
| Exact-SHA Python qualification | [44745940949](https://github.com/eggstack/eggsearch/actions/runs/44745940949) | success |
| PR Python qualification | [44745899428](https://github.com/eggstack/eggsearch/actions/runs/44745899428), attempt 2 | success; canceled attempt 1 not counted |
| Eggpack exact-SHA qualification | [44745944881](https://github.com/eggstack/eggsearch/actions/runs/44745944881) | success |
| Release drift | [44745899462](https://github.com/eggstack/eggsearch/actions/runs/44745899462) | success |
| Egress qualification | [44745899759](https://github.com/eggstack/eggsearch/actions/runs/44745899759) | success |

## Residual limits

The index is not a verified-current catalog, and lexical ranking is not confidence. The accepted macOS spike remains a decision record, not a numeric threshold. ARMv7 physical evidence is not claimed.
