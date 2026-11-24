# M001 Search-Schema Single-Sourcing — Closure Evidence

Status: closed

Source plan: `plans/archive/implementation/mcp-tool-surface-succession/001-search-schema-single-sourcing.md`

Roadmap: `plans/subsystems/mcp-tool-surface-succession-roadmap.md` M001

Decision authority: ADR-0001 (adapter ownership), ADR-0016 (fetch unification). No new ADR.

## Exact code candidate

Baseline `b2abc59687c271a6e0723bdd77e4eeeef54bdb06` on `feat/auto-20261117-044c4885`.
The implementation is the working tree on top of that baseline; no implementation commit exists yet
because the requesting instruction was "no commit unless requested". Every command below ran against
that tree.

## Change

1. `src/core/research.rs`: `RESEARCH_DOMAIN_VALUES` — one constant listing every token
   `ResearchDomain::parse` accepts, including the short aliases (`api`, `distributed`, `ml`, `infra`).
2. `src/mcp/server.rs`: `apply_search_schema_enums` runs for BOTH schema profiles, before the
   compact projection, and injects `goal` for `repo_search`, `research_search`, and
   `security_search`, plus `research_domain` for `research_search`, from the same constants the
   parsers accept. `compact_workflow_schema` and its narrower per-profile injection are deleted.
3. Compact field allow-lists moved from `server.rs::compact_schema` to per-tool
   `COMPACT_FIELDS` constants next to each argument type, dispatched by
   `src/mcp/tools/mod.rs::compact_fields`. `server.rs` keeps only the projection mechanics.
4. Descriptions that restated an enum were shortened, and `research_search.goal` no longer
   describes the repo vocabulary it never accepted.

## Measured before and after

Four search tools, input-schema-only bytes (full profile):

| Tool | Before | After |
|---|---|---|
| `web_search` | 4,118 | 3,864 |
| `repo_search` | 2,136 | 2,054 |
| `research_search` | 1,939 | 2,123 |
| `security_search` | 1,753 | 1,899 |
| total (full input-only) | 64,850 | 64,841 |

Advertisement totals at M001 completion (before M002 and M004):

| Metric | Before | After |
|---|---|---|
| full `tools/list` serialized | 72,388 | 72,379 |
| full per-tool sum | 72,368 | 72,368 |
| compact `tools/list` serialized | 19,739 | 19,871 |

The compact profile grows by 132 B because `repo_search.goal` previously advertised no enum in
either profile; that growth is the accepted cost of both profiles advertising the same accepted
values. No byte budget was raised: `search_four_schema_bytes_stay_slim` (13,100) and the total
(78,250) are unchanged and pass.

## Failing-before / passing-after

Injected `"bogus_compact_field"` into `security_search::COMPACT_FIELDS`:

```text
thread 'every_compact_field_exists_in_the_full_schema' panicked at tests/mcp_schema_slimming.rs:739:5:
compact fields missing from full schemas: ["security_search.bogus_compact_field"]
test result: FAILED. 0 passed; 1 failed; ... 25 filtered out
```

Reverted; the same test passes.

## Deviations from the plan, and why

- **Mechanism.** The plan preferred `#[schemars(schema_with = ...)]` annotations. The shared
  injection step is used instead: the fields are `Option<String>` precisely because they accept
  aliases a Rust enum cannot express, so the schema is already post-processed data, and the
  existing injection machinery only had to move out of the compact-only path. The plan's actual
  requirement — one vocabulary source, both profiles agree — is met and test-enforced.
- **A regression caught and corrected during implementation.** An early version gave
  `knowledge_brief` a nine-field compact allow-list, which it never had (it previously retained
  only required properties). That changed which fields compact hides, violating the milestone
  invariant, and showed up as +622 B. It is now `&[]`, reproducing the previous behaviour exactly.
- **`repo_search.goal` description** was rewritten rather than extended, because it duplicated the
  enum values in prose.

## Requirement-to-evidence mapping

| # | Acceptance criterion | Evidence |
|---|---|---|
| 1 | One allow-list per tool, owned by the tool | `compact_fields()` in `src/mcp/tools/mod.rs`; `server.rs` matches on no tool name |
| 2 | One vocabulary per tool, both profiles agree | `goal_and_research_domain_enums_match_between_profiles_and_runtime` compares full vs compact vs constant for all four field/tool pairs |
| 3 | Advertised values are exactly accepted values | Same test runs every advertised value through `parse_repo_goal`, `parse_research_goal`, `parse_security_goal`, and `ResearchDomain::parse` |
| 4 | A broken allow-list fails | Injected drift above |
| 5 | No budget grows | `search_four_schema_bytes_stay_slim`, `full_and_compact_advertisements_are_smaller_than_the_ten_tool_baseline`, `evidence_bundle_input_schema_stays_bounded` all pass with unchanged ceilings (the full-profile ceiling was later tightened by M002) |
| 6 | No behaviour change | `mcp_tool_contract` 22 passed, `tool_surface_evaluation` 5 passed, `make check` exit 0 |

## Residual risk

- Hidden legacy properties (roughly 27 `#[schemars(skip)]` fields) are unchanged and still accepted
  on the wire but not advertised. This milestone did not remove them.
- The enum injection is a schema post-process rather than source-level annotation, so a future
  generated-schema change could in principle land before the injection runs. The parity test
  compares the final advertised schema, so such a change fails loudly.

## Verification commands

```sh
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --test mcp_schema_slimming   # 25 passed
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --test mcp_tool_contract      # 22 passed
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --test tool_surface_evaluation # 5 passed
RUSTUP_TOOLCHAIN=1.89.0 make check                                                       # exit 0
```
