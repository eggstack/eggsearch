# M006 Intent-Aware Provider Routing — Closure Status

Status: conditionally closed

Source plan: `plans/implementation/agentic-retrieval-integrity/006-intent-aware-provider-routing.md`

Roadmap: `plans/subsystems/agentic-retrieval-integrity-roadmap.md`

Planning baseline: `d751bd7471a647e651ee41b8e51199c66d04d01a`

Implementation candidate: uncommitted worktree changes on top of the baseline above. Status stays
`conditionally closed` until the single closure commit carries this record, the roadmap status, and
the registry row together **and** the named condition below is discharged.

## Named condition

`[search.routing].intent_mode` ships **`off`**. The roadmap's exit boundary for this milestone is
that default routing includes intent-appropriate providers, which requires `apply`. That flip is
withheld because the tier-1 snapshot lane does not measure the shipped default provider set, so the
default change is unmeasured. The gate test records `observe_only` with
`lane_measures_shipped_default_path = false`.

This is a recorded evidence condition, not an implementation gap: the routing machinery, its
evidence, and its fail-closed behaviour are all in place and tested.

## Result

`src/meta/routing.rs` holds the intent→provider table as data. Routing only **adds** providers.

| Derived intent | Preference order | Capability predicate per entry |
|---|---|---|
| `web`, `docs` | none | — (`docs` has no capability flag; deferred) |
| `code` | `github_code`, `gitlab_code`, `gitea_code`, `sourcegraph` | code-search or repository-filter support |
| `issues` | `github_issues`, `gitlab_issues`, `gitea_issues`, `firecrawl_developer` | issue-search support |
| `releases` | `github_releases`, `gitlab_releases`, `gitea_releases` | release-search support |
| `security` | `osv`, `github_advisory`, `rustsec`, `nvd` | security-search support |
| `news` | `tavily`, `brave_api` | news support |

Precedence is fixed: explicit `providers` argument → supplied `profile` → default providers plus
intent additions → existing cooldown and health filter. No provider is ever removed, reordered, or
displaced, so a caller that names its providers gets exactly what it asked for.

A candidate is admitted only when enabled in `[search.providers]`, actually built into the engine
set, not cooling down, and satisfying the table row's predicate. Each disposition is reported:
`added`, `already_selected`, `not_enabled`, `not_built`, `cooling_down`, `capability_missing`,
`cap_exceeded`.

Each table entry must satisfy the same predicate its warning uses. That is enforced by test, which
is what stops routing from hiding a genuine capability gap. Exclusions surface as the new
`intent_routing_capability_excluded` warning (wire code) rather than disappearing.

## Configuration

`[search.routing]` in `src/core/config/routing.rs` (extracted because `src/core/config.rs` is under
a ratchet):

| Field | Default | Bounds | Error |
|---|---|---|---|
| `intent_mode` | `off` | `off`, `apply` | ``invalid intent_mode `x`; valid values: off, apply`` |
| `max_extra_providers` | `2` | `0..=4` | names the bound |

`0` reports the decision and adds nothing. Three default sites agree:
`RoutingSection::default()`, the serde defaults, and `resolve_provider_routing` reading
`config.search.routing` with no copy.

## Snapshot evidence

`make eval-retrieval-quality` was run twice; both exits were 0 and the aggregate blocks were
byte-identical (cases 15, recall 0.9, MRR 0.5822, nDCG 0.6541, authoritative survival 0.9048,
distinct domains 4.333, duplicate slots 0, fingerprint `fnv1a64:9235b0bd8fc215b8`).

Four routing cases were authored because the pre-existing tier-1 cases carry neither specialist
provider lists nor an enabled-provider set:

| Case | recall | MRR | nDCG | survival | domains | duplicates |
|---|---:|---:|---:|---:|---:|---:|
| security routing | 0 → 1 | 0 → 0.2 | 0 → 0.3869 | 0 → 1 | 3 → 4 | 0 |
| code routing | 0 → 1 | 0 → 0.25 | 0 → 0.4307 | 0 → 1 | 3 → 4 | 0 |
| issues routing | 0 → 1 | 0 → 0.25 | 0 → 0.4307 | 0 → 1 | 3 → 4 | 0 |
| docs (control) | unchanged | unchanged | unchanged | unchanged | unchanged | 0 |

The negative control `security-nvd-first` is detected (recall drop to 1.0) and reproduces across two
runs, which is what makes the lane a real gate rather than a rubber stamp. The eleven pre-existing
cases are byte-identical between replays.

## Why the flip is withheld

The lane's default-provider sets are bespoke (`duckduckgo/startpage` or `duckduckgo/yahoo`), not the
shipped `duckduckgo/brave/yahoo`. Routing additions only matter when the addition can enter the
candidate set, so a lane that does not model the shipped set cannot measure the shipped default.
Authoring shipped-set cases is the outstanding work.

## Verification

- `cargo fmt --all`: clean.
- `cargo clippy --locked --all-targets --all-features -- -D warnings`: clean.
- `cargo test --locked --all-features --test provider_routing`: 97 passed (9 new).
- `cargo test --locked --all-features --test retrieval_quality`: 16 passed, 2 ignored (3 new).
- `cargo test --locked --all-features --test config_validation`: 86 passed (6 new).
- `cargo test --locked --all-features --test web_search_integration`: 78 passed (6 new, including
  the attempt-ledger and warning-matrix coverage).
- `cargo test --locked --all-features --test static_guards`: 61 passed.
- Full `cargo test --locked --all-features`: 6,057 passed, 0 failed, 85 binaries.

## Deviations

- **Two modules were split to satisfy ratchets rather than raising a ceiling.**
  `src/meta/provider_diagnostics.rs` moved its routing decision into
  `provider_diagnostics/decisions.rs`, and `src/core/warning.rs` moved new codes into
  `src/core/warning/prefixes.rs`. Both are the ratchet's intended remedy.

## Residual risk

- Routing is lexical at its root: it inherits the classifier's vocabulary limits. A query that
  derives `web` adds nothing, so the change can only help queries with signal.
- Adding a provider costs a network call. `max_extra_providers = 2` bounds the worst case, and the
  existing cooldown filter still applies.
- The `docs` intent has no routing entry because no provider declares a documentation capability;
  adding one later means adding both the flag and the table row, with the agreement test enforcing
  consistency.
