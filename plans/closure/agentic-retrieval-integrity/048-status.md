# M048 Inline HTML Whitespace Fidelity — Closure Evidence

Status: closed

Source plan: plans/implementation/agentic-retrieval-integrity/048-inline-html-whitespace-fidelity.md

Roadmap: plans/subsystems/agentic-retrieval-integrity-roadmap.md M048

Decision authority: existing HTML extraction and sanitization contracts. M003 remains closed; no markdown-first negotiation or fetch contract is reopened.

## Exact code candidate

Implementation candidate 9528d4131591392c0d9f6ec5f0422903e4b7815b on docs/user-facing-polish-20261123, based on c75bfec9c3cc13a53b3b2c061a326d99b1cc2c38. Candidate tree: ee8e56e747c92da196f419d763d14439a4ce97b1. Local commands ran against the content-identical pre-commit tree; the implementation commit is clean. This closure commit will carry the closure records and synchronized plan status only; it changes no source.

Hosted CI run 45127251813 succeeded on this exact SHA via workflow dispatch: Linux `ci`, Windows portability, and macOS canonical/filesystem portability all passed. The Node.js 20 deprecation annotation was non-blocking. Run: https://github.com/eggstack/eggsearch/actions/runs/45127251813.

## Defect and change

The previous inline collector trimmed every non-empty text node, concatenated the remaining parts without a separator, then normalized whitespace. A source boundary around inline markup was lost before normalization, merging words in plain-text and Markdown output. The collector now preserves source-present whitespace through inline text/link/code composition and normalizes only after the boundaries are represented. It does not add spaces where the source contains none.

## Exact-output evidence

src/fetch/render/blocks.rs::inline_whitespace_preserves_source_boundaries_exactly asserts both rendering modes for every case below:

| Source case | Text output | Markdown output |
|---|---|---|
| Multiple spaces before a nested inline link; a space and newline after it; inline code later in the sentence | Before linked text then sample after. | Before [linked text](https://example.com/docs) then &#96;sample&#96; after. |
| Source-adjacent link and code followed by punctuation | Seethisx,next! | See[this](https://example.com/docs)&#96;x&#96;,next! |
| Space inside the link wrapper, no space outside before/after | Before linked after | Before [linked](https://example.com/docs) after |
| Space inside the code wrapper, no space outside before/after | Before sample after | Before &#96;sample&#96; after |

The first case proves repeated source spaces normalize while boundaries survive. The second proves no synthetic spaces are added at source-adjacent link/code or punctuation boundaries. The last two prove spaces inside inline wrappers survive after trimming wrapper content. Existing link URL resolution and inline-code formatting tests remain unchanged. render_blocks_code_whitespace_preserved confirms raw code-block whitespace is unchanged.

## Requirement-to-evidence mapping

| Requirement | Evidence |
|---|---|
| Preserve source-present boundaries in text and Markdown output | Exact-output test above covers nested inline emphasis/link, whitespace around links/code, repeated whitespace, and both modes. |
| Never invent spaces for source-adjacent words or punctuation | The source-adjacent link/code case asserts Seethisx,next! and See[this](...)&#96;x&#96;,next! exactly. |
| Keep link/code markup and raw code blocks stable | Existing render_blocks_inline_link_markdown, render_blocks_inline_code_markdown, render_blocks_inline_code_text_mode_no_backticks, URL-resolution tests, and render_blocks_code_whitespace_preserved pass. |
| Preserve public fetch/parser behavior and boundaries | All-features web_fetch_integration and engine_parse_conformance passed in both local/full-gate and independent reviewer runs. No scraper outcome, sanitization, block boundary, tool/schema, configuration, or dependency changed. |

## Compatibility and residual risk

No wire shape, schema, tool inventory, default, stable identifier, dependency, cache behavior, or migration changed. Extracted prose now retains spaces that existed in source HTML and were previously lost; downstream text matching and passage content may therefore differ as intended. Normalization still collapses repeated whitespace, and no-source-space adjacency remains unchanged. Block-level behavior, skipped nodes, depth caps, sanitization, and raw code-block contents are unchanged.

No unresolved implementation finding blocks closure. No live network or credentialed probe was needed. Final recommendation: closed.

## Verification

- RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --lib fetch::render::blocks: 34 passed.
- RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --test batch_fetch_retrieval: 29 passed (M047 shared candidate suite).
- RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --test mcp_projection: 36 passed.
- RUSTUP_TOOLCHAIN=1.89.0 make check: passed, including formatting, Clippy, no-default-features check, all-features tests, hygiene, dependency policy, and packaging checks.
- RUSTUP_TOOLCHAIN=1.89.0 make docs-check: passed.
- ./packaging/check-repo-hygiene.sh: passed (repo-hygiene: ok).
- Independent read-only review: PASS; no concrete defects found. Web-fetch integration (128 passed, 1 live test ignored), parser conformance (6), static guards (68), and docs-check passed.
- Hosted CI: run 45127251813 completed successfully on the exact candidate SHA above; Linux CI, Windows portability, and macOS canonical/filesystem checks all passed.

## Verification commands

RUSTUP_TOOLCHAIN=1.89.0 make check
RUSTUP_TOOLCHAIN=1.89.0 make docs-check
./packaging/check-repo-hygiene.sh
