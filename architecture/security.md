# Security Subsystem Deep Dive

**Path:** `src/core/security.rs`, `src/core/security_applicability.rs`, `src/meta/security_search.rs`, `src/meta/security_grouping.rs`, `src/meta/security_suggested_fetches.rs`, `src/meta/advisory_range.rs`, `src/meta/version_compare.rs`, `src/meta/adapter/advisory.rs`, `src/meta/adapter/security.rs`, `src/mcp/tools/security_search.rs`
**Purpose:** Advisory-backed vulnerability search with native provider lookups, version applicability assessment, severity filtering, and explicit capability accounting.

---

## Overview

`security_search` combines three evidence lanes — parallel web dispatch
(`security_search_subqueries()` with `advisory` / `vendor` / `defensive`
`PlannedSubquery` lanes), native advisory operations
(`lookup_advisory_scoped()`, `query_advisories_by_package_scoped()`), and
`KevClient` enrichment — then groups, severity-filters, and annotates
results. The entry point is `run_security_search_plan()` in
`src/meta/security_search.rs`; the MCP wrapper is `run_security_search` in
`src/mcp/tools/security_search.rs`. Every native operation ends in a
recorded terminal status; nothing is silently omitted.

---

## Core Types (`src/core/security.rs`)

`VulnerabilityMetadata` is the normalized advisory record: `cve_ids` /
`ghsa_ids` / `osv_ids` / `rustsec_ids`, `ecosystem` / `package`,
`affected_ranges` / `patched_ranges`, `vulnerable_versions` /
`patched_versions`, `severity`, `cvss_score`, `cvss_vector`, `epss_score`,
`kev` (`KevMetadata`), `published_at` / `modified_at` / `withdrawn_at`,
`references` (`VulnerabilityReference`), and `source`
(`VulnerabilitySource`: `Osv`, `GithubAdvisory`, `Nvd`, `Rustsec`,
`CisaKev`, `Generic` with `as_str()`).

`SeverityLevel` (`Critical`, `High`, `Medium`, `Low`, `Unknown` default)
provides `as_str()`, `from_str_loose()`, `rank()` (`Critical` 4 down to
`Unknown` 0), and `meets_minimum()` — `Unknown` never satisfies any
threshold.

`SecurityIdentifiers` holds `cve_ids`, `ghsa_ids`, `osv_ids`, `rustsec_ids`,
`cwe_ids`, `package`, `ecosystem`, `version`, `function_or_api` (from
`symbol:` hints), and `residual_query`. `SecurityIdentifiers::parse()`
merges the explicit `cve_id` / `ghsa_id` / `osv_id` / `rustsec_id` /
`package` / `ecosystem` / `version` fields with free-text regex extraction
(`CVE_RE`, `GHSA_RE`, plus RustSec/CWE patterns) and normalization
(`normalize_cve`, `normalize_ghsa`, `normalize_rustsec`,
`normalize_ecosystem`). `has_strong_identifier()` is true when any advisory
id, CWE id, or package+ecosystem pair is present.

`classify_query_kind()` maps identifiers to `SecurityQueryKind`: `Package`,
`Cve`, `Cwe`, `Api`, `ErrorMessage`, `Concept`, `Unknown` (default), each
with `as_str()`.

`SecuritySearchRequest` carries `query`, `ecosystem`, `package`, `version`,
the four explicit id fields, `severity_min`, `include_kev` /
`include_exploit_context` / `include_defensive_guidance` /
`include_vendor_advisories`, `max_results`, `max_per_group`, `freshness`,
`timeout_ms`, `providers`, `assess_applicability`, and `dependency_files`.
The tool requires a non-empty query, package, or id field, and validates
`severity_min` against `critical, high, medium, low`.

---

## Orchestration (`run_security_search_plan()`)

1. Parse identifiers from fields plus free-text query.
2. Run `security_search_subqueries()` — a `build_search_plan()` generic
   query plus `vendor` (`"{query} vendor advisory security bulletin"`) and
   `defensive` (`"{query} mitigation workaround fix patch"`) lanes through
   `dispatch_subqueries()`, returning cards, warnings, `providers_failed`,
   trust markers, and the raw `RetrievalAttempt` vec.
