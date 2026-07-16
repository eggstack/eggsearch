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

### Fetch Limits (`tests/property_fetch_limits.rs`)

Tests for `src/fetch/limits.rs`:
- `validate_url` rejects empty strings, non-http schemes, oversized URLs
- `validate_url` rejects localhost/private IPs when disallowed
- `validate_url` accepts them when allowed
- `validate_url` accepts valid public HTTPS URLs

## Adversarial Corpus

JSON corpus files in `tests/corpus/adversarial/`:

| File | Purpose |
|------|---------|
| `html_malformed.json` | Malformed HTML: nested elements, broken attributes, XSS vectors, consent pages |
| `structured_text.json` | Malformed JSON/YAML/TOML/CSV/XML, long lines, mixed line endings |
| `url_edge_cases.json` | Non-http schemes, SSRF IPs, credentials, Unicode URLs, port edge cases |
| `sanitize_edge_cases.json` | Bidi overrides, ZWJ sequences, homoglyphs, null bytes, prompt injection |
| `identity_edge_cases.json` | Unusual schemes, fragments, backslashes, multi-slash, encoded chars |

`tests/adversarial_corpus.rs` validates that all corpus files are well-formed JSON with non-empty case arrays.

## Running

```bash
make hardening                          # all property tests + corpus validation
cargo test --locked --all-features --test property_sanitize  # sanitize only
cargo test --locked --all-features --test property_identity  # identity only
cargo test --locked --all-features --test adversarial_corpus # corpus validation
```

All hardening tests are included in `make check` and run in CI.
