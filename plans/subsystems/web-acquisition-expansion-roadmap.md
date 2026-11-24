# Web Acquisition Expansion Roadmap

Status: proposed (M001 and M002 ready; M003, M004, and M005 proposed)
Planning baseline: `d932115d1f86af51710747d243455792ff63a6fb` (`docs/user-facing-polish-20261123`)
Owner: fetch extraction, discovery, and document formats

References: `plans/000-long-term-specification.md` §§1-4; `plans/001-terminology-and-domain-model.md` §4-5; ADR-0001, ADR-0002, ADR-0006 (keyless deterministic retrieval), ADR-0011 (document addressing), ADR-0014 (render bounds), ADR-0022 (nine tools); `architecture/fetch.md`; `docs/tool-matrix.md`.

## 1. Purpose and ownership boundary

Widen what one bounded fetch call can obtain from an explicit URL, without
adding tools, models, or storage the operator did not enable. The workstream
owns selector-scoped extraction, CSS attribute extraction, cache-only fetch
semantics, same-origin site URL discovery, deterministic change comparison,
non-HTML document formats, and deterministic instruction-signal reporting.

It does not own search providers, ranking, the nine-tool surface, browser
interaction, crawling and fetching of discovered URLs, or scheduling.

## 2. Work classification

- **Invariant:** exactly nine advertised tools; bounded bodies and deadlines;
  SSRF and redirect rules unchanged; stable content-derived IDs; no model call
  inside the server; fetched text stays `external_untrusted`; cache identity
  covers every extraction-affecting option.
- **Capability:** selector scoping and attributes; cache-only fetch; site URL
  discovery; change comparison; additional document formats; instruction-signal
  reporting.
- **Infrastructure:** sitemap and robots parsing, a durable snapshot store, a
  format-detection and extraction module set, and a screening corpus.
- **Polish:** diagnostics, documentation, and payload-economy measurements.

## 3. Non-goals

- Recursive crawling, following discovered links, or fetching a sitemap's URLs.
- JS-rendered link discovery; browser interaction; full-page screenshots;
  page-to-PDF export (deferred, see §8).
- OCR, image captioning, or layout-block reconstruction.
- Model-based JSON extraction, summaries, branding, product extraction, or
  answer synthesis.
- Monitoring, scheduling, webhooks, or notifications.
- A tenth tool or a new binary SKU.

## 4. Current state

- `fetch` is contractually "not for search, crawling, or following links"
  (`src/mcp/tool_contract.rs:169`); `include_links` returns links from the one
  fetched document.
- Extraction is whole-document: `ExtractMode` is text, markdown, or
  metadata_only; `focus` performs deterministic query-driven chunk selection
  over the extracted document and never traverses links.
- `FetchCachePolicy` is default, bypass, or refresh; there is no cache-only
  mode and therefore no provable no-network fetch.
- `robots.txt` is not consulted; `[search].live.respect_robots_txt` is reserved
  and explicitly not applied (`src/mcp/state.rs:318`).
- `scraper = "0.20"` and `quick-xml = "0.41"` are already dependencies;
  sitemap parsing needs no new crate.
- PDF extraction is text-only: 16 MiB body, 64 MiB decompressed, 25 pages,
  12k chars per page, no OCR (`docs/features.md` § PDF Extraction).
- `ExtractionCacheKey` carries extract mode, budget class, links, PDF pages and
  OCR policy, media, `renderer_version` (currently 5), and sanitisation.
- No durable content snapshot store exists; the fetch cache is HTTP-freshness
  based and in-memory.

## 5. Target architecture

```text
core/fetch.rs          ExtractionScope, CacheOnly policy, SiteMapDocument, ChangeReport
fetch/scope.rs         selector validation and DOM scoping (scraper)
fetch/attributes.rs    bounded CSS attribute extraction
fetch/site_map.rs      robots -> sitemap index -> sitemap (quick-xml) -> link fallback
fetch/snapshots/       opt-in durable store (M003, gated by ADR)
fetch/documents/       format detection and per-format extraction (M004, feature-gated)
fetch/instruct.rs      deterministic instruction-signal detection (M005)
```

All of it surfaces through the existing `fetch` web item. No tool is added.

