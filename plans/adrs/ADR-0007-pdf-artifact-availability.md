# ADR-0007: PDF Ingestion in Distributed Artifacts

Status: accepted
Date: 2026-11-11
Decision owners: project maintainers
Affected subsystems: fetch and extraction; binary distribution, install/update, and deployment; Python native library and wheel distribution; packaging; documentation.
Related: `plans/000-long-term-specification.md` §1, 3, 5, 7; `plans/001-terminology-and-domain-model.md`; `plans/003-planning-process.md`; `plans/adrs/ADR-0001-ten-tool-surface-and-adapter-ownership.md`; `plans/adrs/ADR-0006-keyless-deterministic-retrieval-and-optional-extraction.md`; `plans/subsystems/pdf-ingestion-fidelity-roadmap.md`.

## Context and forces

ADR-0006 records that baking PDF into the default release binary was considered and **not selected**, on the grounds that existing feature and release policy forbids multiple SKUs and values the lean default. The default feature set is empty; the published distribution requests no feature flags, so every released binary and every default-feature build answers `pdf_not_compiled_in` to any PDF fetch.

Three facts now press on that decision.

1. `docs/features.md` advertises PDF extraction as a product capability with an `[fetch].pdf_enabled` knob. A capability the artifact cannot perform is a documentation defect, not a lean default.
2. Agents encounter PDFs constantly in practice: standards, RFCs, papers, filings, vendor documents. `classify_link` already tags PDF targets in HTML link extraction, so the product visibly points at documents it cannot open.
3. The cost is measured, not speculative. `docs/retrieval-quality-extraction-qualification.md` records the PDF arm at 20,659,408 B against 19,310,288 B HTTP-only: **+1.35 MB, +7%**. Neither arm carries browser, so the comparison is clean.

Forces in tension: a single binary per target with version-free asset names; seven published targets including `armv7-unknown-linux-gnueabihf`; Python wheels built from the same crate; an explicit preference for lean defaults; an equally explicit preference that defaults be the happy path; and a documented policy that a shipped default change requires its own measured acceptance rather than an assertion.

The second gate matters as much as the first. `[fetch].pdf_enabled` defaults to `false`, so compiling the feature in without changing that default would produce a binary that can do PDFs but refuses them — the worst of both states, because the operator now has no error to act on.

## Considered alternatives

1. **Keep PDF as an optional source-build capability, and make the refusal actionable and honest.** Lowest risk and preserves the current artifact graph, but installing users still cannot ingest PDFs without a Rust toolchain, and the advertised capability stays unavailable.
2. **Add `pdf` to the default feature set and default `[fetch].pdf_enabled = true`.** One SKU, documentation matches the artifact, no packaging surgery, every target and wheel gains the capability at a measured +7% size cost. Supersedes ADR-0006's PDF-in-default clause.
3. **Publish a second PDF-enabled asset per target.** Preserves the lean default but doubles the release matrix and asset names, contradicts the version-free asset-name contract, and ADR-0006 already rejected the multiple-SKU shape. Not selected.
4. **Load PDF support at runtime rather than compile time.** Requires either a native library or a second binary, so it is one of alternatives 2 or 3 with more moving parts. Rejected.
5. **Compile the feature in but leave `pdf_enabled = false`.** Produces a binary that can ingest PDFs and refuses to. Rejected as incoherent.

## Decision

If accepted, this ADR selects **alternative 2**, subject to its verification gate:

- `pdf` joins the default feature set. `browser` stays optional; nothing in this decision changes browser availability or policy.
- When the `pdf` feature is compiled in, `[fetch].pdf_enabled` defaults to `true`. The setting remains an operator switch for refusing PDF ingestion, and `pdf_disabled` remains an explicit, documented outcome.
- One binary per target, version-free asset names, and the rest of the release contract are unchanged.
- Acceptance requires a recorded measured comparison on the exact candidate: size delta per target, the existing extraction qualification re-run, installer smoke on each platform, and the wheel build matrix. If `armv7`, a Windows target, or the wheel matrix fails to build or qualify, **stop and return to alternative 1** rather than introducing a second SKU.

Before acceptance, alternative 1's corrective work is still required and is owned by roadmap M001: a PDF-size budget distinct from the generic response cap, a truncation outcome that reports a size problem instead of a parse error, scanned-versus-blank classification without requiring `include_media`, and an actionable `pdf_not_compiled_in` message. Those changes are correct under either alternative, which is why they are not gated on this decision.

## Consequences and tradeoffs

Advantages: the artifact matches the documentation; agents stop hitting a capability wall on ordinary sources; no second SKU; a single qualification path; the size cost is known and bounded.

Costs and risks: every target and wheel grows by roughly 7%; `lopdf` joins the default dependency graph, so its licence, advisory, and MSRV posture must pass `make dependency-policy` and the MSRV all-features check on the default path rather than only under `--all-features`; the ARMv7 and Windows portability jobs must stay green with the new default; and a PDF regression can now affect the default install, which raises the bar on roadmap M001 through M005 landing first.

Deferral: if maintainers prefer to keep the lean default, alternative 1 remains a complete and shippable position, and roadmap M001-M005 still apply unchanged to source-build users.

## Compatibility, security, and migration

Additive only. No MCP tool, tool argument, or wire string changes. `pdf_not_compiled_in` remains in the taxonomy for source builds that disable the feature. Existing `[fetch].pdf_enabled = false` configurations keep working and keep refusing PDFs explicitly. Sanitization tiers, trust framing, SSRF and redirect policy, byte caps, and the ten-tool contract are untouched. Cached derived documents must be invalidated when extraction output changes; the roadmap's milestones own the `renderer_version` bump.

## Verification and status

This ADR is `accepted` (maintainer decision, 2026-11-11). Acceptance settles the selection and supersedes ADR-0006's clause keeping `pdf` out of the default release binary; it does not supply the evidence. The recorded per-target size delta, re-run extraction qualification, installer smoke, wheel matrix, `make dependency-policy`, the MSRV all-features check, and the full canonical gate on the exact candidate are conditions on roadmap milestone M006, which owns them. No production default changes on the strength of this record alone.
