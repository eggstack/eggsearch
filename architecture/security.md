# Security Subsystem Deep Dive

**Path:** `src/core/security.rs`, `src/core/security_applicability.rs`, `src/meta/security_search.rs`, `src/meta/security_grouping.rs`, `src/meta/security_suggested_fetches.rs`
**Purpose:** Security vulnerability and advisory search with normalized metadata, version applicability assessment, and structured result grouping.

---

## Overview

The security subsystem combines web search with native advisory lookups (CVE, GHSA, OSV, RustSec, KEV), version applicability assessment, dependency file parsing, and severity filtering into a unified `security_search` MCP tool.

---

## Core Types (`src/core/security.rs`)

### Vulnerability Metadata

```
VulnerabilityMetadata
  ├── cve_ids / ghsa_ids / osv_ids / rustsec_ids: Vec<String>
  ├── ecosystem / package: Option<String>
  ├── affected_ranges / patched_ranges: Vec<String>
  ├── vulnerable_versions / patched_versions: Vec<String>
  ├── severity: Option<SeverityLevel>
  ├── cvss_score: Option<f64>
  ├── cvss_vector: Option<String>
  ├── epss_score: Option<f64>
  ├── kev: Option<KevMetadata>
  ├── published_at / modified_at / withdrawn_at: Option<String>
  ├── references: Vec<VulnerabilityReference>
  └── source: VulnerabilitySource
```

`SeverityLevel`: `Critical`, `High`, `Medium`, `Low`, `Unknown`

`VulnerabilitySource`: `Osv`, `GithubAdvisory`, `Nvd`, `Rustsec`, `CisaKev`, `Generic`

### Security Identifiers

`SecurityIdentifiers` provides regex-based parsing for:
- CVE identifiers (`CVE-YYYY-NNNNN`)
- GHSA identifiers (`GHSA-xxxx-xxxx-xxxx`)
- OSV identifiers
- RustSec identifiers (`RUSTSEC-YYYY-NNNN`)
- CWE identifiers (`CWE-NNN`)
- Package:ecosystem:version hints
- Symbol hints

### Query Classification

`classify_query_kind()` → `SecurityQueryKind`:
- `Package` — package name detected
- `Cve` — CVE identifier detected
- `Cwe` — CWE identifier detected
- `Api` — API/method name detected
- `ErrorMessage` — error message pattern detected
- `Concept` — generic security concept
- `Unknown` — no classification

### Source Tiers

`classify_source_tier()` → `SecuritySourceTier` (9 tiers):
1. `PrimaryAdvisory` — primary advisory databases (NVD, OSV, RustSec)
2. `VendorAdvisory` — vendor or project security pages
3. `PackageRegistryAdvisory` — package registry advisory data (GitHub Advisories)
4. `MaintainerDiscussion` — maintainer discussion (issues, PRs)
5. `ReleaseNotes` — release notes and changelogs
6. `SecurityResearch` — security research and analysis
7. `NewsOrBlog` — news articles or blog posts
8. `CommunityDiscussion` — forums, Stack Overflow
9. `Unknown`

### Remediation

`SecurityRemediation` with categories:
- `upgrade`, `pin`, `replace`, `remove_dependency`
- `configuration_mitigation`, `feature_disable`
- `vulnerable_api_avoidance`, `transitive_override`
- `vendor_patch`, `monitor_only`, `manual_review`
- `no_action_supported_by_evidence`

`validate_text_safety()` checks offensive/vulnerability-class keyword blocklists.

---

## Applicability Assessment (`src/core/security_applicability.rs`)

### Advisory Range Extraction

`extract_advisory_ranges()` pulls `AdvisoryRange` from `VulnerabilityMetadata`:
- Ecosystem, package name, affected range string
- Fixed, introduced, last_affected versions
- Source attribution

### Version Applicability

`assess_version_applicability()` determines if a version is affected:

| Status | Meaning |
|--------|---------|
| `Affected` | Advisory range matches requested version |
| `NotAffected` | Advisory range explicitly excludes version |
| `Unknown` | Range syntax/ecosystem mapping prevents answer |
| `InsufficientEvidence` | No package/version data available |

### Dependency Finding

`DependencyFinding` captures parsed dependency info:
- Ecosystem, package, version
- Source file, line, kind
- Confidence level
- Dependency relation (direct/transitive/unknown)

### Confidence Levels

| Level | Meaning |
|-------|---------|
| `High` | Structured ranges + exact version match |
| `Medium` | Manifest range or best-effort parsing |
| `Low` | No structured ranges available |

---

## Security Search Orchestration (`src/meta/security_search.rs`)

### Flow

```
1. Parse query for security identifiers (CVE, GHSA, package, etc.)
2. Build web search plan with security intent
3. Run bounded parallel web search dispatch
4. Native advisory lookups:
   a. CVE/GHSA/RustSec → lookup_advisory() per identifier
   b. OSV → query_advisories_by_package() + lookup_by_id()
   c. KEV → KevClient enrichment
5. Version applicability assessment (if package + version provided)
6. Dependency file parsing (if dependency files found)
7. Severity filtering
8. Result grouping
9. Suggested fetch generation
```

### Native Operation Budget

`NativeOperationBudget` limits advisory lookups:
- 32 unique identifiers
- 64 provider operations

Budget exhaustion produces `native_advisory_identifier_cap_reached` or `native_advisory_provider_operation_cap_reached` warnings.

### KEV Enrichment

`KevClient` provides CISA KEV catalog lookup with TTL cache. Enriches CVEs with:
- Vendor, product
- Required action
- Due date
- Known ransomware usage

---

## Result Grouping (`src/meta/security_grouping.rs`)

Groups results into `SecurityResultGroupKind` (9 variants):

| Group | Content |
|-------|---------|
| `AuthoritativeAdvisories` | Primary advisory sources (OSV, NVD, GHSA, RustSec) |
| `VendorAdvisories` | Vendor-provided security bulletins |
| `PackageAdvisories` | Package registry advisory records |
| `KevEntries` | CISA KEV catalog entries |
| `PatchCommitsOrReleases` | Fix commits, patched releases |
| `ExploitDiscussion` | Exploit analysis, PoC discussions |
| `DefensiveGuidance` | Mitigation, hardening guidance |
| `GeneralContext` | General security context |
| `Other` | Unclassified results |

Native advisory capability split (`advisory_capabilities()`): id+package
(OSV, GitHub Advisory); id only (NVD, RustSec, CISA KEV).

---

## Suggested Fetches (`src/meta/security_suggested_fetches.rs`)

Generates `SecuritySuggestedFetch` from:
- Resolved advisory IDs (OSV/NVD/GHSA/RustSec URLs)
- Ecosystem package pages
- Top group results
- Dependency file findings

Supports flags: `include_exploit_context`, `include_defensive_guidance`, `include_vendor_advisories`.

Uses `fetch_ranking` pipeline in `FetchRankMode::Security` mode.

---

## Retrieval Attempt Ledger

All native advisory lookups produce `RetrievalAttempt` records:
- `provider_id`: executing provider
- `outcome`: `RetrievalAttemptOutcome` (`SuccessWithResults`, `SuccessZeroResults`, `Failed`, `TimedOut`, `RateLimited`, `SkippedCapabilityUnavailable`, `SkippedByPolicy`, `InterruptedByDeadline`, `NotApplicable`, …)
- `result_count`: findings from this operation
- `error_class`: classified failure reason

These merge into the retrieval summary alongside web-search results. Lookup failures are never silently discarded.

---

**Back to:** [overview.md](overview.md)
