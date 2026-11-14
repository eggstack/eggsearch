# PDF Ingestion and Extraction Fidelity Roadmap

Status: closing (M001, M002, and M007 implemented; M006 qualification remains open; M003–M005 and M008 ready)
Planning baseline: `9d4d16fc32500196a1b88554b898cd8eddc84731` (branch `feat/auto-20261110-0a674cdc`, 2026-11-11 UTC)

References: `plans/000-long-term-specification.md` §5, 7, 8; `plans/001-terminology-and-domain-model.md`; `plans/002-long-term-roadmap.md`; `plans/003-planning-process.md`; accepted ADR-0002, ADR-0006, ADR-0007, ADR-0008.

Prior closed evidence: `plans/subsystems/deterministic-retrieval-quality-roadmap.md` M002 (PDF/browser extraction matrix) and M006; `plans/subsystems/retrieval-quality-evidence-extension-roadmap.md` M007 paired extraction qualification (`docs/retrieval-quality-extraction-qualification.md`); `plans/subsystems/agentic-retrieval-integrity-roadmap.md` M001 (the four-outcome scraper fidelity contract this workstream mirrors for documents).

## 1. Purpose and ownership boundary

Make PDF ingestion a capability an agent can rely on: reachable in the artifact a user installs, bounded against real-world document sizes, faithful enough to read a two-column paper or a table without silently losing structure, honest when it cannot, and correctly integrated with the document, focus, and chunk surfaces the rest of the product already provides.

Eggsearch owns PDF detection, bounded fetch and decompression, text extraction and layout reconstruction, extraction quality classification, chunking, and MCP response shape. It does not own rasterization, OCR model quality, document rendering, or answer synthesis. This workstream adds no native library, no inference, no new MCP tool, and no crawler.

This is a new subsystem boundary split out of fetch and extraction ownership. The planning process allows a boundary to be split when ownership or dependency analysis warrants it; the reason is that PDF fidelity work spans fetch budgets, packaging availability, a measurement lane, and response economy at once, which no existing workstream owns end to end.

### Work classification

- **Invariant:** bounded I/O and deadlines; no native library or inference in any path; text-only data handling; untrusted-content sanitization and trust framing; ten MCP tools and existing wire strings; stable FNV-1a IDs; deterministic, non-random extraction.
- **Capability:** a PDF fetch that succeeds on an ordinary document; readable reading order, tables, and columns; honest scanned versus blank classification; query focus that selects the relevant part of a PDF.
- **Infrastructure:** captured PDF corpus and fidelity measurement lane; faithful chunk construction for document kinds that are not HTML.
- **Polish:** diagnostics, capability reporting, documentation accuracy.

## 2. Baseline gaps

At the planning baseline (`9d4d16f`, 2026-11-11), detection was sound: Content-Type, `.pdf` extension, and a `%PDF-` magic sniff covered ordinary and mis-served PDFs; `classify_link` tagged PDF link targets; and page-spec parsing, outline extraction, page labels, and password redaction were bounded and tested. The following gaps informed the milestones. Their current disposition is recorded in §9 and the closure records.

Measured or code-evidenced at the planning baseline:

