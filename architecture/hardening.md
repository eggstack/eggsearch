# Hardening

**Location:** `src/fetch/limits.rs`, `src/fetch/client.rs`, `src/fetch/origin.rs`, `src/core/sanitize.rs`, `src/core/identity.rs`, `src/core/retrieval_status.rs`, `src/meta/safe_open.rs`, `src/core/local.rs`, `src/meta/engines/mod.rs`, `src/meta/local_inventory_cache.rs`
**Purpose:** Every untrusted input is validated first, bounded always, sanitized by tier, identified deterministically, and failed softly. Property tests, adversarial corpora, fault injection, and fuzz targets pin the invariants.

---

## Bounded everything

No untrusted I/O runs without a hard cap. Defaults live in `FetchLimits::default()`:

| Bound | Default | Enforced by |
|-------|---------|-------------|
| URL length | 8,192 bytes | `validate_url` rejects oversized URLs |
| Response body | 2,000,000 bytes | `read_bounded_body` streaming cap plus `Content-Length` precheck |
| Extracted text | 12,000 chars default, 50,000 chars cap | `bound_text` and per-tool `max_chars` clamping |
| Timeout | 8,000 ms (DNS phase at most half, 1,500 ms floor) | Shared transport for equal/shorter overrides, one widened client for longer overrides |
| Redirects | 5 | Manual chain walk; every hop re-validated |
| PDF | 25 pages, 12,000 chars per page, 50,000 chars total | PDF extractor caps (`pdf` feature) |

Engine response bodies use one shared implementation: `read_bounded_body()` in `src/meta/engines/mod.rs` checks `Content-Length` upfront, then streams through `push_bounded_chunk()`, which aborts with `ParseFailed` the moment `max_bytes` is crossed. Most engines cap at 2 MiB; NVD and RustSec allow 5 MiB; the KEV catalog path allows 100 MiB with an explicit oversize error. Forge every API response through the bounded reader; never call bare `.text()` or `.bytes()`. The same rule covers forge adapters, which additionally enforce entry, depth, byte, pagination, and concurrency budgets behind a global semaphore.

Git execution is bounded by `run_bounded_command()` in `src/meta/local_inventory_cache.rs`: 16 MiB stdout cap, 64 KiB stderr cap, inventory build timeout, and process-group kill on timeout or cap breach (new session via `setsid`, whole-group termination, spawn-failure and truncation recorded explicitly). The fetch cache enforces separate raw and derived byte maxima so a large raw hit cannot blow up derived storage. Browser startup and navigation carry their own millisecond timeouts.

Character bounds respect Unicode boundaries: `bound_text` clamps on char counts, never splits UTF-8, and appends a single `…` marker with an accurate truncated flag. `truncate_at_word` is the word-aware variant for prose bodies.

### Truncation evidence

`TruncationEvidence` in `src/core/retrieval_status.rs` distinguishes proven from suspected truncation:

| Variant | Meaning |
|---------|---------|
| `None` | Complete as far as anyone can tell |
| `LimitReachedUnknown` | A candidate limit saturated but missing data is unproven; the dimension stays `Satisfied` |
| `ConfirmedByEggsearch` | eggsearch itself truncated (cap, bound, or `TruncatedAfterPartialSuccess`) |
| `ConfirmedByProvider` | The provider reported truncation |

Use `LimitReachedUnknown` unless missing data is proven. Only confirmed variants map the dimension to `Partial`. `effective_truncation_evidence()` infers the confirmed-eggsearch case from the legacy truncated flag so old attempts keep their meaning.

---

## SSRF validation first

Validation runs before any socket opens, and again on every redirect target. Two layers share one policy:

1. `validate_url()` — synchronous shape check: non-empty, `http`/`https` only, within `max_url_len`, no embedded credentials, no blocked literal IP or localhost, no private hostname unless `allow_private_network`.
2. `validate_fetch_target()` — full pipeline: repeats the shape checks, then resolves DNS under a bounded timeout and validates every returned address, pinning the outbound attempt to the validated address set so a record cannot change between check and connect.

