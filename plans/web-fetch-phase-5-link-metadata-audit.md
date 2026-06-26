# web_fetch Phase 5: Link Classification, Metadata Polish, Docs, and Final Audit

## Objective

Close the web_fetch agent-rendering line of work by improving the decision surface around extracted links, enriching fetch/render metadata, updating documentation, and auditing the final MCP/CLI behavior for small-model agent use.

This phase must not add crawling. Link classification is metadata only. Agents may use classified links to decide whether to make a separate explicit `web_fetch` call, but eggsearch must not follow links automatically.

## Dependency on prior phases

This phase assumes the structured document model exists, HTML structural rendering and Markdown mode are implemented, code/Markdown/plaintext detection is implemented, and optional PDF extraction either exists or has been explicitly deferred.

If a prior phase is incomplete, document the gap and avoid hiding it with docs-only polish.

## Non-goals

Do not crawl classified links. Do not recursively fetch assets. Do not rank links with model calls. Do not summarize documents. Do not introduce browser execution. Do not make PDF support mandatory.

## Link classification

Extend `ExtractedLink` additively or add a richer link type while keeping compatibility. Preserve `text` and `url`. Add optional fields for link kind, rel value, and same-domain status.

Recommended link kinds: same-page anchor, same-domain, external, download, source code, documentation, API reference, issue, pull request, release, security advisory, PDF, image, feed, and other.

Classification rules should be deterministic and cheap:

- Same final URL with only fragment change is a same-page anchor.
- Same host is same-domain unless a more specific kind applies.
- `.pdf` paths are PDF links.
- Common image extensions are image links.
- Common source/config extensions are source-code or structured-document links.
- GitHub/GitLab issue paths are issue links.
- GitHub/GitLab pull or merge-request paths are pull-request links.
- Release paths are release links.
- Security advisory paths are security-advisory links.
- Docs hosts or docs paths are documentation or API-reference links when obvious.
- RSS/Atom/feed-looking URLs are feed links.
- Otherwise classify as external, same-domain, or other.

Do not add a public-suffix dependency unless clearly justified. Host equality and simple suffix heuristics are sufficient.

## Link bounding and metadata

Keep the existing link cap, but expose whether links were truncated. Add or verify metadata for links total seen when cheap, links returned, links truncated, bytes read, content length, charset, redirects followed, document kind, detected language, source extension, text chars returned, text truncated, and blocks truncated.

If counting all links would require retaining all links, count during iteration and store only up to the cap.

## MCP schema audit

Audit `WebFetchArgs` for small-model usability. The final surface should remain minimal: required `url`, optional `max_chars`, optional `timeout_ms`, optional `extract_mode`, and optional `include_links`.

If `auto` mode is added, make it the default only if compatibility is not broken. Otherwise, document that omitted mode uses safe automatic detection internally while retaining the public default name. Do not make agents choose content-type-specific options.

Error messages should be actionable. Unsupported content type should name supported types. PDF unsupported should say whether feature or config is missing. Markdown mode must no longer say reserved. Private-network blocking should remain explicit.

## CLI audit

If CLI `eggsearch fetch` exposes modes, update it without making common use harder. Useful flags are `--max-chars`, `--timeout-ms`, `--metadata-only`, `--markdown`, `--links`, and `--json`. Avoid a large specialist flag surface. MCP remains the main codegg integration path.

## Documentation updates

Update README with the final `web_fetch` overview, supported document kinds, extraction modes, examples for HTML docs, Markdown/raw code, optional PDF, untrusted-content warnings, link classification behavior, and truncation fields.

Update AGENTS or contributor guidance to state: do not turn `web_fetch` into a crawler, do not add LLM summarization inside eggsearch, keep active content disabled, keep heavy formats optional, and preserve compatibility fields.

Update CHANGELOG if the repo maintains one.

## Fixture audit

Ensure local, license-safe fixtures exist for HTML docs with headings/lists/code/table, HTML chrome stripping, Markdown README with fenced code, Rust raw source, Python raw source with indentation, TOML config, JSON, unified diff/patch, plain text prose, and optional text-based PDF when PDF support is enabled.

## Regression tests

Add final integration tests for MCP tool surface stability, minimal `web_fetch` call, Markdown extraction mode, metadata-only body suppression through all new fields, link classification, link cap/truncation behavior, distinct byte/text/block/link truncation metadata, external-untrusted trust label, prompt-injection marker detection in fetched text, private-network and redirect blocking, default build without optional PDF dependency, and all-features build.

## Final audit checklist

Before closing the roadmap, confirm that `web_fetch` still fetches exactly one explicit URL, no code path follows extracted links automatically, no JavaScript execution or browser runtime exists, no summarization or model call exists inside eggsearch, all newly exposed text fields are bounded and sanitized, legacy `text` consumers remain functional, README examples match actual output, MCP schema descriptions match accepted enum values, CLI behavior matches MCP behavior where applicable, optional PDF support is clearly optional, and test-count docs are updated if the repo tracks them.

## Acceptance criteria

Agents receive classified links and richer metadata without automatic crawling. The final response shape is documented and tested. Small models can use minimal calls and do not need content-type-specific knowledge. All safety boundaries are preserved. Full test suite passes in default and all-feature configurations.

Run `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all-features` before closing the phase.
