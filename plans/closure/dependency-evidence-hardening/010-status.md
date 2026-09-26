# Dependency Evidence Hardening M010 — Explicit Request Identity Closure Status

Status: closed

Source implementation plan:

- `plans/implementation/dependency-evidence-hardening/011-explicit-request-ecosystem-identity-consistency.md`

Source subsystem roadmap:

- `plans/subsystems/dependency-evidence-hardening-roadmap.md`

Applicable ADR:

- `plans/adrs/ADR-0004-typed-dependency-evidence-semantics.md`

Prior closure evidence:

- M008 corrective closure:
  `plans/closure/dependency-evidence-hardening/008-status.md`
- M009 terminal closure:
  `plans/closure/dependency-evidence-hardening/009-status.md`

Implementation commit:

- `23676ccc34af76eebb4e1adccfb34aa3c8f9203e` — route explicit request applicability through ecosystem identity matrix

Baseline for this corrective: `deb345b5b15f09043d3f3b6bf722e7b5ba72e363`.

Historical M001-M009 closure evidence remains unchanged. M010 is a narrow
corrective that removes the explicit-request identity bypass without changing
parser behavior, provider retrieval, or the public MCP request shape.

## 1. Defect and fix

At baseline `deb345b5`, the explicit request applicability branch in
`src/meta/security_search.rs` used `eq_ignore_ascii_case()` on package
strings, compared ecosystem strings with `eq_ignore_ascii_case()`, evaluated
ranges with the first extracted range ecosystem or `CratesIo` fallback, and
defaulted the emitted assessment ecosystem to `CratesIo`.

After M010, the branch calls
`explicit_request_identity_ecosystem()` in `src/meta/advisory_range.rs`,
which parses the advisory ecosystem with `PackageEcosystem::parse()`, parses
an explicitly supplied request ecosystem with the same parser and requires
enum equality, and compares packages with `packages_match()`. A firm range
assessment requires a mapped advisory ecosystem; unmapped ecosystems produce
no `Affected`/`NotAffected` claim. Range evaluation and the emitted
assessment both use the parsed advisory ecosystem. The original request
package spelling is preserved in the assessment.

Before/after examples (same advisory version metadata, request version
`v1.2.3` inside `>= v1.0.0, < v2.0.0` where applicable):

| Request | Advisory | Before | After |
|---|---|---|---|
| `example.com/mod` (go) | `Example.com/Mod` (go) | `Affected` via case-insensitive match | No assessment; Go identity is exact |
| `Example.com/Mod` (go) | `Example.com/Mod` (go) | `Affected` | `Affected` with Go version semantics, `version_source: request_field` |
| `my-package` (pypi) | `my_package` (pypi) | `Affected` | `Affected`; shared PEP 503 normalization retained |
| `MY_PACKAGE` (pypi) | `my_package` (pypi) | `Affected` | `Affected` |
| `newtonsoft.json` (nuget) | `Newtonsoft.Json` (nuget) | `Affected` | `Affected`; NuGet stays case-insensitive |
| `Lodash` (npm) | `lodash` (npm) | `Affected` via case-insensitive match | No assessment under exact identity |
| `Serde` (crates_io) | `serde` (crates_io) | `Affected` via case-insensitive match | No assessment under exact identity |
| `serde` (crates_io) | `serde` (`terraform`, unmapped) | `Unknown` assessment with fabricated `crates_io` ecosystem | No assessment; `request_ecosystem_unassessed` warning |
| `serde` (crates_io) | `serde` (no ecosystem) | `Unknown` assessment with fabricated `crates_io` ecosystem | No assessment; `request_ecosystem_unassessed` warning |
| `lodash` (npm) | `lodash` (pypi) | `Affected` via case-insensitive ecosystem match | No assessment; ecosystem enum mismatch |
| `serde` (crates_io) | `serde` (crates_io) | `Affected` | `Affected` with `version_source: request_field`, `confidence: high` |

## 2. Identity and range consistency

Go case-distinct module paths do not match in the actual request workflow;
the `go_case_variant_produces_no_assessment` integration fixture fails if
`eq_ignore_ascii_case(pkg)` is restored in the explicit branch. PyPI
(`my-package`, `my_package`, case variants) and NuGet case variants continue
to match. Exact-identity ecosystems (crates.io, npm, Go, Maven, RubyGems,
Packagist, GitHub Actions, OCI) reject case variants.

