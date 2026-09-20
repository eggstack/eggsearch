# Phase 21 — MCP Response Shaping and Discovery Caching

Status: implemented
Depends on: phase 19 benchmark conventions preferred
Baseline for planning: `205ab26fb03c6769035a1c05bb9b1f41c2a9ead1` (`main`)
Governing roadmap: `performance-optimization-roadmap.md`

## Why this phase exists

eggsearch already bounds MCP response size and supports compact/standard/diagnostic projections, but several shaping paths do unnecessary work on already-owned data.

The audited projection code clones JSON subtrees before immediately removing the originals. Batch focus injection serializes a typed `FetchDocument` into JSON, clones that JSON field, deserializes it back into `FetchDocument`, computes focus, and then serializes the focus result again.

Separately, the advertised MCP tool contract is static for a running binary, yet `tools/list` reapplies metadata, reconstructs output schemas, sorts the tool list, and the fingerprint path reserializes contract components each time it is requested.

These are small compared with external network latency, but they are repeated local CPU/allocation costs in agent-heavy sessions and can be removed without changing the tool surface.

## Objective

Reduce serialization, cloning, and repeated contract-construction work in MCP response/discovery paths while preserving byte-for-byte semantic compatibility of the public JSON structures.

The desired end state is:

- owned JSON projection moves removable subtrees instead of cloning them first;
- excerpt trimming mutates arrays in place without unnecessary take/reassignment cycles;
- compact fetch projection determines omitted link counts without cloning the links array;
- focus selection is computed from typed documents before/alongside JSON construction rather than deserializing cloned JSON;
- the static advertised tool definitions and contract fingerprint are cached for the lifetime of a server instance or process;
- compact/standard/diagnostic output contracts remain unchanged.

## Work item 1 — Add projection benchmarks and golden-equivalence fixtures

Extend the benchmark/evidence surface for realistic projection payloads.

At minimum benchmark:

- `web_search` compact projection with 10 and 50 source cards;
- `repo_search` compact projection with several groups and excerpts;
- `research_search` compact projection with grouped evidence and conflicts;
- `security_search` compact projection with vulnerabilities and references;
- `web_fetch` compact/standard projection with links and a structured document;
- `batch_fetch` projection with mixed web/repo responses.

Use representative payload sizes rather than tiny synthetic objects.

Add or expand golden/structural tests that compare the optimized output to the current expected JSON for all response-detail modes.

Performance tests must not replace contract tests.

## Work item 2 — Move projection metadata out of owned JSON objects

Refactor projection functions such as `project_web_search`, `project_repo_search`, `project_research_search`, and `project_security_search`.

The current pattern often does:

~~~text
value.get(key).cloned()
...
obj.remove(key)
~~~

Because the projection function owns the root `serde_json::Value`, prefer:

~~~text
obj.remove(key)
~~~

and retain the returned owned value for summary construction.

Requirements:

- diagnostic mode remains a true passthrough;
- standard mode removes exactly the same fields as today;
- compact mode produces exactly the same summary fields and values;
- null/missing behavior is unchanged;
- no use-after-borrow workarounds introduce a second clone elsewhere.

When several values must be removed before mutating the object, structure the borrow scopes explicitly rather than cloning to satisfy the borrow checker.

## Work item 3 — Trim excerpts and links in place

Review helpers in `src/mcp/projection.rs`.

For group/result excerpt trimming:

- mutate `Vec<Value>` arrays directly;
- avoid `mem::take` followed by immediate reassignment where no ownership transfer is required;
- preserve the exact `projection_excerpts_trimmed` flag semantics.

For compact `web_fetch`:

- determine `links_seen` from the existing metadata or array length;
- remove the owned `links` value directly;
- do not clone the entire links array solely to obtain its length.

Keep the implementation straightforward; do not introduce custom JSON tree types.

## Work item 4 — Compute web focus from typed FetchDocument

The web batch-focus path currently extracts `payload["document"]`, clones the JSON value, and deserializes it to `FetchDocument`.

Refactor the calling path so focus selection is computed while the typed `WebFetchResponse.document` or equivalent typed document is still available.

A preferred flow is:

~~~text
typed fetch response
  -> apply focus to Option<&FetchDocument>
  -> construct JSON response with focus already available
  -> projection
~~~

Requirements:

- no additional fetch/traversal is introduced;
- focus chunk/character caps remain unchanged;
- fetched=false and missing-document behavior remain unchanged;
- sanitization/trust boundaries remain unchanged;
- response field names and optional/null behavior remain unchanged.

If a shared helper is introduced, it should accept typed references and return the existing focus response type. Do not make it depend on MCP JSON.

## Work item 5 — Compute repo focus from typed repo document/content

Apply the same principle to repo batch focus where possible.

The current repo focus helper also deserializes a cloned JSON `document` field. Preserve existing fallback behavior when a typed structured document is unavailable and focus must operate on text/line content.

Do not weaken repo span, line-number, code-context, or trust metadata behavior.

If some path genuinely begins from an externally supplied JSON value rather than a typed response, keep a narrow compatibility helper there instead of forcing all callers through a JSON round trip.

## Work item 6 — Cache static advertised tool definitions

`EggsearchServer::list_tools` currently invokes:

~~~text
apply_contract_metadata(self.tool_router.list_all())
~~~

for each request.

The tool names, descriptions, annotations, input schemas, output schemas, and discovery metadata do not depend on runtime provider state.

Cache the fully decorated/sorted base tool contract at server construction or in a process-wide immutable cache.

