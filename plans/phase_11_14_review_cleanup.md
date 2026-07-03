# Phase 11–14 Review Cleanup Plan

## Purpose

This plan closes the remaining narrow review items after the phase 11–14 implementation pass. The repo is now in strong shape: fetch/local hardening, performance benchmarks, regression corpora, and codegg integration documentation have all landed. The remaining work is targeted cleanup and verification, not another feature expansion.

Primary cleanup targets:

1. Re-review dispatch pending-queue behavior after the phase 12 `swap_remove` optimization.
2. Make CI status visibility concrete and ensure the intended workflows run.
3. Document and test the intentionally conservative local path rejection policy.
4. Verify Criterion remains dev-only and does not affect runtime/minimal builds.
5. Keep `--no-default-features` behavior stable and represented in CI/docs.

## Current state summary

Recent implementation commits added:

- Phase 11: `CodeSpanEvidence` linking fields, centralized local fetch path validation, symlink policy enforcement, web-fetch stable IDs.
- Phase 12: Criterion benchmark harness, CI feature matrix, dispatch/local/warning performance optimizations, feature/deployment docs.
- Phase 13: six regression suites covering schemas, identity, warnings/reason codes, fetch safety, security, research, recipes, and evidence bundle handoff.
- Phase 14: codegg integration guide and MCP response handling contract.

The review posture is now to preserve correctness and contract stability while removing small ambiguity.

## Non-goals

- Do not add another major response model.
- Do not remove phase 11–14 features.
- Do not weaken local path safety for convenience.
- Do not add live-network requirements to normal CI.
- Do not add codegg as a dependency.
- Do not convert benchmarks into required pass/fail gates unless stable thresholds are available.

## Workstream 1: Dispatch ordering audit after `swap_remove`

### Problem

Earlier phases deliberately replaced `swap_remove` in bounded dispatch because it can perturb pending queue order and weaken priority semantics. Phase 12 reports using `swap_remove` again as an optimization. This needs explicit audit.

### Required review

Inspect `src/meta/dispatch.rs` and determine whether `swap_remove` is used in a context where pending order matters.

Questions to answer:

- Is the pending queue sorted by priority/subquery/provider order?
- Does the scheduler scan pending jobs in priority order?
- Does removing a job with `swap_remove` allow a lower-priority job to move earlier and run before a higher-priority job?
- Are final outputs sorted only after execution, or is execution priority itself guaranteed?
- Are per-provider capacity bypasses handled without corrupting global order?

### Acceptable outcomes

Use one of these outcomes:

1. **Ordering matters and `swap_remove` is unsafe.** Replace with `Vec::remove(i)`, `VecDeque`, or another stable removal strategy. Add tests.
2. **Ordering no longer matters by design.** Document the new scheduler semantics and ensure tests assert only the actual contract.
3. **`swap_remove` is safe because the queue is re-sorted or index-scanned correctly.** Add a focused test proving priority order is preserved after multiple removals.

### Required tests

Add or strengthen tests for:

- higher-priority jobs start before lower-priority jobs when capacity allows;
- provider-capacity blocking does not permanently reorder unrelated jobs incorrectly;
- repeated removals from the middle preserve the intended scheduler contract;
- final output ordering remains deterministic;
- deadline skipped/interrupted telemetry remains correct.

### Acceptance criteria

- Scheduler priority semantics are explicit in comments/tests.
- `swap_remove` remains only if a test demonstrates it is safe for the actual contract.
- No regression to spawn-all behavior.

## Workstream 2: CI workflow visibility and correctness

### Problem

The workflow file exists, but connector checks returned no combined statuses or workflow runs for the observed head. This may be a connector limitation, a branch/workflow trigger timing issue, or CI not actually running.

### Required review

Verify GitHub Actions behavior in the repo UI or via API:

- Does `.github/workflows/ci.yml` appear under Actions?
- Did it trigger on the latest push to `main`?
- Are workflow runs green, red, skipped, or disabled?
- Are repository Actions enabled?
- Are workflow permissions sufficient?
- Does the connector only report PR-triggered runs, hiding push runs?

### Required cleanup

- If CI is not running, fix the workflow trigger or repository workflow location.
- If CI is running but connector cannot see push runs, document that limitation in handoff notes.
- If CI is failing, fix the failing job or split unstable jobs out of required CI.
- Keep live smoke tests ignored/opt-in.

### CI matrix expectations

The default CI should cover:

- `cargo fmt --check`
- `cargo clippy --all-features -- -D warnings` or stricter `--all-targets` if feasible
- `cargo test --all-features`
- `cargo test --no-default-features`
- `cargo check --features mock`
- `cargo check --features pdf`
- schema/corpus regression suite
- release build or publish dry-run if stable enough

### Acceptance criteria

- CI behavior is verified, not inferred from commit messages.
- Push/PR triggers are documented.
- Normal CI remains offline and deterministic.
- Any omitted command has a documented reason.

## Workstream 3: Conservative local path rejection policy

### Problem

`validate_local_fetch_path()` currently rejects any requested path containing `..`. This is intentionally conservative and security-positive, but it also rejects benign filenames containing `..` as literal characters. The policy should be deliberate, documented, and tested.

### Required decision

Choose one policy:

1. **Keep conservative substring rejection.** Document that local workspace fetch forbids `..` anywhere in the requested relative path for simplicity and security.
2. **Switch to path-component rejection.** Reject only actual parent-directory components via `Path::components()` while allowing filenames like `notes..draft.md`.

