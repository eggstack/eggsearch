# M007 — Hostile-Input and Memory Bounds

Status: closing
Source: `plans/subsystems/pdf-ingestion-fidelity-roadmap.md` M007
Primary class: invariant
Baseline: `9d4d16fc32500196a1b88554b898cd8eddc84731`
References: `plans/subsystems/pdf-ingestion-fidelity-roadmap.md` §3, §6, §7, §10; `plans/adrs/ADR-0008-pdf-extraction-fidelity-strategy.md` § Decision; `architecture/fetch.md` § 7, § 11; `architecture/config.md` § Validation Rules; `docs/features.md` § PDF Extraction; `docs/config.md` § Fetch Defaults; `src/fetch/pdf.rs`; `src/fetch/types.rs`; `src/fetch/client.rs`; `src/fetch/limits.rs`; `src/core/config.rs`; `fuzz/fuzz_targets/extract_pdf_text.rs`; `tests/corpus/adversarial/pdf_extended.json`.
Dependencies: none; independent of ADR-0007 and ADR-0008.

## Objective

Give PDF ingestion a hard, measured ceiling on how many bytes an untrusted document may expand into before any of that expansion is trusted, so a decompression bomb is refused by a configured bound instead of by the allocator. The ceiling is proven before `lopdf` sees the document, is deterministic for identical bytes and configuration, and leaves every accepted document's extraction byte-identical.

## Non-goals

- No extraction, reading-order, column, table, ligature, chunking, focus, or payload change: those are M003, M004, and M005.
- No change to `pdf_enabled`, `pdf_max_pages`, `pdf_max_chars_per_page`, `pdf_max_total_chars`, `max_bytes`, timeouts, or deadlines. The PDF byte budget is M001's.
- No new MCP tool, argument, provider, or renamed wire string; the tool surface stays at ten.
- No `renderer_version` bump for the byte-bound itself: accepted documents extract identically. The `/Encrypt` narrowing does change an outcome, so it carries the bump and its isolation test (see compatibility).
- No move of extraction onto a blocking pool. That is a liveness change, not an allocation bound, and the byte bound below already caps the CPU a document can demand.
- No OCR, rasterization, native library, second parser, or vendored/patch of `lopdf`.

## Current implementation evidence

Verified against `lopdf` 0.42.0 (the exact `Cargo.lock` pin, `default-features = false`) and this baseline's source:

- **Inflation has no output cap.** `Stream::decompressed_content` dispatches to `decompress_zlib`, which drives `flate2`'s `ZlibDecoder` through `read_to_end` into a growing `Vec` (seeded at twice the compressed length, then unbounded), and to `decompress_lzw`, which drives `weezl`'s `decode_all` into a growing `Vec`. `decode_ascii85` is the only filter whose output is bounded by its input. No filter path accepts, or honours, an output limit.
- **Which streams are inflated on our paths.** Xref streams during `Document::load_mem` (`decode_xref_stream`), object streams during the same load (`ObjectStream::new`), and page content streams during `Document::extract_text` (page content streams are concatenated into one `Vec` before decoding). Font and CMAP dictionaries are read as objects and are not inflated. `load_mem` loads every xref entry eagerly, so an inflated xref or object stream multiplies into per-object allocation.
- **The byte cap bounds compressed input only.** `FetchClient` accumulates the response through `append_bounded` against `max_bytes` (2,000,000 by default) and hands that `&body` to `extract_pdf_text`; every expansion above happens afterwards, outside any cap.
- **Extraction is neither offloaded nor timed out.** `extract_pdf_text` is called inline from the PDF branch of the fetch client, on the async task. `config.fetch.timeout_ms` configures transport-level connect/read/pool timeouts; the only wall-clock deadline in the fetch tool guards the browser path. A deadline here would stop waiting at best and would not cancel a synchronous parse, so it bounds neither time nor allocation.
- **Guards `lopdf` already enforces.** Parser nesting depth 100 and a literal-string bracket cap of 100, a reference-chase limit of 128, a visited set on xref `/Prev` chains, a page-tree depth of 256 with an iterator limit equal to the object count, and explicit cycle rejection on the resource-parent walk.
- **Guards `lopdf` does not enforce.** Inflated output, total object count, and xref stream field geometry: `decode_xref_stream` allocates its row buffers as `vec![0u8; /W[i]]` and rejects only a shorter-than-three array or a negative width, so a document well under the response cap can request a multi-gigabyte allocation before a single entry is read.
- **Bombs are cheap to write.** A deflate stream of repeated bytes compresses to roughly one thousandth of its output, so a few kilobytes inside the 2 MB cap currently expand without limit.
- **Authentication is not decryption.** `src/fetch/pdf.rs` calls `Document::authenticate_password`, which validates a password and returns; `Document::decrypt` is the only call that replaces ciphertext with plaintext and is never invoked. An `/Encrypt` document therefore reaches the extractor undecrypted, and `pdf_password` has never produced plaintext. Ciphertext also cannot be measured by a pre-parse scan.

## Invariants that cannot regress

