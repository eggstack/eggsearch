# M042 Fetch Item Shape Diagnostics — Closure Evidence

Status: closed

Source plan: `plans/archive/implementation/agentic-retrieval-integrity/042-fetch-item-shape-diagnostics.md`

Roadmap: `plans/subsystems/agentic-retrieval-integrity-roadmap.md` M042

Corrective to: M029, closure `plans/closure/agentic-retrieval-integrity/029-status.md`.

Decision authority: accepted ADR-0001 (ten-tool surface) and accepted ADR-0016 (unified fetch
tool and legacy aliases). No new ADR.

## Scope

M042 corrects one defect: a malformed `fetch` item produced a serde implementation detail
instead of an actionable error. The correction is diagnostic only.

1. A fetch item with no `type` tag reports that the tag is missing and names both accepted
   shapes: `{"type":"web","url":"<url>"}` and
   `{"type":"repo","owner":"<owner>","repo":"<repo>","path":"<path>"}`.
2. A fetch item whose `type` is present but not `web` or `repo` names the accepted values and
   the accepted shapes.
3. The check applies to a single item and to every item of an array, keeping the existing
   per-item index prefix.
4. The existing actionable message for a tagged item missing a required field is unchanged and
   is now asserted byte-for-byte.
5. The accepted item shapes, the tag name, the advertised schema, and the tool contract are
   unchanged.

The paper-first contract is the `fetch` accepted-shape note in `docs/tool-matrix.md`.

## Exact code candidate

Implementation candidate `380a55cd7ea7c0b37dc6eff53851926e96e894d0` (parent
`688948b40394563315bbaea6ef9680cc94af3676`). Every command and probe below ran against
that tree. The candidate was rebased onto the concurrently drafted planning commit
`688948b` so that this milestone and the concurrently drafted knowledge-delta proposals share
one linear branch; the rebased candidate carries the same source, test, and documentation tree
as the tree that was qualified, and `688948b` touches planning documents only. This closure commit carries this
record, the plan status, the roadmap status, and the registry rows together.

## Requirement-to-evidence mapping

| # | Acceptance criterion | Evidence |
|---|---|---|
| 1 | A `fetch` item with no `type` tag returns an actionable error naming both accepted shapes | `src/mcp/tools/fetch.rs::validate_fetch_item_shape` gained a missing-tag branch. Test `fetch_item_tag_errors_name_accepted_shapes_for_scalar_and_array` asserts the message contains both shape literals. Live: `fetch item is missing the required `type` tag; accepted shapes: {"type":"web","url":"<url>"} and {"type":"repo","owner":"<owner>","repo":"<repo>","path":"<path>"}`. |
| 2 | A `fetch` item with an unknown `type` value returns an actionable error naming the accepted values | The unknown-tag branch names `web` and `repo` and repeats the accepted shapes. Live: `items[0]: fetch item has an unknown `type` tag; accepted values are `web` and `repo`; accepted shapes: ...`. |
| 3 | No fetch error message names a private Rust type | The serde untagged-enum fallback is no longer reachable for a missing or unknown tag; the test asserts the message does not contain `Repr`. The baseline message was `data did not match any variant of untagged enum Repr`. |
| 4 | A valid `web` item and a valid `repo` item behave exactly as before | The live probe returns the fetched document for a valid web item with an unchanged payload. `tests/mcp_tools.rs::fetch_schema_advertises_tagged_web_and_repo_items` asserts the advertised item schema still carries both tags. |
| 5 | The existing tagged-item field-level message is unchanged | Both the `repo` and the `web` field-level messages are now asserted with `assert_eq!` against the exact prior text, including the optional-field list. |
| 6 | The array form reports the offending index | The array case reports `items[1]` in the test and `items[0]` in the live array probe. |
| 7 | No new tool, argument, schema property, configuration key, or dependency | `tests/mcp_tool_contract.rs::fetch_schema_keeps_its_canonical_argument_properties` asserts the advertised `fetch` argument property set is exactly the prior nine names. `make check` static guards pass. |
| 8 | The broad gate passes with 0 failures | `make check` exit 0 on the candidate; `make docs-check` exit 0. |

## Live before and after

Probes used the candidate debug binary and the baseline `16b4c288` debug binary, MCP
`stdio`, and a pinned configuration. Each response body is quoted as returned.

| Call | Before (`16b4c288`) | After (`380a55c`) |
|---|---|---|
| `{"items": {"url": "https://example.com/"}}` | `failed to deserialize parameters: data did not match any variant of untagged enum Repr` | `failed to deserialize parameters: fetch item is missing the required `type` tag; accepted shapes: {"type":"web","url":"<url>"} and {"type":"repo","owner":"<owner>","repo":"<repo>","path":"<path>"}` |
| `{"items": [{"url": "https://example.com/"}]}` | `failed to deserialize parameters: data did not match any variant of untagged enum Repr` | `failed to deserialize parameters: items[0]: fetch item is missing the required `type` tag; accepted shapes: ...` |
| `{"items": [{"type": "bogus", "url": "https://example.com/"}]}` | `failed to deserialize parameters: data did not match any variant of untagged enum Repr` | `failed to deserialize parameters: items[0]: fetch item has an unknown `type` tag; accepted values are `web` and `repo`; accepted shapes: ...` |
| `{"items": [{"type": "repo", "owner": "eggstack"}]}` | `items[0]: repo item is missing the required `repo` field; accepted shape: {...} (optional: ...)` | identical |
| `{"items": [{"type": "web", "url": "https://example.com/"}]}` | fetched, `ok: true`, `chars_returned: 311` | identical |

Every malformed call stayed a repairable tool error (`isError: true`) on the same tool, and the
valid call was unaffected.

## Verification

Using the pinned Rust 1.89 toolchain on the candidate `380a55c`:

- `make check`: exit 0 — 90 suites, 6,520 passed, 0 failed, 28 ignored.
- `make docs-check`: exit 0.
- Focused: `mcp_tools` 29, `mcp_tool_contract` 22, `lib` 3,947.
- `cargo fmt --all -- --check` and `git diff --check`: clean.

## Known limitations and residual risk

1. The `batch_fetch` legacy alias has its own `BatchFetchArgs` path and was not changed. If it
   surfaces the same item contract through a different payload contract, that is separate work
   rather than a widened milestone; the plan scoped it out.
2. The error remains an invalid-params-class deserialization error rather than a structured
   `repair` object. That is the contract-correct outcome for an uninterpretable invocation
   shape, and changing it would alter the tool contract.
3. A caller matching the exact old string is not a supported contract; the old string named a
   private Rust type.
4. The message names the accepted shapes rather than a machine-readable field-level repair, so
   an agent must still read the message to learn which field was wrong. No new argument or
   schema property was added.

## Compatibility and migration

No tool, argument, schema, or configuration change. Behaviour change: a malformed item reports
a different, more actionable message on the same error channel. No index, storage, or
dependency change; no migration.

## Hosted qualification

The candidate is pushed to `origin/feat/auto-20261117-044c4885` for the hosted pipeline.
Hosted CI was not a requirement of this milestone plan, so the local gate above is the
closure evidence; the hosted run is recorded here when it completes.
## Recommendation

Accept M042 on `380a55cd7ea7c0b37dc6eff53851926e96e894d0`. Every acceptance criterion has
deterministic test evidence, the live before/after responses are recorded, and the residual
limits are explicit.