3. Check `advisory_provider_capabilities()`; without any native capability,
   push `native_advisory_search_unavailable` plus the standing
   `generic_context_untrusted` and `severity_unavailable` advisories.
4. Plan deduplicated identifiers in family order (CVE, GHSA, OSV, RustSec)
   via `plan_unique_advisory_identifiers()`, then execute bounded native
   lookups under `NativeOperationBudget`: `MAX_NATIVE_ADVISORY_IDENTIFIERS`
   (32 unique identifiers) and
   `MAX_NATIVE_ADVISORY_PROVIDER_OPERATIONS` (64 provider calls), surfaced
   as `NativeAdvisoryBudgetSummary` and
   `native_advisory_identifier_cap_reached` /
   `native_advisory_provider_operation_cap_reached` warnings.
5. Query advisories by package coordinate and enrich CVEs through
   `KevClient`.
6. Assess version applicability and parse dependency files when package /
   version data is present.
7. Apply `severity_min` filtering, group with `group_security_results()`,
   generate fetches with `generate_security_suggested_fetches()`, then run
   `materialize_evidence_roles()` and `evidence_postprocess::postprocess()`.

---

## Advisory Capabilities and Terminal Outcomes

`AdvisoryCapabilities { lookup_by_id, query_by_package }` is declared per
engine: `OsvEngine` and `GithubAdvisoryEngine` advertise both operations;
`NvdEngine` and `RustsecEngine` advertise `lookup_by_id` only. KEV coverage
arrives through `KevClient` enrichment rather than the scoped package-query
path.

`src/meta/adapter/advisory.rs` executes one operation per selected provider
(`selected_advisory_engines()`, `advisory_provider_capabilities()`).
`NativeAdvisoryOperation` is `LookupById { vulnerability_id }` or
`QueryByPackage { ecosystem, package, version }`. Each provider attempt ends
in exactly one `ProviderAdvisoryStatus`: `CapabilityUnavailable` (flag
false), `InterruptedByDeadline` (global deadline elapsed, including inner
`tokio::time::timeout` expiry), or `Completed(Result<T, EngineError>)`.
`ProviderAdvisoryOutcome` records provider id, operation, status, and
`duration_ms`.

Outcome mapping to the shared ledger (`native_advisory_attempt()`) never
drops a provider: successes record `SuccessWithResults` /
`SuccessZeroResults`, transport problems become `Failed` / `TimedOut` /
`RateLimited`, unsupported operations become
`SkippedCapabilityUnavailable`, budget-skipped providers become
`SkippedByPolicy`, and out-of-scope lanes become `NotApplicable`. The first
successful `lookup_advisory()` / non-empty `query_advisories_by_package()`
wins; otherwise the first error is returned. Lookup failures therefore
appear in the retrieval summary instead of vanishing.

---

## Native Lookups: CVE / GHSA / OSV / RustSec / KEV

- OSV (`OsvEngine`, `src/meta/engines/osv.rs`): `osv::lookup_by_id()`
  (`GET /vulns/{id}`, 404 → `Ok(None)`, bounded body via
  `read_bounded_body()`) and `osv::query_package()` (explicit
  `package.ecosystem` / `package.name` / `version` request body).
- GitHub Advisory (`GithubAdvisoryEngine`): `search_by_cve`,
  `search_by_ghsa`, and `search_by_package` back both `lookup_advisory()`
  (CVE- / GHSA-prefixed ids) and `query_advisories_by_package()`.
- NVD (`NvdEngine`): `lookup_by_cve()` for CVE-prefixed ids plus
  `keyword_search()` for generic queries.
- RustSec (`RustsecEngine`): `lookup_by_id()` for RUSTSEC-prefixed ids plus
  `search_by_keyword()`.
- KEV (`KevClient`, `src/meta/engines/kev.rs`; `CisaKevEngine`): TTL-cached
  catalog fetch from `KEV_CATALOG_URL` (`DEFAULT_CACHE_TTL` one hour,
  `with_cache_ttl()` override), `lookup()` by CVE id, enrichment with
  vendor/product, required action, due date, and ransomware usage; engine
  `lookup_advisory()` synthesizes `VulnerabilityMetadata` with
  `source: VulnerabilitySource::CisaKev` for CVE ids.

