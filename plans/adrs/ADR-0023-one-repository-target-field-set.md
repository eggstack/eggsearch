# ADR-0023: One Repository-Target Field Set Across Fetch Surfaces

Status: accepted

Accepted: 2026-11-22 (maintainer instruction to implement M001–M008 in full)

Date: 2026-11-22

Decision owners: project maintainers

Related decisions: ADR-0016 (unified fetch discovery and legacy alias compatibility),
ADR-0022 (nine-tool surface after `repo_fetch` retirement), ADR-0001 (adapter ownership)

Affected subsystem roadmap: `plans/subsystems/code-duplication-consolidation-roadmap.md`
M007

## Context and forces

A repository-target fetch is expressed by four Rust shapes today, and every one of them is
maintained by hand:

| Shape | Location | Fields | Fields beyond the common locator |
|---|---|---|---|
| `RepoFetchArgs` | `src/mcp/tools/repo_fetch.rs:24` | 20 | `host`, `timeout_ms`, `response_detail`, `test_fetch_url` |
| `FetchRepoItem` | `src/mcp/tools/fetch.rs:74` | 20 | the same four |
| `BatchFetchItem::Repo` | `src/core/batch_fetch.rs:67` | 20 | `host`, `focus`, `focus_max_chunks`, `focus_max_chars` |
| `RepoFetchRequest` | `src/core/repo_fetch.rs:124` | 18 | `host`, `timeout_ms` |

Sixteen fields are common to all four: `owner`, `repo`, `ref_name`, `commit_sha`, `path`,
`line_start`, `line_end`, `context_before`, `context_after`, `max_chars`, `symbol`,
`symbol_kind`, `match_text`, `expand_to_block`, `max_block_lines`, `prefer_local`.
The arithmetic is exact: 16 common plus the listed extras reproduces 20/20/20/18.

`host` is deliberately **not** in the common set. The three wire shapes carry `Option<String>`
because they accept the aliases `gh`, `gl`, and `cb` and must reject an unknown host with a
repairable tool error naming the accepted values (`CodeHost::accepted_aliases`).
`RepoFetchRequest` carries the parsed `Option<CodeHost>` because that is the execution form.
`CodeHost`'s serde representation is canonical snake_case with no aliases, and teaching it
aliases would replace the current validation message with a generic serde error. Keeping
`host` per shape preserves the diagnostic and the parsed execution type.

The consequence of the four copies is measured, not theoretical. The MCP tool-surface succession
workstream (M001 and M004) had to edit the same conceptual field set in the wire struct, the item
enum, the core request, the conversion match arms, the schema pins, and the documentation. A
contributor adding one repository-target argument still has four structs and at least three
mapping sites to find.

The forces are: (a) the `fetch` tool is advertised (ADR-0022), so its input schema is a durable
contract with a byte budget; (b) `repo_fetch` stays callable as an unadvertised alias with its
current request and response (ADR-0016 §5, ADR-0022 §4); (c) the envelopes are not
interchangeable — the batch item carries focus projection, the tool argument structs carry
response shaping and a test-only URL override, and the execution request carries a parsed host;
(d) `core` is the only layer all four may depend on.

## Considered alternatives

1. **Keep four independent shapes.** Rejected: every new field is a four-struct edit plus
   mappings, pins, and docs, and the copies have already drifted in field order and
   documentation.
2. **Make `BatchFetchItem::Repo` the one shape and drop the others.** Rejected: it would remove
   `timeout_ms` from the tool surfaces and the execution request, and add the focus trio to the
   alias and request, changing three contracts to fix a maintenance problem.
3. **Make `RepoFetchRequest` the one shape.** Rejected: it is execution-shaped and holds no focus
   projection, no response shaping, and no test-only override.
4. **Compose a common locator with `#[serde(flatten)]`.** Selected. One `RepoTarget` owns the
   sixteen common fields with their serde attributes, documentation, and `schemars`
   descriptions; each envelope keeps only its extras. The advertised `fetch` schema is measured
   before and after and must stay inside its existing per-tool budget.
