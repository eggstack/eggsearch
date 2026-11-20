# Knowledge-Delta Coverage and Quality — Corrective Addendum

Status: proposed
Draft: yes — proposed M005–M007; M001–M004 remain closed
Date: 2026-11-20
Planning baseline: `16b4c2888206caeb6654ffebaa039862fc143712` on `feat/auto-20261117-044c4885`; reconciled after M004 closed on `380a55cd7ea7c0b37dc6eff53851926e96e894d0`
Owner: model-cutoff metadata, knowledge-index operability, and evidence-quality evaluation

Controlling documents: `plans/000-long-term-specification.md` §§1–4, 7–9; `plans/001-terminology-and-domain-model.md` §§1–3; `plans/002-long-term-roadmap.md`; `plans/003-planning-process.md`; accepted ADR-0006, ADR-0020, ADR-0021.

Predecessor (closed, not reopened): `plans/subsystems/knowledge-delta-roadmap.md` M001–M004; `plans/archive/implementation/knowledge-delta/001-cutoff-aware-knowledge-brief.md`, `002-local-evidence-index.md`, `003-source-driven-topic-discovery.md`, plus the corrective `plans/implementation/knowledge-delta/004-brief-delta-production-and-honest-diagnostics.md`; exact candidates `c0f71909cfe2e6016512423e55b9d00fdb7220c5` (M001–M003) and `380a55cd7ea7c0b37dc6eff53851926e96e894d0` (M004); closure evidence `plans/closure/knowledge-delta/001-status.md` through `004-status.md`.

## 1. Purpose and corrective classification

The first implementation met its stated acceptance criteria, but its closure records retained limitations in *catalog breadth*, *practical adoption of deliberately opt-in persistence*, and *demonstrated retrieval quality*. These are follow-on omissions in capability/operability qualification, not evidence that M001–M003 were incorrectly closed. Keep their immutable closure history intact. Register new milestones with regression/evaluation evidence that would have exposed these gaps earlier.

The original workstream owns bounded evidence retrieval and the index implementation. This addendum owns only maintaining a useful verified model-cutoff snapshot, making approved opt-in retention usable and inspectable, and measuring/improving **source-backed** discovery under unchanged trust and resource limits. It must not take ownership of general model inference, external crawling, provider transport, or CodeGG orchestration.

## 2. Observed baseline

- `src/meta/knowledge_delta/snapshot.rs` ships exactly **one** verified snapshot row (`gpt-6-astra`). `src/core/knowledge_delta.rs` supplies cutoff/date parsing and resolution. M001 tests verify correctness for known/unknown cutoffs, not breadth or model-ecosystem maintenance. No automatic vendor metadata sync exists.
- `[knowledge_delta.index].enabled` defaults to `false` per accepted ADR-0021, and `knowledge_brief` defaults to `refresh: "live"`, which neither opens nor writes SQLite. Explicit `retain: true` is required to persist selected public evidence. Index-only and related-topic behavior depend on an already seeded index. M002 exercised persistence, privacy and recovery, but not operator success at first-run setup. M004 added `coverage.index_status`, `coverage.empty_state_reason` (including `no_matching_index_hits`) and a permission rejection that names the rejected directory and its mode; the cold-start enable/seed/inspect workflow and the disabled-versus-never-seeded distinction remain unaddressed.
- `src/meta/knowledge_delta/discovery.rs` extracts bounded subject relationships; `src/meta/knowledge_delta/index.rs` expands one indexed step. M003 closure proves a synthetic omitted-name fixture with negative controls, but explicitly states no live precision/recall estimate. `src/meta/knowledge_delta/plan.rs` emits one unchanged query plus bounded English hints (`release`, `compatibility`, `deprecation`). There is no general crawler or LLM loop **by design**, not as a missing implementation.

## 3. Invariants and non-goals

**Invariant:** the ten-tool canonical MCP contract and existing fetch aliases, adapter-only search, bounded authorized fetch, date precision/basis semantics, stable `src_` and `kdelta_` IDs, deterministic ranking/tie order, external-untrusted evidence framing, structured provider failures, explicit no-evidence uncertainty, and keyless/network-free default tests remain intact.

**Invariant:** provider-official metadata requires a verifiable official publication. Secondary catalogs must be labelled and may disagree. Missing/ambiguous/stale cutoffs remain `unknown`; a date after a cutoff never proves what the model actually knows.

**Invariant:** `live` remains read-only and index-disabled by default. No automatic persistence, external prompt logging, new credential collection, private/profile-scoped archive, or implicit consent. Public-only cacheable evidence is the only retained class.