IP classification (`classify_ip` in `src/fetch/limits.rs`) covers loopback, private (10/8, 172.16/12, 192.168/16, ULA, mapped and compatible v6), link-local, carrier-grade NAT (100.64/10), documentation, multicast, and reserved ranges. `is_allowed_by_policy` maps classes onto the two operator flags: public is always allowed, loopback needs `allow_localhost`, everything else needs `allow_private_network`. Hostname policy additionally blocks `localhost`, `host-gateway`, container and cloud-metadata names, and `.internal`, `.private`, `.local`, `.corp`, `.lan`, `.home`, `.home.arpa`, `.invalid`, `.test`, and `.localhost` suffixes. Embedded credentials (`user@`, `user:pass@`) are rejected outright, including on redirect targets. IPv6 literals (including zone-ID forms) and v4-mapped addresses classify through the same path.

Forge endpoints and the egress dialer apply the same fail-closed posture: loopback and private destinations are rejected by default, cross-origin redirects are rejected, credential-bearing endpoints are rejected even under internal policies, and malformed proxies fail closed rather than falling back to direct.

### Redirect chain

Redirects are followed manually, never by the transport: `follow_redirects(false)` on the client, then a hop-by-hop walk in `FetchClient` bounded by `redirect_limit` (5). Every `Location` target re-enters `validate_fetch_target()` — scheme, credentials, literal-IP policy, DNS resolution, and address-set pinning — so a benign first hop cannot launder a blocked second hop. Missing `Location` headers, invalid UTF-8 locations, credential-bearing redirect targets, and cross-origin forge redirects each produce structured errors, not silent truncation. The `validate_redirect_chain` fuzz target feeds multi-hop sequences through the same validator to pin the chain semantics.

## Three-tier sanitization

All untrusted text flows through `src/core/sanitize.rs`. Tier 1 is always on; Tiers 2 and 3 are gated by `[search].sanitize_output` / `[fetch].sanitize_output` (default true) and the `FetchClient` `sanitize_output` flag:

| Tier | Function | Behavior |
|------|----------|----------|
| 1 — strip and bound | `strip_control_chars`, `bound_text` | Removes NUL, CR, ASCII control ranges, bidi overrides, zero-width characters, and line/paragraph separators; preserves `\n` and `\t`; clamps to `max_chars` with `…` and an accurate flag |
| 2 — frame | `frame` | Wraps the field in `<<<EXTERNAL_UNTRUSTED field=... id=...>>>` … `<<<END>>>` delimiters so downstream agents can see the trust boundary |
| 3 — scan | `scan_injection_markers` | Reports prompt-injection patterns (`ignore previous instructions`, `disregard`, `system:`/`assistant:` headers, `<\|im_start\|>`/`<\|im_end\|>`, ChatML tags) as byte-offset hits without modifying text |

Title and snippet caps (`TITLE_MAX_CHARS = 200`, `SNIPPET_MAX_CHARS = 500`) apply Tier 1 at the card level. `TrustMarkers` records what happened per field (`text_sanitized`, `text_truncated`, `text_framed`, `control_chars_removed`, `injection_hits`) and merges across fields with OR-of-booleans and saturating sums, so batch and bundle responses carry exact aggregate markers. `sanitize_output = false` is a rendering choice for trusted pipelines only; it never disables Tier 1.

```rust
let (cleaned, removed) = strip_control_chars(raw);
let (bounded, truncated) = bound_text(&cleaned, SNIPPET_MAX_CHARS);
let hits = scan_injection_markers(&bounded);
let framed = frame(&bounded, "snippet", &stable_id);
```

`scan_injection_markers` returns byte offsets guaranteed valid for the scanned string, so highlighting and redaction cannot split UTF-8. The scanner is total over arbitrary input — property tests assert non-panic, offset validity, and determinism — and the full strip → bound → scan pipeline is one of the three `make fuzz-smoke` targets.