- Every existing bound stays in force: URL length, response bytes, redirects, per-page characters, total characters, page count, outline entries and depth, page-label expansion, timeouts. The new bound is additive to all of them.
- Determinism: identical bytes plus identical configuration produce the identical outcome, measured total, extracted text, chunk IDs, and ordering. The scan is a single forward pass over the buffer with no hash iteration.
- One parser: `lopdf` 0.42.0 stays the only PDF parser, no source patch, no vendor copy, no second parser; `make dependency-policy` and the all-features MSRV check stay green.
- All inflation in production PDF code goes through one bounded function; no unbounded `read_to_end`/`decode_all` call appears in `src/fetch/pdf.rs`.
- Sanitization, control-character stripping, trust markers, the `untrusted` warning, and the FNV-1a identity rules are unchanged.
- Accepted documents keep the exact page, block, chunk, quality, and warning output they produce today; the ten-tool surface and every existing wire string are unchanged.
- Extraction output is cached. The `/Encrypt` narrowing changes an outcome, so this milestone bumps the derived cache `renderer_version` and proves a pre-bump entry is not served.
- Per-page extraction state stays bounded: because every inflated byte is budgeted, the per-page text and the page-content concatenation are bounded by the document budget that the existing character caps then truncate.

## Expected production changes

1. **Measured pre-parse scan.** Add a bounded stream scan in `src/fetch/pdf.rs` (`pub fn scan_stream_budget(bytes, budget) -> Result<usize, FetchError>`) that runs as the first statement of `extract_pdf_text`, before `Document::load_mem`. It walks the raw buffer for `stream`/`endstream` regions using the declared `/Length` with an `endstream` fallback, reads `/Filter` and `/DecodeParms` from each stream dictionary, and inflates the Flate stages through a reader capped at `remaining + 1`, stopping the whole document at the first breach.
2. **Filter completeness.** `FlateDecode` is measured; `ASCII85Decode` contributes at most four fifths of its input and needs no decoder; any other filter name contributes zero because `decompressed_content` returns `Unimplemented` for it and `get_page_content` then falls back to the raw, input-bounded bytes. `LZWDecode` is the exception: `lopdf` inflates it through `weezl`, which reaches us only through `lopdf`, so measuring it would mean owning and testing a second inflate path. A stream using it is refused instead of inflated unmeasured.
3. **Declared geometry.** In the same scan, refuse an xref stream whose declared `/W` width exceeds eight bytes or whose widest declared row buffer exceeds the remaining budget, because that allocation is requested before any entry is read.
4. **Budget.** Per stream the limit is `min(configured, remaining document allowance)`; per document it is the configured total. One number bounds both, so no distribution across streams can evade it.
5. **Config.** New `[fetch].pdf_max_decompressed_bytes`, default 67,108,864 bytes (64 MiB: comfortably above the largest baseline document, and four times the `pdf_max_bytes` default M001 proposes), validated range 1,048,576 to 1,073,741,824, with a validation error naming the key. It reaches `FetchLimits`, then `PdfLimits`, then the scan; every `PdfLimits` literal in the crate, `tests/adversarial_corpus.rs`, and `fuzz/fuzz_targets/extract_pdf_text.rs` gains the field.
6. **One additive outcome.** `FetchError::PdfDecompressionBoundExceeded { measured: usize, budget: usize }` and the matching `FetchErrorKind` rendering as `pdf_decompression_bound_exceeded`, covering measured expansion above the budget, declared geometry above it, and an unmeasurable `LZWDecode` stream. No existing wire string is renamed and `pdf_parse_error` keeps its current meaning.
7. **Encrypted documents.** Refuse every document that declares `/Encrypt` with the existing `pdf_encrypted`, whether or not a caller password is present, and stop calling `authenticate_password`. Two facts force this: ciphertext cannot be measured by a pre-parse scan, and authentication is not decryption, so the current call validates a password and then reads undecrypted bytes. Refusing converts an unreliable outcome into an explicit one and closes the bypass. `pdf_password` stays in the schema for wire compatibility and is documented as unsupported until a later decision implements decryption together with a post-decrypt bound.
8. **Dependency shape.** `flate2` gains an optional, `pdf`-feature-scoped production declaration (`dep:flate2`, `default-features = false`, `rust_backend`) and stays in `[dev-dependencies]` for the existing test helpers. It is already in the lock file, so no crate is added to the graph and no C toolchain is introduced.

## Storage, protocol, and compatibility effects

- One additive config field with a documented default and range; configurations without it keep working and receive the default budget.
- One additive error kind and wire string; every existing PDF and fetch string is unchanged, and the breach is a structural-break outcome, never an empty success.
- Response shape, arguments, page selection, blocks, chunks, and quality fields are untouched.
- No migration. `renderer_version` moves because the `/Encrypt` outcome changes: a cache entry holding ciphertext-derived text for an encrypted URL must not survive. Accepted documents re-extract identically, and the isolation test proves a pre-bump entry is not served.
- One deliberate behaviour narrowing: a document declaring `/Encrypt` returns `pdf_encrypted` even when a password is supplied. At baseline such a document yields a parse error or ciphertext-derived text rather than plaintext, because the authentication call never decrypts; the narrowing replaces an unreliable outcome with an explicit one and removes the unmeasured path. `pdf_password` remains accepted but unsupported, and the reason is documented.

