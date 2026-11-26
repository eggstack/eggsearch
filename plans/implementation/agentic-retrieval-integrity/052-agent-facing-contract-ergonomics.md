# M052 — Agent-Facing Contract Ergonomics

Status: closed
Date: 2026-11-26
Closed: 2026-11-26
Subsystem: agentic retrieval integrity
Milestone class: correctness (agent-facing contract ergonomics). WP1, WP3, and WP4 are polish-class interface refinements; WP2 is infrastructure-class contract uniformity and adds no retrieval capability.
Baseline: `f501a81` on `docs/user-facing-polish-20261123`
Closure: `plans/closure/agentic-retrieval-integrity/052-status.md`
Decision authority: ADR-0022 §3 ("A pointer that cannot be executed without reconstructing the locator is not acceptable"); ADR-0016 §1 and §5 (nine advertised tools, three callable aliases); `docs/tool-matrix.md` error taxonomy; M051 closure record (probes never write routing health).

References: `src/mcp/tools/canonical.rs`; `src/core/workflow_coverage.rs`;
`src/core/workflow.rs`; `src/core/repo_map.rs`; `src/meta/repo_mapper.rs`;
`src/meta/recipe_catalog.rs`; `src/mcp/tools/web_search.rs`;
`src/mcp/tools/repo_search.rs`; `src/mcp/tools/repo_map.rs`;
`src/mcp/tools/provider_status.rs`; `src/mcp/projection.rs`;
`src/meta/probe.rs`; `tests/mcp_tool_contract.rs`;
`tests/mcp_tools.rs`; `tests/repo_workflow.rs`;
`docs/tool-matrix.md`; `architecture/mcp.md`;
`architecture/codegg-contract.md`; `CHANGELOG.md`.

## Objective

Close the three agent-path gaps that survive M051 and are observable in the
shipped response contract. A post-M051 surface review measured the whole
published surface on a release binary and found that the path from a question
to an answer is already two calls, but that the **interface around those calls**
imposes avoidable rework:

1. **Vocabulary transfer is one-way.** `research_search` and
   `security_search` accept the `repo_search` goal spellings, but
   `repo_search` rejects theirs. An agent that has learned one tool's
   vocabulary receives `invalid_semantic_value` on the next.
2. **Follow-up pointers come in two shapes.** `next_actions` is the action
   envelope on four tools; `repo_map` returns none and publishes a separate
   typed `suggested_fetches` list instead. The agent must learn both.
3. **Pointers are not executable as published.** Response-time templates still
   carry `<search_query>`, `<owner>`, and `<repo>` placeholders whose
   values the response already knows, and one `repo_search` action names the
   `fetch` reason `fetch_architecture_doc` while targeting `web_search`.
4. **One advertised description contradicts the implementation.**
   `ProviderStatusArgs.probe` still says probes "update advisory health
   state" after M051 removed every health write.

This milestone closes those four defects without changing the tool inventory,
the alias boundary, the accepted argument set of any tool, the error-code
vocabulary, any provider, any default set, or any retrieval behavior. It
authorises **exactly one** re-pin of the advertised schema fingerprint, because
WP2 and WP4 change advertised schema content.

## Evidence (release binary, 2026-11-26, this egress)

| Observation | Measurement |
|---|---|
| Three `goal` vocabularies with one-way alias tolerance | `repo_search.goal` advertises 10 spellings; `research_search.goal` 13; `security_search.goal` 15. `parse_research_goal` and `parse_security_goal` both fall back to `parse_repo_goal`; `parse_repo_goal` never accepts the long workflow names |
| The transfer failure is reachable | `goal: "security_review"` is accepted by `research_search` and `security_search` and rejected by `repo_search`, where it must be `security` |
| Enumeration surface | 53 accepted enum values across 4 schema sites; `research_domain` advertises 15 values for 9 domains (6 aliases) |
| `repo_map` publishes no action envelope | Live `repo_map` on `BurntSushi/ripgrep` returned `next_actions: null` with `suggested_fetches` present; `repo_search` returns both fields in one response |
| `web_search` templates are unbound | Live response emits `{"items": {"type": "web", "url": "<selected_url>"}}` for `inspect_top_source` and a three-item batch for `fetch_multiple_focused` |
| `repo_search` templates are unbound where binding is free | Live `fetch_config` action emits `{"owner": "<owner>", "repo": "<repo>", "query": "<search_query>"}` although all three values are known from the request; `fetch_architecture_doc` names the `fetch` reason code but targets `web_search` |
| `knowledge_brief` publishes a non-callable pointer | `src/meta/knowledge_delta/execute.rs:910-929` hand-builds `next_actions` as raw JSON: the `fetch` action carries `{"url": ...}` although `fetch` requires `items`, and `broaden_evidence` carries an empty `input_template`. Neither passes through `sanitize_next_actions` |
| The advertised probe description is false | `src/mcp/tools/provider_status.rs:13-18` says probes "update advisory health state"; `src/meta/probe.rs:150-153` states they "never write routing health" |
| One pinned fingerprint covers all of it | `tests/mcp_tool_contract.rs:719-725` pins `3fa97faac725dc97` and 33,850 advertised bytes for full+full |