- **Unavailable in distributed artifacts.** The default feature set is empty and the published distribution requests no feature flags, so released binaries answer `pdf_not_compiled_in`. `docs/features.md` nonetheless documents PDF extraction as a capability. ADR-0006 recorded the "not selected" decision; ADR-0007 now supersedes it, and no release has shipped the capability yet.
- **The default byte cap is smaller than ordinary PDFs.** `[fetch].max_bytes` defaults to 2,000,000 bytes and applies to PDFs. A 2,215,244-byte arXiv paper with a `Content-Length` is refused before download; a 2,858,365-byte chunked response is silently truncated and then reported as `pdf_parse_error` (cross-reference failure), which attributes a size problem to the document.
- **Extraction has no position model.** Text arrives in content-stream order with newlines only on `'`, `"`, `T*`, and `ET`. Measured: a 15-page two-column paper produced 141 non-empty lines with a 4,301-character line; a 311-page standards document produced 928 lines with a 3,173-character line and lost table label-to-value adjacency.
- **Text normalization loses words.** 153 of 175 ligature codepoints in one document arrived space-wrapped, so `defines`, `definition`, and `identifier` were absent from the extracted text.
- **Quality scoring cannot see any of it.** Both documents reported `quality_score: 1.00` with every page `CleanText`.
- **Scanned detection is suppressed by default.** Image counting only runs when `include_media` is true, so a default call classifies an image-only page as `Blank` and returns `pdf_no_extractable_text`; the actionable `ScannedOrImageOnly` classification and warning appear only when the caller opts into media metadata.
- **Query focus is inert.** HTML and text documents are chunked through the shared outline-aware builder; PDFs hand-build one chunk spanning every page, so focus has a single candidate and returns the document prefix. Measured: two disjoint queries produced byte-identical focused text, and document-late terms were absent from the result.
- **Two budgets disagree, and the payload repeats itself.** A PDF response carries `text` bounded by the caller's `max_chars`, per-page `blocks` bounded instead by `pdf_max_total_chars`, and one `chunks` entry that rejoins those blocks. The same characters are therefore serialized up to three times while the caller's declared character budget bounds only one of the three fields. `text_chars_returned` is correct for its documented meaning (the legacy `text` field) and stays that way.
- **Decompression is unbounded.** The response byte cap bounds the compressed input; stream inflation has no output bound, so a small hostile document can expand in memory.
- **Local and batch paths ignore PDFs.** `.pdf` is a binary extension for local workspace reads, and `batch_fetch` cannot pass per-URL page selection.
- **The corpus cannot exercise the claims.** Coverage is 57 unit tests, a fuzz target, synthetic one-page fixtures, and pseudo-PDF strings in the adversarial corpus. There are no captured real PDFs, and the committed extraction pair that names PDF section markers points at sections beyond the default page cap, so it cannot pass as written.

## 3. Invariants and non-goals

- Text-only extraction. No rasterization, no rendering, no OCR, no image extraction, no inference in any execution path.
- No native library and no new parser dependency. `lopdf` stays the only PDF parser with `default-features = false`, and `make dependency-policy`, the MSRV all-features check, and the HTTP dependency-shape guard stay green.
- All existing bounds stay in force: URL length, response bytes, redirects, per-page characters, total characters, page count, outline entries and depth, page-label expansion, timeouts, deadlines.
- Untrusted-content rules are unchanged: sanitization tiers, control-character stripping, `bound_text`, `strip_control_chars`, trust markers, and the `untrusted` warning on every fetched document.
- The ten-tool contract, existing argument names, and existing wire strings are unchanged. New outcome values are additive; `pdf_no_extractable_text` is not renamed.
- Stable IDs remain content-derived FNV-1a. Chunk IDs derive from the document ID and index exactly as the shared builder defines them.
- Determinism: identical bytes produce identical extraction, chunking, IDs, and ordering across runs and platforms.
- Cache isolation: a derived document must never be served for a different extraction behaviour. Every extraction-output change bumps `renderer_version`.
- No default behaviour change lands before its measured acceptance, and no artifact change lands without the packaging qualification path.

Non-goals: OCR and any rasterization; scanned-document reconstruction; PDF editing, generation, signing, or form filling; annotation or attachment extraction; JavaScript actions; embedded-file extraction; recursive crawling to discover PDFs; a PDF-specific MCP tool; changing the page-selection syntax; native PDFium or MuPDF bindings.

## 4. Measurement architecture

Three strictly separate lanes, mirroring the retrieval-quality convention:

1. **Committed corpus (routine CI, no network):** small page subsets of redistributable documents plus documented synthetic shapes, each with provenance (source URL, retrieval date, SHA-256, licence, included pages) and per-document annotations: required words, required table rows, required opening passage, minimum line count. Annotations, not copied body text, keep the fixture set legal and small.
2. **Opt-in live qualification (manual/dispatch):** an ignored recorder fetches original documents into `target/pdf-corpus/`, measures them, and never commits them. Used for release qualification and drift checks, never required for CI.
3. **Host consumer observation:** agents reporting what they could not read stays consumer-owned; this workstream accepts fixtures and shapes, not model quality claims.

