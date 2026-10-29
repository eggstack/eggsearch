# M007 — Expanded Live Retrieval and Consumer Acceptance

Status: active
Source: `plans/subsystems/retrieval-quality-evidence-extension-roadmap.md` M007
Primary class: infrastructure
Baseline: Eggsearch branch `plan/deterministic-retrieval-quality-20261029`; capture the exact final SHA before trials
References: closed M006 plan and closure, `docs/retrieval-quality-benchmark.md`, `docs/retrieval-quality-extraction-matrix.md`, `docs/retrieval-quality-codegg-handoff.md`, ADR-0006, `plans/003-planning-process.md`.
Dependencies: operational access to configured keyless providers, an actual SearXNG endpoint, Chrome, and PDF sources; CodeGG consumer run uses the explicitly authorized `opencode_go/muse-spark-1.3-contributor` route with Harness Lab's bounded controller policy.

## Objective and non-goals

Produce later, independently reviewable evidence for real retrieval quality and
CodeGG consumption. Do not amend or reopen closed M006, alter ranking/provider
selection, require network in CI, or interpret model success as a comparative
retrieval lift.

## Work packages

1. Create a versioned real-query corpus spanning distinct task categories with
   source identities and graded judgments. Either record two independent
   annotation passes and adjudication, or adopt a published human-judged corpus
   with documented provenance, label semantics, and a locked test partition.
   Keep questions public and non-personal. Store qrels, source URLs, retrieval
   constraints, citation/date checked, annotator provenance, disagreement and
   adjudication metadata; do not store copied source bodies.
2. Extend the opt-in observation runner to repeat a frozen query cohort across
   multiple bounded trials for each configured keyless provider, preserving
   provider attempt/outcome, duplicate and unique contributions, result hashes,
   cache caveats, timestamps, latency, and exact source/config fingerprints.
   Aggregate only within matched query/provider pairs and show denominators and
   variation. Keep errors and timeouts separate from zero-result outcomes.
3. Run an explicitly configured SearXNG instance as its own baseline arm, not
   a proxy for Eggsearch's metasearch or a simulated fixture. Record SearXNG
   version/configuration and enabled upstreams without secrets; require exact
   paired query IDs and multiple observations. If it cannot be reached, leave
   the arm blocked and do not substitute another provider.
4. Add paired, rights-safe HTML/JavaScript/PDF sources with declared expected
   passages, using the same URL/document identity and extraction budgets across
   HTTP-only, browser, PDF, and combined builds where applicable. Record Chrome,
   `lopdf`, build features, runtime enablement, extraction status, passage
   coverage, latency and resource data. Run browser cases only on a real
   provisioned browser and preserve public-network/SSRF controls.
5. Use the registered CodeGG Harness Lab case for repeated exact-task consumer
   trials. Preserve manifests, verifier revisions, tool-call counts, task
   outcomes, provider admissions and bounded model usage. Report full-palette
   vs core-palette schema bytes only if measured from CodeGG's actual schemas;
   upstream Eggsearch estimates alone are not consumer acceptance.
6. Publish a separate M007 report and closure evidence without overwriting
   M006's historical report. No algorithm tuning is in scope.

## Verification and acceptance

- Keep existing offline retrieval-quality and extraction tests green; run the
  canonical repo gate on the final exact candidate after changes.
- Live provider, SearXNG, browser/PDF, and model-backed runs are manual,
  opt-in, request/time/token/cost bounded, provenance logged, and never routine
  CI.
- Closure requires multiple valid repeated trials, independently annotated
  held-out cases, a real SearXNG result or an explicitly blocked milestone,
  paired extraction outcomes with unavailable arms labeled, repeated CodeGG
  task evidence, exact revisions/config fingerprints, and no unsupported
  superiority claim.
- Any production behavior change, stale held-out leakage, unsafe source
  capture, or unbudgeted provider/model use stops the run.
