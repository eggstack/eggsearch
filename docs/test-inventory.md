# Test Inventory

Inventory of all hardening and regression test suites.

## Test Counts

| Feature Combo | Tests | Ignored |
|--------------|-------|---------|
| `--all-features` | 6669 | 30 |
| `--features mock` | 6604 | 6 |

Ignored tests are live-network smoke tests (`corpus_runner`, `mcp_http` via `live-mcp-smoke` or `live-release-smoke`, `browser_live_smoke`, `native_forge_smoke`, `retrieval_quality`), the PDF fidelity recorder (`pdf_fidelity`), plus the opt-in live-model comparison (`tool_surface_live`) — they run only via explicit opt-in targets. `make live-release-smoke` builds the same `mcp_http` smoke with `--release`; challenge-solver and proof-of-work timings are only meaningful there.

## Behavioral & Workflow Contracts

| Suite | Feature Gate | Focus |
|-------|-------------|-------|
| `mcp_tools` | `mock` (mostly) | Tool registration, input validation for all 10 tools, including the tagged fetch web/repo item schema, response shape, legacy-name rejection; opt-in screenshot PNG bounds, browser capability/conflict errors, image-out-of-JSON/cache behavior, and ordered batch image association |
| `web_search_integration` | `mock` (mostly) | Web search validation, sanitization, intent reranking, excerpt bounds, explicit-provider unbuildable-id rejection, derived-intent reporting and applied reranking, search-result cache hit/miss/coalesce equivalence and partial-fan-out bypass, blocked-provider classification |
| `engine_parse_conformance` | `mock` | Captured-fixture conformance for the five HTML scrapers: parsed rows, explicit empty state, block classification, and a seeded-drift negative control that requires `parse_error` plus a stable, bounded title/body fingerprint |
| `engine_api_conformance` | none | Table-driven conformance for the nine forge engines: 401/403/500 mapping, malformed-JSON `ParseFailed`, zero-result no-request, per-page limit, documented auth header, skipped invalid URLs, and one conversion case per engine |
| `web_fetch_integration` | `mock` (mostly) | Fetch extraction, truncation, metadata-only mode, safety bounds, anti-bot challenge marking (`fetch_challenge_page` + `manual_interaction_required`), Anubis and ALTCHA solve-and-replay under `[search.challenge]`, fetch-side pass-cookie reuse, lifetime expiry, malformed-lifetime refusal, rejection eviction, in-call recovery from a refused pair set, and cross-origin isolation; plus auto-mode ordering (the solver owns a solvable wall and Chrome is never launched), unresolved-wall marking, interactive challenges staying manual, the deadline leaving no render time, the `browser_completions` counter, and the browser-gated status seam (a `429` and a `500` each escalate once, `402`/`403`/`404` never render, `http_only` never renders, a missing browser keeps the `http_status` error and names the unavailable escalation, a render that ends on `5xx` without a completed wall keeps the original error, and a served origin is not charged to the circuit breaker) |
| `snapshot_change` | `mock` | Deterministic `new` → `same` → `changed` comparison of sanitized extracted text, SQLite privacy inspection, brief projection, and disabled-store warning/no-file behavior |
| `documents` | `documents` (feature-off cases run without it) | Bounded container-document extraction for CSV/TSV, EPUB, DOCX, XLSX, and PPTX: format detection by content type, extension, and container introspection; deterministic part order and one-based part addressing; spreadsheet dates and locale-independent numbers; the `document_empty`/`document_malformed`/`document_unsupported`/`document_encrypted` outcomes; the archive-entry, decompression, inflation-ratio, part, character, and XML-depth bounds; and feature-off parity for CSV and every container format |
| `site_map` | `mock` | Bounded same-origin URL discovery as a `fetch` item: robots `Sitemap:` reading, declared and fallback sitemaps, one index level, duplicate collapse, query normalisation, malformed XML, gzip refusal, cross-origin refusal including a cross-origin redirect, URL/XML-depth truncation, deterministic ordering, and zero fetch of discovered URLs |
| `provider_routing` | `mock` (mostly) | Provider routing, code-host rewrites, diagnostics, capability-skip telemetry, keyless source-provider routing/disabled-skip behavior, credentialed-provider opt-in/configured status reporting, explicit-provider rejection classification matrix (unknown/disabled/missing-credential/not-built), CodeGG parity qualification journeys |
| `repo_workflow` | `mock` (mostly) | Repository evidence discovery end to end, explicit `github_repositories` discovery journey, prefer_local linked-worktree local redirect with no remote fetch, no-match remote fallback with an explicit local-skip reason, `repo_fetch` upstream error classification, and `repo_map` omitted-host and specific-over-generic warning behavior |
| `research_workflow` | `mock` (mostly) | Multi-source research discovery end to end |
| `knowledge_delta_brief` | `mock` | Cutoff-aware `knowledge_brief`: publication-after-cutoff emission, unknown-model non-claim, rejected provider-official override, total provider failure, undated-provider diagnosis and capability reporting, date accounting and empty-state reasons, disabled/unseeded/stale/seeded index states, cold-start enable-seed-restart-reuse, catalog-as-of reporting, excerpt selection without leading site chrome, and no index open or file created on an ordinary live call |
| `knowledge_delta_index` | None | Local evidence index: precision preservation, partition isolation, private-body rollback, corrupt-file refusal, a second connection reading a committed batch, and the actionable permission rejection that names the rejected directory and its required mode |
| `knowledge_delta_discovery` | `mock` | Source-backed omission fixture: an unnamed neighbor appears only when `related_topics` is set and an indexed edge cites it; generic text adds no expanded delta |
| `knowledge_delta_catalog` | None | Bundled cutoff catalog: manifest/snapshot field agreement, alias collisions, precision fidelity, resolver coverage for every verified row, unknown models, and operator precedence |
| `knowledge_delta_quality` | `mock` | Frozen 88-case discovery corpus (`docs/knowledge-delta-evaluation.md`): direct-live, indexed-lexical, and indexed-expansion variants measured for expectation rate, gold recall, false temporal claims, duplicate yield, span coverage, payload budget, and deterministic replay |
| `security_workflow` | `mock` (mostly) | Advisory retrieval, applicability assessment, safety handling, default identifier routing through an enabled native advisory provider, conditional availability/severity warnings, and the bounded advisory summary |
| `dependency_fixtures` | None | Realistic minimized lockfile fixtures per ecosystem/version (`tests/fixtures/dependency_evidence/`) |
| `pdf_fidelity` | `pdf` | Committed synthetic PDF corpus (8 cases, one per document class, 15,237 bytes): line-shape floors/ceilings, intact required words including U+FB00–FB06 ligature tokens, table-row adjacency, ordered passages, per-page classification and image counts, running-head repetition, outline count/depth; 116 baseline assertions and 15 recorded ADR-0008 target statuses; join-lines-and-strip-spaces negative control over 25 metric groups; 16 tests plus 1 ignored opt-in recorder into `target/pdf-corpus/`, including one that pins document-scoped chunks against window-bounded legacy text |
| `evidence_contract` | `mock` (mostly) | Batch fetch isolation, evidence packaging, bundle handoff shape |
| `corpus_runner` | `mock` | Multi-step workflow regression (`tests/corpus/scenarios/`) |
| `config_validation` | None | Config rules and validation, including `[search]` search-cache TTL/entry bounds, field-level `[search]` defaults for partial tables, and the explicit-empty-provider-table rule |
| `keyless_core` | None | Keyless-core runtime invariant |
| `local_workspace_integration` | `mock` | Local workspace search backend, local PDF extension/magic detection, size budget, feature/config gates, explicit empty outcome, trust/sanitization, non-PDF binary rejection, relevance admission (role bonuses never admit), scoped include_local, truncation meaning, second-pass content and symbol recall beyond the first candidate window, bounded git-hint versus walk parity, exhausted-versus-cut-short signalling, linked-worktree shared-remote identity parity with a normal clone, a content-only match found after a cold inventory build that consumed more than the request budget, and timeout and file-cap outcomes that state the reason instead of claiming a completed no-match |
| `inventory_freshness` | `mock` | Inventory lifecycle and freshness |
| `retrieval_attempt_ledger` | None | Attempt ledger validation |
| `retrieval_quality` | `mock` (live observation and recorder ignored) | Synthetic versioned retrieval-quality corpus, URL aliases, incomplete judgments, precision/recall/MRR/nDCG, passage budget, source diversity, evidence/status metrics, deterministic fingerprint, candidate-snapshot replay through the production selection path (tier-1 synthetic, tier-2 recorded, tier-3 recorded into `target/snapshots/`), duplicate-slot/domain/authoritative-survival metrics, three seeded negative controls, derived-intent rollout gate, shipped-default-set routing gate (one case per disclosed intent, keyword-query admission predicate, re-pointed `security-nvd-first` negative control), opt-in bounded provider observations |
| `conflict_source_attribution` | None | Conflict source attribution |
| `bounded_command` | `mock` | Bounded git command execution (caps, timeout, termination) |
| `firecrawl_developer` | None | Developer-index adapter behavior |
| `native_security_attempts` | None | Native advisory attempt preservation and advisory summary mapping |
| `codegg_evidence_contract` | None | CodeGG evidence handoff shape |
| `browser_profiles` | `browser` | Browser profile management |
| `browser_transport` | `browser` | Browser transport orchestration |
| `browser_live_smoke` | `browser` (+ `live-smoke`, ignored) | Live browser smoke (opt-in only) |
| `static_guards` | None | Ownership, layout, crate-wide unsafe, repo-wide process/shell, planning consistency, workflow wiring, forge policy/transport, dependency/workflow, policy guards, integration-test helper single definition, module-size ratchets (explicit ceilings plus an `src/**` sweep enforcing the ordinary 1600-line / 80 KB ceiling), and credentialed-provider endpoint/parameter guards (Kagi v1-only, SerpApi no-extra-verticals/no-billed-extras) (fail-closed) |