---

## Deterministic IDs, never UUIDs

Stable IDs are content-derived FNV-1a 64-bit hashes (`src/core/identity.rs`), never random UUIDs. Every hash input carries the versioned prefix `eggsearch-id-v1\0` plus an entity sub-namespace, length-prefixed fields to prevent boundary ambiguity, and URL canonicalization (lowercased scheme, stripped fragments and trailing slashes, default-port normalization) before hashing. Cross-type IDs never collide by construction; the prefix bump path enables future algorithm migration. Changing ID semantics breaks corpus regression fixtures and cross-tool dedup, so the `schema_identity_registry` suite pins them.

Entity namespaces and their identity keys:

| Prefix | Entity | Hashed fields |
|--------|--------|---------------|
| `src_` | Source card | Canonical URL, title, provider identity |
| `fetch_` | Fetch response | Canonical URL, extraction parameters |
| `doc_` | Document | Canonical URL, document coordinates |
| `suggested_` | Suggested fetch | Source URL, suggested target |
| `batch_` | Batch result | Member fetch identities |
| `chunk_` | Document chunk | Document ID, chunk index and bounds |
| `loc_` | Locator | Repository coordinates, path, ref |
| span | Code span | File identity, span bounds |

Field order is insignificant by design (one `write_str` per field under the entity prefix), while field boundaries are significant (length prefixes defeat `"ab"+"c"` versus `"a"+"bc"` ambiguity). Unicode normalization is byte-exact: fullwidth and ASCII forms hash differently rather than silently merging.

---

## Local filesystem containment

`validate_local_fetch_path()` (`src/core/local.rs`) rejects empty, absolute, traversal (`..`), hidden (unless `include_hidden`), skipped-directory (`target`, `node_modules`, `.git`), binary-extension, oversized, and non-file targets. `safe_open_relative()` (`src/meta/safe_open.rs`) then opens race-free: component-wise walking, NUL rejection, and on Linux descriptor-relative `openat2` with kernel-enforced `RESOLVE_BENEATH | RESOLVE_NO_MAGICLINKS` (plus `RESOLVE_NO_SYMLINKS` when `follow_symlinks = false`), falling back to `openat` with `O_NOFOLLOW` only where `openat2` is unavailable. Symlink escapes, intermediate symlinks, loops, root replacement, and concurrent modification during validation are all rejected or handled; reads after open enforce a hard byte cap. Never bypass these two functions with a direct filesystem read for operator-supplied paths.

`SafeOpenError` taxonomy (`Empty`, `AbsolutePath`, `PathTraversal`, `NullByte`, `NotFound`, `SymlinkDetected`, `NotAFile`, `FileTooLarge`, `FileContentLimitExceeded`, `SafeSymlinkFollowingUnsupported`, `RootOpenFailed`, `Io`) keeps every rejection machine-readable: callers map variants to retrieval attempts and absence kinds instead of string-matching messages. On non-Linux Unix and non-Unix platforms, `follow_symlinks = true` returns `SafeSymlinkFollowingUnsupported` rather than attempting containment without a race-safe primitive.

---

## Forge and engine budgets

Every search-engine adapter funnels HTTP bodies through `read_bounded_body()` with a per-engine `MAX_BODY_BYTES`: 2 MiB for the standard engines (DuckDuckGo, Brave, Mojeek, Yahoo, Startpage, Exa, Tavily, SearxNG, Crossref, OpenAlex, Semantic Scholar, Sourcegraph, GitHub/GitLab/Gitea code, issues, and releases, GitHub advisories, OSV, Firecrawl developer), 5 MiB for NVD and RustSec feeds, and 100 MiB for the KEV catalog bulk download with an explicit oversize error naming the limit. The `Content-Length` precheck rejects oversized responses before the first byte streams; the chunk loop aborts the moment the cap is crossed. The KEV path additionally pre-sizes its buffer at `min(MAX_BODY_BYTES, 64 KiB)` so a hostile length header cannot force a giant allocation.

