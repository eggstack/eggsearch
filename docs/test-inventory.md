# Test Inventory

Auto-generated inventory of all hardening and regression test suites.
Last updated: Phase 2 completion.

## Test Counts

| Feature Combo | Tests | Ignored |
|--------------|-------|---------|
| `--all-features` | 3792 | 5 |
| `--no-default-features` | 3508 | 0 |
| `--features mock` | 3778 | 0 |
| `--features pdf` | 3522 | 0 |

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

## Fuzz Targets (15 targets)

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
| `check` | ~2min | 4 combos |
| `test` | ~30s × 4 | all-features, no-default, mock, pdf |
| `clippy` | ~30s | all-features |
| `schema-corpus` | ~10s | mock |
| `docs-contract` | ~5s | all-features |
| `fmt` | ~2s | N/A |
| `release-build` | ~60s | all-features |
| `publish-check` | ~30s | all-features |
| `hardening` | ~15s | all-features (Makefile target, not a CI job) |
| `fuzz-smoke` | ~45s | all-features (15 targets × 15s) |
| `docs` | ~30s | all-features |