Request ecosystem handling is enum-based: an explicitly supplied request
ecosystem must parse and equal the advisory ecosystem; an invalid caller
ecosystem is never coerced into another ecosystem. When the request omits an
ecosystem, the parsed advisory ecosystem supplies the identity rule.

Unknown/missing advisory ecosystems cannot default to crates.io for package
identity, range evaluation, or emitted assessment ecosystem. The
vulnerability result is preserved; only the firm assessment is withheld, with
`request_ecosystem_unassessed` making the condition visible.

`extract_advisory_ranges()` already requires a parsable `PackageEcosystem`
and emits ranges carrying that ecosystem, so every extracted range agrees
with the parsed advisory ecosystem by construction. Passing the parsed
advisory ecosystem to `assess_version_applicability()` therefore cannot
silently select an inconsistent first-range ecosystem. No separate
inconsistent-metadata regression was added because such metadata is not
representable by the current model.

## 3. Warning contract change

Additive warning code `request_ecosystem_unassessed`
(`WarningCode::RequestEcosystemUnassessed`, severity `Warning`) is emitted
when an explicit package/version request cannot be assessed because advisory
ecosystem metadata is missing or unrecognized. No existing code described
this condition: `version_mismatch` reports a package found with no matching
ranges, `version_match_unavailable` reports `assess_applicability != true`,
and `dependency_provenance_unverified_for_advisory` covers dependency
findings, not request fields. The new code maps through `KNOWN_PREFIXES` and
`convert_warnings()`; unknown-ecosystem advisories no longer surface as
`UnknownWarning` or a fabricated crates.io assessment.

No recurrence static guard was added. The Go case-variant integration fixture
is the stable behavioral guard; a source-text guard on
`eq_ignore_ascii_case` would couple to local variable names without adding
signal beyond that fixture.

## 4. Verification

All local gates were run against implementation candidate
`23676ccc34af76eebb4e1adccfb34aa3c8f9203e`:

| Gate | Result |
|---|---|
| `cargo fmt --check` | Pass |
| `cargo clippy --locked --all-targets --all-features -- -D warnings` | Pass |
| `cargo test --locked --features mock --test security_applicability_contract` | Pass; 67 tests including 10 explicit-request identity integration fixtures |
| `cargo test --locked --features mock --test security_applicability_regression` | Pass; 24 tests |
| `cargo test --locked --features mock --test security_workflow` | Pass; 15 tests |
| `cargo test --locked --features mock --test static_guards` | Pass; 44 tests |
| `cargo test --locked --all-features` (lib) | Pass; 3,302 library tests including 9 explicit-request helper unit tests, all integration suites, 4 doctests |
| `make bench-check` | Pass; Criterion harness compiled with `--no-run` |
| `make check` | Pass; fmt, clippy, no-default check, all-features tests, hygiene, packaging contract |
| Hosted repository CI | Pass; [run 36253474005](https://github.com/eggstack/eggsearch/actions/runs/36253474005) on the exact implementation candidate |

No parser or shared evidence code changed beyond the new request-identity
helper and its call site, so no standalone parser fuzz campaign was required
by the plan. The change introduces no new I/O, network access, execution, or
authorization surface; ambiguous cases become unmatched or warned rather than
`Affected`/`NotAffected`.

## 5. Residual findings and recommendation

| Severity | Residual |
|---|---|
| High/Medium | None identified |
| Low, accepted limitation | Exact-identity ecosystems may omit case-variant advisory matches where no shared normalization contract exists; this cannot merge distinct identities. |
| Low, accepted limitation | Advisories with missing/unrecognized ecosystems produce no firm request assessment by design; the vulnerability record remains visible with `request_ecosystem_unassessed`. |

No future plan is blocked by this work. Tool-surface M006 evaluation and M007
decomposition were already ready before M010 and remain ready; dependency
evidence M001-M009 remain closed historical evidence. Closing M010 returns
the dependency evidence hardening roadmap and registry to `closed` with this
record as the current control point.

Recommendation: **closed**. M010 acceptance criteria are met on the exact
candidate above.
