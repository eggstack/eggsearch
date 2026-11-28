# Web Acquisition M007 — Container-Document Extraction Fidelity and Bound Correctives — Status

Status: closed
Date: 2026-11-26
Milestone: M007 — Container-Document Extraction Fidelity and Bound Correctives
Milestone class: corrective
Decision authority: ADR-0027 (default enablement); ADR-0026 Decision 2 (format,
dependency, bounds, outcome, and determinism contract — unchanged); M004 and M006
closure records
Baseline: `40e362f1b16f6d2fb5efb4fb91a0c01a85c64144`
Plan: `plans/implementation/web-acquisition-expansion/007-container-document-extraction-fidelity-correctives.md`

## Candidate and changes

Candidate: `cd075fc704b5c03dc2bc28f6a5c5408a320801a4` on
`m008-documents-default-qualification`, the baseline plus this change set, committed
and pushed so the hosted matrix could compile it. The change is library-internal: no
tool, argument, wire string, config key, or dependency moves.

- `src/fetch/documents/extract.rs` (1,411 lines / 49,337 bytes; ordinary ceiling 1,600
  lines / 80 KB): `Event::GeneralRef` handling through `decode_general_ref`, the
  `text_token` merge of adjacent character data, doctype acceptance, qualified
  attribute keys, the streamed XLSX size scan with observed-content bounds, the
  incremental CSV cell count, and per-part truncation carried on `RawPart`.
- `src/fetch/documents/tests.rs` (595 lines): one fixture per defect plus the observed
  bounding-rectangle refusal.
- `tests/documents.rs` (418 lines): three end-to-end cases across the tool boundary.
- `src/mcp/tools/web_fetch_options.rs`: `document_unsupported` classified as a
  capability miss.
- `tests/mcp_tools.rs`: the tool-boundary case for both sides of that classification.
- `src/fetch/cache.rs` and `src/fetch/cache/tests.rs`: `renderer_version` 6 and its
  isolation case.

## Per-defect evidence

| # | Defect | Baseline observation at `40e362f` | Candidate case | Candidate observation |
|---|---|---|---|---|
| 1 | XML character and entity references discarded | `Q&amp;A &lt;tag&gt; caf&#233; &quot;x&quot;` extracted as `QA tag caf x` | `xml_references_resolve_into_the_surrounding_text_run` (unit, exact equality) and `docx_character_references_survive_the_tool_boundary` (integration) | `Q&A <tag> caf\u{e9} "x" 'y'` exactly; an undeclared `&nbsp;` survives as the literal `a&nbsp;b` |
| 2 | A plain `<!DOCTYPE html>` refused | `document_malformed: DTD is not supported` | `xml_accepts_plain_doctypes_and_still_refuses_dtd_payloads` (unit) and `epub_documents_with_a_plain_doctype_extract` (integration) | plain doctype extracts; internal subset, `SYSTEM`, and `PUBLIC` still refuse, in either case |
| 3 | Namespace-qualified attributes collide on the local name | producer attribute order decided whether the slide relationship resolved | `pptx_relationship_ids_are_independent_of_attribute_order` | both orderings extract `Slide Two` with equal titles and no truncation |
| 4 | The declared `<dimension>` is treated as content | a sheet declaring `A1:XFD1048576` with one populated cell refused | `xlsx_declared_dimension_does_not_bound_observed_cells` | the sheet extracts `Only data` |
| 5 | The token bound binds before the cell bound | 45,000 one-cell rows reached the token bound | `xlsx_many_rows_reach_the_cell_bound_not_the_token_bound` | the sheet extracts: 45,000 cells is under the 200,000-cell bound and the streamed scan applies no token bound to it |
| 5 | Row-by-column product treated as content | an over-cap sheet refused without naming a real bound | `xlsx_cell_count_and_bounding_area_are_bounded_on_observed_cells` | a two-cell `A1`/`XFD1048576` sheet refuses at the observed bounding rectangle |
| 6 | CSV cell count enforced after the row is built | refusal only after the whole delimiter-dense row existed | `csv_cell_bound_refuses_during_the_scan` and `delimiter_dense_csv_is_refused_at_the_cell_bound` | refusal raised at the cap during the scan; peak allocation in the table below |
| 7 | Ceilings documented as if they were defaults | see the plan's defect table | documentation diff in `docs/features.md`, `docs/config.md`, `architecture/fetch.md` | ceilings and defaults stated separately |
| 8 | `document_unsupported` reports as an upstream failure | tool code `upstream_failed` | `document_unsupported_container_is_capability_unavailable` and `malformed_container_remains_upstream_failed` | `capability_unavailable` with `data.code = document_unsupported`; `document_malformed` still `upstream_failed` |
| 9 | Truncation inferred at document level from the last part | the last part was flagged whatever part was cut | `truncation_is_attributed_to_the_part_that_was_cut` | the cut part is flagged, later parts are not, `document_truncated` is true |