`VulnerabilityMetadata::merge()` deduplicates id/range/reference vectors
across providers and collapses mismatched sources to `Generic`.

---

## Applicability Assessment

`extract_advisory_ranges()` (`src/meta/advisory_range.rs`) converts
`affected_ranges` / `patched_ranges` / `vulnerable_versions` into
`AdvisoryRange { ecosystem, package, affected_range, fixed_versions,
introduced_versions, last_affected_versions, source }`, requiring a parsable
`PackageEcosystem` and non-empty package.

`assess_version_applicability()` returns a tri-state
`ApplicabilityOutcome { status, reasons, matched_ranges }`: exact fixed
match → `NotAffected`; explicit last-affected match or satisfied affected
range → `Affected`; no structured ranges or unparseable syntax →
`Unknown`. Multi-range combination uses `RangeMatch::combine()` — `Affected`
dominates, all-`NotAffected` stays `NotAffected`, any `Unknown` mixed with
`NotAffected` collapses to `Unknown`, so unevaluable ranges can never
produce a false safe verdict. `ApplicabilityStatus` is `Affected` /
`NotAffected` / `Unknown` (default) / `InsufficientEvidence` (query lacks
package/version data); `ApplicabilityConfidence` is `High` (structured
ranges + exact match) / `Medium` / `Low` (default).

Dependency parsing yields `DependencyFinding { ecosystem, package, version,
source_file, source_line, source_kind, confidence, relation,
resolved_version, version_requirement, reference_kind, reference_value,
provenance, target_context, integrity_hash }` with
`DependencySource` (`LockFile`, `Manifest`, `Dockerfile`, `WorkflowFile`,
`AdvisoryMetadata`, `RequestField`) and `DependencyRelation` (`Direct`,
`Transitive`, `Unknown`).

Typed evidence semantics (ADR-0004): `version` is a legacy
display/compatibility projection and never proves resolution.
`resolved_version` carries exact selected versions from lock evidence;
`version_requirement` carries manifest constraints (including strings shaped
like exact numbers, e.g. Cargo `serde = "1.0.193"`); `reference_kind` /
`reference_value` carry source references (tags, SHAs, digests, paths);
`provenance` carries the source locator or class; `target_context` carries
framework/environment scoping; `integrity_hash` carries checksum-only
observations such as `go.sum` entries. Requirement, reference, and integrity
evidence can never produce `Affected` / `NotAffected`; the orchestrator
emits an explicit `Unknown` assessment with a
`weak_evidence_ignored_for_exact_applicability` warning instead of silently
skipping (which would look like negative evidence). Unknown or unresolved
evidence never becomes `NotAffected`.

`compose_confidence()` bounds every assessment by both sides: High requires
High dependency evidence and High advisory/range confidence. `packages_match()`
compares package identity with ecosystem rules (PyPI canonicalization:
lowercase, collapse runs of `-`, `_`, `.` to `-`; NuGet is case-insensitive;
all other ecosystems compare exact text). Request-field and dependency-file
package matching share this one ecosystem identity policy through
`explicit_request_identity_ecosystem()`; unmapped advisory ecosystems never
inherit crates.io identity or version semantics and surface
`request_ecosystem_unassessed` instead of a firm assessment.
Dependency-driven assessments populate `version_source` and
`dependency_relation` from the matching finding; caller-supplied
package+version uses `version_source: RequestField` as exact request
evidence.

`parse_dependency_file_report()` returns a `DependencyParseReport {
findings, status, diagnostics }` with `ParseStatus` (`Complete`, `Partial`,
`Unsupported`, `Malformed`). Unrecognized filenames yield `Unsupported`;
per-format malformed/partial detection lands with the format milestones.
Dispatch uses path-aware basename extraction (native separators plus a
conservative backslash fallback) so Windows and Unix paths route
identically. `vendor/modules.txt` is recognized only when the immediate
parent directory is `vendor`; bare `modules.txt` files stay `Unsupported`.