Placement authority is the table in `architecture/testing.md`.

## PDF Fidelity Corpus (`tests/fixtures/pdf/`)

| Document class | Case file | PDF bytes |
|----------------|-----------|----------:|
| `two_column` | `two_column_interleaved/arxiv_style_two_column.pdf` | 1,886 |
| `standards_table` | `standards_table/rfc_style_standards_table.pdf` | 3,489 |
| `clean_text` | `clean_text/single_column_notes.pdf` | 1,466 |
| `image_only` | `image_only/scanned_placeholder.pdf` | 774 |
| `encrypted` | `encrypted/locked_placeholder.pdf` | 903 |
| `malformed` | `malformed/truncated_catalog.pdf` | 48 |
| `cid_corrupt` | `cid_corrupt/garbled_codes.pdf` | 1,011 |
| `outline_heavy` | `outline_heavy/nested_outline.pdf` | 5,660 |
| **Total** | **8 cases** | **15,237** |

Every case is generated synthetically by this repository; no third-party document bytes are committed. `corpus.json` records the licence, page count, SHA-256, expected outcome, and a construction recipe per case. The suite asserts 116 baseline annotations and 15 recorded ADR-0008 target statuses (an improvement forces a manifest update), and the join-lines-and-strip-spaces negative control must be rejected by 25 declared metric groups, with the two-column case covering all five metric families plus running heads. Corpus budget: 3 MiB total and 1 MiB per case, asserted by the suite.

