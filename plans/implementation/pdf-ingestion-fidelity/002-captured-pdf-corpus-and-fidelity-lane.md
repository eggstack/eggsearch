# M002 — Captured PDF Corpus and Fidelity Lane

Status: closing
Source: `plans/subsystems/pdf-ingestion-fidelity-roadmap.md` M002
Primary class: infrastructure
Baseline: `9d4d16fc32500196a1b88554b898cd8eddc84731`
References: `architecture/testing.md`; `docs/test-inventory.md`; `tests/fixtures/retrieval_quality/extraction_pairs.json`; `tests/retrieval_quality.rs`; `src/fetch/pdf.rs`; `docs/retrieval-quality-extraction-matrix.md`; `plans/adrs/ADR-0008-pdf-extraction-fidelity-strategy.md`.
Dependencies: none hard. Produces the measurement lane M003, M004, and M005 depend on. Provides the target values ADR-0008 accepted as its fidelity gates; the lane itself does not depend on the decision.

## Objective

Give PDF extraction a committed, licence-clean corpus and a deterministic fidelity lane that fails when extraction degrades. The lane is the gate for every later change in this workstream; without it, M003, M004, and M005 can only assert improvement, which is exactly the defect this workstream exists to correct.

## Non-goals

- No production extraction change. This milestone adds fixtures, metrics, and a suite, and must leave extracted output byte-identical.
- No ranking, retrieval, or chunking change.
- No full-document fixture copies. Committed fixtures are page subsets with annotations, never whole third-party documents.
- No network in CI. Live capture is an ignored, opt-in recorder.
- No claim of extraction quality in this milestone; it only establishes the ability to measure.

## Current implementation evidence

- PDF coverage today is 57 unit tests against generated one-page documents, a `fuzz/fuzz_targets/extract_pdf_text.rs` target, `tests/corpus/adversarial/pdf_extended.json` (28 cases of pseudo-PDF strings), and one synthetic searchable PDF in the extraction matrix.
- `docs/retrieval-quality-extraction-matrix.md` explicitly states the report "does not claim broad multi-page table, columns, outline, corrupt-CID or scan fixture recall."
- The committed extraction pair `rfc9110-pdf-text` names expected sections `15.5.5` and `15.5.11`, which occur far beyond the default 25-page extraction budget, so the case cannot pass as written without page selection. It also asserts document terms, not structural invariants, so it would pass on a single flattened line.
- No metric currently detects the two measured failure shapes: a 4,301-character line, or a table whose labels and values are separated by page order.

## Invariants that cannot regress

- Fixtures are third-party material: keep them small, keep provenance, keep the licence, and commit annotations rather than bodies.
- The suite is deterministic, offline, and keyless. No test may require network, credentials, a browser, or a rendered image.
- Fixture bytes count against repository hygiene and packaging guards; keep the committed corpus under a declared byte budget.
- Existing extraction output must be unchanged by this milestone; any diff means a production change slipped in.
- The corpus must not encode a single aggregate score that a regression can hide inside.

## Expected production changes

None. The only source changes permitted are pure metric helpers if they are placed in the crate rather than the test tree; prefer keeping metrics in shared test support and adding unit tests for them.

## Fixture and annotation design

Location: `tests/fixtures/pdf/` with one `corpus.json` manifest and one subdirectory per case.

Manifest entries carry:

- `id`, `document_class` (`two_column`, `standards_table`, `clean_text`, `image_only`, `encrypted`, `malformed`, `cid_corrupt`, `outline_heavy`), `source_type`.
- `license` and `provenance { url, retrieved, sha256, pages }`. A case without a recorded licence may not be committed; it belongs to the opt-in lane.
- `expected_outcome`: `extracted`, `empty`, or `structural_break`.
- `required_words`: words that must appear intact, chosen to catch known damage (for example `defines`, `definition`, `identifier` in a document whose ligature runs are space-split).
- `required_table_rows`: ordered `{label, value}` pairs whose adjacency the extractor must preserve.
- `opening_passage`: an ordered list of sentence prefixes that must appear in that order, catching reading-order inversion in a two-column document.
- `min_body_lines` and `max_body_line_chars`: shape floors and ceilings.
- `page_selection` when the case requires it, so the case can reach the pages it annotates.

Case coverage must include every `document_class` above. Prefer documents with clear redistribution terms and small page subsets (one to four pages each). Encrypted, malformed, CID-corrupt, and image-only cases may be generated locally and committed as synthetic fixtures with a documented construction, avoiding third-party licensing entirely.

Committed corpus budget: 3 MiB total, no single case above 1 MiB, enforced by an assertion in the suite and consistent with repository hygiene.

## Ordered work packages

1. Define the manifest schema and the annotation vocabulary; add a loader with strict validation (unknown class, missing licence, missing annotation, or over-budget case fails the suite).
2. Capture and commit the case set, one case at a time, verifying each expected outcome by hand before it becomes an assertion.
3. Implement the metric functions: line-shape distribution, required-word presence, table-row adjacency, ordered-passage presence, and per-page classification. Each takes text plus annotations and returns a structured result; unit-test each against a passing and a failing input.
4. Add `tests/pdf_fidelity.rs` gated on the `pdf` feature, asserting every case against its annotations and reporting a per-case table with `--nocapture`.
5. Add the lane's negative control: apply a deliberately degrading transform to extracted text (join all lines, then strip spaces) and assert the metric functions reject it. This proves the lane can detect the failure shapes it exists to catch.
6. Add the opt-in recorder as an ignored test that fetches named source documents into `target/pdf-corpus/`, measures them, prints the report, and never writes to the repository.
7. Register the suite in `architecture/testing.md` and `docs/test-inventory.md`.

## Verification

- `cargo test --locked --features pdf --test pdf_fidelity -- --nocapture`
- `cargo test --locked --features pdf --test pdf_fidelity` twice, confirming byte-identical reports
- `cargo test --locked --all-features --lib fetch::pdf::`
- `cargo test --locked --all-features --test static_guards`
- `./packaging/check-repo-hygiene.sh`
- `make check`; `make docs-check`

## Static guards and documentation updates

- `architecture/testing.md` gains the suite row with its contract; `docs/test-inventory.md` gains the case and assertion counts.
- `docs/retrieval-quality-extraction-matrix.md` gains a pointer to the new lane and stops implying the matrix is the fidelity authority.
- The committed fixture files must not trip repository hygiene or packaging include/exclude rules; verify with `make packaging-check` in CI rather than locally if the sandbox blocks it.
- `AGENTS.md` is not modified; testing placement is owned by `architecture/testing.md`.

## Acceptance and stop conditions

- Every `document_class` has at least one case with recorded provenance or a documented synthetic construction.
- Running the suite against unchanged extraction passes; running it against the degraded transform fails, and the failure names the specific annotation that was violated.
- Two consecutive runs produce identical reports.
- The suite runs offline with no credentials and completes within the routine CI budget.
- Total committed fixture bytes are within the declared budget.
- Stop if a case cannot be committed legally; move it to the opt-in lane and record the substitution rather than shipping an unlicensed document.
- Stop if a metric can only be satisfied by a production extraction change; that belongs to M003 and must not be smuggled in here.

## Closure evidence

`plans/closure/pdf-ingestion-fidelity/002-status.md` records the case inventory with licence and provenance, per-case assertion counts, the pass output on unchanged extraction, the negative-control failure output, the byte total, and the suite registration diff.
