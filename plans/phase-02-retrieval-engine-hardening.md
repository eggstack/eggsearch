# Phase 2: Retrieval Engine Hardening

Status: ready after Phase 1
Depends on: Phase 1 correctness and security closure
Primary goal: systematically validate retrieval boundaries against malformed, adversarial, and partial inputs.

## 1. Scope

Eggsearch already has a strong example-based test suite. This phase adds the missing layer: property testing, fuzzing, fault injection, and deterministic adversarial corpora around the most security- and reliability-sensitive code paths.

The target is not to maximize test count. The target is to make invariants executable and discover classes of defects that fixture tests cannot anticipate.

## 2. Required Outcomes

- Core safety and resource bounds are expressed as properties.
- Fuzz targets run locally and in a bounded CI configuration.
- Every parser or renderer has a curated malformed-input corpus.
- Timeout, cancellation, panic, partial response, and provider-failure behavior is deterministic.
- Regression artifacts produced by fuzzing can be promoted into normal tests.
- Hardening does not introduce network-dependent CI.

## 3. Workstream A: Test Architecture

### Tasks

1. Add a dedicated hardening test layout, for example:

```text
fuzz/
  fuzz_targets/
  corpus/
tests/adversarial/
tests/property_*.rs
```

2. Select a Rust property-testing library and fuzz harness compatible with the MSRV and release policy.
3. Document how to run short CI fuzz smoke tests and longer local campaigns.
4. Add seed corpus management rules.
5. Define a process for turning minimized crashes into ordinary regression tests.
6. Ensure fuzz-only dependencies do not enter the runtime dependency graph.

### Acceptance

A contributor can run all property tests and a bounded fuzz smoke pass from documented commands without network access.

## 4. Workstream B: Fetch and URL Properties

### Properties

- Parsed accepted targets use only HTTP or HTTPS.
- Embedded credentials are never accepted by full validation.
- Address classification is stable across literal and DNS-resolved forms.
- Redirect resolution cannot widen policy.
- Redirect count never exceeds the configured limit.
- Retained response bytes never exceed `max_bytes`.
- Returned text never exceeds the effective character limit by Unicode scalar count.
- Metadata-only mode never builds full rendered content.
- Equivalent canonical URLs produce equivalent identities where the identity contract requires it.

### Fuzz targets

- URL strings.
- Relative and absolute redirect locations.
- Sequences of redirect targets.
- Content-Type values.
- Content-Length values.
- Chunk segmentation around byte boundaries.
- Mixed UTF-8 and lossy-decoding inputs.

### Fault injection

Introduce a deterministic internal response-stream test adapter if needed so tests can control:

- first chunk size;
- later chunk size;
- stream error after N bytes;
- timeout before headers;
- timeout during body;
- inconsistent Content-Length;
- redirect without Location;
- redirect with invalid encoding.

## 5. Workstream C: Rendering and Document Parsing

### HTML corpus

Cover:

- deeply nested elements;
- malformed comments;
- broken attributes;
- oversized tables;
- nested lists;
- SVG, MathML, XMP, CDATA, template, noscript, iframe, object, and embed content;
- duplicate headings;
- script-like text in benign prose;
- prompt-injection markers split across nodes;
- pathological whitespace and zero-width characters;
- invalid UTF-8;
- pages with no obvious content root;
- consent and block pages.

### Structured text corpus

Cover:

- JSON, JSONL, YAML, TOML, XML, CSV, diff, patch, notebooks, reStructuredText, and AsciiDoc;
- very long lines;
- missing final newline;
- mixed line endings;
- invalid delimiters;
- nested notebook outputs;
- large generated files.

### PDF corpus

When the PDF feature is enabled, cover:

- empty PDFs;
- encrypted PDFs;
- malformed xref tables;
- recursive or cyclic object references;
- compressed streams;
- image-only pages;
- very large page counts;
- metadata-only extraction;
- per-page and total character caps;
- invalid UTF encodings.

