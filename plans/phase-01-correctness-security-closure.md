# Phase 1: Correctness and Security Closure

Status: ready for implementation
Depends on: none
Blocks: all subsequent roadmap phases
Primary goal: restore a trustworthy baseline by closing every known invariant violation before adding new retrieval capabilities.

## 1. Scope

This phase addresses six confirmed defects and one release-gate drift:

1. The prefetched first response chunk can bypass `max_bytes`.
2. `allow_localhost` does not work independently from `allow_private_network`.
3. `include_hidden` unintentionally disables `SKIP_DIRS` rejection for direct local fetch.
4. `follow_symlinks = false` does not reject symlinks in intermediate path components.
5. `max_indexed_files` does not terminate local traversal globally.
6. Multi-provider deadline telemetry marks a subquery complete after its first terminal provider job.
7. `docs_safety_vocabulary` is documented but omitted from local and remote release gates.

No new provider, index, or user-visible workflow feature should be introduced in this phase.

## 2. Required Invariants

- `body.len()` never exceeds `FetchLimits.max_bytes` at any point in the fetch pipeline.
- Localhost policy and private-network policy are independent and cover literal and DNS-resolved addresses identically.
- Hidden-component policy and skipped-directory policy are independently configurable.
- `follow_symlinks = false` means no traversed path component may be a symlink.
- Local scanning performs at most `max_indexed_files` file considerations across all configured roots.
- Deadline telemetry reports partial completion whenever planned work remains nonterminal.
- The documented local release gate and GitHub Actions execute the same documentation-contract inventory.

## 3. Workstream A: Fetch Body Bound

### Tasks

1. Introduce a single bounded append helper in `src/fetch/client.rs` or an adjacent internal module.
2. Route the PDF magic-prefetch chunk and every subsequent stream chunk through that helper.
3. Define exact semantics when a chunk exceeds the remaining budget:
   - append only the remaining bytes;
   - set `truncated = true`;
   - stop consuming the body;
   - never allocate based on attacker-controlled chunk length beyond the received chunk object itself.
4. Ensure `bytes_read`, truncation fields, and document metadata describe the retained bytes consistently.
5. Review metadata-only PDF behavior so it cannot bypass the same body cap.

### Tests

- No `Content-Length`, first chunk smaller than cap.
- No `Content-Length`, first chunk exactly equal to cap.
- No `Content-Length`, first chunk larger than cap.
- Prefetched first chunk plus later chunk crossing the cap.
- Honest `Content-Length` larger than cap still exits early.
- Text, HTML, and PDF-sniff paths all use the same cap semantics.

### Acceptance

A regression test must demonstrate that pre-fix code retains more than `max_bytes`, while post-fix code never does.

## 4. Workstream B: Network Policy Matrix

### Tasks

1. Replace the current overlapping boolean checks with an address classification model, for example:
   - `Loopback`;
   - `Private`;
   - `LinkLocal`;
   - `CarrierGradeNat`;
   - `Documentation`;
   - `Reserved`;
   - `Multicast`;
   - `Public`.
2. Apply policy after classification:
   - loopback allowed only by `allow_localhost`;
   - other nonpublic ranges allowed only by `allow_private_network`;
   - public addresses always allowed.
3. Apply the same classifier to IPv4 literals, IPv6 literals, IPv4-mapped IPv6, and DNS-resolved addresses.
4. Preserve DNS answer pinning.
5. Update safety documentation with a four-state policy table.
6. Ensure capability diagnostics expose the active escape hatches accurately.

### Tests

Test all four combinations of the two booleans against:

- `127.0.0.1`;
- `::1`;
- DNS name resolving to loopback;
- `10.0.0.1`;
- `192.168.1.1`;
- `169.254.169.254`;
- CGNAT;
- documentation ranges;
- public IPv4 and IPv6.

### Acceptance

`allow_localhost = true` with `allow_private_network = false` permits loopback and rejects RFC1918/private ranges.

## 5. Workstream C: Local Path Policy

### Tasks

