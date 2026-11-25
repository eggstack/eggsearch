# ADR-0026: Web Acquisition Expansion — Snapshot Persistence, Container Formats, and Instruction-Signal Scope

Status: accepted

Date: 2026-11-24

Decision owners: project maintainers

Related specification sections:

- `plans/000-long-term-specification.md` §4
- `plans/001-terminology-and-domain-model.md` §4-5

Affected subsystem roadmaps:

- `plans/subsystems/web-acquisition-expansion-roadmap.md`

Related records: ADR-0006 (deterministic retrieval), ADR-0007 (format artifact
availability), ADR-0008 (PDF extraction fidelity), ADR-0011 (document
addressing), ADR-0014 (render bounds), ADR-0021 (optional local knowledge
index, the storage precedent).

Supersedes: none.

## Context

The web acquisition expansion roadmap has three remaining milestones that
cannot be settled inside an implementation plan:

1. **M003 change comparison** needs durable per-URL content snapshots. The
   current fetch cache is an in-memory LRU (`src/fetch/cache.rs:342`) with raw
   and derived byte caps; nothing survives a restart, so "did this change since
   last time" cannot be answered.
2. **M004 container documents** needs a format and dependency decision. No
   archive, Office, or EPUB crate is a dependency today, and the 1.89 MSRV plus
   the packaging guards constrain the choice.
3. **M005 instruction-signal screening** proposes a new trust field. Whether
   that field should exist at all is a trust-semantics decision, not a local
   implementation choice.

These are recorded together because they share one set of constraints for the
roadmap: additive optional surface, bounded I/O, deterministic output, no model
calls in the server, no new MCP tool, and no second binary SKU.

## Decision drivers

- Derived state is opt-in and bounded; nothing durable is written silently.
- Determinism over wall-clock time, hash iteration, or parser ordering.
- Reuse existing dependencies and hardening patterns rather than adding a
  second storage or archive stack.
- Fail closed on malformed, encrypted, or oversized input.
- Never imply a safety guarantee the server cannot enforce.

## Decision 1 — Durable snapshots use SQLite behind an opt-in store

### Considered options

- **A. File per snapshot.** Rejected: pruning, atomic replacement, concurrent
  readers, and deletion hygiene are all worse than a single bounded database.
- **B. Reuse the in-memory fetch cache.** Rejected: no restart durability, so
  the milestone's user-visible outcome is impossible.
- **C. SQLite through the existing bundled `rusqlite`.** Selected. It is already
  a normal dependency, and `src/meta/knowledge_delta/index.rs:157` is a working
  precedent for bounded pages, `secure_delete`, restrictive permissions, and
  typed degradation.
- **D. A new database or service dependency.** Rejected: operational and
  packaging weight for an opt-in comparison feature.

### Decision

A new opt-in snapshot store lives under `[fetch.snapshots]`, disabled by
default. It stores sanitised extracted text and metadata only, and it serves
M003's `change` response.

### Storage contract

```toml
[fetch.snapshots]
enabled = false
path = "/optional/override/snapshots.sqlite3"
max_entries = 10000
max_bytes = 134217728
retention_days = 30
```

- Default path: `<data_local_dir>/eggsearch/snapshots.sqlite3`, directory mode
  `0700`, file mode `0600`.
- One table keyed by (scope, normalised URL, extract mode, scoping fingerprint,
  representation class, caller key), with `content_sha256`, sanitised `text`,
  character count, `fetched_at`, and `last_seen`. An index on `last_seen`
  supports deterministic pruning.
- **Scope correction.** The key includes `CacheScope` — anonymous or the opaque
  profile ID. The draft M003 plan omitted scope; without it, profile-scoped
  content would leak across profiles.
- `secure_delete = true`, `journal_mode = DELETE`, `busy_timeout = 5000`, and a
  `max_page_count` derived from `max_bytes`, matching the knowledge-index
  hardening.
- `user_version` carries the schema version. A newer schema is refused with a
  typed error rather than opened read-write.
- Pruning after each write: expired entries first, then oldest `last_seen`,
  then lexicographic key, until both entry and byte caps hold.