Suitable shapes include:

- an `Arc<[rmcp::model::Tool]>` stored on `EggsearchServer`;
- a `OnceLock`/LazyLock for the static decorated list if construction is demonstrably independent of server state.

Requirements:

- `list_tools` returns the same ordered tools;
- output schemas remain attached exactly as today;
- tool descriptions/annotations remain sourced from `tool_contract`;
- no runtime provider state is frozen into the static list;
- future dynamic hydration, if present elsewhere, remains separate.

Do not change MCP pagination semantics.

## Work item 7 — Cache the contract fingerprint

The fingerprint is deterministic over the static tool contract and canonical discovery metadata.

Compute it once from the cached decorated contract and retain the resulting string/Arc string on the server or in the same immutable cache.

`tool_fingerprint()` must continue returning the same content fingerprint for an unchanged contract.

Add a regression test that independently recomputes the reference fingerprint from the advertised tools and proves the cached value matches.

Do not weaken fingerprint coverage merely to make caching easier.

## Work item 8 — Consider output-schema caching as an implementation detail

If the whole decorated tool list is cached, separate per-tool output schema caching may be unnecessary.

Do not add another cache layer unless:

- `output_schema_for` remains used independently in a meaningful hot path; and
- measurement shows repeated construction survives the tool-list cache.

If retained, prefer immutable LazyLock schemas rather than a mutex-protected map.

## Correctness invariants

Preserve:

- ten stable tool names;
- exact tool ordering;
- descriptions and annotations;
- input/output schema meaning;
- compact/standard/diagnostic projection semantics;
- response-detail stamps and projection telemetry flags;
- focus selection semantics and bounds;
- trust/sanitization markers;
- retrieval/conflict/routing/capability summary values;
- batch result ordering and budgets.

## Required tests

At minimum:

1. golden equivalence for compact/standard/diagnostic web search;
2. golden equivalence for repo/research/security projections;
3. web fetch compact links omission metadata equivalence;
4. batch mixed-item projection equivalence;
5. focus output equivalence before/after typed-path refactor;
6. missing/null document focus behavior;
7. advertised tool list equality across repeated calls;
8. deterministic tool ordering;
9. cached fingerprint equality with an independently recomputed reference;
10. no runtime provider/config state leaks into the static contract cache.

## Performance evidence

Record before/after Criterion results for the representative projection cases.

Add a benchmark or tight characterization for repeated `tools/list` construction/fingerprint access. Prefer measuring the internal decorated-contract builder directly rather than spinning up an HTTP server.

Expected improvement shape:

- fewer cloned JSON trees;
- no JSON deserialize cycle for typed focus paths;
- O(1)-ish repeated access to the static tool contract/fingerprint after initialization.

Do not require a fixed percentage improvement.

## Non-goals

Phase 21 does not:

- change response-detail defaults;
- remove fields from diagnostic mode;
- shrink schemas by changing their meaning;
- add new MCP tools;
- add binary serialization;
- replace serde_json;
- change focus ranking;
- change batch scheduling/budget allocation;
- change provider routing.

## Suggested execution order

~~~text
1. add representative projection/focus benchmarks and golden fixtures
2. replace clone-then-remove projection patterns with moves
3. simplify excerpt/link trimming
4. thread typed web document into focus computation
5. thread typed repo document/content into focus computation
6. cache decorated tool definitions
7. cache fingerprint
8. rerun MCP contract/schema/protocol tests
9. rerun benchmarks and record evidence
~~~

## Acceptance criteria

Phase 21 is complete only when:

1. realistic MCP projection benchmarks exist.
2. identified clone-before-remove JSON patterns are eliminated from the owned projection paths.
3. excerpt/link trimming avoids the audited unnecessary full-array copies.
4. normal typed web focus no longer clones and deserializes the serialized document JSON.
5. normal typed repo focus avoids the same round trip where typed data is available.
6. focus output and bounds are unchanged.
7. repeated `tools/list` does not rebuild/sort the static contract from scratch.
8. the contract fingerprint is cached and remains identical to the reference computation.
9. all ten stable tools, schemas, descriptions, annotations, and ordering remain unchanged.
10. compact/standard/diagnostic golden-equivalence tests pass.
11. targeted before/after benchmark evidence is recorded.
12. `make check` passes on the exact implementation candidate.

## Handoff notes

Keep this phase mechanical. The performance opportunity comes from ownership: these functions already own the JSON tree, and the fetch pipeline already has typed documents before serialization.

Do not use the campaign as a reason to redesign MCP envelopes or create a custom serialization layer. If an optimization makes the code substantially harder to read for a negligible benchmark win, record it as rejected.

## Implementation record

Implemented in performance candidate `5a8822ba538e89f9b8f441a328925fab78300754`. Owned projection paths now move metadata before removal, trim arrays in place, and omit compact links without cloning the link array. Batch web focus uses the typed cached/fetched `FetchDocument` when available and retains a narrow JSON fallback for compatibility-only inputs. `EggsearchServer` constructs and fingerprints its sorted decorated tool contract once per instance; repeated `tools/list` and fingerprint access reuse it.

Post-change Criterion characterization on Rust 1.98.1 / x86_64-apple-darwin: compact projection medians were 32.6 µs for 50 web cards, 33.3/33.1/33.9 µs for repo/research/security grouped payloads, and 12.6 µs for a 100-link compact fetch. Projection contract tests, focus/batch tests, tool-surface contract tests, and full gates passed. A separate output-schema cache was rejected because the decorated contract cache removes the repeated construction path.