Placeholder sites: `src/core/workflow_coverage.rs:498-500`, `:509`,
`:545-547`, `:556`, `:655`, `:670`, `:691`, and
`src/meta/recipe_catalog.rs:116-284`. Recipe templates are static
documentation and are explicitly out of WP3's binding rule.

## Contract

- The nine advertised tools, three callable aliases, argument sets, and stable
  error codes are unchanged. No tool or alias is added, removed, or renamed.
- WP1 widens **accepted spellings only**. `REPO_GOAL_VALUES` stays the
  advertised enum, so no advertised value is added or removed.
- WP2 makes `next_actions` the one canonical follow-up contract. It does not
  remove `suggested_fetches`; that field stays as a typed, executable
  convenience and is documented as such. Removal would be a breaking change
  and needs its own ADR.
- WP3 binds a template field only when the response has exactly one value for
  it. A field whose value is the model's choice (selecting one of several
  candidate URLs) keeps its placeholder. A pointer that cannot be made
  executable is omitted rather than published with nothing to bind.
- WP4 changes description text only; it changes no accepted value, default, or
  behavior.
- Exactly one fingerprint re-pin lands in WP4 and absorbs every schema change
  from WP2 and WP4. The old and new fingerprint and byte count are recorded in
  the closure record before the pin is edited.
- Retrieval behavior, routing, provider selection, budgets, cooldowns, and
  trust framing are unchanged. Mode Off stays zero-remote-dispatch.
- Every bound value passes the existing sanitization tier for its field; no
  page body, credential, or local path is introduced into a pointer.

## Work packages

### WP1 — One `goal` vocabulary (S)

Change:

- Define `parse_repo_goal` as `WorkflowKind::parse(s).or_else(<existing short
  arms>)`. The short arms stay because `WorkflowKind::parse` has no
  `understand` or `debug` spelling, and those two are the most used
  `repo_search` goals.
- Make `parse_security_goal` delegate to `parse_repo_goal` (or delete it and
  call `parse_repo_goal` directly) so one implementation serves all three
  tools and cannot drift again.
- Keep `REPO_GOAL_VALUES` as the advertised list and keep `goal_error`
  listing it. The repair hint continues to name ten spellings.

Accepted widening, stated for the record: `repo_search.goal` will also accept
the ten `WorkflowKind` names plus the `api`, `error`, `research`, and
`comparative` aliases that `WorkflowKind::parse` already honours for
`security_search`. Each maps to a kind `repo_search` already reaches through
a short spelling, so no new workflow kind, coverage model, or default source
set appears.

Tests:

- A table test asserts every `WorkflowKind` variant's `as_str()` parses
  through `parse_repo_goal` to the same kind as its `REPO_GOAL_VALUES` short
  spelling.
- A `repo_search` test asserts `goal: "security"` and
  `goal: "security_review"` produce identical `sources` defaults and
  identical `resolved_hints_summary`.
- An error test asserts an unsupported `goal` still lists the ten advertised
  values and carries a repairable `invalid_semantic_value` code.

Acceptance: one spelling set is accepted by `repo_search`,
`research_search`, and `security_search`; the advertised enum, error text,
and repair hint are unchanged. **Stop:** if unification would require a new
`WorkflowKind` variant, a changed coverage model, or a changed default source
set, stop and register that as its own milestone.

### WP2 — One canonical follow-up pointer (M)

Change:

