# Phase 10: Local Workspace Trust and Identity Hardening

## Objective

Strengthen local workspace search/fetch so coding agents can safely use current-checkout evidence without confusing it with remote web/code-host evidence. Local results should carry explicit workspace identity, git state, remote matching, generated/vendor/test/source classification, and trust markers.

This phase is critical for codegg because local workspace evidence is often the most relevant evidence for coding tasks, but it can also be dirty, untracked, generated, vendored, stale relative to remote, or maliciously crafted as prompt-injection content.

## Current context

Eggsearch already has a `local_workspace` provider, local search/fetch paths, trust markers, repo matching metadata, dirty-state warnings, stable identity helpers, and evidence bundles. Phases 6–9 will enrich code evidence, workflow hints, security verdicts, and research metadata. Phase 10 makes local workspace handling precise enough for agents to prefer local evidence intentionally.

## Non-goals

- Do not build a persistent full-text index unless already present and bounded.
- Do not watch filesystem changes continuously.
- Do not execute project code or scripts.
- Do not trust local content as instructions.
- Do not allow path traversal outside configured roots.
- Do not hide dirty/untracked state.

## Workstream 1: Workspace identity model

### Required fields

For each local workspace root, expose a compact identity snapshot:

```rust
pub struct LocalWorkspaceIdentity {
    pub workspace_id: String,
    pub root: String,
    pub canonical_root: String,
    pub git_present: bool,
    pub git_root: Option<String>,
    pub remote_urls: Vec<String>,
    pub normalized_remotes: Vec<String>,
    pub current_branch: Option<String>,
    pub head_commit: Option<String>,
    pub dirty_state: LocalDirtyState,
    pub untracked_count: Option<u32>,
    pub ignored_count: Option<u32>,
}
```

Keep output bounded; do not list every untracked file unless requested and capped.

### Identity rules

- `workspace_id` should be stable for the canonical root and git remote/head where appropriate.
- Remote URL normalization should support common GitHub/GitLab SSH/HTTPS forms.
- Root paths should not leak excessive host-specific details if a config option later redacts them, but initial behavior can preserve current paths.

## Workstream 2: Remote repository matching

### Required behavior

When a local result is returned for a query with host/owner/repo hints, include a remote match object:

```rust
pub struct LocalRemoteMatch {
    pub requested_host: Option<String>,
    pub requested_owner: Option<String>,
    pub requested_repo: Option<String>,
    pub matched: bool,
    pub match_confidence: EvidenceConfidence,
    pub matched_remote: Option<String>,
    pub reasons: Vec<String>,
}
```

### Match cases

- HTTPS remote matches host/owner/repo.
- SSH remote matches host/owner/repo.
- `.git` suffix is ignored.
- Case normalization is conservative and host-aware.
- Multiple remotes are handled deterministically.
- Mismatches are explicit; do not silently prefer local content for a different repo.

## Workstream 3: Local file classification

### Required classification fields

For each local result/fetch:

- `source_role`
- `language`
- `is_generated`
- `is_vendor`
- `is_test`
- `is_example`
- `is_config`
- `is_lockfile`
- `first_party_confidence`

Reuse phase 6 classification helpers where possible.

### Generated/vendor heuristics

Detect common generated/vendor paths:

- `target/`, `dist/`, `build/`, `out/`, `.next/`, `.nuxt/`
- `node_modules/`, `vendor/`, `third_party/`, `extern/`
- generated comments such as `@generated`, `Code generated`, `DO NOT EDIT`
- lockfiles and minified bundles

## Workstream 4: Trust markers and prompt-injection handling

### Requirements

Local content should be treated as local evidence, not instructions. Even if local content is more provenance-trusted than remote snippets, it can contain prompt-injection markers.

- Preserve control-character stripping/framing/injection-scan markers.
- Emit structured warning for local prompt-injection marker hits.
- Distinguish `local_trusted` provenance from `instruction_trusted`; the latter should never be implied.
- Include affected source/fetch IDs in warnings where possible.

## Workstream 5: Local fetch path hardening

### Requirements

- Reject path traversal outside allowed roots.
- Canonicalize paths before reading.
- Enforce max bytes/chars before returning content.
- Preserve line-number slicing and truncation metadata.
- Preserve symlink policy explicitly. If symlinks are followed, ensure the final target remains under allowed roots unless config explicitly permits otherwise.

### Tests

- `../` traversal rejected.
- Symlink escaping root rejected or handled according to policy.
- Large file truncates predictably.
- Binary file detection avoids returning junk text.
- Line slicing returns correct lines and stable IDs.

## Workstream 6: Local search ranking and preference rules

### Required behavior

Local results should rank high only when they are relevant and trusted enough for the task.

Ranking signals:

- exact path/symbol match
- host/owner/repo remote match
- clean checkout vs dirty checkout
- first-party source over vendor/generated
- implementation/test/example role match
- recent modified file if task context asks for current work

Do not blindly rank local content above remote evidence when repo identity mismatches or file is generated/vendor.

## Workstream 7: Evidence bundle integration

### Required behavior

Evidence bundles should preserve local workspace metadata, dirty state, remote match, source role, and trust markers. Bundle consumers should be able to tell local evidence from remote evidence immediately.

Add gap kinds if absent:

- `local_checkout_dirty`
- `local_remote_mismatch`
- `local_generated_or_vendor_only`
- `local_untracked_file`
- `local_source_unfetched`

## Workstream 8: Provider status and recipes

### Requirements

`provider_status` should expose local workspace state without scanning entire projects:

- enabled/configured
- number of roots
- invalid roots count
- git roots detected if cheap
- local capabilities

Workflow recipes from phase 7 should mark local workspace investigation as available only when local provider is configured.

## Tests

Add tests for:

- Clean checkout identity and remote match.
- Dirty checkout warning and metadata.
- Untracked file metadata where supported.
- HTTPS and SSH remote URL normalization.
- Remote mismatch does not claim match.
- Generated/vendor/test/example classification.
- Path traversal and symlink escape rejection.
- Local prompt-injection marker warning includes source/fetch ID.
- Local evidence bundle preserves dirty state and trust metadata.
- Provider status local capability state matches config.

## Acceptance criteria

- Local results are clearly labeled with workspace identity and git state.
- Remote repo matching is deterministic and tested for common URL forms.
- Local generated/vendor/test/source roles are visible and used in ranking.
- Local fetch is path-safe and bounded.
- Local prompt-injection markers are structured warnings, not generic strings only.
- Evidence bundles preserve local metadata.
- Agents can safely prefer local evidence when it is relevant, clean, and repo-matched.
- `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all-features` pass.