Acceptance rule: no extraction, chunking, or payload change is accepted without a captured-corpus measurement showing no regression on every annotated invariant, plus either a measured improvement or an explicitly recorded no-change decision. A corpus that cannot detect a deliberately degraded extractor is itself a defect; M002 must include a seeded negative control.

## 5. Milestone dependency graph

```text
M001 PDF fetch contract and budgets ─┬─> M006 artifact availability (hard)
                                     └─> M008 local and batch ingestion (hard)
M002 captured corpus and fidelity lane ─┬─> M003 (hard) ──> M004
                                        ├─> M005
                                        └─> M008 (soft)
M007 hostile-input bounds (independent)

Gates satisfied: ADR-0007 (accepted) unblocked M006; ADR-0008 (accepted) unblocked M003 and M004.
```

## 6. Milestones and exit boundaries

| Milestone | Class | Handoff | Dependencies | User-visible exit |
|---|---|---|---|---|
| M001 — PDF fetch contract and budgets | invariant (corrective) | `plans/implementation/pdf-ingestion-fidelity/001-pdf-fetch-contract-and-budgets.md` | none | A PDF larger than the generic response cap can be fetched under a PDF-specific budget; oversize reports a size problem, not a parse error; an image-only page reports scanned without `include_media` |
| M002 — captured PDF corpus and fidelity lane | infrastructure | `002-captured-pdf-corpus-and-fidelity-lane.md` | none | CI measures extraction fidelity per document from committed annotations and rejects a deliberately degraded extractor |
| M003 — text-stream fidelity | capability | `003-text-stream-fidelity.md` | hard M002; ADR-0008 accepted | Lines, words, and ligatures survive extraction: no collapsed multi-thousand-character lines, no damaged words, hyphenation joined |
| M004 — structural reconstruction and honest quality | capability | `004-structural-reconstruction-and-quality-signal.md` | hard M003; ADR-0008 accepted | Columns read in author order, table rows keep label-to-value adjacency, and a layout-collapsed document cannot report full quality |
| M005 — chunking, focus, and payload economy | capability (corrective) | `005-pdf-chunking-focus-and-payload-economy.md` | hard M002 | Two disjoint focus queries on a PDF return different, relevant chunks; the same text is no longer shipped three times; budgets agree |
| M006 — artifact availability | capability | `006-artifact-availability.md` | ADR-0007 accepted; hard M001 | An installed release ingests an ordinary PDF with no source build and no extra configuration |
| M007 — hostile-input and memory bounds | invariant | `007-hostile-input-and-memory-bounds.md` | none | A decompression-bomb document is rejected by a bound rather than by exhausting memory |
| M008 — local workspace and batch ingestion | capability | `008-local-workspace-and-batch-ingestion.md` | hard M001; soft M005 | A PDF inside a local repository can be read, and `batch_fetch` can select pages per URL |

M001, M002, and M007 have no predecessors and are the first handoffs. M005 depends on M002's corpus for its focus annotations, so it lands after M002 closes but is otherwise independent of M003 and M004: it operates on blocks and outline entries, so better lines and structure improve it without changing its contract.

ADR-0008 accepted the layout strategy, which discharges the interface gate for M003 and M004; M003's remaining dependency is M002's corpus, and M004's is M003. ADR-0007 accepted the artifact decision, which unblocks M006; that milestone additionally requires M001, because a default install must carry the corrected fetch contract before PDFs become reachable everywhere. M008 requires M001 and depends on M005 only softly, so it may land before the extraction milestones and must state which chunk shape it inherits.

## 7. Cross-cutting concerns

