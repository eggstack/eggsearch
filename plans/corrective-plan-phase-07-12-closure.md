# Corrective Plan: Phase 7–12 Closure

## Objective

Close the remaining correctness and wiring gaps after the Phase 7–12 implementation pass. The implementation appears broad and mostly aligned with the roadmap: provider diagnostics, package ecosystem expansion, security applicability, evidence bundles, code-host coverage, and the regression corpus all landed. This corrective pass should be narrow and verification-driven.

The most important issue is security applicability correctness. A likely inverted comparison in range evaluation can produce incorrect `affected`/`not_affected` results. Because this path can influence defensive dependency triage, it must be corrected before treating Phase 9 as closed.

## Scope

In scope:

1. Fix security applicability version/range evaluation correctness.
2. Add adversarial and ecosystem-specific tests for affected/fixed range semantics.
3. Audit Gitea/Forgejo configured-host wiring end to end.
4. Clean stale package resolver documentation and capability examples.
5. Tighten CI/test verification evidence and feature matrix.
6. Add targeted corpus scenarios for the corrected behavior.

Out of scope:

- New package ecosystems beyond those already added.
- Full dependency solving.
- Runtime exploitability analysis.
- Persistent provider health storage.
- More code-host providers.
- New MCP tool concepts beyond what already landed.

## Current state summary

Since the Phase 7–12 plans, the repo added:

- `src/meta/provider_diagnostics.rs` for process-local provider health, cooldowns, routing decisions, and capability telemetry.
- Expanded `PackageEcosystem` and `package_resolver` support for Go, Maven, NuGet, RubyGems, Packagist, OCI, and GitHub Actions.
- `security_applicability`, `advisory_range`, `dependency_parse`, and `version_compare` modules.
- Evidence bundle core/meta modules.
- Code-host fetch support for Codeberg and helper URL builders for Gitea/Forgejo.
- Corpus runner and scenarios.
- GitHub Actions CI workflow.

The remaining risks are concentrated in correctness, not missing top-level architecture.

## Workstream 1: Fix security applicability range evaluation

### Problem

`advisory_range::evaluate_range()` appears to invert at least the `>=` comparison branch. For a range expression such as `>= 2.0.0`, a version lower than `2.0.0` should fail and a version greater than or equal to `2.0.0` should pass. The current branch appears to accept `Less` or `Equal`, which can invert applicability conclusions.

This is high priority because false `not_affected` answers are dangerous, and false `affected` answers produce noisy security work.

### Required changes

Audit and correct every comparison operator in advisory range evaluation:

- `>= X`: pass when `version >= X`; fail when `version < X`.
- `> X`: pass when `version > X`; fail when `version <= X`.
- `<= X`: pass when `version <= X`; fail when `version > X`.
- `< X`: pass when `version < X`; fail when `version >= X`.
- `= X`: pass when equal; fail otherwise.

For comma-separated ranges, all clauses must be satisfied. Example: `>= 2.0.0, < 3.0.0` means affected only if both clauses are true.

If any clause cannot be compared, return `None`/unknown instead of treating it as true. Do not silently fall through to `Some(true)` for unrecognized range syntax.

### Implementation notes

Prefer making the evaluator explicit and small:

```rust
fn evaluate_clause(version: &str, clause: &str, ecosystem: &PackageEcosystem) -> Option<bool> {
    // parse operator and target
    // compare version to target
    // return Some(true/false) or None
}

fn evaluate_range(version: &str, range: &str, ecosystem: &PackageEcosystem) -> Option<bool> {
    let mut saw_clause = false;
    for clause in split_clauses(range) {
        saw_clause = true;
        match evaluate_clause(version, clause, ecosystem)? {
            true => continue,
            false => return Some(false),
        }
    }
    if saw_clause { Some(true) } else { None }
}
```

Do not default unknown syntax to affected or unaffected. Return unknown with a reason.

### Required tests

Add table-driven tests for every operator:

| Range | Version | Expected |
|---|---:|---|
| `>= 2.0.0` | `1.9.9` | false |
| `>= 2.0.0` | `2.0.0` | true |
| `>= 2.0.0` | `2.1.0` | true |
| `> 2.0.0` | `2.0.0` | false |
| `> 2.0.0` | `2.0.1` | true |
| `<= 2.0.0` | `1.9.9` | true |
| `<= 2.0.0` | `2.0.0` | true |
| `<= 2.0.0` | `2.0.1` | false |
| `< 2.0.0` | `1.9.9` | true |
| `< 2.0.0` | `2.0.0` | false |
| `= 2.0.0` | `2.0.0` | true |
| `= 2.0.0` | `2.0.1` | false |
| `>= 2.0.0, < 3.0.0` | `1.9.9` | false |
| `>= 2.0.0, < 3.0.0` | `2.5.0` | true |
| `>= 2.0.0, < 3.0.0` | `3.0.0` | false |

Also test unknown syntax:

- `range = "banana"`, `version = "1.0.0"` returns unknown.
- Mixed known/unknown clauses return unknown unless a known clause already conclusively fails.

### Acceptance criteria