**Non-goals:** switch the index on globally, introduce a daemon/feed subscription table, recurse into links or new hosts, autonomous browser actions, a model-in-the-loop researcher, embedding/vector dependency, scraping authenticated vendor pages, a new MCP tool, or a second release binary. A changed default or new feed/crawl protocol would require its own proposed ADR and explicit decision, not silent scope expansion here.

## 4. Milestone decomposition and dependencies

| Milestone | Primary class | Status | Handoff | Dependencies | User-visible exit |
|---|---|---|---|---|---|
| M005 — Verified model-cutoff coverage | capability | proposed | `plans/implementation/knowledge-delta/005-verified-model-cutoff-coverage.md` | Hard: closed M001 and accepted ADR-0020; all satisfied. No unresolved architectural decision. | Priority model IDs resolve to sourced, precision-preserving verified cutoffs or explicit, auditable unknowns, with more than the existing single verified record. |
| M006 — Opt-in index setup and seeding | capability | proposed | `plans/implementation/knowledge-delta/006-opt-in-index-onboarding-and-seeding.md` | Hard: closed M002/M003 and accepted ADR-0021; all satisfied. Independent of M005. | Operator can explicitly enable index, retain approved evidence, inspect state, restart and retrieve related-topic hits with no implicit persistence. |
| M007 — Live discovery quality qualification | capability | proposed | `plans/implementation/knowledge-delta/007-live-discovery-quality-qualification.md` | Hard: closed M001–M003; satisfied. Interface: existing live/index/related schema. Soft: M005 catalog and M006 setup for realistic evaluations. | Quantified precision/recall, dating and cost on held-out and live cases; any change to ranking/expansion has measured gain and guard-bounded regression. |

M005 and M006 may be implemented in parallel; M007 may build its offline corpus in parallel but its final live qualification should use the candidate behavior after M005/M006 changes, or clearly record its own exact candidate and the missing integration. All remain **proposed** until assigned; none is implicitly active merely because hard prerequisites are closed.

## 5. Cross-cutting requirements

**Storage and migrations.** M005 changes the bundled *cutoff* snapshot, not the SQLite event schema. M006 uses ADR-0021's existing index and privacy protections; a schema change needs explicit migration/fault-injection evidence. M007 must not write to the index absent the established `retain` request.

**Protocol/compatibility.** No new MCP operation or change in existing tool names. Metadata additions are additive; any change to output ordering, cutoff resolution, `refresh` values, `next_actions` or `retain` requires documented compatibility tests and a decision review.

**Security and failure semantics.** Validate and bound all catalog inputs and external source metadata. No untrusted document or model-catalog field becomes an instruction or new network destination. Failed source verification is `unknown`; offline/empty index stays visibly stale; failed providers contribute an attempt record rather than silent coverage.

**Operations/performance.** Publish catalog provenance and freshness, read-only index diagnostics, and measured time/bytes/providers/RSS. Keep Rust 1.89, bundled SQLite, and default release targets; no required runtime metadata API or network call on startup.

**Observability.** Report verified-versus-unknown catalog coverage, retrieval hit provenance and temporal basis, index-enabled/empty/retained state, and M007 source-backed novel-hit yield. Measurements have explicit denominator, sampling date, and exact SHA.

**Documentation.** Update `architecture/research.md`, `architecture/evidence-workflow.md`, `architecture/config.md`, `architecture/codegg-contract.md`, `docs/config.md`, `docs/agent-workflows.md`, `docs/test-inventory.md` and the model/catalog/index operator documentation as each milestone requires. Keep source-of-truth tests and SDK/Python parity in sync if contracts change.

## 6. Evaluation strategy and risks

M005 uses curated, timestamped official/secondary records with unknown controls and conflict fixtures; release a reproducible source manifest. M006 uses a clean install, public-source seeded index and restart, plus refusal/failure cases. M007 compares equal-budget `knowledge_brief` direct/related and `research_search` where comparable, under withheld-subject and no-change controls. It includes a separately labelled live study, not just synthetic examples.

Risks include catalog terms and stale metadata, alias confusion across model editions, stateful SQLite privacy, cold-start zero-recall when index is empty, authority bias in source ranking, uncertain event dates, and a quality metric that rewards novelty without validity. Never claim measurable improvement without an independently adjudicated denominator and executable fixture manifest.

## 7. Completion and status

These corrective milestones close independently on an exact implementation SHA with the focused and broad tests in their handoffs, doc/SDK impacts, security and failure evidence, and `plans/closure/knowledge-delta/005-status.md` through `007-status.md`. Update this addendum and `plans/registry.md` on each closure; do not modify predecessor closure records. A missing or non-reproducible live evaluation keeps M007 open rather than converting the fixture into a production recall claim.
