# M056 Closure — Live-Surface Diagnostic Honesty and Keyless-Tier Degradation Correctives

Status: closed
Date: 2026-11-27
Candidate: worktree on `7485595` plus the M056 working tree (all five work packages + trial-hygiene note)
Plan: `plans/implementation/agentic-retrieval-integrity/056-live-surface-diagnostic-honesty-correctives.md` (now `active`; this record closes it)
Baseline: commit `7485595` (`feat/auto-20261126-82ca75e6`, debug build 0.4.5, macOS arm64)

## Trial hygiene (WP0)

The live re-trial asserted the binary under test before recording evidence: `/healthz` reported `version 0.4.5` from the worktree `target/debug/eggsearch` build (rebuilt after all code edits), and a fresh `init` + `tools/list` session advertised exactly the 9 canonical tools (`build_evidence_bundle`, `fetch`, `knowledge_brief`, `provider_status`, `repo_map`, `repo_search`, `research_search`, `security_search`, `web_search`) with no legacy aliases. The stale-PATH-binary failure mode from the 2026-11-27 trial (10 tools from `~/.cargo/bin/eggsearch` 0.4.4) did not recur.

## Requirement-to-evidence matrix

| WP | Requirement | Evidence | Result |
|---|---|---|---|
| WP1 | Bare `cutoff` / `knowledge_cutoff` / `cutoff_date` fields fail closed with repairable `invalid_semantic_value` naming `cutoff_override` + four subfields | Option B implemented: `KnowledgeBriefArgs` manual `Deserialize` captures the three exact near-miss keys into a `#[schemars(skip)]` marker; `run_knowledge_brief` rejects first. Live probe id 71: `{"code":"invalid_semantic_value","message":"invalid cutoff_override 'cutoff'; accepted values: date, precision, source, reason","repair":{"accepted":["date","precision","source","reason"],"field":"cutoff_override"}}`, `isError: true`. Integration test `cutoff_near_miss_fields_return_repairable_cutoff_override_error` covers all three keys + valid-override acceptance. Fingerprint unchanged `d7968115e378f708` (`mcp_tool_contract` pinned-surface test passes). | Pass |
| WP2 | Resolved cutoff + empty `timestamp_capable_providers` emits typed cause-naming warning | `cutoff_unverifiable_warning` in `execute.rs` fires only when cutoff is `Some` and the list is empty; states resolved date, precision (`DatePrecision::as_str`), and origin (`CutoffOrigin::as_str`). Live probe id 75 (`gpt-6-luna`): `knowledge_brief_cutoff_unverifiable_no_timestamp_providers: cutoff resolved to 2026-05-18 (precision: day, origin: provider_official) but no selected provider advertises result_timestamps, so the dated comparison is unverified`. Unit test asserts both non-emission guards; integration test asserts presence (mock `duckduckgo`) and absence (timestamp-capable `hn_algolia`). Warning code documented in `docs/tool-matrix.md` `knowledge_brief` row. | Pass |
| WP3 | `transport` field semantics at field level | One sentence added in `docs/tool-matrix.md` fetch row and `docs/features.md` Response Metadata: `transport` names the producing pipeline (`http`/`browser`); `browser_escalated` alone records `auto` fallback. No code, no fingerprint change. | Pass |
| WP4 | Reserved hostnames fail closed with distinct message | Message-only variant (no new `FetchErrorKind`; keeps `PrivateNetworkBlocked`, avoids MCP mapping churn — guard-compatible). Both `validate_url` and `validate_fetch_target_with_resolved_addrs` share `hostname_block_error`/`is_reserved_hostname` (`.invalid`, `.test`, `.localhost` suffixes; bare `localhost`, `.internal`/`.local`, IPs unchanged; `allow_private_network` gate preserved). Live probe id 73: `private_network_blocked: ... reserved hostname can never resolve: nonexistent-domain-xyz12345.invalid`. Unit tests contrast all 9 hostname classes in both paths + allowance; MCP case extended in `web_fetch_integration.rs`. | Pass |
| WP5 | Keyless-tier degradation runbook | `docs/provider-setup.md` gains `### Keyless-tier degradation runbook` (probe-first, `consecutive_failures`/`cooldown_until`/`wall clock exhausted`, fixture refresh per `engine_parse_conformance.rs`, `[search.challenge]` tuning from `live-release-smoke` timing only; restates no API fallback ADR-0006, no interactive/OCR/service solving ADR-0009). `architecture/engines.md` Scraper Fidelity points to it. E5 baseline captured (id 74): duckduckgo/brave/yahoo healthy, startpage 1 consecutive timeout degraded, solver counters all zero. | Pass |

## WP1 option decision

Option B (targeted pre-deserialization scan) chosen over option A (`deny_unknown_fields`): option A cannot produce the required structured `invalid_semantic_value` repair naming the four subfields (it yields a generic serde unknown-field error at the `Parameters` layer before the tool method runs), while option B preserves ordinary unknown-field tolerance and keeps the advertised schema byte-identical. Reason recorded here per the plan's stop conditions. No fingerprint re-pin.

## Verification run

- `make check` (canonical gate, Rust 1.89): green on the integrated tree, including `fmt --check`, clippy, dependency-policy, packaging checks.
- Focused suites: `knowledge_delta_brief` 19/19, `cutoff_warning_tests` 1/1, `fetch::limits::tests` 51/51, `web_fetch_integration` reserved-hostname case, `static_guards` 69/69, docs inventories (`docs_tool_names`, `docs_provider_inventory`, `docs_keyless_contract`, `docs_safety_vocabulary`), `mcp_tool_contract` pinned-surface, clippy `--all-targets --all-features -D warnings`.
- `git diff --check`: clean.
- Live re-trial (loopback Streamable HTTP, keyless defaults, worktree 0.4.5 binary): E1 id 71, E2 id 75, E4 id 73, E5 id 74 as above; E3 verified by doc change (no live probe needed — wording fix).

## Invariants held

- ADR-0020 trust hierarchy unchanged: `model_id` stays a catalog lookup key (E2 resolution shows `provider_official` tier from the bundled dataset, never an LLM self-report); override `provider-official` rejection untouched; unknown models still resolve to `unknown`.
- ADR-0006 / ADR-0009 / ADR-0010: no scraper, solver, or ceiling changes.
- E4 stays fail-closed pre-DNS; private-network blocks byte-identical.
- No tool, alias, argument (beyond WP1's declared reject), provider, default-set, trust-framing, or payload-budget change.

## Known limitations

- The E2 live warning co-occurs with `knowledge_brief_no_dated_sources` when providers return no parseable timestamps; both warnings are independently correct (coverage degraded AND undated).
- New tests are keyless/mock-backed; live-provider timestamp behavior (a timestamp-capable provider suppressing the warning) is asserted at the unit level only.
- `fuzz/Cargo.lock` remains outside the checked dependency-policy graph (pre-existing, unchanged).

## Recommendation

Closed. Registry and roadmap updated to `M056 closed` in the same commit.