## 6. Dependency graph

```text
M001 selectors + cache-only ── interface ──> M003 change comparison (scope in snapshot key)
M002 site discovery ────────── soft ───────> M003 (a changed page set becomes expressible)
ADR for derived-state persistence ── hard ──> M003
M004 documents ─────────────── independent ──> M005 (screening runs over its text too)
M005 screening ─────────────── independent
```

## 7. Milestones

| Milestone | Class | Status | Handoff | Dependencies | User-visible exit |
|---|---|---|---|---|---|
| M001 — Selector scoping, attributes, and cache-only policy | capability | ready | `plans/implementation/web-acquisition-expansion/001-selector-scoping-and-cache-only-policy.md` | None | A caller narrows extraction to chosen DOM regions, reads named attributes, and can demand a provably offline fetch |
| M002 — Bounded site URL discovery | capability | ready | `plans/implementation/web-acquisition-expansion/002-bounded-site-url-discovery.md` | None | One call returns a deterministic, same-origin URL inventory with explicit provenance and truncation |
| M003 — Deterministic change comparison | capability | proposed | `plans/implementation/web-acquisition-expansion/003-deterministic-change-comparison.md` | Hard: ADR for derived-state persistence; interface: M001 | A second fetch reports new/same/changed/removed plus a bounded deterministic diff |
| M004 — Document formats beyond PDF | capability | proposed | `plans/implementation/web-acquisition-expansion/004-document-formats-beyond-pdf.md` | None (feature-gated) | An explicit URL to CSV, EPUB, DOCX, XLSX, or PPTX returns bounded structured text |
| M005 — Instruction-signal screening | capability | proposed | `plans/implementation/web-acquisition-expansion/005-instruction-signal-screening.md` | None | Extracted documents carry bounded, deterministic injection signals with spans, without blocking |

## 8. Cross-cutting requirements

- **Security:** every new network read goes through the existing `FetchClient`
  validation; a sitemap never authorizes a fetch of a URL it lists; selector and
  attribute input is length-capped and parsed before any I/O.
- **Cache:** any option that changes extracted output joins the extraction key;
  a route or scope change must never share a cache entry.
- **Payload economy:** every addition reports its own bounded counts and
  participates in `max_total_chars`.
- **Protocol:** additive optional fields only; no tool rename; alias behavior
  unchanged; Python binding schema parity must hold.
- **Determinism:** ordering is defined by document order or lexicographic URL
  order, never by hash iteration or wall-clock time.
- **Documentation:** `docs/tool-matrix.md`, `architecture/fetch.md`,
  `docs/features.md`, `docs/config.md` when configuration appears, and
  `docs/test-inventory.md` when suites change.

## 9. Verification strategy

Mock-feature suites per milestone; a captured sitemap and robots corpus for
M002; golden diff fixtures for M003; per-format fixtures including a deliberately
malformed file for M004; a benign and an injection corpus for M005. `make check`
and `make docs-check` on every milestone. No test requires the network.

## 10. Risks and decision points

- **Contract wording.** "Not for crawling" and "site map" are adjacent; M002
  must state the boundary precisely and update the contract text in the same
  change.
- **Robots ambiguity.** Reading `Sitemap:` directives is discovery, not
  politeness enforcement. The reserved `respect_robots_txt` flag must not be
  silently repurposed.
- **Cache correctness under scoping.** A scope-blind key would return the wrong
  body; the fingerprint is mandatory before the first merge.
- **Diff determinism.** Line diffing has a cost bound; a cap fallback must be
  explicit rather than silently truncated.
- **Dependency weight.** M004 adds crates to a 1.89 MSRV build with strict
  packaging guards; the `pdf` feature history is the template.
- **Screening false positives.** M005 is a spike until measured on a corpus;
  it must never block or alter trust markers.

Deferred: full-page screenshots, page-to-PDF export, mobile emulation,
same-origin crawl, and structured JSON extraction. Each is either adjacent to
accepted decisions or needs its own contract.

## 11. Completion definition

Each milestone closes on an exact candidate with its acceptance criteria
exercised, its closure record written, and the roadmap and registry updated in
the same commit. A roadmap milestone is not complete because its infrastructure
landed.
