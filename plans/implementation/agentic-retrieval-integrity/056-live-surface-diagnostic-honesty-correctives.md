# M056 — Live-Surface Diagnostic Honesty and Keyless-Tier Degradation Correctives

Status: closed
Closure: `plans/closure/agentic-retrieval-integrity/056-status.md`
Date: 2026-11-27
Subsystem: agentic retrieval integrity
Milestone class: invariant (corrective program)
Baseline: commit `7485595` (`feat/auto-20261126-82ca75e6`, debug build 0.4.5, macOS arm64)
Corrects: M051 (`plans/implementation/agentic-retrieval-integrity/051-live-surface-reliability-correctives.md`), M054 (`plans/implementation/agentic-retrieval-integrity/054-agent-facing-payload-economy-correctives.md`), M055 (`plans/implementation/agentic-retrieval-integrity/055-fetch-extraction-completeness-correctives.md`)
Closure of record for the corrected surface: `plans/closure/agentic-retrieval-integrity/051-status.md`, `plans/closure/agentic-retrieval-integrity/052-status.md`
Trial evidence: live loopback Streamable HTTP MCP sessions against the worktree debug binary on 2026-11-27 (nine canonical tools, `tools/list` fingerprint-verified after rebuilding from the worktree; an earlier session ran a stale 0.4.4 binary from PATH and is excluded from evidence). Nineteen `tools/call` invocations covering `provider_status` (with and without `probe: true`), `web_search` (compact and diagnostic), `research_search`, `security_search`, `knowledge_brief` (bare topic, `cutoff`-as-string, and recent-date variants), `fetch` (web focused, repo file, mixed batch, PDF sparse and dense, `render` string/object/invalid, metadata-only, site inventory), `repo_map`, `repo_search`, `build_evidence_bundle`, legacy `web_fetch` alias, error probes (missing query, unknown tool), and offset pagination. Artifacts under `/tmp/egg-live/` on the trial host (not committed).

Companion plans: M054 owns payload economy and error taxonomy; M055 owns extraction completeness and render-path status. This plan owns diagnostic honesty: warning vocabulary for degraded states the trial proved are currently silent or misleading, plus the keyless-tier degradation runbook the trial proved is missing.

Decision authority: ADR-0022 (agent-facing contract), ADR-0006 (keyless HTML scraping first-class; upstream markup drift is maintenance, never API fallback), ADR-0009 (bounded proof-of-work solving only), ADR-0020 (cutoff-aware knowledge brief; bundled catalog is the trust root), AGENTS.md § Conventions (bounded I/O, content-derived IDs, sanitize untrusted text), `plans/003-planning-process.md` §7 (a corrective pass is a new plan).

## Objective

Close five diagnostic-honesty gaps the 2026-11-27 live trial measured, without changing tool inventory, aliases, accepted arguments, provider set, default provider set, trust framing, or payload budgets:

1. A caller passing a bare `cutoff` string to `knowledge_brief` gets silent success with `no_cutoff_resolved`, because serde drops the unknown field. The accepted shape is `cutoff_override` (`date`, `precision`, `source`, `reason`).
2. A `knowledge_brief` call whose cutoff resolves but no routable provider advertises `result_timestamps` degrades to undated coverage with no warning naming the cause.
3. The `transport` response field reads as the fallback that engaged; it names the pipeline that produced the response. `transport: browser` with `browser_escalated: false` is correct (browser was primary, not an escalation) but misread on sight, including by this trial.
4. A reserved non-resolvable hostname (`.invalid`) fails with `private_network_blocked: private network access is disabled`, sending hosts toward firewall policy instead of the real cause: the name can never resolve.
5. Keyless-tier degradation (duckduckgo challenge-cooldown, startpage timeouts, solver wall-clock exhaustion) is diagnosable only by archaeology across probe output; no runbook orders the response (probe, cooldowns, fixture refresh per ADR-0006, `[search.challenge]` tuning).

Non-goals: no new providers or provider defaults; no scraper parser changes (drift maintenance stays under ADR-0006 and the engine conformance suite); no solver algorithm or budget-default changes (ADR-0009/ADR-0010 own the ceilings; operators already tune `[search.challenge]`); no cutoff trust-model change (ADR-0020 stands: `model_id` is a catalog lookup key, overrides rejecting `provider-official` labels stand, see Evidence E0); no schema-size work (M054 WP3 owns it); no extraction work (M055 owns it).