## Exact extracted strings

The two high-severity cases assert equality rather than containment.

- DOCX body `Q&amp;A &lt;tag&gt; caf&#233; &quot;x&quot; &apos;y&apos;` extracts to
  `Q&A <tag> caf\u{e9} "x" 'y'` — one text run, so no separator enters `Q&A`.
- EPUB content `a&nbsp;b` extracts to `a&nbsp;b`. `nbsp` is not declared by the
  document and there is deliberately no HTML entity table, so the source text survives
  instead of being expanded or dropped.

## Deliberate substitutions from the plan

1. **The XLSX size scan streams instead of deriving `MAX_XML_TOKENS`.** Package 4 asked
   for `MAX_XML_TOKENS` to be derived from `MAX_CELL_COUNT` with a per-row factor and a
   fixed overhead. That derivation would raise the token ceiling to roughly 1.4 million
   `XmlToken` values, which is not a bound worth having. The sheet scan instead streams
   quick-xml events and never materialises a token vector, so there is no second bound to
   drift: the cell bound is the only bound on a sheet. `MAX_XML_TOKENS` stays 262,144 and
   governs the token-scanned formats only (DOCX body, PPTX slide documents, EPUB spine
   documents). The plan's stop condition for a CSV streaming rewrite is not met; the CSV
   count is incremental inside the existing scan.
2. **The observed bounding rectangle is bounded, not only the cell count.** Calamine's
   sparse-to-dense conversion allocates the rectangle between the minimum and maximum
   observed column and row, so two cells at `A1` and `XFD1048576` are two cells and
   sixteen billion allocation slots. The plan named the cell count; the implementation
   bounds both, and the refusal names the rectangle.
3. **The defect-9 fixture uses the EPUB heading cap (4,096 characters), not the document
   character budget.** `document_chars.min(MAX_DOCUMENT_CHARS)` is consumed in part order,
   so a document whose first part reached the budget always loses every later part, which
   makes "part 1 cut, later parts uncut" unreachable through the document budget. The
   heading cap produces the same shape honestly: part 1 is cut and flagged, part 2 is
   whole.

## CSV peak allocation

Fixture: one delimiter-dense CSV row of `a,` pairs, 4,194,304 bytes admitted
(4 MiB), so the row carries 2,097,152 cells — 10.5 times the 200,000-cell bound.
Probe: a release-profile binary that calls
`extract_document(&body, DocumentFormat::Csv, ..)` once, identical in both trees and
differing only in the tree it is compiled from; peak resident set from
`/usr/bin/time -l`. The two binaries were confirmed byte-different, and the
outcomes differ, so the same code was not measured twice.

| Tree | Peak resident | Ratio to admitted body | Outcome |
|---|---|---|---|
| `40e362f` | 70,582,272 bytes (67.31 MiB) | 16.83x | `document_malformed` |
| candidate | 20,201,472 bytes (19.27 MiB) | 4.82x | `document_malformed` |

The refusal is the same bound in both trees; what changed is when it is raised. Before,
the whole row was materialised before the count, so peak tracked the separator count.
After, the count is enforced during the scan, so peak is a small multiple of the
admitted body — a 3.49x reduction on this fixture, bounded by `MAX_CELL_COUNT` rather
than by the input.

## Derived cache version

`renderer_version` is 6 (`src/fetch/cache.rs`), covering packages 1, 2, 5, and 6 — every
one of them either changes extracted text or produces text where a refusal was cached. The
invalidation is global by design: it drops every derived entry class, not only container
documents, because the version is part of the derived key rather than per-format state.
`cache_derived_does_not_serve_pre_container_fidelity_entries` stores a version-5 entry and
shows it is not served. There is exactly one bump in this change set.

## Tool-boundary error codes

| Fetch code | Tool error code before | Tool error code after |
|---|---|---|
| `document_unsupported` | `upstream_failed` | `capability_unavailable` |
| `document_disabled` | `capability_unavailable` | `capability_unavailable` |
| `document_malformed` | `upstream_failed` | `upstream_failed` |
| `document_empty` | `upstream_failed` | `upstream_failed` |
| `document_encrypted` | `upstream_failed` | `upstream_failed` |

