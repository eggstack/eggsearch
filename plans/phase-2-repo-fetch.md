# Phase 2 Plan: Exact `repo_fetch` / Code Fetch Tool

## Objective

Add an explicit MCP tool for fetching repository objects and source-code spans by structured locator instead of by generic browser URL. The tool should serve codegg's most common post-search operation: fetch this exact file or line range with stable provenance, line numbers, truncation metadata, and raw content suitable for model inspection.

This phase builds on Phase 1 exact code evidence metadata. Search results should be able to suggest a structured fetch target, and this tool should execute that target deterministically.

## Current baseline

`web_fetch` already supports bounded single-URL retrieval, raw-source transformations for some code-host browser URLs, structured document output, line-preserving rendering for source/code-like content, link extraction, prompt-injection framing, byte/character caps, and private-network controls. That should remain the generic explicit URL fetch path.

The missing coding-agent interface is a structured repository fetch request. A model should not have to construct `raw.githubusercontent.com` URLs, decide whether to fetch a browser page or raw file, or post-process a whole file just to inspect a 40-line context window.

## Non-goals

Do not remove `web_fetch`. Do not implement recursive repository crawling. Do not clone repositories in this phase. Do not require local workspace indexing. Do not promise commit SHA resolution unless the provider/API actually returns it. Do not execute code, build projects, or run tests.

## Proposed MCP tool

Expose a new stable MCP tool named `repo_fetch` unless there is a strong naming reason to use `code_fetch`. Prefer `repo_fetch` because it may later fetch non-code repository objects such as README, changelog, workflow files, lockfiles, or manifests.

Request shape:

```rust
pub struct RepoFetchRequest {
    pub host: Option<CodeHost>,
    pub owner: String,
    pub repo: String,
    pub ref_name: Option<String>,
    pub commit_sha: Option<String>,
    pub path: String,
    pub line_start: Option<u32>,
    pub line_end: Option<u32>,
    pub context_before: Option<u32>,
    pub context_after: Option<u32>,
    pub max_chars: Option<usize>,
    pub timeout_ms: Option<u64>,
    pub include_full_file_metadata: Option<bool>,
}
```

Response shape:

```rust
pub struct RepoFetchResponse {
    pub locator: RepoLocator,
    pub fetched: bool,
    pub status: Option<u16>,
    pub content_type: Option<String>,
    pub language: Option<String>,
    pub source_role: Option<SourceRole>,
    pub browser_url: String,
    pub raw_url: String,
    pub permalink_url: Option<String>,
    pub content_sha256: Option<String>,
    pub ref_resolved: Option<String>,
    pub line_start: Option<u32>,
    pub line_end: Option<u32>,
    pub returned_line_start: Option<u32>,
    pub returned_line_end: Option<u32>,
    pub total_lines: Option<u32>,
    pub text: Option<String>,
    pub lines: Vec<RepoFetchedLine>,
    pub document: Option<FetchDocument>,
    pub truncated: bool,
    pub warnings: Vec<String>,
    pub trust: FetchTrust,
    pub trust_markers: TrustMarkers,
}
```

`RepoFetchedLine` should include at least `{ number, text }`. Preserve exact line numbers even when only a subrange is returned.

Use `ref_name` for branch/tag names and `commit_sha` for stable refs. If both are supplied, prefer `commit_sha` for permalink construction but keep `ref_name` in the locator for operator context. Validate that path is relative, not absolute, and cannot escape a repository root.

## Host support

Start with GitHub because raw URL derivation is straightforward and native search results already emphasize GitHub. Support GitLab if existing transform logic is clean enough. Codeberg/Gitea/Forgejo can be acknowledged as metadata-parsed but not fetch-native yet unless their raw URL patterns are already deterministic and tested.

The tool should return a clear validation or unsupported-host error when a host cannot be fetched structurally. Avoid silently falling back to generic web fetch in a way that changes response semantics. A host-level fallback may be acceptable only if the response includes a warning like `repo_fetch_host_fallback_to_web_fetch`.

## Implementation strategy

Create a `src/core/repo_fetch.rs` module for request/response structs and locator validation. Reuse `CodeHost`, `CodeEvidence`, `SourceRole`, and language inference from Phase 1.

Create a `src/fetch/repo_fetch.rs` or equivalent implementation module that builds raw URLs and delegates the actual HTTP operation to the same hardened client path used by `web_fetch` where practical. Reuse timeout, max-bytes, max-chars, redirect limit, private-network checks, user agent, sanitization, and document rendering. Avoid introducing a separate HTTP stack.

