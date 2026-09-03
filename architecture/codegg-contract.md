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
| Fetch result | `fetch_` | url (or locator) + line_start + line_end + text_prefix | `fetch_2b3c4d5e6f789012` |
| Code span | `span_` | locator + line_start + line_end + symbol | `span_3c4d5e6f78901234` |
| Batch fetch item | `batch_` | label + index | `batch_5e6f789012345678` |
| Evidence bundle | `bundle_` | goal + source_ids + fetch_ids | `bundle_4d5e6f7890123456` |
| Locator | `loc_` | host + owner + repo + ref_name + path | `loc_5e6f789012345678` |
| Document | `doc_` | url + title + kind | `doc_6f78901234567890` |
| Document chunk | `chunk_` | doc_id + chunk_index + heading_path | `chunk_7890123456789012` |

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
| `freshness_unenforced` | Warning | freshness hint requested but no provider enforces it |
| `native_code_search_unavailable` | Warning | intent=code but no code search provider |
| `profile_degraded` | Warning | profile fell back to default providers |
| `profile_provider_not_built` | Warning | provider in profile has no constructed engine |
| `local_repo_match` | Info | local checkout found matching requested repo |
| `local_repo_dirty` | Warning | local checkout has uncommitted changes |
| `request_deadline_exceeded` | Warning | subqueries skipped due to deadline |
| `native_advisory_search_unavailable` | Warning | only generic web search was used |
| `version_match_unavailable` | Warning | affected version could not be determined |
| `kev_match` | Error | CVE(s) found in KEV catalog |
| `kev_absent_not_proof` | Warning | no CVE(s) found (absence is not proof) |
| `prompt_injection_marker_detected` | Error | injection markers detected in content |
| `coding_profile_degraded` | Warning | coding profile fell back to default |
| `package_resolution_fallback` | Warning | registry API failed, using fallback metadata |
| `local_repo_state_unknown` | Warning | local workspace dirty state could not be determined |
| `local_search_timeout` | Warning | local workspace search exceeded time limit |
| `local_search_truncated` | Warning | local workspace results were truncated |

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

> For the full operator-facing threat model — including fetch network boundaries, configuration escape hatches, prompt-injection handling, and recommended host-agent policy — see [threat-model.md](../docs/threat-model.md).

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
- `pinned_browser_permalink` — commit-stable browser URL
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
- `source_role_configuration` — configuration file

**Source kind:**
- `kind_official_docs` — official documentation
- `kind_package_registry` — package registry listing
- `kind_release_notes` — release notes
- `kind_issue_thread` — issue discussion
- `kind_pull_request` — pull request
- `kind_security_advisory` — security advisory
- `kind_source_file` — source code file

**Evidence strength:**
- `sparse_code_evidence` — limited code-level evidence

**Security:**
- `authoritative_advisory` — authoritative security advisory
- `vendor_advisory` — vendor-provided security advisory
- `security_consideration` — security-related evidence

**Research:**
- `primary_research_source` — primary research source
- `reference_implementation` — reference implementation
- `benchmark_source` — benchmark data source

