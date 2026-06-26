# web_fetch Phase 5: Link Classification, Metadata Polish, Docs, and Final Audit

## Objective

Close the web_fetch agent-rendering line of work by improving the decision surface around extracted links, enriching fetch/render metadata, updating documentation, and auditing the final MCP/CLI behavior for small-model agent use.

This phase must not add crawling. Link classification is metadata only. Agents may use classified links to decide whether to make a separate explicit `web_fetch` call, but eggsearch must not follow links automatically.

## Dependency on prior phases

This phase assumes:

- Structured document model is present.
- HTML structural rendering and Markdown mode are implemented.
- Code/Markdown/plaintext detection is implemented.
- Optional PDF extraction either exists or has been explicitly deferred.

If a prior phase is incomplete, this phase should first document the gap and avoid hiding it with docs-only polish.

## Non-goals

- Do not crawl classified links.
- Do not recursively fetch assets.
- Do not rank links with model calls.
- Do not summarize documents.
- Do not introduce browser execution.
- Do not make PDF support mandatory.

## Link classification

Extend `ExtractedLink` or add a new richer link type while keeping compatibility. If changing `ExtractedLink` is additive, prefer adding fields:

```rust
pub struct ExtractedLink {
    pub text: String,
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<LinkKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rel: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub same_domain: Option<bool>,
}
```

Recommended enum:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LinkKind {
    SamePageAnchor,
    SameDomain,
    External,
    Download,
    SourceCode,
    Documentation,
    ApiReference,
    Issue,
    PullRequest,
    Release,
    SecurityAdvisory,
    Pdf,
    Image,
    Feed,
    Other,
}
```

Classification rules should be deterministic and cheap:

- `#fragment` or same final URL with only fragment change -> `same_page_anchor`.
- Same registrable/domain host -> `same_domain` unless a more specific kind applies.
- `.pdf` or `application/pdf` hints in URL -> `pdf`.
- Image extensions -> `image`.
- `.rs`, `.py`, `.js`, `.ts`, `.go`, `.toml`, `.yaml`, `.json`, `.diff`, `.patch`, etc. -> `source_code` or relevant structured kind.
- GitHub/GitLab paths containing `/issues/` -> `issue`.
- GitHub/GitLab paths containing `/pull/`, `/pulls/`, or `/-/merge_requests/` -> `pull_request`.
- Paths containing `/releases` -> `release`.
- Paths containing `/security/advisories` or advisory-looking paths -> `security_advisory`.
- Docs hosts or docs paths -> `documentation` / `api_reference` if obvious.
- `rss`, `atom`, `.xml` feed-looking URLs -> `feed`.
- Otherwise external/same-domain/other.

Do not use public suffix list dependency unless already present or clearly justified. Host equality and suffix-ish heuristics are sufficient for this phase.

## Link bounding and metadata

Current extractor caps links at a constant. Preserve a cap but expose whether links were truncated.

Add or ensure these fields exist in document/fetch metadata:

- `links_total_seen` if cheap to count.
- `links_returned`.
- `links_truncated`.
- `bytes_read`.
- `content_length`.
- `charset`.
- `redirects_followed`.
- `document_kind` or `document.kind`.
- `detected_language` for code-like resources.
- `source_extension`.
- `text_chars_returned`.
- `text_truncated`.
- `blocks_truncated`.

If total link counting requires retaining all links, avoid memory growth. It is acceptable to count while streaming DOM iteration and only store up to cap.

## MCP schema audit

Audit `WebFetchArgs` for small-model usability.

Recommended final shape:

- `url`: required.
- `max_chars`: optional.
- `timeout_ms`: optional.
- `extract_mode`: optional, with accepted values documented clearly.
- `include_links`: optional.

If `auto` mode is added, make it the default only if compatibility is not broken. Otherwise, document that omitted mode uses safe automatic detection internally while retaining the public default name. Do not make agents choose content-type-specific options.

Schema/docs should avoid requiring agents to pass debug fields. The minimal call must remain:

```json
{"url":"https://example.com/page"}
```

Error messages should be actionable:

- Unsupported content type should say which types are supported.
- PDF unsupported should say whether feature/config is missing.
- Markdown mode should no longer say reserved.
- Private-network blocking should remain explicit.

## CLI audit

If CLI `eggsearch fetch` exists, update it to expose new modes without making common use harder.

Recommended flags:

- `--max-chars N`
- `--timeout-ms N`
- `--metadata-only`
- `--markdown`
- `--links`
- `--json`

Do not add many specialist flags unless they directly map to config. Keep CLI useful for debugging but MCP remains the main integration surface for codegg.

## Documentation updates

Update README with:

- Updated `web_fetch` overview.
- Supported document kinds table.
- `extract_mode` accepted values and behavior.
- Examples for HTML docs, Markdown/raw code, and optional PDF.
- Warning that all fetched content is untrusted data.
- Explanation that links are classified but not followed.
- Explanation of truncation fields.

Update AGENTS.md or equivalent contributor guidance with:

- Do not turn `web_fetch` into a crawler.
- Do not add LLM summarization inside eggsearch.
- Keep active content disabled.
- Keep heavy formats optional.
- Preserve compatibility fields.

Update changelog if the repo maintains one.

## Fixture audit

Add or verify fixtures for:

- HTML docs page with headings, lists, code, and table.
- HTML page with chrome/script/style/nav/footer to strip.
- Markdown README with headings and fenced code.
- Rust raw source file.
- Python raw source file with indentation.
- TOML config.
- JSON document.
- Unified diff/patch.
- Plain text prose.
- Optional text-based PDF when PDF feature is enabled.

Fixtures should be small, local, and license-safe.

## Regression tests

Add final integration tests for:

- MCP tool surface remains stable.
- Minimal `web_fetch` call works.
- `extract_mode = "markdown"` works.
- `metadata_only` does not leak content through any new fields.
- Links are classified and bounded.
- `links_truncated` is set when cap is exceeded.
- Truncation metadata distinguishes byte cap, text cap, block cap, and link cap.
- All content remains `external_untrusted`.
- Prompt-injection marker scanning still catches hostile fetched text.
- Private-network and redirect-blocking tests still pass.
- Default build passes without optional PDF dependency.
- All-features build passes.

## Final audit checklist

Before closing this roadmap, confirm:

- `web_fetch` still fetches exactly one explicit URL.
- No code path follows extracted links automatically.
- No JavaScript execution or browser runtime exists.
- No summarization or model call exists inside eggsearch.
- All newly exposed text fields are bounded and sanitized.
- Legacy `text` consumers remain functional.
- README examples match actual output shape.
- MCP schema descriptions match actual accepted enum values.
- CLI behavior matches MCP behavior where applicable.
- Optional PDF support is clearly optional and disabled by default unless explicitly decided otherwise.
- Test count/docs are updated if the repo tracks test counts.

## Acceptance criteria

- Agents receive classified links and richer metadata without automatic crawling.
- The final response shape is documented and tested.
- Small models can use minimal calls and do not need content-type-specific knowledge.
- All safety boundaries are preserved.
- Full test suite passes in default and all-feature configurations.

Run:

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
```
