# M006 — Opt-In Index Onboarding, Diagnostics and Safe Seeding

Status: proposed
Draft: yes — corrective plan, implementation not started
Date: 2026-11-20
Subsystem: knowledge-delta
Milestone class: capability
Planning baseline: `16b4c2888206caeb6654ffebaa039862fc143712` on `feat/auto-20261117-044c4885`
Source addendum: `plans/subsystems/knowledge-delta-corrective-addendum.md` M006
Closed predecessors: M002/M003 (`plans/archive/implementation/knowledge-delta/002-local-evidence-index.md`, `003-source-driven-topic-discovery.md`), `plans/closure/knowledge-delta/002-status.md`, `003-status.md`; candidate `c0f71909cfe2e6016512423e55b9d00fdb7220c5`.
Authority: accepted ADR-0021 (opt-in, public-only retained evidence, no implicit write or feed crawler); `plans/000-long-term-specification.md` §§3–4, 7–9; `plans/003-planning-process.md` §7.

## Corrective finding and why earlier gates missed it

The index is correctly disabled by default. A plain `knowledge_brief` call uses live retrieval but does **not** write to SQLite; `retain: true` requires explicit enablement, and `related_topics` can only use previously retained evidence. M002 verified secure storage and restart; M003 verified a seeded synthetic graph. Neither demonstrated an operator's *cold-start, explicit-enable, initial-seed, restart, lookup* workflow, so an apparently functional index can remain empty indefinitely without a clear remedy. This is an onboarding/observability gap, **not** grounds to enable persistence by default.

## Objective, readiness and non-goals

Make the index's consent model and empty/ready/stale states immediately understandable and demonstrate successful bounded evidence retention and related-topic reuse with an ordinary end-to-end procedure. Keep `live` strictly no-read/no-write.

**Hard dependencies:** M002 and M003 closed, ADR-0021 accepted — satisfied. **Interface:** existing `KnowledgeBriefArgs` fields `retain`, `refresh` (`live | prefer_live | index`), `related_topics`, and `[knowledge_delta.index]`. Independent of M005.

In scope: a documented minimal configuration and example calls; precise diagnostic state/repair messages; an optional read-only local index status/doctor command if simpler documentation and existing response diagnostics do not suffice; a deterministic seeding/restart integration test; security guardrails for zero-write and local permissions.

Out of scope: setting `enabled = true` as a default, writing on `live` without `retain`, a new MCP tool, a feed-subscription table, crawler, scheduled refresh daemon, automatic fetch across new hosts, importing private/profile/no-store bodies, or changing ADR-0021's path/partition/privacy contracts.

## Ordered work packages

1. **Operator experience audit.** On a fresh config, exercise `knowledge_brief` with omitted `refresh`, `retain: true` while disabled, index-only lookup while disabled, explicitly enabled index with no documents, and an initial explicit retain. Record current errors and telemetry: `index_status`, `freshness_unverified`, `fetch_attempted`, `fetch_completed`. Identify misleading states rather than renaming schemas for aesthetics.
2. **Minimal safe configuration and examples.** Document exact `[knowledge_delta.index]` TOML with `enabled = true` and existing bounds. Show two distinct caller operations: (a) `knowledge_brief` `refresh: "live", retain: true` on a public topic with the normal fetch gate; (b) later `refresh: "index", related_topics: true` or `prefer_live`. Show what a *successful* retain looks like, what an empty result means, how to disable/delete the index, and why source dates remain only evidence dates.
3. **Explicit diagnostics.** Ensure a disabled, empty, no-cacheable-source, stale, or unreadable index yields actionable but non-sensitive diagnostics; differentiate index not opened from index opened-but-empty and from "no eligible documents retained". If needed, add a bounded read-only `eggsearch` CLI diagnostic, not a mutating refresh command; assess CLI ownership under `architecture/commands.md`. Never emit raw excerpts, full database paths or user topics in logs by default.
4. **End-to-end seed and reuse.** Write an offline integration fixture with a public cacheable HTTP response, explicit `retain`, SQLite reopen after process/state restart, lexical recall, and a source-backed neighbor reached by `related_topics`. Preserve `kdelta_`, `src_`, original external-untrusted framing, date precision and partition isolation. Prove that `live` without `retain` does not create a directory/file, and disabled retain fails before any write.
5. **Failure/repair matrix.** Test missing source date, provider failure, disallowed body (`no-store`, `private`, pass-cookie or browser profile), corrupt/locked/read-only/full index, deleted index, and migration refusal. Explain "no eligible evidence" without implying no changes occurred. Avoid on-error clearing, chmod widening, or rebuilding source data silently.
6. **Documentation and ownership.** Update `docs/config.md`, `docs/agent-workflows.md`, `docs/codegg-integration.md` as relevant, `architecture/research.md`, `architecture/evidence-workflow.md`, `architecture/config.md`, `architecture/testing.md` and `docs/test-inventory.md`. Add a short cookbook for CodeGG/eggui consumers to pass explicit `retain` and model ID.

## Verification

Focused, exact-candidate commands:
```sh
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --test knowledge_delta_index
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --features mock --test knowledge_delta_brief
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --features mock --test knowledge_delta_discovery
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --features mock --test fetch_safety
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --features mock --test static_guards
RUSTUP_TOOLCHAIN=1.89.0 make check
RUSTUP_TOOLCHAIN=1.89.0 make docs-check
```
Extend `tests/knowledge_delta_index.rs`, `tests/knowledge_delta_brief.rs`, `tests/knowledge_delta_discovery.rs` with the actual cold-start/seed/reopen regression and failure cases; no live network in standard test gate. If a CLI command is added, also test its read-only behavior and argument/help/docs parity.

## Acceptance and stop conditions

- A clean operator can explicitly enable, populate and inspect the local index using documented commands and public-only material; after restart, a related-topic query returns provenance-backed evidence from the retained corpus.
- Running the documented ordinary live workflow without consent still creates **zero** index files/writes; disabled index remains the default and a disabled `retain` fails with a usable message. Error cases distinguish not-seeded, unsupported/undated, blocked content, stale and unavailable index without reporting false freshness.
- Strict file permissions, partition isolation, retention caps, redaction and schema compatibility are unchanged; no new background work or host expansion.
- Exact-candidate focused/broad gates and user-facing docs/SDK examples pass.

**Stop:** implementation would change automatic retention behavior, discard blocked content, weaken no-store/profile rules, make source-dependent initialization mandatory, or add an unauthorized feed/refresh service. If a read-only diagnostics route requires new authorization, propose a separate decision.

## Closure evidence

`plans/closure/knowledge-delta/006-status.md`: steps from empty config through explicit retain/restart/retrieval, status/warning samples, zero-write and rejection tests, security and migration results, docs/consumer contract diff, code and candidate SHA, test outputs and residual gaps. Update addendum and registry; leave M002/M003 closure evidence untouched.