Forge adapters (`forge_adapter`, code-host fetch) enforce entry, depth, byte, pagination, and concurrency budgets behind a shared semaphore (`MAX_CONCURRENT_FORGE_REQUESTS`): nested repository maps preserve entries only within depth bounds, refs with slashes and 40-hex SHAs encode without ambiguity, Gitea without a base URL reports a structured configuration failure instead of guessing, and `commit_sha` always comes from `resolved_ref`, never the entry object SHA.

## Process, cache, and transport bounds

Git subprocesses run through `run_bounded_command()` with a process-group kill switch: the child starts in a fresh session (`setsid`), a watchdog thread enforces the inventory build timeout, and on timeout or cap breach the whole process group is terminated so orphaned grandchildren cannot linger. Results record how the command ended (`SpawnFailed`, timeout, truncation flags) instead of silently dropping output; stdout at or above the 16 MiB cap marks the inventory stale-safe rather than poisoning the cache. The `mock`-gated `test_harness` module exposes the same runner (`run`, `run_for_inventory`, `INVENTORY_TIMEOUT`, `STDOUT_CAP`, `STDERR_CAP`) so `bounded_command` tests exercise production code paths.

The fetch cache separates raw and derived maxima: a fresh raw hit with a derived miss re-runs the shared extraction pipeline locally instead of issuing another network request, and derived keys bucket by `max_chars` so differently-bounded views never share entries. Timeout overrides reuse the shared transport for equal or shorter values and build exactly one widened client for longer values, so resolved-route connect policy matches the requested timeout without rebuilding per call. Batch setup performs one timeout adjustment per call regardless of batch width. Local inventory and derived-cache internals use shared immutable ownership; public owned-return wrappers sit at the boundary so hot paths never copy deeply.

Browser rendering carries millisecond startup and navigation timeouts plus per-origin browser concurrency of 1 (HTTP concurrency 2), enforced by the same `OriginController` semaphores as direct fetches. Dynamic fetch targets (`web_fetch`, `batch_fetch`, `repo_fetch`) stay direct with resolved-address pinning; only provider upstreams may use the `egress` route, and chain failures never fall back to direct.

## Absence versus failure

A dimension that found nothing is not a dimension that failed. `EvidenceAbsenceKind` separates completed-no-match from retrieval failure; `RetrievalDimensionState` makes the terminal reading authoritative (`Satisfied`, `CompletedNoMatch`, `Failed`, `SkippedByPolicy`, `CapabilityUnavailable`, `Interrupted`, `Partial`, `NotApplicable`). Provider-scoped advisory outcomes preserve every selected provider's identity, zero result, capability skip, deadline, and error attempt — collapsing them would hide exactly the signal operators need. Required-role-missing after successful retrieval reads `insufficient`; required-role-indeterminate after provider failure reads `indeterminate_due_to_failures`. Conflict metadata fires only for directly comparable values sharing an entity key, so unrelated sources never manufacture disagreement.

## Soft failures and backpressure

The metadata adapter returns a response, never an error: every provider contributes a retrieval attempt (success, zero-result, capability skip, deadline, or structured error) and partial failure yields partial results. Panics are contained per provider, hangs are cancelled on timeout, and output ordering is deterministic regardless of completion order. Health tracking (unknown → healthy/degraded, cooldown after repeated failures, success clears cooldown) and per-origin backpressure sit underneath: `OriginController` (`src/fetch/origin.rs`) enforces per-origin semaphores, bounded retries with backoff, and circuit breaking, so one hot or hostile origin cannot starve the rest.

`OriginPolicy` defaults:

| Knob | Default | Effect |
|------|---------|--------|
| `http_concurrency` | 2 | At most two concurrent HTTP fetches per origin |
| `browser_concurrency` | 1 | At most one browser render per origin |
| `retry_max_attempts` | 2 | Bounded retries, then the attempt fails structured |
| `retry_base_delay_ms` / `retry_max_delay_ms` | 250 / 4,000 | Exponential backoff window per origin |
| `circuit_failure_threshold` | 3 | Consecutive retryable failures open the circuit |
| `circuit_duration_ms` | 60,000 | Open-circuit window before the origin is retried |