**Query context:**
- `symbol_hint_match` — symbol name matched
- `path_hint_match` — file path matched
- `language_hint_match` — programming language matched
- `file_hint_match` — filename matched
- `error_context_match` — error text matched (exact-error mode)
- `version_migration_context` — version/migration context present
- `package_name_match` — package name matched
- `source_type_match` — source type matched query intent

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
    "root_path": "/Users/dev/projects/myrepo",
    "remote_host": "github",
    "remote_owner": "myorg",
    "remote_repo": "myrepo",
    "branch": "main",
    "commit": "a1b2c3d",
    "dirty_state": "clean",
    "match_confidence": "exact",
    "reasons": ["remote URL matches requested owner/repo"]
  }
}
```

### 8.2 Match Confidence

| Level | Meaning |
|-------|---------|
| `exact` | Remote URL matches requested host/owner/repo exactly |
| `strong` | Owner/repo matches but host differs (alias resolution) |
| `weak` | Partial match (name similarity only) |

### 8.3 Dirty State

| State | Meaning | Harness Action |
|-------|---------|----------------|
| `clean` | No uncommitted changes | Proceed normally |
| `dirty` | Uncommitted changes exist | Warn user; content may be stale relative to HEAD |
| `unknown` | Could not determine dirty state | Treat as dirty (conservative) |
| `not_git` | Not a git repository | Ignore dirty state |

### 8.4 File Classification Flags

Source cards from local workspace results include file classification metadata:

```json
{
  "file_classification": {
    "is_source": true,
    "is_test": false,
    "is_config": false,
    "is_documentation": false,
    "is_generated": false,
    "language": "rust",
    "size_bytes": 4096
  }
}
```

Harnesses can use classification flags to:
- Filter results by file type (source, test, config, docs)
- Detect generated files that may be stale
- Apply language-specific tooling

### 8.5 Workspace ID

Local workspace results include a `workspace_id` string that identifies the
local checkout across calls. Use this to:
- Track local workspace state across tool invocations
- Deduplicate results from the same workspace
- Correlate `repo_search`, `repo_fetch`, and `repo_map` calls

```json
{
  "workspace_id": "ws_a1b2c3d4e5f6a7b8"
}
```

The workspace ID is deterministic and derived from the workspace root path.
It does not change between calls unless the workspace configuration changes.

### 9. Retrieval Dimension State

| State | Meaning | Harness Action |
|-------|---------|----------------|
| `satisfied` | Evidence was retrieved (results found) | Use evidence; fetch full source before final use |
| `completed_no_match` | Provider responded successfully with zero results | Mark role as attempted; no evidence available |
| `failed` | Provider returned an error | Flag provider as degraded; consider retry |
| `skipped_by_policy` | Provider was excluded by budget or configuration | Note budget exhaustion; no retry needed |
| `capability_unavailable` | Provider does not support the requested capability | Do not retry this provider for this capability |
| `interrupted` | Global deadline prevented completion | Note deadline; remaining providers may still succeed |
| `partial` | Results were truncated after partial success | Fetch additional pages or sources if available |
| `not_applicable` | Role was not requested for this operation | Ignore for evidence purposes |

### 9.2 State Interpretation Order

When multiple dimensions exist for the same evidence role, harnesses
should interpret states in this priority order (highest to lowest):

1. `satisfied` — evidence exists, use it
2. `partial` — partial evidence exists, supplement if possible
3. `failed` — provider error, flag for retry
4. `interrupted` — deadline, may succeed on retry with more time
5. `completed_no_match` — no evidence from this provider
6. `skipped_by_policy` — excluded by policy/budget
7. `capability_unavailable` — provider cannot serve this role
8. `not_applicable` — role not requested

### 9.3 Dimension Count Fields

The `retrieval_summary` includes both attempt-level and dimension-level
count fields:

| Field | Level | Meaning |
|-------|-------|---------|
| `attempted_job_count` | Attempt | Total terminal retrieval attempts |
| `completed_job_count` | Attempt | Attempts with success or not-applicable |
| `failed_job_count` | Attempt | Attempts that failed, timed out, rate-limited, or were interrupted |
| `policy_skipped_count` | Attempt | Attempts skipped by policy |
| `capability_skipped_count` | Attempt | Attempts skipped due to unavailable capability |
| `attempted_dimension_count` | Dimension | Total role-expanded dimensions |
| `completed_dimension_count` | Dimension | Dimensions with evidence or no-match |
| `failed_dimension_count` | Dimension | Dimensions with failure or deadline interruption |
| `not_applicable_count` | Dimension | Dimensions where the role was not applicable |

**Invariant:** `attempted_job_count == completed_job_count + failed_job_count + policy_skipped_count + capability_skipped_count`.

### 9.4 Dimension State Fixtures

```json
{
  "retrieval_summary": {
    "attempted_job_count": 4,
    "completed_job_count": 2,
    "failed_job_count": 1,
    "policy_skipped_count": 1,
    "capability_skipped_count": 0,
    "attempted_dimension_count": 4,
    "completed_dimension_count": 2,
    "failed_dimension_count": 1,
    "not_applicable_count": 0,
    "dimensions": [
      {
        "evidence_role": "primary_implementation",
        "provider_id": "duckduckgo",
        "state": "satisfied",
        "absence_kind": "not_applicable",
        "attempt_outcome": "success_with_results",
        "result_count": 5,
        "truncated": false
      },
      {
        "evidence_role": "official_documentation",
        "provider_id": "startpage",
        "state": "failed",
        "absence_kind": "provider_failed",
        "attempt_outcome": "failed",
        "error_class": "connection_refused",
        "truncated": false
      },
      {
        "evidence_role": "usage_example",
        "provider_id": "brave",
        "state": "skipped_by_policy",
        "absence_kind": "provider_skipped_by_policy",
        "attempt_outcome": "skipped_by_policy",
        "truncated": false
      },
      {
        "evidence_role": "authoritative_security_advisory",
        "provider_id": "osv",
        "state": "completed_no_match",
        "absence_kind": "no_matching_evidence_found",
        "attempt_outcome": "success_zero_results",
        "truncated": false
      }
    ]
  }
}
```

### 9.5 Native Advisory Budget Warnings

Security responses may include budget-related warnings:

| Code | Severity | Meaning |
|------|----------|---------|
| `native_advisory_identifier_cap_reached` | Warning | Unique identifier limit reached; additional identifiers not scheduled |
| `native_advisory_provider_operation_cap_reached` | Warning | Provider-operation limit reached; provider operations skipped by policy |
| `native_advisory_provider_does_not_supply_manifest_metadata` | Warning | Advisory provider does not provide dependency manifest metadata |

These warnings are advisory. The retrieval summary's dimension states
(`SkippedByPolicy` for budget-excluded providers) provide the
machine-readable signal.

---

## 10. Capability Discovery

`provider_status` returns provider descriptors, cached health snapshots,
`code_hosts`, `server_capabilities`, `tool_capabilities`, and
`workflow_recipes`. The `probe` request field is reserved and currently
ignored; the tool reports configured state rather than performing live
provider probes.

Each provider descriptor includes `routable` (bool), `skip_reason`
(optional human-readable string), and `skip_code` (optional machine-readable
code from the `ProviderSkipCode` enum). Stable `skip_code` values:
`unknown_provider`, `disabled_by_user`, `missing_api_key`,
`missing_searxng_config`, `missing_base_url`, `invalid_base_url`,
`missing_local_backend`, `credential_not_configured`,
`credential_env_missing`, `credential_invalid`, `cooldown_active`,
`not_built`, `unknown`.

### 10.1 Server Capabilities

```json
{
  "generic_search": true,
  "explicit_fetch": true,
  "batch_fetch": true,
  "repo_search": true,
  "repo_fetch": true,
  "repo_map": true,
  "document_fetch": true,
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

### 10.2 Tool Capabilities

Per-tool feature details:

```json
{
  "repo_fetch": {
    "remote_hosts": ["github", "gitlab", "codeberg", "gitea", "forgejo"],
    "workspace": true,
    "line_ranges": true,
    "context_lines": true,
    "max_chars_enforced": true,
    "symbol_search": true,
    "expand_to_block": true,
    "max_block_lines": true
  },
  "repo_search": {
    "profiles": ["generic", "coding", "security", "research"],
    "package_resolution": ["crates_io", "pypi", "npm", "go", "maven", "nuget", "rubygems", "packagist", "oci", "github_actions"],
    "local_workspace": true,
    "subquery_telemetry": true,
    "supported_hosts": ["github", "gitlab", "codeberg", "gitea", "forgejo"]
  }
}
```

### 10.3 Routing Decision

Every search response includes a `routing_decision` field:

```json
{
  "routing_decision": {
    "requested_profile": "coding",
    "selected_providers": ["github_code", "duckduckgo"],
    "skipped_providers": [
      {
        "provider_id": "gitlab_code",
        "reason": "[not_built] Not built",
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

### 10.4 Retrieval outcome semantics

Security responses retain one retrieval attempt per selected native provider
operation. The attempt's `provider_id` is the executing provider, not an
identifier-family guess. Interpret outcomes as follows:

- `success_zero_results` means the provider completed and found no match;
- `failed`, `timed_out`, `rate_limited`, and `interrupted_by_deadline` are retrieval failures;
- `skipped_capability_unavailable` means the operation applied but the provider cannot perform it;
- `skipped_by_policy` means an otherwise capable provider was deliberately suppressed;
- `not_applicable` means the operation did not apply;
- `limit_reached_unknown` is possible, unconfirmed truncation.

Advisory records may deduplicate across providers, but attempts must not be
deduplicated away. Required roles with capability or policy skips remain
indeterminate in workflow coverage.

### 10.5 `web_fetch` Metadata-Only Mode

`web_fetch` supports `extract_mode = "metadata_only"` for explicit URL
fetches.

- HTML pages return title and description metadata without body text or
  a structured document.
- Non-HTML responses suppress body text and do not build a structured
  document.
- PDF responses with the `pdf` feature enabled return a minimal
  document that carries fetch context but no extracted body text.

Use `metadata_only` when you need page metadata but not the body.

### 10.6 Extractive Excerpts and Result Timestamps (Additive)

`SourceCard` may carry `excerpts: Vec<SourceExcerpt>` (at most 3,
500 chars each, 1,200 total) and `metadata.published_at` (RFC 3339).
Excerpts appear only when the caller requested them and are
`external_untrusted` like snippets. Stable IDs never include
excerpt/timestamp evidence, so harness deduplication keys are
unaffected. Harnesses should treat unknown `provenance` variants as
opaque and skip them, never crash.

### 10.7 Focused Fetch and Cache Controls (Additive)

`web_fetch` may return a `focus` selection (`chunks` in document
order with stable chunk IDs, `truncated`, `total_chars`) alongside
unchanged `text`/`document` fields. `focus` is null when the caller
did not request it. `cache_status` distinguishes `hit`,
`revalidated`, `miss`, `bypassed`, and `not_cacheable`; a `miss`
after `refresh` is a normal fresh fetch. Harnesses must not infer
transport internals from cache status beyond these documented
values.

---

## 11. Implementation Checklist

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
- [ ] Inspect provider-scoped retrieval attempts before treating security evidence as complete
- [ ] Do not treat `limit_reached_unknown` as confirmed truncation
- [ ] Do not require credentials for baseline search (keyless-core invariant)
- [ ] Use `provider_status` to check routability before invoking specialized tools
- [ ] Prefer native adapters when routable; fall back to keyless providers
- [ ] Preserve provenance distinctions; never label web results as native forge evidence
- [ ] Do not prompt for API keys on baseline operations
- [ ] Suggest optional credentials only when user explicitly needs native capability

---

## 12. Keyless-Core Invariant

eggsearch guarantees that a clean installation with no configuration file and
no provider credential environment variables starts successfully and provides
a useful keyless MCP search/fetch service. Harnesses must implement the
following:

### 12.1 Do Not Require Credentialed Providers

Baseline search, fetch, security, and research operations must work without
API keys. Harnesses must NOT:
- Prompt the user for API keys to perform baseline search
- Require credentials before attempting a tool call
- Treat missing credentials as a global server failure

### 12.2 Inspect Provider Status

Before routing, check `provider_status` to determine:
- Whether the server core is healthy
- Which providers are routable
- Whether missing credentials are provider-scoped

### 12.3 Prefer Native Adapters When Routable

When a native adapter (GitHub, GitLab, etc.) is routable, prefer it for
specialized operations. This provides better provenance and precision.

### 12.4 Continue with Keyless Providers When Adapters Unavailable

When optional adapters are not routable (missing credentials, disabled),
continue with keyless web providers. The response may be degraded but must
not fail.

### 12.5 Preserve Provenance Distinctions

Never label a generic web search result as native forge evidence. Use
`evidence_role` and `routing_decision` to distinguish provenance modes:
- `native forge adapter used` — adapter provided the result
- `keyless public HTTP/local route used` — explicit fetch, no adapter
- `generic web discovery used` — keyless web search result
- `provider capability unavailable` — adapter not routable
- `provider skipped because credential missing` — provider-scoped skip

### 12.6 Do Not Prompt for Keys on Baseline Operations

Harnesses must not display "API key required" warnings or prompts for
baseline search operations. Credential-related prompts are appropriate only
when the user explicitly requests a capability that requires native adapter
access (e.g., private repository access, exact code search on a specific
forge).

### 12.7 Suggest Optional Credentials Contextually

When a user's workflow would benefit from native adapter access (e.g.,
searching a private GitHub repository), suggest the optional credential
configuration. This suggestion must be:
- Contextual to the specific workflow, not a global prompt
- Labeled as optional enhancement, not a requirement
- Accompanied by a keyless fallback alternative

---

## 13. Schema Stability Rules

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
