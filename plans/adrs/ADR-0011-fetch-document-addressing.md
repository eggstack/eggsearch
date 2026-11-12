# ADR-0011: Fetch Document Addressing — Offsets over Opaque Cursors

Status: accepted
Date: 2026-11-12
Decision owners: project maintainers
Affected subsystems: MCP tool surface (`web_fetch`, `batch_fetch`, `research_search`); fetch extraction and cache; evidence and citation identity.
Related: ADR-0001 (ten-tool surface); `plans/implementation/agentic-retrieval-integrity/009-long-running-progress-and-pagination.md` (superseded in part); `018-pagination-consistency-across-pages.md` (superseded in part: its page-budget mechanism, not its defect statements or its scope labels); `019-fetch-document-budget-and-truncation-semantics.md`; `020-offset-addressing-and-cursor-retirement.md`.

## Context and forces

M009 introduced a 512-byte opaque replay cursor (`e1.…`) for `web_fetch`, `batch_fetch`, and
`research_search`, carried in `next_cursor` and echoed back as `cursor`. It is self-describing,
stateless server-side, and guarded by a derived plan key. It also shipped **unreleased**: the crate
is at 0.4.4, the newest tag is `v0.4.4`, and the cursor appears only under `Unreleased` in
`CHANGELOG.md`. So its shape is still a decision, not a compatibility obligation.

A review of that design produced four findings.

**The `web_fetch` cursor cannot return more than one call already returns.** Extraction truncates
blocks at the caller's `max_chars` (`src/fetch/render/blocks.rs`), and chunking consumes that same
already-truncated block set (`src/fetch/client.rs:1124`). Page capacity is *also* `max_chars`
(`src/mcp/tools/web_fetch.rs:993`). Available text and page size are therefore equal by
construction, so a second page can only ever be the arithmetic spill of per-chunk separator
accounting. A live trial at `max_chars = 12000` produced 14 chunks then 1 chunk — the second page
was ~800 characters of spill, not a second page of document.

**A cursor only walks forward.** Reading a conclusions section at character 40,000 requires
requesting every preceding page. Document comprehension is inherently random-access, and an opaque
token cannot be reasoned about, retried meaningfully, or inspected for proximity to the end.

**The cursor's core justification does not apply here.** Opaque cursors are recommended because
offset pagination skips and duplicates records when the underlying collection mutates between
calls. A fetched document is immutable within its cache TTL and content-addressed: the derived cache
key already includes `raw_content_hash`, and `doc_id`/`chunk_id` are content-derived FNV-1a. If the
page changes, the hash changes and the key stops matching — a detectable mismatch, not a silent
skip. The property cursors exist to protect is already provided by the cache key.

**MCP offers no range primitive to build on.** `resources/read` returns whole `contents`; there is
no protocol-level chunking, streaming iterator, or byte-range negotiation. Addressability has to be
a tool-level affordance, so there is no convention to inherit.

Two further constraints come from outside the repository. Practitioner guidance for large documents
recommends exactly this shape — read tools taking `offset`/`limit`, with truncation stated
explicitly ("displayed lines 1-200 of 5000; use the offset parameter to continue"). And a study of
MCP session logs found no agent-initiated second-chunk requests with a cursor at all, concluding
that the first slice must contain the answer. Taken together: random access must be cheap, and
*selection* matters more than page walking.

## Considered alternatives

- **Keep the opaque cursor and fix only the page budget.** Rejected: M018's work makes page shape
  exact but cannot create content the extractor never produced, and the forward-only walk remains.
- **Add `offset`/`limit` alongside `cursor`.** Rejected: two addressing schemes for one document
  poses the schema a question it should not be able to ask, and an agent deciding between them
  spends a turn guessing.
- **Address documents by `chunk_id` only.** Rejected as the *reading* address: chunk boundaries are
  an extraction artefact, so a caller would need the chunk index before it could read anything, and
  a chunk can exceed a useful window. Chunk ids stay the citation address.
- **Return an MCP `resources/read` URI.** Rejected: no range support, and it moves document reading
  out of the tool contract that already owns fetch semantics.
- **Out-of-band pointer with a signed download URL.** Rejected: adds a transport surface, a
  signing secret, and an expiry mechanism to solve a problem bounded by the cache TTL.

## Decision

- **Document reads are addressed by a derived request key plus character offsets.** The call is the
  original fetch request re-specified with `offset` and `limit`; the plan key is derived
  server-side from that request and never echoed by the caller. There is no opaque token.
- **The unit is Unicode scalar values**, matching the `chars().count()` convention used throughout
  the codebase. Byte offsets would be cheaper and wrong for the audience.
- **Ranges are half-open `[from, to)` and clamp rather than error.** An offset past the end returns
  an empty window with `text_from == text_total`, never a validation failure.
- **Offsets address pre-framing text.** The trust envelope (`<<<EXTERNAL_UNTRUSTED …>>>`) is applied
  to the returned slice, so framing characters can never shift an offset.
- **`text_total` must be the true addressable total.** The document is extracted at the internal raw
  budget (`max_chars_cap`), and the caller's `max_chars` bounds the returned window, not the
  extractable document. This is a prerequisite, not an optimisation: without it `text_total` merely
  echoes the caller's own truncation and offsets address nothing useful.
- **Truncation is reported as two distinct facts**: the document was truncated at the internal
  budget, and the window was clipped to the requested limit. A caller must never be told "not
  truncated" while holding less than the document contains.
- **`chunk_id` remains the citation and evidence address.** Reading and citing are different jobs
  and get different addresses; neither replaces the other.
- **Collections use index offsets under the same derived key.** `batch_fetch` and
  `research_search` return records rather than text, so their offset unit is the item index; the
  same content-derived key guards a changed set. The opaque cursor codec is retired, not retained
  as a legacy path.
- **Selection is the primary lever for first-slice usefulness.** `focus` and heading-aware slicing
  decide whether the first window contains the answer, and are measured on that basis (first-slice
  precision) rather than on page count.

## Consequences and tradeoffs

Advantages: random access in one call; a surface an agent can reason about and retry without
echoing state; simpler code (the codec and its 512-byte budget disappear); and a cache that
improves, because the extraction key stops varying by caller budget and entries become shareable
across callers.

Tradeoffs: the whole document is extracted on a cache miss even when the caller reads a small
window, so CPU and memory per fetch rise; `max_chars_cap` stops being a validation ceiling and
becomes a real cost knob; and chunk ids change for existing fixtures because boundaries move. No
persistence or cross-request state is introduced, so nothing new can be forged.

## Compatibility, security, and migration

The cursor surface is unreleased, so this is a replacement rather than a deprecation: no released
client observes `next_cursor`. Ten MCP tools are unchanged; only arguments and additive response
fields move. Sanitization tiers, trust framing, bounded reads, redirect policy, and cache isolation
are unaffected — extraction grows, sanitisation still runs on output. Offsets are bounded by the
internal budget and cannot be used to read outside the cached document.

## Verification and status

Adoption authorizes the shape; it does not change behaviour. Implementation lands through
`019-fetch-document-budget-and-truncation-semantics.md` (the prerequisite) and
`020-offset-addressing-and-cursor-retirement.md`, each with byte-identity tests for cursor-ignoring
callers, boundary tests for clamping and UTF-8 safety, and a measured first-slice precision record.
Until those close, the shipped behaviour is unchanged.
