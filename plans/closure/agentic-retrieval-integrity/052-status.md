# M052 — Agent-Facing Contract Ergonomics: Closure Record

Milestone: M052
Status: closed
Date: 2026-11-26
Candidate: the working tree applied to `f501a81` (commit separate)
Plan: `plans/implementation/agentic-retrieval-integrity/052-agent-facing-contract-ergonomics.md`
Roadmap: `plans/subsystems/agentic-retrieval-integrity-roadmap.md`

## Result

Four bounded packages closed one agent-path defect each. No tool, alias,
argument, provider, default set, or error code was added, removed, or renamed.
One advertised-fingerprint re-pin covers the two schema changes, and the
corrected probe description.

| Package | Change | Outcome |
|---|---|---|
| WP1 — one `goal` vocabulary | `parse_repo_goal` tries `WorkflowKind::parse` first and keeps its short arms; `parse_security_goal` delegates to it | A goal learned on `research_search` or `security_search` is accepted by `repo_search` |
| WP2 — one canonical follow-up pointer | `RepoMapResponse.next_actions` added, built from the existing `to_batch_item()` conversion; `knowledge_brief` converged onto `AgentNextAction` | Six tools that suggest follow-ups now publish `next_actions`; `suggested_fetches` stays populated |
| WP3 — executable templates | `NextActionBindings` + `bind_next_actions` applied at the MCP tool layer; `fetch_architecture_doc` renamed to `search_architecture_doc` | Templates carry the values the response already knows; placeholders remain only for a genuine model choice |
| WP4 — accurate description and one re-pin | `ProviderStatusArgs` `probe` documented as implemented; pin updated once | The model-facing description matches `src/meta/probe.rs` |

## Evidence per package

### WP1

- `parse_repo_goal` = `WorkflowKind::parse(s).or_else(<short arms>)`; the short
  arms remain because `WorkflowKind::parse` has no `understand`, `debug`, or
  `compare` spelling and no hyphenated `pre-change`/`post-change` variants.
- Three tests: all ten workflow-name/short-spelling pairs resolve identically;
  every `REPO_GOAL_VALUES` entry plus the hyphenated variants still parses; an
  unsupported value is rejected.
- `REPO_GOAL_VALUES`, `goal_error`, and every default source mapping are
  unchanged, so the advertised enum and repair hint stay at ten spellings.
- Live: `repo_search` with `goal: "security_review"` returned results with no
  `invalid_semantic_value`.

### WP2

- `RepoMapResponse.next_actions` is serialized with `skip_serializing_if` like
  its neighbours; `build_repo_map_next_actions` maps each suggestion through
  `to_batch_item()` into `{"items": <repo item>}` targeting `fetch`, then
  `sanitize_next_actions` bounds the list.
- `knowledge_brief` now builds `AgentNextAction` values: the `fetch` action
  carries `{"items": {"type": "web", "url": <cited url>}}` instead of the
  rejected bare `{"url": ...}`, and `broaden_evidence` carries the request
  topic instead of an empty template. Both pass through `sanitize_next_actions`.
- Tests: crate unit tests assert every template deserializes as `FetchArgs`
  and that follow-ups stay within `MAX_NEXT_ACTIONS` and name only advertised
  tools; an integration test asserts the action shape and that an action with
  no source cards survives sanitization; a projection test asserts
  `next_actions` and `suggested_fetches` both survive `compact` and `brief`.
- Live: `repo_map` on `BurntSushi/ripgrep` returned one suggestion and one
  matching action; a 15-entry call with no classified important file returned
  neither, which is the documented empty case.

### WP3

- `NextActionBindings` and `bind_next_actions` live in `core/workflow.rs`.
  Binding runs at each MCP tool seam immediately before serialization, where the
  request is in scope, so no `meta` builder signature changed.
- Per-field rule: `owner`/`repo`/`host`/`ref_name` from the resolved target;
  `query` (and `research_question`, `vulnerability_or_package`) verbatim from the
  request; `url` only when the action names exactly one source card;
  `path`/`symbol_or_query` only when unambiguous.
- `fetch_architecture_doc` → `search_architecture_doc`, because the action
  targets `web_search`. The pair is defined at
  `src/core/workflow_coverage.rs:454` and was not pinned elsewhere.
- Tests: four binding unit tests (repo placeholders, single-candidate URL,
  multi-candidate placeholder, unknown/absent values), a composition test over
  real gap-driven actions, and an assertion that the architecture gap names a
  search reason.
