# Hardening: Property Testing and Adversarial Corpus

## Overview

Phase 2 added property-based testing and adversarial input corpora to validate invariants on security- and reliability-sensitive pure functions. This complements the existing example-based test suite by discovering classes of defects that fixture tests cannot anticipate.

## Property Tests

Property tests use `proptest` (dev-dependency only) and run without network access.

### Sanitize (`tests/property_sanitize.rs`)

Tests for `src/core/sanitize.rs`:
- `strip_control_chars` output contains no unsafe characters
- `strip_control_chars` is idempotent
- `strip_control_chars` removal count matches actual removals
- `strip_control_chars` preserves `\n` and `\t`
- `bound_text` output respects `max_chars` limit
- `bound_text` truncated flag is accurate
- `bound_text` appends `…` when truncated
- `bound_text` Unicode boundary never panics
- `bound_text` framing overhead cannot exceed cap
- `scan_injection_markers` never panics on arbitrary input
- `scan_injection_markers` byte offsets are valid
- `frame` output structure is correct

### Identity (`tests/property_identity*.rs`)

Tests for `src/core/identity.rs`:
- All ID functions are deterministic (same inputs → same output)
- All IDs have correct prefixes and lengths
- `canonicalize_url` is idempotent
- `canonicalize_url` strips trailing slashes, fragments, www prefix, default ports
- `canonicalize_url` lowercases scheme and preserves non-default ports
- URL canonicalization is applied before hashing (equivalent URLs → same ID)
- Different inputs produce different IDs (collision resistance)
- Cross-type IDs never collide (src, fetch, doc, suggested, batch, chunk, span)
- Unicode normalization (fullwidth vs ASCII) produces different IDs
- Source ID field order is insensitive

### Fetch Limits (`tests/property_fetch_limits.rs`)

Tests for `src/fetch/limits.rs`:
- `validate_url` rejects empty strings, non-http schemes, oversized URLs
- `validate_url` rejects localhost/private IPs when disallowed
- `validate_url` accepts them when allowed
- `validate_url` accepts valid public HTTPS URLs

### Fetch Redirects (`tests/property_fetch_redirects.rs`)

Tests for `src/fetch/limits.rs` IP classification and policy enforcement:
- Private TLD rejection (.internal, .private, .local, .lan)
- CGNAT range (100.64-127) rejection
- Public range (100.0-63, 100.128-255) acceptance
- 172.16-31 private range rejection
- 169.254 link-local rejection
- Port boundary validation
- URL query params and fragments don't affect acceptance

### Fetch URL Edges (`tests/property_fetch_url_edge.rs`)

Tests for URL scheme and structure validation:
- Non-http schemes (ftp, file, ws, gopher, mailto, ssh) rejection
- javascript:, data:, blob: URL rejection
- Empty host rejection
- URL length limits (max_url_len)
- IPv6 literal acceptance
- Port 0 and high ports acceptance

### Fetch Response (`tests/property_fetch_response.rs`)

Tests for `FetchClient` fetch behavior:
- Embedded credentials rejected (username, user:pass, empty username)
- Metadata-only mode skips body extraction
- Text mode returns body content
- max_chars respects cap
- Content-Length precheck rejects oversized responses
- Timeout enforced on slow responses
- Redirect to credentials blocked
- sanitize=false skips framing
- Redirect count never exceeds limit
- Content-Length larger than actual body handled gracefully
- Redirect without Location header handled
- Redirect with invalid UTF-8 Location handled

### Render Safety (`tests/property_render_safety.rs`)

Tests for `src/core/sanitize.rs`:
- `strip_control_chars` safety, idempotency, count accuracy
- `strip_control_chars` preserves safe chars (ASCII, CJK, emoji, etc.)
- `bound_text` output respects max_chars, preserves prefix, appends ellipsis
- `bound_text` idempotency
- `bound_text` never splits UTF-8 at invalid boundaries
- `frame` output structure and content preservation
- `scan_injection_markers` safety, offset bounds, determinism
- Unsafe elements never appear in framed output

### Render Code (`tests/property_render_code.rs`)

Tests for `src/fetch/render/` renderers:
- `render_code`, `render_diff`, `render_plaintext`, `render_csv` never panic
- Output bounded to max_chars
- Deterministic output for same inputs
- Line numbers monotonic and non-overlapping
- Language metadata preserved when provided
- Empty input produces no blocks

### Local FS (`tests/property_local_fs.rs`)

Tests for filesystem path handling:
- Path segments have valid file names
- Absolute/relative path construction
- Binary extension detection
- Skip directory matching
- Max file bytes/indexed files boundaries

### Local FS Extended (`tests/property_local_fs_extended.rs`)

