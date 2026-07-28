# Test Inventory

Auto-generated inventory of all hardening and regression test suites.
Last updated: Phase 3-5 production closure.

## Test Counts

| Feature Combo | Tests | Ignored |
|--------------|-------|---------|
| `--all-features` | 3950 | 9 |
| `--no-default-features` | 3662 | 0 |
| `--features mock` | 3935 | 0 |
| `--features pdf` | 3677 | 0 |

## Property Tests (15 suites, 243 tests)

| Suite | Feature Gate | Tests | Focus |
|-------|-------------|-------|-------|
| `property_sanitize` | None | 15 | strip_control_chars, bound_text, scan_injection_markers, frame |
| `property_identity` | None | 17 | source_id, canonicalize_url, cross-type collisions, Unicode normalization |
| `property_identity2` | None | 15 | fetch_id, suggested_fetch_id, batch_fetch_id, doc_id |
| `property_identity3` | None | 9 | chunk_id, code_span_id, locator_id |
| `property_fetch_limits` | None | 11 | validate_url: scheme, length, localhost, private IP |
| `property_fetch_redirects` | None | 24 | validate_url: TLDs, IP ranges, ports, schemes |
| `property_fetch_url_edge` | None | 11 | URL scheme/path/length edge cases |
| `property_fetch_response` | None | 16 | FetchClient: credentials, metadata-only, text mode, max_chars, Content-Length, timeout, redirect limit, sanitization |
| `property_render_safety` | None | 15 | strip_control_chars, bound_text, frame, scan_injection_markers safety |
| `property_render_code` | None | 12 | render_code, render_diff, render_plaintext, render_csv |
| `property_render_metadata` | None | 11 | TrustMarkers merge, sanitization metadata consistency, outline-reference bounds |
| `property_local_fs` | None | 12 | Path joining, extensions, skip dirs, binary extensions, scoring |
| `property_local_fs_extended` | None | 25 | Symlinks, path traversal, hidden paths, root containment, permission denied |
| `dispatch_fault_injection` | `mock` | 29 | Provider failure, timeout, hang, health transitions, concurrency, panic |
| `adversarial_corpus` | None | 10 | Structural validation of adversarial corpus JSON files |

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

## Fuzz Targets (23 targets)

Source of truth: `fuzz/Cargo.toml` [[bin]] entries.

| Target | Focus |
|--------|-------|
| `validate_url` | URL validation with default limits |
| `validate_redirect_target` | URL validation with permissive limits |
| `validate_redirect_chain` | Multi-hop redirect target sequences |
| `validate_content_type` | extract_content with varying Content-Type |
| `parse_content_length` | Content-Length header value parsing |
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
| `bounded_response_reader` | Forge response bounded reader (UTF-8 + byte cap) |
| `workflow_kind_parse` | Workflow kind parsing |
| `classify_absence` | Absence classification |
| `detect_entity_scoped_conflicts` | Entity-scoped conflict detection |
| `retrieval_failure_expansion` | Retrieval failure expansion across roles |
| `attempt_summary_generation` | Attempt summary generation |
| `workflow_resolution` | Workflow resolution |
| `research_role_mapping` | Research role mapping |

## Schema/Contract Tests (6 suites)

| Suite | Focus |
|-------|-------|
| `schema_identity_registry` | Identity function stability |
| `fetch_safety` | Fetch safety bounds |
| `security_applicability_corpus` | Security applicability pipeline |
| `research_evidence_corpus` | Research evidence regression |
| `recipes_next_actions` | Workflow hint generation |
| `evidence_bundle_handoff` | Evidence bundle packaging |

## Documentation Contract Tests (4 suites)

| Suite | Focus |
|-------|-------|
| `docs_config_snippets` | TOML snippet validation |
| `docs_provider_inventory` | Provider ID validation |
| `docs_tool_names` | Tool name validation |
| `docs_safety_vocabulary` | Safety vocabulary validation |

## CI Jobs

| Job | Duration | Feature Combos |
|-----|----------|----------------|
| `ci` | ~3min | fmt + clippy + no-default-features check + all-features tests |