## Property Tests (19 suites)

| Suite | Feature Gate | Tests | Focus |
|-------|-------------|-------|-------|
| `property_sanitize` | None | 15 | strip_control_chars, bound_text, scan_injection_markers, frame |
| `property_identity` | None | 16 | source_id, canonicalize_url, cross-type collisions, Unicode normalization |
| `property_identity2` | None | 15 | fetch_id, suggested_fetch_id, batch_fetch_id, doc_id |
| `property_identity3` | None | 9 | chunk_id, code_span_id, locator_id |
| `property_excerpt_economy` | None | 5 | Excerpt budget monotonicity, deterministic shingle suppression, counter identity, termination |
| `property_fetch_limits` | None | 13 | validate_url: scheme, length, localhost, private IP; (feature `pdf`) the stream-scan total never exceeds its budget and extraction reports only the configured budget |
| `property_fetch_redirects` | None | 27 | validate_url: TLDs, IP ranges, ports, schemes |
| `property_fetch_url_edge` | None | 20 | URL scheme/path/length edge cases |
| `property_fetch_response` | None | 18 | FetchClient: credentials, metadata-only, text mode, max_chars, Content-Length, timeout, redirect limit, sanitization |
| `property_render_safety` | None | 16 | strip_control_chars, bound_text, frame, scan_injection_markers safety |
| `property_render_code` | None | 12 | render_code, render_diff, render_plaintext, render_csv |
| `property_render_metadata` | None | 11 | TrustMarkers merge, sanitization metadata consistency, outline-reference bounds |
| `property_local_fs` | None | 22 | Path joining, extensions, skip dirs, binary extensions, scoring |
| `property_local_fs_extended` | None | 35 | Symlinks, path traversal, hidden paths, root containment, permission denied |
| `property_forge_url` | None | 17 | Forge URL validation, credential rejection, loopback, private ranges |
| `property_conflict` | None | 24 | Conflict detection, entity scoping, source attribution |
| `property_retrieval` | None | 47 | Retrieval attempt ledger, absence kinds, truncation evidence, derived-intent determinism/bounds/reason-code shape |
| `property_dependency_parse` | None | 8 | Dispatch determinism, per-file budgets, identity/provenance invariants, line bounds, canonicalization |
| `property_knowledge_delta` | None | 9 | Cutoff comparison relations, date-accounting partition invariants, and cutoff precision handling |