## Evidence

Measured on the worktree debug binary (0.4.5) over loopback Streamable HTTP with keyless defaults, 2026-11-27. Earlier stale-binary session excluded (see Stop conditions).

| Finding | Measured evidence | Source | Why prior verification missed it |
|---|---|---|---|
| E0 (context) Cutoff trust model is sound; no change required. | `KnowledgeBriefArgs` takes `model_id` plus structured `cutoff_override` (`src/mcp/tools/knowledge_brief.rs:20-47`); `resolve_cutoff()` precedence is caller override > operator config > bundled snapshot tiers (`provider_official` > `models_dev` > `openrouter`) > `unknown` (`src/core/knowledge_delta.rs:513-557`); `parse_caller_override` rejects `provider-official`/`official` source labels (`src/core/knowledge_delta.rs:649-654`); the bundled dataset is `src/meta/knowledge_delta/catalog.json` (16 entries, verified 2026-11-20) with only `verified` rows compiled into `snapshot.rs`; unrecognized models resolve to `unknown`, never a guessed date. Maintenance procedure: `docs/knowledge-delta-cutoffs.md`. | code + dataset | The trial initially misattributed `no_cutoff_resolved` to a product defect; it was a caller-shape error (bare `cutoff` string), which is itself finding E1. |
| E1 (P2) Unknown `knowledge_brief` fields drop silently. | `{"topic": ..., "cutoff": "2025-06-01"}` returned success with `empty_state_reason: no_cutoff_resolved`; re-runs without `cutoff` behaved identically. No `deny_unknown_fields` exists anywhere in `src/mcp/tools/`. | live `tools/call` ids 23/43 + `src/mcp/tools/knowledge_brief.rs` | Contract tests pin accepted shapes, not rejected ones; no negative test passes a near-miss field name. |
| E2 (P2) Resolved cutoff with zero timestamp-capable providers warns nothing. | All brief calls returned `timestamp_capable_providers: []` (keyless set advertises no `result_timestamps`) with `no_cutoff_resolved`; when a cutoff does resolve, coverage degrades to undated with no cause-naming warning. `execute.rs:202` computes the provider list; `execute.rs:401` emits `freshness_unverified` only for index/offline refreshes. | live `tools/call` ids 40-43 + `src/meta/knowledge_delta/execute.rs:202,401` | The keyless trial path never has timestamp-capable providers, so the silent-degradation branch is the only branch and was never contrasted. |
| E3 (P3) `transport` field misreads as escalation state. | `render: browser` on a plain page returned `transport: browser` with `browser_escalated: false` and no browser activity needed; the trial read this as contradictory before `docs/tool-matrix.md:191` clarified that `transport` names the producing pipeline. Field-level schema description is terse. | live `tools/call` id 58 + `docs/tool-matrix.md:191` | Documented at the matrix level, not at the field level where hosts read it. |
| E4 (P3) Reserved hostnames report as policy blocks. | `https://nonexistent-domain-xyz12345.invalid/page` returned `private_network_blocked: private network access is disabled`. Root cause: `is_private_hostname` classifies `.invalid` (RFC 2606, never resolvable) as private (`src/fetch/limits.rs:444-459`), so the pre-DNS policy gate rejects before resolution is attempted. Fail-closed is correct; the message misdirects. | live `tools/call` id 56 + `src/fetch/limits.rs:300-304,381-385,444-459` | Tests pin the current message (`limits.rs:553-601`); no test contrasts reserved-TLD vs private-network causes. |
| E5 (P2) Keyless degradation has no runbook. | `provider_status` probe showed duckduckgo in cooldown (7 consecutive failures, `#challenge-form` block), startpage 4 consecutive timeouts, only brave healthy; challenge solver 5 attempted / 0 succeeded / 5 wall-clock exhausted; one `knowledge_brief` call transiently failed `upstream_failed` when all live providers flaked simultaneously (non-reproducible on retry). All signals exist in probe output; no doc orders probe > cooldowns > fixture refresh (ADR-0006) > `[search.challenge]` tuning (`docs/config.md:292-323`, `timeout_ms` 10000 default, difficulty-6 needs >=8000). | live `tools/call` id 60 probe + server lifecycle counters | Prior trials recorded provider health as evidence, never as an operator procedure. |
| E6 (test-hygiene, no product change) Stale PATH binary shadowed the worktree build. | First session advertised 10 tools (aliases `web_fetch`/`batch_fetch`/`repo_fetch`, no canonical `fetch`) from `~/.cargo/bin/eggsearch` 0.4.4; `/healthz` reported `0.4.4` vs worktree `0.4.5`. Rebuilt-server session advertised the correct 9. | `/tmp/egg-live/server.log`, `/healthz` | Trial procedure did not pin the binary under test. |