### Properties

- Rendering never panics.
- Output block counts and text are bounded.
- Sanitization metadata is internally consistent.
- Unsafe elements never appear as executable content.
- Document chunk identities are deterministic.
- Outline references never point outside the emitted block set.

## 6. Workstream D: Sanitization and Identity

### Sanitization properties

- Tier 1 always removes prohibited controls.
- Tier 2 framing is balanced and deterministic.
- Tier 3 marker counts match emitted warning metadata.
- Bounding never splits Rust strings at invalid byte boundaries.
- Framing overhead cannot cause output to exceed the public cap without an explicit documented rule.
- Sanitization is idempotent where intended, or non-idempotence is documented and tested.

### Identity properties

- Stable IDs are deterministic across process runs.
- Field-order differences in maps do not alter IDs unless specified.
- URL canonicalization is idempotent.
- Default ports and fragments behave according to the identity contract.
- Unicode normalization behavior is explicit and tested.
- Truncated text prefixes never panic and use Unicode scalar boundaries.
- Distinct entity types cannot collide through prefix confusion.

## 7. Workstream E: Local Filesystem Adversarial Testing

### Corpus and properties

- Absolute paths, prefixes, parent components, repeated separators, dot components, and unusual Unicode names.
- Hidden paths and skipped directories.
- Symlink loops.
- Intermediate symlinks.
- Root replacement between inventory and fetch.
- Files removed or changed during scan.
- Permission-denied directories.
- Sparse files and very large reported lengths.
- Non-UTF filenames.
- Multiple roots with overlapping canonical paths.
- Git worktrees and submodules.

Required properties:

- No accepted path escapes its configured canonical root.
- Traversal and file consideration stop at configured bounds.
- Files larger than `max_file_bytes` are not read into memory.
- Errors do not expose unintended file contents.
- Search and direct fetch use equivalent path policy.

## 8. Workstream F: Dispatch and Provider Fault Injection

### Scenarios

- All providers succeed in different completion orders.
- Some providers fail immediately.
- Some time out.
- Provider future panics.
- Spawned task is cancelled.
- Global deadline expires with mixed pending and running jobs.
- Per-provider concurrency is saturated.
- Provider returns duplicate or malformed result metadata.
- Health registry transitions through healthy, degraded, cooldown, and recovery.

### Properties

- Output ordering does not depend on completion order.
- Concurrency limits are never exceeded.
- Every started job reaches a terminal accounting state.
- Provider health counts reflect actual calls.
- Partial-result telemetry is exact.
- Panic handling releases all relevant counters.
- No task survives request completion unexpectedly.

## 9. Workstream G: Regression and CI Integration

### Tasks

1. Add normal deterministic adversarial tests to the required release gate.
2. Add short property-test execution to normal CI.
3. Add bounded fuzz smoke jobs on pull requests or a separate scheduled workflow.
4. Run longer fuzz campaigns on a schedule, not on every commit.
5. Upload minimized crash artifacts from scheduled jobs.
6. Keep all tests offline.
7. Add a test inventory document generated or contract-checked against actual CI commands.

## 10. Verification Matrix

Every boundary should have at least:

- one success fixture;
- one rejection fixture;
- one exact-boundary fixture;
- one over-bound fixture;
- one randomized property;
- one malformed-input fixture;
- one cancellation or failure fixture where asynchronous behavior applies.

## 11. Definition of Done

- All listed target classes have property or fuzz coverage.
- Fuzz smoke tests run in CI within a bounded duration.
- Longer scheduled campaigns produce retained artifacts.
- No hardening test relies on public network access.
- Found defects are converted into ordinary regression tests.
- Full existing feature matrix remains green.
- Documentation explains local and CI execution.

## 12. Handoff Notes

Do not begin by fuzzing the entire MCP server. Start with pure functions and narrow adapters: URL classification, canonicalization, sanitization, renderers, path validation, and dispatch accounting. Expand outward only after useful invariants and corpus promotion are established.