# ADR-0027: Container Documents in the Default Artifact Set

Status: accepted

Date: 2026-11-26

Decision owners: project maintainers

Accepted: 2026-11-26 by maintainer instruction ("the goal should be to ship this
as a default enabled feature, not gated")

Supersedes: ADR-0026 § Decision 2, the non-default clause only. That decision's
format detection, dependency set, bounds, outcome taxonomy, and determinism
contract are adopted unchanged.

Extends: ADR-0007 (the measured default-enablement template), ADR-0014 §4 (one
artifact per target), ADR-0024 §1 (single artifact alignment) and §5 (the size
review bar)

Related specification sections:

- `plans/000-long-term-specification.md` §4
- `plans/001-terminology-and-domain-model.md` §4-5

Affected subsystem roadmaps:

- `plans/subsystems/web-acquisition-expansion-roadmap.md` (M007, M008)
- `plans/subsystems/python-pypi-distribution-roadmap.md`

Related records: ADR-0005 (Python native boundary), ADR-0006 (keyless
deterministic retrieval), ADR-0011 (document addressing),
`plans/closure/web-acquisition-expansion/004-status.md`,
`plans/closure/web-acquisition-expansion/006-status.md`.

## Context

M004 shipped CSV, EPUB, DOCX, XLSX, and PPTX extraction behind a non-default
`documents` feature. ADR-0026 required that promotion to the default set be a
separate packaging change carrying size measurements, and M006 added the
operator switch and the capability triad without touching defaults. The result
is a complete capability that no published artifact carries.

In every default build and every published wheel today:

- `provider_status.documents_capabilities` reports `compiled: false` and
  therefore `configured: false`, `usable: false`;
- `[fetch].documents_enabled` defaults to `false` and is inert because the code
  is absent;
- a `.docx`, `.epub`, `.xlsx`, or `.pptx` URL returns
  `unsupported_content_type`, with a rebuild hint appended for DOCX and XLSX
  only (`fetch_tool_error`, `src/mcp/tools/web_fetch_options.rs`);
- a CSV URL returns the bounded table preview as ordinary text rather than a
  `document`;
- `docs/features.md` § Document Formats tells the reader to run
  `cargo build --features documents`.

ADR-0007 called exactly this shape a documentation defect rather than a lean
default: the product page advertises a format family the artifact cannot open.
The audience that needs it — standards bodies, procurement portals, filings,
shared drives — publishes DOCX, XLSX, and CSV as the normal case, and none of
those users has a Rust toolchain.

Three further facts shape the decision.

1. **The cost is now measurable.** At `v0.4.4` (public release assets, read
   2026-11-26) the seven targets publish 17,960,304 / 21,655,040 / 18,524,080 /
   17,710,392 / 19,779,368 / 25,869,312 / 21,457,240 bytes. Three of them
   already exceed ADR-0024 §5's 20 MB figure carrying `pdf` and `browser` and
   no container support, so that figure is a review trigger the project already
   passes with a stated reason, not an absolute ceiling.
2. **The producer needs no change.** The generated release workflow builds with
   neither `--features` nor `--no-default-features`, so default-set membership
   is the only lever. No `release/eggpack/` file and no generated workflow
   changes for this decision.
3. **The capability is not shippable yet.** An audit of the landed extraction on
   `40e362f` found nine defects, two of them high severity: XML character and
   entity references are dropped silently, so `Q&amp;A` extracts as `QA`; and a
   plain `<!DOCTYPE html>`, ordinary in EPUB 2 and 3 XHTML, is refused as
   malformed. Making the capability a default would ship those to every user.
   Milestone M007 owns them and closes first.

Forces in tension: one artifact per target with version-free asset names; a
Python wheel built from the same crate; an explicit preference for lean
defaults (ADR-0006); an equally explicit preference that a default be the happy
path and that a shipped default move be measured rather than asserted
(ADR-0007); and a documented policy against a second SKU (ADR-0006, ADR-0007,
ADR-0014, ADR-0024).

## Decision drivers

- One version, one artifact set: the CLI binary and the wheel advertise the same
  capabilities.
- A default must be the capability's correct path, not a refusing path.
- Every default move is measured on the exact candidate before publication.
- No new operator surface, config key, tool, argument, or wire string is
  introduced by enabling what already exists.
- A user who never fetches a container document must not acquire new files,
  network behaviour, or payload fields.

## Considered alternatives

1. **Keep `documents` non-default and document source builds (status quo).**
   Rejected. It preserves a lean default but leaves the published capability
   unreachable for every installed user, which is the state ADR-0007 rejected
   for PDF. It also keeps `provider_status` reporting a capability triad that
   is uniformly false, which is diagnostic noise rather than information.
2. **Add `documents` to the default feature set and to the wheel feature list,
   keeping `[fetch].documents_enabled` as an operator switch that defaults to
   the compiled state.** Selected. It is ADR-0007 alternative 2 applied to the
   container formats: one SKU, documentation matches the artifact, no packaging
   surgery, and the existing bounds and outcome contract are untouched.
3. **Compile the feature in and also remove `[fetch].documents_enabled`.**
   Rejected. It would be the only format family without an operator refusal
   path, it would change a documented config key and collapse the
   `configured` dimension of `documents_capabilities`, and it buys nothing the
   switch costs.
4. **Publish a second, documents-enabled asset per target.** Rejected on the
   same grounds as ADR-0007 alternative 3: it doubles the release matrix and
   contradicts version-free asset names.
5. **Load container support at runtime.** Rejected: it is alternative 2 or 4
   with more moving parts.
6. **Enable the default before fixing the audit findings.** Rejected. A default
   build would silently drop `&amp;` and refuse ordinary EPUB XHTML, which is
   worse than the current honest refusal.

## Decision

This ADR selects **alternative 2**, subject to the conditions in § Evidence gate.

1. `documents` joins the default feature set:
   `default = ["pdf", "browser", "documents"]` in `Cargo.toml`. `egress`
   stays outside the default set and `mock` stays test-only.
2. The Python wheel keeps `default-features = false` and adds the feature
   explicitly: `features = ["pdf", "browser", "documents"]` in
   `bindings/python/Cargo.toml`. One wheel per platform; no variant, no second
   project name.
3. `bindings/python/Cargo.lock` is regenerated. It is a tracked release input
   and currently contains neither `zip` nor `calamine`, so the `--locked`
   qualification build fails until it is.
4. `[fetch].documents_enabled` remains an operator switch and keeps its
   feature-derived default (`cfg!(feature = "documents")`,
   `src/core/config.rs:431` and `src/fetch/limits.rs:208`). Because the default
   is already derived from the feature, adding the feature to the default set
   flips the shipped default to `true` with no config change.
   `document_disabled` remains an explicit, documented outcome.
5. Membership in the default set is the only release lever. No
   `release/eggpack/` file, no generated workflow, no asset name, and no
   installer changes.
6. **Prerequisite.** Milestone M007 (extraction fidelity and bound correctives)
   closes before the default flips. Its `renderer_version` bump is what makes
   the flip safe: derived cache keys carry no feature bit, so without the bump
   a CSV URL cached by a feature-off build could be served as a plain table
   preview under a build that should return a `document`. Both milestones land
   in the same release, so the released artifact carries the bumped version and
   no reachable pre-bump entry.

### Artifact and configuration contract

| Concern | Before | After |
|---|---|---|
| `Cargo.toml` default | `["pdf", "browser"]` | `["pdf", "browser", "documents"]` |
| Wheel features | `["pdf", "browser"]` | `["pdf", "browser", "documents"]` |
| Wheel lockfile | no `zip`, no `calamine` | regenerated, both present |
| `[fetch].documents_enabled` default | `false` | `true` |
| `documents_capabilities.compiled` | `false` | `true` |
| `provider_status.server_capabilities.container_documents` | `false` | `true` |
| A DOCX/EPUB/XLSX/PPTX URL | `unsupported_content_type` | extracted content, or a typed `document_*` outcome |
| A CSV URL | bounded table preview as text | `document.format = "csv"` with parts |
| ODF, RTF, legacy binary Office | `unsupported_content_type` | `document_unsupported` |
| Release assets | 7 names | unchanged: the same 7 names |

## Evidence gate

Acceptance of the decision is not evidence of the move. Before the default
change is published, the candidate must record:

- the per-target size delta against the `v0.4.4` bytes above, on the exact
  candidate, with each target's absolute size stated rather than only the
  delta, and a written reason for any target whose absolute size grows past the
  20 MB review figure that three targets already exceed;
- the wheel size delta, following ADR-0024's measured form;
- `make check`, `make docs-check`, `make packaging-check`,
  `make dependency-policy`, and the MSRV 1.89 all-features build on the
  candidate;
- the hosted `Python native qualification` matrix green on the wheel feature
  change;
- M007's closure record, which is the precondition for any of the above.

If a target fails to build with the new default, or the wheel matrix fails to
qualify, stop and return to alternative 1 rather than introducing a second SKU.

## Consequences and tradeoffs

Advantages: the artifact matches the documentation; an agent stops hitting a
capability wall on ordinary office and data sources; `provider_status` reports
one coherent story; one qualification path; the size cost is measured per
target.

Costs and risks: every target and wheel grows by the measured delta; `zip` and
`calamine` move from the all-features graph to the default graph, so their
licence, advisory, and MSRV posture must pass `make dependency-policy` and the
1.89 default build rather than only `--all-features`; a container-parser
regression can now affect the default install, which raises the bar on M007
landing first; `bindings/python/Cargo.lock` becomes a mandatory regeneration
input; and the `documents` feature stops being a free way to keep the binary
lean for a source builder, who must now pass
`--no-default-features --features pdf,browser` to reproduce the old shape — a
shape that must keep compiling and keep its `unsupported_content_type` and
`document_disabled` behaviour.

Deferral: alternative 1 remains a complete and shippable position if M007 or the
measurement gate fails, and every M007 corrective is correct under either
alternative, which is why it is not gated on this decision.

## Compatibility and migration

Additive for configuration and protocol; genuinely changing for two default-path
behaviours, which is why they are named here rather than in a changelog only:

- A CSV fetch in a default build returns `representation: "document"` with
  `document.format`, `document.parts`, and `document.parts_total` instead of a
  table preview read as text. A caller that pattern-matched on extracted text
  for `.csv` sees a different shape.
- An ODF, RTF, or legacy binary Office URL returns `document_unsupported`
  instead of `unsupported_content_type`. After M007 both classify as
  `capability_unavailable` at the tool boundary, which is the intended class for
  a capability this build does not have.

Unchanged: nine advertised tools plus three aliases, every argument name, every
existing wire string, `pdf_not_compiled_in` and `document_disabled` in the
taxonomy for builds that disable the features, the trust framing, the SSRF and
redirect policy, byte caps, the seven release asset names, and the installer
contract. `unsupported_content_type` remains reachable for a body that is
neither text, HTML, PDF, nor a recognised container.

## Security implications

Container parsing stays in memory and never extracts to disk; actual inflated
bytes are counted by a bounded reader; no DTD subset, external entity,
`XInclude`, or external relationship is resolved; ZIP entry, ratio, part, cell,
character, and XML-depth bounds are unchanged by this decision. The exposure
that does change is breadth: every install now parses attacker-supplied
archives and spreadsheets by default, so the M007 bounds work is a security
precondition, not a tidy-up. M007 in particular closes a CSV structural
amplification in which a delimiter-dense body reaches roughly 30× its admitted
size, and the flip raises the worst case from "0 bytes parsed" to "the full
membrane under a default install".

## Verification and status

This ADR is **accepted** (maintainer instruction, 2026-11-26). Acceptance settles
the selection and supersedes ADR-0026 Decision 2's non-default clause. It does
not supply the evidence: the size measurements, the packaging and dependency
gates, the hosted wheel qualification, and M007's closure are conditions on
roadmap M008, which owns them. No production default changes on the strength of
this record alone.

## Supersession

This record supersedes ADR-0026 Decision 2's "non-default for the first release
that carries it" clause and nothing else. If the format set, the dependency
choice, or the bounds contract later changes, a new ADR supersedes that part of
ADR-0026 or this record and links to both.
