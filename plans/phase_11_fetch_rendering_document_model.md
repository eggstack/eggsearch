# Phase 11: Fetch Rendering and Document Model Polish

## Objective

Tighten `web_fetch`, `repo_fetch`, `batch_fetch`, and the internal document model so fetched content is consistently structured, bounded, citation-ready, and safe for coding agents. This phase should also absorb the remaining polish items from the phase 6–10 pass that are most directly related to fetching: local path/symlink enforcement verification, binary/unsupported-content behavior, code-span completeness, and docs consistency around fetch metadata.

The goal is not to add a browser or crawler. The goal is to make explicit fetches produce high-quality, predictable evidence objects.

## Current context

Eggsearch now has:

- bounded HTTP fetch with sanitization and trust markers;
- code-host URL handling;
- PDF feature-gated support;
- repo fetch by structured locator;
- local workspace fetch support;
- `FetchDocument` / document-like extraction;
- code context and code-span metadata;
- stable source/fetch/span IDs;
- batch fetch and evidence bundle handoff.

The remaining issue is consistency. Agents need every fetched object to answer the same questions: what was fetched, how much was returned, what structure was detected, what was omitted, what IDs link it to prior results, and whether it is safe to treat only as evidence.

## Non-goals

- Do not execute JavaScript.
- Do not crawl links automatically.
- Do not perform OCR.
- Do not render arbitrary pages with a browser engine.
- Do not summarize fetched content with an LLM.
- Do not follow local symlinks outside configured roots.
- Do not return binary content bodies.

## Workstream 1: Unified fetched-document shape

### Problem

Different fetch paths can expose similar metadata with different field names or nesting. Agents should not need separate logic for every fetch source.

### Required behavior

Define or normalize a common fetched-document projection used by web, repo, local, and batch fetch outputs. It can be embedded inside existing response types; do not break compatibility.

Recommended conceptual shape:

```rust
pub struct FetchedDocumentView {
    pub fetch_id: Option<String>,
    pub source_id: Option<String>,
    pub url: Option<String>,
    pub locator: Option<RepoLocator>,
    pub title: Option<String>,
    pub content_type: Option<String>,
    pub detected_kind: FetchedDocumentKind,
    pub language: Option<String>,
    pub text: Option<String>,
    pub text_char_count: usize,
    pub original_char_estimate: Option<usize>,
    pub truncated: bool,
    pub truncation_reason: Option<String>,
    pub sections: Vec<DocumentSection>,
    pub links: Vec<DocumentLink>,
    pub code_span: Option<CodeSpanEvidence>,
    pub trust: FetchTrust,
    pub trust_markers: TrustMarkers,
    pub warnings: Vec<AgentWarning>,
}
```

Do not duplicate content if existing fields already carry text. A lightweight view or helper that produces the normalized projection for tests may be enough.

### Tests

- `web_fetch` HTML response exposes consistent title/content type/truncation/trust metadata.
- `web_fetch` markdown/text response exposes detected kind and text metadata.
- `repo_fetch` source response exposes language, source role, code context, code span, and trust metadata.
- `batch_fetch` preserves per-item fetch IDs and structured warnings.
- Evidence bundle can ingest both web and repo fetches without losing document-kind metadata.

## Workstream 2: Local fetch hardening completion

### Problem

The previous polish pass added binary extension and skip-dir helpers, but the fetch path itself still needs explicit review for traversal, canonicalization, symlink escape, and binary/invalid UTF-8 enforcement.

### Required behavior

Local fetch must be path-safe and bounded:

- Canonicalize configured roots at startup or first use.
- Canonicalize candidate target paths before reading.
- Reject any target that is not under an allowed canonical root.
- Reject `..` traversal, absolute path escape, and encoded traversal if applicable.
- Default `follow_symlinks = false` must prevent symlink traversal.
- If symlink following is enabled later, final resolved path must still be under allowed root unless an explicit unsafe option exists. Do not add that unsafe option in this phase.
- Reject known binary extensions before reading large bodies.
- Detect invalid UTF-8 or binary-like bytes if extension does not catch it.
- Emit structured warnings for skipped binary content, local path rejection, symlink rejection, and truncation.

### Implementation guidance

Add a central helper with a narrow contract:

```rust
pub fn validate_local_fetch_path(root: &Path, requested_relative_path: &str, cfg: &LocalConfig) -> Result<PathBuf, LocalFetchPathError>
```

`LocalFetchPathError` should be structured enough to map to warnings and tests.

### Tests

- `../secret.txt` is rejected.
- absolute path input is rejected for workspace-relative fetch.
- `a/../../secret.txt` is rejected.
- symlink inside root pointing outside root is rejected.
- symlink inside root pointing inside root is accepted only if policy allows it.
- binary extension is rejected before body read.
- invalid UTF-8 is rejected or converted into an explicit unsupported-content result.
- large local file truncates deterministically with structured warning.
- normal local source file still fetches successfully.

## Workstream 3: Code-span completeness polish

### Problem

`CodeSpanEvidence` now exists, but it is compact. Agents benefit when a span carries enough direct linkage to avoid reconstructing provenance from sibling fields.

### Required behavior

Extend code-span metadata additively to include the most important linking fields:

- `source_id: Option<String>`
- `fetch_id: Option<String>`
- `locator_id: Option<String>` where available
- `path: Option<String>`
- `source_role: Option<SourceRole>`
- `imports: Vec<String>` or a bounded import count/summary if full imports are too noisy
- `trust: Option<FetchTrust>`
- `permalink_url: Option<String>`
- `raw_permalink_url: Option<String>`

If adding all fields would duplicate too much, add `links`/`provenance` subobject with IDs and URLs only.

### Tests

- Remote repo span includes path and permalink when known.
- Local workspace span includes workspace locator and local trust.
- Span ID remains stable when non-identity metadata changes.
- Span ID changes when locator/path/line range/symbol changes.
- Evidence bundle preserves span linkage fields.

## Workstream 4: HTML/Markdown structure consistency

### Required behavior

Improve document extraction consistency for common web docs:

- headings with hierarchy path;
- paragraphs/lists/code blocks/tables where cheaply extractable;
- link text + URL with cap and truncation marker;
- code fence language for markdown;
- page title and canonical URL where available;
- prompt-injection marker scan scope recorded in trust markers.

### Tests

- HTML headings produce section hierarchy.
- Markdown code fences preserve language labels.
- Link cap emits structured warning.
- Prompt-injection marker in page body appears in structured warnings with source/fetch ID when available.

## Workstream 5: PDF and non-HTML behavior clarity

### Required behavior

PDF support should be explicit and predictable:

- If PDF feature disabled, return a clear structured warning/error state.
- If PDF extracted, record page count when available and extraction limits.
- If PDF extraction fails, return a typed failure, not a generic provider failure.
- Non-text content should produce unsupported content metadata rather than garbage text.

### Tests

- PDF-disabled build returns expected warning/error shape.
- Unsupported binary content returns unsupported kind.
- Plain text files still parse as text.

## Workstream 6: Batch fetch consistency

### Required behavior

Batch fetch should preserve per-item isolation:

- per-item stable fetch ID;
- per-item source ID if supplied;
- per-item warnings and structured warnings;
- per-item truncation reason;
- global budget exhaustion warning;
- deterministic ordering independent of completion order.

### Tests

- Failed item does not poison successful item.
- Per-item structured warnings remain attached to correct item.
- Global budget exhaustion is reported once.
- Ordering is stable.

## Workstream 7: Docs and schema examples

Update:

- README fetch sections;
- `docs/tool-matrix.md`;
- `docs/agent-workflows.md`;
- `AGENTS.md`;
- schema example tests.

Add explicit docs for:

- local symlink/path policy;
- unsupported/binary content behavior;
- code-span provenance fields;
- document section/link caps;
- PDF feature behavior.

## Acceptance criteria

- Fetch outputs are consistent across web, repo, local, and batch paths.
- Local fetch path traversal/symlink/binary/large-file behavior is tested in the real fetch path.
- Code-span metadata carries direct provenance links.
- HTML/Markdown/PDF/text behavior is documented and typed.
- Batch fetch remains deterministic and per-item isolated.
- `cargo fmt --check` passes.
- `cargo clippy --all-targets --all-features -- -D warnings` passes.
- `cargo test --all-features` passes.
- `cargo test --no-default-features` passes.