Tests for `src/core/local.rs` `validate_local_fetch_path`:
- Symlink rejection when `follow_symlinks=false`
- Symlink acceptance when `follow_symlinks=true`
- Intermediate symlink rejection
- Path traversal (`../`) rejection
- Absolute path rejection
- Hidden path rejection/acceptance based on `include_hidden`
- Skip directories always rejected
- Binary extension rejection
- File size limit enforcement
- Root containment property (resolved path within canonical root)
- Symlink escape root rejection
- Symlink loops rejected
- Permission denied handled
- Sparse files within size limit accepted
- Overlapping roots both reject cross-root access
- Root replacement between validate and read
- Concurrent file modification during validate
- Multiple validate calls on same path consistent

### Dispatch Fault Injection (`tests/dispatch_fault_injection.rs`)

Tests for `src/meta/adapter.rs` provider dispatch (requires `mock` feature):
- All providers succeed → results returned
- Partial failure → partial results
- All failures → empty results
- Provider timeout doesn't block others
- Duplicate results deduplicated
- Output ordering independent of completion order
- Hang providers cancelled on timeout
- Mixed success/failure/hang scenarios
- max_results respected
- Engine selection by provider ID
- Health transitions: Unknown → Healthy on success
- Health transitions: Unknown → Degraded on failure
- Health recovery after failure (success resets count)
- Health cooldown after repeated failures (>=3)
- Cooldown cleared by success
- Health view returns Unknown for unseen provider
- Concurrent searches do not exceed provider count
- All jobs reach terminal state
- Adapter provider IDs match configured engines
- Panic in provider does not collapse others
- All providers panic returns empty results
- Concurrency saturation does not exceed limit
- Malformed result metadata does not panic
- Global deadline with mixed pending and running
- Partial-result telemetry is exact
- Panic in provider releases counters
- Output ordering deterministic across runs

### Forge Adapter (`tests/forge_adapter.rs`)

Tests for `src/meta/forge_adapter.rs` endpoint validation and response handling:
- HTTP loopback rejected by default
- HTTP private address rejected by default
- HTTPS private DNS name rejected unless internal-forge policy enabled
- Internal forge accepted when explicitly configured
- Credential-bearing HTTP endpoint rejected even with internal policy
- Cross-origin redirect rejected
- IPv6 loopback/private/documentation ranges handled correctly
- Nested GitLab namespaces and refs with slashes encoded correctly
- Gitea without base URL reports structured configuration failure
- Resolved ref used correctly in URL construction
- Nested repository maps preserve all entries within depth bounds

### Local Inventory (`src/meta/local_inventory_cache.rs` unit tests)

Tests for inventory lifecycle and invalidation:
- First search builds inventory automatically
- Second search reuses cached inventory without full traversal
- Concurrent first searches result in one build or bounded duplicate work
- Build timeout falls back deterministically
- Cache poisoning through failed partial build is impossible
- Configuration change invalidates inventory
- HEAD change triggers rebuild
- Index mtime change triggers rebuild
- `validate_entry` rejects deleted, oversized, or symlink entries before content read
- Stale entries skipped gracefully during search

### Evidence Postprocess (`src/core/evidence_postprocess.rs` unit tests)

Tests for evidence role assignment and workflow coverage:
- Evidence roles are deterministic under randomized input order
- Empty optional roles do not degrade required coverage
- Required role missing after successful retrieval → `insufficient`
- Required role indeterminate after provider failure → `indeterminate_due_to_failures`
- Conflict metadata emitted only for directly comparable values
- Retrieval summary correctly counts success/failure/skipped per provider

### Render Metadata (`tests/property_render_metadata.rs`)

Tests for `TrustMarkers` and `DocumentOutlineEntry`:
- `TrustMarkers::merge` ORs booleans and sums counts
- `TrustMarkers::merge` is commutative
- `TrustMarkers::merge` is associative
- Framing structure (EXTERNAL_UNTRUSTED delimiters)
- Injection marker detection (benign vs. adversarial text)
- `strip_control_chars` idempotency
- `bound_text` respects Unicode char boundaries
- Outline `block_index` references within bounds
- Outline entries reference heading blocks
- Document chunk IDs are deterministic
- Chunk IDs are unique within document

## Adversarial Corpus

JSON corpus files in `tests/corpus/adversarial/`:

| File | Cases | Purpose |
|------|-------|---------|
| `html_malformed.json` | 24 | Malformed HTML: nested elements, broken attributes, XSS, consent pages |
| `html_extended.json` | 31 | SVG/MathML/XMP/CDATA/template/noscript/iframe/object/embed, deep nesting, MathML formulas |
| `structured_text.json` | 30 | Malformed JSON/YAML/TOML/CSV/XML, diff, patch, long lines, mixed line endings |
| `structured_text_extended.json` | 47 | Diff/patch, notebooks, RST, AsciiDoc, CSV variants, JSONL, BOM, null bytes |
| `url_edge_cases.json` | 31 | SSRF, credentials, Unicode URLs, port edges |
| `sanitize_edge_cases.json` | 19 | Bidi overrides, ZWJ, homoglyphs, null bytes, prompt injection |
| `identity_edge_cases.json` | 16 | Unusual schemes, fragments, backslashes, multi-slash |
| `pdf_extended.json` | 28 | PDF magic bytes, truncated headers, binary content, embedded HTML, encrypted, malformed xref, cyclic refs |
| `filesystem_extended.json` | 30 | Path traversal, symlinks, permissions, Unicode filenames, special files |

