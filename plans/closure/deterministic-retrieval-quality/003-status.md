# M003 Keyless Provider Contribution and Resilience — Closure Status

Status: closed

Source plan: `plans/implementation/deterministic-retrieval-quality/003-keyless-provider-contribution-and-resilience.md`

Roadmap: `plans/subsystems/deterministic-retrieval-quality-roadmap.md`

Implementation candidate: `3ef5242c928181b27343a463015c09912dcabbc5`

Report and fixture: `docs/retrieval-quality-provider-contribution.md`, `tests/fixtures/retrieval_quality/provider_contribution.json`

## Outcome

The synthetic provider fixture deterministically covers one `generic_web` task across DuckDuckGo, Startpage and Yahoo. Individual relevant hits are 1/3, 2/3 and 2/3; the combined set returns all 3 relevant fixture documents with two overlapping relevant documents. Brave and Mojeek are marked `not_selected`; SearXNG is `not_configured`. This validates provider-attribution metric arithmetic only. It is not live upstream evidence.

The existing provider tests preserve explicit provider selection, per-provider failures/cooldown, partial results, empty-success versus failure, attempt ledgers and request contracts. Targeted verification passed: 193 cases across dispatch fault, provider routing, probe conformance and web search integration; 36 passed in provider request contracts; the new provider contribution fixture test passed. The all-feature `make check` passed at the M002 candidate before this fixture was added; exact-candidate fixture and provider regression commands passed here.

## Decision and limits

No staged dispatch, provider suppression or production routing code changed. There was no opt-in live provider observation; real provider marginal recall, overlap, latency, timeout behavior and cost remain unknown. That is recorded, rather than inferred from synthetic assignments. The live lane remains optional for the offline closure and can inform integrated M006 comparison if explicitly run.

## Safety and compatibility

No API key requirement, anti-bot bypass, background health database, arbitrary provider omission, response change or default configuration change was introduced. SearXNG-style keyless HTML paths and failure visibility remain as before.

## Verification

- `cargo test --locked --features mock --test retrieval_quality synthetic_provider_contribution_and_overlap_are_reproducible`: passed.
- `cargo test --locked --features mock --test dispatch_fault_injection --test provider_routing --test provider_probe_conformance --test web_search_integration`: 193 passed.
- `cargo test --locked --all-features --test provider_request_contract`: 36 passed.
- Repository gates `make check`, `make bench-check`, and `make docs-check` passed before the fixture change; the exact candidate fixture and provider regression suites passed after the change.
