# M007 — Cross-Result Excerpt Economy

Status: ready
Source: `plans/subsystems/agentic-retrieval-integrity-roadmap.md` M007
Primary class: capability
Baseline: `d751bd7471a647e651ee41b8e51199c66d04d01a` (the worktree also carries the uncommitted retrieval-defaults and scraper changes from the prior turn; re-read `git status` before starting and do not revert them)
References: `src/mcp/projection.rs`; `src/mcp/output_schema.rs`; `src/core/source_card.rs`; `src/meta/adapter/normalization.rs`; `src/core/config.rs`; `tests/mcp_projection.rs`; `tests/extract_fetch_contract.rs`; `plans/closure/standalone-mcp-local-inference/002-status.md`; `docs/retrieval-quality-context-efficiency.md`
Dependencies: hard M004 — closed at `c9912fc0e4824c4ec6b1231be2057fb4cd0ba8ab` (`plans/closure/agentic-retrieval-integrity/004-status.md`), whose snapshot lane is the non-regression gate for this payload change. Interface: the closed M002 brief-evidence contract (`plans/closure/standalone-mcp-local-inference/002-status.md`), which this milestone may extend but must not weaken.

## Objective

Give every reduced MCP search response one hard, response-wide ceiling on excerpt text, suppress near-duplicate passages deterministically, and report exactly how much text was collapsed — without touching ranking, identity, card membership, or the complete `diagnostic` payload.

Today excerpts are bounded only per card: `MAX_EXCERPTS_PER_CARD = 3`, `MAX_EXCERPT_CHARS = 500`, `MAX_EXCERPT_TOTAL_CHARS = 1200` (`src/core/source_card.rs:216-223`), applied in `src/meta/adapter/normalization.rs:321-349`. Nothing bounds the total across results, and nothing notices that ten providers describing one page yield ten near-identical snippets. `src/mcp/projection.rs` only trims to one excerpt per card in compact mode (`trim_card_excerpts`, line 671) and clips individual strings inside the brief fitting loop (`clip_brief_strings`, line 542).

The economy is a presentation rule, not an evidence rule. `src/mcp/projection.rs` is already the single central projection boundary (`project_with_budget`, line 61) that all four search tools call with the configured budget, and M002's plan designated it as the one place presentation-budget logic lives. This milestone adds one sibling module in `mcp` and changes nothing below it.

Scope is the card passage text a model actually reads: `snippet` and `excerpts[*].text` on `results[]` or `groups[].results[]`. Treating the two as one family is not a new decision — `merge_excerpts` already seeds its excerpt-dedup key set from the card snippet (`src/meta/adapter/normalization.rs:106-132`), and `ExcerptProvenance::ProviderSnippet` is the default provenance. `vulnerabilities[]`, batch `items[]`, and fetch `text` are out of scope.

## Non-goals

- No embeddings, vector similarity, LLM summarisation, generated text, or answer synthesis. The similarity signal is lexical and inference-free.
- No new MCP tool, no tool rename, no argument addition. The budget is server configuration only; a per-call override is deferred.
- No ranking, fusion, RRF, dedup, filtering, ordering, or truncation change. Suppression removes duplicated passage text and nothing else.
- No removal of a result, group, or card, and no change to any stable ID.
- No change to `diagnostic` or `standard` output, to `build_evidence_bundle` (which stays identity-preserving and never carries excerpts), to `brief_max_chars` semantics, to per-card excerpt caps, or to CLI/`sdk` output.
- No change to `MAX_EXCERPTS_PER_CARD`, `MAX_EXCERPT_CHARS`, `MAX_EXCERPT_TOTAL_CHARS`, or `MAX_EXCERPT_REQUEST_COUNT`.
- No new cache, persistence, or cross-request state.

## Invariants that cannot regress