Permits are `OwnedSemaphorePermit`s scoped to the fetch; success resets the failure counters while retryable, rate-limited, and non-retryable classes feed distinct backoff paths. The probe service (`src/meta/probe.rs`) applies the same discipline to provider health checks with its own `PROBE_MAX_CONCURRENCY` semaphore, so probing cannot wedge dispatch.

---

## Property, adversarial, and fault-injection strategy

- **Property (`tests/property_*.rs`, 17 suites):** invariant assertions over arbitrary input — determinism, idempotency (`strip_control_chars`, `canonicalize_url`), bound respect, UTF-8 safety, offset validity, merge commutativity/associativity (`TrustMarkers`), root containment, ledger validity, dependency-parse dispatch determinism and per-file budgets, and never-panic renderers. Each file names its module under test so the invariant stays beside the code it pins.
- **Adversarial (`tests/corpus/adversarial/`, 9 files, ~270 cases):** hand-built edge corpora. `adversarial_corpus.rs` asserts structural validity; behavioral suites assert handling.
- **Fault injection (`tests/dispatch_fault_injection.rs`):** provider success/partial/total failure, timeouts, hangs, concurrency saturation, panic containment, health and cooldown transitions, deterministic ordering, exact partial-result telemetry. Requires the `mock` feature.
- **Probe conformance (`tests/provider_probe_conformance.rs`):** the shared probe service under the same failure taxonomy plus explicit-request-after-degraded semantics and descriptor source-of-truth for native versus local domain filtering.

Adversarial corpus files:

| File | Cases | Focus |
|------|-------|-------|
| `html_malformed.json` | 24 | Nested elements, broken attributes, XSS shapes, consent pages |
| `html_extended.json` | 31 | SVG, MathML, CDATA, template, noscript, iframe/object/embed, deep nesting, formulas |
| `structured_text.json` | 27 | Malformed JSON/JSONL/YAML/TOML/XML/CSV, diff, patch, long lines, mixed endings |
| `structured_text_extended.json` | 47 | Notebooks, reStructuredText, AsciiDoc, CSV variants, BOM, null bytes |
| `url_edge_cases.json` | 31 | SSRF shapes, credentials, Unicode URLs, port edges |
| `sanitize_edge_cases.json` | 19 | Bidi overrides, zero-width joiners, homoglyphs, null bytes, prompt injections |
| `identity_edge_cases.json` | 16 | Unusual schemes, fragments, backslashes, multi-slash forms |
| `pdf_extended.json` | 28 | Magic bytes, truncated headers, binary content, embedded HTML, encrypted files, malformed xref, cyclic refs |
| `filesystem_extended.json` | 30 | Traversal, symlinks, permissions, Unicode names, special files |

Promote real incidents into the cheapest layer that captures them: pure-function bug → property test; malformed-input handling → adversarial JSON entry; multi-step workflow break → `corpus_runner` scenario; provider failure mode → fault-injection case.

---

## Property test index

16 `proptest` suites pin the pure-function invariants. Representative properties per file:

| Suite | Module under test | Key properties |
|-------|-------------------|----------------|
| `property_sanitize` | `core/sanitize.rs` | Strip output holds no unsafe chars, is idempotent, removal counts match, `\n`/`\t` preserved, `bound_text` respects caps with accurate flags and `…` suffix, Unicode boundaries never panic, framing overhead capped |
| `property_identity`, `property_identity2`, `property_identity3` | `core/identity.rs` | All ID functions deterministic with correct prefixes and lengths, canonicalization idempotent (trailing slashes, fragments, `www`, default ports, case), equivalent URLs hash equal, distinct inputs diverge, cross-type IDs never collide, fullwidth versus ASCII diverge, field order insensitive |
| `property_fetch_limits` | `fetch/limits.rs` | Empty, non-HTTP, oversized, localhost, and private-IP rejection; acceptance when flags allow; valid public HTTPS acceptance |
| `property_fetch_redirects` | `fetch/limits.rs` | Private TLD rejection, CGNAT and 172.16/12 and link-local rejection, public-range acceptance, port boundaries, query and fragment neutrality |
| `property_fetch_url_edge` | `fetch/limits.rs` | `javascript:`/`data:`/`blob:` and exotic-scheme rejection, empty-host rejection, length limits, IPv6 literal acceptance, port 0 and high-port acceptance |
| `property_fetch_response` | `FetchClient` | Credential rejection, metadata-only skips extraction, text mode returns content, `max_chars` respected, `Content-Length` precheck, slow-response timeout, credential-redirect blocked, `sanitize=false` skips framing, redirect count capped, short-body and headerless-redirect grace |
| `property_render_safety` | `core/sanitize.rs` | Safety, idempotency, count accuracy, safe-char preservation (ASCII, CJK, emoji), prefix preservation, ellipsis, UTF-8 boundaries, frame structure, offset bounds, determinism |
| `property_render_code` | `fetch/render/` | Code, diff, plaintext, and CSV renderers never panic, stay bounded, stay deterministic, keep line numbers monotonic and non-overlapping, preserve language metadata, emit no blocks for empty input |
| `property_local_fs`, `property_local_fs_extended` | `core/local.rs` | Segment validity, absolute/relative construction, binary and skip-dir matching, size and count boundaries, symlink accept/reject by flag, intermediate-symlink and escape rejection, traversal and absolute rejection, hidden-path policy, root containment, loop and permission handling, concurrent-modification consistency |
| `property_render_metadata` | `TrustMarkers`, outline | Merge ORs booleans and sums counts, merge commutative and associative, framing delimiters exact, benign versus adversarial detection, chunk IDs deterministic and unique, block references in bounds |
| `property_forge_url` | `core/repo_fetch.rs` | Generated URLs parse and round-trip owner/repo/ref/path, slash-refs and SHAs encode, browser versus raw forms stay on expected hosts |
| `property_conflict` | `core/conflict.rs` | Version-range, date, benchmark, mutable-versus-pinned, and metadata conflicts fire only on comparable values, entity keys scope comparison, output deterministic under permutation |
| `property_retrieval` | `core/retrieval_status.rs`, `core/workflow_coverage.rs` | Ledger accepts well-formed and rejects violations, absence classification separates no-evidence from failure, coverage maps roles and failures deterministically |
| `property_dependency_parse` | `meta/dependency_parse/` | Dispatch deterministic across ecosystems, per-file budgets constant per input, identity/provenance semantics stable, source lines bounded, URL/canonical forms equivalent |

Dispatch fault injection (`tests/dispatch_fault_injection.rs`, requires `mock`) covers the adapter layer the property suites cannot reach: all-succeed, partial-failure, all-fail, timeout-without-blocking, dedup, completion-order independence, hang cancellation, mixed scenarios, `max_results` respect, engine selection, health transitions and cooldowns, concurrency saturation, malformed-metadata tolerance, global deadlines, exact telemetry, counter release on panic, and cross-run determinism.

## Fuzz-target design

23 `cargo-fuzz`/`libfuzzer` targets in `fuzz/fuzz_targets/`, registered in `fuzz/Cargo.toml`. Each target is a thin loop over one parsing or bounding boundary asserting the production invariant:

