# M048 — Inline HTML Whitespace Fidelity

Status: active
Date: 2026-11-23
Subsystem: agentic retrieval integrity
Milestone class: invariant (new rendering-fidelity milestone)
Baseline: c75bfec9c3cc13a53b3b2c061a326d99b1cc2c38 (docs/user-facing-polish-20261123, clean; based on origin/main)
Source roadmap: §2 current-state gaps, §5 dependency graph, §6 M048
Decision authority: existing HTML text extraction and sanitization contract. M003 remains closed; this plan does not amend or reopen markdown-first negotiation.

References: plans/003-planning-process.md `7; architecture/fetch.md; src/fetch/render/blocks.rs, especially collect_inline_text and collect_text_parts; existing inline-render tests in src/fetch/render/blocks.rs.

## Objective

Preserve the source HTML’s whitespace boundaries when combining inline text, links, and code in extracted text. Normalization may collapse whitespace runs, but must not erase a source boundary and merge neighboring words. Apply the same fidelity in plain-text and Markdown rendering without inventing spaces where the source contains none.

## Evidence

The live comparison against Python’s tutorial showed missing spaces around inline links in both default text and Markdown modes; Markdown retained link/code markup but did not fix the spacing. One Markdown passage contained a form like The[list](...)data.

The code trims each non-empty text node in collect_text_parts, concatenates the resulting parts with an empty separator in collect_inline_text, then normalizes whitespace. Once boundary spaces are trimmed, normalize_whitespace cannot recover them. Existing tests exercise paragraphs with spaces around inline code/links but assert only that substrings are present, not the complete rendered sentence, so the boundary loss passes unnoticed.

## Scope

In scope: correct boundary whitespace preservation in HTML inline extraction for text and Markdown representations; add exact-output regression tests for inline text, links, and code, including adjacent markup and punctuation; document the extraction fidelity behavior in the owning architecture section.

Out of scope: new HTML/Markdown dependency or changes to markdown-first negotiation, link resolution, block boundaries, or document chunking; altering source text beyond boundary-whitespace fidelity; adding spaces absent in source; changing code-block whitespace; changes to MCP schemas, tool inventory, fetch budgets, focus ranking, browser policy, or sanitization/trust rules.

## Invariants that cannot regress

- Source boundary whitespace is retained through inline collection; whitespace normalization happens after boundaries are represented.
- No synthetic separator is inserted when adjacent source text has no whitespace. Punctuation remains attached as in the source.
- Markdown link syntax, URL resolution, inline-code markup, and raw code-block contents remain unchanged.
- Block-level paragraph/list/table boundaries, depth caps, skipped-node behavior, sanitization, and the four scraper outcomes remain unchanged.
- No new dependency, tool, schema field, default, network requirement, or credential requirement.

## Expected production changes

- src/fetch/render/blocks.rs: retain or reconstruct text-node boundary whitespace before normalization while preserving inline link/code wrappers.
- src/fetch/render/blocks.rs unit tests: assert exact text-mode and Markdown-mode output for whitespace on each side of inline links/code, markup with no source whitespace, punctuation, and repeated whitespace.
- tests/web_fetch_integration.rs if needed to prove corrected rendered text reaches the public fetch response; no new suite unless necessary.
- architecture/fetch.md and a CHANGELOG.md entry for the user-visible extraction correction.

## Compatibility and migration

No wire shape, schema, identifier, or configuration change. Fetched prose may gain spaces that were present in source HTML but previously lost; this can change downstream text matching and passage contents, which is the intended correction. Markdown link/code notation and no-source-whitespace adjacency remain stable. No migration.

## Dependencies

- Hard: none. The implementation is local to the existing HTML rendering layer.
- Interface: existing fetch response and sanitization contracts; no new interface dependency.
- Soft: none.
- M003 is related history only, not reopened; its accepted negotiation/fallback behavior is unchanged.

## Ordered work packages

1. Add failing exact-output unit tests demonstrating current boundary loss in plain text and Markdown. Include a no-whitespace adjacency case to prevent a blanket join-with-space workaround, plus inline code and punctuation cases.
2. Change the collector to preserve source text-node whitespace at the join boundary and normalize only after original boundaries and generated inline wrappers are combined. Keep raw code-block collection separate.
3. Verify nested inline elements, empty/skipped nodes, URL resolution, depth limits, and repeated whitespace. Add an integration assertion only if unit-level rendered blocks do not cover MCP-visible text.
4. Update architecture/fetch.md and CHANGELOG.md. No test-inventory update is needed unless a suite name or count changes.

## Verification

```sh
cargo test --locked --all-features --lib fetch::render::blocks
cargo test --locked --all-features --test web_fetch_integration
cargo test --locked --all-features --test engine_parse_conformance
cargo test --locked --all-features --test static_guards
make check
make docs-check
./packaging/check-repo-hygiene.sh
```

The tests are local and deterministic; no live URL is required. Run cargo fmt as part of make check.

## Acceptance and stop conditions

- Exact-output tests pass for text and Markdown modes, including source spaces around an inline element and a case with no source spaces; no words merge and no unsupported separator is added.
- Existing link/code formatting, block/code-block behavior, parser conformance outcomes, and fetch response contracts remain unchanged.
- Focused tests and broad gates pass with no network or credentials.
- Stop and record a design note if the markup tree cannot preserve source boundaries without changing code-block or block-level semantics; do not paper over it by unconditionally joining parts with spaces.

## Required closure evidence

Create plans/closure/agentic-retrieval-integrity/048-status.md with the exact candidate SHA, before/after exact-output cases for both modes, a requirement-to-test matrix, confirmation of unchanged link/code/block behavior, focused and broad test results, hosted qualification, and residual risks.
