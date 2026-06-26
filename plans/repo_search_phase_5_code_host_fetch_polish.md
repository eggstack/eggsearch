# Repo Search Phase 5: Code-Host Fetch Polish

## Context

Previous repo-search phases make `web_search` repo-aware while keeping the model-facing tool surface simple:

- Phase 1 added typed repo/code metadata and URL parsing.
- Phase 2 added repo-query hints and search planning.
- Phase 3 adds optional `github_code` under `web_search(intent = "code")`.
- Phase 4 adds optional `github_issues` and `github_releases` under `web_search(intent = "issues" | "releases")`.

Phase 5 improves `web_fetch` for explicit code-host URLs. This is not a crawler or repo browser. It remains one URL in, one bounded document out.

The primary workflow is:

1. Codegg research agent calls `web_search(intent = "code")`.
2. Agent chooses one source-file result URL.
3. Agent calls `web_fetch({ "url": "https://github.com/owner/repo/blob/ref/path" })`.
4. `eggsearch` returns bounded text for that one explicit source file.

## Goals

1. Improve `web_fetch` handling for explicit GitHub/GitLab/Codeberg source-file URLs.
2. Preserve the one-explicit-URL fetch boundary.
3. Do not clone repositories, list directories, crawl links, or fetch multiple files.
4. Convert supported code-host browser source-file URLs to raw content URLs internally after URL safety validation.
5. Return bounded source text with deterministic metadata where possible.
6. Reject or bound binary/large files safely.
7. Preserve SSRF, redirect, timeout, size, and untrusted-content protections.

## Non-goals

Do not add a `repo_fetch` or `github_fetch` tool.

Do not allow directory fetching.

Do not recursively fetch imports, dependencies, linked files, adjacent files, or README/changelog references.

Do not clone repositories.

Do not use GitHub/GitLab APIs to inspect repository trees in this phase.

Do not execute code.

Do not render notebooks or run build systems.

Do not bypass `web_fetch` content limits.

Do not treat fetched source text as trusted instructions.

## Current fetch contract to preserve

`web_fetch`:

- accepts one explicit HTTP(S) URL;
- rejects `file://`;
- blocks localhost/private-network URLs by default;
- validates redirects;
- does not execute JavaScript;
- does not crawl linked pages;
- returns bounded extracted content;
- labels returned text as untrusted.

All code-host fetch polish must preserve these properties.

## Supported URL shapes

### GitHub

Support browser source-file URLs:

```text
https://github.com/owner/repo/blob/main/src/lib.rs
https://github.com/owner/repo/blob/v1.2.3/src/lib.rs
https://github.com/owner/repo/blob/<sha>/src/lib.rs#L10-L25
```

Internal raw URL candidate:

```text
https://raw.githubusercontent.com/owner/repo/<ref>/src/lib.rs
```

Keep the response's user-visible/canonical URL as the original browser URL where possible, but record final/raw URL in fetch metadata if the existing response type supports it.

### GitLab

Support browser source-file URLs:

```text
https://gitlab.com/group/project/-/blob/main/src/lib.rs
https://gitlab.com/group/subgroup/project/-/blob/main/src/lib.rs
```

Potential raw URL shape:

```text
https://gitlab.com/group/project/-/raw/main/src/lib.rs
```

For nested groups, preserve namespace path correctly.

### Codeberg / Forgejo-like

Support common Codeberg source-file URLs:

```text
https://codeberg.org/owner/repo/src/branch/main/src/lib.rs
https://codeberg.org/owner/repo/src/tag/v1.2.3/src/lib.rs
```

Potential raw URL shape may be:

```text
https://codeberg.org/owner/repo/raw/branch/main/src/lib.rs
https://codeberg.org/owner/repo/raw/tag/v1.2.3/src/lib.rs
```

Verify actual Codeberg raw URL behavior during implementation. If uncertain, do not rewrite; fetch the browser page and extract text as current behavior does.

## URL rewrite design

Add a deterministic helper, likely in `src/core/code_metadata.rs` or a new `src/core/code_fetch.rs`:

```rust
pub struct CodeHostFetchTarget {
    pub original_url: String,
    pub raw_url: Option<String>,
    pub source_kind: SourceKind,
    pub code: Option<CodeMetadata>,
}

pub fn resolve_code_host_fetch_target(url: &str) -> Option<CodeHostFetchTarget>
```

Rules:

- Return `None` for non-code-host URLs.
- Return `Some` only for source-file URLs, not repo roots, directories, issues, PRs, releases, tags, or commits.
- Use existing URL parsing to classify source kind and metadata.
- Only produce `raw_url` when the raw URL transformation is well understood.
- Preserve line anchors in metadata but do not rely on anchors for HTTP retrieval.

## Safety validation order

The implementation must not use raw URL rewriting to bypass SSRF restrictions.

Recommended order:

1. Parse and validate the original user-provided URL as HTTP(S).
2. Apply existing localhost/private-network/file-scheme checks to original URL.
3. If it is a recognized source-file URL, compute a raw URL candidate.
4. Parse and validate the raw URL as HTTP(S).
5. Apply the same localhost/private-network restrictions to the raw URL host.
6. Fetch the raw URL with existing timeout/size/redirect controls.
7. Validate every redirect target using existing redirect policy.
8. Return bounded text.

Do not skip validation because the raw host is known.

## Content handling

### Text vs binary

For raw source files:

- Accept text-like content types:
  - `text/*`
  - `application/json`
  - `application/xml`
  - `application/x-toml` if seen
  - `application/yaml` / `application/x-yaml` if seen
  - `application/octet-stream` only if bytes pass UTF-8/text sniffing and size limits
- Reject likely binary content when UTF-8/text sniffing fails.
- Preserve existing max bytes and max chars limits.

### Line anchors

If the original URL includes line anchors like `#L10-L25`, do not fetch only that range in Phase 5 unless trivial and safe. Prefer full bounded file text. The line range can be surfaced as metadata.

Optional later enhancement: if line anchors are present, include an additional `selected_range` excerpt. Do not add that in this phase unless existing response shape supports it cleanly.

### Syntax/language metadata

Use existing `CodeMetadata.language` inference from path extension.

If the fetch response has a metadata field, include:

```json
{
  "source_kind": "source_file",
  "code": {
    "host": "github",
    "owner": "tokio-rs",
    "repo": "axum",
    "path": "axum/src/routing/mod.rs",
    "ref_name": "main",
    "language": "rust",
    "line_start": 10,
    "line_end": 25
  }
}
```

If the existing `WebFetchResponse` does not support this metadata, add an optional metadata field in a backward-compatible way.

## WebFetchResponse extension

Inspect the current fetch response model before implementing. If it only returns title/body/metadata for pages, add optional code metadata carefully:

```rust
pub struct WebFetchResponse {
    ...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_kind: Option<SourceKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<CodeMetadata>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fetch_transform: Option<FetchTransform>,
}
```

Suggested transform metadata:

```rust
pub struct FetchTransform {
    pub kind: FetchTransformKind,
    pub original_url: String,
    pub transformed_url: String,
}

pub enum FetchTransformKind {
    CodeHostRawFile,
}
```

Keep this optional. Non-code fetch responses should be unchanged.

## GitHub raw fetch behavior

Implementation steps:

1. Detect GitHub `blob` URL using existing `CodeMetadata` parser.
2. Extract owner, repo, ref, path.
3. Build raw URL:
   ```text
   https://raw.githubusercontent.com/{owner}/{repo}/{ref}/{path}
   ```
4. Validate raw URL.
5. Fetch raw URL through existing HTTP fetch machinery.
6. Return content as text/plain/source text with metadata.

Important edge cases:

- `ref` may contain slashes in branch names. Current URL parser treats the first segment after `blob` as ref and the rest as path, which cannot disambiguate branch names with slashes. This is acceptable for Phase 5, but document it. A later API-backed resolver could handle branch-name ambiguity.
- URL-encoded path segments should remain encoded or be decoded/re-encoded safely. Avoid double-decoding path traversal patterns.
- Do not allow `..` path traversal to influence host/path outside the URL builder. Since raw URL is remote HTTP path, traversal is not local FS risk, but still avoid normalization surprises.

## GitLab raw fetch behavior