## Fault Injection & Adversarial

| Suite | Feature Gate | Tests | Focus |
|-------|-------------|-------|-------|
| `dispatch_fault_injection` | `mock` | 34 | Provider failure, timeout, hang, health transitions, concurrency, panic, credentialed quota isolation (rate-limited class, cooldown threshold, auth vs quota classes) |
| `provider_probe_conformance` | `mock` | 23 | Shared probe service: success/skip/timeout/HTTP/parse/network/panic, probe-health isolation, search-budget derivation and override, explicit-after-degraded, bounded messages, descriptor source-of-truth |
| `adversarial_corpus` | None | 28 | Structural validation of adversarial corpus JSON files, plus (feature `pdf`) pre-parse decompression-bound bombs for content, object, xref and chained-filter streams, `/W` geometry and `LZWDecode` refusals, the `/FlateDecode`-then-unknown-filter chain, and the `/Encrypt` with/without-password matrix |
| `provider_request_contract` | `mock` | 36 | Engine request migration, date/domain validation, Brave params/news endpoint, telemetry, legacy fixtures, automatic gzip/br advertisement, searxng wire compression, chunked gzip/Brotli bounded-body regression with transfer-shape control, decoded-body limits, compressed-response deadlines, keyless source-provider wire contracts (Wikipedia/HN/arXiv/PubMed/GitHub repositories) incl. arXiv pacing gate, credentialed-provider wire contracts (SerpApi pinned engine/no extra verticals/query-parameter credential; Kagi v1 POST + bearer + search-only collection, terminal quota), provider-scoped failures, credential redaction in every error path, and oversized-body rejection |
| `extract_fetch_contract` | `mock` (1 test) | 24 | Excerpt bounds/merge/sanitization, Brave excerpts/timestamps, focus ranking/caps/validation, cache policy/max-age/refresh/bypass, batch cache controls, markdown-first negotiation (Accept policy, cache-key isolation, bounded fallback, `Vary: Accept`, markdown link extraction, controlled A/B) |
| `batch_fetch_retrieval` | `mock` | 27 with PDF/browser; 22 without | Mixed web/repo/workspace parity, per-URL PDF page validation/selection/cache isolation and metadata-only/HTML behavior, local workspace PDF parity, render/profile and cache controls, repo source selection and `prefer_local`, host-local-scope authorization, preflight, order/aggregate truncation, UTF-8 boundaries, focus, metadata-only rejection, failure isolation, locator safety, suggested-fetch round-trip, batch next-actions, per-item challenge marking, repo readable-text and emitted-payload budget accounting, structural degradation markers, and payload telemetry |
| `provider_capability_contract` | None | 23 | Provider native-capability enforcement (brave_api/exa/tavily/firecrawl/serpapi/kagi), HTML-scraper none, domain-filter exclusivity, keyless source-provider inventory/keyless/default-exclusion/timestamp/freshness claims (incl. `nvd` freshness non-enforcement), credentialed-provider required-key posture/opt-in exclusion/engine-builder reachability with typed missing-key skips, `structured_api` exclusivity, AGENTS.md prose agreement |
| `provider_workstream_regression` | `mock` | 7 | Provider inventory (44 IDs), capability descriptors, constraint enforcement matrix, keyless vs optional-credential posture, URL dedup with stable IDs, Tavily sanitization, CodeGG backward-compatible deserialization |
| `egress_routing` | None (proxy composition under `egress`) | 35 | Egress config validation (incl. IPv6 literals), credential redaction, custom-dialer SSRF gate, deterministic HTTP/SOCKS/multi-hop composition, pool reuse, handshake cancellation/deadline, HTTPS TLS/SNI through CONNECT, gzip/Brotli decode with decoded-body limits, authenticated HTTP/SOCKS with credential non-forwarding, malformed-proxy fail-closed, routed redirects |
| `proxy_route_feasibility` | None (Eggress comparison under `egress`) | 11 (+1 with `egress`) | Loopback-only dynamic-proxy characterization: CONNECT authority and pinned-IP/Host/SNI behavior, HTTP forward and pin rejection, SOCKS5/SOCKS5H, authentication, proxy TLS, malformed route, timeout, pinned redirect, `NO_PROXY`, loopback refusal, handler-level pinned CONNECT and SOCKS5 targeting, proxy `407` mapping, redirect route-change refusal, and optional Eggress custom-dialer comparison |
| `fetch_proxy_route` | `mock` | 7 | `[fetch.proxy]` handler contract: `no_proxy` reporting for single and batch items, direct route metadata omission, `proxy_required` plaintext refusal before any direct request, unreachable-endpoint fail-closed, missing credentials before target lookup, `cache_only` under a proxy route, and redacted `provider_status` without environment names |
| `snapshot_change` | `mock` | 6 | Persistent deterministic comparison across `new`/`same`/`changed`/`removed`, sanitized storage and privacy, brief-projection retention, disabled and corrupt-store degradation, caller-key validation, and the no-store default |
| `documents` | `documents` (feature-off cases run without it) | 3 without the feature; 8 with it | Container-document extraction and outcome contract for CSV/TSV, EPUB, DOCX, XLSX, and PPTX, deterministic part order across repeated runs, the `documents_enabled` switch, the capability triad, and feature-off parity for CSV and container formats |