- **Security:** decompression bounds are part of hostile-input handling, not an optimization; a bound must exist before extraction cost can grow. No new outbound request, redirect, or credential path is introduced. PDF bodies remain bounded fetches subject to the existing SSRF and redirect policy.
- **Data handling:** no image bytes are decoded or emitted; extracted text stays untrusted external content. Fixtures are page subsets under documented licences with provenance, and annotate expectations rather than copying document bodies.
- **Protocol and schema:** additive fields and additive outcome values only. Any new advertised value is documented where the fetch outcome taxonomy lives, and `tests/docs_tool_names` and provider-inventory guards must stay untouched and green.
- **Cache correctness:** extraction output is cached; a fidelity change without a `renderer_version` bump serves stale, worse documents. Each milestone that changes output states the bump.
- **Bounds and determinism:** every new surface (corpus size, annotation bounds, line buffers, layout state) needs an explicit cap and a saturation test. Layout heuristics must be total orders with written tie-breaks, not hash-order aggregation.
- **Observability:** `provider_status` capability reporting and per-page metadata are the observable surface for "can this build read PDFs, and did it read this one well". They must agree with the code.
- **Documentation:** `docs/features.md`, `architecture/fetch.md`, `docs/config.md`, `docs/test-inventory.md`, and `architecture/testing.md` are the owning documents; `AGENTS.md` stays an index and must not restate PDF rules.

## 8. Risks and deferred work

Layout heuristics are the main fidelity risk: column detection can mis-order a document that mixes spanning headings with columns, and row association can mis-group a table with merged cells. Corpus overfitting is the second: a heuristic tuned to a handful of documents can regress on the next one, which is why M002 requires a negative control and M003/M004 require per-document annotations rather than a single aggregate score. Enabling the feature by default raises the cost of an extraction regression, because a bad extraction would then reach every install. Decompression bounds can reject a legitimate but extremely compressible document if set too low. Fixture provenance can decay as upstream documents change; the hash and retrieval date are the mitigation, and refresh is manual by design.

Deferred without a plan: OCR and rasterization, image and attachment extraction, PDF generation or editing, annotation extraction, page-image thumbnails, table-to-structured-data extraction beyond row adjacency, and any native binding.

## 9. Closure and status

The roadmap was approved for execution on 2026-11-11. M001, M002, and M007 are implemented with closure records and remain `closing` pending SHA and status reconciliation. M006's production change landed at `5605cc2` with the browser default work and remains `closing` pending macOS arm64 size/native qualification, installer smoke beyond Linux x86_64, realistic-mix qualification, and the wheel matrix. M003, M004, M005, and M008 remain `ready`, subject to their hard dependencies.

Each milestone needs its own `plans/closure/pdf-ingestion-fidelity/NNN-status.md` recording the implementation candidate, the corpus measurement before and after, tests and guards run with outcomes, unchanged safety evidence, failures and unrun verification, and residual risk. The roadmap may only mark a milestone closed when its closure record exists and the registry row is updated in the same commit. No milestone is pre-closed. M001 and M005 are corrective: each must record the defect observed at baseline and why the existing verification did not catch it.

## 10. Recorded decisions

These shapes are decided for this workstream. Changing one requires a new milestone or an explicit superseding decision, not a silent deviation.

1. **Text-only, permanently bounded.** PDF extraction stays deterministic and text-only. OCR and rasterization are out of scope until a separate accepted decision says otherwise.
2. **One parser.** `lopdf` remains the only PDF parser. A second parser is a fallback recorded in ADR-0008, not a parallel path.
3. **Four-outcome document contract.** Every PDF fetch resolves to exactly one of: extracted, explicit empty (blank or image-only, classified and warned), structural break (parse, encryption, or hostile structure), or unavailable (feature or configuration gate). Success with empty text is never reported as absence.
4. **Honest quality.** A quality score may not be high for output the extractor knows it mangled. Line-shape and structure confidence participate in classification.
5. **One document model.** PDFs produce blocks, outline entries, chunks, and metadata through the same shared builders as HTML and text; a document kind may not carry a private, degenerate document shape.
6. **Measure before changing defaults.** A default-behaviour or artifact change requires recorded corpus evidence inside its own milestone, and artifact availability requires an accepted ADR.
7. **No page-cap workaround.** Raising `pdf_max_pages` is not a fidelity strategy; page selection exists and must be discoverable rather than replaced by a larger default budget.
