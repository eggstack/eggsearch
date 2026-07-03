# eggsearch MCP Response Handling Contract

**Audience:** Coding-agent harness developers (codegg and similar).
**Status:** Phase 14 Workstream 3 — stable, versioned contract.
**Scope:** MCP tool responses from `web_search`, `repo_search`,
`security_search`, `research_search`, `repo_fetch`, `web_fetch`,
`batch_fetch`, `build_evidence_bundle`, and `provider_status`.

This document defines the machine-readable contract that harnesses must
implement to correctly consume, deduplicate, triage, and route eggsearch
MCP output. All types, codes, and semantics here are **stable** — breaking
changes follow the semver-compatible schema migration rules in AGENTS.md.

---

## 1. Deterministic Identity System

Every tool response carries **deterministic, content-derived IDs**
alongside the legacy random UUID-based `id` fields. Harnesses should
use the deterministic IDs for deduplication, cross-tool linking, and
evidence tracking.

### 1.1 ID Formats

| Entity | Prefix | Key Fields | Example |
|--------|--------|------------|---------|
| Source card | `src_` | provider_id + url + title + source_kind | `src_a1b2c3d4e5f6a7b8` |
| Suggested fetch | `suggested_` | url + group + priority | `suggested_1a2b3c4d5e6f7890` |
| Fetch result | `fetch_` | url + text_prefix | `fetch_2b3c4d5e6f789012` |
| Code span | `span_` | url + language + line_start + line_end + symbol | `span_3c4d5e6f78901234` |
| Evidence bundle | `bundle_` | goal + source_ids + fetch_ids | `bundle_4d5e6f7890123456` |
| Locator | `loc_` | host + owner + repo + ref_name + path | `loc_5e6f789012345678` |
| Document | `doc_` | url + title + kind | `doc_6f78901234567890` |

### 1.2 Linking Rules

```
SourceCard.stable_id  <-->  EvidenceBundleSource.source_id
SourceCard.stable_id  <-->  SuggestedFetch.source_id
FetchKey(fetch_url)   <-->  EvidenceBundleFetchedItem.fetch_id
FetchKey(fetch_url)   <-->  RepoFetchResponse.fetch_id
```

**Invariant:** `bundle_source_ids` ⊆ `search_result_stable_ids`.
Every source ID in an evidence bundle must originate from a search
response the caller previously received.

### 1.3 ID Verification

Harnesses should verify ID linkage when chaining tools:

```rust
fn verify_source_link(
    source: &SourceCard,
    fetch: &RepoFetchResponse,
) -> bool {
    fetch.stable_id.as_deref() == compute_fetch_id(&fetch.fetched_url, &fetch.text)
}
```

---

## 2. Structured Warnings

Every MCP tool response includes:
- `warnings: Vec<String>` — legacy human-readable strings
- `structured_warnings: Vec<AgentWarning>` — machine-readable, deduplicated

Harnesses **MUST** read `structured_warnings` and never parse `warnings`
for programmatic decisions.

### 2.1 Warning Severity → Harness Action

```rust
fn handle_warning(warning: &AgentWarning) {
    match warning.severity {
        Error => {
            // Blocks action. Show error panel. Optionally request user review.
            block_or_request_user_review(warning);
        }
        Warning => {
            // Degrades capability. Show in review panel as advisory.
            show_in_review_panel(warning);
        }
        Notice | Info => {
            // Attach to evidence metadata for downstream inspection.
            attach_to_evidence_metadata(warning);
        }
    }
}
```

### 2.2 Key Warning Codes

| Code | Severity | Meaning |
|------|----------|---------|
| `safe_search_unenforced` | Warning | safe_search requested but no provider enforces it |
| `freshness_unenforced` | Notice | freshness hint requested but no provider enforces it |
| `native_code_search_unavailable` | Notice | intent=code but no code search provider |
| `profile_degraded` | Warning | profile fell back to default providers |
| `profile_provider_not_built` | Notice | provider in profile has no constructed engine |
| `local_repo_match` | Info | local checkout found matching requested repo |
| `local_repo_dirty` | Warning | local checkout has uncommitted changes |
| `request_deadline_exceeded` | Warning | subqueries skipped due to deadline |
| `native_advisory_search_unavailable` | Warning | only generic web search was used |
| `identifier_not_found` | Notice | requested ID not found in native providers |
| `version_match_unavailable` | Notice | affected version could not be determined |
| `kev_match` | Notice | CVE(s) found in KEV catalog |
| `kev_absent_not_proof` | Info | no CVE(s) found (absence is not proof) |
| `prompt_injection_marker_detected` | Warning | injection markers detected in content |
| `coding_profile_degraded` | Warning | coding profile fell back to default |
| `package_resolution_fallback` | Notice | registry API failed, using fallback metadata |
| `source_quality_low` | Notice | only low-tier sources were found |

