# Phase 8: Security Applicability and Defensive-Action Output

## Objective

Make `security_search` produce compact, evidence-linked, defensive security triage output that a coding agent can safely use. The tool should distinguish affected, not affected, unknown, and insufficient-evidence states; explain version-range reasoning; preserve advisory provenance; and recommend defensive actions without overclaiming exploitability.

This phase should improve the security workflow for codegg while preserving eggsearch as a bounded retrieval/evidence tool, not a vulnerability scanner or exploit assistant.

## Current context

The repo already contains security-oriented search, OSV/RustSec/GHSA/CVE handling, KEV context, dependency parsing, and tri-state applicability work from prior passes. The corrective pass also improved warning truthfulness. Phase 8 should tighten the agent-facing shape so security outputs are immediately actionable for defensive coding tasks.

## Non-goals

- Do not provide exploit instructions.
- Do not attempt runtime exploitability determination.
- Do not scan networks or execute vulnerable code.
- Do not treat advisory absence as proof of safety.
- Do not collapse `unknown` into `not_affected`.
- Do not fetch arbitrary linked exploit pages automatically.

## Workstream 1: Normalize applicability verdicts

### Required verdict states

Use a stable enum for advisory/package applicability:

- `affected`
- `not_affected`
- `unknown`
- `insufficient_evidence`

Use `unknown` when advisory range syntax, ecosystem mapping, package aliasing, or version parsing prevents a firm answer. Use `insufficient_evidence` when the query lacks package/version/dependency data needed to assess applicability.

### Required fields

Each applicability item should include:

```rust
pub struct SecurityApplicabilityVerdict {
    pub advisory_id: String,
    pub source: String,
    pub ecosystem: Option<String>,
    pub package: Option<String>,
    pub version: Option<String>,
    pub status: ApplicabilityStatus,
    pub confidence: EvidenceConfidence,
    pub matched_ranges: Vec<String>,
    pub fixed_versions: Vec<String>,
    pub reasons: Vec<String>,
    pub warnings: Vec<AgentWarning>,
    pub source_ids: Vec<String>,
    pub fetch_ids: Vec<String>,
}
```

The exact type may differ, but it should preserve status, confidence, version-range rationale, and evidence links.

## Workstream 2: Defensive remediation categories

### Required categories

Add or normalize remediation categories:

- `upgrade`
- `pin`
- `replace`
- `remove_dependency`
- `configuration_mitigation`
- `feature_disable`
- `vulnerable_api_avoidance`
- `transitive_override`
- `vendor_patch`
- `monitor_only`
- `manual_review`
- `no_action_supported_by_evidence`

Each category should include a short rationale and evidence links.

### Constraints

- Do not recommend a specific version unless advisory metadata or package registry metadata supports it.
- If fixed versions are unknown, say so and recommend manual review or vendor advisory fetch.
- If applicability is unknown, do not recommend `no_action_supported_by_evidence`.

## Workstream 3: Advisory source quality and provenance

### Required source classes

Classify security sources:

- `primary_advisory`
- `vendor_advisory`
- `maintainer_advisory`
- `database_record`
- `kev_record`
- `release_note`
- `patch_commit`
- `issue_thread`
- `exploit_discussion`
- `defensive_guidance`
- `secondary_article`
- `unknown`

Add rank/quality reasons for security output:

- official database
- vendor maintained
- maintainer source
- version range present
- fixed version present
- KEV match
- patch evidence
- release-note evidence
- low authority / secondary only

## Workstream 4: Package and dependency-file mapping

### Requirements

- Keep ecosystem parsing conservative and explicit.
- Preserve installed version source: direct user arg, lockfile, manifest, dependency file, inferred package resolver.
- Distinguish direct dependency from transitive dependency when available.
- Include dependency file path and line/package context where possible.
- Handle aliases/casing carefully; do not over-normalize package names across ecosystems.

### Tests

- Cargo.lock direct and transitive dependency findings.
- package-lock direct/transitive where parser supports it.
- go.mod/go.sum version source distinction.
- Missing version -> insufficient evidence, not not_affected.
- Unsupported ecosystem -> unknown with warning.

## Workstream 5: KEV and exploit context boundaries

### Requirements

KEV/exploit fields should answer defensive questions:

- Is any CVE in this result listed in KEV?
- Is exploit discussion present in sources?
- Is exploit context primary, secondary, or unverified?
- What defensive urgency does this imply?

Do not include procedural exploit steps. If snippets contain exploit instructions, preserve trust/sanitization markers and avoid elevating them into recommended actions.

### Tests

- CVE with KEV match gets KEV evidence and defensive urgency.
- CVE without KEV match gets absence-not-proof warning.
- Exploit-discussion result is classified without generating exploit instructions.

## Workstream 6: Security suggested fetches

### Required suggested fetch types

Security search should recommend explicit fetches for:

- primary advisory
- vendor advisory
- OSV/GHSA/RustSec/NVD record
- maintainer issue or patch
- release notes / changelog
- package manifest / lockfile context
- defensive guidance

Each suggested fetch should include:

- stable ID
- source ID
- reason code
- advisory/package/version context
- priority
- expected information gain

### Reason codes

- `primary_advisory`
- `vendor_guidance`
- `database_record`
- `patch_evidence`
- `fixed_version_release_notes`
- `dependency_context`
- `defensive_guidance`
- `kev_context`

## Workstream 7: Response shape and backward compatibility

### Requirements

Preserve existing `SecuritySearchResponse` fields. Add structured fields rather than replacing current groups/cards.

Recommended top-level additions:

- `applicability_verdicts`
- `remediation_actions`
- `security_evidence_summary`
- `source_quality`
- `next_actions`

If existing `applicability` already covers most of this, extend it rather than creating duplicates. Avoid large payload expansion by keeping summaries compact and linking to evidence IDs.

## Tests

Add tests for:

- Affected package/version returns `affected` with matched range and fixed version.
- Not affected package/version returns `not_affected` only when range comparison is conclusive.
- Unknown range syntax returns `unknown`.
- Missing version returns `insufficient_evidence`.
- Defensive action is `upgrade` when fixed version exists.
- Defensive action is `manual_review` when applicability unknown.
- No exploit instructions are included in remediation action text.
- Suggested fetches have stable IDs and source IDs.
- Evidence bundle preserves security verdicts and remediation actions if included.

## Acceptance criteria

- `security_search` produces clear defensive verdicts without overclaiming.
- Unknown and insufficient-evidence states remain distinct from not affected.
- Remediation actions are category-based, evidence-linked, and defensive.
- KEV/exploit context is represented safely and conservatively.
- Dependency-file findings preserve version source and direct/transitive context where supported.
- Suggested fetches prioritize primary advisory/vendor/patch/release evidence.
- Backward compatibility is preserved for existing response consumers.
- `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all-features` pass.