- Ten MCP tools, additive-only schema, and existing field names and semantics are unchanged.
- Surfaced text is already sanitized: the economy reads `snippet`/`excerpts[*].text` exactly as produced by `sanitize_field` (`src/meta/adapter/normalization.rs:451`). It never re-sanitizes, never un-frames, and never touches `trust_markers`. A card whose only injection-bearing passage was suppressed still reports its `injection_hits`, so suppression can never quiet a warning.
- Framing stays balanced. Framed text is `<<<EXTERNAL_UNTRUSTED field=... id=...>>>\n<body>\n<<<END>>>` (`src/core/sanitize.rs:336-348`). Reduction rewrites the body between the existing header and footer using the existing `clip_framed_text` logic (`src/mcp/projection.rs:598`), so a reduced passage still has exactly one header and one footer and no marker text is ever clipped in half.
- Card/group order is preserved. Suppression removes passage text only; it never removes, reorders, or renumbers a card, group, or surviving excerpt object, and never edits `stable_id`, `id`, `url`, `title`, `providers`, `score`, `trust`, `quality`, or `metadata`.
- Only text that duplicates an earlier surviving passage is suppressed. Because admission is first-wins in response order, a suppressed passage is by construction a near-duplicate of text that survives earlier in the same response; a card's unique evidence is never the thing dropped.
- `diagnostic` remains the complete bounded response. Nested budgets keep their composition: excerpt text ≤ excerpt budget, whole response ≤ `brief_max_chars` in brief mode.
- The result is a pure function of the response value and the configured budget. No clocks, no randomness, no hash-iteration order, no environment reads.
- Bounded work and memory: O(n·k) comparisons with `n ≤ 256` passages and `k ≤ n` admitted, each comparison over at most 256 shingle hashes.

## Expected production changes

1. **Configuration.** Add `excerpt_budget_chars` to `McpSection` (`src/core/config.rs:107-127`) with default `4000`, validated `256..=32000` in `AppConfig::validate` beside the existing `brief_max_chars` check (line 1252). The three default sites must agree: the `default_excerpt_budget_chars()` serde function, the `Default for McpSection` impl, and the docs table. Unit: characters of the serialized passage as it appears in the response, including `<<<EXTERNAL_UNTRUSTED ...>>>` framing and `<<<END>>>` — the same counting unit as `brief_serialized_chars` (`src/mcp/projection.rs:381`) and `brief_max_chars`. This is not a token claim; no tokenizer is introduced. The two budgets stay independent: a brief response is bounded by the smaller effective ceiling because step 4 fits `brief_max_chars` after step 2 has already bounded excerpt text. Do not add a cross-field ordering check — `brief_max_chars` accepts 512, which would invalidate the 4000 default.

2. **Economy module.** New `src/mcp/excerpt_economy.rs` (ordinary file, plan budget 600 lines / 40 KB; guard ceiling stays 1,600 lines / 80 KB and needs no ratchet row). Public surface, all `pub(crate)`:
   - `pub const DEFAULT_EXCERPT_BUDGET_CHARS: usize = 4000;`, `MIN_EXCERPT_BUDGET_CHARS: usize = 256`, `MAX_EXCERPT_BUDGET_CHARS: usize = 32_000`, `MAX_ECONOMY_PASSAGES: usize = 256`, `MIN_SHINGLES_FOR_SIMILARITY: usize = 4`, `MIN_PARTIAL_CHARS: usize = 200`, `SHINGLE_LEN: usize = 4`, `DUPLICATE_OVERLAP: f64 = 0.80`.
   - `pub fn apply(value: &mut serde_json::Value, budget_chars: usize) -> Option<ExcerptEconomyReport>`: locates the primary array once (`results` when present, else `groups[].results`, matching the existing `["results","groups","vulnerabilities","items"]` discovery order at `src/mcp/projection.rs:111-118`), walks passages in response order (`snippet` first, then `excerpts` ascending), applies redundancy suppression and the budget, inserts the report object, and stamps per-card markers.
   - `ExcerptEconomyReport` with the eight counters below, serialized as one additive top-level object.

