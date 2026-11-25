# M002 — Bounded Site URL Discovery

Status: closed
Date: 2026-11-24
Subsystem: web acquisition expansion
Milestone class: capability
Baseline: `d932115d1f86af51710747d243455792ff63a6fb` on `docs/user-facing-polish-20261123`
Decision authority: `architecture/fetch.md` (bounded network access); tool contract text in `src/mcp/tool_contract.rs`.

References: ADR-0001, ADR-0002, ADR-0006, ADR-0022; `plans/003-planning-process.md` §2.4; `src/fetch/client.rs`; `src/mcp/state.rs:318`; `quick-xml 0.41`.

## Objective

Answer "which URLs exist under this origin" in one bounded call, deterministically,
without fetching any of them. The caller then selects targets explicitly, which
is what `fetch` is for today.

## Current implementation evidence

- `fetch` is contractually "not for search, crawling, or following links"
  (`src/mcp/tool_contract.rs:169`).
- `include_links` returns the link set of one fetched document only; there is no
  sitemap, robots, or site inventory support anywhere in `src/`.
- `robots.txt` is not consulted; `[search].live.respect_robots_txt` is reserved
  and unapplied (`src/mcp/state.rs:318-321`).
- `quick-xml` is already a dependency, so sitemap XML adds no crate.
- Every network read must pass `FetchClient` validation and bounded-body reads.

## Contract

A new item variant on the existing `fetch` tool:

```text
{ "type": "site", "url": "https://example.com/docs/", 
  "limit": 200, "include_subdomains": false, 
  "link_discovery": true, "normalize_query": "keep" | "drop" }
```

- `limit` default 200, maximum 2,000.
- Output key `site_map`:
  `{ origin, urls: [{ url, source, lastmod? }], counts: { sitemap, link,
  cross_origin_skipped, duplicates, total }, sitemap_index_used,
  robots_sitemaps, truncated }`.
- `source` is `sitemap` or `link`; ordering is lexicographic by
  (host, path, query) with a stable tie-break on the raw URL.
- `truncated` is true only when a documented bound stopped discovery.

Discovery order, each step optional and separately counted:

1. `/robots.txt` on the requested origin; read `Sitemap:` directives only.
2. Those sitemaps, then `/sitemap.xml` and `/sitemap_index.xml` if none were
   declared. A sitemap index is followed at most one level.
3. When `link_discovery` is true and the inventory is still below `limit`,
   one fetch of the entry URL contributes same-origin `<a href>` targets.

Rules:

- Every candidate URL is normalised: lowercase scheme and host, default port
  removed, fragment removed, path percent-encoding normalised, query kept or
  dropped per `normalize_query`. Duplicates collapse to the first occurrence
  and are counted.
- A candidate whose host differs from the requested origin is dropped and
  counted as `cross_origin_skipped`. It is never fetched, resolved, or
  contacted. `include_subdomains` is an explicit opt-in that admits the requested
  host and hosts ending in `.<requested-host>` on a label boundary. It never
  widens to a sibling registrable domain; callers request the parent host when
  they want the whole site. No public-suffix dependency is introduced.
- The entry URL, robots, and each sitemap pass the ordinary `FetchClient`
  validation independently; a sitemap pointing elsewhere does not authorize it.
- Redirects are followed only within the requested origin. A cross-origin
  redirect ends that read with `site_map_cross_origin_redirect`.

## Bounds

At most 1 robots read, 8 sitemap reads, 64 KiB per body, 2,000 emitted URLs,
and one 5-second discovery budget inside the existing request deadline. Bodies
use `read_bounded_body`; XML depth is capped and external entities are never
resolved.

## Robots boundary

Reading `Sitemap:` is discovery, not politeness enforcement. This milestone
does not implement `respect_robots_txt`, does not change its reservation text,
and documents that `Disallow` is not applied. If enforcement is wanted, it is a
separate plan that owns the config flag and its documentation.

## Contract text change

`fetch` currently says it is not for crawling or following links. The precise
replacement distinguishes discovery from traversal: it returns an inventory and
never fetches a discovered URL. `docs/tool-matrix.md`, the CodeGG tool
description, and `src/mcp/tool_contract.rs` change together.

## Ordered work packages

1. Core types and normalisation with unit tests (URL edge cases, IDN, IPv6
   literals, duplicate collapse, ordering).
2. Robots and sitemap parsing over `quick-xml` with bounded reads and fixtures.
3. Same-origin link fallback reusing the existing HTML extractor.
4. Tool wiring as a new item kind across the three request shapes and the
   aggregate budget.
5. Contract text, tool matrix, CodeGG description, and feature docs.

## Verification

Focused: a new `site_map` suite with captured robots and sitemap fixtures
(plain, index, gzipped refusal, malformed XML, cross-origin entries, duplicate
entries, over-limit), plus a mock origin asserting exactly which paths were
requested. `--test web_fetch_integration` for the item shape and budget
interaction. Broad: `make check`; `make docs-check`.

## Acceptance and stop conditions

- Deterministic output for the same fixture across runs and insertion orders.
- Zero requests to any URL that appears only as a discovered entry.
- Cross-origin entries counted and never contacted.
- A malformed sitemap yields an explicit `site_map_malformed` outcome, never a
  silently empty inventory.
- **Stop:** discovery needs a second network stack, a search provider, or
  fetching discovered URLs to be useful.

## Non-goals

- Fetching, scoring, or filtering discovered URLs by content.
- JS-rendered discovery, form actions, or pagination inference.
- SERP supplementation (a hosted crawler's technique; not available keyless).
- Sitemap generation or writing to the origin.
- Robots enforcement.

## Closure evidence

Fixture corpus, requested-path log from the mock origin, determinism run, bound
exhaustion cases, cross-origin refusal evidence, contract and docs diff, and
limitations.