Add `run_repo_fetch` in `src/mcp/tools.rs` and register the MCP tool in the server wiring. Include it in `provider_status` or a new tool-capability status section if the repo has a standard place for tool capabilities.

## Line range behavior

Line range handling must be deterministic:

- If no line range is supplied, return from the top of the file up to `max_chars`, with `truncated` set appropriately.
- If `line_start` is supplied without `line_end`, return that line plus context.
- If both are supplied, validate `line_start <= line_end`.
- Apply `context_before` and `context_after` after validating the requested range.
- Clamp returned range to file boundaries.
- Preserve `requested_line_start`, `requested_line_end`, `returned_line_start`, and `returned_line_end` in the response.
- If the line range cannot be honored because the file is truncated before the target, return a clear warning.

Prefer line slicing after full bounded body retrieval. Do not fetch arbitrary huge files. If a provider supports range requests later, that can be added in a later optimization pass.

## Security and trust behavior

Keep external fetched repository content as external untrusted unless it comes from an explicitly configured local workspace backend in a later phase. Repository source code can include prompt-injection-like comments, so sanitization and marker scanning should remain active.

Reject private-network and localhost raw URLs unless the existing fetch configuration explicitly allows them. This matters for self-hosted GitLab/Gitea later.

Validation should reject:

- Empty owner/repo/path.
- Path traversal such as `../`.
- Absolute paths.
- Non-HTTP(S) generated URLs.
- Excessive context values.
- Zero or inverted line ranges.
- `max_chars` above the fetch cap.

## Suggested fetch integration

Update `RepoSuggestedFetch` so Phase 1 `CodeEvidence` can produce a structured fetch suggestion in addition to a URL. Suggested fetches may include:

```rust
pub structured_repo_fetch: Option<RepoFetchRequest>
```

If the current schema should remain compact, add a new `fetch_kind` and `locator` object instead. Keep URL-only suggested fetches for backward compatibility.

When `repo_search` returns a source-file result with code evidence, it should suggest `repo_fetch` with path/ref/line range where available and `recommended_extract_mode` only as legacy URL-fetch guidance.

## Tests

Add pure validation tests for `RepoFetchRequest`:

- Valid GitHub source file locator.
- Empty fields rejected.
- Path traversal rejected.
- Inverted line range rejected.
- Excessive max/context rejected or clamped according to existing config style.

Add URL construction tests:

- GitHub locator -> raw URL.
- GitHub locator -> browser URL.
- GitHub locator with commit SHA -> permalink URL.
- GitHub branch ref remains browser/raw fetchable but permalink is omitted or marked branch-based if no commit is known.

Add mocked HTTP fetch tests using `httpmock`:

- Full small file returns lines and document.
- Requested line range returns only selected context with correct line numbers.
- Character cap truncates output and reports truncation.
- Prompt-injection markers inside source comments are detected and warned.
- 404 and 429 are reported as structured failures/warnings consistent with `web_fetch` behavior.

Add MCP tool tests:

- Tool schema accepts minimal request.
- Tool returns validation errors for bad request.
- Tool response serializes expected fields.

## Documentation

Update README with a `repo_fetch` section after `web_fetch` or after `repo_search`. Explain when to use `repo_fetch` versus `web_fetch`:

- Use `repo_search` to discover source evidence.
- Use `repo_fetch` to fetch a known repository file/span.
- Use `web_fetch` for arbitrary URLs and non-repository pages.

Add a compact JSON example showing a source result from `repo_search` followed by a `repo_fetch` call.

## Acceptance criteria

- `repo_fetch` is exposed as an MCP tool and covered by tests.
- GitHub repository file fetch works through structured locator fields.
- Returned source text includes stable line numbers and range metadata.
- The tool reuses existing fetch safety limits and trust/sanitization behavior.
- `repo_search` can emit structured suggested fetches when code evidence is present.
- Existing `web_fetch` behavior is unchanged.

## Suggested implementation order

1. Add `RepoFetchRequest`, `RepoFetchResponse`, locator, and validation types.
2. Add GitHub URL construction helpers and tests.
3. Implement fetch execution using existing fetch client infrastructure.
4. Add line slicing and line-number rendering.
5. Register MCP tool and tests.
6. Integrate structured suggested fetches from repo search.
7. Update README.
