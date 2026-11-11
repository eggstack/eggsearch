# Optional Extraction Comparison Matrix

This report records the source-build comparison for M002. The offline tests use controlled HTTP fixtures and generated synthetic PDF content. Browser execution is classified as limited because this environment has no Chrome/Chromium executable and the production browser policy rejects loopback fixtures. No production SSRF relaxation or external test origin was used.

This matrix is a build-feature and controlled-fixture comparison, not the PDF extraction-fidelity authority. Fidelity is measured by `tests/pdf_fidelity.rs` against the eight-class synthetic corpus in `tests/fixtures/pdf/corpus.json`, with per-case line-shape, intact-word, table-adjacency, ordered-passage, per-page-classification, running-head, and outline assertions plus a join-lines-and-strip-spaces negative control. Real-document measurement is the ignored recorder into `target/pdf-corpus/`. Suite registration is in `architecture/testing.md` and counts are in `docs/test-inventory.md`.

## Feature arms

| Arm | Build command | Runtime | Release executable | Outcome |
|---|---|---|---:|---|
| HTTP only | `cargo build --locked --release --no-default-features` | PDF disabled; HTTP transport | 20.5 MiB (rounded by `ls -lh`) | pass for static HTTP; PDF gives the existing unavailable/disabled outcome |
| PDF | `cargo build --locked --release --features pdf` | `fetch.pdf_enabled=true` in the PDF fixture | 21.8 MiB | pass for generated searchable text PDF and PDF metadata propagation |
| Browser | `cargo build --locked --release --features browser` | browser policy is feature-gated; Chrome absent | 25.4 MiB | limited: build and deterministic policy/classifier tests pass; real rendering skipped |
| PDF + browser | `cargo build --locked --release --features 'pdf browser'` | both compile-time features; PDF runtime explicitly enabled by the PDF test | 26.8 MiB / 28,081,112 bytes | pass for combined-feature batch behavior; browser rendering limited |

Feature fingerprints are the four exact Cargo feature sets above. The combined build's `--version` startup was below the 0.01 s resolution of `/usr/bin/time`, with 6,912 KiB maximum RSS for that short CLI process. This is a version-command observation, not a cold fetch or browser-startup measurement. Size figures are stripped release executables on this Linux x86_64 host; they do not include Chrome or external extensions.

Optional dependency versions resolved by `Cargo.lock`: `lopdf 0.42.0` and `chromiumoxide 0.9.1`.

## Controlled case classification

| Case | HTTP only | PDF | Browser | PDF + browser |
|---|---|---|---|---|
| Static HTML fetch/extraction | pass | pass | supported by policy/classifier; render unrun | pass on HTTP path |
| PDF magic/type and disabled behavior | pass: feature unavailable | pass: runtime enabled | pass: feature absent means no PDF extraction | pass: runtime enabled |
| Searchable text PDF and metadata | unsupported by design | pass: generated one-page PDF, expected text and metadata propagated | unsupported by design | pass: covered by PDF feature suite |
| PDF page/quality classification and bounds | feature unavailable | pass: 57 PDF module unit tests | feature unavailable | pass via same compiled PDF code |
| JS shell / inserted or late DOM | HTTP-only limitation | HTTP-only limitation | limited: deterministic shell classification passes; no Chrome | limited: same browser limitation |
| Redirects, verification, interactive wall, mixed content, malformed encoding, injection bait | HTTP fetch contract tests pass where applicable; not all were paired as extraction truth cases | same HTTP path | limited: no controlled render fixture | limited: no controlled render fixture |
| Browser private-target denial / public-only policy | n/a | n/a | pass: policy tests, including loopback/private-target rejection | pass: same policy tests |
| Cold/warm browser latency, JS passage recall, CPU/RSS, time-to-first-useful-text | not applicable | not measured as a paired benchmark | skip: no Chrome | skip: no Chrome |

The generated PDF check asserts the expected text `Hello from PDF`, `kind=pdf`, byte counts, source extension and redirect count. At the time of this comparison the PDF module suite contributed 57 passing unit tests for page selection, limits, metadata and quality classifications; current counts live in `docs/test-inventory.md`. OCR is not provided; image-only/scanned pages remain explicitly low-quality or unsupported. This report does not claim broad multi-page table, columns, outline, corrupt-CID or scan fixture recall; those shapes are measured by `tests/pdf_fidelity.rs`, not by this matrix.

## Verification

- `cargo test --locked --no-default-features --test web_fetch_integration`: 70 passed.
- `cargo test --locked --features pdf --test web_fetch_integration`: 72 passed.
- `cargo test --locked --lib --features pdf fetch::pdf::`: 57 passed.
- `cargo test --locked --features browser --test browser_transport --test browser_profiles`: 98 passed.
- `cargo test --locked --features 'mock pdf browser' --test batch_fetch_retrieval`: 13 passed. `mock` is required because this suite uses the mock engine; omitting it fails compilation.
- HTTP-only, PDF, browser and combined release build commands above all succeeded.

No extractor behavior, default feature, public tool, or security policy changed. Detailed paired quality and resource figures for real JS rendering remain unmeasured and are not represented as passes.

## Representation negotiation A/B (agentic retrieval integrity M003)

M003 added default-on `Accept: text/markdown;q=1.0, text/html;q=0.9, */*;q=0.1`
for HTML-ish targets. The controlled A/B serves one logical document in both
representations from `httpmock` and fetches it through `run_web_fetch`:

| Representation | Body chars | Extracted block chars | Approx tokens (chars/4) | Headings | Table rows | Code lines | Links | Focus-chunk hit |
|---|---:|---:|---:|---:|---:|---:|---:|---|
| markdown | 356 | 251 | 63 | 2 | 3 | 4 | 2 | yes |
| html | 560 | 251 | 63 | 2 | 3 | 4 | 2 | yes |

The Markdown body is 36.4% smaller while extracted text, headings, table cells,
code lines, link URLs, and the focus chunk are identical. Tests assert
`markdown_bytes < html_bytes`, `markdown_block_chars <= html_block_chars`, the
exact table rows and code lines, outline equality, and link-URL equality, so a
Markdown-path fidelity regression fails rather than passing silently.

A curl-level spot check of two Cloudflare-fronted origins observed
`text/markdown` for the Markdown policy (7,031 and 18,334 bytes) versus
`text/html` for the HTML policy (213,421 and 317,846 bytes), with
`vary: accept-encoding, accept`. That comparison is not an end-to-end Eggsearch
measurement and the HTML bodies include scripts, styles, and navigation, so it
overstates content savings. It also observed no `x-markdown-tokens` header from
either origin. Table and code-block fidelity on the negotiated path is
evidenced by the controlled fixture, not by the spot check.