## Ordered work packages

1. Add the bounded scanner and its unit tests in `src/fetch/pdf.rs`: cap honouring, `remaining` accounting, chunked inflate that never buffers more than `cap + 1`, `endstream` fallback, filter chains, and the `/W` geometry check.
2. Add `PdfDecompressionBoundExceeded` to `src/fetch/types.rs` with its kind, wire string, and message; assert the mapping.
3. Wire the budget through `FetchLimits`, `PdfLimits`, and the fetch client's PDF branch; update the config struct, default, validation, and `docs/config.md`.
4. Call the scan first in `extract_pdf_text` and map a breach to the new outcome before any `lopdf` call; assert no page metadata, text, or blocks are produced on breach.
5. Replace the `authenticate_password` block with an `is_encrypted()` refusal that runs before any page access, cover the `/Encrypt`-without-password, `/Encrypt`-with-password, and non-encrypted paths, document `pdf_password` as unsupported, and bump `renderer_version` with its isolation test.
6. Add the bomb shapes to `tests/adversarial_corpus.rs`, built at test time with `flate2` and a small configured budget so no fixture allocates the bomb's expansion.
7. Extend `tests/property_fetch_limits.rs` with the allocation property and `fuzz/fuzz_targets/extract_pdf_text.rs` with the same assertion.
8. Update `architecture/fetch.md`, `architecture/config.md`, `docs/features.md`, and the per-suite counts in `docs/test-inventory.md`.

## Verification

- `cargo test --locked --all-features --lib fetch::pdf::` — scanner unit tests and the existing extraction tests.
- `cargo test --locked --all-features --test adversarial_corpus` — bomb shapes, corpus cases, and the extractor corpus test.
- `cargo test --locked --all-features --test property_fetch_limits` — the bound property over arbitrary bytes.
- `cargo test --locked --all-features --test web_fetch_integration --test fetch_safety` — end-to-end PDF fetches and the unchanged safety envelope.
- `cargo test --locked --all-features --test config_validation --test docs_config_snippets` — key default, range, and documented snippet.
- `cargo test --locked --all-features --test static_guards --test docs_tool_names` — the new inflate guard and the untouched tool surface.
- `make check`; `make fuzz-smoke`.

## Static guards and documentation updates

- `tests/static_guards.rs` gains a fail-closed guard: non-test `src/fetch/pdf.rs` contains no `decompressed_content` call and no `.decompress(` call, so inflation can only enter through the bounded scanner.
- `src/fetch/types.rs` documents the new variant next to `PdfParseError`; `architecture/fetch.md` § 7 owns the bound, the scan, and the refusal set, and § 11 owns the error taxonomy row; `architecture/config.md` § Validation Rules owns the range.
- `docs/features.md` § Limits states the decompressed-byte budget, the per-document meaning, and the breach outcome; `docs/config.md` § Fetch Defaults gains the row with its default and range.
- No new test suite: tests extend existing behavioral suites, so `architecture/testing.md` is untouched; only the per-suite counts in `docs/test-inventory.md` change.

## Acceptance and stop conditions

- A content-stream bomb whose deflate output is eight times the configured budget is refused with `pdf_decompression_bound_exceeded`, and the scan's own measured total never exceeds the budget by more than one inflate block.
- The same holds for an object-stream bomb, an xref-stream bomb, a chained-filter bomb, and an `/W` width bomb.
- An `LZWDecode` stream is refused with the same kind rather than inflated unmeasured.
- The two baseline documents of 2,215,244 and 2,858,365 bytes, and the existing PDF extraction tests, still extract identically under the default budget, with their measured totals recorded.
- A document declaring `/Encrypt` returns `pdf_encrypted` with and without a caller password, and no page content is read from it; a non-encrypted document is unaffected.
- `renderer_version` is bumped and a pre-bump cached entry for an encrypted URL is demonstrably not served.
- No wire string is renamed, no tool or argument is added, and `tests/docs_tool_names` is untouched and green.
- Stop if the bound cannot be enforced without vendoring or patching `lopdf`, or without a second parser: require an ADR rather than shipping a fork.
- Stop if the only available mechanism is a wall-clock deadline, because that does not bound allocation. Escalate instead of documenting a deadline as a bound.
- Stop if a dependency beyond the already-present `flate2` is needed to measure the filters this plan measures; escalate the filter instead of weakening the bound.

## Closure evidence

`plans/closure/pdf-ingestion-fidelity/007-status.md` records the candidate SHA, each bomb shape with its compressed and claimed expanded size, the configured caps used in CI, the observed outcome kind per shape, the scan's measured totals and high-water figures proving no test allocated the full expansion, the `/W` and `LZWDecode` refusals, the encrypted-document outcome matrix with the `renderer_version` bump and its isolation test, the recorded fact that the fetch deadline does not cover extraction and is therefore not the bound, and the unchanged-cap evidence for the two baseline documents plus the full extraction suite.