- Comparison operators are correct.
- Unknown range syntax yields `unknown`, not implicit affected/not affected.
- Table-driven tests cover all operators and boundary cases.
- Security applicability responses use `unknown` when range evaluation returns unknown.

## Workstream 2: Applicability status semantics and false-negative prevention

### Problem

`version_in_ranges()` returns `(bool, reasons)`, which cannot distinguish `not affected` from `unknown`. The public model has `ApplicabilityStatus::{Affected, NotAffected, Unknown}`, so the internal evaluator should preserve a three-state result.

### Required changes

Replace or supplement boolean applicability with a tri-state internal type:

```rust
pub enum RangeMatch {
    Affected,
    NotAffected,
    Unknown,
}
```

Rules:

- If a version exactly matches a fixed version, status is `NotAffected` with high confidence for that advisory source.
- If version matches an explicit affected version list, status is `Affected`.
- If version is not in an explicit affected version list and the list is declared complete by source semantics, status may be `NotAffected`; otherwise prefer `Unknown`.
- If version satisfies affected range, status is `Affected`.
- If version is outside a successfully evaluated affected range, status is `NotAffected` for that range.
- If no structured range exists, status is `Unknown`.
- If range syntax cannot be evaluated, status is `Unknown`.

Where multiple ranges/advisories apply:

- Any `Affected` result should dominate `NotAffected`.
- `Unknown` plus no `Affected` but some `NotAffected` can be `NotAffected` only if all relevant ranges were evaluated successfully.
- Otherwise return `Unknown`.

### Tests

- No ranges -> `Unknown`.
- Unknown range syntax -> `Unknown`.
- Fixed version exact match -> `NotAffected`.
- Affected range match -> `Affected`.
- Outside affected range -> `NotAffected`.
- Multiple ranges with one affected -> `Affected`.
- Multiple ranges with one unknown and no affected -> `Unknown`, unless policy explicitly says not affected.

### Acceptance criteria

- No security applicability path collapses unknown into not affected.
- Public response statuses correctly use `unknown`.
- Reasons explain why status is unknown or not affected.

## Workstream 3: OSV/RustSec affected/fixed range fixture hardening

### Problem

The new applicability implementation needs source-specific fixtures. Simple synthetic `VulnerabilityMetadata` tests are useful but not enough to catch source parsing mistakes.

### Required changes

Add fixtures for:

- OSV `ranges` with `introduced` and `fixed` events.
- OSV explicit affected `versions` list.
- OSV multiple affected packages.
- RustSec patched/unaffected ranges as represented in the repo’s metadata model.
- Unsupported OSV `GIT` range, which should return `Unknown` unless commit comparison is explicitly implemented.

### Tests

For each fixture:

- Extract advisory ranges.
- Evaluate a vulnerable version.
- Evaluate a fixed version.
- Evaluate a version outside the affected interval.
- Assert warning/reason behavior for unsupported ranges.

### Acceptance criteria

- Fixture tests cover real source shapes, not only synthetic range strings.
- Unsupported source semantics are conservative.

## Workstream 4: Dependency parser confidence and line mapping audit

### Problem

`dependency_parse.rs` is large and newly added. It should be audited for parser confidence and source-line behavior before security applicability is considered reliable.

### Required checks

Audit parser behavior for:

- `Cargo.lock` exact versions.
- `package-lock.json` exact versions.
- `go.mod` module requirements.
- Maven POM group/artifact/version.
- NuGet `.csproj` PackageReference.
- Gemfile.lock.
- composer.lock.
- Dockerfile `FROM` references.
- GitHub Actions `uses:` references.

For each parser:

- Exact lockfile entries should be high confidence.
- Manifest pinned versions should be medium/high depending on ecosystem.
- Version ranges should not be treated as exact resolved versions.
- Missing versions should remain `None` and not become `latest`.
- Line numbers should point to the dependency entry where feasible.

### Tests

Add at least one malformed fixture per parser family:

- Invalid JSON lock file.
- XML with missing version.
- YAML/workflow with invalid `uses` value.
- Dockerfile with variable tag.

Expected result: warnings or low-confidence findings, not panics.

### Acceptance criteria

- Dependency parser confidence semantics are documented and test-covered.
- Malformed input never panics.
- Version ranges are not treated as exact installed versions.

## Workstream 5: Gitea/Forgejo configured-host wiring audit

### Problem

Helper URL builders exist for Gitea/Forgejo, but configured-host behavior must be verified end to end. It is not enough for helper functions to exist if MCP request parsing, provider config, repo_fetch, repo_map, web_fetch transforms, and provider_status do not expose or use them correctly.

### Required audit points

1. Configuration:
   - There is a documented config shape for Gitea/Forgejo base URLs.
   - Base URLs are normalized and validated.
   - Arbitrary unconfigured hosts are not rewritten or fetched through code-host raw transforms.

2. `repo_fetch`:
   - Configured Gitea host can build browser/raw URLs.
   - Configured Forgejo host can build browser/raw URLs.
   - Unsupported/unconfigured host returns clear error.
   - Commit SHA/permalink handling is either implemented or explicitly documented as unsupported.