- Live `web_search`: `inspect_top_source` carried the real URL;
  `fetch_multiple_focused` kept `<selected_url>` across three candidates.
- Live `repo_search`: `fetch_config` and `fetch_manifest` carried
  `owner: "BurntSushi"`, `repo: "ripgrep"`, and the caller's query verbatim.

### WP4

- The advertised `probe` description now states that probes follow
  `[search].timeout_ms` and never write routing health or start a cooldown.
- One re-pin covers WP2's output-schema field and this description:

| Value | Before | After |
|---|---|---|
| full+full fingerprint | `3fa97faac725dc97` | `678e2a042ab5b39b` |
| full+full advertised bytes | 33,850 | 33,983 |
| `tool_surface_live` byte delta from the M029 baseline | −44,239 | −44,106 |
| `tool_surface_live` token delta from the M029 baseline | −11,060 | −11,026 |
| compact+full fingerprint | `6e6a6ea7fee6e9f5` | unchanged (21,542 bytes) |

- `CHANGELOG.md` carries a `Changed` entry naming both fingerprints so a host
  that caches `tools/list` refreshes.
- `tests/mcp_tool_contract.rs` gains `probe_description_never_claims_a_health_write`,
  a string assertion over the advertised schema so the wording cannot silently
  regress. The contract suite is 23 tests.

## Documentation

- `docs/tool-matrix.md` — new § Follow-up pointers (canonical `next_actions`,
  the binding table, the retained typed `suggested_fetches`), and the accepted
  `repo_search` goal spellings.
- `architecture/mcp.md` — § Follow-up pointers: envelope, binding rule,
  sanitization, and the tool-layer seam.
- `architecture/codegg-contract.md` — the fingerprint is re-pinned
  deliberately, one milestone per event, with old and new recorded.
- `CHANGELOG.md` — `Changed` and `Fixed` entries.

## Gates

| Gate | Result |
|---|---|
| `make check` | pass |
| `make docs-check` | pass |
| `make packaging-check` | pass |
| `python3 packaging/check-planning-consistency.py` | `planning-consistency: ok` |
| `git diff --check` | clean |

Test inventory: 6,669 all-features and 6,604 mock-feature tests, up 17 from
6,652 / 6,587 at `f501a81`.

## Deviations

1. **One reason code, not two.** The plan allowed a closed two-value set
   (`fetch_important_file`, `fetch_repo_metadata`). `RepoMapSuggestedFetch`
   carries no origin discriminator after construction, so every action uses
   `fetch_important_file`. The set stays closed; the second value would have
   required a heuristic on the path.
2. **Binding lives at the tool layer.** The plan named the builder sites. The
   request is only in scope at the MCP tool seam, and binding there covers every
   builder without changing `meta` signatures.
3. **The round-trip proof is a crate unit test.** `FetchArgs` is
   `pub(crate)`, so the deserialization proof sits beside the builder rather
   than in an integration test. The integration test asserts the item shape.
4. **A second pinned figure moved.** `tests/tool_surface_live.rs` pins the
   candidate's byte and token delta from the M029 baseline; both shifted by the
   same 133 bytes. It is part of the same re-pin, not a separate event.
5. **`fetch_multiple_focused` binds `focus`, not `url`.** The multi-candidate
   action keeps `<selected_url>` because the candidate is the caller's choice,
   but its `focus` key is bound to the caller's query.
6. **`build_evidence_bundle` keeps three placeholders.**
   `<fetched_items>`, `<research_goal>`, and `<source_cards>` describe evidence
   the caller has already collected; the response cannot supply them, so the
   stop rule applies and they stay placeholders.

## Not verified

- No release-binary live run for this milestone. The pointer capture used the
  debug binary, which is sufficient for payload shape; no timing claim is made.
- No live `security_search` or `research_search` capture. Their seams are the
  same helper and are covered by the binding unit tests.
- The plan's `tests/mcp_tools.rs` web-search case is not a separate integration
  test: the single-candidate and multi-candidate URL rules are covered by the
  `core/workflow.rs` binding unit tests, and the live capture shows the real
  bound URL and the retained placeholder. The `goal` equivalence case is covered
  by `security_and_security_review_resolve_identically` in `canonical.rs`.
- A live 429, a live challenge wall, and other network-conditional states are
  outside this milestone's surface.
