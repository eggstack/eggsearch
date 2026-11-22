# ADR-0022: Nine-Tool Surface After Repository-Read Consolidation

Status: accepted

Date: 2026-11-22

Decision owners: project maintainers

Related specification sections: `plans/000-long-term-specification.md` §1, §2, §7

Related decisions: ADR-0001, ADR-0016, ADR-0020

Affected subsystem roadmap: `plans/subsystems/mcp-tool-surface-succession-roadmap.md`

## Context and forces

`fetch` accepts tagged `web` and `repo` items and executes both. A repo item carries every
field `repo_fetch` accepts (plus `focus`) and reaches the same
`core::repo_fetch::RepoFetchRequest` path (`src/mcp/tools/batch_fetch.rs:450` and `:472`).
Two advertised names therefore express one repository-read capability, and an agent must
choose between them before it knows whether a file is "structured" enough to deserve a
specialist.

ADR-0016 deliberately preserved `repo_fetch` as a specialist while replacing the two
advertised fetch entries with `fetch`. That decision was correct for the fetch-name
consolidation and did not examine the repo variant. The measured surface now shows
`repo_fetch` as the only remaining advertised tool whose capability is fully contained by
another advertised tool.

The canonical specification states that the inventory MUST remain exactly ten and that a
count or name change requires a later accepted ADR plus coordinated tool-matrix, contract
test, and CodeGG documentation updates. This record is that ADR.

## Considered alternatives

1. Keep ten and only slim schemas. Rejected: it leaves the duplicate capability and the
   routing ambiguity untouched, which is the larger cost.
2. Fold `repo_fetch` into `fetch` as a request mode. Rejected: `fetch` already discriminates
   by item `type`; a second discriminator adds a mode for no new capability.
3. Remove `repo_fetch` from dispatch entirely. Rejected: ADR-0016's alias precedent already
   gives callable-but-unadvertised compatibility, and removing dispatch would break existing
   callers and the Python convenience method for no surface benefit.
4. Remove `repo_fetch` from the canonical inventory, keep it callable as a finite alias, and
   point every follow-up at `fetch` repo items. Selected.

## Decision

1. After the owning milestone (M004) lands, the canonical advertised inventory is nine tools:
   `build_evidence_bundle`, `fetch`, `knowledge_brief`, `provider_status`, `repo_map`,
   `repo_search`, `research_search`, `security_search`, `web_search`.
2. `repo_fetch` remains callable with its current request and response contract. It is
   removed from `tools/list`, canonical metadata, presets, `next_actions`, SDK discovery, and
   release-smoke expected inventories, in the same way `web_fetch` and `batch_fetch` already
   are.
3. Generated follow-ups that currently name `repo_fetch` must name `fetch` and carry the repo
   item, using the existing `structured_repo_fetch_to_batch_item` conversion. A pointer that
   cannot be executed without reconstructing the locator is not acceptable.
4. Python and SDK callers may keep a `repo_fetch` convenience method and continue to reach it
   through the native execute path. Discovery lists report nine names.
5. No other tool, argument, accepted value, provider, configuration key, or dependency
   changes. Repository-read behaviour, workspace and forge validation, PDF handling, trust,
   and bounds are unchanged.

## Supersession and scope

This ADR supersedes only:

- ADR-0001's clause fixing the canonical inventory at exactly ten tools, and only to the
  extent the count becomes nine;
- ADR-0016 §1's retention of `repo_fetch` as a canonical specialist, and its clause fixing the
  advertised count at nine, which is nine here for a different reason;
- ADR-0020's clause fixing the count at ten after `knowledge_brief`.

Adapter ownership, the alias mechanism, fetch unification, and every other contract in those
records remain in force. Their files are not rewritten; on acceptance the owning milestone
appends a supersession pointer to each, following the practice already visible in ADR-0016.

## Consequences and tradeoffs

Agents face one repository-read choice and one argument grammar for single and multiple
targets. The advertised inventory loses one name and roughly 2.7 KB of input schema.
Follow-up suggestions become uniform: a `fetch` call with a repo item.

The cost is a coordinated multi-surface change: the canonical specification, `AGENTS.md`,
the tool matrix, contract tests, CodeGG documentation, release smoke scripts, the Python
facade, tool-surface fixtures, and corpus scenarios all name `repo_fetch` today. A partial
change leaves a projection disagreeing, which is why M003's parity guard precedes M004.

## Compatibility and migration

No alias is removed from dispatch. Existing `repo_fetch` requests and responses remain
structurally identical. Consumers that enumerate `tools/list` must migrate to `fetch` repo
items; consumers that call by name need no change. The response envelope differs between the
two entry points, and that difference is preserved rather than normalized: a migrated caller
receives a `results[]` item instead of a `RepoFetchResponse`.

## Verification and status

M004 must update every code-derived inventory and prove, on one candidate: nine advertised
names, a successful direct `repo_fetch` call, equivalent content for the same locator through
a `fetch` repo item, no `repo_fetch` in `next_actions` or SDK discovery, and passing
`make check`, `make docs-check`, packaging contract checks, and the Python suite.

This ADR is accepted on 2026-11-22. M004 implements it on the same candidate: nine
advertised names, a routed and callable `repo_fetch` alias, follow-ups that name `fetch`
with a repo item, and supersession pointers appended to ADR-0001, ADR-0016, and ADR-0020.
Acceptance itself changed no code; the milestone did.