Cargo uses structured TOML: `Cargo.lock` package entries yield exact
resolved versions with the lock `source` preserved as provenance (git/path
sources never inherit crates.io provenance); `Cargo.toml` dependency,
dev/build, and target-specific tables yield requirements, with `package =`
rename identity, `workspace = true` as unresolved inheritance, and git/path
provenance retained. `go.mod` requirements are minimum-version requirements
(`Manifest`, Medium), with `// indirect` mapped to `Transitive` and
`replace` directives retained as provenance (local replacements also typed
as path references). `go.sum` is integrity-only evidence and can never
drive resolved applicability. `vendor/modules.txt` headers yield exact
vendored versions (`## explicit` maps to `Direct`). Python requirements
implement the dependency-specifier subset that matters for evidence:
exact vs wildcard equality, arbitrary equality, ranges, compatible
release, direct `name @ URL` references, extras, and environment markers
(markers/extras preserved as target context, never evaluated); a
requirement is never a resolved version even when pinned. Poetry/uv
(`toml`) and Pipfile (`serde_json`) locks yield exact resolved versions
with source/path/git/index provenance preserved.

NuGet `packages.lock.json` is parsed as the versioned `dependencies`
target graph (never the legacy `libraries` shape): the target-framework
key (with optional runtime identifier) becomes target context, the
package key is the identity, `resolved` is exact evidence,
`requested` stays a requirement, `Direct`/`Transitive`/
`CentralTransitive`-like types map to relations (unknown future types
map to `Unknown` without rejection), `Project` entries become
project-reference provenance with no resolved version, and content
hashes are integrity metadata only. Unknown lock versions or
non-package target maps yield partial/unsupported diagnostics.

`.csproj` and Maven POM inputs are parsed structurally with `quick-xml`
0.38 (`default-features = false`, streaming pull parser over `&str`;
no encoding/serde/async surface, no external entity resolution, linear
scan over the 1 MiB-bounded input). `PackageReference` works across
line layouts in attribute and child-`Version` forms with `Condition`
and target-framework context preserved; versions stay requirements.
POM project dependencies are separated from `dependencyManagement`,
nested `exclusions`, plugin dependencies, and parent coordinates by
element-depth analysis; scope/optionality become target context and
`${...}` interpolation stays an unresolved requirement. Truncated XML
yields partial/malformed reports rather than silent empty output.
Gradle lockfiles keep exact coordinates with deterministic dedup;
`build.gradle`/`build.gradle.kts` accept literal and `("...")`
coordinates (including `platform(...)` wrappers) with dynamic/property
versions recorded as unresolved requirements, never resolved versions.

JavaScript lockfiles are format/version aware with explicit
unsupported diagnostics for future lock versions. npm reads
`lockfileVersion`: v2/v3 `packages` skip the root project entry,
derive scoped names from nested `node_modules` paths, type
workspace/link entries as workspace provenance (never registry
evidence), preserve git/tarball/file sources, and mark root-declared
dependencies direct; v1 `dependencies` are walked recursively under a
bounded depth with deterministic dedup. Yarn Classic grouped selectors
(including scoped packages) yield exact versions with descriptor
constraints kept as requirements; modern Berry locks are detected via
`__metadata` (supported versions 5-8, others explicit unsupported)
with exact `version:` plus `resolution:` identity, so `npm:` stays
registry evidence while workspace/portal/link/file/git protocols keep
non-registry provenance. pnpm v6 (`/name@version` keys with peer
suffixes stripped) and v9 (`name@version` IDs plus `snapshots`
isolation) use `importers` for direct marking and specifier
preservation, with workspace/link entries kept as workspace evidence.
No package identity ever contains YAML quote characters or peer-suffix
text. YAML-backed formats use narrow bounded generated-format parsers
rather than a generic YAML crate: the lockfile grammars needed are
small, machine-generated, and version-pinned in fixtures, so a general
parser would add alias/anchor attack surface and dependency footprint
for no maintenance benefit; the decision is revisited only if a future
lock version outgrows the generated grammar. Workflow files reuse the
same narrow-parser decision with a bounded `uses:` extraction seam
(quoted scalars and trailing comments handled, expressions never
evaluated).