## Forge Adapter Tests (`tests/forge_adapter.rs`)

| Test | Focus |
|------|-------|
| `test_http_loopback_rejected` | HTTP loopback rejected by default |
| `test_http_private_address_rejected` | HTTP private address rejected by default |
| `test_https_private_dns_rejected_without_policy` | HTTPS private DNS rejected unless internal-forge enabled |
| `test_internal_forge_accepted` | Internal forge accepted when explicitly configured |
| `test_credential_bearing_http_rejected` | Credential-bearing HTTP rejected even with internal policy |
| `test_cross_origin_redirect_rejected` | Cross-origin redirect rejected |
| `test_ipv6_loopback_handled` | IPv6 loopback/private/documentation ranges handled |
| `test_nested_gitlab_namespaces_encoded` | Nested GitLab namespaces with slashes encoded |
| `test_gitea_without_base_url_reports_failure` | Gitea without base URL reports configuration failure |
| `test_resolved_ref_used_in_urls` | Resolved ref used correctly in URL construction |
| `test_nested_entries_preserved` | Nested repository maps preserve all entries within depth |

## Recipe Action Tests (`tests/recipes_next_actions.rs`)

| Test | Focus |
|------|-------|
| `next_action_template_keys_are_valid_for_target_tool` | Template keys match actual tool arg schemas |
| `research_evidence_gap_actions_have_evidence_gap_and_rationale` | All evidence gap actions populate evidence_gap and rationale |
| `every_recipe_step_tool_is_known_mcp_tool` | Recipe steps reference valid MCP tools |
| `next_action_tool_names_are_valid` | Next action tool names are in known list |
| `next_action_priorities_are_bounded` | Priority values within 1..=5 |
| `next_action_hints_capped_at_max` | Action count respects MAX_NEXT_ACTIONS |

## Adversarial Corpus (9 files, 271+ cases)

| File | Cases | Focus |
|------|-------|-------|
| `html_malformed.json` | 24 | Malformed HTML, deeply nested, broken attributes |
| `html_extended.json` | 31 | SVG, MathML, CDATA, template, noscript, prompt-injection markers |
| `structured_text.json` | 27 | JSON, JSONL, YAML, TOML, XML, CSV, diff, patch |
| `structured_text_extended.json` | 47 | Notebooks, reStructuredText, AsciiDoc, long lines |
| `url_edge_cases.json` | 31 | Scheme, path, length, malformed URL edge cases |
| `sanitize_edge_cases.json` | 19 | Control chars, framing, injection markers |
| `identity_edge_cases.json` | 16 | URL canonicalization, percent-encoding |
| `pdf_extended.json` | 28 | PDF magic bytes, encrypted, malformed xref, cyclic refs |
| `filesystem_extended.json` | 30 | Symlinks, path traversal, hidden paths, binary files |