3. **Deterministic redundancy rule.** For each passage, take the sanitized text, strip the frame envelope if present (header line and `<<<END>>>` footer), lowercase with `str::to_lowercase`, and split tokens on every non-alphanumeric `char`. Passages with no tokens are never duplicates. Then:
   - If the normalized token strings are equal → duplicate.
   - Else build 4-token shingle fingerprints (contiguous windows; FNV-1a 64 over the tokens, reusing the hash family from `src/core/identity.rs`) and require `min(|A|,|B|) >= 4`; shorter passages are compared only by the equality rule, so stub snippets such as `"Read more"` cannot collapse a long passage.
   - Duplicate iff `|A ∩ B| / min(|A|,|B|) >= 0.80` (overlap coefficient) **and** an exact shingle-key comparison confirms the overlap, so a 64-bit hash collision can never itself suppress a passage.
   - Comparison is greedy against admitted passages only, in response order: the first occurrence survives, later near-duplicates are suppressed. That is the entire tie-break — there are no similarity ties to break, and no ordering depends on hash iteration because the reference set is a `Vec` in response order and set operations are sort-free counted intersections over fixed-size fingerprint vectors.
   - Framing is stripped before tokenizing precisely so that common envelope tokens (`EXTERNAL_UNTRUSTED`, `field=snippet`) cannot inflate similarity.

4. **Hard budget.** Admitted passages accumulate their serialized character length in response order. When the next passage does not fit: if the remaining budget is at least `MIN_PARTIAL_CHARS`, admit it clipped to the remaining budget (frame-preserving, UTF-8 boundary safe) and count it in `clipped_count`; otherwise drop it and count it in `over_budget_count`. Once the budget is exhausted, similarity work stops and every remaining passage is counted as `over_budget`; the identity `admitted_count + redundant_count + over_budget_count == considered_count` therefore always holds. The total excerpt characters in the response never exceed `budget_chars`. Drop whole passages rather than emitting sub-200-character stubs, and never truncate mid-frame.