| Group | Targets | Invariant |
|-------|---------|-----------|
| URL and redirect policy | `validate_url`, `validate_redirect_target`, `validate_redirect_chain` | No panic on arbitrary strings; accept/reject matches the policy matrix |
| Content classification | `validate_content_type` | Content-Type plus URL plus magic-byte classification never panics and stays consistent |
| Extraction | `extract_content`, `extract_content_bytes`, `mixed_utf8_extract`, `extract_pdf_text` | Lossy and raw byte inputs extract within bounds without panicking |
| Sanitize pipeline | `strip_control_chars`, `scan_injection_markers`, `sanitize_pipeline` | Strip → bound → scan composes; strip is idempotent; offsets stay in bounds |
| Bounding | `bounded_response_reader`, `chunk_boundary`, `build_document_chunks` | Byte caps hold across chunked accumulation; chunk IDs stay unique and contiguous |
| Workflow and retrieval | `workflow_kind_parse`, `workflow_resolution`, `research_role_mapping`, `classify_absence`, `detect_entity_scoped_conflicts`, `retrieval_failure_expansion`, `attempt_summary_generation` | Parsers total, roles resolve, absence versus failure stays distinguished, ledger summaries validate |
| Identity | `canonicalize_url` | Canonicalization via `source_id` never panics and stays idempotent |
| Dependency evidence | `dependency_parse` | Arbitrary lockfile bytes across every ecosystem never panic; per-file budgets, line bounds, and canonicalization hold |

Representative shapes:

```rust
fuzz_target!(|data: &str| {
    let limits = FetchLimits::default();
    let _ = validate_url(data, &limits);
});
```

```rust
fuzz_target!(|data: &str| {
    let (cleaned, _) = strip_control_chars(data);
    let (bounded, _) = bound_text(&cleaned, 500);
    let _ = scan_injection_markers(&bounded);
});
```

The bounded-reader target feeds arbitrary bytes as a chunk stream through the production `push_bounded_chunk()` and asserts the cap holds after every append. The dependency-parse target routes arbitrary bytes plus a random ecosystem discriminant through `parse_dependency_file` and asserts determinism, per-file budgets, identity/provenance stability, and line bounds. Smoke coverage is `make fuzz-smoke` (60 s each on `validate_url`, `sanitize_pipeline`, `bounded_response_reader`, `dependency_parse`); full campaigns extend the timer.

### Seed corpus rules

Fuzz seed corpora live in `fuzz/corpus/<target>/` and stay minimal by default: at most 5–10 inputs per target, each exercising a distinct path (valid, malformed, empty, boundary-length, Unicode). No network-derived seeds — never commit live URLs or resolvable host addresses. No seed file over 8 KB; no corpus directory over 100 KB total. Descriptive filenames (`https_with_port.txt`, `empty.txt`); rename `cargo fuzz tmin` outputs before commit. Seed changes review like code.

### Crash promotion

When fuzzing surfaces a panic, hang, or failed assertion:

1. Reproduce locally with the artifact as input and zero extra time budget.
2. Minimize with `cargo fuzz tmin`.
3. Classify as true bug (file an issue, fix the defect) versus test limitation (adjust bounds or assertions, document why).
4. Promote to a deterministic regression test: pure-function crashes become explicit `#[test]` cases in the owning `tests/property_*.rs`; malformed-input crashes become entries in `tests/corpus/adversarial/*.json`; integration-level crashes go to the owning behavioral suite or `corpus_runner.rs`.
5. Verify with `make check`, then add the minimized input to the seed corpus to prevent regression.

Fuzz-only dependencies (`libfuzzer-sys`) never enter the runtime graph. All hardening suites run in `make check` and CI.

Related reading:

- `architecture/testing.md` — suite naming, mock harness, canonical commands
- `architecture/fetch.md` — fetch pipeline, cache, and browser transport
- `architecture/evidence-workflow.md` — retrieval attempts, coverage, truncation
- `docs/test-inventory.md` — per-suite counts and feature gates

---

[← Back to Overview](overview.md)
# Dependency parser failure invariant

Recognized structured TOML and JSON dependency documents retain an explicit
parse status. Invalid syntax is `Malformed`; valid syntax with an unsupported
root shape is `Unsupported`; a valid supported empty document is `Complete`
with no findings. An empty finding list alone is never proof of successful
parsing.