`tests/adversarial_corpus.rs` validates that all corpus files are well-formed JSON with non-empty case arrays.

## Running

```bash
cargo test --locked --all-features --test property_sanitize  # sanitize only
cargo test --locked --all-features --test property_identity  # identity only
cargo test --locked --all-features --test property_fetch_limits  # fetch URL validation
cargo test --locked --all-features --test property_render_code  # renderers
cargo test --locked --all-features --test property_render_metadata  # TrustMarkers + outline bounds
cargo test --locked --all-features --test property_local_fs_extended  # symlinks, traversal, root containment
cargo test --locked --all-features --test dispatch_fault_injection  # dispatch (requires mock)
cargo test --locked --all-features --test adversarial_corpus # corpus validation
```

## Fuzz Harness

Cargo-fuzz targets in `fuzz/` using `libfuzzer-sys` (22 registered targets):

| Target | What it fuzzes |
|--------|---------------|
| `validate_url` | URL parsing and policy validation |
| `validate_redirect_target` | URL validation with permissive limits |
| `validate_redirect_chain` | Multi-hop redirect target sequences |
| `validate_content_type` | Content-type classification plus Content-Type-dependent extraction |
| `chunk_boundary` | Bounded text splitting at various char boundaries |
| `mixed_utf8_extract` | HTML extraction from mixed UTF-8 and lossy bytes |
| `extract_content` | HTML content extraction from strings |
| `extract_content_bytes` | HTML content extraction from raw bytes |
| `strip_control_chars` | Control character removal (idempotency check) |
| `scan_injection_markers` | Injection pattern detection |
| `build_document_chunks` | Document chunking (uniqueness, contiguity checks) |
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

```bash
# Smoke test (15s per target)
cargo +nightly fuzz run validate_url -- -max_total_time=15

# Full campaign (5 minutes)
cargo +nightly fuzz run validate_url -- -max_total_time=300

# List all targets
cargo +nightly fuzz list
```

## Seed Corpus Management

Fuzz seed corpora are stored in `fuzz/corpus/<target>/`. Rules:

- **Minimal by default.** Each target should have at most 5–10 seed inputs that exercise distinct code paths (valid URL, malformed URL, empty input, boundary-length input, Unicode input).
- **No network-derived seeds.** Seeds must be hand-crafted or derived from local test fixtures. Never commit seeds containing live URLs or IP addresses that resolve to real hosts.
- **Size cap.** No individual seed file may exceed 8 KB. Corpus directories should stay under 100 KB total.
- **Naming.** Use descriptive filenames (e.g., `https_with_port.txt`, `empty.txt`). Auto-generated filenames from `cargo fuzz tmin` output should be renamed before commit.
- **Review.** Seed corpus changes go through the same review as code changes. Maintainers should verify that each seed exercises a genuinely distinct code path.

Seed corpora are `.gitignore`d by default because `cargo-fuzz` manages them locally. To share seeds across contributors, commit selected inputs to `fuzz/corpus/<target>/` and document the rationale in the PR description.

## Crash Promotion Process

When fuzzing (local or CI) finds a minimizing input that triggers a panic, hang, or incorrect assertion:

1. **Reproduce locally.** Run `cargo +nightly fuzz run <target> -- -max_total_time=0` with the artifact file as input to confirm the crash.
2. **Minimize.** Use `cargo +nightly fuzz tmin <target> <artifact>` to reduce the input to its smallest triggering form.
3. **Classify.** Determine whether the crash is a true bug or a test-limitation issue:
   - **True bug:** File an issue, add a regression test in `tests/` (not as a fuzz target), and fix the underlying defect.
   - **Test limitation:** Adjust the test's input bounds or assertion logic, and document the adjustment.
4. **Promote.** Add the minimized input as a deterministic regression test:
   - For pure-function crashes: add to the appropriate `tests/property_*.rs` file with an explicit `#[test]` that asserts the expected behavior.
   - For adversarial corpus cases: add a new entry to the appropriate `tests/corpus/adversarial/*.json` file.
   - For integration-level crashes: add to `tests/integration.rs` or `tests/corpus_runner.rs`.
5. **Verify.** Run `make check` to confirm the regression test passes and no existing tests break.
6. **Never re-fuzz blindly.** After promotion, add the minimized input to the fuzz target's seed corpus (if applicable) to prevent regression.

Fuzz-only dependencies (`libfuzzer-sys`) do not enter the runtime dependency graph.

All hardening tests are included in `make check` and run in CI.