5. **Composition with `response_detail`.** One call site: the `Compact` arm of `project_with_budget`, after the tool projection (and therefore after compact's existing per-card trim to one excerpt) and before the value is returned; `project_brief` calls `project(tool, value, Compact)` first (`src/mcp/projection.rs:101`), so brief inherits the result and its own fitting loop still runs last.

   | `response_detail` | Per-card trim | Excerpt budget + suppression | `excerpt_economy` |
   |---|---|---|---|
   | `diagnostic` | none | not applied | absent |
   | `standard` | none | not applied | absent |
   | `compact` | ≤ 1 excerpt per card (existing) | applied | present |
   | `brief` | ≤ 1 excerpt per card, then brief field reduction and `brief_max_chars` fitting | applied | present |

6. **Reported counts.** Additive only, same object level as `response_detail` and `projection_excerpts_trimmed`:

   | Field | Wire type | Meaning |
   |---|---|---|
   | `excerpt_economy.budget_chars` | integer | configured ceiling that was applied |
   | `excerpt_economy.considered_count` | integer | passages walked (capped at `MAX_ECONOMY_PASSAGES`) |
   | `excerpt_economy.admitted_count` | integer | passages kept, including clipped ones |
   | `excerpt_economy.redundant_count` | integer | suppressed as near-duplicates |
   | `excerpt_economy.over_budget_count` | integer | dropped for budget exhaustion |
   | `excerpt_economy.clipped_count` | integer | admitted with reduced text |
   | `excerpt_economy.chars_before` | integer | serialized excerpt characters before the step |
   | `excerpt_economy.chars_after` | integer | serialized excerpt characters after the step |
   | `results[].excerpts_suppressed` / `groups[].results[].excerpts_suppressed` | integer | passages suppressed on that card; present only when > 0, mirroring the existing `excerpts_truncated` marker |

   Per-card `excerpts_suppressed` exists so that a card losing its only snippet cannot be mistaken for a provider that returned none. Register `excerpt_economy` as an `{"type": "object"}` property in the `web_search`, `repo_search`, `research_search`, and `security_search` output schemas (`src/mcp/output_schema.rs`); the 1,200-byte-per-schema guard has ample headroom.

7. **Boundedness.** Passages beyond `MAX_ECONOMY_PASSAGES` are not examined; they are counted as `over_budget` and their text is removed, so the hard ceiling holds for pathological payloads. Retain fingerprints only for admitted passages, and drop a candidate's fingerprints as soon as it is rejected. Memory is O(k × shingles) ≈ 256 × 256 × 8 bytes at worst; no response copy, no unbounded `String` growth (every reduction is in place with `truncate`/`remove` on existing arrays).

## Storage, protocol, and compatibility effects

- Configuration only: `[mcp].excerpt_budget_chars` is a serde-defaulted field, so existing config files keep parsing and a file without it inherits `4000`.
- Additive response fields; no existing field is renamed, retyped, or repurposed. `excerpt_economy` and `excerpts_suppressed` are new keys under existing objects and loose schemas already set `additionalProperties: true`.
- `diagnostic` and `standard` responses must remain byte-identical to the baseline. `compact` and `brief` necessarily shrink and gain the report object; that is a recorded deviation from M002's "legacy detail values preserve their prior meaning", justified because those two modes are the documented reduced modes and the reduction is reported rather than silent. The closure record must publish the compact/brief byte delta against the M002 fixture set.
- The pre-existing brief behaviour that omits whole records to fit `brief_max_chars` (with `returned_count`/`omitted_count`/`output_truncated`) is unchanged and unrelated: this milestone's no-card-removal invariant applies to the excerpt step alone.
- No storage, cache, protocol version, or CLI change. The text fallback is rendered from the same projected value through `map_tool_result` (`src/mcp/tools/common.rs:328`), so both channels inherit the reduction and the counts.
- Dependency flow is respected: the new module lives in `mcp` and depends on nothing below it; no `core`, `meta`, or `commands` change beyond the config field.

## Ordered work packages

1. Config: `McpSection.excerpt_budget_chars`, serde default, `Default` impl, `validate` range, and the three-sites-agree check; unit tests in `src/core/config/tests.rs` and TOML acceptance/rejection in `tests/config_validation.rs`.
2. `src/mcp/excerpt_economy.rs`: frame-envelope stripping, tokenizer, FNV shingle fingerprints, overlap coefficient with exact confirmation, greedy admission, budget accumulation with frame-preserving clipping, report and per-card markers. Unit tests inside the module for tokenizer, short-passage floor, exact-equality, threshold boundary, and `MIN_PARTIAL_CHARS` behaviour.
3. `src/mcp/projection.rs`: single call in the `Compact` arm, doc comment on the detail-mode composition table, and retention of the report through the brief fitting loop (do not add it to the removable optional-key list at line 233).
4. `src/mcp/output_schema.rs`: four additive properties, keeping every schema under the 1,200-byte guard.
5. `tests/mcp_projection.rs`: detail-mode matrix (report present in compact/brief, absent in standard/diagnostic), byte-identity of the baseline fixture set for standard/diagnostic, stable IDs and card order unchanged, UTF-8 clipping, balanced frames after clipping, `excerpts_suppressed` only when > 0, and the counter identity on every fixture.
6. `tests/property_*` file `tests/property_excerpt_economy.rs`: proptest determinism (same input twice → identical value), budget never exceeded, card count and order preserved, frames balanced, no token loss beyond the budget, and a saturation case with 256 passages × 500 characters plus a >256-passage case asserting the cap and the counter identity.
7. `tests/extract_fetch_contract.rs`: end-to-end through `web_search` with the `mock` engine — a duplicate-passage response collapses to one surviving passage with `redundant_count >= 1`, and a distinct-passage response keeps every card and passage. Record both byte counts.
8. Docs: `docs/config.md` (`[mcp]` snippet and semantics), `architecture/config.md` (`[mcp]` table row), `architecture/mcp.md` (detail-mode composition and the `excerpt_economy` contract), `docs/test-inventory.md` and `architecture/testing.md` (new suite plus changed counts), `CHANGELOG.md` (append-only).

## Verification

```bash
cargo fmt --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo check --locked --no-default-features
cargo test --locked --all-features --test mcp_projection
cargo test --locked --all-features --test property_excerpt_economy
cargo test --locked --all-features --test extract_fetch_contract
cargo test --locked --features mock --test web_search_integration
cargo test --locked --all-features --test mcp_tools
cargo test --locked --all-features --test evidence_contract
cargo test --locked --all-features --test config_validation
cargo test --locked --all-features --test docs_config_snippets
cargo test --locked --all-features --test static_guards
cargo test --locked --features mock --test retrieval_quality
make eval-retrieval-quality
./packaging/check-repo-hygiene.sh
python3 packaging/check-planning-consistency.py
make docs-check
make check
```

`make eval-retrieval-quality` (and the `retrieval_quality` replay) is the M004 gate: payload economy must leave every snapshot metric and row order byte-identical. Run it twice and diff. Finish with `cargo test --locked --all-features`.

## Acceptance and stop conditions

- `diagnostic` and `standard` responses are byte-identical to baseline across the frozen `tests/mcp_projection.rs` fixtures; a recursive key diff shows only the new keys where the budget applied.
- In `compact` and `brief`, the summed serialized length of every `snippet` and `excerpts[*].text` is at or below `excerpt_budget_chars`, including framing.
- Card count, group count, group order, card order, and every `stable_id` are identical to the same response with the budget effectively disabled. No card is ever dropped by this step.
- Duplicate fixture: the later of two ≥ 80% overlapping passages is suppressed, the first survives verbatim, `redundant_count >= 1`, and the suppressed card still carries an `excerpts_suppressed` count. Distinct fixture: nothing is suppressed.
- Negative control that must not collapse: two passages differing only in a version, CVE ID, or numeric identifier are retained. If the rule suppresses them, the threshold or the signal is wrong — stop for a decision instead of loosening the guard.
- Saturation: 256 passages of 500 characters finish inside the budget with bounded memory; more than 256 passages are capped and reported, and `admitted_count + redundant_count + over_budget_count == considered_count` holds in every case.
- Two runs on the same input produce byte-identical JSON. No `HashSet`/`HashMap` iteration influences any output.
- Frames are balanced in every retained passage, no passage is clipped below `MIN_PARTIAL_CHARS`, and `trust_markers` are unchanged from the unprojected payload.
- Stop if enforcement requires changing stable IDs, ranking, card membership, per-card excerpt caps, the brief evidence contract, or `MAX_EXCERPTS_PER_CARD`; or if it requires invalid JSON, invalid UTF-8, or an unbalanced frame.

## Closure evidence

`plans/closure/agentic-retrieval-integrity/007-status.md` must record: the implementation candidate SHA; the exact commands above with their results; the M004 snapshot non-regression evidence (two identical `make eval-retrieval-quality` runs); the diagnostic/standard byte-identity proof; the compact and brief byte deltas against the M002 context-efficiency fixtures (48.0% web and 74.6% fetch baselines, plus the 11.8% sparse-corpus figure); the redundancy fixture table with survivor sets and counts; saturation numbers (passages, worst-case comparisons, peak allocation); the determinism replay; and residual risk (threshold false-positive/false-negative behaviour on unseen provider phrasing, and the fact that the budget is character-based and not a model-token measurement). The closure commit carries this record, the roadmap status update, and the `plans/registry.md` row together.