- Add `next_actions: Vec<AgentNextAction>` to `RepoMapResponse`
  (`src/core/repo_map.rs`), serialized with `skip_serializing_if =
  "Vec::is_empty"` like the surrounding additive fields.
- Build it in `src/meta/repo_mapper.rs` from the existing
  `RepoMapSuggestedFetch::to_batch_item()` conversion, so the envelope is
  `{"items": <batch_item>}`, `tool` is `fetch`, and `reason_code` comes
  from a closed two-value set (`fetch_important_file`, `fetch_repo_metadata`).
  `source_ids` is empty because `repo_map` has no source cards;
  `sanitize_next_actions` drops only an unknown tool name or an empty reason
  code and clamps priority, so an action with no source cards is accepted.
- Run the result through `sanitize_next_actions` before serialization, matching
  the other five tools, and cap at `MAX_NEXT_ACTIONS`.
- Converge `knowledge_brief` onto `AgentNextAction` and
  `sanitize_next_actions`, and correct its `fetch` template to the `items`
  shape. It is the only tool that hand-builds `next_actions` as raw JSON.
- Keep `suggested_fetches` unchanged. Document `next_actions` as the
  canonical follow-up and `suggested_fetches` as a typed convenience that
  hosts may use directly.

Tests:

- `tests/repo_workflow.rs`: a live-shaped `repo_map` response carries
  between one and five `next_actions`, each with `tool: "fetch"`, a non-empty
  `input_template.items`, and a `"type": "repo"` tag.
- A unit test asserts the generated template round-trips through
  `FetchArgs` deserialization without modification, proving it is directly
  callable.
- A projection test asserts `next_actions` survives `compact` and
  `brief` while `suggested_fetches` is unchanged.
- A guard test asserts no emitted pointer names `repo_fetch`,
  `web_fetch`, or `batch_fetch`.

Acceptance: every tool that suggests a follow-up (`web_search`, `repo_search`,
`security_search`, `research_search`, `knowledge_brief`, and now `repo_map`)
emits `next_actions` through `sanitize_next_actions`, and every emitted pointer
names an advertised tool. `suggested_fetches` stays available and typed, but
`next_actions` is the one canonical follow-up shape. **Stop:** if `repo_map`'s
suggestion set needs more than two reason codes, or if the conversion cannot
produce a callable `fetch` item for a suggestion, publish only the actions
that are callable and record the rest in `suggested_fetches` — do not widen
the reason-code vocabulary.

### WP3 — Executable follow-up templates (M)

Change:

- Add one binding helper next to the template builders that fills a template
  field from a known response value, and apply the rule below at every
  response-time construction site (`src/core/workflow_coverage.rs` and the
  tool-local builders in `web_search.rs` and `repo_search.rs`).
- Binding rule, applied per field:
  - `owner`, `repo`, `host`, `ref_name` — always bound from the
    resolved request.
  - `query` — bound from the request query verbatim (sanitized), never from
    a derived intent, so the pointer repeats what the caller asked.
  - `url` — bound only when the action targets exactly one candidate.
    Multi-candidate actions keep `<selected_url>` because the choice is the
    model's.
  - `path`, `symbol_or_query` — bound only when the response has one
    unambiguous value; otherwise keep the placeholder.
- Correct the `repo_search` `fetch_architecture_doc` action (the pair is
  defined at `src/core/workflow_coverage.rs:454` and is not pinned in any test or
  doc today): it targets
  `web_search`, so its reason code must read `search_architecture_doc`
  (a new wire string in an additive field, named in `docs/tool-matrix.md`),
  or the action must be omitted when no architecture document exists.
- Leave `src/meta/recipe_catalog.rs` templates untouched: recipes are static
  documentation of a multi-step workflow, not a pointer into a specific
  response.

Tests:

- `tests/mcp_tools.rs`: a live-shaped `web_search` response's
  `inspect_top_source` template contains the first result's URL, and its
  `fetch_multiple_focused` template still contains `<selected_url>`.
- `tests/repo_workflow.rs`: a `repo_search` `fetch_config` action contains
  the request's `owner`, `repo`, and `query`, with no angle-bracket
  placeholder in any bound field.
- `tests/mcp_tools.rs`: a `knowledge_brief` `fetch` action carries the
  `items` envelope, and its `broaden_evidence` action either carries the
  request topic as `query` or is omitted.
