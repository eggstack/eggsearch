# M002 — Optional PDF and Browser Extraction Comparison Matrix

Status: closed
Source: `plans/subsystems/deterministic-retrieval-quality-roadmap.md` M002
Primary class: infrastructure; feature-specific capability evaluation
Baseline: `7ad8e81b2d8f7036d35dcb2772a1778c73d08c3b`
References: `plans/000-long-term-specification.md` ``4, 6-9; ADR-0002, ADR-0006; `docs/features.md`; `architecture/fetch.md`; `architecture/hardening.md`.
Dependencies: hard M001 (closed at `310611b733c2067ddeab184618156ef072934a6e`); operational system Chrome/Chromium for real browser test. Upstream chromiumoxide dependency cleanup in repository-hardening M006 is independent and **not a blocker** for testing existing browser functionality.

## Objective

Establish meaningful extraction-quality, cost, security and footprint comparisons for default HTTP, optional PDF, optional browser and their combination, without deleting or de-prioritizing these valuable existing features. Distinguish compile-time availability, runtime enablement, requested transport, transport actually used, extraction quality and failure class.

## Current evidence

`src/fetch/pdf.rs` uses `lopdf` text extraction without OCR; `docs/features.md` documents 25-page, 12k/page and 50k-document caps with per-page quality flags. `src/fetch/browser/` uses optional `chromiumoxide`; HTTP-first `auto` escalation handles JS shells, but does not solve CAPTCHA/login challenges. `tests/browser_transport.rs`, `tests/browser_live_smoke.rs` and PDF extraction/fuzz coverage exist. Root `Cargo.toml` has `default = []` with `pdf` and `browser` opt-ins; Python wheels deliberately omit optional features.

## Work packages

1. Define **four comparable build/runtime arms**: default HTTP-only; `cargo test --locked --features pdf` and `fetch.pdf_enabled=true`; `--features browser` and enabled render policy; `--features "pdf browser"` with both enabled. Keep `egress` out of the quality arms; run `--all-features` only as separate regression qualification. Record binary/extension size and feature fingerprint so tests never compare mismatched builds.
2. Add controlled, bounded local HTTP fixture server for static HTML, document links, CSR/JS DOM insertion, late JS content, redirect chains, non-interactive verification shell, interactive login wall, mixed content, malformed encodings, and script prompt-injection bait. Browser-network policy must remain public-only; local loopback testing needs an explicit test harness seam or approved public test origin, **not** relaxation of production SSRF checks. If safe controlled browser fixture access cannot be established, report the browser-live arm blocked/limited; do not mark it passed.
3. Provide synthetic/licensed PDFs: searchable text, tables/columns, metadata/outline, page selection, sparse/CID-corrupt, scanned/image-only and oversized/long documents. Ground truth by expected words, page references, ordering where semantically defined, extracted body completeness, and quality classification. Do not treat OCR absence as a broken text extractor; record unsupported images distinctly.
4. Measure paired relevant passage recall@budget, content loss, correctness of `transport`/`browser_escalated`/`truncated`/`pdf` and error codes, time-to-first-useful-text, p50/p95 fetch time, CPU/RSS, startup cost, executable size and local warm/cache performance. Separate cold/warm results and PDF-only, JS-only, and mixed batches. Preserve `focus` query and matching `max_chars` across all arms.
5. Retain browser session, origin, and auth/profile isolation tests; assert Chrome is never downloaded, launch failure is explicit, interactive verification not solved, browser requests bounded/intercepted, dynamic private destinations fail closed, PDF page/char bounds and MIME validation preserved. Run one resource-aware native Linux or macOS workflow where Chrome exists; Windows correctness smoke if practical; do not compile or install optional builds on SBCs as part of this work.
6. Publish comparison report and attributable failure cases. If improvements to specific extractors are warranted, record small corrective work as separate milestones; **do not** broaden into OCR, arbitrary website crawling, CAPTCHAs, or changing published default/Python packages here.

## Compatibility and ownership

No new public tool or default feature, no second release SKU, no required headless browser or optional PDF dependencies for core. Continue using `web_fetch`/`batch_fetch` input/output, cache partitioning and `eggfetch` transport; browser CDP is an optional fetch transport, not an autonomous browsing agent. Add only test-owned fixture paths and additive diagnostics if proven necessary and compatibility-qualified.

## Tests and qualification

- `cargo test --locked --no-default-features --test web_fetch_integration`.
- `cargo test --locked --features pdf --test web_fetch_integration` plus new PDF truth suite.
- `cargo test --locked --features browser --test browser_transport --test browser_profiles` plus new browser fixture suite.
- `cargo test --locked --features "pdf browser" --test batch_fetch_retrieval` plus combined matrix suite.
- `cargo test --locked --all-features`, `make check`, `make bench-check`, `make docs-check`; explicit live browser smoke under opt-in only.
- Verify feature absence: disabled features produce the established clear unavailable outcome and cannot silently change `http_only` behavior.

## Acceptance, stop and closure

All four arms have per-case pass/fail/unsupported/skip classification with reference passages and bounded latency/memory records. Browser tests are not reported as passes when Chrome/network fixture support is absent. Existing security checks and default build behavior are unchanged, and a source-built PDF/browser consumer can reproduce the matrix. Stop on SSRF relaxation, profile leakage, unbounded execution or pressure to add OCR/default SKU. Closure `plans/closure/deterministic-retrieval-quality/002-status.md` includes exact source/build features, browser/PDF version, fixtures, numerical deltas, regression tests and limitations.

## Outcome

Closed against candidate `25f4910cdf68acf513a4f46e125c19e8dc84c5a8`. The four feature builds and controlled HTTP/PDF/browser-policy suites are recorded in `docs/retrieval-quality-extraction-matrix.md`. Browser rendering is limited because no Chrome/Chromium executable or safe local fixture seam exists in this environment; no external origin was used and production SSRF checks were not weakened. M006 retains the operational browser/live-reference qualification.