## Fuzz Targets (23 registered targets)

Source of truth: `fuzz/Cargo.toml` [[bin]] entries.

| Target | Focus |
|--------|-------|
| `validate_url` | URL validation with default limits |
| `validate_redirect_target` | URL validation with permissive limits |
| `validate_redirect_chain` | Multi-hop redirect target sequences |
| `validate_content_type` | Content-type classification plus extraction with varying Content-Type |
| `chunk_boundary` | Bounded text splitting at various char boundaries |
| `mixed_utf8_extract` | HTML extraction from mixed UTF-8 and lossy bytes |
| `extract_content` | HTML extraction from strings |
| `extract_content_bytes` | HTML extraction from raw bytes |
| `strip_control_chars` | Control char stripping |
| `scan_injection_markers` | Injection marker scanning |
| `build_document_chunks` | Document chunking |
| `extract_pdf_text` | PDF text extraction |
| `canonicalize_url` | URL canonicalization via source_id |
| `sanitize_pipeline` | Full sanitize pipeline: strip → bound → scan |
| `bounded_response_reader` | Production bounded chunk-append logic (byte cap across streamed chunks) |
| `workflow_kind_parse` | Workflow kind parsing |
| `classify_absence` | Absence classification |
| `detect_entity_scoped_conflicts` | Entity-scoped conflict detection |
| `retrieval_failure_expansion` | Retrieval failure expansion across roles |
| `attempt_summary_generation` | Attempt summary generation |
| `workflow_resolution` | Workflow resolution |
| `research_role_mapping` | Research role mapping |
| `dependency_parse` | Typed dependency-evidence parsing (Cargo/npm/pip/Go/Maven/Gradle lockfile grammar) |

## Schema/Contract Tests (14 suites)

| Suite | Focus |
|-------|-------|
| `schema_identity_registry` | Identity function stability |
| `fetch_safety` | Fetch safety bounds |
| `security_applicability_corpus` | Security applicability pipeline |
| `security_applicability_contract` | Security applicability assessment contract |
| `mcp_http` | Loopback Streamable HTTP lifecycle, transport hardening, and compact-profile initialized `tools/list`; ignored `live-mcp-smoke` calls all canonical tools/aliases against public defaults, extracts a public PDF, and forces browser rendering with Cargo defaults on and no `mock`; `make live-release-smoke` reruns it with `--release` for solver timing |
| `security_applicability_regression` | Security applicability regression |
| `research_evidence_corpus` | Research evidence regression |
| `research_semantic_roles` | Research semantic role mapping |
| `recipes_next_actions` | Workflow hint generation |
| `evidence_bundle_handoff` | Evidence bundle packaging |
| `evidence_integration` | Evidence integration pipeline |
| `structured_local_code_intelligence` | Structured local code intelligence (4-language fixtures, ranking, fallback, budgets, repo-map enrichment, and linked-worktree structured search preserving repository-relative paths) |
| `provider_capability_contract` | Provider native-capability enforcement contract |
| `egress_qualify_contract` | Egress qualification CI contract (exact target-set equality, per-target egress check, non-publishing, MSRV 1.89, route-seam trigger and symbol coverage) |

## Tool Consolidation Contracts (5 suites)

| Suite | Tests | Focus |
|-------|-------|-------|
| `mcp_tool_contract` | 22 | Canonical registry parity, sanitization, deterministic fingerprints, compact-profile size/all-name contract, and the fetch canonical argument-property surface |
| `mcp_schema_slimming` | 27 | Full/compact schema budgets, batch per-target option advertisement, legacy-field acceptance, and cross-profile goal/research_domain enum parity |
| `mcp_2026_protocol` | 14 | Structured results, output schemas, repairable error contract |
| `tool_inventory_parity` | 4 | One inventory across the contract registry, core follow-up gate, presets, SDK discovery, documented preset table, and documented canonical list; compatibility aliases stay routed but undiscoverable |
| `mcp_projection` | 35 | Brief/compact/standard/diagnostic projections: status, trust, security applicability summary and budget retention, provider and batch failure summaries, bounded failure details, repository/research reduction, character budgets, UTF-8 framing, intact over-800-character document extraction within budget, native advisory summary retention, payload budget fields retained, 15-task byte/retention trace, bundle identity |