Recommended outcome: keep conservative substring rejection unless there is a concrete user need for `..` in filenames. The local fetch surface is security-sensitive and operator-controlled; false rejection is safer than path ambiguity.

### Required tests

If keeping conservative policy:

- `../secret.txt` rejected.
- `a/../../secret.txt` rejected.
- `notes..draft.md` rejected with the same path traversal error.
- docs explicitly mention this behavior.

If switching to component policy:

- `ParentDir` components rejected.
- `notes..draft.md` accepted if under root and file exists.
- encoded traversal cases still rejected if decoding occurs anywhere.

### Acceptance criteria

- The path policy is documented in README/AGENTS/codegg integration docs.
- Tests prove the chosen behavior.
- Error messages do not imply support for edge cases that are rejected.

## Workstream 4: Criterion and dependency isolation audit

### Problem

Criterion was added for benchmarks. It is acceptable as a dev dependency, but it pulls additional crates into `Cargo.lock`. Ensure it does not affect runtime builds, minimal builds, or compile times for normal users beyond dev/test contexts.

### Required review

- Confirm `criterion` is under `[dev-dependencies]`, not `[dependencies]`.
- Confirm `cargo tree --release` or equivalent runtime tree does not include Criterion.
- Confirm `cargo build --release --no-default-features` does not compile Criterion.
- Confirm `cargo test --no-default-features` does not unexpectedly require benchmark-only features.
- Confirm docs describe benchmarks as dev-only.

### Optional cleanup

If Criterion still feels too heavy:

- Gate benchmark dependencies behind a `bench` feature if practical.
- Replace Criterion with a lightweight internal benchmark binary.
- Keep Criterion but document the lockfile impact.

### Acceptance criteria

- Runtime dependency tree is not polluted by benchmark-only crates.
- Minimal builds remain viable.
- Benchmark docs are accurate.

## Workstream 5: No-default-features stability

### Problem

The repo now relies on multiple features (`mock`, `pdf`, local/fetch/security/research paths, etc.). `--no-default-features` should continue to compile and test a meaningful minimal server surface.

### Required behavior

- `cargo check --no-default-features` passes.
- `cargo test --no-default-features` passes.
- CI runs at least check and test for no-default-features.
- Tests that require optional features are gated correctly.
- Docs define what is available in minimal mode.

### Tests/review points

- Ensure schema/contract tests do not assume optional PDF/fetch behavior in no-default mode.
- Ensure public docs do not claim disabled features are available in minimal mode.
- Ensure `provider_status` accurately reports not-built/unavailable optional capabilities.

### Acceptance criteria

- Minimal mode remains a first-class build target.
- Optional feature availability is reflected in provider/tool capability output.

## Workstream 6: Security remediation text-safety refinement

### Problem

`SecurityRemediation.validate_text_safety()` uses a broad exploit keyword blocklist. This is useful, but terms like `sql injection`, `xss`, `rce`, or `remote code execution` can appear in legitimate defensive remediation text. If the validator blocks rather than warns, it may suppress valid guidance.

### Required review

- Determine whether `validate_text_safety()` is advisory or blocking.
- Confirm generated remediation text uses defensive phrasing and does not include exploit instructions.
- Add tests for defensive vulnerability naming such as `upgrade to fix RCE advisory` if that wording should be allowed, or document why it is flagged.
- Consider splitting terms into severity classes:
  - `offensive_instruction_terms`: payload, shellcode, heap spray, ROP chain, exploit steps.
  - `vulnerability_class_terms`: XSS, SQL injection, RCE, overflow.

### Recommended behavior

- Offensive-instruction terms should be hard warnings/errors.
- Vulnerability-class terms should be allowed in defensive context, or produce a low-severity notice rather than blocking.
- The validator should check phrases/intent where possible, not only substrings.

### Acceptance criteria

- Defensive remediation can name vulnerability classes without being incorrectly blocked, if that is the chosen policy.
- Exploit-procedural text remains flagged.
- Tests cover both categories.

## Workstream 7: Final verification pass

Run and record:

```bash
cargo fmt --check
cargo clippy --all-features -- -D warnings
cargo test --all-features
cargo test --no-default-features
cargo check --features mock
cargo check --features pdf
cargo test --features mock --test schema_identity_registry --test fetch_safety --test security_applicability_corpus --test research_evidence_corpus --test recipes_next_actions --test evidence_bundle_handoff
cargo bench --no-run
```

If any command is intentionally omitted or unavailable, document why.

## Suggested commit structure

1. `test(dispatch): lock scheduler priority semantics after optimization`
2. `docs(local): document conservative workspace path policy`
3. `test(local): cover conservative dotdot path rejection`
4. `ci: verify no-default and schema corpus workflow visibility`
5. `docs(perf): document criterion dev-only dependency isolation`
6. `fix(security): refine remediation text-safety categories`
7. `docs: record final phase 11-14 cleanup verification`

## Completion checklist

- [ ] Dispatch scheduler priority/order semantics are tested after `swap_remove` review.
- [ ] CI workflow behavior is verified in Actions or documented if connector visibility is limited.
- [ ] Conservative `..` path policy is documented and tested, or replaced with component-aware validation.
- [ ] Criterion is confirmed dev-only and absent from runtime/minimal builds.
- [ ] `cargo test --no-default-features` is verified and represented in CI.
- [ ] Security remediation text safety distinguishes exploit instructions from defensive vulnerability naming.
- [ ] Final offline verification commands are run and recorded.
