# M002 Optional PDF and Browser Extraction Matrix — Closure Status

Status: closed

Source plan: `plans/implementation/deterministic-retrieval-quality/002-pdf-browser-extraction-matrix.md`

Roadmap: `plans/subsystems/deterministic-retrieval-quality-roadmap.md`

Implementation candidate: `25f4910cdf68acf513a4f46e125c19e8dc84c5a8`

Report: `docs/retrieval-quality-extraction-matrix.md`

## Outcome

All four source feature arms built: HTTP-only, PDF, browser, and PDF plus browser. The PDF arm passes generated searchable-text and metadata checks and 57 PDF module unit tests. Browser build, classifier, request-policy, profile and transport orchestration tests pass. The combined feature batch suite passes with the required `mock` flag.

The real browser-rendering arm is limited, not passed: the host has no Chrome/Chromium executable, and the production browser policy rejects loopback targets. No safe fixture seam existed, so no local browser fetch was attempted and no SSRF policy was weakened. Browser-rendered recall, p50/p95 fetch time, browser startup, and paired CPU/RSS remain unmeasured. A provisioned Chrome host remains an M006 operational qualification item.

## Measurements and fixtures

- Release executable sizes: HTTP-only 20.5 MiB, PDF 21.8 MiB, browser 25.4 MiB, combined 26.8 MiB (rounded host output); combined exact size 28,081,112 bytes.
- Combined `--version` startup was below `/usr/bin/time` 0.01 s resolution; maximum RSS was 6,912 KiB. This is not fetch latency.
- Generated one-page PDF truth checks `Hello from PDF`, document kind, byte counts, source extension and redirect count. Existing PDF unit coverage: 57 passing cases.
- HTTP-only fetch suite: 70 passed. PDF-enabled fetch suite: 72 passed. Browser transport/profile suites: 98 passed. Combined-feature batch fetch suite: 13 passed.
- Release builds succeeded for all four feature sets. No default feature, public schema, Python wheel or production fetch behavior changed.

## Safety and compatibility

Existing browser public-network/private-target checks, profile behavior, and PDF runtime enablement and caps remain unchanged. Browser tests were not represented as real rendering passes. OCR, CAPTCHA handling, arbitrary crawling, default-feature changes and new release SKUs were not introduced.

## Integrated verification

- `make check`: passed, including formatting, clippy, no-default check, all-feature tests, hygiene, dependency policy and packaging checks.
- `make bench-check`: passed.
- `make docs-check`: passed.

Closure is accepted as completion of the comparison and reproducible source-feature evidence. The explicit browser operational limitation is carried forward to M006.