1. Move `SKIP_DIRS` checks outside the `include_hidden` condition.
2. Keep hidden-component rejection controlled exclusively by `include_hidden`.
3. Decide whether direct fetch should ever permit skipped directories. Default answer: no.
4. If an override is required, add a separate narrowly named configuration field rather than overloading `include_hidden`.
5. Walk every existing path component with `symlink_metadata` when `follow_symlinks = false`.
6. Preserve final canonical containment checks even after component-level validation.
7. Ensure search, fetch, map, security dependency-file reads, and batch workspace fetch share the same path-policy primitive.

### Tests

- Hidden file accepted when `include_hidden = true`.
- `.git/config`, `target/...`, and `node_modules/...` rejected regardless of `include_hidden`.
- Final-component symlink rejected.
- Intermediate-directory symlink rejected.
- Symlink inside the root accepted only when `follow_symlinks = true` and canonical target remains within root.
- Escaping symlink rejected under every configuration.

### Acceptance

Direct fetch cannot access any path that repository search intentionally excludes as a skipped directory, unless a future explicit override is added and documented.

## 6. Workstream D: Global Local-Scan Bound

### Tasks

1. Replace the boolean stop return with an explicit control enum or propagate a stop flag through every recursion level.
2. Check the bound before incrementing the counter so `files_scanned` never exceeds the configured limit.
3. Stop traversing remaining siblings, parent frames, and subsequent roots immediately after the limit is reached.
4. Preserve `truncated = true` and ensure timeout and truncation can both be reported if applicable.
5. Add deterministic directory ordering if filesystem enumeration currently affects which files are considered before the cap.

### Tests

- Deep nested tree exceeding the cap.
- Wide sibling tree exceeding the cap.
- Multiple configured roots sharing one global cap.
- Exact-cap tree.
- Cap of one.
- Timeout and cap reached near-simultaneously.

### Acceptance

`files_scanned <= max_indexed_files` for every result, and no additional directory traversal occurs after the stop condition is raised.

## 7. Workstream E: Deadline Completeness Accounting

### Tasks

1. Track planned, running, terminal-success, terminal-failure, pending, and cancelled job counts per subquery.
2. Define subquery states:
   - complete;
   - partially_complete;
   - skipped;
   - interrupted.
3. Preserve existing aggregate fields where compatibility requires them, but add precise telemetry fields additively.
4. Do not mark a subquery complete until all planned jobs are terminal.
5. Ensure panic and join-error paths decrement provider and subquery counts correctly.
6. Add structured warnings scoped to affected subqueries or providers where possible.

### Tests

- One fast and one slow provider for the same subquery.
- One success and one failure before deadline.
- One success and one timeout at deadline.
- Entirely skipped subquery.
- Panic path.
- Several subqueries with mixed complete and partial states.

### Acceptance

A subquery with one completed provider and one cancelled provider is reported as partial/interrupted, not complete.

## 8. Workstream F: Release-Gate Parity

### Tasks

1. Add `docs_safety_vocabulary` to `make docs-tests`.
2. Add the same binary to the GitHub Actions docs-contract job.
3. Update testing documentation only if the actual inventory changes.
4. Add a small contract test or script ensuring Makefile and CI documentation-test inventories remain aligned.
5. Run the complete feature matrix with `--locked` consistently where intended.

### Acceptance

The local `make check` gate and GitHub Actions run the complete documented contract suite, including safety vocabulary checks.

## 9. Verification Sequence

1. Add failing regression tests for each defect.
2. Implement one workstream at a time.
3. Run targeted tests after each workstream.
4. Run:
   - `cargo fmt --check`;
   - `cargo clippy --all-targets --all-features -- -D warnings`;
   - all feature-matrix tests;
   - schema corpus;
   - documentation contracts;
   - release build;
   - docs build;
   - publish dry-run.
5. Review documentation and provider-status capability output against actual behavior.

## 10. Definition of Done

- All seven items are fixed.
- Every fix has a negative regression test.
- No public contract is silently removed or renamed.
- Safety documentation includes the corrected network policy matrix.
- Local path behavior is consistent across all workspace-capable tools.
- Deadline telemetry accurately represents partial work.
- Full release gate passes from a clean checkout.

## 11. Handoff Notes

Prefer small commits grouped by workstream. Do not combine the network-policy refactor with unrelated fetch rendering changes. If an existing contract test prevents the correct behavior, update the contract explicitly and document why rather than preserving a known defect.