GitHub Actions `uses:` values are typed references, never versions:
full SHAs are immutable commit references (High), tag-like refs stay
tags (Medium), branch/other mutable refs stay branches (Low), and
`${{ }}`/interpolated values are explicit unknown expression evidence.
`owner/repo/path@ref` reusable workflows keep their full path with
workflow provenance; local `./` and same-repository `$/` actions are
local findings, never external packages; `docker://` actions feed OCI
evidence. Docker `FROM [--platform=...] image [AS name]` parsing skips
internal stage aliases and `scratch`, reads tags, tag+digest pairs
(digest as immutable provenance plus integrity hash), untagged
latest, and ARG-interpolated refs distinctly, with registry ports
split correctly; findings use `Dockerfile` source kind. Compose
`image:` values share the same reference grammar. Dockerfile variants
(`Dockerfile`, `Dockerfile.*`, `*.dockerfile`) and compose filenames
(exact plus `docker-compose.*`/`compose.*` YAML) route under bounded
explicit basename rules, never bare path substrings.

Bundler `Gemfile.lock` files are parsed with an explicit section state
machine: only four-space resolved spec rows under a `GEM`/`GIT`/`PATH`
`specs:` block become exact findings; six-space child requirements
never do. `GEM`/`GIT`/`PATH` provenance (remote plus revision/branch
for git sources) is preserved, top-level `DEPENDENCIES` marks direct
package identities, and Gemfile Ruby code is never evaluated.
Composer `composer.lock` keeps `packages` and `packages-dev` exact
versions as resolved evidence with dev-package context preserved;
`source`/`dist` metadata (type, URL, commit reference, shasum)
distinguishes registry archives from VCS/path/custom provenance, dev
branch aliases stay distinguishable via `dev version` provenance plus
the commit reference, and empty versions never become exact findings.

---

## Budgets, diagnostics, and evidence assembly

A `DependencyParserBudget` (`core/security_applicability.rs`, owned by
the applicability layer) bounds every pipeline stage: file-list length,
files collected per root, aggregate bytes across a root, findings plus
warnings per file, and aggregate findings. Every parser receives the
per-file view and truncates before returning (`truncate_findings`
emits a `DependencyFileBudgetExceeded` warning per cap hit); the
collection seam (`cap_file_list`, `assemble_dependency_evidence` in
`meta/advisory_range.rs`) enforces the per-root caps and emits
`DependencyFileBudgetExceeded` once per breach. Caps compose (per-file
wins) and fail safe: truncation shortens evidence, never changes a
status or invents a finding.

Collection stays bounded above the per-file view: at most five
candidate lockfiles per root, deduped by content identity, with
`unsupported`/`malformed` files attempted (so their diagnostics land
as evidence) while empties and oversize files are skipped without
warnings. File-skip warnings (`Unreadable`) and per-format parse
warnings unify on `DependencyParseMalformed`, `DependencyFormatUnsupported`
(partial counts as its own kind, never conflated), and
`DependencyParsePartial`; `parse_diagnostic_warning()` maps report
`diagnostics:warning_codes` strings (including legacy `kind_*` and
`partial` prefixes) into these, so `ParseStatus` and warning codes
are two views of one fact. Weak-evidence outcomes (`Unsupported`, and
`Malformed` unless the advisory carries native version data) emit
`WeakEvidenceIgnoredForExactApplicability`, documenting that only
exact pinning drove applicability.

Applicability matching runs over a per-file finding index
(`build_finding_index`, matched position sets threading
through the matcher loop) rather than a re-paired scan, so grouped
advisories share one candidate ordering and per-advisory cost stays
sublinear in findings.

---

## Version Range and Comparison (`src/meta/version_compare.rs`)

`compare_versions_for_ecosystem()` and `version_satisfies_range()` dispatch
per ecosystem: crates/npm/Go/NuGet/RubyGems/Packagist/PyPI use semver-like
comparison (`compare_semver_like()` over numeric core segments with zero
padding, pre-release below release, build metadata ignored) and semver
range evaluation (`evaluate_semver_range()`); Maven uses `compare_maven()`
/ `evaluate_maven_range()`; OCI and GitHub Actions support equality only.
Uncomparable inputs return `None`, which the applicability layer treats as
`Unknown` rather than `NotAffected`.

---

## Severity Filtering

When `severity_min` is set, vulnerabilities without a qualifying severity
are dropped via `meets_minimum()`, and grouped cards carrying advisory
metadata below the threshold are retained only if they lack severity data
entirely (generic context is not severity-filtered away). The outcome is
always announced: `severity_min_applied: dropped {vulns} vulnerabilities
and {cards} source cards below severity_min={level}`, or
`severity_min_unenforced` when no card or vulnerability carries severity
metadata.