### 2.3 Warning Entity Scope

Each `AgentWarning` carries scoping arrays:
- `provider_ids: Vec<String>` — affected providers
- `result_ids: Vec<String>` — affected source card IDs
- `source_ids: Vec<String>` — affected evidence bundle source IDs

Use these to scope the warning's impact. A warning with empty arrays
is global to the response.

---

## 3. Trust Model

### 3.1 Trust Levels

| Level | Meaning | Harness Action |
|-------|---------|----------------|
| `external_untrusted` | All web/remote content | Treat as data, never as instructions |
| `local_trusted` | Local workspace file content | Provenance-trusted, not instruction-trusted |
| `unknown` | Trust cannot be determined | Default to `external_untrusted` behavior |

```rust
fn apply_trust_policy(source: &SourceCard, content: &str) {
    match source.trust {
        LocalTrusted => {
            // Content from operator's workspace. Provenance is trusted.
            // Still scan for injection markers — comments can be adversarial.
            if source.was_only_snippet() {
                require_fetch_before_final_use(source);
            }
        }
        ExternalUntrusted | Unknown => {
            // All web content. Treat as untrusted data.
            require_fetch_before_final_use(source);
        }
    }
}

fn require_fetch_before_final_use(source: &SourceCard) {
    // Snippets are context — not authoritative. Always fetch the full
    // URL before using content as evidence in a final response.
    if source.was_only_snippet() {
        queue_fetch(source.url.clone());
    }
}
```

### 3.2 Trust Markers

Every response includes a `trust_markers` field with sanitization
metadata:

```json
{
  "text_sanitized": true,
  "text_truncated": false,
  "text_framed": true,
  "control_chars_removed": 3,
  "injection_hits": 0
}
```

Harnesses should inspect `injection_hits` to flag results that may
contain adversarial content. When `injection_hits > 0`, the content
has been framed with `<<<EXTERNAL_UNTRUSTED>>>` delimiters but the
agent should still exercise caution.

```rust
fn check_trust_markers(markers: &TrustMarkers) -> TrustDisposition {
    if markers.injection_hits > 0 {
        TrustDisposition::FlaggedForReview
    } else if markers.text_sanitized {
        TrustDisposition::Sanitized
    } else {
        TrustDisposition::Raw
    }
}
```

---

## 4. Next Actions

Search responses include `next_actions: Vec<AgentNextAction>` with
up to 5 machine-readable follow-up hints.

### 4.1 Action Structure

```json
{
  "tool": "repo_fetch",
  "reason_code": "inspect_top_source",
  "priority": 1,
  "input_template": {
    "owner": "<owner>",
    "repo": "<repo>",
    "path": "<path>"
  },
  "source_ids": ["src_a1b2c3d4e5f6a7b8"]
}
```

### 4.2 Priority Semantics

| Priority | Meaning | Harness Behavior |
|----------|---------|-----------------|
| 1 | Most productive next step | Auto-suggest or execute if safe |
| 2 | High value alternative | Show as primary suggestion |
| 3 | Worthwhile follow-up | Show in suggestions panel |
| 4 | Optional enrichment | Show as secondary option |
| 5 | Exploratory | Show only on explicit request |

### 4.3 Reason Code Examples

| Reason Code | Tool | Meaning |
|-------------|------|---------|
| `inspect_top_source` | `repo_fetch` | Fetch the highest-ranked source card |
| `fetch_primary_advisory` | `web_fetch` | Fetch a primary security advisory |
| `fetch_counterpoint` | `web_fetch` | Fetch contradicting evidence |
| `bundle_evidence` | `build_evidence_bundle` | Package gathered evidence for handoff |
| `fetch_source_code` | `repo_fetch` | Fetch source code from a suggested URL |
| `resolve_package` | `repo_search` | Resolve package metadata from registry |

### 4.4 Input Template Handling

Harnesses must replace `<placeholders>` in `input_template` with
actual values from the response context. Placeholders use angle-bracket
notation and correspond to fields on the target tool's request type.