- A property-style test asserts no emitted `input_template` string value
  matches `^<[a-z_]+>$` for a field the builder was given a value for.
- A test asserts every emitted template still deserializes as the target
  tool's argument type.

Acceptance: a caller can execute any emitted pointer without inventing a value
the response already knew; every remaining placeholder marks a genuine choice.
**Stop:** if a builder cannot distinguish "one value" from "several" without a
plumbing change, bind nothing in that builder and record the gap rather than
guessing a value.

### WP4 — Accurate pinned description and the single re-pin (S)

Change:

- Rewrite the `ProviderStatusArgs.probe` documentation to state what M051
  implemented: probes run concurrently, follow `[search].timeout_ms` as their
  per-provider deadline, report success, failure class, latency, and
  `skip_code` only, never write routing health or start a cooldown, and
  report non-routable providers as skipped rather than failed. Keep it under
  the existing per-field style; `MAX_TOOL_DESCRIPTION_LEN` bounds the tool
  description, not this field.
- Recompute the full+full fingerprint and advertised byte count and update the
  pin at `tests/mcp_tool_contract.rs:719-725` once, absorbing WP2's output
  schema change and this description change. Record the old and new values in
  the closure record before editing.
- Add a `CHANGELOG.md` `Changed` entry naming the corrected probe
  semantics and the fingerprint change, so a host that caches `tools/list`
  by fingerprint knows to refresh.

Tests:

- The pin test passes with the new values.
- A test asserts the probe description contains no claim that a probe writes
  health state (string assertion over the advertised `inputSchema`), so the
  wording cannot silently regress.
- A test asserts the advertised tool count is still nine and the routed count
  is still twelve.

Acceptance: the model-facing description matches `src/meta/probe.rs`, the pin
is updated exactly once, and the surface still advertises nine tools.
**Stop:** if any other advertised description is found to be false during this
package, list it in the closure record and do not fix it here; open a follow-up
so one re-pin stays one event.

## Ordering and dependencies

1. WP1 (vocabulary) — independent, no schema change.
2. WP2 (pointer contract) — independent of WP1, changes `repo_map`'s output
   schema.
3. WP3 (binding) — depends on WP2 for the `repo_map` builder; otherwise
   independent.
4. WP4 (re-pin) — last, because it absorbs every schema change above.

Dependencies: no hard dependency outside the milestone; M051 is closed.
Interface dependencies: ADR-0022 §3 (pointer executability), ADR-0016 §1/§5
(inventory and aliases). Operational dependency: a release-binary live capture
for the evidence table, which this plan already records.

## Verification

Focused:

```bash
RUSTUP_TOOLCHAIN=1.89.0 cargo fmt --all
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --lib 'canonical::'
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --lib 'next_action'
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --features mock --test repo_workflow
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --features mock --test mcp_tools
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --test mcp_tool_contract
```

Broad (canonical gate):

```bash
RUSTUP_TOOLCHAIN=1.89.0 make check
RUSTUP_TOOLCHAIN=1.89.0 make docs-check
RUSTUP_TOOLCHAIN=1.89.0 make packaging-check
python3 packaging/check-planning-consistency.py
```

Live (release binary, opt-in, not part of `make check`): capture
`next_actions` for `web_search`, `repo_search`, and `repo_map`;
exercise `goal: "security_review"` against all three search tools; record the
payload sizes and the new fingerprint.

Baseline counts for the closure record: 6,652 all-features tests and 6,587
mock-feature tests at `f501a81`. The closure record states the new counts and
the delta rather than asserting an unchanged total.

## Documentation and guard updates

- `docs/tool-matrix.md` — `repo_search` `goal` row gains the accepted
  workflow spellings; the pointer contract states that `next_actions` is
  canonical and `suggested_fetches` is a typed convenience; the
  `provider_status` `probe` row matches the corrected semantics.
- `architecture/mcp.md` — pointer construction and the binding rule, stated
  once.
- `architecture/codegg-contract.md` — the fingerprint is re-pinned per
  advertising change, with old and new recorded in the closure record.
- `docs/test-inventory.md` and `architecture/testing.md` — the new tests
  and their suites, if a suite count changes.
- `skills/eggsearch-dev/SKILL.md` — only if a suite is added or renamed.
- `CHANGELOG.md` — `Fixed` (vocabulary transfer, pointer executability,
  probe description) and `Changed` (fingerprint).
