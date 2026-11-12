# M007 Cross-Result Excerpt Economy — Closure Status

Status: closed

Source plan: `plans/implementation/agentic-retrieval-integrity/007-cross-result-excerpt-economy.md`

Roadmap: `plans/subsystems/agentic-retrieval-integrity-roadmap.md`

Planning baseline: `d751bd7471a647e651ee41b8e51199c66d04d01a`

Implementation candidate and closure commit: the commit that carries this record. Implementation,
closure evidence, roadmap status, and the registry row all moved in that single commit on top of the
planning baseline above.

## Result

One response-wide excerpt budget with deterministic redundancy suppression, applied at the MCP
projection boundary in `src/mcp/excerpt_economy.rs`.

Counting unit: characters of the serialized passage text **including** the
`<<<EXTERNAL_UNTRUSTED …>>>` / `<<<END>>>` framing — the same unit `brief_serialized_chars` and
`brief_max_chars` already use. No tokenizer claim is made.

Configuration: `[mcp].excerpt_budget_chars`, default `4_000`, validated `256..=32_000`, independent
of `brief_max_chars` so the 4,000 default does not conflict with that field's 512 floor.

Composition: `diagnostic` and `standard` are untouched and carry no report. `compact` and `brief`
both apply the pass, with brief's `brief_max_chars` fitting running last so the smaller ceiling wins.

## Suppression rule

Deterministic, inference-free, and first-wins:

1. Strip the trust-frame envelope so `EXTERNAL_UNTRUSTED` / `field=snippet` tokens cannot inflate
   similarity.
2. Lowercase and split on non-alphanumeric characters.
3. Equal normalized text is a duplicate.
4. Otherwise build 4-token shingle sets, fingerprint them with FNV-1a 64, and treat a pair as
   duplicate only when the overlap coefficient `|A∩B| / min(|A|,|B|) >= 0.80` **and** an exact
   shingle comparison confirms it, so a hash collision can never suppress on its own.
5. Passages with fewer than four tokens compare by exact equality only.
6. Greedy against already-admitted passages in response order: the earliest occurrence survives.

Ordering is therefore stable and independent of hash iteration. Suppression never reorders
surviving results, never changes a stable ID, and never drops a whole result — only passage text is
collapsed or clipped.

## Reported fields

Top-level `excerpt_economy` carries `budget_chars`, `considered_count`, `admitted_count`,
`redundant_count`, `over_budget_count`, `clipped_count`, `chars_before`, `chars_after`. A card that
lost passages carries `excerpts_suppressed`, present only when greater than zero, mirroring the
existing `excerpts_truncated`.

The counter identity `admitted_count + redundant_count + over_budget_count == considered_count` is
enforced by test. `considered_count` includes passages beyond the internal `MAX_ECONOMY_PASSAGES`
cap (counted as `over_budget`) so the identity always holds; the cap is observable as
`admitted + redundant <= 256`.

## Measured effect

| Payload | before → after | Result |
|---|---|---|
| duplicate fixture | 220 → 139 chars (−36.8%) | considered 3, admitted 2, redundant 1 |
| distinct fixture | 159 → 159 | 0 suppressed (no false positives) |
| 15-case retrieval corpus, compact | 2,411 → 1,434 chars (−40.5%) | — |
| saturation, 256 passages / 4,000 budget | 159,634 → 4,000 chars | admitted 7 (1 clipped), over_budget 249 |

Compact 4,165 B versus diagnostic 4,807 B on the duplicate fixture.

## Wiring note

The module landed with the config value parsed and validated but inert, because the four search-tool
call sites were outside its ownership boundary. The parent completed that wiring: `web_search`,
`repo_search`, `research_search`, and `security_search` now pass
`ProjectionLimits { brief_max_chars, excerpt_budget_chars }` into `project_with_limits`, so the
budget is live rather than parsed-only. This is recorded because a parsed-but-unused config key is
exactly the kind of defect a closure record should name.

## Verification

- `cargo fmt --all -- --check`: clean.
- `cargo clippy --locked --all-targets --all-features -- -D warnings`: clean.
- `cargo test --locked --all-features --test mcp_projection`: 31 passed (24 → 31).
- `cargo test --locked --all-features --test property_excerpt_economy`: 5 passed (new file).
- `cargo test --locked --all-features --test extract_fetch_contract`: 26 passed (24 → 26).
- `cargo test --locked --all-features --test config_validation`: 86 passed.
- `cargo test --locked --all-features --test evidence_contract`: 12 passed.
- Full `cargo test --locked --all-features --no-fail-fast`: 6,026 passed, 1 failed, 27 ignored across
  89 targets at the time of the run; the failure and both static-guard breaches were concurrent-work
  items outside this milestone and were resolved afterwards.

## Decisions recorded

- `considered_count` counts passages beyond the internal cap so the identity holds; the cap is
  observable through `admitted + redundant <= 256` rather than a separate field.
- Brief drops `excerpt_economy` only as a last resort when `brief_max_chars` cannot otherwise be
  met, and sets `output_truncated` when it does. It is not in the ordinary removable list.
- Single-token differences collapse only once a passage reaches roughly 23 tokens, because
  `(W−4)/W >= 0.80` only then holds. The near-miss negative control passes at advisory scale.

## Residual risk

- The 0.80 overlap threshold is a judgement call. It is applied to already-sanitized text and is
  guarded by an exact-comparison confirmation, so the failure mode is under-suppression, not silent
  deletion.
- Suppression is per response, so two separate calls each pay their own budget and can still repeat
  the same passage across calls.
- The budget measures characters, not model tokens, so the token saving varies with the model.