Implementation steps:

1. Detect GitLab `/-/blob/<ref>/<path>` URL.
2. Preserve nested namespace before `/-/`.
3. Build raw URL:
   ```text
   https://gitlab.com/{namespace}/-/raw/{ref}/{path}
   ```
4. Validate and fetch as above.

Branch names with slashes have the same ambiguity as GitHub. Document and accept for Phase 5 unless a reliable disambiguation exists.

## Codeberg raw fetch behavior

Implementation should verify Codeberg raw URL shape. If reliable:

```text
https://codeberg.org/{owner}/{repo}/raw/branch/{ref}/{path}
https://codeberg.org/{owner}/{repo}/raw/tag/{ref}/{path}
```

If not reliable, do not rewrite Codeberg in this phase. It is better to leave Codeberg as normal HTML fetch than to produce broken raw URLs.

Acceptance can be staged:

- GitHub raw fetch required.
- GitLab raw fetch preferred.
- Codeberg raw fetch optional unless verified.

## Tests

### URL resolution tests

- GitHub blob URL resolves to raw URL.
- GitHub blob URL with line anchor preserves line metadata.
- GitHub repo root does not resolve to raw file target.
- GitHub tree URL does not resolve to raw file target.
- GitHub issue/PR/release URL does not resolve to raw file target.
- GitLab nested namespace blob resolves to raw URL.
- Codeberg source-file URL resolves only if implementation supports verified raw shape.
- Unknown URL returns `None`.

### Safety tests

- Raw URL candidate must be HTTP(S).
- Original localhost/private URL is rejected before rewrite.
- Raw localhost/private URL would be rejected if ever produced.
- Redirect from raw URL to private network is rejected by existing redirect policy.
- Binary content is rejected or bounded according to existing fetch behavior.
- Oversized source file is bounded by `max_bytes` and `max_chars`.

### Fetch tests with mock HTTP server

- GitHub blob URL fetch calls raw endpoint and returns raw source text.
- Response includes code metadata if response model supports it.
- Non-code URL uses existing fetch behavior.
- GitHub raw endpoint 404 returns clear fetch error.
- Raw endpoint timeout returns timeout error.
- Raw endpoint binary bytes are rejected or safely bounded.

### Regression tests

- `web_fetch` still rejects `file://`.
- `web_fetch` still rejects localhost/private-network URLs by default.
- `web_fetch` does not follow links.
- `web_fetch` does not execute JavaScript.
- `web_fetch` still enforces `max_chars` cap.

## Documentation

Update README:

- Explain that `web_fetch` can fetch one explicit code-host source-file URL.
- Show example:
  ```json
  { "url": "https://github.com/tokio-rs/axum/blob/main/axum/src/routing/mod.rs" }
  ```
- State that source-file browser URLs may be fetched as raw text internally.
- State that this does not clone repos, list directories, or fetch linked files.
- State that source code is untrusted data.

Update AGENTS.md:

- After `web_search(intent = "code")`, fetch only one selected URL at a time.
- Do not use `web_fetch` to crawl adjacent files or directories.
- Use Codegg local tools for local workspace search; use `eggsearch` for public remote source evidence.

## Validation commands

Run:

```bash
cargo fmt
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```

If all-feature clippy is unsupported, record the exact successful command and reason.

## Final acceptance checklist

- [ ] GitHub blob source-file URLs can be fetched as raw bounded text.
- [ ] GitLab blob source-file URLs can be fetched as raw bounded text, or are explicitly deferred with tests/documentation.
- [ ] Codeberg raw fetch is implemented only if verified; otherwise safely deferred.
- [ ] Repo roots, directories, issues, PRs, releases, tags, and commits are not rewritten as raw files.
- [ ] Original and transformed URLs pass the same safety validation policy.
- [ ] Redirect validation still blocks private/localhost targets.
- [ ] Binary/oversized files are rejected or bounded.
- [ ] Optional code metadata is included in fetch responses if response shape supports it.
- [ ] `web_fetch` remains one explicit URL per call.
- [ ] No cloning, crawling, directory listing, or linked-file fetching is added.
- [ ] README and AGENTS clearly document the boundary.