## Invariants that cannot regress

- ADR-0020 trust hierarchy: `model_id` stays a catalog lookup key; overrides keep rejecting `provider-official` labels; unrecognized models stay `unknown`, never guessed. (E0 is context, not a work package.)
- ADR-0006: scrapers stay first-class; no API-fallback proposal enters this plan.
- ADR-0009/ADR-0010: solver ceilings and defaults unchanged; operator tuning only.
- E4 fix stays fail-closed: reserved hostnames remain rejected pre-DNS; only the error kind/message changes.
- No tool, alias, argument, provider, default-set, trust-framing, or fingerprint change beyond what the packages below state. Any fingerprint-affecting change (WP1 option A) is declared before implementation.
- `make check` stays green throughout; `git diff --check` clean per commit.

## Work packages

### WP0 — Trial-hygiene runbook note (P3, S)

Record the stale-binary lesson so the next live trial does not repeat it: assert `/healthz` version and `tools/list` count against the candidate under test before recording evidence. One paragraph in the trial-evidence section of this plan's closure record (or the next trial plan), no production change.

Acceptance: closure record states the binary assertion performed.

### WP1 — Near-miss field repair hint for `knowledge_brief` (P2, S)

When `knowledge_brief` arguments contain an unknown field shaped like a cutoff (`cutoff`, `knowledge_cutoff`, `cutoff_date`), reject with the existing repairable `invalid_semantic_value` code naming `cutoff_override` and its four required subfields. Two implementation options, decided at implementation time with the reason recorded: (A) `#[serde(deny_unknown_fields)]` on `KnowledgeBriefArgs` (stricter; verify no contract test breaks and whether the advertised fingerprint re-pins, declaring it here if so); (B) a targeted pre-deserialization scan for the known near-miss names returning `ToolError::invalid_semantic_value`. Option B matches the existing repairable-error taxonomy; option A is stronger if fingerprint-stable.

Acceptance: bare-`cutoff` call fails closed with a repair naming `cutoff_override`; valid calls unchanged; `make eval-tool-surface` unaffected.

Tests: extend the `knowledge_brief` behavioral coverage (currently exercised via `tests/knowledge_delta_brief.rs` and Eval corpus) with a near-miss rejection case; if option A, update `tests/mcp_tool_contract.rs` / `tests/docs_tool_names.rs` expectations only as the guards require.

### WP2 — `cutoff_unverifiable` warning when no timestamp-capable provider is routable (P2, S)

In `run_knowledge_brief`, when a cutoff resolves and `timestamp_capable_providers` is empty, emit a typed warning naming the cause (e.g. `cutoff_unverifiable_no_timestamp_providers`: cutoff resolved to `<date>` but no selected provider advertises `result_timestamps`, so dated comparison is unverified). Both values are already computed (`execute.rs:198-211`, coverage block ~`execute.rs:395-413`). Warning payload states the resolved date, precision, and origin; it never claims the cutoff is wrong.

Acceptance: keyless brief with a resolving `model_id` (e.g. `gpt-6-luna`) emits the warning; a run with a timestamp-capable provider does not; warning code documented in `docs/tool-matrix.md` `knowledge_brief` row.

Tests: unit coverage in the knowledge-delta execute tests plus one MCP-level assertion on the warning code; `docs/test-inventory.md` updated only if a suite is added.

### WP3 — `transport` field semantics at the field level (P3, XS)

Docs-only. Extend the `transport` field description (fetch response schema docs and `docs/tool-matrix.md` fetch row) with one sentence: names the pipeline that produced the response (`http` or `browser`); `browser_escalated` alone records whether `auto` fell back after HTTP. No code, no fingerprint change.

