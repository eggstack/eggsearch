# Plan 007 — Parser Budgets, Diagnostics, Fuzzing, and Adversarial Qualification

Status: implementation plan

Source roadmap:
`plans/subsystems/dependency-evidence-hardening-roadmap.md`

Milestone: M007

Primary class: invariant + infrastructure + polish

Baseline for planning: `e0dab301cc04bf5e1e232177e2e2647262a1fdd7`

Hard dependencies: M001-M006 closed.

## Objective

Close the dependency-parser hardening workstream with deterministic aggregate
resource limits, user-visible parse completeness diagnostics, property/fuzz
coverage of the production dispatcher, realistic multi-ecosystem regression
fixtures, and exact-candidate qualification.

## Current implementation evidence

- Individual dependency files are already root-contained and capped at 1 MiB.
- `SecuritySearchRequest::validate()` does not currently impose a dedicated
  dependency-file count cap.
- Parser findings are accumulated without a per-file or aggregate finding cap.
- Applicability work scales with advisory records times dependency findings.
- `parse_dependency_file()` historically returned only a Vec, conflating
  malformed/unsupported/empty cases.
- The existing cargo-fuzz inventory covers URL, extraction, sanitization,
  workflow/retrieval, and chunking boundaries but no dependency parser.

M001 must have introduced the parse-result/diagnostic seam. This milestone
completes its budget and observability policy.

## Required production changes

### 1. Dependency parser budget type

Add one small typed budget/configuration object with explicit limits for:

- dependency files per security request;
- findings per file;
- aggregate findings per request;
- parser nesting/depth only where a format parser needs it;
- diagnostic count/message length.

Choose defaults from realistic fixture measurements and existing security
request limits. Limits must be constants/configured policy with tests, not
magic values spread across parsers.

### 2. Deterministic truncation

Apply limits in stable source order. When a limit is reached:

- stop producing new findings deterministically;
- preserve already-produced evidence;
- mark the parse/request partial;
- emit a stable machine-readable warning code with the limit type;
- never report the truncated result as complete.

Do not use hash iteration order as a truncation boundary.

### 3. Aggregate applicability complexity

Ensure security applicability does not perform avoidable repeated work after
the finding cap. Build deterministic keyed indexes by ecosystem/canonical
package identity if useful, while preserving assessment order and dedup
semantics.

Characterize before/after with representative large synthetic inventories;
do not add a brittle CI timing threshold.

### 4. Diagnostic surfacing

Map parse reports into existing `structured_warnings`/search warnings with
stable codes for at least:

- dependency parse malformed;
- dependency format/version unsupported;
- dependency parse partial;
- dependency file/finding budget exceeded;
- weak evidence ignored for exact applicability.

Avoid leaking arbitrary file content in warnings. File paths may be reported
only through the already-authorized local path representation and should be
bounded/sanitized.

### 5. Property tests

Add a dedicated property-test suite for dependency parsing. Properties include:

- no panic for arbitrary UTF-8;
- deterministic report/findings;
- findings never exceed configured budgets;
- no empty package identity;
- source line is within input line count when present;
- requirement/reference/integrity evidence never sets the exact resolved field;
- canonicalization is idempotent for ecosystems that define it;
- dedup/order is stable.

### 6. Fuzz target

Add a production-boundary fuzz target that feeds a bounded filename/format
selector plus arbitrary text through the dependency parse-report seam.

The target must assert no panic, deterministic structural invariants, budget
bounds, and valid source-line positions. Keep fuzz-only dependencies out of
the runtime graph.

Add the target to `fuzz/Cargo.toml`; decide whether it belongs in
`make fuzz-smoke` based on runtime and criticality. If added to smoke, keep
the smoke set practical and update documentation.

### 7. Realistic fixture corpus

Add checked-in minimized fixtures generated or modeled directly from current
package-manager formats for every supported ecosystem/version covered by
M002-M006. Include adversarial cases discovered in the audit.

Add at least one multi-step security corpus scenario proving:

- weak/partial dependency evidence cannot produce a false safe verdict;
- one malformed dependency file does not erase valid findings from another;
- truncation is visible as partial evidence, not absence.

### 8. Dependency and binary footprint qualification

Record the final production dependency graph and release-binary size against
the pre-workstream baseline. Any XML/YAML/PEP parser added during earlier
milestones must be explicitly accounted for.

Run `make bench-check` as characterization and report meaningful parser
hotspots if observed.

## Required tests

Focused tests should cover every warning code and each budget boundary at
below/exactly/above limit values.

Update:

- `tests/security_applicability_contract.rs`;
- `tests/security_applicability_regression.rs`;
- `tests/security_applicability_corpus.rs`;
- property-test ownership according to `architecture/testing.md`;
- fuzz inventory and seeds according to `architecture/hardening.md`.

## Verification

Run against the exact closure candidate:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --locked --features mock --test security_workflow
cargo test --locked --features mock --test security_applicability_contract
cargo test --locked --features mock --test security_applicability_regression
cargo test --locked --features mock --test security_applicability_corpus
make fuzz-smoke
make bench-check
make check
```

Also run the new dependency fuzz target for at least a short explicit campaign:

```bash
cargo +nightly fuzz run <dependency-target> -- -max_total_time=60
```

Use the final target name chosen in implementation and record it in the closure
record.

## Documentation updates

Update:

- `architecture/security.md`;
- `architecture/hardening.md`;
- `architecture/testing.md`;
- `architecture/maintenance.md`;
- `docs/test-inventory.md`;
- `skills/eggsearch-dev/SKILL.md` if command/inventory guidance changes.

## Acceptance criteria

- Dependency file and finding work is bounded at per-file and aggregate levels.
- Truncation/unsupported/malformed states are machine-readable and cannot be
  mistaken for valid absence.
- Property tests and fuzz target cover the production parser boundary.
- Realistic fixtures cover every supported current format version named by the
  roadmap.
- Security corpus proves weak evidence cannot create false safe/affected exact
  claims.
- Final dependency graph/binary size and benchmark characterization are
  recorded.
- `make check` passes on the exact candidate.

## Stop conditions

Do not hide a parser crash by catching panics at the workflow layer; fix the
parser. Do not raise budgets simply to make a generated fixture pass. If a
format requires unbounded alias/entity expansion, constrain or reject that
feature and document the unsupported case.

## Closure evidence required

The closure record must contain:

- exact SHA;
- requirement-to-test matrix for M001-M007 invariants;
- parser warning/budget table;
- supported format/version matrix;
- fuzz target/campaign outcome;
- property test outcome;
- canonical gate outcome;
- final `cargo tree`/binary-size comparison;
- residual unsupported cases with severity;
- recommendation: closed, conditionally closed, or corrective pass required.
