# M002 Captured PDF Corpus and Fidelity Lane — Closure Status

Status: closing

Source plan: `plans/implementation/pdf-ingestion-fidelity/002-captured-pdf-corpus-and-fidelity-lane.md`

Roadmap: `plans/subsystems/pdf-ingestion-fidelity-roadmap.md`

Planning baseline: `9d4d16fc32500196a1b88554b898cd8eddc84731` (2026-11-11 UTC)

Implementation candidate: the uncommitted worktree on that baseline. No closure commit exists yet.

## Result

PDF extraction now has a committed corpus, deterministic metrics, and a negative control that proves the
lane can detect the failure shapes it exists to catch. The production extractor is unchanged.

`tests/fixtures/pdf/corpus.json` is the manifest: schema version 1, corpus revision `pdf-fidelity-v1`, a
`fixture_policy` string, a byte budget, and one entry per case carrying `id`, `document_class`,
`source_type`, `license`, `provenance`, `expected_outcome`, `construction`, and annotations. Every
committed PDF is synthetic; the manifest records the construction recipe and the SHA-256.

## Case inventory

| Class | File | Bytes | SHA-256 (first 16) |
|---|---|---:|---|
| `two_column` | `two_column_interleaved/arxiv_style_two_column.pdf` | 1,886 | `fd905a7e4910a05e` |
| `standards_table` | `standards_table/rfc_style_standards_table.pdf` | 3,489 | `fd4c8861aba86f32` |
| `clean_text` | `clean_text/single_column_notes.pdf` | 1,466 | `46bb02055720afad` |
| `image_only` | `image_only/scanned_placeholder.pdf` | 774 | `fe6287e8af04d99f` |
| `encrypted` | `encrypted/locked_placeholder.pdf` | 903 | `7a3ee3d80ae0bb36` |
| `malformed` | `malformed/broken_xref.pdf` | 701 | `0175cfebfba998af` |
| `cid_corrupt` | `cid_corrupt/garbled_codes.pdf` | 1,011 | `f6574c816f79b203` |
| `outline_heavy` | `outline_heavy/nested_outline.pdf` | 5,660 | `1ec28842f4170377` |

Total 15,890 bytes against a 3 MiB corpus budget and a 1 MiB per-case budget, both asserted by the suite.

## Metrics and negative control

`tests/pdf_fidelity.rs` implements line-shape distribution, required-word presence, table-row adjacency,
ordered-passage presence, per-page classification (over text plus the production per-page metadata), and
running-head repetition. Each metric has a passing and a failing unit test.

The suite asserts 116 baseline annotations and records 15 ADR-0008 target statuses. The split is
deliberate: the corpus contains both the current failure shapes and the invariants that already hold, so
the lane passes on unchanged extraction while an improvement fails the recorded target status until the
manifest is updated. That makes M003 and M004 changes visible instead of silent.

The negative control joins all lines and strips spaces, then requires every declared metric group to
reject the degraded text. Observed failures include:

- `line_shape.min_body_lines: observed 1 below 7`
- `line_shape.max_body_line_chars: observed 600 above 500`
- `table_row_adjacency.Columns: / 2: label and value are not on one line`
- `page_classification.page_count: observed 1 expected 2`

25 metric groups reject the degraded corpus.

## Corpus substitution

The committed corpus is synthetic for every class. There is no PDF page-splitter in this environment
(`qpdf`, `mutool`, `pdftk`, and `gs` are absent), and committing third-party page subsets would require
a licence record that this workstream cannot establish for arXiv or IETF material. The plan's stop
condition covers this case: the real documents already named in the workstream's evidence (arXiv
1706.03762 and RFC 9110) are measured only by the ignored recorder into `target/pdf-corpus/`, are never
committed, and are never required by CI. The substitution is recorded in `fixture_policy` and here.

## Verification

- `cargo test --locked --features pdf --test pdf_fidelity` — 15 passed, 1 ignored.
- Two consecutive runs produced byte-identical report lines.
- `cargo test --locked --all-features --lib fetch::` — 631 passed at the M002 candidate.
- `cargo test --locked --all-features --test static_guards` — 60 passed.
- `./packaging/check-repo-hygiene.sh` — ok.
- `rustfmt --edition 2021 --check tests/pdf_fidelity.rs` — clean; the file also passes
  `cargo clippy --all-features --test pdf_fidelity -D warnings`.

## Registration

`architecture/testing.md` gains the suite row and the layout row plus the run command;
`docs/test-inventory.md` gains the suite row, the case/assertion counts, the corpus table, and the
ignored-recorder note; `docs/retrieval-quality-extraction-matrix.md` gains a pointer that makes the
fidelity lane, not the matrix, the extraction-fidelity authority. `AGENTS.md` is unchanged.

## Residual risk

The corpus is synthetic, so it cannot prove behaviour on arbitrary real-world producer output. The
opt-in recorder is the real-document check. Recorded target statuses track the current extractor, so
M003 and M004 must update the manifest in the same change that improves the measured shape, or the lane
will fail — which is the intended ratchet.