## Tool-Surface Evaluation (2 suites)

| Suite | Feature Gate | Tests | Focus |
|-------|-------------|-------|-------|
| `tool_surface_evaluation` | None | 5 | Labeled 43-fixture discovery corpus for full and compact profiles (top-1/recall@3/MRR), description/total/instructions/compact byte budgets, fingerprint determinism, forbidden-primary exclusion, synthetic next-action hydration mechanics |
| `tool_surface_live` | None (comparison `#[ignore]`d) | 1 (+1 ignored) | Layer 3 report contract (fingerprint, config, deltas); opt-in manual multi-model comparison via `EGGSEARCH_EVAL_MODEL` |

Corpus: `tests/fixtures/tool_surface/cases.json` (43 cases) with `README.md`
baseline (72906 per-tool serialized definition bytes, 1614 instruction bytes, 43/43 top-1; the
post-consolidation v0.3.9 figure was 73037 definition bytes before `repo_map`
stopped advertising `providers`). Re-run with `make eval-tool-surface`.

## Documentation Contract Tests (5 suites)

| Suite | Focus |
|-------|-------|
| `docs_config_snippets` | TOML snippet validation |
| `docs_provider_inventory` | Provider ID validation (code-derived from `KNOWN_PROVIDER_IDS`) |
| `docs_tool_names` | Tool name validation (code-derived from `src/mcp/server.rs`) |
| `docs_safety_vocabulary` | Safety vocabulary validation |
| `docs_keyless_contract` | Keyless-core runtime contract |

## Repository Hygiene

Deterministic `packaging/check-repo-hygiene.sh` runs in `make check` (`make hygiene`):

- forbidden transcript/log artifacts (`typescript`, `*.script`, coverage/profraw);
- ANSI escapes in unexpected root text artifacts;
- tracked build outputs (`target/`, `node_modules/`, etc.);
- oversized unexpected root blobs over 100 KiB (allowlist: `Cargo.lock`, `CHANGELOG.md`, `README.md`, `AGENTS.md`, `LICENSE`);
- tracked editor/temp files (`*.swp`, `*.bak`, `.DS_Store`).

`packaging/check-contract.sh` (part of `make check`) also runs
`packaging/check-packaged-assets.py`, which fails when an asset read by
`include_str!` or `include_bytes!` under `src/` is absent from
`cargo package --list`, so the published crate and the maturin sdist always
compile from the tarball they ship.

## CI Jobs

| Job | Duration | Feature Combos |
|-----|----------|----------------|
| `ci` | ~3min | fmt + clippy + no-default-features check + all-features tests + hygiene + packaging-check |
| `windows-portability` | ~4min | `cargo check --locked --all-features --target x86_64-pc-windows-msvc` on `windows-latest` with pinned Rust 1.89; routine PR/main sentinel so Windows cfg breakage cannot hide behind Linux-green CI |
| `python-native-qualification` | varies | Native wheel builds for five targets; exact abi3 qualification covers CPython 3.11–3.15 (25 smoke cells); native extension suite coverage is recorded separately below; extracted sdist offline rebuild and smoke; Cargo and Python dependency audits |

## Python Native Binding

| Suite | Runtime | Tests | Focus |
|-------|---------|-------|-------|
| `bindings/python/tests/test_native.py` | CPython 3.11–3.15; PyO3 extension | 28 (23 on release wheels; 5 debug-only barrier tests skip) | Ten-operation asyncio smoke, canonical schema/type coverage, typed method delegation and explicit null/false/empty preservation, async context manager, structured errors, Mode-Off local trust, bounded JSON conversion and invalid inputs, cycles, started-barrier cancellation with Rust drop/permit recovery, concurrent close with `client_closed` taxonomy, 32-call saturation and recovery, conversion-failure leak checks, sequential event-loop recovery, and default-config selection (`from_default_config`, explicit override, missing fallback, invalid TOML, token non-leak) |

Run from the repository root after installing the extension into an active
virtual environment:

```bash
maturin develop --manifest-path bindings/python/Cargo.toml --locked
python -m pytest -q bindings/python/tests
python -m compileall bindings/python/python/eggsearch
pyright bindings/python/python/eggsearch bindings/python/examples
```
