# Optional Extraction Comparison Matrix

This report records the source-build comparison for M002. The offline tests use controlled HTTP fixtures and generated synthetic PDF content. Browser execution is classified as limited because this environment has no Chrome/Chromium executable and the production browser policy rejects loopback fixtures. No production SSRF relaxation or external test origin was used.

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

The generated PDF check asserts the expected text `Hello from PDF`, `kind=pdf`, byte counts, source extension and redirect count. The PDF module suite adds 57 passing unit tests for page selection, limits, metadata and quality classifications. OCR is not provided; image-only/scanned pages remain explicitly low-quality or unsupported. This report does not claim broad multi-page table, columns, outline, corrupt-CID or scan fixture recall.

## Verification

- `cargo test --locked --no-default-features --test web_fetch_integration`: 70 passed.
- `cargo test --locked --features pdf --test web_fetch_integration`: 72 passed.
- `cargo test --locked --lib --features pdf fetch::pdf::`: 57 passed.
- `cargo test --locked --features browser --test browser_transport --test browser_profiles`: 98 passed.
- `cargo test --locked --features 'mock pdf browser' --test batch_fetch_retrieval`: 13 passed. `mock` is required because this suite uses the mock engine; omitting it fails compilation.
- HTTP-only, PDF, browser and combined release build commands above all succeeded.

No extractor behavior, default feature, public tool, or security policy changed. Detailed paired quality and resource figures for real JS rendering remain unmeasured and are not represented as passes.