Acceptance: a host reading only the field description cannot misread E3.

### WP4 — Reserved-hostname error distinct from policy block (P3, S)

In `src/fetch/limits.rs`, distinguish reserved non-resolvable hostnames (`.invalid`, and any sibling reserved suffix already listed in `is_private_hostname`) from private-network policy blocks: same fail-closed rejection pre-DNS, but a distinct `FetchErrorKind` (or distinct message under the existing kind if the taxonomy guard prefers it — check `tests/static_guards.rs` and the `FetchErrorKind` code-derived inventories first) stating the name is reserved and can never resolve. Update the message-pinning tests in `limits.rs:553-601` and any behavioral suite asserting the old text for `.invalid` hosts.

Acceptance: `.invalid` host fails closed with the reserved-name message; private-network blocks unchanged; redirect-target validation (`limits.rs:381-385`) shares the distinction.

Tests: extend `src/fetch/limits.rs` unit tests contrasting `.invalid` vs `.internal`/`.local` causes; one MCP-level fetch assertion on the new message.

### WP5 — Keyless-tier degradation runbook (P2, S)

Docs-only. Add a short runbook to `docs/provider-setup.md` (with a pointer from `architecture/engines.md` § Scraper Fidelity): when keyless results degrade, run `provider_status` with `probe: true`; read consecutive-failure counts, `cooldown_until`, and solver `wall clock exhausted` counters; refresh captured fixtures under `tests/fixtures/engines/<provider>/` per the conformance suite (`tests/engine_parse_conformance.rs`) if markup drifted; tune `[search.challenge]` (`timeout_ms` toward the 30000 bound, `max_difficulty` only with matching `max_attempts`) using `live-release-smoke` timing, never debug-build timing. Explicitly restate: no API fallback (ADR-0006), no interactive/OCR/service solving (ADR-0009).

Acceptance: a reader with only E5's probe output can execute the runbook without archaeology.

## Verification

```bash
make check
cargo test --locked --features mock --test knowledge_delta_brief
cargo test --locked --all-features --test web_fetch_integration
cargo test --locked --all-features --test mcp_tools
```

Plus a manual limited live re-trial (not CI) on the probes in E1-E5: bare-`cutoff` rejection text, warning emission with a resolving `model_id`, `.invalid` message, and a `probe: true` baseline, recording response text and warning codes.

## Stop conditions

- Any package requiring a tool, alias, argument, provider, default-set, or trust-framing change beyond WP1's declared option: stop and split into a separate registered milestone.
- WP1 option A re-pinning the advertised fingerprint: declare the re-pin in this plan's closure record with old/new values; if the re-pin exceeds a description-only delta, stop and split.
- WP4 taxonomy change tripping a code-derived inventory guard (`tests/docs_tool_names.rs`, `tests/docs_provider_inventory.rs`, `tests/static_guards.rs`): follow the guard's ownership table; if the guard forbids the new kind name, stop and use the message-only variant.
- Evidence of actual upstream markup drift during the re-trial: out of scope here; file under the engines conformance path (ADR-0006), do not fold parser changes into this plan.
- Solver timing questions during the re-trial: operator config only; no code ceiling changes in this plan.

## Closure evidence required

Per-WP acceptance outputs; the WP1 option decision with its reason; the new warning code(s) in `docs/tool-matrix.md` (and `architecture/engines.md` only for the runbook pointer); `CHANGELOG.md` entry; `docs/test-inventory.md` and `architecture/testing.md` updated if a suite is added; `plans/registry.md` and the roadmap status updated in the same closure commit.

## Appendix — trust-model confirmation (no change)

E0 records the 2026-11-27 verification that `knowledge_brief` does not trust an LLM's self-reported cutoff: `model_id` resolves exclusively against the bundled `catalog.json` dataset (16 entries, `verified_at: 2026-11-20`) with caller-override > operator-config > `provider_official` > `models_dev` > `openrouter` > `unknown` precedence; caller overrides must carry date/precision/source/reason and are rejected when labeled `provider-official`; unrecognized models resolve to `unknown`. Maintenance stays under `docs/knowledge-delta-cutoffs.md`. This appendix is evidence for why no trust-model work package exists in this plan.