---

## 5. Suggested Fetch Reason Codes

Suggested fetches on `repo_search`, `security_search`, and
`research_search` responses include stable reason codes.

### 5.1 FetchRankReason Variants

**Provenance stability:**
- `pinned_raw_permalink` — commit-pinned raw content URL (most stable)
- `pinned_browser_permalinks` — commit-stable browser URL
- `mutable_raw_url` — mutable raw content
- `mutable_browser_url` — mutable browser URL
- `generic_web_url` — generic web page

**Evidence confidence:**
- `exact_confidence` — exact match evidence
- `strong_confidence` — strong match evidence
- `weak_confidence` — weak match evidence
- `unknown_confidence` — confidence could not be determined

**Source role:**
- `source_role_implementation` — implementation code
- `source_role_documentation` — documentation
- `source_role_readme` — README
- `source_role_example` — example code
- `source_role_test` — test code
- `source_role_changelog` — changelog
- `source_role_migration` — migration guide
- `source_role_benchmark` — benchmark

**Source kind:**
- `kind_official_docs` — official documentation
- `kind_package_registry` — package registry listing
- `kind_release_notes` — release notes
- `kind_issue_thread` — issue discussion
- `kind_pull_request` — pull request
- `kind_security_advisory` — security advisory

**Query context:**
- `symbol_hint_match` — symbol name matched
- `path_hint_match` — file path matched
- `language_hint_match` — programming language matched
- `file_hint_match` — filename matched
- `error_context_match` — error text matched (exact-error mode)
- `version_migration_context` — version/migration context present
- `package_name_match` — package name matched

### 5.2 Harness Selection Logic

```rust
fn select_fetch(candidates: &[SuggestedFetch]) -> &SuggestedFetch {
    // Candidates are pre-ranked by score. Prefer:
    // 1. Pinned provenance (raw permalink > browser permalink)
    // 2. Exact/strong confidence
    // 3. Implementation/documentation source role
    // 4. Symbol/path hint match for code queries
    candidates.iter().max_by_key(|f| f.score.unwrap_or(0))
}
```

---

## 6. Security Applicability

`security_search` responses with `assess_applicability: true` include
per-package/version applicability assessments.

### 6.1 Status Definitions

| Status | Meaning | Harness Action |
|--------|---------|----------------|
| `affected` | Advisory range matches requested version | Flag for upgrade/review |
| `not_affected` | Advisory range explicitly excludes version | Mark as resolved |
| `unknown` | Range syntax/ecosystem mapping prevents answer | Fetch more evidence |
| `insufficient_evidence` | No package/version data available | Request dependency files |

### 6.2 Confidence Levels

| Level | Meaning |
|-------|---------|
| `high` | Structured ranges + exact version match |
| `medium` | Manifest range or best-effort parsing |
| `low` | No structured ranges available |

### 6.3 Remediation Categories

| Category | Harness Action |
|----------|----------------|
| `upgrade` | Suggest version bump in manifest |
| `pin` | Pin to specific safe version |
| `replace` | Suggest alternative dependency |
| `remove_dependency` | Remove dependency entirely |
| `configuration_mitigation` | Apply config-level hardening |
| `feature_disable` | Disable vulnerable feature/code path |
| `vulnerable_api_avoidance` | Avoid calling vulnerable API surface |
| `transitive_override` | Override transitive dependency version |
| `vendor_patch` | Apply vendor-provided patch |
| `monitor_only` | Monitor for upstream fixes; no immediate action |
| `manual_review` | Manual review required; insufficient evidence |
| `no_action_supported_by_evidence` | No actionable remediation from evidence |

```rust
fn suggest_remediation(assessment: &ApplicabilityAssessment) -> Vec<Action> {
    assessment.rem remediation.iter().map(|r| match r.category {
        Upgrade => Action::BumpVersion(r.fixed_versions.clone()),
        Pin => Action::PinVersion(r.fixed_versions.clone()),
        ConfigurationMitigation => Action::ApplyConfigMitigation(r.description.clone()),
        MonitorOnly => Action::AddToWatchlist(r.description.clone()),
        ManualReview => Action::FlagForHumanReview(r.description.clone()),
        NoActionSupportedByEvidence => Action::None,
        _ => Action::Custom(r.category.as_str().into(), r.description.clone()),
    }).collect()
}
```

### 6.4 Safety Boundary