3. `web_fetch`:
   - GitHub/GitLab/Codeberg browser-to-raw transforms still work.
   - Gitea/Forgejo browser-to-raw transform requires configured host context or remains disabled with a clear warning.

4. `repo_map`:
   - Native configured Gitea/Forgejo map works if implemented.
   - If not implemented, fallback mode emits `native_repo_map_unavailable` or equivalent.

5. `provider_status`:
   - Reports Codeberg/Gitea/Forgejo capabilities accurately.
   - Does not claim native tree/search support unless actually implemented.

### Tests

- Configured Gitea `repo_fetch` URL construction and validation.
- Configured Forgejo `repo_fetch` URL construction and validation.
- Unconfigured Gitea-like URL is not rewritten by `web_fetch`.
- Codeberg transform remains enabled.
- Provider status reflects supported hosts and configured-host availability.
- Batch fetch handles configured Gitea/Forgejo locator if supported; otherwise per-item error is explicit.

### Acceptance criteria

- Helper functions are wired to real tool behavior or docs clearly mark them helper-only.
- Provider status does not overclaim.
- Tests cover configured and unconfigured host behavior.

## Workstream 6: Package resolver documentation and metadata cleanup

### Problem

At least one module-level doc still says the resolver is for crates.io, PyPI, and npm, even though new ecosystems are implemented. This is minor but misleading for maintainers and agents reading source docs.

### Required changes

Update stale docs in:

- `src/meta/package_resolver.rs` module header.
- README package ecosystem section.
- AGENTS package-aware retrieval guidance.
- Provider status examples.
- MCP tool descriptions, if they still list only the old ecosystems.

Ensure docs distinguish:

- Metadata lookup vs dependency solving.
- Deterministic fallback URL generation vs verified registry API response.
- Ecosystems with OSV mapping vs ecosystems without OSV mapping, such as OCI/GitHub Actions if unsupported by OSV in this implementation.

### Tests

If provider-status snapshots/assertions exist, update them to include the new ecosystem list and avoid stale claims.

### Acceptance criteria

- No source/module docs claim only crates.io/PyPI/npm support.
- Public docs reflect actual implemented ecosystems.
- Unsupported advisory mapping for OCI/GitHub Actions is explicit.

## Workstream 7: CI verification evidence

### Problem

The repo now has a CI workflow, but the connector did not show workflow runs or commit statuses for the latest commit. The implementation commit claims 2399 tests pass and clippy clean, but there is no visible CI evidence through the connector.

### Required changes

Audit `.github/workflows/ci.yml` and ensure it will run on:

- push to `main`;
- pull requests to `main`.

Check whether branch protection, workflow permissions, or repository settings prevent workflow execution. If the workflow was added in the same commit, verify a subsequent commit triggers it.

Consider adding a trivial docs-only commit only if necessary to trigger CI, but do not do that as part of implementation unless explicitly desired. In this pass, a normal code/doc cleanup commit should naturally trigger CI.

### CI commands

The workflow should run at minimum:

```bash
cargo fmt --check
cargo clippy --all-features --all-targets -- -D warnings
cargo test --all-features
cargo publish --dry-run
```

If `--locked` was removed intentionally, document why. Prefer `--locked` if `Cargo.lock` is committed and the crate is not blocked by local path/version quirks.

### Acceptance criteria

- CI workflow is syntactically valid.
- A commit after workflow introduction has visible CI run/status, or the absence is explained.
- Local verification commands are recorded in the final commit message.

## Workstream 8: Corpus coverage for corrected cases

### Required additions

Add corpus scenarios or targeted corpus tests for:

- Advisory range boundary: `>= 2.0.0, < 3.0.0` with versions below, inside, and at fixed boundary.
- Unknown range syntax returning `unknown`.
- Unsupported OSV/GIT range returning `unknown`.
- Gitea/Forgejo configured host support or explicit unsupported warning.
- Package resolver fallback for one new ecosystem such as Maven or NuGet.

### Acceptance criteria

- Corpus would have caught the inverted `>=` bug.
- Corpus verifies conservative unknown behavior.
- Corpus validates new-host capability claims.

## Final acceptance checklist

- [ ] `>=`, `>`, `<=`, `<`, and `=` range comparisons are correct.
- [ ] Comma-separated range clauses require all clauses to pass.
- [ ] Unknown range syntax produces `Unknown`, not affected/not affected.
- [ ] Internal applicability evaluation preserves affected/not_affected/unknown.
- [ ] False-negative prevention tests exist for advisory applicability.
- [ ] OSV/RustSec fixtures cover introduced/fixed and unsupported range forms.
- [ ] Dependency parser confidence and malformed input behavior are test-covered.
- [ ] Configured Gitea/Forgejo support is wired end to end or docs/provider status clearly mark it unsupported.
- [ ] Package resolver docs list the actual supported ecosystems.
- [ ] Provider status does not overclaim package/security/code-host capabilities.
- [ ] Corpus includes corrected advisory boundary cases.
- [ ] CI workflow runs or missing CI visibility is explained.
- [ ] `cargo fmt --check`, clippy, and tests pass.
