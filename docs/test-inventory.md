# Test Inventory

Inventory of all hardening and regression test suites.

## Test Counts

| Feature Combo | Tests | Ignored |
|--------------|-------|---------|
| `--all-features` | 5207 | 23 |
| `--features mock` | 4958 | 1 |

Ignored tests are live-network smoke tests (`corpus_runner`, `browser_live_smoke`, `native_forge_smoke`) plus the opt-in live-model comparison (`tool_surface_live`) — they run only via explicit opt-in targets.

## Property Tests (16 suites)

| Suite | Feature Gate | Tests | Focus |
|-------|-------------|-------|-------|
| `property_sanitize` | None | 15 | strip_control_chars, bound_text, scan_injection_markers, frame |
| `property_identity` | None | 16 | source_id, canonicalize_url, cross-type collisions, Unicode normalization |
| `property_identity2` | None | 15 | fetch_id, suggested_fetch_id, batch_fetch_id, doc_id |
| `property_identity3` | None | 9 | chunk_id, code_span_id, locator_id |
| `property_fetch_limits` | None | 11 | validate_url: scheme, length, localhost, private IP |
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
| `property_retrieval` | None | 44 | Retrieval attempt ledger, absence kinds, truncation evidence |

## Fault Injection & Adversarial

| Suite | Feature Gate | Tests | Focus |
|-------|-------------|-------|-------|
| `dispatch_fault_injection` | `mock` | 32 | Provider failure, timeout, hang, health transitions, concurrency, panic |
| `provider_probe_conformance` | `mock` | 20 | Shared probe service: success/skip/timeout/HTTP/parse/network/panic, cooldown, explicit-after-degraded, bounded messages, descriptor source-of-truth |
| `adversarial_corpus` | None | 16 | Structural validation of adversarial corpus JSON files |
| `provider_request_contract` | `mock` | 12 | Engine request migration, date/domain validation, Brave params/news endpoint, telemetry, legacy fixtures |
| `extract_fetch_contract` | `mock` (1 test) | 13 | Excerpt bounds/merge/sanitization, Brave excerpts/timestamps, focus ranking/caps/validation, cache policy/max-age/refresh/bypass, batch cache controls |
| `batch_fetch_retrieval` | `mock` | 13 | Mixed focused/unfocused batch, web+workspace repo batch, aggregate truncation, UTF-8 boundaries, focus with cache hit, metadata-only rejection, failure isolation, locator safety, suggested-fetch round-trip, batch next-actions, locator/policy helpers |
| `provider_capability_contract` | None | 8 | Provider native-capability enforcement (brave_api/exa/tavily/firecrawl), HTML-scraper none, domain-filter exclusivity, AGENTS.md prose agreement |
| `provider_workstream_regression` | `mock` | 7 | Provider inventory (37 IDs), capability descriptors, constraint enforcement matrix, URL dedup with stable IDs, Tavily sanitization, CodeGG backward-compatible deserialization |

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

## Fuzz Targets (22 registered targets)

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

## Schema/Contract Tests (13 suites)

| Suite | Focus |
|-------|-------|
| `schema_identity_registry` | Identity function stability |
| `fetch_safety` | Fetch safety bounds |
| `security_applicability_corpus` | Security applicability pipeline |
| `security_applicability_contract` | Security applicability assessment contract (formerly phase-8 suite) |
| `mcp_http` | Loopback Streamable HTTP lifecycle and transport hardening |
| `security_applicability_regression` | Security applicability regression |
| `research_evidence_corpus` | Research evidence regression |
| `research_semantic_roles` | Research semantic role mapping |
| `recipes_next_actions` | Workflow hint generation |
| `evidence_bundle_handoff` | Evidence bundle packaging |
| `evidence_integration` | Evidence integration pipeline |
| `structured_local_code_intelligence` | Structured local code intelligence (4-language fixtures, ranking, fallback, budgets, repo-map enrichment) |
| `provider_capability_contract` | Provider native-capability enforcement contract |

## Tool Consolidation Contracts (4 suites)

| Suite | Tests | Focus |
|-------|-------|-------|
| `mcp_tool_contract` | 14 | Canonical registry parity: aliases, discovery text, sanitization, fingerprint determinism |
| `mcp_schema_slimming` | 16 | Slimmed ordinary schema size budget with legacy-field acceptance |
| `mcp_2026_protocol` | 13 | Structured results, output schemas, repairable error contract |
| `mcp_projection` | 14 | Response-detail projection: failure-vs-absence, trust, conflicts, truncation, bundle identity, byte reduction |

## Tool-Surface Evaluation (2 suites)

| Suite | Feature Gate | Tests | Focus |
|-------|-------------|-------|-------|
| `tool_surface_evaluation` | None | 4 | Labeled 43-fixture discovery corpus (top-1/recall@3/MRR), description/total/instructions/compact byte budgets, fingerprint determinism, forbidden-primary exclusion, synthetic next-action hydration mechanics |
| `tool_surface_live` | None (comparison `#[ignore]`d) | 1 (+1 ignored) | Layer 3 report contract (fingerprint, config, deltas); opt-in manual multi-model comparison via `EGGSEARCH_EVAL_MODEL` |

Corpus: `tests/fixtures/tool_surface/cases.json` with `README.md` baseline
(77952 definition bytes, 5275 instruction bytes, 43/43 top-1). Re-run with
`make eval-tool-surface`.

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

## CI Jobs

| Job | Duration | Feature Combos |
|-----|----------|----------------|
| `ci` | ~3min | fmt + clippy + no-default-features check + all-features tests |