Every applicability response includes the warning:
`applicability_not_exploitability`. Harnesses must NOT treat
`affected` status as proof of exploitability or `not_affected` as
proof of safety. This is metadata comparison, not runtime analysis.

---

## 7. Research Evidence Model

`research_search` responses include structured evidence analysis:
claims, conflicts, source quality, and evidence gaps.

### 7.1 Research Claims

```json
{
  "id": "claim_abc123",
  "text": "axum provides faster routing than actix-web based on benchmarks",
  "claim_type": "performance",
  "confidence": "medium",
  "supporting_source_ids": ["src_def456", "src_ghi789"],
  "conflicting_source_ids": ["src_jkl012"],
  "missing_evidence": ["reproducible benchmark with identical workload"],
  "source_quality_notes": ["sources are vendor blogs, not peer-reviewed"]
}
```

**Harness handling:** Claims are deterministic metadata, NOT truth
judgments. Harnesses should:
1. Use `supporting_source_ids` to fetch primary evidence
2. Use `conflicting_source_ids` to present counterpoints
3. Use `missing_evidence` to suggest follow-up fetches
4. Never assert claims as factual without verification

### 7.2 Research Conflicts

Conflicts link opposing sources with a topic:

```json
{
  "id": "conflict_xyz789",
  "topic": "axum vs actix-web performance",
  "claim_ids": ["claim_abc123"],
  "side_a_source_ids": ["src_def456"],
  "side_b_source_ids": ["src_jkl012"],
  "notes": ["benchmarks use different workloads"]
}
```

Harnesses should present both sides and let the user decide.

### 7.3 Evidence Gaps

| Gap Kind | Meaning | Recommended Action |
|----------|---------|-------------------|
| `no_primary_source` | No authoritative source found | Fetch official docs |
| `no_recent_source` | All sources are stale | Fetch recent discussions/releases |
| `no_benchmark_source` | No benchmarks found | Search for benchmarks |
| `no_security_source` | No security analysis found | Search security advisories |
| `no_migration_changelog` | No changelogs/migration guides | Fetch changelog |
| `only_secondary_sources` | Only blog/news sources found | Fetch primary sources |
| `conflicting_evidence_unresolved` | Conflicting claims remain unresolved | Fetch counterpoints |
| `source_needs_fetch` | Source card has no fetch yet | Fetch source URL |
| `version_context_missing` | No version context provided | Request version info |

### 7.4 Source Quality Signals

| Signal | Meaning |
|--------|---------|
| `primary_source` | Official/primary documentation |
| `maintained_current` | Recently updated content |
| `version_specific` | Content is version-pinned |
| `commit_pinned` | URL contains commit SHA |
| `reproducible_benchmark` | Benchmark with reproducible methodology |
| `peer_reviewed` | Peer-reviewed or standards body |
| `stale_source` | Content is outdated |
| `secondary_source` | Derived/summarized content |
| `anecdotal_source` | Personal experience, not systematic |
| `marketing_source` | Vendor marketing material |

---

## 8. Local Workspace Metadata

When `repo_search` includes local workspace results, source cards carry
additional identity and state metadata.

### 8.1 Local Repo Match

```json
{
  "local_repo_match": {
    "root": "/Users/dev/projects/myrepo",
    "remote_identity": {
      "host": "github",
      "owner": "myorg",
      "repo": "myrepo"
    },
    "branch": "main",
    "commit": "a1b2c3d",
    "dirty_state": "clean",
    "workspace_id": "ws_abc123def456",
    "match_confidence": "exact",
    "reasons": ["remote URL matches requested owner/repo"]
  }
}
```

### 8.2 Match Confidence

| Level | Meaning | Score Boost |
|-------|---------|-------------|
| `exact` | Remote URL matches requested host/owner/repo exactly | +50 |
| `strong` | Owner/repo matches but host differs (alias resolution) | +50 |
| `weak` | Partial match (name similarity only) | +50 |

### 8.3 Dirty State

| State | Meaning | Harness Action |
|-------|---------|----------------|
| `clean` | No uncommitted changes | Proceed normally |
| `dirty` | Uncommitted changes exist | Warn user; content may be stale relative to HEAD |
| `unknown` | Could not determine dirty state | Treat as dirty (conservative) |
| `not_git` | Not a git repository | Ignore dirty state |

### 8.4 File Classification Flags

Local workspace results include boolean classification flags:

| Flag | Meaning | Harness Action |
|------|---------|----------------|
| `is_generated` | Auto-generated file (build output, protobuf, etc.) | Deprioritize; likely not first-party logic |
| `is_vendor` | Vendored third-party code | Treat as external untrusted |
| `is_test` | Test file | Link to corresponding implementation |
| `is_example` | Example/demo code | Treat as supplementary |
| `is_config` | Configuration file | Treat as operational context |
| `is_lockfile` | Dependency lockfile | Use for reproducibility, not logic |

### 8.5 Workspace ID

`workspace_id` is a deterministic FNV-1a hash of:
- Root directory path
- Remote URLs
- HEAD commit

Use this to track workspace state across calls without re-discovering
the repository each time. If `workspace_id` changes, re-fetch workspace
metadata.

---

## 9. Capability Discovery

`provider_status` returns `server_capabilities` and `tool_capabilities`
advertising which tool classes are available.

### 9.1 Server Capabilities

```json
{
  "generic_search": true,
  "explicit_fetch": true,
  "batch_fetch": true,
  "repo_search": true,
  "repo_fetch": true,
  "repo_map": true,
  "security_search": true,
  "research_search": true,
  "evidence_bundle": true,
  "pdf_fetch": false,
  "local_workspace": true
}
```

Harnesses should check capabilities before invoking specialized tools.
If a capability is `false`, fall back to `web_search` with appropriate
`intent` hints.

### 9.2 Tool Capabilities

Per-tool feature details:

```json
{
  "repo_fetch": {
    "remote_hosts": ["github", "gitlab", "codeberg", "gitea", "forgejo"],
    "workspace": true,
    "line_ranges": true,
    "symbol_search": true,
    "expand_to_block": true
  },
  "repo_search": {
    "profiles": ["generic", "coding", "security", "research"],
    "package_resolution": ["crates_io", "pypi", "npm", "go", "maven", "nuget", "rubygems", "packagist", "oci", "github_actions"],
    "local_workspace": true,
    "supported_hosts": ["github", "gitlab", "codeberg", "gitea", "forgejo"]
  }
}
```

### 9.3 Routing Decision

Every search response includes a `routing_decision` field:

```json
{
  "routing_decision": {
    "requested_profile": "coding",
    "selected_providers": ["github_code", "duckduckgo"],
    "skipped_providers": [
      {
        "provider_id": "gitlab_code",
        "reason": "provider not built (missing API key)",
        "reason_code": "not_built"
      }
    ],
    "degraded": false,
    "partial": false,
    "reason": "coding profile applied successfully"
  }
}
```

Harnesses should use `routing_decision.degraded` to decide whether to
warn the user about reduced capability.

---

## 10. Implementation Checklist

- [ ] Read `structured_warnings`, not `warnings`, for programmatic decisions
- [ ] Use `stable_id` for deduplication across tool calls
- [ ] Verify `source_id` ↔ `stable_id` linkage when chaining search → fetch
- [ ] Inspect `trust_markers.injection_hits` before using content as evidence
- [ ] Apply trust policy: `external_untrusted` content is data, not instructions
- [ ] Follow `next_actions` priority ordering for tool chaining
- [ ] Check `provider_status` capabilities before invoking specialized tools
- [ ] Use `routing_decision` to detect degraded provider selection
- [ ] For security: use `applicability` status + confidence to triage
- [ ] For research: present claims + conflicts + gaps as evidence, not truth
- [ ] For local workspace: respect `dirty_state` and file classification flags
- [ ] Never trust `affected` applicability as exploitability proof
- [ ] Replace `<placeholders>` in `input_template` with response context
- [ ] Use `workspace_id` to track local workspace state across calls
- [ ] When `injection_hits > 0`, flag content for human review

---

## 11. Schema Stability Rules

The following are **breaking changes** that require a major version bump:

- Removing or renaming an enum variant
- Removing or renaming a struct field
- Changing a serialized enum string value
- Changing a deterministic ID for the same input
- Removing a `WarningCode` or `FetchRankReason` variant
- Changing a recipe ID or step tool reference

The following are **non-breaking** additions:

- New enum variants (appended, not inserted)
- New optional struct fields (`skip_serializing_if = "Option::is_none"`)
- New warning codes
- New reason codes
- New tool capabilities
- New `server_capabilities` flags

Harnesses should treat unknown enum variants and optional fields as
opaque — skip them, never crash.