---

## Grouping and Suggested Fetches

`classify_security_result()` routes cards into `SecurityResultGroupKind` (9
variants: `AuthoritativeAdvisories`, `VendorAdvisories`,
`PackageAdvisories`, `KevEntries`, `PatchCommitsOrReleases`,
`ExploitDiscussion`, `DefensiveGuidance`, `GeneralContext`, `Other`):
cisa.gov hosts → `KevEntries`; osv.dev / nvd.nist.gov / github.com/advisories
/ ghsa / rustsec.org → `AuthoritativeAdvisories`; advisory paths and
`SourceKind::SecurityAdvisory` → `VendorAdvisories`; commit/pull/release /
changelog URLs → `PatchCommitsOrReleases`; exploit/poc/metasploit →
`ExploitDiscussion`; mitigation/hardening/defensive/best-practice →
`DefensiveGuidance`; repo issue URLs → `PackageAdvisories`; everything else
→ `GeneralContext`. `group_security_results()` emits groups in
`CANONICAL_GROUP_ORDER` with labels from `security_group_label()`.

`generate_security_suggested_fetches()` builds `FetchCandidate`s via
`FetchCandidateBuilder::new()` for authoritative CVE/GHSA/OSV/RustSec URLs
plus ecosystem package pages, top group results, and dependency-file
locators, then ranks in `FetchRankMode::Security`. `include_exploit_context`,
`include_defensive_guidance`, and `include_vendor_advisories` filter their
group kinds when `Some(false)`; `None` keeps default-on behavior while
authoritative advisories and KEV entries are always included as primary
evidence.

---

## No Silent Omissions

`RetrievalAttemptOutcome` covers every terminal state:
`SuccessWithResults`, `SuccessZeroResults`, `Failed`, `TimedOut`,
`RateLimited`, `SkippedByPolicy`, `SkippedCapabilityUnavailable`,
`NotApplicable`, `InterruptedByDeadline`, `TruncatedAfterPartialSuccess`.
Capability skips (`CapabilityUnavailable`), deadline interruptions,
budget-skipped providers (`SkippedByPolicy`), and `NotApplicable` lanes all
produce ledger entries with provider id, subquery id, operation id, outcome,
result count, error class, and duration. Missing native capability,
untrusted generic context, unavailable severity, and budget exhaustion each
produce named warnings, so an empty advisory section is always explained,
never silent.

---

**Back to:** [overview.md](overview.md)
# Dependency provenance and identity

Exact advisory applicability requires a resolved version and compatible
artifact evidence. Cargo findings require an explicit crates.io index source;
source-less Cargo lock entries are treated as local/workspace or unverified.
Go vendored module entries are eligible unless the manifest marks a
replacement. npm lock entries with omitted source metadata use npm's default
registry convention, while explicit registry URLs must be `registry.npmjs.org`.
PyPI default lock entries and explicit `pypi.org` sources, plus Gemfile.lock
entries from `rubygems.org`, may be assessed. Maven, NuGet, Packagist, OCI, and
GitHub Actions findings lack a checked-in public-registry origin contract and
remain unverified. Cargo git/path entries, Go replacements, local or workspace
references, and npm/Composer/Python custom sources are withheld as `Unknown`
when public registry identity is not established. Findings remain in the
response, with `dependency_provenance_unverified_for_advisory` on the
assessment.

Package identity is ecosystem-specific: PyPI lowercases and folds runs of
hyphen, underscore, and period to one hyphen; NuGet compares case-insensitively
([NuGet package ID matching](https://learn.microsoft.com/en-us/nuget/consume-packages/finding-and-choosing-packages));
npm identifiers are exact because published names must be lowercase
([npm naming rules](https://docs.npmjs.com/package-name-guidelines/)); other
ecosystems use exact text until a registry-specific normalization contract is
established. Go module paths are exact and never globally case-folded
([Go Modules Reference](https://go.dev/ref/mod)); Cargo package-name fields are
case-sensitive ([Cargo registry index](https://doc.rust-lang.org/cargo/reference/registry-index.html)).