- `AGENTS.md` — no change expected. The inventory, alias boundary, and
  ownership rules are unchanged; contributor detail belongs in
  `architecture/mcp.md`.

## Migration and compatibility

- `repo_search` accepts more spellings and rejects none it accepted before.
  No caller breaks.
- `repo_map` gains an additive output field. Consumers that validate
  responses against a closed schema must accept an unknown field, which the
  contract already requires for additive fields.
- `suggested_fetches` is unchanged and remains populated, so no consumer that
  reads it is affected.
- Bound templates contain real values instead of placeholders. A consumer that
  matched the literal placeholder string would change behaviour; no documented
  contract promises placeholder text, and the placeholder remains for
  multi-candidate actions.
- The fingerprint changes once. Hosts cache `tools/list` by content
  fingerprint plus server version (`docs/tool-matrix.md`), so the version
  bump and `CHANGELOG` entry are the migration signal. No tool name, schema
  property name, or accepted value is removed.

## Acceptance criteria

- One `goal` spelling set is accepted by all three search tools; the
  advertised enum is unchanged.
- Every tool that suggests a follow-up emits `next_actions`, and every emitted
  pointer names an advertised tool.
- No emitted pointer leaves a placeholder for a value the response already
  knew; every remaining placeholder marks a genuine model choice.
- The advertised `probe` description matches the implementation.
- The fingerprint is re-pinned exactly once, with old and new values recorded.
- Nine advertised tools, twelve routed names, unchanged aliases and error codes.
- `make check`, `make docs-check`, `make packaging-check`, and planning
  consistency pass on the candidate.

## Stop and split rules

- A package that needs a new tool, alias, argument, error code, provider, or
  default-set change is removed from this milestone and registered separately.
  This milestone is an ergonomics corrective, not a surface expansion.
- WP2 stops rather than removing `suggested_fetches`. Removal is a breaking
  change and requires its own ADR.
- WP3 stops rather than inventing a value. An unfillable field keeps its
  placeholder or the action is omitted.
- WP4 stops rather than folding a second description correction into the same
  re-pin; a second false description opens its own plan.

## Non-goals

- Item 5 of the surface review: defaulting `response_detail` away from
  `diagnostic` and projecting `fetch`'s `$defs` union under `compact`.
  That is a default-behavior change with its own measurement and belongs in a
  separate milestone.
- Removing `web_fetch`, `batch_fetch`, or `repo_fetch` from dispatch.
  ADR-0016 §5 and ADR-0022 §2/§4 keep them callable; retiring them is a
  compatibility break already declined twice.
- Merging any vertical tool (`research_search`, `knowledge_brief`,
  `security_search`, `repo_map`) into a general tool. ADR-0020 alternative 1
  and ADR-0016's rejection of mode-flag fetch payloads govern.
- Advertising the long goal spellings on `repo_search`. Accepted spellings
  are a parser concern; the advertised enum stays at ten so the schema and its
  repair hint stay small.
- Changing `research_domain`'s advertised alias list. Recorded here as a
  known redundancy, not this milestone's scope.
- Any change to retrieval quality, routing, providers, budgets, or trust.

## Risks

- Binding values into templates duplicates data already in `results[]`.
  The cost is bounded by `MAX_NEXT_ACTIONS` and is smaller than the cost of
  an agent fetching the wrong target.
- Template binding touches builders shared with `workflow_coverage` and
  evidence-gap actions; a wrong binding silently points at the wrong URL.
  The per-field rule and the round-trip deserialization test are the guard.
- `repo_map`'s empty `source_ids` is accepted today but is not exercised by
  any action; a sanitizer change could start dropping those entries. The guard
  test pins the behaviour.
- A fingerprint re-pin invalidates every host's cached `tools/list`.
  That is intended and must be released as a versioned change, not slipped into
  a patch.
- WP1's widening admits `api`, `error`, `research`, and `comparative`
  on `repo_search`. Each maps to an existing kind, but the accepted-spelling
  set and the advertised set diverge; the tool-matrix must say so once.

## Closure evidence

Per-package focused test output, the release-binary pointer capture, the
before and after fingerprint and byte count, the `make check` /
`make docs-check` / `make packaging-check` results, planning consistency,
the test-count delta, and the candidate SHA, recorded in
`plans/closure/agentic-retrieval-integrity/052-status.md`.
