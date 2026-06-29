# Phase 7: Security Context Enrichment

## Purpose

Make `security_search` and security-flavored `repo_search` more useful for coding agents by attaching richer, normalized vulnerability and defensive-programming context to search results.

This phase is not about turning eggsearch into a scanner. It is about retrieval quality for codegg security workflows: when an agent is researching an API, dependency, function, package, CVE, CWE, or exploit class, eggsearch should return structured context that helps the agent decide what matters, what sources are authoritative, and what code-level defensive patterns are relevant.

## Non-goals

Do not implement active vulnerability scanning, exploit execution, proof-of-concept generation, pentest automation, or dependency graph resolution. Do not attempt to decide exploitability of the user’s local code. Do not add risky autonomous security actions.

## Current baseline

The repo already has:

- `security_search` as a dedicated MCP tool.
- `repo_search` fields for package, ecosystem, version, compare version, and `include_security_context`.
- Package resolver support for crates.io, PyPI, and npm.
- OSV integration in the security path.
- Source cards with vulnerability metadata slots.

This phase should build on those surfaces and normalize the response so agents can compare sources reliably.

## Data model additions

Add or extend a normalized security context type:

```rust
pub struct SecurityContext {
    pub query_kind: SecurityQueryKind,
    pub identifiers: Vec<SecurityIdentifier>,
    pub affected_packages: Vec<AffectedPackageSummary>,
    pub vulnerability_summaries: Vec<VulnerabilitySummary>,
    pub defensive_guidance: Vec<DefensiveGuidance>,
    pub source_quality: SecuritySourceQuality,
    pub warnings: Vec<String>,
}

pub enum SecurityQueryKind {
    Package,
    Cve,
    Cwe,
    Api,
    ErrorMessage,
    Concept,
    Unknown,
}

pub struct SecurityIdentifier {
    pub kind: SecurityIdentifierKind,
    pub value: String,
    pub confidence: EvidenceConfidence,
}

pub enum SecurityIdentifierKind {
    Cve,
    Cwe,
    Ghsa,
    Osv,
    Package,
    Ecosystem,
    Version,
    FunctionOrApi,
}
```

Keep the schema compact. The goal is agent-readable structured context, not a full vulnerability database mirror.

## Source quality classification

Add deterministic source quality metadata for security results:

```rust
pub enum SecuritySourceTier {
    PrimaryAdvisory,
    VendorAdvisory,
    PackageRegistryAdvisory,
    MaintainerDiscussion,
    ReleaseNotes,
    SecurityResearch,
    NewsOrBlog,
    CommunityDiscussion,
    Unknown,
}
```

Map common domains and metadata:

- `nvd.nist.gov` -> `PrimaryAdvisory`
- `osv.dev` -> `PrimaryAdvisory`
- `github.com/advisories`, `GHSA` -> `PackageRegistryAdvisory`
- package registry advisories -> `PackageRegistryAdvisory`
- official project security pages -> `VendorAdvisory` or `MaintainerDiscussion`
- GitHub issues/PRs/releases -> `MaintainerDiscussion` or `ReleaseNotes`
- random blogs/news -> lower confidence unless query asks for exploit discussion or field reports.

Add `RankReason::SecurityPrimarySource`, `RankReason::SecurityMaintainerSource`, and `RankReason::VersionAffectedMatch` if not already present.

## Query interpretation

Extend security query parsing to detect:

- CVE IDs: `CVE-YYYY-NNNN...`
- GHSA IDs: `GHSA-xxxx-xxxx-xxxx`
- OSV IDs where obvious.
- CWE IDs: `CWE-79`, `CWE-89`, etc.
- Package ecosystem hints: `crate:`, `npm:`, `pypi:`, `ecosystem:`.
- Version constraints: `introduced`, `fixed`, `<`, `<=`, `>=`, `^`, `~`, semver-like strings.
- API/function names from `symbol:` or obvious code tokens.

Do not overfit. When uncertain, put a warning or low-confidence identifier rather than forcing a type.

## Provider strategy

For `security_search`, use a source mix:

1. OSV or configured advisory API when package/CVE/GHSA identifiers are present.
2. Existing search providers for official docs, maintainer discussions, release notes, and migration guides.
3. GitHub issue/release providers when repo/package hints resolve to a repository.

For `repo_search(include_security_context = true)`, do not duplicate the entire `security_search` response. Attach a compact `security_context` summary plus security-specific warnings and suggested fetches.

## Defensive guidance extraction

Add deterministic guidance categories without making prescriptive exploit claims:

```rust
pub struct DefensiveGuidance {
    pub category: DefensiveGuidanceCategory,
    pub summary: String,
    pub source_urls: Vec<String>,
    pub confidence: EvidenceConfidence,
}

pub enum DefensiveGuidanceCategory {
    UpgradeOrPin,
    InputValidation,
    OutputEncoding,
    AuthenticationOrAuthorization,
    DeserializationHardening,
    PathTraversalHardening,
    SsrFHardening,
    SqlInjectionHardening,
    XssHardening,
    CryptoConfiguration,
    ResourceLimit,
    SafeApiUsage,
    Unknown,
}
```

Keep summaries derived from retrieved source cards or deterministic category matching. Do not synthesize exploit instructions. Phrase guidance defensively.

## Ranking behavior

Security search should prioritize:

- Exact identifier match over fuzzy text match.
- Primary advisories over blogs for vulnerability facts.
- Official fix/release notes over general discussion for remediation.
- Maintainer issue/PR threads for regression/patch details.
- Sources with affected version metadata when version was supplied.

Add warnings when:

- A package was found but no vulnerability matched the supplied version.
- Version comparison was not possible.
- Advisory sources disagree.
- Only low-tier sources were found.
- Results are broad concept-level matches rather than exact vulnerability matches.

## Response changes

For `security_search`, include:

```json
{
  "security_context": { ... },
  "groups": [...],
  "suggested_fetches": [...],
  "warnings": [...],
  "telemetry": {...}
}
```

For `repo_search` with `include_security_context`, include a smaller context object:

```json
"security_context": {
  "query_kind": "package",
  "identifiers": [...],
  "vulnerability_count": 3,
  "highest_severity": "high",
  "source_quality": {...},
  "warnings": [...]
}
```

## Safety boundaries

This phase must keep content retrieval and classification defensive:

- No exploit execution.
- No payload generation.
- No vulnerability validation against live targets.
- No instructions for bypassing protections.
- Exploit-related search results may be returned as external untrusted source cards only when the query is clearly defensive/research-oriented; do not enrich them into step-by-step procedures.

## Tests

Add tests for:

- CVE parsing.
- GHSA parsing.
- CWE parsing.
- Package/version hint parsing.
- Source tier classification for NVD, OSV, GitHub advisories, release notes, maintainer issues, and random blogs.
- Exact CVE query ranks advisory sources before blogs.
- Package security query includes OSV/advisory context when available through mocks.
- Version mismatch produces a warning, not a false vulnerable claim.
- `repo_search(include_security_context = true)` attaches compact context.
- Security results preserve `external_untrusted` trust labels.

Use mocked providers and local fixtures. Do not require live OSV/NVD/network access in tests.

## Documentation

Update README and AGENTS.md with:

- Security search examples for CVE, package+version, API misuse, and CWE.
- Explanation of source tiering.
- Warnings that results are retrieval context, not exploitability determination.
- codegg recommendation: use `security_search` for dedicated security research; use `repo_search(include_security_context = true)` when security is secondary to API/repo understanding.

## Acceptance criteria

Phase 7 is complete when:

- `security_search` returns normalized `security_context`.
- Exact vulnerability/package identifiers are parsed and reflected in response metadata.
- Security source tiering influences ranking and is exposed in metadata.
- Defensive guidance categories are attached when supported by source evidence.
- `repo_search(include_security_context = true)` returns compact context without bloating normal repo search.
- Safety boundaries are documented and tested.
- All new behavior has deterministic tests.
- `cargo fmt`, clippy, and tests pass.