- Privacy: never raw HTML, request headers, credentials, TLS material, or
  browser profile data. Browser-rendered responses are **not** snapshotted in
  v1 because a persistent profile can personalise the body.

### Diff and determinism

- Normalisation applied identically on write and read: Tier-1 sanitisation, LF
  line endings, trailing whitespace stripped per line, trailing blank lines
  removed. No Unicode compatibility normalisation, which would alter content.
- Equality uses SHA-256 over the normalised text; `sha2` is already a normal
  dependency. Addressing uses the existing FNV-1a rules. No random IDs.
- Line diff uses `similar 2.7.0` (Apache-2.0, MSRV 1.60, already in the lock as an
  `httpmock` dev-transitive) with the Patience algorithm and three lines of
  context.
- The cost guard is **size-based, not time-based**: if the line counts exceed a
  fixed bound, the response reports added/removed counts with
  `truncated: true` and no hunks. A wall-clock deadline would make output
  non-deterministic and is explicitly rejected.
- `removed` requires the handler to observe 404 or 410 before error conversion;
  the snapshot is retained and marked last-seen.

### Consequences

Positive: restart-durable comparison, bounded and prunable, reuses
`rusqlite`, and adds no dependency. Negative: a new at-rest surface, opt-in
default off, browser-rendered content excluded, and a small change to the
non-success fetch outcome so 404/410 remain observable.

## Decision 2 — Container formats use zip 9 plus calamine behind an optional feature

### Considered options

- **A. `zip 2.6` for EPUB, DOCX, and PPTX, defer XLSX.** No second archive
  major, but drops a requested format and leaves spreadsheet handling absent.
- **B. `zip 9` for all containers plus `calamine 0.38` for XLSX.** Selected:
  one archive major, correct spreadsheet dates and number formats, and one
  dependency edge (calamine already pins zip 9).
- **C. `calamine` only.** Rejected: it does not read DOCX, PPTX, or EPUB.
- **D. ODF and legacy binary formats through more crates.** Rejected as scope.

### Decision

A new optional `documents` feature provides CSV/TSV, EPUB, DOCX, XLSX, and PPTX
extraction. It is non-default for the first release that carries it;
promoting it to the default set is a separate packaging change with size
measurements.

### Dependency contract

| Concern | Choice | Evidence |
|---|---|---|
| Archive | `zip` major 9, exact minor pinned at implementation | MSRV of `9.8.1` is exactly 1.89; MIT |
| Compression | `default-features = false`, `deflate-flate2` | `flate2` already uses the Rust backend |
| XLSX | `calamine 0.38` with `dates` | MSRV 1.88; MIT; depends on zip 9 |
| XML | `quick-xml 0.41` (existing) | no new crate |
| HTML/EPUB text | `scraper 0.20` (existing) | no new crate |
| CSV/TSV | none | `src/fetch/render/csv.rs` already renders a bounded table |

If no zip 9 minor with MSRV at or below 1.89 is available at implementation
time, XLSX is deferred and option A is taken. A second archive major is not
accepted as a workaround.

### Format and bounds contract

- Detection: content type, then URL extension, then container introspection —
  `mimetype` for EPUB, `[Content_Types].xml` for OOXML. The `PK` magic alone is
  insufficient.
- Bounds: 16 MiB input, 64 MiB decompressed, 100:1 ratio, 4,096 entries,
  4,096 parts, 200,000 characters before the caller budget, XML depth 64, no
  DTDs or external entities. ZIP central-directory sizes are attacker-controlled,
  so actual inflated bytes are counted by a bounded reader, and the pre-scan
  runs before `calamine` inflates the archive.
- Determinism: DOCX body order, PPTX `presentation.xml` slide order, workbook
  sheet order, EPUB `container.xml` → OPF spine order, fixed table
  serialisation, no locale-dependent number or date rendering.
- Outcomes: content, `document_empty`, `document_malformed`,
  `document_unsupported`, `document_encrypted`. A malformed container never
  returns an empty success.
