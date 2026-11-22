# M002 Provider Capability Declaration Compaction — Closure Evidence

Status: closed

Candidate: the working tree on `feat/auto-20261117-044c4885` above baseline
`fb9d6538f8ea2e21e08add43fb3f71ebe6ceaa64`, uncommitted because the request did not ask for a
commit. The canonical gate ran on exactly this tree; its output and the diff fingerprint are in
`plans/closure/code-duplication-consolidation/gate.md`.

Source plan: `plans/implementation/code-duplication-consolidation/002-provider-capability-declaration-compaction.md`

Roadmap: `plans/subsystems/code-duplication-consolidation-roadmap.md` M002

Decision authority: no ADR. `tests/provider_capability_contract.rs` remains the capability
claim authority; this milestone changes source shape only.

## Change

- `tests/provider_capability_contract.rs::capability_matrix_matches_the_pinned_table` pins all
  44 provider IDs with a sparse table of true flag names, builds an explicit all-false
  `ProviderCapabilities`, sets the named flags, asserts equality with the descriptor, and
  asserts the table covers exactly `KNOWN_PROVIDER_IDS` once each. There are 38 non-empty rows;
  `searxng` is the one all-false row.
- `src/core/provider.rs`: the 39 full 24-field literals in `built_in_provider_descriptor`
  became `ProviderCapabilities { <true flags>: true, ..ProviderCapabilities::none() }`.
  Override order was preserved, including `arxiv` and `pubmed`.
- `built_in_provider_descriptor`: 1,421 → 763 lines.

## Equivalence evidence

- The pin passed against the unmodified descriptor implementation before any production edit.
- Negative control: removing `brave_api.supports_safe_search` from the table failed with
  `capability mismatch for brave_api` (actual `true`, pinned `false`); the table was restored.
- The pin passed after each of the six family batches.
- Final: `provider_capability_contract` 24 passed, `provider_routing` 99 passed,
  `core::provider` 47 passed.
- No flag, provider id, display name, kind, credential requirement, configured/routable state,
  skip reason, or skip code changed; the equality test is the byte-level flag proof.

## Finding corrected in the same change

`architecture/engines.md` still said `nvd` declared `supports_freshness: true`. The source and
the contract test both say false (the engine maps only `keywordSearch`), so the stale sentence
was corrected to match the tested code. No capability was changed.

## Deferred alternative

A static `const PROVIDERS` table remains available if the provider count makes the match
unwieldy. It was not taken here; the milestone deliberately kept the existing match and renamed
no API.