5. **Emit the common field block with a declarative macro.** Viable fallback. It keeps the
   serialized schema byte-identical because expansion is textual, at the cost of a macro that
   carries serde and `schemars` attributes. Use it only if alternative 4 exceeds the budget.

## Decision

1. **`core::repo_fetch::RepoTarget` is the one definition of the common repository locator.**
   It carries the sixteen fields above with the documentation and serde attributes the four
   shapes use today, and derives `Default`, `Clone`, `Debug`, `Serialize`, `Deserialize`, and
   `JsonSchema`.
2. **`host` stays on each envelope** with its current type: `Option<String>` on the three wire
   shapes, `Option<CodeHost>` on the execution request. `RepoTarget` does not contain `host`.
3. **The envelopes compose the locator.** `RepoFetchArgs` carries `host` + `RepoTarget` +
   `timeout_ms` + `response_detail` + `test_fetch_url`; `BatchFetchItem::Repo` carries `host` +
   `RepoTarget` + the focus trio; `RepoFetchRequest` carries `host` + `RepoTarget` + `timeout_ms`.
   Fields are flattened into the same JSON object, so no caller sees a nesting change.
4. **`FetchRepoItem` is retired as a second 20-field type.** `fetch`'s repo variant holds
   `RepoFetchArgs`, the same type the alias deserializes. The advertised property names and
   accepted values do not change; only the Rust type name and `schemars` definition name do.
5. **One conversion owns locator-to-request.** `RepoFetchRequest` is built from `RepoTarget`
   plus `host` and the timeout override via one `From`/`TryFrom`; no surface re-maps the sixteen
   fields by hand. Validation special cases (workspace host, path rules, symbol kind) stay in
   the request type where they live today.
6. **`repo_fetch` keeps its callable contract.** Its request field names, accepted values, and
   response shape do not change; only the Rust type that holds the locator changes. The alias
   remains unadvertised and outside discovery (ADR-0016 §5).
7. **The advertised `fetch` schema is a measured budget, not an assumption.** The milestone
   records definition bytes, input-schema bytes, `$defs` count, and the serialized full-profile
   total before and after. If flattening exceeds the per-tool budget, the macro fallback in
   alternative 5 preserves this decision's field-set single-sourcing without the byte change.
8. **A parity test pins the field sets.** A JSON fixture containing every field round-trips
   through each envelope, and a key-set assertion proves each envelope's serialized keys equal
   `RepoTarget` keys plus its declared extras. Adding a field without updating every envelope
   fails the test.

## Consequences and tradeoffs

Advantages: one field list and one conversion for the repository locator; a new field is a
one-struct change plus the envelope that actually needs it; two 20-field wire types collapse into
one; the review's highest-blast-radius finding is closed.

Tradeoffs: `schemars` represents a flattened struct as `allOf`/reference, so the advertised
`fetch` and `repo_fetch` schemas may grow slightly and will change property ordering; the schema
pins in `tests/mcp_tool_contract.rs` and the budgets in `tests/mcp_schema_slimming.rs` must be
updated in the same commit; the tool argument structs gain one nested field in Rust construction,
which is why M001's `Default` derives should land first; `host` remains duplicated by design and
must be documented as the one intentional exception.

## Compatibility, security, and migration

No JSON field name, accepted value, or response field changes. No tool is added, renamed, or
retired. No configuration key or dependency changes. The alias contract in ADR-0016 and the
nine-tool surface in ADR-0022 are untouched. Security posture is unchanged: locator validation
(`validate_repo_path`, host allow-listing, SSRF policy) remains in the same functions, and the
alias diagnostics for an unknown host are preserved because `host` keeps its wire type.

## Verification and status

Acceptance of this ADR makes M007 dependency-ready; it does not change code. M007 records the
before/after schema measurements, the alias parity probe, the round-trip parity test, and the
full `make check` result in its closure record. Accepted 2026-11-22, so M007 is implemented in
the same change set as this acceptance record.
