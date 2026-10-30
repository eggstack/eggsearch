# M007 — Cross-domain Retrieval, Extraction, and Resource Qualification

Status: closing
Source: `plans/subsystems/retrieval-quality-evidence-extension-roadmap.md` M007
Primary class: infrastructure
Baseline: merged Eggsearch candidate `5a99711`; capture exact source and binary identities per run
References: closed M006 comparison and closure, M007 implementation `001-expanded-live-and-consumer-acceptance.md`, `docs/retrieval-quality-benchmark.md`, `docs/retrieval-quality-extraction-matrix.md`, `docs/retrieval-quality-codegg-handoff.md`, ADR-0006, `plans/003-planning-process.md`.
Dependencies: public redistribution rights for dataset components; configured keyless providers and local SearXNG; real Chrome and PDF feature builds; bounded access to `opencode_go/muse-spark-1.3-contributor` for CodeGG acceptance.

## Objective and non-goals

Complete M007's remaining evidence gaps with a rights-attributed, multi-domain held-out query corpus; matched repeated keyless and SearXNG trials; paired HTTP/browser/PDF extraction cases including JavaScript-only content and a scanned PDF; resource measurements; and CodeGG schema/tool-call measurements where the harness exposes the exact serialized boundary. Keep all operational trials opt-in and bounded. Do not amend M006, change ranking/provider/fetch behavior, add network work to routine CI, or infer broad quality from unjudged live results.

## Work packages

1. Select at least three additional published held-out retrieval datasets spanning distinct tasks/domains. Verify each constituent dataset's query/qrel/title permissions rather than inferring permission from BEIR packaging; retain citations, archive/revision fingerprints, split and judgment semantics, attribution, and per-component license. Include only fields needed for title/identifier matching; do not copy passages or source bodies. Lock the test selection and document exclusions.
2. Extend the external runner to select reproducible strata across datasets and to report paired query/provider trials, successful empty responses, errors/timeouts, title/identifier matches, latency, duplicate/unique document contributions where observable, and per-dataset denominators. Keep the existing per-run attempt ceiling and provider allowlist. Run repeated bounded cohorts against available keyless providers and explicitly configured SearXNG; publish negative/unavailable outcomes without treating them as quality failures.
3. Add original, rights-safe JavaScript-only HTML and image-only scanned-PDF fixtures with declared markers and source hashes. Serve them only for manual qualification through an ephemeral public test origin accepted by Eggsearch's public-network policy; do not weaken SSRF/private-address controls or commit tunnel credentials. Compare the same public URL and budgets in HTTP-only, browser, PDF, and combined builds as applicable. Record unsupported OCR as a bounded absence outcome.
4. Measure cold and warm latency, time to first useful text, peak RSS, and executable/feature size for the paired static HTML, JavaScript-only HTML, text PDF, and scanned PDF cases. Record host/runtime/build identity and cache state; use repeated paired trials and medians/ranges, not thresholds. If reliable process-level resource attribution is unavailable, report it as unmeasured with the measurement boundary.
5. Extend the CodeGG Harness Lab acceptance with repeated task cases across retrieval categories on the already authorized `opencode_go/muse-spark-1.3-contributor` route. Measure actual serialized full/core tool-schema bytes only at an observed CodeGG/provider boundary; record tool discovery, fetch, bundle, provider admissions, per-case verification, token/cost budget and exact CodeGG/Eggsearch identities. If the harness cannot expose request schemas or provider attribution without changing production behavior, preserve the limitation and do not substitute Eggsearch-only estimates.
6. Publish updated evidence and a separate M007 closure record only after the acceptance criteria below pass. Keep the existing M006 artifacts and historical claims unchanged.

## Verification and acceptance

- The expanded corpus has at least four dataset/task families total, including SciFact, with source component license and attribution recorded and a fixed held-out test selection.
- At least three matched queries per added dataset receive two valid repeated observations for every provider arm reported as comparable. Provider errors and successful empty responses remain distinct. Small live samples carry no superiority claim.
- A reachable local SearXNG adapter arm is repeated with exact query IDs, configuration fingerprint, source revision and selected engine list.
- Static HTML, JS-only HTML, text PDF, and scanned PDF outcomes are paired across the supported feature arms; browser runs use real Chrome; OCR absence is explicit. SSRF/private-target guards remain green.
- Resource rows contain repeated paired latency and process/executable metrics or explicitly identify a measurement as unavailable. No CI threshold is introduced.
- The final merged candidate passes hosted Linux, macOS, and Windows CI; local macOS evidence alone does not close this cross-platform qualification gate.
- CodeGG acceptance has at least three successful verified turns across at least two registered cases/categories. Report schema bytes/provider attribution only if directly observed; keep model budget inside the existing operator-approved controller limit.
- Run `make check`, `make docs-check`, JSON/license/corpus validation, and `git diff --check` on the exact final source. Update registry, M007 roadmap, evidence report, and closure record together. M007 cannot close while any required unqualified work package is silently omitted.

## Stop conditions

Stop dataset import when component-level reuse rights are unclear. Stop public fixture exposure after the bounded trial and close the ephemeral origin. Stop model/provider runs at the controller budget, unrecognized route, or missing admission receipt. Stop browser trials on any private-target resolution or policy rejection; do not add a bypass.

## Progress — 2026-10-30

The four-family corpus and bounded cross-domain runner are implemented. SciFact,
Natural Questions, FEVER, and Touché 2020 are recorded in the fixture README;
Touché remains offline-only because opaque qrel IDs do not support safe live
matching. Three matched queries were selected in each of SciFact, NQ, and FEVER
and repeated twice for DuckDuckGo, Startpage, Yahoo, and the local SearXNG arm.
Per-run reports and SearXNG identity are under
`plans/closure/retrieval-quality-evidence-extension/evidence/`; aggregate
interpretation is in `docs/retrieval-quality-cross-domain-live.md`. This completes the corpus and repeated-provider work packages for the reported
arms. Paired HTTP-only, PDF-only, browser-only, and combined extraction arms are
recorded in `docs/retrieval-quality-extraction-qualification.md`; browser used
real Chrome and the scanned PDF explicitly returned the no-OCR classification.
Three CodeGG RFC 9110 turns and one FEVER turn passed independent verification;
the exact schema-byte and upstream-provider attribution boundaries remain
unavailable and are recorded in `docs/retrieval-quality-consumer-acceptance.md`.
Resource and extraction evidence is complete for this macOS source candidate.
M007 remains in closing status until the final merged candidate passes hosted
Linux, macOS, and Windows CI and a SHA-specific closure record is committed.