The fetch-layer code strings are unchanged; only the tool error code for the capability
miss moved, which is why no `FetchErrorKind` was added.

## Feature-off parity

`cargo test --locked --no-default-features --test documents` (the `make parity-check`
lane) runs the three parity
cases and passes: `docx_without_the_feature_keeps_the_unsupported_outcome`,
`capability_report_is_honest_without_the_feature`, and
`csv_without_the_feature_keeps_the_table_preview`. A container URL still returns
`unsupported_content_type` and a CSV URL still returns the table preview.

## Independent audit and remediation

An independent verifier audited this change set against the plan and returned a narrow
FAIL: every behavioural fix worked and every executed suite was green, but four
acceptance-letter items were unmet and two observations were recorded. Each finding was
either remediated in code and fixtures or recorded here as a property of the design; none
is left silent.

| Finding | Severity | Resolution |
|---|---|---|
| 1. The "no `Cargo.toml` change" invariant is breached by the co-mingled M008 work | Medium | Expected and owned elsewhere: M008 is the default-set flip, and no M007 behaviour depends on it. The feature-off parity lane now runs through `make parity-check` rather than a hand-typed `--no-default-features` command. |
| 2. WP4's derived `MAX_XML_TOKENS` bound and its guard are absent | Medium | Structural alternative, not an omission: the streamed sheet scan has no token vector to bound, so a derivation would create a second bound with nothing to enforce it. Recorded as substitution 1 above. |
| 3. "Across the workbook" contradicted per-sheet enforcement | Medium | Counters hoisted. The cell count is workbook-wide; the bounding rectangle stays per sheet, because that rectangle is the allocation the reader performs. The documented phrase now matches the code. |
| 4. The doctype and `&nbsp;` cases were weaker than the acceptance text | Medium | Strengthened at the extractor boundary: the unit cases now assert `assert_eq!` on the exact extracted text, and the reference fixture there gained a hexadecimal reference and a CDATA section adjacent to character data. The two cross-boundary cases keep `contains`, because their job is to prove the references survive the MCP projection, not to re-assert the exact string the unit case owns; the doctype case also asserts `parts_total == 1`. |
| 5. Two XLSX halves pass at `40e362f` as well | Low | Characterization, not regression. Both the 200,001-cell half and the sparse `A1`/`XFD1048576` half already refused at baseline with the same message, so they lock in refusal identity rather than prove a fix. Only the dimension sheet and the 45,000-row sheet are fail-on-baseline for defects 4 and 5, and the table above now says so. |
| 6. `TokenKind::Truncated` is a new inert token kind | Low | Accepted redundancy. The variant's `kind` is unused, but the token is the carrier of the `truncated` flag that DOCX, EPUB, and PPTX OR into their own state, so removing it would lose the clip signal. Replacing the marker with a returned flag is a follow-up refinement, not a corrective. |
| 7. The briefing's file list omitted the documentation diff | Info | The documentation exists and defect 7 is closed by it. |

## Verification on this candidate

| Command | Result |
|---|---|
| `cargo test --locked --features documents --lib fetch::documents` | 15 passed |
| `cargo test --locked --features documents --test documents` | 11 passed |
| `cargo test --locked --no-default-features --test documents` | 3 passed |
| `cargo test --locked --all-features --test mcp_tools document_error_mapping` | 2 passed |
| `cargo test --locked --all-features --lib fetch::cache` | 59 passed |
| `cargo test --locked --all-features --test static_guards` | 69 passed |
| `make check` (canonical gate) | exit 0; 6,699 cases across 101 suites; `parity-check` lane present |
| Hosted `Python native qualification`, run 46115248218 on `cd075fc` | 41/41 jobs succeeded, so this change set compiles and passes in the wheel's platforms and interpreters, not only on the host |

The canonical `make check` result above is the final run, on the tree that carries the
remediation in the audit section, and it covers the four documents cases itself. An
earlier gate run had completed before the last fixture edits were compiled; those cases
were re-run directly on the current sources and then re-covered by the final gate.

## Limitations

- The nine defects are closed against the plan's own observations at `40e362f`; no
  third-party corpus was re-run, and `tests/tool_surface_live.rs` is `--ignored` and
  model-gated, so it is not evidence here.
- `MAX_XML_TOKENS` is no longer derived from `MAX_CELL_COUNT` because the sheet scan has no
  token bound to derive. This is a substitution, recorded above, not a completed package.
- No hosted qualification was run. The documented provider-side smokes are unaffected by
  this change, but this record does not claim them.