- A separate `DocumentFormat` value carries the format. `DocumentKind` is not
  extended, because it is matched exhaustively across the renderer.

### Consequences

Positive: one archive major, correct XLSX handling, no OCR or model. Negative:
two optional dependencies and measurable binary growth; the zip 9 MSRV sits on
the 1.89 boundary, which is why the fallback is recorded rather than guessed.

## Decision 3 — Instruction-signal screening is rejected

### Considered options

- **A. Ship deterministic `injection_signals` (rejected).**
- **B. Keep the existing marker scan and document its limits (selected).**
- **C. Model-based classification.** Not available: the server makes no model
  calls.
- **D. Structural-anomaly warnings only.** Deferred to a separate sanitisation
  plan if the underlying gaps matter.

### Decision

M005 is a **no-go**. No `injection_signals` field ships, and no corpus is
assembled for it.

Reasons, in order of weight:

1. **The signal already exists.** `scan_injection_markers`
   (`src/core/sanitize.rs:221`) detects override phrases, system/assistant
   role prefixes, and ChatML tags, records byte offsets, and `TrustMarkers`
   reports the counts. The marginal capability is spans and excerpts, not
   detection.
2. **Invisible-character evasion is already handled.** Tier-1 sanitisation
   strips bidi, zero-width, and control characters, and the marker scan strips
   them before matching so `ign<U+200B>ore` still matches.
3. **The precision bar is unattainable with fixed patterns.** Quoted attacks in
   security articles, API docs, and legal text are ordinary benign content. The
   plan's bar — at most two reviewed false positives on 200 benign pages with at
   least 80 percent recall — is a model-class target, and model calls are
   forbidden here.
4. **Hidden text is an extraction gap, not an instruction signal.**
   `aria-hidden="true"` is skipped (`src/fetch/render/blocks.rs:342`), but
   `display:none` and `visibility:hidden` text survives because extraction has
   no CSS engine. That is a fidelity defect with a deterministic fix, and it
   should not be marketed as injection detection.
5. **Homoglyph mixing cannot be separated from legitimate multilingual text**
   without a model.
6. **A weak field implies a guarantee.** The honest contract is
   `external_untrusted` plus framing plus marker counts. An
   `injection_signals` field would add payload and review burden while
   overstating what the server knows.

### Consequences

- The fetch trust contract is unchanged.
- `docs/safety.md` should state the marker scan's scope and limits if that is
  not already explicit.
- If structural anomalies are wanted, a separate sanitisation plan owns
  CSS-hidden text and confusable-script handling. It must not be presented as
  instruction detection.
- M005 is closed as no-go rather than left proposed, so the roadmap does not
  imply pending work.

## Compatibility and migration

- `[fetch.snapshots]` is additive and disabled by default; the existing fetch
  cache and its keys are unchanged.
- `documents` is additive and non-default; PDF behavior and the default feature
  set are unchanged.
- No tool is added, renamed, or removed; the advertised surface stays at nine
  tools plus three aliases.
- An operator who never enables the new sections sees no new files, network
  behavior, or payload fields.

## Security implications

- The snapshot store is a new at-rest surface: opt-in, `0600`, sanitised text
  only, no raw bodies or credentials, `secure_delete` on prune, an explicit purge
  path, and browser-rendered content excluded.
- Container parsing stays in memory, never extracts to disk, counts actual
  inflated bytes, and resolves no external entities.
- Rejecting M005 avoids a false assurance of instruction detection.

## Verification

- M003: golden diffs (insert, delete, modify, reorder, whitespace-only,
  Unicode), determinism across repeated runs, status transitions, prune and
  retention, disabled-store degradation, scope isolation, and an inspection
  proving no raw body is stored.
- M004: a per-format fixture corpus including malformed, empty, encrypted, and
  decompression-bomb inputs; ordering, part addressing, caps, and the four
  outcomes.
- M005: this record and the roadmap status change only. No code, no corpus, no
  latency claim.

## Supersession

None. A later change to the storage protocol or the format dependency set
supersedes the affected decision only, through a new ADR that links here.
