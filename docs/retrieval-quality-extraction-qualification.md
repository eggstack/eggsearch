# M007 paired extraction and resource qualification — 001

Status: paired feature-arm observations complete on macOS; independent hosted
Linux, macOS, and Windows qualification of the final merged candidate remains
required before M007 closure.

## Candidate and fixtures

All four Eggsearch binaries were built from source `28545de1fa899281f41980f4de31046df3431f22`
with the release profile (opt-level 3, thin LTO, one codegen unit, stripped).
The same four original synthetic fixtures were served from one ephemeral public
origin through a temporary Cloudflare quick tunnel. The tunnel and local fixture
server were stopped after the trials. No Eggsearch network policy or request
headers were changed. The origin, fixture hashes, Chrome version, and exact
binary identities are in `plans/closure/retrieval-quality-evidence-extension/evidence/extraction-fixture-origin-001.json`.

Three cold cache-bypass and warm default-cache pairs were run per fixture and
feature arm using the same public URL, 30-second tool timeout, and diagnostic
response detail. The HTTP-only, PDF-only, browser-only, and combined feature
arms used the explicitly recorded per-arm TOML configuration fingerprints.
Per-call observations are in the four `extraction-paired-*-001.json` files in
the evidence directory.

## Results

Each supported marker row represents six calls (three cold and three warm).
“Recovered” means the expected synthetic marker was present in extracted text.
PDF-disabled errors and the image-only PDF classification are retained as
bounded outcomes.

| Arm | Binary size | Static HTML | JS-only HTML | Text PDF | Scanned PDF | Sampled peak process-tree RSS |
|---|---:|---|---|---|---|---:|
| HTTP only | 19,310,288 B | 6/6 recovered | 0/6; HTTP did not execute JavaScript | PDF disabled, 6/6 | PDF disabled, 6/6 | 23,768 KiB |
| PDF | 20,659,408 B | 6/6 recovered | 0/6; HTTP did not execute JavaScript | 6/6 recovered | 6/6 `pdf_no_extractable_text`; OCR unavailable | 24,096 KiB |
| Browser | 23,871,336 B | 6/6 recovered | 6/6 recovered with Chrome | PDF disabled, 6/6 | PDF disabled, 6/6 | 1,216,844 KiB |
| PDF + browser | 25,216,504 B | 6/6 recovered | 6/6 recovered with Chrome | 6/6 recovered | 6/6 `pdf_no_extractable_text`; OCR unavailable | 1,202,784 KiB |

Median cold/warm complete-tool-response latency in milliseconds:

| Arm | Static HTML | JS-only HTML | Text PDF | Scanned PDF |
|---|---:|---:|---:|---:|
| HTTP only | 504 / 26 | 447 / 26 | 416 / 432 | 417 / 436 |
| PDF | 625 / 26 | 540 / 33 | 466 / 30 | 538 / 602 |
| Browser | 513 / 31 | 2,542 / 28 | 482 / 492 | 485 / 442 |
| PDF + browser | 572 / 27 | 2,594 / 28 | 486 / 26 | 704 / 574 |

Warm rows are cache hits where a response was cacheable; errors have no cache
status. `web_fetch` does not expose a streaming extraction event, so the
complete response duration is only an upper bound on time to first useful text.
RSS is sampled for the whole Eggsearch process tree at 100 ms intervals and is
reported as the maximum observed within each feature-arm run. Browser subprocesses
can persist across calls, so these arm-level maxima are not per-request or
per-case attribution. They are environment observations, not thresholds.

## Qualification limits

The host was macOS 26.6.2 x86_64 with Google Chrome 156.0.8078.48. Browser
configuration used the explicit Chrome executable path because automatic
browser discovery did not find this installation. A first attempt through a
different temporary tunnel returned that tunnel provider's interstitial to
Chrome; those observations were excluded. The qualified runs used the same
Cloudflare-tunneled synthetic origin for all arms. The image-only PDF contains
no text layer; the expected bounded result is no extracted text and no OCR.

No extraction, ranking, or feature-default behavior was changed. Hosted
cross-platform CI on the final merged candidate remains a separate closure gate.
