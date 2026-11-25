# Web Acquisition M002 — Bounded Site URL Discovery — Status

Status: closed
Date: 2026-11-24
Milestone: M002 — Bounded Site URL Discovery
Decision authority: `architecture/fetch.md`; `src/mcp/tool_contract.rs`
Baseline: `d932115d1f86af51710747d243455792ff63a6fb`

## Candidate and changes

Local candidate: the baseline plus this change set. `fetch` gains a third item
type, `site`, which returns a same-origin URL inventory and never fetches a
discovered URL. The advertised tool count and aliases are unchanged.

- `src/fetch/site_map.rs`: discovery order, robots `Sitemap:` reading, sitemap
  index handling (one level), same-origin link fallback, URL normalisation,
  deterministic ordering, and the bounds.
- `src/core/batch_fetch.rs`: `BatchFetchItem::Site` and
  `BatchFetchItemType::Site`.
- `src/fetch/client.rs`: one crate-private bounded resource read reusing the
  existing validation, DNS pinning, redirect handling, and `read_bounded_body`.
- `src/mcp/tools/{fetch,batch_fetch,batch_fetch_item,batch_fetch_budget}.rs`,
  `src/mcp/projection.rs`: item validation, execution, budget accounting, and
  projection of the `site_map` payload.
- `src/mcp/tool_contract.rs`, `docs/tool-matrix.md`,
  `docs/codegg-integration.md`: the fetch contract now distinguishes discovery
  from traversal.
- `bindings/python/python/eggsearch/types.py`: additive item and response
  typing.
- `tests/site_map.rs` plus `tests/fixtures/site_map/`: six integration cases
  and four unit cases.

## Requirement-to-evidence matrix

| Criterion | Evidence | Outcome |
|---|---|---|
| Deterministic output across runs and insertion orders | `site_discovery_is_deterministic_and_never_fetches_discovered_urls` | pass |
| Zero requests to discovered-only URLs | Mock origin requested-path log, same test | pass |
| Cross-origin entries counted and never contacted | `site_discovery_rejects_cross_origin_targets_and_malformed_xml` | pass |
| Malformed sitemap is explicit | `site_map_malformed`; same test plus `malformed_and_external_entity_xml_are_rejected` | pass |
| One sitemap-index level | `site_discovery_does_not_recurse_into_a_child_index`; `site_discovery_follows_one_sitemap_index_level_and_fallbacks` | pass |
| Bounds and truncation | `site_discovery_refuses_gzip_and_reports_url_limit_and_xml_depth_truncation` | pass |
| Query normalisation and duplicate collapse | `site_discovery_query_drop_collapses_duplicate_urls` | pass |
| Robots `Disallow` not applied | No enforcement path; `respect_robots_txt` reservation text unchanged | pass |
| Contract text updated with the tool surface | `src/mcp/tool_contract.rs`, `src/mcp/server.rs`, `docs/tool-matrix.md`, CodeGG doc | pass |

XML parsing handles only predefined and numeric entity references and rejects
unknown entities; DTDs and external entities are never resolved.

## Deviation from the plan wording

The plan required `include_subdomains` to use an exact registrable-domain
match. The implementation first used a finite public-suffix list, which can
misclassify an unlisted multi-label suffix and admit a sibling registrable
domain. That is a scope-widening defect, so the rule was replaced with an exact,
dependency-free one: `include_subdomains` admits the requested host and hosts
ending in `.<requested-host>` on a label boundary. Callers request the parent
host when they want a whole site. The plan text and schema description were
updated in the same change; no public-suffix dependency was added.

## Verification on this candidate

| Command | Outcome |
|---|---|
| `cargo test --locked --features mock --test site_map` | pass: 6 passed |
| `cargo test --locked --all-features --lib site_map` | pass: 4 passed |
| `cargo test --locked --features mock --test mcp_tools` | pass: 29 passed |
| `cargo test --locked --features mock --test web_fetch_integration` | pass: 130 passed, 1 ignored |
| `make docs-check` | pass |
| `cargo fmt --check` | pass |
| `RUSTUP_TOOLCHAIN=1.89 make check` | pass: fmt, clippy, no-default check, all-features tests, hygiene, dependency policy, packaging |

## Limitations

- Link fallback reads one page; it does not follow pagination, forms, or
  JavaScript-rendered links.
- Sitemaps served with `Content-Encoding: gzip` beyond the ordinary transport
  decompression are refused with an explicit error rather than decoded.
- `include_subdomains` is host-suffix scoped, as described above.
- Discovery is discovery: `respect_robots_txt` remains reserved and unapplied.
