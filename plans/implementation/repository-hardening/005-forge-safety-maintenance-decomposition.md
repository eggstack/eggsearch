# Plan 005 — Forge Safety Ownership Maintenance Decomposition

Status: blocked

Source roadmap:
`plans/subsystems/repository-hardening-roadmap.md`

Milestone: M005 forge safety ownership decomposition

Primary class: maintenance

Planning baseline: `c49c600b76e690bb1bc52f641554cca6bf79f36f`

Hard dependency:

- M004 must close first so process/platform helper movement is complete and this
  milestone does not combine unrelated cross-cutting refactors.

Relevant long-term requirements:

- `plans/000-long-term-specification.md#4`
- `plans/000-long-term-specification.md#9`

## Objective

Decompose the current high-density forge adapter so host-independent URL,
address, SSRF, redirect, and response-budget policy has one small auditable
owner while GitHub/GitLab/Gitea/Forgejo/Codeberg request mechanics live behind
that owner.

This is a behavior-preserving maintenance change. It must improve review/change
locality without creating per-host security-policy forks.

## Current evidence

`src/meta/forge_adapter.rs` is roughly 3,000 lines / 100 KB and currently
contains both:

- shared safety machinery: base-URL validation, DNS/address classification,
  private/loopback rejection, response/read budgets, error previews, URL
  helpers;
- host-specific tree/default-branch/commit/contents execution and normalization.

`architecture/maintenance.md` already identifies this exact next slice and
keeps an explicit file-size ratchet exception pending decomposition.

`tests/static_guards.rs` also reads `src/meta/forge_adapter.rs` directly to
enforce bounded-body and immutable-permalink rules, so decomposition must move
those guards with ownership rather than weaken them.

## Invariants

- Supported forge host set and native provider IDs remain unchanged.
- URL credentials, unsupported schemes, loopback/private/link-local/multicast
  addresses, and disallowed redirects remain fail-closed.
- DNS validation behavior and address-classification coverage are unchanged.
- Forge body reads remain bounded through the shared budget owner.
- `commit_sha` continues to come from resolved commit/ref identity, never an
  entry object SHA.
- Deterministic entry ordering/truncation and source-card behavior remain
  unchanged.
- No host-specific module may bypass shared policy by constructing an
  independent HTTP client.

## Non-goals

- Adding a new forge/provider.
- Changing API endpoints or authentication semantics.
- Improving ranking/search result quality.
- Replacing eggfetch.
- Genericizing repo/research/security workflows.
- Simultaneously splitting `core/config.rs`, `core/security.rs`, or local
  workspace modules.

## Required work

### 1. Lock behavior before moving code

Run and, where needed, expand focused tests covering:

- base URL validation;
- all IPv4/IPv6 address classes and mapped/compatible forms;
- DNS resolution rejection;
- credential/scheme rejection;
- redirect rejection;
- aggregate/per-response read budgets;
- default branch and commit resolution;
- URL/permalink construction using commit SHA;
- depth/entry truncation;
- GitHub/GitLab/Gitea/Forgejo/Codeberg parity.

Do not start module movement while an existing safety test is failing.

### 2. Establish the module boundary

Convert the current forge implementation to a directory-backed module while
preserving the existing Rust path `crate::meta::forge_adapter` for internal
callers/tests.

A target decomposition should separate responsibilities similar to:

```text
src/meta/forge_adapter/
  mod.rs          # facade/orchestration and stable internal exports
  policy.rs       # endpoint/address/DNS/redirect/security policy
  budget.rs       # bounded response/read telemetry
  urls.rs         # immutable browser/raw URL construction
  github.rs       # GitHub execution/normalization
  gitlab.rs       # GitLab execution/normalization
  gitea.rs        # Gitea/Forgejo/Codeberg execution/normalization
```

Exact filenames may vary after code inspection, but policy/budget ownership must
not be duplicated.

### 3. Keep one transport/policy construction seam

Host modules receive a validated endpoint/policy/client context; they do not
construct looser clients or reimplement address classification.

Static guards should fail if host-specific modules introduce:

- bare unbounded `.text()`, `.bytes()`, or `.json()`;
- direct private-address allowlists;
- a second redirect policy;
- entry object SHA use in commit permalink construction.

### 4. Replace file-size exception with ratchets on new owners

Update `orchestration_module_size_ratchet` / maintenance tables so the old
single-file ceiling cannot reappear.

The facade and policy modules should have substantially lower ceilings than the
current ~3,050-line exception. Host modules receive individual ceilings based on
their moved baseline, not arbitrary tiny targets.

Do not split functions solely to hit a LOC number.

### 5. Documentation

Update:

- `architecture/maintenance.md` ownership/ratchet table;
- `architecture/security.md` forge policy owner;
- `architecture/fetch.md` if transport-owner diagrams name the old file;
- static-guard descriptions.

## Failure and recovery semantics

No error class should change as a side effect of module movement. Provider
failure, timeout, capability skip, read-budget exhaustion, and URL-policy
failure must retain their current observable mapping.

If extracting policy requires changing behavior to make an interface fit, stop
and separate the semantic change into a corrective plan.

## Focused verification

At minimum:

```bash
cargo test --locked --all-features --test forge_adapter
cargo test --locked --all-features --test property_forge_url
cargo test --locked --all-features --test native_forge_smoke
cargo test --locked --all-features --test static_guards
cargo test --locked --features mock --test provider_workstream_regression
make check
make docs-check
make bench-check
```

Run ignored native forge smoke tests only when credentials/network policy allow;
their absence is not a substitute for deterministic unit/property coverage.

## Acceptance criteria

- Shared forge safety policy has one explicit module owner.
- Host modules cannot bypass common address/redirect/read-budget policy.
- Existing `crate::meta::forge_adapter` call paths remain compatible.
- Static guards follow the new ownership and catch the same or stronger classes
  of regression.
- The old ~3,000-line single-file exception is retired in favor of bounded
  module ratchets.
- All deterministic forge/provider/property tests remain behaviorally green.

## Stop conditions

Stop if:

- a host requires a genuinely different security policy that cannot be modeled
  through current typed policy;
- moving code changes result ordering, immutable identity, or provider outcome
  semantics;
- M004 is not closed;
- the refactor grows a new generic transport abstraction duplicating eggfetch.

## Closure evidence required

Create `plans/closure/repository-hardening/005-status.md` containing:

- before/after file/LOC responsibility map;
- shared policy ownership proof;
- static-guard updates;
- host parity and security test matrix;
- full verification and exact candidate SHA.
