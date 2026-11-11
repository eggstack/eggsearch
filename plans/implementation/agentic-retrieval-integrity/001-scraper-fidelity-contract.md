# M001 — Scraper Fidelity Contract

Status: closed
Source: `plans/subsystems/agentic-retrieval-integrity-roadmap.md` M001
Primary class: invariant (corrective)
Baseline: `cd81f4d5e966364918fe2be25485b8ebbf424e71`
References: ADR-0006; `architecture/engines.md`; `architecture/meta.md` § Dispatch config and error taxonomy; `src/meta/engines/error.rs`; `src/meta/engines/mod.rs`; `src/meta/adapter/error.rs`; `src/meta/provider_diagnostics.rs`; `src/meta/adapter/status.rs`.
Dependencies: none. This milestone does not depend on M002–M005 and must not be blocked by them.

## Objective

Make a scraper's structural break distinguishable from a genuine empty result, end to end, so the retrieval ledger and agent-facing absence semantics stay truthful when upstream markup changes.

## Why this is corrective

At baseline the five HTML scrape engines (`brave`, `duckduckgo`, `mojeek`, `startpage`, `yahoo`) return `Ok(vec![])` when their expected result containers are absent. `EngineError::ParseFailed` is only constructed by the per-engine `sel()` helper for invalid selector syntax, which is a programming error, not drift. Dispatch therefore records `RetrievalAttemptOutcome::SuccessZeroResults`, and `retrieval_status` reports `no_matching_evidence_found`.

The original verification missed this because engine tests build miniature hand-written HTML that always contains the expected containers (`src/meta/engines/duckduckgo.rs` unit tests), no captured provider markup exists, and no test asserts the empty-state path. A zero-result search and a renamed CSS class are indistinguishable to every existing test.

## Non-goals

- No new providers, no replacement of scrapers with APIs, no selector redesign beyond what fixtures prove necessary.
- No CAPTCHA solving, proxy rotation, user-agent spoofing, or retry-policy widening.
- No change to RRF, ranking, tool schemas, or response projections.
- No general HTML-parser replacement or dependency swap.

## Invariants that cannot regress

- Ten MCP tools, stable IDs, sanitization tiers, trust framing, and deadline bounds are unchanged.
- The per-provider attempt ledger keeps every selected provider's terminal outcome; capability skips stay distinct from failures.
- Existing wire strings for `error_class` remain valid; any new class is additive.
- A genuine zero-result search still reports success-with-zero-results. This milestone must not convert real absence into a failure.
- Engine requests keep using `read_bounded_body` and existing caps.

## Expected production changes

1. **Shared scrape-state contract.** Add a small shared helper in `src/meta/engines/mod.rs` that every scraper calls after selection: results present → `Ok(rows)`; no results but a documented empty-state marker matched → `Ok(vec![])`; neither → `Err(EngineError::ParseFailed { engine, reason: "result container missing" })`.
2. **Per-engine sentinels.** Each scraper declares its result container selector and its empty-state markers as constants next to the parse function. Markers must come from captured real markup, not guessed strings.
3. **Block and challenge classification.** Add `EngineError::Blocked { engine, reason }`, `ErrorClass::Blocked`, and the mirrored `FailureClass`. Classify as non-retryable and failure, never absence. Wire it through `classify()`, health recording, and warnings using the existing `[class] message` convention.
4. **Captured fixtures.** `tests/fixtures/engines/<provider>/results.html`, `empty.html`, and `blocked.html` for each of the five scrapers. Redact identifiers, keep each file small, strip cookies and tokens.
5. **Conformance suite.** New `tests/engine_parse_conformance.rs` that is table-driven over the five engines and asserts all three outcomes per engine, including a seeded-drift case where a fixture's result container class is renamed and the parser must return `ParseFailed`.
6. **Diagnostic shape signal.** `provider_status` reports a per-scraper `parse_shape` of `ok`, `empty`, or `shifted`, derived from the most recent classified outcome, so an operator sees drift without reading logs.

## Storage, protocol, and compatibility effects

- `error_class` gains one additive value. `architecture/meta.md` and any taxonomy test must list it; existing values are untouched.
- No storage migration. Fixtures are new tracked files under `tests/fixtures/engines/`; they must satisfy repository hygiene (no transcripts, no ANSI dumps, no build output).
- `provider_status` gains one optional field; `tests/mcp_projection.rs` and `docs/tool-matrix.md` need the additive note if it appears in compact/standard projections.

## Ordered work packages

1. Add captured fixtures for all five scrapers (results, empty, blocked). Verify each fixture actually reproduces the expected state before writing code.
2. Add the shared scrape-state helper and `EngineError::Blocked`; extend the error taxonomy and classification.
3. Convert `duckduckgo`, `startpage`, `yahoo`, `mojeek`, and `brave` HTML parsers to the helper; replace guessed empty strings with fixture-derived markers.
4. Update existing unit tests that assert empty output without a marker (`mojeek` has one); they must either supply the marker or expect `ParseFailed`.
5. Add `tests/engine_parse_conformance.rs` with the seeded-drift negative control.
6. Add `parse_shape` to provider status and its tests.
7. Update `architecture/engines.md` (new scraper fidelity section), `architecture/meta.md` (error taxonomy), `docs/test-inventory.md`, `architecture/testing.md`, and `skills/eggsearch-dev/SKILL.md` placement notes.

## Verification

- `cargo test --locked --features mock --test engine_parse_conformance`
- `cargo test --locked --features mock --test provider_probe_conformance --test provider_routing`
- `cargo test --locked --features mock --test web_search_integration --test keyless_core`
- `cargo test --locked --all-features --test dispatch_fault_injection --test retrieval_attempt_ledger --test provider_capability_contract`
- `make check`; `make docs-check`

## Acceptance and stop conditions

- Every scraper distinguishes results, explicit empty state, and structural break; the conformance suite fails if any engine reverts to bare empty output.
- A renamed container in a fixture yields `ParseFailed` and a `blocked` fixture yields `Blocked`, both surfacing as provider failure rather than absence.
- No existing wire value is removed; `make check` passes.
- Stop if the change requires a new dependency, widens retry behaviour, or alters ranking.

## Closure evidence

`plans/closure/agentic-retrieval-integrity/001-status.md` must record the exact candidate SHA, the five fixtures added, the conformance and ledger test outputs, the observed before/after ledger outcome for a simulated drift case, and confirmation that genuine zero-result searches still report success-with-zero-results.
